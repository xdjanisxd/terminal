# Releases

The `Release` GitHub Actions workflow verifies, builds, packages, and smoke-tests native binaries. Every archive contains exactly `terminal` (`terminal.exe` on Windows), `README.md`, `config.example.toml`, and `LICENSE`. The example config is reference material and is not installed as an active `config.toml`. Windows jobs also build MSI installers from the same executable, without another Cargo build.

| Runner | Rust target | Release asset |
| --- | --- | --- |
| `windows-2025` | `x86_64-pc-windows-msvc` | `terminal-windows-x86_64.zip`, `terminal-windows-x86_64.msi` |
| `windows-11-vs2026-arm` | `aarch64-pc-windows-msvc` | `terminal-windows-aarch64.zip`, `terminal-windows-aarch64.msi` |
| `ubuntu-24.04` | `x86_64-unknown-linux-gnu` | `terminal-linux-x86_64.tar.gz` |
| `ubuntu-24.04-arm` | `aarch64-unknown-linux-gnu` | `terminal-linux-aarch64.tar.gz` |
| `macos-15-intel` | `x86_64-apple-darwin` | `terminal-macos-x86_64.tar.gz` |
| `macos-15` | `aarch64-apple-darwin` | `terminal-macos-aarch64.tar.gz` |

These are native builds. The workflow installs Python 3.14 and pinned Pillow 12.1.0 on each runner and checks the committed branding set before running packaging tests. Branding validation compares decoded pixels, exact dimensions and formats, all ICO/ICNS resolutions, the Linux icon path set, and rejects embedded image metadata/color profiles. It intentionally does not compare compressed PNG bytes: Pillow's Windows wheel uses zlib-ng while other platforms can use a different zlib implementation, and both may encode the same pixels differently. Check mode is read-only, so Linux CI validates the committed files without rewriting them. The workflow also runs workspace formatting, check, unit and integration tests, doc tests, and Clippy on each runner before release builds. Its packaging step checks the archive manifest and launches the extracted release executable with `--help` from a temporary directory outside the checkout. Existing native PTY tests and Windows PowerShell tests remain in the workspace test suite. The smoke test does not create a graphical window; interactive shell and GPU behavior still need human testing on each platform. All six runner combinations are configured for CI validation; they are only confirmed as built once the workflow has completed successfully on GitHub.

The Linux binaries target GNU libc and are dynamically linked. The workflow checks `ldd` for missing shared libraries on its build runners. Users need a compatible GNU libc system and installed graphics, window-system, and font libraries; compatibility with older distributions or every desktop environment is not implied by a successful runner smoke test. The macOS archives contain a command-line binary, not an app bundle, and are unsigned and not notarized. Gatekeeper may require the user to approve opening the binary. The Windows PowerShell OSC 7 integration script is compiled into the executable; it does not need a file from the source checkout.

## Publishing

Merge a validated change to `master`, then push an annotated or lightweight tag in the form `vX.Y.Z` (for example, `v0.1.0`) at the release commit. A tag push runs the same six verification and packaging jobs. Only after all succeed does the workflow create a GitHub Release with the six archives and two MSI installers attached. The workflow uses the repository's built-in `GITHUB_TOKEN` with release contents permission; it needs no separate secret. Ordinary master pushes, pull requests, and manual workflow runs upload inspection artifacts but do not publish a Release. Other branch pushes do not trigger this workflow. A failed target or installer smoke test blocks publication of the complete eight-asset release set.

Packaging and local smoke testing can be repeated on a matching native host after a release build:

```sh
cargo build --locked --release -p terminal-app --bin terminal --target <rust-target>
python scripts/package_release.py <rust-target>
```

The script rejects a target that does not match the host, avoiding a misleading executable smoke result from cross-compilation. Windows packaging also verifies that executable icon resources match the committed ICO.

## Windows MSI

WiX Toolset **6.0.2** is pinned in `.config/dotnet-tools.json`; the build script derives the matching UI extension version from that manifest. CI installs .NET 8, restores the local .NET tool, and compiles `installer/windows/terminal.wxs` with warnings treated as errors and normal MSI validation enabled. Authoring is source controlled and requires no GUI tools. Both `x64` and `arm64` MSI platforms are supported. ARM64 uses the existing native ARM64 runner for application execution and install/uninstall validation; it is not replaced by an emulated x64 build.

On Windows, after the existing release build:

```powershell
./scripts/build_windows_msi.ps1 -Target x86_64-pc-windows-msvc
# On an ARM64 native runner, use aarch64-pc-windows-msvc instead.
# For a release-version test build:
./scripts/build_windows_msi.ps1 -Target x86_64-pc-windows-msvc -ReleaseTag v1.2.3
```

The script uses the `vX.Y.Z` tag on release runs, and `crates/app/Cargo.toml`'s package version for local/PR runs. The package currently uses `0.0.0`, so inspection builds use that version; they must not be distributed as versioned releases. MSI supports three numeric fields with limits `255.255.65535`; invalid or prerelease tags fail rather than being silently truncated. The executable retains its Cargo-derived file version; release MSI ProductVersion comes from the tag. Asset names do not contain the version, consistently with ZIP names.

The MSI is per-machine and requires the normal administrator consent. It installs only `terminal.exe` and `LICENSE` into the native 64-bit `Program Files\Terminal` directory, independent of any ZIP extraction. Windows ARM64 uses the ARM64 package and native Program Files directory. The **Terminal** shortcut goes into the common Start Menu, with the canonical icon; Installed Apps uses that icon too. The Windows executable, window, and taskbar use the same canonical branding assets. The Start Menu shortcut and installer metadata use the stable application identifier `io.github.xdjanisxd.terminal`. No desktop shortcut or directory-selection UI is offered in this first slice. Installation directory overrides are unsupported.

System PATH is enabled by default to make `terminal` available to all users. MSI's declarative Environment table appends one directory using `[~]`, preserves existing entries, and removes its own value on uninstall. No PowerShell or other executable installer custom actions are used; a declarative property assignment normalizes the fixed path before component selection. A non-transitive component condition conservatively skips PATH ownership when the existing system PATH contains the install path, case-insensitively (including trailing-slash variants). Major upgrades transfer ownership only when the previous MSI recorded ownership; the old package's entry is removed and recreated transactionally. A pre-existing user-managed entry remains the user's responsibility, including removal after uninstall. The conservative substring guard can also skip a longer path beginning with the same directory; resolve such unusual PATH layouts manually. Repairs retain component ownership and do not duplicate the entry. Do not rename or edit the installer-owned PATH value by hand. Existing processes keep their environment; open a new shell/session, or sign out and in if necessary.

Uninstall through Installed Apps removes the payload, common shortcut, installer-owned PATH value, and `HKLM\Software\io.github.xdjanisxd.terminal` installer values. MSI never installs a mandatory `config.toml` and never targets `%APPDATA%\terminal` or any Saved Workspaces location. User config and app-managed state survive uninstall and upgrades. The example config remains available in the ZIP/repository.

The shared, fixed UpgradeCode in `terminal.wxs` identifies Terminal across versions and architectures. WiX generates a new ProductCode for each package; newer three-field versions perform a major upgrade and older versions are blocked. Removal of the previous version is scheduled after transaction initialization, before installing the replacement, allowing rollback and avoiding PATH/shortcut overlap. Only one architecture of Terminal should be installed at a time. Same-version packages are inspection/repair builds, not upgrades; increment the release tag for every shipped upgrade and keep the UpgradeCode unchanged.

### Validation

`build_windows_msi.ps1` rejects a mismatched executable PE architecture or missing executable branding metadata. `validate_windows_msi.ps1` reads the MSI database and checks ProductVersion, native architecture template, per-machine scope, stable UpgradeCode, executable/license payload, Program Files location, icon, common Start Menu target, Environment row/ownership condition, UI, and upgrade rows. WiX then extracts the cabinet and embedded icon; SHA-256 hashes must match the already-built binary, license, and canonical ICO. Extraction runs no installation and changes no machine state. Intermediate files and extension cache are ignored under `target/` and `.wix/`; only `.msi` files are uploaded.

On disposable GitHub-hosted Windows runners, `smoke_windows_msi.ps1` refuses to proceed if installation files, shortcut, registry metadata, or a matching PATH entry already exist. It builds a higher-version test MSI from the same binary under `target/` (never uploaded as a release asset), silently installs the release MSI, checks `terminal.exe --help`, optional config, shortcut target/icon, and repair without duplicate PATH. It upgrades to the test MSI, checks old/new product registration and retained PATH ownership, then uninstalls in a `finally` block. It checks removal of files, shortcut, metadata, and exact restoration of the original PATH. A second install/uninstall models a pre-existing user-managed PATH entry and verifies it is never claimed or removed. It changes no current-process environment and never manually deletes application directories. MSI logs are stored under the runner's temporary directory and uploaded on failure. This smoke script deliberately rejects developer machines and self-hosted runners. Graphical/PTY startup, upgrade rollback, and retained user state require the manual checks below.

Local validation for this task produced the x86_64 MSI at Cargo version `0.0.0` and test tag version `0.1.1`, passed MSI compilation/table/payload checks, and smoke-tested the preserved x86_64 ZIP. Local ARM64 cross-compilation could not link because an ARM64 MSVC linker was unavailable; ARM64 is still configured on its native CI runner. Hosted installation tests and clean-VM acceptance have not run locally. Do not treat the x86_64 package's metadata checks as evidence of ARM64 installation success.

### Manual acceptance on a clean Windows VM

Repeat for x64 and ARM64:

1. Download the matching MSI and install using its normal UI.
2. Verify **Terminal** appears in Installed Apps with its icon and version.
3. Launch **Terminal** from Start Menu; verify Start Menu, taskbar, and window icons.
4. Verify normal shell/PTY startup and interactive input.
5. Open a new PowerShell session and run `terminal --help`; if the parent environment is stale, sign out and in first.
6. Verify it starts without user config. Create a test user config and Saved Workspace through normal app behavior and record their contents.
7. Install a newer test MSI (higher three-field version); verify a single Installed Apps entry, new binary, unchanged user state, working shortcut, and one PATH entry. Verify an older MSI is rejected.
8. Uninstall through Installed Apps. Verify installed files, shortcut, owned PATH entry, and installer registry values are removed.
9. Verify user config and Saved Workspaces still exist unchanged.
10. Separately test an existing install-directory PATH entry before installation: reinstall/repair must not duplicate it and uninstall must preserve it. No desktop shortcut should appear.

### Unsigned limitation and deferred work

Both MSI and executable remain unsigned; there is no signing infrastructure in this slice. SmartScreen/security prompts may warn, and the installer does not weaken or bypass them. WiX/package validation does not establish trust or SmartScreen reputation. Signing, MSIX, Linux installers, macOS app/DMG packaging, package-manager publication, and auto-update are deferred. Native ARM64 CI, install/uninstall smoke, and the clean-VM graphical/upgrade checks are only confirmed after they actually run successfully; configuration alone is not validation evidence.
