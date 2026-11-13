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
