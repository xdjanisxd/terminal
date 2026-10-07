# Windows release reproduction only. Observation limits do not alter application shutdown.
param([ValidateRange(1, 100)][int]$Runs = 5, [string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
$repository = Split-Path $PSScriptRoot -Parent
$executable = Join-Path $repository 'target/release/terminal.exe'
if (!$OutputDirectory) { $OutputDirectory = Join-Path $repository ('target/shutdown-' + [Guid]::NewGuid().ToString('N')) }
$null = New-Item -ItemType Directory -Path $OutputDirectory -Force
$OutputDirectory = (Resolve-Path -LiteralPath $OutputDirectory).Path
$pwsh = (Get-Command pwsh.exe).Source
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class TerminalShutdownProbe {
    private delegate bool Callback(IntPtr window, IntPtr parameter);
    [DllImport("user32.dll")] private static extern bool EnumWindows(Callback callback, IntPtr parameter);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll")] private static extern int GetClassName(IntPtr window, StringBuilder name, int count);
    [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll", SetLastError = true)] private static extern bool PostMessage(IntPtr window, uint message, IntPtr wparam, IntPtr lparam);
    // Process.MainWindowHandle can select winit's initially visible event-target helper.
    public static IntPtr Window(uint process, bool visible) {
        IntPtr result = IntPtr.Zero;
        EnumWindows((window, parameter) => {
            uint owner; GetWindowThreadProcessId(window, out owner);
            var name = new StringBuilder(256); GetClassName(window, name, name.Capacity);
            if (owner == process && name.ToString() == "Window Class" && (!visible || IsWindowVisible(window))) {
                result = window; return false;
            }
            return true;
        }, IntPtr.Zero);
        return result;
    }
    public static bool Close(IntPtr window) { return PostMessage(window, 0x0010, IntPtr.Zero, IntPtr.Zero); }
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)] private struct Entry {
        public uint size, usage, id; public UIntPtr heap; public uint module, threads, parent;
        public int priority; public uint flags;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)] public string executable;
    }
    [DllImport("kernel32.dll")] private static extern IntPtr CreateToolhelp32Snapshot(uint flags, uint process);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] private static extern bool Process32First(IntPtr snapshot, ref Entry entry);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] private static extern bool Process32Next(IntPtr snapshot, ref Entry entry);
    [DllImport("kernel32.dll")] private static extern bool CloseHandle(IntPtr handle);
    public static uint[] Descendants(uint process) {
        var snapshot = CreateToolhelp32Snapshot(2, 0);
        if (snapshot == new IntPtr(-1)) throw new InvalidOperationException("Process snapshot failed");
        try {
            var parents = new Dictionary<uint, uint>(); var entry = new Entry(); entry.size = (uint)Marshal.SizeOf(entry);
            if (Process32First(snapshot, ref entry)) do { parents[entry.id] = entry.parent; } while (Process32Next(snapshot, ref entry));
            var result = new HashSet<uint> { process }; bool changed;
            do { changed = false; foreach (var pair in parents) if (result.Contains(pair.Value) && result.Add(pair.Key)) changed = true; } while (changed);
            result.Remove(process); var ids = new uint[result.Count]; result.CopyTo(ids); return ids;
        } finally { CloseHandle(snapshot); }
    }
}
'@
function Observe-Children($Process, $Observed) {
    foreach ($processId in [TerminalShutdownProbe]::Descendants($Process.Id)) {
        if (!$Observed.ContainsKey($processId)) {
            try { $candidate = [Diagnostics.Process]::GetProcessById($processId); $Observed[$processId] = @{ Process=$candidate; Started=$candidate.StartTime } } catch { }
        }
    }
}
function Read-Shared($Path) {
    if (!(Test-Path -LiteralPath $Path)) { return '' }
    $stream = [IO.File]::Open($Path, 'Open', 'Read', 'ReadWrite')
    $reader = [IO.StreamReader]::new($stream)
    try { return $reader.ReadToEnd() } finally { $reader.Dispose() }
}
$rows = [Collections.Generic.List[object]]::new()
$oldDiagnostics = $env:TERMINAL_STARTUP_DIAGNOSTICS
$oldLog = $env:TERMINAL_STARTUP_LOG
try {
    $env:TERMINAL_STARTUP_DIAGNOSTICS = '1'
    foreach ($shell in @('cmd', 'pwsh-no-profile', 'pwsh-profile')) {
        $program = if ($shell -eq 'cmd') { 'cmd.exe' } else { $pwsh }
        $arguments = if ($shell -eq 'pwsh-no-profile') { '["-NoProfile"]' } else { '[]' }
        foreach ($panes in @(1, 3)) {
            $leaf = '{ kind = "pane", session = "local_shell" }'
            $layout = if ($panes -eq 1) { $leaf } else { '{ kind = "split", axis = "vertical", first = ' + $leaf + ', second = { kind = "split", axis = "horizontal", first = ' + $leaf + ', second = ' + $leaf + ' } }' }
            $workspace = Join-Path $OutputDirectory "$shell-$panes.toml"
            [IO.File]::WriteAllText($workspace, @"
[shell]
program = '$program'
args = $arguments
[workspace]
[[workspace.tabs]]
title = "Shutdown reproduction"
layout = $layout
"@)
            foreach ($mode in @('immediate', 'visible', 'ready')) {
                foreach ($run in 1..$Runs) {
                    $prefix = Join-Path $OutputDirectory "$shell-$panes-$mode-$run"
                    $env:TERMINAL_STARTUP_LOG = "$prefix.log"
                    $child = Start-Process -FilePath $executable -ArgumentList @('--workspace', ('"' + $workspace + '"')) -WindowStyle Hidden -PassThru -RedirectStandardError "$prefix.stderr"
                    $clock = [Diagnostics.Stopwatch]::StartNew()
                    $ready = $false
                    $closeAccepted = $false
                    $forced = $false
                    $observed = @{}
                    try {
                        while (!$child.HasExited -and $clock.Elapsed.TotalSeconds -lt 20) {
                            $child.Refresh()
                            $text = Read-Shared "$prefix.log"
                            $stage = if ($shell -eq 'cmd') { 'first-shell-output-rendered' } else { 'prompt-ready' }
                            $ready = ([regex]::Matches($text, "stage=$stage session=")).Count -ge $panes
                            $window = [TerminalShutdownProbe]::Window($child.Id, $mode -ne 'immediate')
                            if ($window -ne [IntPtr]::Zero -and ($mode -ne 'ready' -or $ready)) {
                                Observe-Children $child $observed
                                $closeAccepted = [TerminalShutdownProbe]::Close($window)
                                if ($closeAccepted) { break }
                            }
                            Start-Sleep -Milliseconds 5
                        }
                        $closeMs = $clock.Elapsed.TotalMilliseconds
                        $exitClock = [Diagnostics.Stopwatch]::StartNew()
                        while (!$child.WaitForExit(10) -and $exitClock.ElapsedMilliseconds -lt 10000) { Observe-Children $child $observed }
                        $natural = $child.HasExited
                        $exitMs = $clock.Elapsed.TotalMilliseconds - $closeMs
                        Observe-Children $child $observed
                        $survivors = @($observed.Values | Where-Object { try { !$_.Process.HasExited -and $_.Process.StartTime -eq $_.Started } catch { $false } })
                        if (!$natural) {
                            $forced = $true
                            # Cleanup only this harness-owned process tree after recording failure.
                            $child.Kill($true)
                            $child.WaitForExit()
                        }
                        $row = [pscustomobject]@{ Shell=$shell; Panes=$panes; Mode=$mode; Run=$run; Pid=$child.Id; Ready=$ready; CloseAccepted=$closeAccepted; CloseMs=[math]::Round($closeMs,1); ExitMs=[math]::Round($exitMs,1); Forced=$forced; ExitCode=$child.ExitCode; ObservedChildren=$observed.Count; SurvivingChildren=($survivors.Process.Id -join ',') }
                        $rows.Add($row)
                        $rows | Export-Csv -LiteralPath (Join-Path $OutputDirectory 'runs.csv') -NoTypeInformation
                        Write-Output ($row | ConvertTo-Json -Compress)
                    } finally {
                        if (!$child.HasExited) { $child.Kill($true); $child.WaitForExit() }
                        foreach ($entry in $observed.Values) {
                            if (!$entry.Process.HasExited -and $entry.Process.StartTime -eq $entry.Started) { $entry.Process.Kill($true); $entry.Process.WaitForExit() }
                            $entry.Process.Dispose()
                        }
                        $child.Dispose()
                    }
                }
            }
        }
    }
} finally {
    $env:TERMINAL_STARTUP_DIAGNOSTICS = $oldDiagnostics
    $env:TERMINAL_STARTUP_LOG = $oldLog
}
Write-Output "Results: $OutputDirectory"
if ($rows | Where-Object { $_.Forced -or !$_.CloseAccepted -or ($_.Mode -eq 'ready' -and !$_.Ready) -or $_.ExitCode -ne 0 -or $_.SurvivingChildren }) { throw 'Shutdown/readiness failure; inspect runs.csv and logs.' }
