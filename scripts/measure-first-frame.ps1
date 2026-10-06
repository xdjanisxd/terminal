# Sequential native Windows release measurements. Startup caches are not cleared.
param(
    [ValidateRange(1, 100)][int]$Runs = 15,
    [string]$OutputDirectory,
    [switch]$RendererDiagnostics,
    [string]$Workspace
)
$ErrorActionPreference = 'Stop'
$repository = Split-Path $PSScriptRoot -Parent
$executable = Join-Path $repository 'target/release/terminal.exe'
if (!(Test-Path -LiteralPath $executable)) { throw 'Build release terminal-app first.' }
if (!$OutputDirectory) { $OutputDirectory = Join-Path $repository ('target/first-frame-' + [Guid]::NewGuid().ToString('N')) }
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Use a fresh output directory.' }
$null = New-Item -ItemType Directory -Path $OutputDirectory
$output = (Resolve-Path -LiteralPath $OutputDirectory).Path
$workspacePath = if ($Workspace) { (Resolve-Path -LiteralPath $Workspace).Path } else { Join-Path $output 'workspace.toml' }
$cmd = ConvertTo-Json -InputObject ([string](Get-Command cmd.exe).Source) -Compress
if (!$Workspace) {
[IO.File]::WriteAllText($workspacePath, @"
[workspace]
active_tab = 0
[[workspace.tabs]]
title = "First frame measurement"
[workspace.tabs.layout]
kind = "pane"
[workspace.tabs.layout.session.command]
program = $cmd
args = ["/d", "/k"]
"@)
}
$os = Get-CimInstance Win32_OperatingSystem
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
$video = Get-CimInstance Win32_VideoController | Select-Object Name, DriverVersion
@{ os = $os.Caption; build = $os.BuildNumber; cpu = $cpu.Name; video = $video;
   architecture = $env:PROCESSOR_ARCHITECTURE; backendOverride = $env:WGPU_BACKEND;
   rendererDiagnostics = [bool]$RendererDiagnostics;
   executableSha256 = (Get-FileHash -LiteralPath $executable).Hash;
   rustc = (rustc -Vv | Out-String).Trim(); caches = 'unchanged; fresh process each run'
} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $output 'environment.json')
$previous = @{}
foreach ($name in @('TERMINAL_STARTUP_DIAGNOSTICS','TERMINAL_STARTUP_LOG','TERMINAL_RENDERER_DIAGNOSTICS')) {
    $previous[$name] = [Environment]::GetEnvironmentVariable($name)
}
$samples = @()
$costs = @()
try {
    $env:TERMINAL_STARTUP_DIAGNOSTICS = '1'
    $env:TERMINAL_RENDERER_DIAGNOSTICS = if ($RendererDiagnostics) { '1' } else { $null }
    foreach ($run in 1..$Runs) {
        $log = Join-Path $output "run-$run.log"
        $env:TERMINAL_STARTUP_LOG = $log
        $child = Start-Process -FilePath $executable -ArgumentList @('--workspace', ('"' + $workspacePath + '"')) -WindowStyle Hidden -PassThru -RedirectStandardError (Join-Path $output "run-$run.stderr")
        $clock = [Diagnostics.Stopwatch]::StartNew()
        $text = ''
        $forced = $false
        $descendants = @()
        try {
            while ($clock.Elapsed.TotalSeconds -lt 20 -and !$child.HasExited) {
                if (Test-Path -LiteralPath $log) {
                    $stream = [IO.File]::Open($log, 'Open', 'Read', 'ReadWrite')
                    $reader = [IO.StreamReader]::new($stream)
                    try { $text = $reader.ReadToEnd() } finally { $reader.Dispose() }
                    if ($text.Contains('stage=first-shell-output-rendered')) { break }
                }
                Start-Sleep -Milliseconds 25
            }
            # Collect only descendants of this launch, after the timed interval.
            $processes = @(Get-CimInstance Win32_Process)
            $parents = @([uint32]$child.Id)
            do {
                $next = @($processes | Where-Object { $_.ParentProcessId -in $parents -and $_.ProcessId -notin $descendants } | ForEach-Object { [uint32]$_.ProcessId })
                $descendants += $next
                $parents = $next
            } while ($next.Count -gt 0)
        } finally {
            $closeClock = [Diagnostics.Stopwatch]::StartNew()
            if (!$child.HasExited) {
                $null = $child.CloseMainWindow()
                if (!$child.WaitForExit(3000)) {
                    $forced = $true
                    Stop-Process -Id $child.Id -Force
                    if (!$child.WaitForExit(3000)) { throw 'Measured app did not exit after forced cleanup.' }
                }
            }
            foreach ($descendant in $descendants) {
                $remaining = Get-Process -Id $descendant -ErrorAction SilentlyContinue
                if ($remaining) {
                    $forced = $true
                    Stop-Process -Id $descendant -Force
                    if (!$remaining.WaitForExit(3000)) { throw "Child $descendant remained alive." }
                    $remaining.Dispose()
                }
            }
            $closeMs = $closeClock.Elapsed.TotalMilliseconds
            $exitCode = $child.ExitCode
            $child.Dispose()
        }
        if (!$text.Contains('stage=first-shell-output-rendered')) { throw "Run $run has no rendered shell output." }
        $previousIndex = -1
        foreach ($stage in @('first-frame-presented', 'window-shown', 'pty-started', 'pty-spawn-complete')) {
            $index = $text.IndexOf(('stage=' + $stage + ' '))
            if ($index -le $previousIndex) { throw "Run $run has invalid presentation/shell ordering at $stage." }
            $previousIndex = $index
        }
        $milestones = @{}
        # Ignore stages after the successful first present. Sum repeated calls/retries.
        foreach ($line in ($text -split '\r?\n')) {
            if ($line -match 'cost=(\S+) duration_us=(\d+)') {
                $costs += [pscustomobject]@{ run=$run; stage=$Matches[1]; duration_us=[long]$Matches[2] }
            }
            if ($line -match 'stage=(\S+) elapsed_us=(\d+)') {
                $milestones[$Matches[1]] = [long]$Matches[2]
                if ($Matches[1] -eq 'first-frame-presented') { break }
            }
        }
        if (!$milestones.ContainsKey('first-frame-presented')) { throw "Run $run has no successful first frame." }
        $samples += [pscustomobject]@{ run=$run; first_frame_us=$milestones['first-frame-presented']; fonts_to_frame_us=($milestones['first-frame-presented']-$milestones['fonts-started']); forced_cleanup=$forced; close_ms=[math]::Round($closeMs,2); exit_code=$exitCode; descendants=$descendants.Count }
        $samples | Export-Csv -NoTypeInformation -LiteralPath (Join-Path $output 'samples.csv')
        $costs | Export-Csv -NoTypeInformation -LiteralPath (Join-Path $output 'costs.csv')
        Start-Sleep -Milliseconds 250
    }
} finally {
    foreach ($name in $previous.Keys) { [Environment]::SetEnvironmentVariable($name, $previous[$name]) }
}
Write-Output "Measured $($samples.Count) sequential runs: $output"
