# Loaded after the user's profile by the local PowerShell launch command.
if ($global:__terminalOsc7Installed) { return }
$__terminalStartupTimer = $null
if ($env:TERMINAL_STARTUP_DIAGNOSTICS -eq '1') {
    $__terminalStartupTimer = [System.Diagnostics.Stopwatch]::StartNew()
    [Console]::Write(("{0}]9;terminal-startup;shell-integration-begin{1}" -f [char]27, [char]7))
}

$originalPrompt = (Get-Command prompt -CommandType Function -ErrorAction SilentlyContinue).ScriptBlock
if ($null -eq $originalPrompt) { return }

$global:__terminalOsc7OriginalPrompt = $originalPrompt
function global:prompt {
    $promptText = & $global:__terminalOsc7OriginalPrompt
    try {
        $location = Get-Location -ErrorAction Stop
        if ($location.Provider.Name -eq 'FileSystem') {
            $uri = [System.Uri]::new($location.ProviderPath)
            if ($uri.IsAbsoluteUri -and $uri.IsFile) {
                $sequence = '{0}]7;{1}{2}' -f [char]27, $uri.AbsoluteUri, [char]7
                [Console]::Write($sequence)
            }
        }
    } catch {
        # Metadata must never prevent the original prompt from rendering.
    }
    # The app queues saved startup input only after the first prompt is ready.
    [Console]::Write(('{0}]133;A{1}' -f [char]27, [char]7))
    $promptText
}
$global:__terminalOsc7Installed = $true

if ($null -ne $__terminalStartupTimer) {
    $__terminalStartupTimer.Stop()
    $__terminalStartupUs = [long]($__terminalStartupTimer.ElapsedTicks * 1000000 / [System.Diagnostics.Stopwatch]::Frequency)
    [Console]::Write(("{0}]9;terminal-startup;shell-integration-end shell_duration_us={1}{2}" -f [char]27, $__terminalStartupUs, [char]7))
}
