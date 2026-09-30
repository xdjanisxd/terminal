# Current Task

Goal: isolate the legacy Windows PowerShell ARM64 ConPTY startup failure.

Branch: `feat/configurable-shell`. No commit or push authorized.

The instrumented ARM64 smoke confirms CPR was generated and successfully written;
legacy PowerShell still times out before its command body produces output.
The new non-ignored Windows boundary matrix passes all 28 launches on x86_64.
Production policy and the original smoke criteria remain unchanged.

Files: app test module/main.rs, `.agent/SESSION.md`, this file and
[boundary investigation](ARM64_POWERSHELL_CONPTY.md).

Next step: run `native_powershell_boundary_probe -- --nocapture` on ARM64 and
identify the first failing transport/command boundary before choosing a fix.
Executable identity and interpretation guidance are in the linked investigation.

Validated locally: all 28 diagnostic launches, four dedicated original smoke
runs, configurable-shell/reply tests, 128 app tests, 639 workspace tests (four
existing ignored), four doc tests, fmt, workspace check, Clippy and diff checks.
Remaining validation: ARM64 boundary matrix and ownership/fix confirmation.
