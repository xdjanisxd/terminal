# Native Windows release diagnostics; changes only generated files and child environments.
param(
    [ValidateRange(1, 100)][int]$Runs = 7,
    [string]$OutputDirectory,
    [string]$PowerShellProgram,
    [string]$ConfiguredConfig,
    [switch]$RequireConfiguredPrompt,
    [switch]$RequireGracefulShutdown
)
$ErrorActionPreference = 'Stop'
$repository = Split-Path $PSScriptRoot -Parent
$executable = Join-Path $repository 'target/release/terminal.exe'
if (!(Test-Path -LiteralPath $executable)) { throw 'Build cargo build --release -p terminal-app first.' }
if (!$OutputDirectory) {
    $OutputDirectory = Join-Path $repository ("target/pty-startup-" + [Guid]::NewGuid().ToString('N'))
}
$null = New-Item -ItemType Directory -Path $OutputDirectory -Force
$OutputDirectory = (Resolve-Path -LiteralPath $OutputDirectory).Path
if ($PowerShellProgram) {
    if (!(Test-Path -LiteralPath $PowerShellProgram -PathType Leaf)) { throw 'PowerShellProgram must name an existing executable.' }
    $powerShell = [pscustomobject]@{ Source = (Resolve-Path -LiteralPath $PowerShellProgram).Path }
} else {
    $powerShell = Get-Command pwsh.exe -ErrorAction SilentlyContinue
    if (!$powerShell) { $powerShell = Get-Command powershell.exe -ErrorAction SilentlyContinue }
}
if ($ConfiguredConfig) { $ConfiguredConfig = (Resolve-Path -LiteralPath $ConfiguredConfig).Path }
$commandShell = Get-Command cmd.exe -ErrorAction SilentlyContinue
$hook = [IO.File]::ReadAllText((Join-Path $repository 'crates/app/src/powershell_osc7.ps1'))

# JSON string escaping is compatible with TOML basic strings, including embedded newlines.
function Write-ShellWorkspace($Name, $Program, $ShellArguments) {
    $encodedProgram = ConvertTo-Json -InputObject ([string]$Program) -Compress
    $encodedArguments = ($ShellArguments | ForEach-Object {
        ConvertTo-Json -InputObject ([string]$_) -Compress
    }) -join ', '
    $configuration = @"
[workspace]
active_tab = 0
[[workspace.tabs]]
title = "Startup measurement"
[workspace.tabs.layout]
kind = "pane"
[workspace.tabs.layout.session.command]
program = $encodedProgram
args = [$encodedArguments]
"@
    [IO.File]::WriteAllText((Join-Path $OutputDirectory "$Name.toml"), $configuration)
}

$scenarios = @('configured')
if ($powerShell) {
    Write-ShellWorkspace 'profile' $powerShell.Source @('-NoExit', '-Command', $hook)
    Write-ShellWorkspace 'no-profile' $powerShell.Source @('-NoProfile', '-NoExit', '-Command', $hook)
    $scenarios += @('profile', 'no-profile')
}
if ($commandShell) {
    Write-ShellWorkspace 'cmd' $commandShell.Source @()
    $scenarios += 'cmd'
}
$failed = $false
$previousDiagnostics = $env:TERMINAL_STARTUP_DIAGNOSTICS
$previousLog = $env:TERMINAL_STARTUP_LOG
$previousConfig = $env:TERMINAL_CONFIG
$forcedShutdowns = 0
try {
    $env:TERMINAL_STARTUP_DIAGNOSTICS = '1'
    if ($ConfiguredConfig) { $env:TERMINAL_CONFIG = $ConfiguredConfig }
    foreach ($run in 1..$Runs) {
        foreach ($scenario in $scenarios) {
            $log = Join-Path $OutputDirectory "$scenario-$run.log"
            # Remove only this exact generated log so stale readiness cannot end a launch.
            if (Test-Path -LiteralPath $log) { Remove-Item -LiteralPath $log }
            $env:TERMINAL_STARTUP_LOG = $log
            $launchArguments = @()
            if ($scenario -ne 'configured') {
                $workspace = Join-Path $OutputDirectory "$scenario.toml"
                $launchArguments = @('--workspace', ('"' + $workspace + '"'))
            }
            $launch = @{ FilePath = $executable; PassThru = $true; WindowStyle = 'Hidden' }
            if ($launchArguments.Count -gt 0) { $launch.ArgumentList = $launchArguments }
            $child = Start-Process @launch
            $deadline = [Diagnostics.Stopwatch]::StartNew()
            $rendered = $false
            $prompt = $false
            try {
                while ($deadline.Elapsed.TotalSeconds -lt 20 -and !$child.HasExited) {
                    if (Test-Path -LiteralPath $log) {
                        $stream = [IO.File]::Open($log, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
                        $reader = [IO.StreamReader]::new($stream)
                        try { $text = $reader.ReadToEnd() } finally { $reader.Dispose() }
                        $rendered = $text.Contains('stage=first-shell-output-rendered')
                        $prompt = $text.Contains('stage=prompt-ready')
                        if ($rendered -and ($prompt -or $scenario -eq 'cmd' -or ($scenario -eq 'configured' -and !$RequireConfiguredPrompt))) { break }
                    }
                    # Harness polling only; no sleep is introduced in application startup.
                    Start-Sleep -Milliseconds 50
                }
                Write-Output "$scenario run=$run rendered=$rendered prompt=$prompt log=$log"
                if (!$rendered -or (($scenario -in @('profile', 'no-profile') -or ($scenario -eq 'configured' -and $RequireConfiguredPrompt)) -and !$prompt)) { $failed = $true }
            } finally {
                if (!$child.HasExited) {
                    $null = $child.CloseMainWindow()
                    if (!$child.WaitForExit(3000)) {
                        $forcedShutdowns++
                        Write-Warning "$scenario run=$run required forced parent termination; child cleanup is unverified."
                        Stop-Process -Id $child.Id -Force
                    }
                }
                $child.Dispose()
            }
        }
    }
} finally {
    $env:TERMINAL_STARTUP_DIAGNOSTICS = $previousDiagnostics
    $env:TERMINAL_STARTUP_LOG = $previousLog
    $env:TERMINAL_CONFIG = $previousConfig
}

Write-Output "forced-parent-shutdowns=$forcedShutdowns"
if ($RequireGracefulShutdown -and $forcedShutdowns -gt 0) { throw 'One or more launches failed graceful shutdown.' }
if ($failed) { throw 'One or more launches failed readiness checks; inspect their logs.' }
