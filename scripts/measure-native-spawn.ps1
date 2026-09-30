# Manual release probe outside the rendering/event-loop path; no production dependencies.
param(
    [ValidateRange(1, 100)][int]$Runs = 7,
    [string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$repository = Split-Path $PSScriptRoot -Parent
$probe = Join-Path $repository 'target/release/examples/spawn_probe.exe'
$helper = Join-Path $repository 'target/release/pty_test_helper.exe'
if (!(Test-Path -LiteralPath $probe) -or !(Test-Path -LiteralPath $helper)) {
    throw 'Build cargo build --release -p terminal-pty --example spawn_probe --bin pty_test_helper first.'
}
if (!$OutputDirectory) { $OutputDirectory = Join-Path $repository ('target/native-spawn-' + [Guid]::NewGuid().ToString('N')) }
$null = New-Item -ItemType Directory -Path $OutputDirectory -Force
$OutputDirectory = (Resolve-Path -LiteralPath $OutputDirectory).Path
$pwsh = (Get-Command pwsh.exe -ErrorAction Stop).Source
$cmd = (Get-Command cmd.exe -ErrorAction Stop).Source
$previousDiagnostics = $env:TERMINAL_STARTUP_DIAGNOSTICS
$previousLog = $env:TERMINAL_STARTUP_LOG
try {
    $env:TERMINAL_STARTUP_DIAGNOSTICS = '1'
    foreach ($run in 1..$Runs) {
        foreach ($scenario in @('cmd', 'no-profile', 'profile', 'helper')) {
            $program = switch ($scenario) { 'cmd' { $cmd } 'helper' { $helper } default { $pwsh } }
            $shellArguments = switch ($scenario) {
                'cmd' { @() }
                'helper' { @('exit', '0') }
                'no-profile' { @('-NoProfile', '-NoExit') }
                'profile' { @('-NoExit') }
            }
            $log = Join-Path $OutputDirectory "$scenario-$run.log"
            if ((Test-Path -LiteralPath $log) -or (Test-Path -LiteralPath "$log.native") -or (Test-Path -LiteralPath (Join-Path $OutputDirectory "ordinary-$scenario-$run.log"))) { throw "Refusing to overwrite $log" }
            $env:TERMINAL_STARTUP_LOG = $log
            $child = Start-Process -FilePath $probe -ArgumentList (@('"' + $program + '"') + $shellArguments) -PassThru -WindowStyle Hidden -RedirectStandardOutput "$log.stdout" -RedirectStandardError "$log.stderr"
            try {
                if (!$child.WaitForExit(20000)) { Stop-Process -Id $child.Id -Force; throw "Probe timeout: $scenario" }
                if ($child.ExitCode -ne 0) { throw "Probe failed: $scenario; inspect $log.stderr" }
                # The buffered native stamps and outer probe stamps have separate origins.
                [IO.File]::AppendAllText($log, [IO.File]::ReadAllText("$log.stdout"))
            } finally { $child.Dispose() }

            if ($scenario -ne 'helper') {
                # Independent ordinary Windows spawn without ConPTY. Measure Start
                # separately from initialization/exit, with both streams redirected.
                $info = [Diagnostics.ProcessStartInfo]::new($program)
                $info.UseShellExecute = $false
                $info.CreateNoWindow = $true
                $info.RedirectStandardOutput = $true
                $info.RedirectStandardError = $true
                if ($scenario -eq 'cmd') { $info.ArgumentList.Add('/c'); $info.ArgumentList.Add('exit') }
                else {
                    if ($scenario -eq 'no-profile') { $info.ArgumentList.Add('-NoProfile') }
                    $info.ArgumentList.Add('-Command'); $info.ArgumentList.Add('exit')
                }
                $process = [Diagnostics.Process]::new()
                $process.StartInfo = $info
                $clock = [Diagnostics.Stopwatch]::StartNew()
                try {
                    $null = $process.Start()
                    $startUs = $clock.Elapsed.TotalMicroseconds
                    $out = $process.StandardOutput.ReadToEndAsync()
                    $err = $process.StandardError.ReadToEndAsync()
                    if (!$process.WaitForExit(20000)) { $process.Kill(); throw 'Ordinary spawn timeout' }
                    $exitUs = $clock.Elapsed.TotalMicroseconds
                    $null = $out.GetAwaiter().GetResult(); $null = $err.GetAwaiter().GetResult()
                    if ($process.ExitCode -ne 0) { throw 'Ordinary spawn failed' }
                    [IO.File]::WriteAllText((Join-Path $OutputDirectory "ordinary-$scenario-$run.log"), "ordinary elapsed_us=0 stage=start-begin`nordinary elapsed_us=$([long]$startUs) stage=start-end`nordinary elapsed_us=$([long]$exitUs) stage=exit`n")
                } finally { $process.Dispose() }
            }
            Write-Output "$scenario run=$run log=$log"
        }
    }
} finally {
    $env:TERMINAL_STARTUP_DIAGNOSTICS = $previousDiagnostics
    $env:TERMINAL_STARTUP_LOG = $previousLog
}
