# Legacy PowerShell / ARM64 ConPTY boundary investigation

The root cause remains unresolved. ARM64 run 36714617811 confirms that the smoke
harness generated, queued and successfully wrote CPR `CSI 1;1 R`. Legacy Windows
PowerShell nevertheless remained running for 15 seconds without executing the
marker or producing OSC 7/133. PowerShell 7 and cmd succeeded on that runner.
Successful pipe writes do not prove that ConPTY consumed the reply or that the
PowerShell host completed initialization. The log does not justify attributing
the failure to Terminal, portable-pty, Windows, emulation or shell integration.

That run's setup log reports Windows 11 Enterprise 10.0.26200 and runner image
`windows-11-vs2026-arm64`, version `20260920.164.1`. It does not report the legacy
executable's PE machine, runtime process architecture or PowerShell version;
those require the new probe. The System32 path alone does not establish them.

## Reproducer

Run on the affected Windows machine from the repository root:

```powershell
cargo test -p terminal-app --bin terminal native_powershell_boundary_probe -- --nocapture
```

This non-ignored Windows test runs each available PowerShell executable through
both the production portable-pty backend and ordinary redirected CreateProcess
(CREATE_NO_WINDOW, open redirected stdin, captured stdout/stderr). Explicit
ShellConfig program/args pass through the production shell policy. Command mode
prevents automatic interactive hook injection; integration is introduced only
in cases E/F. ConPTY uses the existing SmokeTerminal parser/typed reply path and
SmokeSession recorder of actual PTY writes. Every case requires natural exit 0;
B–G also require their output marker. F additionally requires OSC 7 and OSC 133.
No process-spawn-only success, timeout increase or architecture exception exists.

| Case | Body / observation |
| --- | --- |
| A | `-Command "exit 0"` |
| B | `-Command "Write-Output 'TERMINAL_MINIMAL_MARKER'; exit 0"` |
| C | `-Command "[Console]::WriteLine('TERMINAL_MINIMAL_MARKER'); exit 0"` |
| D | UTF-16LE/base64 `-EncodedCommand` equivalent of B |
| E | Beginning marker, production integration script, installation flag, B |
| F | Beginning marker, script, installed marker, explicit `& prompt`, B; verify OSC 7, then OSC 133 |
| G | Version, process/OS architecture and Windows version, then B |

All use `-NoLogo -NoProfile -NonInteractive`. OSC verification does not change
the command invocation; it is a separate observation of F. The matrix continues
after failed cases and lists all failing executable/case/transport boundaries.
It records exact program/args, spawn time, first output time, elapsed time,
natural exit/state before cleanup, queries, generated/queued/completed replies,
write errors, rendered output, markers, OSC observations and raw output.

Executable identity includes the resolved path and PE machine field. G reports
actual process and OS architecture via RuntimeInformation when its body executes.
A redirected G can distinguish emulation (process architecture differs from OS)
even if ConPTY G hangs. PE ARM64EC/ARM64X labels alone are not proof of runtime
architecture. The backend exposes no child handle/PID; actual architecture of a
child that never executes G remains unknown rather than inferred from its path.

## Local Windows x86_64 results

All 28 launches pass (A–G, two shells, two transports). Minimal exit, both marker
writes, encoded command, script loading and explicit prompt all succeed. F emits
OSC 7 and OSC 133 with both transports. Encoding does not change the local result.
There is no local failing boundary.

- OS: Windows 10.0.26200.0, OS/process architecture X64.
- Legacy: System32 WindowsPowerShell/v1.0/powershell.exe, PE x64,
  PowerShell 5.1.26100.9549.
- Control: installed full-path pwsh.exe, PE x64, PowerShell 7.6.6.

Local logs are under ignored `target/powershell-boundary.log`. An initial probe
assertion missed the redirected F marker because OSC preceded it on the same
line. The corrected redirected check uses the unique output substring; redirected
processes do not echo their supplied command. ConPTY still checks parsed terminal
cells, not command text/title output.

## Backend / host boundary

portable-pty 0.9.0 creates ConPTY with INHERIT_CURSOR, RESIZE_QUIRK and
WIN32_INPUT_MODE flags, then attaches the child using STARTUPINFOEXW and
CreateProcessW with EXTENDED_STARTUPINFO_PRESENT/CREATE_UNICODE_ENVIRONMENT.
It sets standard handles invalid for the ConPTY child; redirected controls use
ordinary pipes instead. No backend/dependency or production API changes were made.

The initial CSI 6 n is consistent with ConPTY cursor inheritance, not evidence
that the supplied PowerShell body executed. Microsoft documents the need to
[service the inherited cursor query](https://learn.microsoft.com/en-us/windows/console/createpseudoconsole).
CSI ?9001 h selects [Win32 keyboard input encoding](https://raw.githubusercontent.com/microsoft/terminal/main/doc/specs/%234999%20-%20Improved%20keyboard%20handling%20in%20Conpty.md);
CSI ?1004 h enables focus reporting. These mode changes do not request an
immediate readiness reply. Titles/attributes/cursor visibility can be produced
by console initialization before the supplied command starts. No extra input
events or unrequested terminal replies are injected. Whether the ARM64 legacy
host blocks in a console API requires the matrix result and potentially a native
stack trace; current output cannot establish that.

## Next decision

Run this exact matrix on ARM64 before changing the native smoke matrix:

- A–D ConPTY fail, redirected pass: isolate the ConPTY/host/backend boundary,
  independent of integration. This still does not prove which component owns it.
- D passes while B fails: investigate command-line construction/parsing.
- A–D pass and E fails: investigate script installation.
- E passes and F fails: investigate explicit prompt/OSC execution.
- Everything passes: investigate original smoke invocation and intermittent host
  initialization with the recorded executable identity.

Keep the current default/cmd/PATH-pwsh/full-path legacy/native failure coverage.
No proven external limitation currently justifies dropping or exempting legacy
PowerShell. ARM64 confirmation and the final ownership decision remain pending.
