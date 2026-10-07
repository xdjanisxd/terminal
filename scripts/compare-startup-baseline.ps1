# Measurement only: fresh processes, unchanged caches, matched shell/config inputs.
param(
    [ValidateRange(1, 30)][int]$Runs = 15,
    [ValidateRange(0, 10)][int]$Warmups = 2,
    [string]$OutputDirectory = 'target/perf/alacritty-baseline-compare',
    [string]$Alacritty = 'C:/Program Files/Alacritty/alacritty.exe',
    [string[]]$Scenarios = @('cmd', 'no-profile-hook', 'normal-hook'),
    [ValidateSet('terminal', 'alacritty')][string[]]$Applications = @('terminal', 'alacritty'),
    [ValidateRange(2, 60)][int]$ObservationSeconds = 12,
    [switch]$SkipLauncherPrime,
    [switch]$QuietLogs
)
$ErrorActionPreference = 'Stop'
$repository = Split-Path $PSScriptRoot -Parent
$terminal = Join-Path $repository 'target/release/terminal.exe'
if (!(Test-Path -LiteralPath $terminal)) { throw 'Build cargo build --release -p terminal-app first.' }
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Use a fresh output directory.' }
$null = New-Item -ItemType Directory -Path $OutputDirectory
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
    public static int Width, Height, Dpi;
    [DllImport("user32.dll")] private static extern uint GetDpiForWindow(IntPtr hwnd);
    public static int Observe(int pid) {
        int flags = 0;
        EnumWindows((hwnd, unused) => {
            uint owner; Rect rect;
            GetWindowThreadProcessId(hwnd, out owner);
            if (owner == pid && GetClientRect(hwnd, out rect) && rect.Right > 100 && rect.Bottom > 100) {
                flags |= 1; Width = rect.Right; Height = rect.Bottom; Dpi = (int)GetDpiForWindow(hwnd);
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
    'no-profile-hook' = @{ Program = $pwsh; Arguments = @('-NoProfile', '-NoExit', '-EncodedCommand', $encodedHook) }
    'normal-hook' = @{ Program = $pwsh; Arguments = @('-NoExit', '-EncodedCommand', $encodedHook) }
}
foreach ($scenario in $Scenarios) {
    if (!$definitions.ContainsKey($scenario)) { throw "Unknown scenario $scenario" }
    $definition = $definitions[$scenario]
    $program = TomlString $definition.Program
    $arguments = ($definition.Arguments | ForEach-Object { TomlString $_ }) -join ', '
    $root = TomlString $repository
    # Preserve the authoritative active config; use explicit commands for identical probes.
    $base = [IO.File]::ReadAllText((Join-Path $env:APPDATA 'terminal/config.toml'))
    if ([regex]::Matches($base, '(?m)^session\s*=\s*"local_shell"').Count -ne 1) { throw 'Expected one LocalShell pane.' }
    $commandTable = "[workspace.tabs.layout.session.command]
program = $program
args = [$arguments]"
    $text = [regex]::Replace($base, '(?m)^session\s*=\s*"local_shell"[^\r\n]*', [System.Text.RegularExpressions.MatchEvaluator]{ param($match) $commandTable })
    [IO.File]::WriteAllText((Join-Path $output "terminal-$scenario.toml"), $text)
    [IO.File]::WriteAllText((Join-Path $output "alacritty-$scenario.toml"), @"
[general]
live_config_reload = false
[window]
padding = { x = 0, y = 0 }
dynamic_padding = false
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
$probeDir = Join-Path $output 'clink-probe'
$null = New-Item -ItemType Directory -Path $probeDir
# Additional CLINK_PATH scripts supplement the configured profile; no settings writes.
[IO.File]::WriteAllText((Join-Path $probeDir 'startup-readiness.lua'), @'
local recorded = false
clink.onbeginedit(function()
    if recorded then return end
    local path = os.getenv("STARTUP_COMPARISON_PROMPT")
    if not path then return end
    local file = io.open(path, "w")
    if file then file:write("clink-onbeginedit"); file:close(); recorded = true end
end)
'@)
$sourceFiles = @($terminal, $Alacritty, $pwsh, $cmd, (Join-Path $env:APPDATA 'terminal/config.toml'), (Join-Path $env:APPDATA 'alacritty/alacritty.toml'), (Join-Path $repository 'crates/app/src/powershell_osc7.ps1'))
$files = @($sourceFiles | ForEach-Object { $item=Get-Item -LiteralPath $_; [pscustomobject]@{ Path=$item.FullName; Bytes=$item.Length; Sha256=(Get-FileHash -LiteralPath $_).Hash } })
$os = Get-CimInstance Win32_OperatingSystem
$profilePaths = @(& $pwsh -NoProfile -Command '$PROFILE.AllUsersAllHosts; $PROFILE.AllUsersCurrentHost; $PROFILE.CurrentUserAllHosts; $PROFILE.CurrentUserCurrentHost')
$profiles = @($profilePaths | ForEach-Object { [pscustomobject]@{ Path=$_; Exists=(Test-Path -LiteralPath $_); Sha256=if(Test-Path -LiteralPath $_){(Get-FileHash -LiteralPath $_).Hash}else{$null} } })
@{ Revision=(git -C $repository rev-parse HEAD); Branch=(git -C $repository branch --show-current); Architecture=$env:PROCESSOR_ARCHITECTURE; OS=$os.Caption; Build=$os.BuildNumber; CPU=(Get-CimInstance Win32_Processor | Select-Object -First 1).Name; GPU=@(Get-CimInstance Win32_VideoController | Select-Object Name,DriverVersion); Files=$files; Profiles=$profiles; CmdAutoRun=(Get-ItemProperty -LiteralPath 'HKCU:/Software/Microsoft/Command Processor').AutoRun; AlacrittyVersion=(& $Alacritty --version); PwshVersion=(& $pwsh -NoProfile -Command '$PSVersionTable.PSVersion.ToString()'); Runs=$Runs; Warmups=$Warmups; Scenarios=$Scenarios; WorkingDirectory=$repository; Cache='warm/repeated fresh processes; unchanged/uncontrolled caches'; Prompt='pwsh: original prompt returns, then UTC timestamp file; cmd: Clink onbeginedit writes file, OS last-write UTC'; Instrumentation=if($QuietLogs){'Terminal startup diagnostics and Alacritty verbose/event logging disabled; identical per-shell probe'}else{'Terminal startup diagnostics; Alacritty -vvv --print-events; identical per-shell probe'}; StartedUtc=[DateTime]::UtcNow.ToString('o') } | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath (Join-Path $output 'environment.json')
$rows = [Collections.Generic.List[object]]::new()
$cleanup = [Collections.Generic.List[object]]::new()
for ($cycle = 1; $cycle -le ($Warmups + $Runs); $cycle++) {
    $warmup = $cycle -le $Warmups
    $run = if ($warmup) { $cycle - $Warmups - 1 } else { $cycle - $Warmups }
    for ($offset = 0; $offset -lt $Scenarios.Count; $offset++) {
        $scenario = $Scenarios[($cycle - 1 + $offset) % $Scenarios.Count]
        # Alternating order limits systematic first/second-launch bias.
        $apps = @(if ($cycle % 2) { 'terminal'; 'alacritty' } else { 'alacritty'; 'terminal' }) | Where-Object { $_ -in $Applications }
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
            if (!$QuietLogs) { $info.Environment['TERMINAL_STARTUP_DIAGNOSTICS'] = '1' } else { $null = $info.Environment.Remove('TERMINAL_STARTUP_DIAGNOSTICS') }
            $info.Environment['TERMINAL_STARTUP_LOG'] = "$prefix.log"
            $info.Environment['STARTUP_COMPARISON_PROMPT'] = "$prefix.prompt"
            if ($scenario -eq 'cmd') {
                $info.Environment['CLINK_PATH'] = if ($env:CLINK_PATH) { "$($env:CLINK_PATH);$probeDir" } else { $probeDir }
            }
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
            $maxPoll = 0.0; $lastPoll = 0.0; $finished = $null; $descendants = @()
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
                        $ready = if ($scenario.EndsWith('-hook') -or $scenario -eq 'cmd') {
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
                    $prompt = if ($scenario -eq 'cmd') {
                        ((Get-Item -LiteralPath "$prefix.prompt").LastWriteTimeUtc - $utc).TotalMilliseconds
                    } else { ([long](ReadShared "$prefix.prompt") - $utc.Ticks) / 10000.0 }
                }
                if ($null -eq $visible -or $null -eq $prompt -or $child.HasExited) { throw "Missing visibility/readiness or early exit: $prefix" }
                $drift = ([DateTime]::UtcNow - $utc).TotalMilliseconds - $clock.Elapsed.TotalMilliseconds
                # Query shell provenance only after all measured endpoints.
                $descendants = @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$($child.Id)")
                $shell = @($descendants | Where-Object { $_.Name -eq [IO.Path]::GetFileName($definitions[$scenario].Program) })
                if ($shell.Count -ne 1) { throw "Expected exactly one direct shell child: $prefix" }
                $descendants | Select-Object Name,ProcessId,ParentProcessId,CreationDate,CommandLine | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath "$prefix.children.json"
                $row = [pscustomobject]@{
                    App = $app; Scenario = $scenario; Run = $run; Pid = $child.Id
                    Warmup = $warmup; Cache = 'warm/repeated-fresh-process; unchanged/uncontrolled caches'
                    LaunchUtc = $utc.ToString('o'); StartReturnedMs = $startReturned
                    ProcessCreatedMs = $createdMs
                    ShellProcessCreatedMs = ($shell[0].CreationDate.ToUniversalTime() - $utc).TotalMilliseconds
                    Width = [StartupWindows]::Width; Height = [StartupWindows]::Height; Dpi = [StartupWindows]::Dpi
                    ClockDriftMs = $drift
                    WindowExistsMs = $exists; WindowVisibleMs = $visible; InputIdleMs = $idle
                    FocusMs = $focus; PromptFunctionMs = $prompt; MaxPollGapMs = $maxPoll
                    ExitedEarly = $child.HasExited; ObservedMs = $clock.Elapsed.TotalMilliseconds
                }
            } finally {
                $forced = $false
                if (!$child.HasExited) {
                    # Close only the measured app, with tree cleanup on timeout.
                    $null = $child.CloseMainWindow()
                    if (!$child.WaitForExit(3000)) { $forced = $true; $child.Kill($true); $child.WaitForExit() }
                }
                [IO.File]::WriteAllText("$prefix.stdout", $stdout.GetAwaiter().GetResult())
                [IO.File]::WriteAllText("$prefix.stderr", $stderr.GetAwaiter().GetResult())
                if ($app -eq 'alacritty') {
                    $nativeLog = Join-Path $pairTemp "Alacritty-$($child.Id).log"
                    if (Test-Path -LiteralPath $nativeLog) { Copy-Item -LiteralPath $nativeLog -Destination "$prefix.log" }
                }
                $survivors = @($descendants | Where-Object { Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue })
                $cleanup.Add([pscustomobject]@{ App=$app; Scenario=$scenario; Run=$run; Warmup=$warmup; ExitCode=$child.ExitCode; Forced=$forced; SurvivingChildren=$survivors.Count })
                $cleanup | Export-Csv -LiteralPath (Join-Path $output 'cleanup.csv') -NoTypeInformation
                $child.Dispose()
                if ($forced -or $survivors.Count) { throw "Abnormal cleanup: $prefix" }
            }
            $rows.Add($row)
            $rows | Export-Csv -LiteralPath (Join-Path $output 'all-external.csv') -NoTypeInformation
            $rows | Where-Object { !$_.Warmup } | Export-Csv -LiteralPath (Join-Path $output 'external.csv') -NoTypeInformation
            Write-Output "$app $scenario $run exists=$([Math]::Round($exists,1)) visible=$([Math]::Round($visible,1)) prompt=$prompt"
        }
    }
}

$post = @($sourceFiles | ForEach-Object { [pscustomobject]@{ Path=(Get-Item -LiteralPath $_).FullName; Sha256=(Get-FileHash -LiteralPath $_).Hash } })
$post | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $output 'post-hashes.json')
foreach ($file in $files) { if (($post | Where-Object Path -eq $file.Path).Sha256 -ne $file.Sha256) { throw "Source changed during collection: $($file.Path)" } }

foreach ($row in $rows) { if ($row.Width -ne 800 -or $row.Height -ne 600 -or $row.Dpi -ne 96) { throw "Unexpected window dimensions/DPI: $($row.App) $($row.Scenario) $($row.Run)" } }
