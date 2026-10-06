param([string]$OutputDirectory = 'target/perf/wgpu-device-init/overlay')
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$recipe = Get-Content -LiteralPath (Join-Path $repo 'docs/perf/wgpu-device-init/native-overlay.json') -Raw | ConvertFrom-Json
$cargoRoot = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $HOME '.cargo' }
$destination = [IO.Path]::GetFullPath((Join-Path $repo $OutputDirectory))
$targetRoot = [IO.Path]::GetFullPath((Join-Path $repo 'target')) + [IO.Path]::DirectorySeparatorChar
if (-not $destination.StartsWith($targetRoot, [StringComparison]::OrdinalIgnoreCase)) { throw 'Overlay must be inside this repository target directory.' }
if (Test-Path -LiteralPath $destination) { throw 'Choose a new output directory; existing overlays are never overwritten.' }
foreach ($entry in @(@{Name='wgpu-hal';Edits=$recipe.hal}, @{Name='wgpu-core';Edits=$recipe.core})) {
    $sources = @(Get-ChildItem -Path (Join-Path $cargoRoot ('registry/src/*/' + $entry.Name + '-' + $recipe.version)) -Directory)
    if ($sources.Count -ne 1) { throw "Expected exactly one cached source for $($entry.Name) $($recipe.version)." }
    $package = Join-Path $destination $entry.Name
    New-Item -ItemType Directory -Path $package -Force | Out-Null
    Copy-Item -Path (Join-Path $sources[0].FullName '*') -Destination $package -Recurse
    foreach ($edit in $entry.Edits) {
        $path = Join-Path $package $edit.path
        $source = [IO.File]::ReadAllText($path)
        if ($edit.append) { $source += "
" + $edit.append }
        else {
            $index = $source.IndexOf($edit.old, [StringComparison]::Ordinal)
            if ($index -lt 0) { throw "Trace marker missing: $($entry.Name)/$($edit.path)" }
            $source = $source.Substring(0,$index) + $edit.replacement + $source.Substring($index + $edit.old.Length)
        }
        [IO.File]::WriteAllText($path,$source,(New-Object Text.UTF8Encoding($false)))
    }
}
Write-Output "Prepared measurement-only sources in $destination. See docs/WGPU_DEVICE_INIT.md for build and restoration steps."
