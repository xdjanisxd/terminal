param([string]$Label = 'fair')
$ErrorActionPreference = 'Stop'
$directory = Join-Path (Split-Path $PSScriptRoot -Parent) "target/perf/throughput/$Label"
function Summary($values) {
    $sorted = @($values | Sort-Object)
    $middle = [int][Math]::Floor($sorted.Count / 2)
    $median = if ($sorted.Count % 2) { $sorted[$middle] } else { ($sorted[$middle - 1] + $sorted[$middle]) / 2 }
    return ('{0:0.###} [{1:0.###}, {2:0.###}]' -f $median, $sorted[0], $sorted[-1])
}
$result = foreach ($mode in @('visible', 'parser')) {
    foreach ($workload in @('finite', 'short', 'long', 'ansi', 'continuous', 'input')) {
        $files = @(Get-ChildItem -LiteralPath $directory -Filter "$workload-*-$mode.log")
        if ($files.Count -lt 3) { throw "Need at least three runs for $mode/$workload" }
        $runs = foreach ($file in $files) {
            $content = Get-Content -LiteralPath $file.FullName -Raw
            if ($mode -eq 'visible' -and $content -notmatch 'complete=true') { throw "Incomplete: $file" }
            $run = @{}
            foreach ($match in [regex]::Matches($content, '(\w+)=(?:Some\()?([\d.]+)')) {
                $run[$match.Groups[1].Value] = [double]::Parse($match.Groups[2].Value, [Globalization.CultureInfo]::InvariantCulture)
            }
            foreach ($match in [regex]::Matches($content, '(rendered|presented|peak_chunks|peak_bytes): (\d+)')) {
                $run[$match.Groups[1].Value] = [double]$match.Groups[2].Value
            }
            foreach ($match in [regex]::Matches($content, '(projection|generation|upload|submission|render_present): ([\d.]+)(ns|µs|ms|s)')) {
                $factor = switch ($match.Groups[3].Value) { ns { 0.000001 }; µs { 0.001 }; ms { 1 }; s { 1000 } }
                $run[$match.Groups[1].Value + '_ms'] = [double]::Parse($match.Groups[2].Value, [Globalization.CultureInfo]::InvariantCulture) * $factor
            }
            $run
        }
        $row = [ordered]@{ mode = $mode; workload = $workload; runs = $runs.Count }
        foreach ($key in @('bytes', 'elapsed_ms', 'mb_s', 'batches', 'bytes_per_batch', 'parse_ms', 'redraws', 'terminal_frames', 'rendered', 'presented', 'peak_chunks', 'peak_bytes', 'max_drain_ms', 'frame_p95_ms', 'frame_max_ms', 'probe_dispatch_ms', 'probe_ack_ms', 'scrollback', 'projection_ms', 'generation_ms', 'upload_ms', 'submission_ms', 'render_present_ms')) {
            $values = @($runs | ForEach-Object { if ($_.ContainsKey($key)) { $_[$key] } })
            if ($values.Count) { $row[$key] = Summary $values }
        }
        [pscustomobject]$row
    }
}
$result | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $directory 'summary.json')
$result | Select-Object mode, workload, elapsed_ms, mb_s, batches, presented, max_drain_ms, frame_p95_ms, probe_dispatch_ms | Format-Table -AutoSize
Write-Output "All aggregate medians and ranges: $directory/summary.json"
