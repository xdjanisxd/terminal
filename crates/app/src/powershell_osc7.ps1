# Loaded after the user's profile by the local PowerShell launch command.
if ($global:__terminalOsc7Installed) { return }

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
    $promptText
}
$global:__terminalOsc7Installed = $true
