# Windows validation

This page records the local Windows checks run for this revision. Automated
tests cover the Windows build, configuration, PTY lifecycle, and native
PowerShell behavior. They do not replace a visual check of the GPU window.
The user launched the release executable and reported that it seems to work,
but the font looked poor; the active user config now selects Cascadia Mono.

Validation date: 2026-11-13. Repository HEAD at the time: `83dba58`.

## Host and results

- Host: Windows 11, x64 (`Microsoft Windows 10.0.26300`), PowerShell 5.1.26100.9711.
- Rust: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, host target `x86_64-pc-windows-msvc`; Cargo 1.99.1.
- `cargo fmt --all -- --check`: passed.
- `cargo check --workspace --all-targets`: passed.
- `cargo test --workspace --all-targets`: passed; 912 passed, 0 failed, 20 ignored across 75 test harnesses.
- `cargo test --workspace --doc`: passed; 4 passed, 0 failed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- Locked release build for `x86_64-pc-windows-msvc`: passed. The executable is `target/x86_64-pc-windows-msvc/release/terminal.exe` (10,082,816 bytes).

The 20 ignored tests remain opt-in and are not counted as verified. The
workspace tests include native PowerShell PTY probes, including Ctrl+C
interruption and PSReadLine word navigation.

## Windows package checks

- `scripts/test_package_release.py`: passed (1 test).
- `scripts/generate_branding.py --check`: passed (10 icon assets). This local
  run used the bundled Python environment's Pillow 13.0.0; CI installs the
  repository-pinned Pillow 12.1.0.
- `scripts/package_release.py x86_64-pc-windows-msvc`: passed, producing
  `dist/terminal-windows-x86_64.zip` and checking the archive manifest,
  embedded icon, extracted files, and executable CLI.
- `scripts/build_windows_msi.ps1 -Target x86_64-pc-windows-msvc`: passed with
  .NET SDK 8.0.100 and repository-pinned WiX 6.0.2. MSI database validation
  checked product metadata, architecture, branding, payloads, shortcuts, the
  PATH row and ownership condition, upgrade rows, and UI. WiX cabinet/icon
  extraction also matched the source executable, license, and ICO hashes. Output:
  `dist/terminal-windows-x86_64.msi` (version 0.2.2).
- MSI installation, repair, upgrade, and uninstall were not run locally. The
  smoke script intentionally restricts machine changes to disposable
  GitHub-hosted runners; the manual `Windows MSI` workflow runs those checks.

## Commands and logs

Run from the repository root with Cargo on `PATH`:

```powershell
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo test --workspace --doc
cargo clippy --workspace --all-targets -- -D warnings
cargo build --locked --release -p terminal-app --bin terminal --target x86_64-pc-windows-msvc
```

Captured local output is under the ignored `target/windows-validation/`
directory: `test-workspace-escalated.log`, `test-doc.log`, `clippy.log`, and
`release-build.log`. The initial sandboxed run could not access PowerShell's
user PSReadLine history file and failed the Ctrl+C native smoke test. The exact
test passed when rerun with normal host access, and the complete workspace
test run also passed with normal host access. No test assertion or product code
was changed to work around that sandbox restriction.

## Manual acceptance still needed

These checks require an interactive Windows desktop and are not established by
the automated results above:

- Inspect text and rendering quality in the already-launched app after
  restarting it to load the updated font setting.
- In a PowerShell pane, verify Ctrl+C interrupts a running foreground command
  and returns to a usable prompt; verify Ctrl+Left and Ctrl+Right move by words.
- Open tabs and horizontal/vertical splits, then confirm focus and pane resizing.
- Change a pane's working directory, save and reopen a workspace, and confirm
  its layout and pane directories restore.

The MSI install, repair, upgrade, and uninstall smoke tests are handled by the
Windows installer validation workflow, separately from this local Rust run.

## Windows default working directory investigation

The installed MSI registry entry reports version `0.2.2`. The common Start
Menu link targets `Program Files\Terminal\terminal.exe`; its recorded working
directory at inspection time was the current account's profile folder. The
user could not find a Start in field and reported a Program Files working
directory from direct executable launches. I could not confirm the inspected
common link was the shortcut the user launched, so the shortcut alone does not
explain the behavior. No MSI install was run locally. The MSI authoring remains
unchanged while the app's default launch-directory behavior is handled and
validated separately.

A temporary Shell Link probe confirmed Windows expands a literal
`%USERPROFILE%` when launching that link. This does not prove Windows Installer
will preserve the variable in a per-machine shortcut, so MSI database checks
alone are not treated as evidence of the installed shortcut's runtime behavior.

## Normal Windows shell launch default (2026-11-14)

Normal Windows local-shell sessions now start in the launching user's valid
`USERPROFILE` when no explicit or inherited root is configured. This is set in
the app's PTY spawn configuration, so it also applies when launching the EXE
directly. Explicit and inherited roots retain precedence. Direct `-e`/
`--execute` commands keep the caller's working directory.

The Windows workspace test run passed: 912 passed, 0 failed, and 20 ignored
across 75 harnesses. The native shell smoke deliberately ran with process CWD
`crates/app` while the expected home was the user profile, then confirmed both
PowerShell `Get-Location` and CMD `%CD%` reported that profile. The targeted
probe passed 1/1, and the CLI `-e` caller-CWD regression test passed in the full
suite.

After the runtime change, the x64 MSI was rebuilt with unchanged shortcut
authoring and its MSI tables and extracted payload hashes validated. The
portable ZIP was regenerated and its manifest, icon, and executable CLI smoke
passed. The inspected installation is version `0.2.2`; to make an upgrade
candidate without editing the workspace version, the local MSI was built with
`-ReleaseTag v0.2.6`. This sets the MSI ProductVersion to `0.2.6`, while the
embedded executable still reports the Cargo version `0.2.2`. This is a local
test package only: no source version change, Git tag, or publication was made.
I did not install the MSI locally; the user later confirmed the MSI now works.
`cargo build --release` updates the build artifact, not an existing installation
under Program Files. MSI install/repair/upgrade/uninstall smoke remains a
GitHub-hosted-runner check.

Current local artifacts:

- `dist/terminal-windows-x86_64.msi` (ProductVersion `0.2.6`, EXE version
  `0.2.2`): 3,182,592 bytes; SHA-256
  `A0B3A55214F78C965C86C8B7D1EACB1EE37248223B79CF3C00AA9BE750882824`.
- `dist/terminal-windows-x86_64.zip`: 3,752,211 bytes; SHA-256
  `865A472E43E55834ADB6D290CD5C296E503B179A9E038EC1763A476AED89B5A2`.
