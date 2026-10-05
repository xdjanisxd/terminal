# Summarize measured events; unavailable milestones remain empty, never inferred.
param([string]$InputDirectory = 'target/startup-comparison-clean')
$ErrorActionPreference = 'Stop'
$directory = (Resolve-Path -LiteralPath $InputDirectory).Path
$external = Import-Csv (Join-Path $directory 'external.csv')
$samples = foreach ($sample in $external) {
    $prefix = Join-Path $directory "$($sample.App)-$($sample.Scenario)-$($sample.Run)"
    $record = [ordered]@{ App = $sample.App; Scenario = $sample.Scenario; Run = [int]$sample.Run; Cache = $sample.Cache }
    foreach ($name in @('StartReturnedMs', 'ProcessCreatedMs', 'WindowExistsMs', 'WindowVisibleMs', 'InputIdleMs', 'FocusMs', 'PromptFunctionMs', 'MaxPollGapMs')) {
        $record["external-$name"] = if ($sample.$name -ne '') { [double]$sample.$name } else { $null }
    }
    if ($sample.App -eq 'terminal') {
        foreach ($line in Get-Content -LiteralPath "$prefix.log") {
            if ($line -match 'stage=([^ ]+).*elapsed_us=(\d+)') {
                $name = "stage-$($Matches[1])"
                if (!$record.Contains($name)) { $record[$name] = [double]$Matches[2] / 1000 }
            } elseif ($line -match 'cost=([^ ]+) duration_us=(\d+)') {
                $name = "cost-$($Matches[1])"
                if (!$record.Contains($name)) { $record[$name] = [double]$Matches[2] / 1000 }
            }
        }
        foreach ($definition in @(
            @('interval-font', 'fonts-started', 'fonts-ready'),
            @('interval-gpu', 'gpu-started', 'gpu-ready'),
            @('interval-font-through-renderer', 'fonts-started', 'renderer-ready'),
            @('interval-renderer-to-present', 'renderer-ready', 'first-frame-presented'),
            @('interval-present-to-show', 'first-frame-presented', 'window-shown'),
            @('interval-renderer-to-show', 'renderer-ready', 'window-shown'),
            @('interval-pty-create', 'pty-create-begin', 'pty-created'),
            @('interval-child-spawn', 'child-spawn-requested', 'child-spawned'),
            @('interval-spawn-to-bytes', 'child-spawned', 'first-pty-bytes'),
            @('interval-spawn-to-text', 'child-spawned', 'first-shell-text-processed'),
            @('interval-spawn-to-prompt', 'child-spawned', 'prompt-ready'),
            @('interval-text-to-render', 'first-shell-text-processed', 'first-shell-output-rendered')
        )) {
            $start = $record["stage-$($definition[1])"]; $end = $record["stage-$($definition[2])"]
            if ($null -ne $start -and $null -ne $end) { $record[$definition[0]] = $end - $start }
        }
    } else {
        $markers = [ordered]@{
            'welcome' = 'Welcome to Alacritty'; 'scale-factor' = 'Window scale factor:'
            'wgl-selected' = 'Using WGL'; 'loading-font' = 'Loading .* font'
            'renderer-entered' = 'Running on '; 'renderer-selected' = 'Using OpenGL 3.3 renderer'
            'glyph-fill-start' = 'Filling glyph cache with common glyphs'; 'glyph-fill-done' = 'Cell size:'
            'pty-dimensions' = 'PTY dimensions:'; 'conpty-api' = 'Using Windows API for pseudoconsole'
            'initialisation-complete' = 'Initialisation complete'; 'first-redraw-request' = 'RedrawRequested'
            'first-pty-wakeup-proxy' = 'payload: Terminal\(Wakeup\)'
        }
        foreach ($line in Get-Content -LiteralPath "$prefix.stdout") {
            if ($line -match 'Unable to create log file|\[ERROR\]') { throw "Invalid Alacritty run: $prefix" }
            if ($line -match '^\[([0-9.]+)s\]') {
                $ms = [double]$Matches[1] * 1000
                foreach ($name in $markers.Keys) {
                    if ($line -match $markers[$name] -and !$record.Contains("stage-$name")) { $record["stage-$name"] = $ms }
                }
            }
        }
        foreach ($definition in @(
            @('interval-window-context-proxy', 'scale-factor', 'loading-font'),
            @('interval-font-surface-proxy', 'loading-font', 'renderer-entered'),
            @('interval-renderer-proxy', 'renderer-entered', 'glyph-fill-start'),
            @('interval-glyph-fill', 'glyph-fill-start', 'glyph-fill-done'),
            @('interval-window-through-glyphs', 'scale-factor', 'glyph-fill-done'),
            @('interval-glyphs-to-pty', 'glyph-fill-done', 'pty-dimensions'),
            @('interval-pty-setup-proxy', 'pty-dimensions', 'initialisation-complete'),
            @('interval-init-to-wakeup-proxy', 'initialisation-complete', 'first-pty-wakeup-proxy')
        )) {
            $start = $record["stage-$($definition[1])"]; $end = $record["stage-$($definition[2])"]
            if ($null -ne $start -and $null -ne $end) { $record[$definition[0]] = $end - $start }
        }
    }
    # A wide schema keeps missing milestones empty across heterogeneous applications.
    [pscustomobject]$record
}
$columns = @('App', 'Scenario', 'Run', 'Cache') + @($samples | ForEach-Object { $_.PSObject.Properties.Name } | Where-Object { $_ -notin @('App', 'Scenario', 'Run', 'Cache') } | Sort-Object -Unique)
$samples | Select-Object $columns | Export-Csv (Join-Path $directory 'samples.csv') -NoTypeInformation
$summary = foreach ($group in $samples | Group-Object App, Scenario) {
    foreach ($metric in $columns | Where-Object { $_ -notin @('App', 'Scenario', 'Run', 'Cache') }) {
        $values = @($group.Group | ForEach-Object { if ($null -ne $_.$metric) { [double]$_.$metric } } | Sort-Object)
        if ($values.Count) {
            $middle = [int][Math]::Floor($values.Count / 2)
            $median = if ($values.Count % 2) { $values[$middle] } else { ($values[$middle - 1] + $values[$middle]) / 2 }
            [pscustomobject]@{ App = $group.Group[0].App; Scenario = $group.Group[0].Scenario; Metric = $metric; N = $values.Count; MedianMs = $median; MinMs = $values[0]; MaxMs = $values[-1] }
        }
    }
}
$summary | Export-Csv (Join-Path $directory 'summary.csv') -NoTypeInformation
Write-Output "Summarized $($samples.Count) launches into samples.csv and summary.csv."
