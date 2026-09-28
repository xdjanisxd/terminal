param(
    [Parameter(Mandatory)][string]$Msi,
    [Parameter(Mandatory)][ValidateSet('x86_64', 'aarch64')][string]$Architecture,
    [Parameter(Mandatory)][string]$Version
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not (Test-Path -LiteralPath $Msi -PathType Leaf) -or (Get-Item -LiteralPath $Msi).Length -eq 0) {
    throw "Missing or empty MSI: $Msi"
}
$installer = New-Object -ComObject WindowsInstaller.Installer
$database = $installer.OpenDatabase((Resolve-Path -LiteralPath $Msi).Path, 0)
function Read-MsiRows([string]$Sql) {
    $view = $database.OpenView($Sql)
    $rows = [System.Collections.Generic.List[object]]::new()
    try {
        [void]$view.Execute()
        while ($null -ne ($record = $view.Fetch())) {
            try {
                $count = $record.GetType().InvokeMember('FieldCount', 'GetProperty', $null, $record, $null)
                $values = for ($i = 1; $i -le $count; $i++) {
                    $record.GetType().InvokeMember('StringData', 'GetProperty', $null, $record, @($i))
                }
                $rows.Add(@($values))
            } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($record) }
        }
    } finally {
        [void]$view.Close()
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($view)
    }
    return ,$rows
}
function Assert-Msi([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw "MSI validation: $Message" }
}
try {
    $properties = @{}
    foreach ($row in (Read-MsiRows 'SELECT `Property`, `Value` FROM `Property`')) { $properties[$row[0]] = $row[1] }
    Assert-Msi ($properties.ProductName -eq 'Terminal') 'display name'
    Assert-Msi ($properties.ProductVersion -eq $Version) 'version'
    Assert-Msi ($properties.UpgradeCode -eq '{3A57FB13-D5F0-4A85-8CD0-768A2B988F9C}') 'stable upgrade identity'
    Assert-Msi ($properties.ALLUSERS -eq '1') 'per-machine scope'
    Assert-Msi ($properties.ARPPRODUCTICON -eq 'TerminalIcon') 'Installed Apps icon'
    $summary = $database.SummaryInformation(0)
    try {
        $expected = if ($Architecture -eq 'x86_64') { 'x64;1033' } else { 'Arm64;1033' }
        $template = $summary.GetType().InvokeMember('Property', 'GetProperty', $null, $summary, @(7))
        Assert-Msi ($template -eq $expected) 'native architecture template'
    } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($summary) }
    $files = Read-MsiRows 'SELECT `File`, `FileName`, `FileSize` FROM `File`'
    Assert-Msi ($files.Count -eq 2) 'only executable and license payload'
    $binary = @($files | Where-Object { $_[0] -eq 'TerminalExe' })
    Assert-Msi ($binary.Count -eq 1 -and $binary[0][1] -match '(^|\|)terminal\.exe$' -and [int]$binary[0][2] -gt 0) 'terminal.exe payload'
    $directories = Read-MsiRows 'SELECT `Directory_Parent`, `DefaultDir` FROM `Directory` WHERE `Directory` = ''INSTALLFOLDER'''
    Assert-Msi ($directories.Count -eq 1 -and $directories[0][0] -eq 'ProgramFiles64Folder' -and $directories[0][1] -match '(^|\|)Terminal$') 'stable native Program Files location'
    $shortcuts = Read-MsiRows 'SELECT `Name`, `Target`, `Directory_`, `Icon_` FROM `Shortcut`'
    Assert-Msi ($shortcuts.Count -eq 1 -and $shortcuts[0][0] -match '(^|\|)Terminal$' -and $shortcuts[0][1] -eq '[#TerminalExe]' -and $shortcuts[0][2] -eq 'ProgramMenuFolder' -and $shortcuts[0][3] -eq 'TerminalIcon') 'Start Menu shortcut and icon; no desktop shortcut'
    $identity = Read-MsiRows 'SELECT `PropVariantValue` FROM `MsiShortcutProperty` WHERE `Shortcut_` = ''TerminalShortcut'' AND `PropertyKey` = ''System.AppUserModel.ID'''
    Assert-Msi ($identity.Count -eq 1 -and $identity[0][0] -eq 'io.github.xdjanisxd.terminal') 'stable Start Menu application identity'
    $icons = Read-MsiRows 'SELECT `Name` FROM `Icon` WHERE `Name` = ''TerminalIcon'''
    Assert-Msi ($icons.Count -eq 1) 'embedded installer icon'
    $environment = Read-MsiRows 'SELECT `Name`, `Value` FROM `Environment`'
    Assert-Msi ($environment.Count -eq 1 -and $environment[0][0].Contains('*') -and $environment[0][0].Contains('-') -and $environment[0][0].EndsWith('PATH') -and $environment[0][1] -eq '[~];[TERMINAL_INSTALL_PATH]') 'append-only, removable system PATH entry'
    $pathCondition = Read-MsiRows 'SELECT `Condition` FROM `Component` WHERE `Component` = ''SystemPath'''
    Assert-Msi ($pathCondition.Count -eq 1 -and $pathCondition[0][0] -eq 'NOT (EXISTING_SYSTEM_PATH ~>< TERMINAL_INSTALL_PATH) OR (WIX_UPGRADE_DETECTED AND EXISTING_PATH_OWNED)') 'pre-existing PATH guard and upgrade ownership transfer'
    $upgrades = Read-MsiRows 'SELECT `UpgradeCode` FROM `Upgrade`'
    Assert-Msi ($upgrades.Count -ge 2) 'major upgrade and downgrade detection'
    $dialogs = Read-MsiRows 'SELECT `Dialog` FROM `Dialog` WHERE `Dialog` = ''WelcomeEulaDlg'''
    Assert-Msi ($dialogs.Count -eq 1) 'normal interactive installer UI'
    Write-Host "Validated MSI tables: $Architecture, $Version, files, branding, shortcuts, PATH, upgrades"
} finally {
    [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($database)
    [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($installer)
}
