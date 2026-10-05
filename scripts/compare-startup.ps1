# Measurement only: fresh processes, unchanged caches, matched shell/config inputs.
param(
    [ValidateRange(1, 30)][int]$Runs = 7,
    [string]$OutputDirectory = 'target/startup-comparison',
    [string]$Alacritty = 'C:/Program Files/Alacritty/alacritty.exe',
    [string[]]$Scenarios = @('cmd', 'no-profile', 'normal', 'no-profile-hook', 'normal-hook'),
    [ValidateSet('terminal', 'alacritty')][string[]]$Applications = @('terminal', 'alacritty'),
    [ValidateRange(2, 60)][int]$ObservationSeconds = 12,
    [switch]$SkipLauncherPrime,
    [switch]$QuietLogs
)
$ErrorActionPreference = 'Stop'
$repository = Split-Path $PSScriptRoot -Parent
$terminal = Join-Path $repository 'target/release/terminal.exe'
if (!(Test-Path -LiteralPath $terminal)) { throw 'Build cargo build --release -p terminal-app first.' }
$null = New-Item -ItemType Directory -Path $OutputDirectory -Force
$output = (Resolve-Path -LiteralPath $OutputDirectory).Path
if (Test-Path -LiteralPath (Join-Path $output 'external.csv')) {
    throw 'Use a fresh output directory: existing samples must not be mixed or overwritten.'
}
$pwsh = (Get-Command pwsh.exe).Source
$cmd = (Get-Command cmd.exe).Source

# EnumWindows includes hidden HWNDs; MainWindowHandle alone would miss them.
Add-Type -TypeDefinition @'
using System;
using System.Diagnostics;
using System.Runtime.InteropServices;
public static class StartupWindows {
    private delegate bool Callback(IntPtr hwnd, IntPtr state);
    [DllImport("user32.dll")] private static extern bool EnumWindows(Callback cb, IntPtr state);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] private static extern bool GetClientRect(IntPtr hwnd, out Rect rect);
    [DllImport("user32.dll")] private static extern IntPtr GetForegroundWindow();
    private struct Rect { public int Left, Top, Right, Bottom; }
    public static int Observe(int pid) {
        int flags = 0;
        EnumWindows((hwnd, unused) => {
            uint owner; Rect rect;
            GetWindowThreadProcessId(hwnd, out owner);
            if (owner == pid && GetClientRect(hwnd, out rect) && rect.Right > 100 && rect.Bottom > 100) {
                flags |= 1;
                if (IsWindowVisible(hwnd)) flags |= 2;
                if (GetForegroundWindow() == hwnd) flags |= 4;
            }
            return true;
        }, IntPtr.Zero);
        return flags;
    }
}
'@

function TomlString([string]$value) { ConvertTo-Json -InputObject $value -Compress }
function ReadShared([string]$path) {
    if (!(Test-Path -LiteralPath $path)) { return '' }
    $stream = [IO.File]::Open($path, 'Open', 'Read', 'ReadWrite')
    $reader = [IO.StreamReader]::new($stream)
    try { $reader.ReadToEnd() } finally { $reader.Dispose() }
}

$hook = [IO.File]::ReadAllText((Join-Path $repository 'crates/app/src/powershell_osc7.ps1'))
# Supplemental matched prompt observation: before prompt text returns, not a visible-frame timestamp.
$hook += @'

$global:__comparisonPrompt = (Get-Command prompt -CommandType Function).ScriptBlock
function global:prompt {
    $text = & $global:__comparisonPrompt
    if (!$global:__comparisonRecorded) {
        [IO.File]::WriteAllText($env:STARTUP_COMPARISON_PROMPT, [DateTime]::UtcNow.Ticks.ToString())
        $global:__comparisonRecorded = $true
    }
    $text
}
'@

$encodedHook = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($hook))
$definitions = @{
    'cmd' = @{ Program = $cmd; Arguments = @() }
    'no-profile' = @{ Program = $pwsh; Arguments = @('-NoProfile', '-NoLogo') }
    'normal' = @{ Program = $pwsh; Arguments = @('-NoLogo') }
    'no-profile-hook' = @{ Program = $pwsh; Arguments = @('-NoProfile', '-NoLogo', '-NoExit', '-EncodedCommand', $encodedHook) }
    'normal-hook' = @{ Program = $pwsh; Arguments = @('-NoLogo', '-NoExit', '-EncodedCommand', $encodedHook) }
}
foreach ($scenario in $Scenarios) {
    if (!$definitions.ContainsKey($scenario)) { throw "Unknown scenario $scenario" }
    $definition = $definitions[$scenario]
    $program = TomlString $definition.Program
    $arguments = ($definition.Arguments | ForEach-Object { TomlString $_ }) -join ', '
    $root = TomlString $repository
    # Explicit command sessions preserve exact args (no automatic Terminal PowerShell hook).
    [IO.File]::WriteAllText((Join-Path $output "terminal-$scenario.toml"), @"
[font]
family = "JetBrainsMono Nerd Font Mono"
size = 16
[theme]
background = "#000000"
[workspace]
project_root = $root
active_tab = 0
[[workspace.tabs]]
title = "Startup comparison"
[workspace.tabs.layout]
kind = "pane"
[workspace.tabs.layout.session.command]
program = $program
args = [$arguments]
"@)
    [IO.File]::WriteAllText((Join-Path $output "alacritty-$scenario.toml"), @"
[general]
live_config_reload = false
[font]
size = 12.0
[font.normal]
family = "JetBrainsMono Nerd Font Mono"
style = "Regular"
[colors.primary]
background = "#000000"
[terminal]
shell = { program = $program, args = [$arguments] }
[debug]
persistent_logging = true
"@)
}
# This normally primes process-launch/JIT paths, but also warms cmd's image.
# Single first-launch collection can omit it; launcher overhead then remains unprimed.
if (!$SkipLauncherPrime) {
    $prime = [Diagnostics.ProcessStartInfo]::new($cmd)
    $prime.UseShellExecute = $false
    $prime.CreateNoWindow = $true
    $prime.ArgumentList.Add('/d')
    $prime.ArgumentList.Add('/c')
    $prime.ArgumentList.Add('exit')
    $probe = [Diagnostics.Process]::Start($prime)
    $probe.WaitForExit()
    $probe.Dispose()
}
$null = [StartupWindows]::Observe(0)
$null = [Diagnostics.Stopwatch]::StartNew().Elapsed.TotalMilliseconds
$null = [DateTime]::UtcNow
$rows = [Collections.Generic.List[object]]::new()
foreach ($run in 1..$Runs) {
    foreach ($scenario in $Scenarios) {
        # Alternating order limits systematic first/second-launch bias.
        $apps = @(if ($run % 2) { 'terminal'; 'alacritty' } else { 'alacritty'; 'terminal' }) | Where-Object { $_ -in $Applications }
        foreach ($app in $apps) {
            $prefix = Join-Path $output "$app-$scenario-$run"
            # Alacritty uses create_new(TEMP/Alacritty-PID.log); isolate reused Windows PIDs.
            # Both applications in the pair inherit the same otherwise-empty temp directory.
            $pairTemp = Join-Path $output "temp-$scenario-$run"
            $null = New-Item -ItemType Directory -Path $pairTemp -Force
            $info = [Diagnostics.ProcessStartInfo]::new()
            $info.FileName = if ($app -eq 'terminal') { $terminal } else { $Alacritty }
            $info.WorkingDirectory = $repository
            $info.UseShellExecute = $false
            $info.CreateNoWindow = $true
            $info.RedirectStandardOutput = $true
            $info.RedirectStandardError = $true
            $info.Environment['TERMINAL_STARTUP_DIAGNOSTICS'] = '1'
            $info.Environment['TERMINAL_STARTUP_LOG'] = "$prefix.log"
            $info.Environment['STARTUP_COMPARISON_PROMPT'] = "$prefix.prompt"
            $info.Environment['TEMP'] = $pairTemp
            $info.Environment['TMP'] = $pairTemp
            foreach ($name in @('TERMINAL_RENDERER_DIAGNOSTICS', 'TERMINAL_RENDER_DIAGNOSTICS', 'TERMINAL_THROUGHPUT_DIAGNOSTICS', 'WGPU_BACKEND', 'ALACRITTY_LOG')) {
                $null = $info.Environment.Remove($name)
            }
            if ($app -eq 'terminal') {
                $info.ArgumentList.Add('--workspace')
            } else {
                $info.ArgumentList.Add('--config-file')
            }
            $info.ArgumentList.Add((Join-Path $output "$app-$scenario.toml"))
            if ($app -eq 'alacritty' -and !$QuietLogs) {
                $info.ArgumentList.Add('-vvv')
                $info.ArgumentList.Add('--print-events')
            }
            $child = [Diagnostics.Process]::new()
            $child.StartInfo = $info
            $clock = [Diagnostics.Stopwatch]::StartNew()
            $utc = [DateTime]::UtcNow
            $exists = $null; $visible = $null; $idle = $null; $focus = $null
            $maxPoll = 0.0; $lastPoll = 0.0; $finished = $null
            $null = $child.Start()
            $startReturned = $clock.Elapsed.TotalMilliseconds
            $createdUtc = $child.StartTime.ToUniversalTime()
            $createdMs = ($createdUtc - $utc).TotalMilliseconds
            $lastPoll = $startReturned
            $stdout = $child.StandardOutput.ReadToEndAsync()
            $stderr = $child.StandardError.ReadToEndAsync()
            try {
                while ($clock.Elapsed.TotalSeconds -lt $ObservationSeconds -and !$child.HasExited) {
                    $now = $clock.Elapsed.TotalMilliseconds
                    $maxPoll = [Math]::Max($maxPoll, $now - $lastPoll); $lastPoll = $now
                    $flags = [StartupWindows]::Observe($child.Id)
                    $observed = $clock.Elapsed.TotalMilliseconds
                    if ($null -eq $exists -and ($flags -band 1)) { $exists = $observed }
                    if ($null -eq $visible -and ($flags -band 2)) { $visible = $observed }
                    if ($null -eq $focus -and ($flags -band 4)) { $focus = $observed }
                    if ($null -eq $idle) {
                        try { if ($child.WaitForInputIdle(0)) { $idle = $clock.Elapsed.TotalMilliseconds } } catch { }
                    }
                    # Give each run the same minimum dwell, allow profiled shells longer.
                    if ($clock.Elapsed.TotalSeconds -gt 2) {
                        $ready = if ($scenario.EndsWith('-hook')) {
                            Test-Path -LiteralPath "$prefix.prompt"
                        } elseif ($app -eq 'terminal') {
                            (ReadShared "$prefix.log").Contains('stage=first-shell-output-rendered')
                        } else { $clock.Elapsed.TotalSeconds -gt 6 }
                        if ($ready -and $null -eq $finished) { $finished = $clock.Elapsed.TotalSeconds }
                        if ($null -ne $finished -and $clock.Elapsed.TotalSeconds -gt $finished + 0.5) { break }
                    }
                    Start-Sleep -Milliseconds 2
                }
                $prompt = $null
                if (Test-Path -LiteralPath "$prefix.prompt") {
                    $prompt = ([long](ReadShared "$prefix.prompt") - $utc.Ticks) / 10000.0
                }
                $row = [pscustomobject]@{
                    App = $app; Scenario = $scenario; Run = $run; Pid = $child.Id
                    Cache = if ($run -eq 1) { 'first-observed-uncontrolled-cache' } else { 'warm-fresh-process' }
                    LaunchUtc = $utc.ToString('o'); StartReturnedMs = $startReturned
                    ProcessCreatedMs = $createdMs
                    WindowExistsMs = $exists; WindowVisibleMs = $visible; InputIdleMs = $idle
                    FocusMs = $focus; PromptFunctionMs = $prompt; MaxPollGapMs = $maxPoll
                    ExitedEarly = $child.HasExited; ObservedMs = $clock.Elapsed.TotalMilliseconds
                }
            } finally {
                if (!$child.HasExited) {
                    # Close only the measured app, with tree cleanup on timeout.
                    $null = $child.CloseMainWindow()
                    if (!$child.WaitForExit(3000)) { $child.Kill($true); $child.WaitForExit() }
                }
                [IO.File]::WriteAllText("$prefix.stdout", $stdout.GetAwaiter().GetResult())
                [IO.File]::WriteAllText("$prefix.stderr", $stderr.GetAwaiter().GetResult())
                if ($app -eq 'alacritty') {
                    $nativeLog = Join-Path $pairTemp "Alacritty-$($child.Id).log"
                    if (Test-Path -LiteralPath $nativeLog) { Copy-Item -LiteralPath $nativeLog -Destination "$prefix.log" }
                }
                $child.Dispose()
            }
            $rows.Add($row)
            $rows | Export-Csv -LiteralPath (Join-Path $output 'external.csv') -NoTypeInformation
            Write-Output "$app $scenario $run exists=$([Math]::Round($exists,1)) visible=$([Math]::Round($visible,1)) prompt=$prompt"
        }
    }
}
