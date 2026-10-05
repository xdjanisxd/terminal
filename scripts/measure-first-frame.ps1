# Fresh-process Windows release startup sampling; does not evict OS/driver caches.
param([ValidateRange(3,100)][int]$Runs = 7, [string]$OutputDirectory, [string]$Workspace)
$ErrorActionPreference = 'Stop'
$repository = Split-Path $PSScriptRoot -Parent
$executable = Join-Path $repository 'target/release/terminal.exe'
if (!$OutputDirectory) { $OutputDirectory = Join-Path $repository ('target/first-frame-' + [Guid]::NewGuid().ToString('N')) }
$null = New-Item -ItemType Directory -Path $OutputDirectory -Force
$OutputDirectory = (Resolve-Path -LiteralPath $OutputDirectory).Path
$previousDiagnostics = $env:TERMINAL_STARTUP_DIAGNOSTICS
$previousLog = $env:TERMINAL_STARTUP_LOG
try {
    $env:TERMINAL_STARTUP_DIAGNOSTICS = '1'
    foreach ($run in 1..$Runs) {
        $log = Join-Path $OutputDirectory ('launch-' + $run + '.log')
        if (Test-Path -LiteralPath $log) { throw "Refusing to overwrite $log" }
        $env:TERMINAL_STARTUP_LOG = $log
        $launch = @{ FilePath = $executable; PassThru = $true; WindowStyle = 'Hidden' }
        if ($Workspace) { $launch.ArgumentList = @('--workspace', ('"' + (Resolve-Path -LiteralPath $Workspace).Path + '"')) }
        $child = Start-Process @launch
        $deadline = [Diagnostics.Stopwatch]::StartNew()
        $ready = $false
        $contents = ''
        try {
            while ($deadline.Elapsed.TotalSeconds -lt 30 -and !$child.HasExited) {
                if (Test-Path -LiteralPath $log) {
                    $stream = [IO.File]::Open($log, 'Open', 'Read', 'ReadWrite')
                    $reader = [IO.StreamReader]::new($stream)
                    try { $contents = $reader.ReadToEnd() } finally { $reader.Dispose() }
                    $ready = $contents.Contains('stage=first-shell-output-rendered')
                    if ($ready) { break }
                }
                Start-Sleep -Milliseconds 50
            }
            if (!$ready -or !$contents.Contains('stage=window-shown')) { throw "Launch $run did not reach visible frame and shell output; inspect $log" }
            $ordering = @('first-frame-presented', 'window-shown', 'pty-started', 'pty-spawn-complete')
            $previousIndex = -1
            foreach ($stage in $ordering) {
                $index = $contents.IndexOf(('stage=' + $stage + ' '))
                if ($index -le $previousIndex) { throw "Launch $run has invalid presentation/shell ordering at $stage; inspect $log" }
                $previousIndex = $index
            }
        } finally {
            if (!$child.HasExited) {
                $null = $child.CloseMainWindow()
                if (!$child.WaitForExit(3000)) { Stop-Process -Id $child.Id -Force }
            }
            $child.Dispose()
        }
        Write-Output "launch=$run ready=$ready log=$log"
    }
} finally {
    $env:TERMINAL_STARTUP_DIAGNOSTICS = $previousDiagnostics
    $env:TERMINAL_STARTUP_LOG = $previousLog
}
