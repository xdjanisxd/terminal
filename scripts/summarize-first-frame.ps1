param([Parameter(Mandatory=$true)][string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
$root = $OutputDirectory
$samples = @(Import-Csv "$root/samples.csv")
$costs = @(Import-Csv "$root/costs.csv")
if (!$samples.Count -or !$costs.Count) { throw 'No startup samples or scopes to summarize.' }
function Stats($values) {
    $v = @($values | ForEach-Object { [double]$_ } | Sort-Object)
    $n = $v.Count
    $median = if ($n % 2) { $v[[int][math]::Floor($n / 2)] } else { ($v[$n/2-1]+$v[$n/2])/2 }
    '{0:F3} | {1:F3}-{2:F3}' -f ($median/1000),($v[0]/1000),($v[-1]/1000)
}
$top = @('font-family-preparation','font-metrics','wgpu-instance','surface-create','adapter-request','device-request','surface-capabilities-and-config','surface-configure','initial-grid-resize','initial-ui-and-pane-layout','initial-projection','surface-acquire','shader-and-layout','rectangle-pipeline','glyph-pipeline','glyph-atlas','frame-texture-view','instance-generation','buffer-create-upload','render-encoding','queue-submit','pre-present-notify','first-present')
$groups = [ordered]@{
    'CPU font preparation + metrics'=@('font-family-preparation','font-metrics')
    'Instance + adapter + device/queue'=@('wgpu-instance','adapter-request','device-request')
    'Surface creation + capabilities + configure'=@('surface-create','surface-capabilities-and-config','surface-configure')
    'Shader/layout + both pipelines'=@('shader-and-layout','rectangle-pipeline','glyph-pipeline')
    'Atlas + frame view + buffers'=@('glyph-atlas','frame-texture-view','buffer-create-upload')
    'Initial grid/layout/projection + instances'=@('initial-grid-resize','initial-ui-and-pane-layout','initial-projection','instance-generation')
    'Acquire + encode + submit + notify + present'=@('surface-acquire','render-encoding','queue-submit','pre-present-notify','first-present')
    'Non-overlapping measured scopes'=$top
    'Glyph pipeline + atlas + glyph layout (defer upper bound)'=@('glyph-pipeline','glyph-atlas','glyph-bind-group-layout')
}
$perRun = foreach ($s in $samples) {
    $row = [ordered]@{run=$s.run; first_frame_us=$s.first_frame_us; fonts_to_frame_us=$s.fonts_to_frame_us}
    $cs = @($costs | Where-Object run -eq $s.run)
    foreach ($name in $groups.Keys) { $row[$name]=[long](($cs | Where-Object stage -in $groups[$name] | Measure-Object duration_us -Sum).Sum) }
    $row['unscoped_us'] = [long]$s.fonts_to_frame_us-$row['Non-overlapping measured scopes']
    [pscustomobject]$row
}
$perRun | Export-Csv -NoTypeInformation "$root/grouped.csv"
'TOTALS'
foreach ($name in @('first_frame_us','fonts_to_frame_us') + @($groups.Keys) + @('unscoped_us')) {
    "$name | $(Stats ($perRun | ForEach-Object { $_.$name }))"
}
'STAGES'
foreach ($stage in @($costs.stage | Select-Object -Unique)) {
    $values = foreach ($s in $samples) { ($costs | Where-Object { $_.run -eq $s.run -and $_.stage -eq $stage } | Measure-Object duration_us -Sum).Sum }
    "| $stage | $(Stats $values) |"
}
'CLEANUP'
"forced=$(@($samples | Where-Object forced_cleanup -ne False).Count); close_ms range=$(( $samples | Measure-Object close_ms -Minimum -Maximum).Minimum)-$(( $samples | Measure-Object close_ms -Minimum -Maximum).Maximum)"
