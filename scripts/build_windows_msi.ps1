param(
    [Parameter(Mandatory)][ValidateSet('x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc')]
    [string]$Target,
    [string]$ReleaseTag = $env:TERMINAL_RELEASE_TAG,
    [string]$OutputDirectory = 'dist'
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    # Tags are the release version source; Cargo is the local/PR fallback.
    if ($ReleaseTag) {
        if ($ReleaseTag -notmatch '^v(\d+)\.(\d+)\.(\d+)$') { throw 'Expected vX.Y.Z release tag' }
        $version = $ReleaseTag.Substring(1)
    } else {
        $manifest = Get-Content crates/app/Cargo.toml -Raw
        if ($manifest -notmatch '(?m)^version\s*=\s*"(\d+\.\d+\.\d+)"') { throw 'Missing Cargo version' }
        $version = $Matches[1]
    }
    $parts = $version.Split('.') | ForEach-Object { [uint32]$_ }
    if ($parts[0] -gt 255 -or $parts[1] -gt 255 -or $parts[2] -gt 65535) {
        throw 'Version exceeds MSI limits: 255.255.65535'
    }
    $version = $parts -join '.'
    $architecture = if ($Target.StartsWith('x86_64')) { 'x86_64' } else { 'aarch64' }
    $wixArchitecture = if ($architecture -eq 'x86_64') { 'x64' } else { 'arm64' }
    $binary = Join-Path $root "target/$Target/release/terminal.exe"
    if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) { throw "Missing release binary: $binary" }
    $pe = [IO.File]::ReadAllBytes($binary)
    $peOffset = [BitConverter]::ToInt32($pe, 0x3c)
    $machine = [BitConverter]::ToUInt16($pe, $peOffset + 4)
    $expectedMachine = if ($architecture -eq 'x86_64') { 0x8664 } else { 0xaa64 }
    if ($machine -ne $expectedMachine) { throw 'Executable architecture does not match MSI target' }
    $metadata = [Diagnostics.FileVersionInfo]::GetVersionInfo($binary)
    if ($metadata.ProductName -ne 'Terminal' -or $metadata.OriginalFilename -ne 'terminal.exe') {
        throw 'Executable branding metadata is missing'
    }
    $toolVersion = (Get-Content .config/dotnet-tools.json -Raw | ConvertFrom-Json).tools.wix.version
    & dotnet tool restore
    if ($LASTEXITCODE) { throw 'WiX tool restore failed' }
    & dotnet tool run wix -- extension add "WixToolset.UI.wixext/$toolVersion"
    if ($LASTEXITCODE) { throw 'WiX UI extension restore failed' }
    $dist = if ([IO.Path]::IsPathRooted($OutputDirectory)) { $OutputDirectory } else { Join-Path $root $OutputDirectory }
    $intermediate = Join-Path $root "target/installer/$Target"
    New-Item -ItemType Directory -Force $dist, $intermediate | Out-Null
    $rtf = Join-Path $intermediate 'license.rtf'
    $license = (Get-Content LICENSE -Raw).Replace('\', '\\').Replace('{', '\{').Replace('}', '\}')
    $license = $license.Replace("`r", '').Replace("`n", '\par ')
    Set-Content -LiteralPath $rtf -Encoding ascii -Value ("{\rtf1\ansi\deff0 {\fonttbl {\f0 Segoe UI;}}\f0\fs20 " + $license + '}')
    $msi = Join-Path $dist "terminal-windows-$architecture.msi"
    & dotnet tool run wix -- build installer/windows/terminal.wxs -arch $wixArchitecture -wx `
        -ext "WixToolset.UI.wixext/$toolVersion" -intermediateFolder $intermediate `
        -d "Version=$version" -d "Binary=$binary" `
        -d "Icon=$root/assets/branding/windows/terminal.ico" -d "License=$root/LICENSE" `
        -d "LicenseRtf=$rtf" -o $msi
    if ($LASTEXITCODE) { throw 'WiX MSI compilation/validation failed' }
    & "$PSScriptRoot/validate_windows_msi.ps1" -Msi $msi -Architecture $architecture -Version $version
    # Read the cabinet and icon back out, without running or installing the MSI.
    $extracted = Join-Path $intermediate ([Guid]::NewGuid().ToString('N'))
    & dotnet tool run wix -- msi decompile $msi -x $extracted -o "$extracted/package.wxs"
    if ($LASTEXITCODE) { throw 'MSI payload extraction failed' }
    foreach ($pair in @(
        @($binary, "$extracted/File/TerminalExe"),
        @("$root/LICENSE", "$extracted/File/LicenseFile"),
        @("$root/assets/branding/windows/terminal.ico", "$extracted/Icon/TerminalIcon")
    )) {
        if ((Get-FileHash -LiteralPath $pair[0]).Hash -ne (Get-FileHash -LiteralPath $pair[1]).Hash) {
            throw "MSI payload changed: $($pair[0])"
        }
    }
    Write-Host "Built and validated $msi ($version, $wixArchitecture)"
} finally {
    Pop-Location
}
