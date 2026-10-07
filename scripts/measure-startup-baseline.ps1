# Fresh-process Windows release baseline; no cache clearing or production changes.
param(
    [ValidateRange(1,100)][int]$Runs = 15,
    [ValidateRange(0,10)][int]$Warmups = 2,
    [string]$OutputDirectory,
    [string]$Config = (Join-Path $env:APPDATA 'terminal/config.toml')
)
$ErrorActionPreference = 'Stop'
$repository = Split-Path $PSScriptRoot -Parent
$executable = Join-Path $repository 'target/release/terminal.exe'
if (!(Test-Path -LiteralPath $executable)) { throw 'Build release terminal-app first.' }
if (!$OutputDirectory) { $OutputDirectory = Join-Path $repository ('target/startup-baseline-' + [Guid]::NewGuid().ToString('N')) }
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Use a fresh output directory.' }
$null = New-Item -ItemType Directory -Path $OutputDirectory
$output = (Resolve-Path -LiteralPath $OutputDirectory).Path
$configPath = (Resolve-Path -LiteralPath $Config).Path
$baseConfig = [IO.File]::ReadAllText($configPath)
$pwsh = (Get-Command pwsh.exe -ErrorAction Stop).Source
$cmd = (Get-Command cmd.exe -ErrorAction Stop).Source
$scenarios = @('cmd', 'no-profile', 'configured-pwsh')
foreach ($scenario in $scenarios) {
    $program = if ($scenario -eq 'cmd') { $cmd } else { $pwsh }
    $shellArgs = if ($scenario -eq 'no-profile') { '["-NoProfile"]' } else { '[]' }
    $shell = "[shell]`nprogram = $(ConvertTo-Json -InputObject $program -Compress)`nargs = $shellArgs`n`n"
    # Preserve all active settings except shell selection. No user config writes.
    $pattern = '(?ms)^\[shell\][^\r\n]*\r?\n.*?(?=^\[|\z)'
    $text = if ([regex]::IsMatch($baseConfig, $pattern)) {
        [regex]::Replace($baseConfig, $pattern, [System.Text.RegularExpressions.MatchEvaluator]{ param($match) $shell })
    } else { $shell + $baseConfig }
    [IO.File]::WriteAllText((Join-Path $output "$scenario.toml"), $text)
}
$os = Get-CimInstance Win32_OperatingSystem
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
@{
    revision = (git -C $repository rev-parse HEAD); branch = (git -C $repository branch --show-current)
    os = $os.Caption; build = $os.BuildNumber; cpu = $cpu.Name
    video = @(Get-CimInstance Win32_VideoController | Select-Object Name, DriverVersion)
    architecture = $env:PROCESSOR_ARCHITECTURE; backendOverride = $env:WGPU_BACKEND
    rustc = (rustc -Vv | Out-String).Trim(); executableSha256 = (Get-FileHash -LiteralPath $executable).Hash
    executable = $executable; configSource = $configPath; configSha256 = (Get-FileHash -LiteralPath $configPath).Hash
    pwsh = $pwsh; pwshVersion = (& $pwsh -NoProfile -Command '$PSVersionTable.PSVersion.ToString()')
    cmd = $cmd; runs = $Runs; warmups = $Warmups; workingDirectory = $repository
    caches = 'unchanged; fresh process each run; warm/uncontrolled caches'
    rendererDiagnostics = $false; startupDiagnostics = $true
    launchArguments = '--workspace <isolated config> --project-root <repository>'
    shellArguments = @{ cmd = @(); 'no-profile' = @('-NoProfile'); 'configured-pwsh' = @() }
    integration = 'normal configured LocalShell integration; pwsh profiles preserved except NoProfile'
    startedUtc = [DateTime]::UtcNow.ToString('o')
} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $output 'environment.json')
$previous = @{}
foreach ($name in @('TERMINAL_STARTUP_DIAGNOSTICS','TERMINAL_STARTUP_LOG','TERMINAL_RENDERER_DIAGNOSTICS')) {
    $previous[$name] = [Environment]::GetEnvironmentVariable($name)
}
$required = @('application-started','window-created','fonts-started','fonts-ready','gpu-started','gpu-ready','renderer-ready','first-frame-presented','window-shown','pty-started','child-spawn-requested','child-spawned','first-pty-bytes','prompt-ready','first-shell-text-processed','first-shell-output-rendered','pty-spawn-complete')
$samples = @()
$costs = @()
$cleanup = @()
try {
    $env:TERMINAL_STARTUP_DIAGNOSTICS = '1'
    $env:TERMINAL_RENDERER_DIAGNOSTICS = $null
    for ($cycle = 1; $cycle -le ($Warmups + $Runs); $cycle++) {
        $warmup = $cycle -le $Warmups
        $run = if ($warmup) { $cycle } else { $cycle - $Warmups }
        for ($offset = 0; $offset -lt $scenarios.Count; $offset++) {
            $scenario = $scenarios[($cycle - 1 + $offset) % $scenarios.Count]
            $label = if ($warmup) { "warmup-$scenario-$run" } else { "$scenario-$run" }
            $log = Join-Path $output "$label.log"
            $env:TERMINAL_STARTUP_LOG = $log
            $workspace = Join-Path $output "$scenario.toml"
            $child = Start-Process -FilePath $executable -WorkingDirectory $repository -ArgumentList @('--workspace', ('"' + $workspace + '"'), '--project-root', ('"' + $repository + '"')) -WindowStyle Hidden -PassThru -RedirectStandardError (Join-Path $output "$label.stderr")
            $clock = [Diagnostics.Stopwatch]::StartNew()
            $text = ''
            $descendants = @()
            $forced = $false
            try {
                while ($clock.Elapsed.TotalSeconds -lt 20 -and !$child.HasExited) {
                    if (Test-Path -LiteralPath $log) {
                        $stream = [IO.File]::Open($log, 'Open', 'Read', 'ReadWrite')
                        $reader = [IO.StreamReader]::new($stream)
                        try { $text = $reader.ReadToEnd() } finally { $reader.Dispose() }
                        if ($text.Contains('stage=first-shell-output-rendered') -and $text.Contains('stage=prompt-ready')) { break }
                    }
                    Start-Sleep -Milliseconds 25
                }
                # Descendant inventory is outside the measured interval.
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
                        if (!$child.WaitForExit(3000)) { throw 'Measured app did not exit.' }
                    }
                }
                foreach ($descendant in $descendants) {
                    $remaining = Get-Process -Id $descendant -ErrorAction SilentlyContinue
                    if ($remaining) {
                        $forced = $true
                        Stop-Process -Id $descendant -Force
                        if (!$remaining.WaitForExit(3000)) { throw "Descendant $descendant remained alive." }
                        $remaining.Dispose()
                    }
                }
                $cleanup += [pscustomobject]@{ scenario=$scenario; run=$run; warmup=$warmup; forced=$forced; close_ms=$closeClock.Elapsed.TotalMilliseconds; exit_code=$child.ExitCode; descendants=$descendants.Count }
                $child.Dispose()
                $cleanup | Export-Csv -NoTypeInformation -LiteralPath (Join-Path $output 'cleanup.csv')
            }
            $text = [IO.File]::ReadAllText($log)
            if ([regex]::Matches($text, 'stage=child-spawned ').Count -ne 1) {
                throw "$label requires exactly one local-shell pane"
            }
            $milestones = @{}
            $beforePresent = $true
            foreach ($line in ($text -split '\r?\n')) {
                if ($beforePresent -and $line -match 'cost=(\S+) duration_us=(\d+)') {
                    if (!$warmup) { $costs += [pscustomobject]@{ scenario=$scenario; run=$run; stage=$Matches[1]; duration_us=[long]$Matches[2] } }
                }
                if ($line -match 'stage=(\S+) .*?elapsed_us=(\d+)') {
                    $stage = $Matches[1]
                    if (!$milestones.ContainsKey($stage)) { $milestones[$stage] = [long]$Matches[2] }
                    if ($stage -eq 'first-frame-presented') { $beforePresent = $false }
                }
            }
            foreach ($stage in $required) { if (!$milestones.ContainsKey($stage)) { throw "$label missing $stage" } }
            $previousIndex = -1
            foreach ($stage in @('first-frame-presented','window-shown','pty-started','pty-spawn-complete')) {
                $index = $text.IndexOf('stage=' + $stage + ' ')
                if ($index -le $previousIndex) { throw "$label invalid presentation/PTY ordering" }
                $previousIndex = $index
            }
            if (!$warmup) {
                foreach ($stage in $milestones.Keys) { $samples += [pscustomobject]@{ scenario=$scenario; run=$run; stage=$stage; elapsed_us=$milestones[$stage] } }
                $samples | Export-Csv -NoTypeInformation -LiteralPath (Join-Path $output 'milestones.csv')
                $costs | Export-Csv -NoTypeInformation -LiteralPath (Join-Path $output 'costs.csv')
            }
            if ($forced) { throw "$label required forced cleanup; inspect before continuing" }
            Start-Sleep -Milliseconds 250
        }
        Write-Output "Completed cycle $cycle of $($Warmups + $Runs)"
    }
} finally {
    foreach ($name in $previous.Keys) { [Environment]::SetEnvironmentVariable($name, $previous[$name]) }
}
$metadataPath = Join-Path $output 'environment.json'
$metadata = Get-Content -LiteralPath $metadataPath -Raw | ConvertFrom-Json
if ((Get-FileHash -LiteralPath $executable).Hash -ne $metadata.executableSha256 -or
    (Get-FileHash -LiteralPath $configPath).Hash -ne $metadata.configSha256) {
    throw 'Executable or source configuration changed during collection.'
}
$metadata | Add-Member -NotePropertyName completedUtc -NotePropertyValue ([DateTime]::UtcNow.ToString('o'))
$metadata | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $metadataPath
Write-Output "Measured $Runs runs per scenario ($Warmups warmups): $output"
