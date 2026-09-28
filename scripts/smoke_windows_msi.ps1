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
$script:CurrentStage = 'preflight'
$script:ProcessTimeoutSeconds = 600
function Start-SmokeStage([string]$Stage, [string]$Details) {
    $script:CurrentStage = $Stage
    Write-Host "[$Target] [$Stage] BEGIN $Details"
}
function Complete-SmokeStage([string]$Details = 'completed') {
    Write-Host "[$Target] [$script:CurrentStage] END $Details"
}
$osArchitecture = [Runtime.InteropServices.RuntimeInformation]::OSArchitecture
$processArchitecture = [Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture
Write-Host "MSI smoke preflight: target=$Target MSI=$msiPath runner=$env:RUNNER_NAME runnerEnvironment=$env:RUNNER_ENVIRONMENT OSArchitecture=$osArchitecture ProcessArchitecture=$processArchitecture Is64BitProcess=$([Environment]::Is64BitProcess) PROCESSOR_ARCHITECTURE=$env:PROCESSOR_ARCHITECTURE PROCESSOR_ARCHITEW6432=$env:PROCESSOR_ARCHITEW6432"
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
    $safeLabel = $Label -replace '[^A-Za-z0-9_-]', '-'
    $log = Join-Path $env:RUNNER_TEMP "terminal-msi-$safeLabel.log"
    $commandArguments = $Arguments + @('/qn', '/norestart', '/L*v', "`"$log`"")
    Start-SmokeStage $Label "MSI=$script:msiPath command=msiexec.exe $($commandArguments -join ' ')"
    $process = $null
    try {
        $process = Start-Process -FilePath 'msiexec.exe' -ArgumentList $commandArguments -PassThru -WindowStyle Hidden
        if (-not $process.WaitForExit([int]($script:ProcessTimeoutSeconds * 1000))) {
            try { $process.Kill($true) } catch { try { $process.Kill() } catch {} }
            [void]$process.WaitForExit(15000)
            throw "MSI smoke stage '$script:CurrentStage' timed out after $($script:ProcessTimeoutSeconds)s; terminated msiexec PID $($process.Id); MSI=$script:msiPath; log=$log"
        }
        $exitCode = $process.ExitCode
        if ($exitCode -notin @(0, 3010)) { throw "MSI smoke stage '$script:CurrentStage' failed: msiexec exit code $exitCode; MSI=$script:msiPath; log=$log" }
        Complete-SmokeStage "msiexec exit code=$exitCode log=$log"
    } catch {
        if ($_.Exception.Message -match "MSI smoke stage '$script:CurrentStage'") { throw }
        throw "MSI smoke stage '$script:CurrentStage' failed to launch or wait for msiexec. MSI=$script:msiPath log=$log error=$($_.Exception.Message)"
    } finally {
        if ($null -ne $process) { $process.Dispose() }
    }
}
function Assert-Smoke([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw "MSI smoke stage '$script:CurrentStage': $Message" }
}
function Invoke-InstalledHelp([string]$Executable, [string]$Label) {
    Start-SmokeStage $Label "MSI=$script:msiPath executable=$Executable command=`"$Executable`" --help"
    $exists = Test-Path -LiteralPath $Executable -PathType Leaf
    Write-Host "[$Target] [$Label] installed executable exists before --help: $exists path=$Executable"
    Assert-Smoke $exists "installed executable does not exist before --help: $Executable"
    $stdoutPath = Join-Path $env:RUNNER_TEMP ("terminal-help-$Label-" + [Guid]::NewGuid().ToString('N') + '.out')
    $stderrPath = Join-Path $env:RUNNER_TEMP ("terminal-help-$Label-" + [Guid]::NewGuid().ToString('N') + '.err')
    $exitCode = $null
    $stdout = ''
    $stderr = ''
    $process = $null
    try {
        try {
            $process = Start-Process -FilePath $Executable -ArgumentList @('--help') -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath -PassThru -WindowStyle Hidden
            if (-not $process.WaitForExit(120000)) {
                try { $process.Kill($true) } catch { try { $process.Kill() } catch {} }
                [void]$process.WaitForExit(15000)
                $stderr = "Process timed out after 120s and was terminated; PID=$($process.Id)."
                $exitCode = 'timeout'
            } else {
                $exitCode = $process.ExitCode
            }
        } catch {
            if ($stderr) { $stderr += "`n" }
            $stderr += $_.ToString()
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
            throw "MSI smoke stage '$script:CurrentStage' --help failed`nMSI: $script:msiPath`nTarget: $Target`nExecutable: $Executable`nExit code: $exitCode`nstdout:`n$safeStdout`nstderr:`n$safeStderr"
        }
        Complete-SmokeStage "--help exit code=$exitCode"
    } finally {
        if ($null -ne $process) { $process.Dispose() }
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
Start-SmokeStage 'upgrade MSI preparation' "MSI=$msiPath target=$Target reading package identity and building higher-version upgrade MSI"
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
Complete-SmokeStage "upgrade MSI built and validated: $upgradeMsi"
$installedMsi = $msiPath
try {
    Invoke-Msi -Arguments @('/i', "`"$msiPath`"") -Label 'baseline install'
    $exe = Join-Path $installDir 'terminal.exe'
    Start-SmokeStage 'installed --help' "MSI=$msiPath executable=$exe"
    Invoke-InstalledHelp -Executable $exe -Label 'installed'
    Assert-Smoke (-not (Test-Path -LiteralPath (Join-Path $installDir 'config.toml'))) 'config remains optional'
    Start-SmokeStage 'Start Menu shortcut check' "MSI=$msiPath shortcut=$shortcut expectedTarget=$exe"
    Assert-Smoke (Test-Path -LiteralPath $shortcut -PathType Leaf) 'Start Menu shortcut'
    $shell = New-Object -ComObject WScript.Shell
    try {
        $link = $shell.CreateShortcut($shortcut)
        try {
            Assert-Smoke ($link.TargetPath -eq $exe -and $link.IconLocation -match 'TerminalIcon') 'shortcut target and icon'
        } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($link) }
    } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($shell) }
    Complete-SmokeStage 'shortcut exists with expected target and icon'
    Start-SmokeStage 'PATH check' "MSI=$msiPath expectedInstallDir=$installDir"
    $pathInstalled = [Environment]::GetEnvironmentVariable('Path', 'Machine')
    Assert-Smoke (@($pathInstalled.Split(';') | Where-Object { $_.TrimEnd('\') -ieq $installDir }).Count -eq 1) 'PATH entry exactly once'
    Assert-Smoke ($pathInstalled.StartsWith($pathBefore)) 'unrelated PATH preserved'
    Complete-SmokeStage 'single owned PATH entry; unrelated entries preserved'
    Invoke-Msi -Arguments @('/fa', "`"$msiPath`"") -Label 'repair'
    Start-SmokeStage 'repair verification' "MSI=$msiPath checking PATH=$installDir"
    Assert-Smoke ([Environment]::GetEnvironmentVariable('Path', 'Machine') -eq $pathInstalled) 'repair does not duplicate PATH'
    Complete-SmokeStage 'repair preserved exactly one PATH entry'
    Invoke-Msi -Arguments @('/i', "`"$upgradeMsi`"") -Label 'upgrade'
    $installedMsi = $upgradeMsi
    Assert-Smoke ([Environment]::GetEnvironmentVariable('Path', 'Machine') -eq $pathInstalled) 'upgrade retains exactly one owned PATH entry'
    Assert-Smoke ((Get-ItemProperty $registry).InstallerPathOwned -eq 1) 'upgrade retains PATH ownership'
    Assert-Smoke ((Test-Path -LiteralPath $exe) -and (Test-Path -LiteralPath $shortcut)) 'upgrade retains executable and shortcut'
    Start-SmokeStage 'upgraded --help' "MSI=$upgradeMsi executable=$exe"
    Invoke-InstalledHelp -Executable $exe -Label 'upgraded'
    $installer = New-Object -ComObject WindowsInstaller.Installer
    try {
        Assert-Smoke ($installer.ProductState($identity.ProductCode) -eq -1) 'old product removed by major upgrade'
        Assert-Smoke ($installer.ProductState($upgradeIdentity.ProductCode) -eq 5) 'new product installed'
    } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($installer) }
} finally {
    # Attempt rollback even after a failed install or assertion. Never remove directories manually.
    Start-SmokeStage 'uninstall' "MSI=$installedMsi command=msiexec.exe /x `"$installedMsi`" /qn /norestart"
    Invoke-Msi -Arguments @('/x', "`"$installedMsi`"") -Label 'uninstall'
}
Start-SmokeStage 'PATH cleanup' "MSI=$msiPath installDir=$installDir shortcut=$shortcut registry=$registry"
Assert-Smoke (-not (Test-Path -LiteralPath $installDir)) 'installed files removed'
Assert-Smoke (-not (Test-Path -LiteralPath $shortcut)) 'shortcut removed'
Assert-Smoke (-not (Test-Path $registry)) 'installer metadata removed'
Assert-Smoke ([Environment]::GetEnvironmentVariable('Path', 'Machine') -eq $pathBefore) 'original machine PATH restored'
Complete-SmokeStage 'installed files, shortcut, metadata removed and original PATH restored'
Write-Host 'MSI install, --help, shortcut, PATH, repair, upgrade and uninstall smoke passed'

# Model a user-managed pre-existing PATH entry; the MSI must never claim it.
$userManagedPath = "$pathBefore;$installDir"
Start-SmokeStage 'pre-existing PATH preservation test' "MSI=$msiPath userManagedPathEntry=$installDir"
try {
    [Environment]::SetEnvironmentVariable('Path', $userManagedPath, 'Machine')
    $installedMsi = $msiPath
    try {
        Invoke-Msi -Arguments @('/i', "`"$msiPath`"") -Label 'pre-existing PATH install'
        Assert-Smoke ([Environment]::GetEnvironmentVariable('Path', 'Machine') -eq $userManagedPath) 'pre-existing PATH is not duplicated'
        $metadata = Get-ItemProperty $registry
        Assert-Smoke (-not ($metadata.PSObject.Properties.Name -contains 'InstallerPathOwned')) 'pre-existing PATH is not claimed'
        Invoke-Msi -Arguments @('/i', "`"$upgradeMsi`"") -Label 'pre-existing PATH upgrade'
        $installedMsi = $upgradeMsi
        Assert-Smoke ([Environment]::GetEnvironmentVariable('Path', 'Machine') -eq $userManagedPath) 'upgrade preserves pre-existing PATH'
        $metadata = Get-ItemProperty $registry
        Assert-Smoke (-not ($metadata.PSObject.Properties.Name -contains 'InstallerPathOwned')) 'upgrade does not acquire user-managed PATH'
    } finally {
        Invoke-Msi -Arguments @('/x', "`"$installedMsi`"") -Label 'pre-existing PATH uninstall'
    }
    Assert-Smoke ([Environment]::GetEnvironmentVariable('Path', 'Machine') -eq $userManagedPath) 'pre-existing PATH survives uninstall'
} finally {
    Start-SmokeStage 'PATH cleanup' "MSI=$msiPath restoring original machine PATH"
    [Environment]::SetEnvironmentVariable('Path', $pathBefore, 'Machine')
}
Complete-SmokeStage 'original machine PATH restored after preservation test'
Complete-SmokeStage 'user-managed PATH preserved through install, upgrade, uninstall and restoration'
Write-Host 'Pre-existing user-managed system PATH preservation smoke passed'
