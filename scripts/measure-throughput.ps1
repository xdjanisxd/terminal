# Build first: cargo build --release -p terminal-app --bin terminal --example throughput-workload
param(
    [ValidateRange(3, 20)][int]$Runs = 3,
    [string]$Label = 'baseline',
    [string[]]$Workloads = @('finite', 'short', 'long', 'ansi', 'continuous', 'input')
)
$ErrorActionPreference = 'Stop'
$repository = Split-Path $PSScriptRoot -Parent
$directory = Join-Path $repository "target/perf/throughput/$Label"
$null = New-Item -ItemType Directory -Force $directory
$exe = Join-Path $repository 'target/release/terminal.exe'
$helper = Join-Path $repository 'target/release/examples/throughput-workload.exe'
if (!(Test-Path $exe) -or !(Test-Path $helper)) { throw 'Build both release targets first.' }
$previous = $env:TERMINAL_THROUGHPUT_DIAGNOSTICS
$previousRows = $env:TERMINAL_THROUGHPUT_ROWS
$env:TERMINAL_THROUGHPUT_DIAGNOSTICS = '1'
$env:TERMINAL_THROUGHPUT_ROWS = '31'
try {
    for ($run = 1; $run -le $Runs; $run++) {
        foreach ($workload in $Workloads) {
            $parserLog = Join-Path $directory "$workload-$run-parser.log"
            & $helper parser $workload 2> $parserLog
            if ($LASTEXITCODE -ne 0) { throw "Parser workload failed: $workload" }
            $log = Join-Path $directory "$workload-$run-visible.log"
            $process = Start-Process -FilePath $exe -ArgumentList '--throughput-run', $workload -WindowStyle Hidden -PassThru -RedirectStandardError $log
            if (!$process.WaitForExit(70000)) { $process.Kill(); throw "Workload timed out: $workload" }
            if ($process.ExitCode -ne 0 -or !(Select-String -Path $log -Quiet -Pattern 'complete=true')) {
                throw "Incomplete workload: $log"
            }
            Write-Output "run=$run workload=$workload complete"
        }
    }
} finally {
    $env:TERMINAL_THROUGHPUT_DIAGNOSTICS = $previous
    $env:TERMINAL_THROUGHPUT_ROWS = $previousRows
}
Write-Output "Raw aggregates: $directory"
