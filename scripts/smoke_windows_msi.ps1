param(
    [Parameter(Mandatory)][string]$Msi,
    [Parameter(Mandatory)][ValidateSet('x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc')]
    [string]$Target
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
# Do not run installation tests on a developer machine or an existing installation.
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'MSI smoke installation is restricted to disposable GitHub-hosted runners'
}
$msiPath = (Resolve-Path -LiteralPath $Msi).Path
$installDir = Join-Path $env:ProgramFiles 'Terminal'
$shortcut = Join-Path ([Environment]::GetFolderPath('CommonPrograms')) 'Terminal.lnk'
$registry = 'HKLM:\Software\io.github.xdjanisxd.terminal'
if ((Test-Path -LiteralPath $installDir) -or (Test-Path -LiteralPath $shortcut) -or (Test-Path $registry)) {
    throw 'Refusing to alter an existing Terminal installation'
}
$pathBefore = [Environment]::GetEnvironmentVariable('Path', 'Machine')
if ($pathBefore.IndexOf($installDir, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
    throw 'Refusing to alter an existing Terminal PATH entry'
}
function Invoke-Msi([string[]]$Arguments, [string]$Label) {
    $log = Join-Path $env:RUNNER_TEMP "terminal-msi-$Label.log"
    $process = Start-Process msiexec.exe -ArgumentList ($Arguments + @('/qn', '/norestart', '/L*v', "`"$log`"")) -Wait -PassThru -WindowStyle Hidden
    if ($process.ExitCode -notin @(0, 3010)) { throw "$Label failed: MSI exit $($process.ExitCode); see $log" }
}
function Assert-Smoke([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw "MSI smoke: $Message" }
}
function Invoke-InstalledHelp([string]$Executable, [string]$Label) {
    $stdoutPath = Join-Path $env:RUNNER_TEMP ("terminal-help-$Label-" + [Guid]::NewGuid().ToString('N') + '.out')
    $stderrPath = Join-Path $env:RUNNER_TEMP ("terminal-help-$Label-" + [Guid]::NewGuid().ToString('N') + '.err')
    $exitCode = $null
    $stdout = ''
    $stderr = ''
    try {
        try {
            $process = Start-Process -FilePath $Executable -ArgumentList @('--help') -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath -Wait -PassThru -WindowStyle Hidden
            $exitCode = $process.ExitCode
        } catch {
            $stderr = $_.ToString()
        }
        if (Test-Path -LiteralPath $stdoutPath) { $stdout = [IO.File]::ReadAllText($stdoutPath) }
        if (Test-Path -LiteralPath $stderrPath) {
            $capturedStderr = [IO.File]::ReadAllText($stderrPath)
            if ($stderr) { $stderr += "`n" }
            $stderr += $capturedStderr
        }
        # Keep line breaks and tabs, while removing terminal control characters from diagnostics.
        $safeStdout = [regex]::Replace($stdout, '[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]', '')
        $safeStderr = [regex]::Replace($stderr, '[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]', '')
        $valid = ($exitCode -eq 0) -and $safeStdout.StartsWith('Usage: terminal ') -and ($safeStderr -notmatch 'panicked at|failed printing to stdout')
        if (-not $valid) {
            throw "MSI smoke: $Label --help failed`nExecutable: $Executable`nExit code: $exitCode`nstdout:`n$safeStdout`nstderr:`n$safeStderr"
        }
    } finally {
        Remove-Item -LiteralPath $stdoutPath, $stderrPath -Force -ErrorAction SilentlyContinue
    }
}
function Read-PackageIdentity([string]$Path) {
    $installer = New-Object -ComObject WindowsInstaller.Installer
    $database = $installer.OpenDatabase($Path, 0)
    $view = $database.OpenView('SELECT `Property`, `Value` FROM `Property`')
    $properties = @{}
    try {
        [void]$view.Execute()
        while ($null -ne ($record = $view.Fetch())) {
            try {
                $key = $record.GetType().InvokeMember('StringData', 'GetProperty', $null, $record, @(1))
                $properties[$key] = $record.GetType().InvokeMember('StringData', 'GetProperty', $null, $record, @(2))
            } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($record) }
        }
        return $properties
    } finally {
        [void]$view.Close()
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($view)
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($database)
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($installer)
    }
}
$identity = Read-PackageIdentity $msiPath
$next = $identity.ProductVersion.Split('.') | ForEach-Object { [int]$_ }
if ($next[2] -lt 65535) { $next[2]++ }
elseif ($next[1] -lt 255) { $next[1]++; $next[2] = 0 }
elseif ($next[0] -lt 255) { $next[0]++; $next[1] = 0; $next[2] = 0 }
else { throw 'No higher MSI version is available for the upgrade smoke test' }
$upgradeDir = "target/installer/$Target/upgrade-smoke"
& "$PSScriptRoot/build_windows_msi.ps1" -Target $Target -ReleaseTag ('v' + ($next -join '.')) -OutputDirectory $upgradeDir
$upgradeMsi = (Resolve-Path -LiteralPath (Join-Path $upgradeDir (Split-Path $msiPath -Leaf))).Path
$upgradeIdentity = Read-PackageIdentity $upgradeMsi
$installedMsi = $msiPath
try {
    Invoke-Msi -Arguments @('/i', "`"$msiPath`"") -Label 'install'
    $exe = Join-Path $installDir 'terminal.exe'
    Assert-Smoke (Test-Path -LiteralPath $exe -PathType Leaf) 'installed executable'
    Invoke-InstalledHelp -Executable $exe -Label 'installed'
    Assert-Smoke (-not (Test-Path -LiteralPath (Join-Path $installDir 'config.toml'))) 'config remains optional'
    Assert-Smoke (Test-Path -LiteralPath $shortcut -PathType Leaf) 'Start Menu shortcut'
    $shell = New-Object -ComObject WScript.Shell
    try {
        $link = $shell.CreateShortcut($shortcut)
        try {
            Assert-Smoke ($link.TargetPath -eq $exe -and $link.IconLocation -match 'TerminalIcon') 'shortcut target and icon'
        } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($link) }
    } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($shell) }
    $pathInstalled = [Environment]::GetEnvironmentVariable('Path', 'Machine')
    Assert-Smoke (@($pathInstalled.Split(';') | Where-Object { $_.TrimEnd('\') -ieq $installDir }).Count -eq 1) 'PATH entry exactly once'
    Assert-Smoke ($pathInstalled.StartsWith($pathBefore)) 'unrelated PATH preserved'
    Invoke-Msi -Arguments @('/fa', "`"$msiPath`"") -Label 'repair'
    Assert-Smoke ([Environment]::GetEnvironmentVariable('Path', 'Machine') -eq $pathInstalled) 'repair does not duplicate PATH'
    Invoke-Msi -Arguments @('/i', "`"$upgradeMsi`"") -Label 'upgrade'
    $installedMsi = $upgradeMsi
    Assert-Smoke ([Environment]::GetEnvironmentVariable('Path', 'Machine') -eq $pathInstalled) 'upgrade retains exactly one owned PATH entry'
    Assert-Smoke ((Get-ItemProperty $registry).InstallerPathOwned -eq 1) 'upgrade retains PATH ownership'
    Assert-Smoke ((Test-Path -LiteralPath $exe) -and (Test-Path -LiteralPath $shortcut)) 'upgrade retains executable and shortcut'
    Invoke-InstalledHelp -Executable $exe -Label 'upgraded'
    $installer = New-Object -ComObject WindowsInstaller.Installer
    try {
        Assert-Smoke ($installer.ProductState($identity.ProductCode) -eq -1) 'old product removed by major upgrade'
        Assert-Smoke ($installer.ProductState($upgradeIdentity.ProductCode) -eq 5) 'new product installed'
    } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($installer) }
} finally {
    # Attempt rollback even after a failed install or assertion. Never remove directories manually.
    Invoke-Msi -Arguments @('/x', "`"$installedMsi`"") -Label 'uninstall'
}
Assert-Smoke (-not (Test-Path -LiteralPath $installDir)) 'installed files removed'
Assert-Smoke (-not (Test-Path -LiteralPath $shortcut)) 'shortcut removed'
Assert-Smoke (-not (Test-Path $registry)) 'installer metadata removed'
Assert-Smoke ([Environment]::GetEnvironmentVariable('Path', 'Machine') -eq $pathBefore) 'original machine PATH restored'
Write-Host 'MSI install, --help, shortcut, PATH, repair, upgrade and uninstall smoke passed'

# Model a user-managed pre-existing PATH entry; the MSI must never claim it.
$userManagedPath = "$pathBefore;$installDir"
try {
    [Environment]::SetEnvironmentVariable('Path', $userManagedPath, 'Machine')
    $installedMsi = $msiPath
    try {
        Invoke-Msi -Arguments @('/i', "`"$msiPath`"") -Label 'preexisting-path-install'
        Assert-Smoke ([Environment]::GetEnvironmentVariable('Path', 'Machine') -eq $userManagedPath) 'pre-existing PATH is not duplicated'
        $metadata = Get-ItemProperty $registry
        Assert-Smoke (-not ($metadata.PSObject.Properties.Name -contains 'InstallerPathOwned')) 'pre-existing PATH is not claimed'
        Invoke-Msi -Arguments @('/i', "`"$upgradeMsi`"") -Label 'preexisting-path-upgrade'
        $installedMsi = $upgradeMsi
        Assert-Smoke ([Environment]::GetEnvironmentVariable('Path', 'Machine') -eq $userManagedPath) 'upgrade preserves pre-existing PATH'
        $metadata = Get-ItemProperty $registry
        Assert-Smoke (-not ($metadata.PSObject.Properties.Name -contains 'InstallerPathOwned')) 'upgrade does not acquire user-managed PATH'
    } finally {
        Invoke-Msi -Arguments @('/x', "`"$installedMsi`"") -Label 'preexisting-path-uninstall'
    }
    Assert-Smoke ([Environment]::GetEnvironmentVariable('Path', 'Machine') -eq $userManagedPath) 'pre-existing PATH survives uninstall'
} finally {
    [Environment]::SetEnvironmentVariable('Path', $pathBefore, 'Machine')
}
Write-Host 'Pre-existing user-managed system PATH preservation smoke passed'
