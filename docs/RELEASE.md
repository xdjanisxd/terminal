# Releases

The current `Release` GitHub Actions workflow verifies, builds, packages, and smoke-tests Linux GNU binaries only. Each archive contains `terminal`, `README.md`, `config.example.toml`, and `LICENSE`; Linux archives also contain the per-user installer, desktop integration guide, desktop entry, and hicolor icons. The example config is reference material and is not installed as an active `config.toml`. See the [Linux release-readiness record](LINUX_RELEASE_READINESS.md) for RC evidence and the [final Linux sign-off](LINUX_RELEASE_SIGNOFF.md). Windows MSI scripts remain in the repository but are not part of the current workflow. macOS packaging is not configured in the current workflow.

| Runner | Rust target | Release asset |
| --- | --- | --- |
| `ubuntu-24.04` | `x86_64-unknown-linux-gnu` | `terminal-linux-x86_64.tar.gz` |
| `ubuntu-24.04-arm` | `aarch64-unknown-linux-gnu` | `terminal-linux-aarch64.tar.gz` |

Both package jobs are native Ubuntu builds. The workflow installs Python 3.14 and pinned Pillow 12.1.0, checks committed branding, then runs formatting, workspace checks/tests, doc tests, and Clippy on Linux runners. Branding validation compares decoded pixels, exact dimensions and formats, ICO resolutions, the Linux icon path set, and rejects embedded image metadata/color profiles. Its package smoke checks the archive manifest and executable CLI from a temporary directory without opening a window. The Linux shared-library check uses `ldd`. Interactive shell and GPU behavior still need native acceptance; CI configuration alone does not establish that evidence.

The Linux binaries target GNU libc and are dynamically linked. Users need a compatible GNU libc system plus graphics, window-system, and font support. A successful runner smoke test does not establish compatibility with older distributions or every desktop environment. The tested build's observed ABI floor and environment are recorded in the Linux readiness and sign-off reports.

The proposed Linux release is built from workspace package version `0.2.2`, following the latest existing tag `v0.2.1`. The executable reports it with `terminal --version` or `terminal -V`. Published asset names remain target-stable; local artifacts use `terminal-linux-x86_64.tar.gz` and `terminal-linux-aarch64.tar.gz`.

## Publishing

After final release sign-off, merge the release commit to `master` and push a stable tag in the form `vX.Y.Z` (for example, `v0.2.2`). The workflow rejects prerelease tags for publication. A stable tag push runs the two Linux verification and packaging jobs, then creates a GitHub Release with `terminal-linux-x86_64.tar.gz` and `terminal-linux-aarch64.tar.gz`. A branch workflow dispatch uploads inspection artifacts and does not publish. Do not dispatch this workflow from a tag during unpublished candidate validation.

Packaging and local smoke testing can be repeated on a matching native host after a release build:

```sh
cargo build --locked --release -p terminal-app --bin terminal --target <rust-target>
python scripts/package_release.py <rust-target>
```

The script rejects a target that does not match the host, avoiding a misleading executable smoke result from cross-compilation. Windows packaging also verifies that executable icon resources match the committed ICO.

## Manual Windows MSI tooling

This tooling is not part of the current release workflow and was not validated
by the Linux release candidate. Its manifest fallback reads the centralized
workspace version. MSI ProductVersion accepts only three numeric fields, so a
Cargo prerelease version is rejected; use a stable `-ReleaseTag` for a Windows
release build. No Windows runtime or installer implementation changed for this
Linux task.

WiX Toolset **6.0.2** is pinned in `.config/dotnet-tools.json`; the manual build script derives the matching UI extension version from that manifest. A Windows packaging run requires .NET 8, restores the local .NET tool, and compiles `installer/windows/terminal.wxs` with warnings treated as errors and normal MSI validation enabled. Authoring is source controlled and requires no GUI tools. Both `x64` and `arm64` MSI platforms are supported by the script. Native ARM64 execution and install/uninstall validation require an ARM64 Windows runner; this Linux candidate did not run those checks.

On Windows, after the existing release build:

```powershell
./scripts/build_windows_msi.ps1 -Target x86_64-pc-windows-msvc
# On an ARM64 native runner, use aarch64-pc-windows-msvc instead.
# For a release-version test build:
./scripts/build_windows_msi.ps1 -Target x86_64-pc-windows-msvc -ReleaseTag v1.2.3
```

The script uses the `vX.Y.Z` tag on release runs, and the workspace Cargo version for local builds. MSI supports three numeric fields with limits `255.255.65535`; prerelease versions fail rather than being silently truncated. The executable retains its Cargo-derived file version; MSI ProductVersion comes from the stable tag. Asset names do not contain the version, consistently with ZIP names.

The MSI is per-machine and requires the normal administrator consent. It installs only `terminal.exe` and `LICENSE` into the native 64-bit `Program Files\Terminal` directory, independent of any ZIP extraction. Windows ARM64 uses the ARM64 package and native Program Files directory. The **Terminal** shortcut goes into the common Start Menu, with the canonical icon; Installed Apps uses that icon too. The Windows executable, window, and taskbar use the same canonical branding assets. The Start Menu shortcut and installer metadata use the stable application identifier `io.github.xdjanisxd.terminal`. No desktop shortcut or directory-selection UI is offered in this first slice. Installation directory overrides are unsupported.

System PATH is enabled by default to make `terminal` available to all users. MSI's declarative Environment table appends one directory using `[~]`, preserves existing entries, and removes its own value on uninstall. No PowerShell or other executable installer custom actions are used; a declarative property assignment normalizes the fixed path before component selection. A non-transitive component condition conservatively skips PATH ownership when the existing system PATH contains the install path, case-insensitively (including trailing-slash variants). Major upgrades transfer ownership only when the previous MSI recorded ownership; the old package's entry is removed and recreated transactionally. A pre-existing user-managed entry remains the user's responsibility, including removal after uninstall. The conservative substring guard can also skip a longer path beginning with the same directory; resolve such unusual PATH layouts manually. Repairs retain component ownership and do not duplicate the entry. Do not rename or edit the installer-owned PATH value by hand. Existing processes keep their environment; open a new shell/session, or sign out and in if necessary.

Uninstall through Installed Apps removes the payload, common shortcut, installer-owned PATH value, and `HKLM\Software\io.github.xdjanisxd.terminal` installer values. MSI never installs a mandatory `config.toml` and never targets `%APPDATA%\terminal` or any Saved Workspaces location. User config and app-managed state survive uninstall and upgrades. The example config remains available in the ZIP/repository.

The shared, fixed UpgradeCode in `terminal.wxs` identifies Terminal across versions and architectures. WiX generates a new ProductCode for each package; newer three-field versions perform a major upgrade and older versions are blocked. Removal of the previous version is scheduled after transaction initialization, before installing the replacement, allowing rollback and avoiding PATH/shortcut overlap. Only one architecture of Terminal should be installed at a time. Same-version packages are inspection/repair builds, not upgrades; increment the release tag for every shipped upgrade and keep the UpgradeCode unchanged.

### Validation

`build_windows_msi.ps1` rejects a mismatched executable PE architecture or missing executable branding metadata. `validate_windows_msi.ps1` reads the MSI database and checks ProductVersion, native architecture template, per-machine scope, stable UpgradeCode, executable/license payload, Program Files location, icon, common Start Menu target, Environment row/ownership condition, UI, and upgrade rows. WiX then extracts the cabinet and embedded icon; SHA-256 hashes must match the already-built binary, license, and canonical ICO. Extraction runs no installation and changes no machine state. Intermediate files and extension cache are ignored under `target/` and `.wix/`.

On a disposable GitHub-hosted Windows runner, `smoke_windows_msi.ps1` refuses to proceed if installation files, shortcut, registry metadata, or a matching PATH entry already exist. It builds a higher-version test MSI from the same binary under `target/`, silently installs the MSI, checks `terminal.exe --help`, optional config, shortcut target/icon, and repair without duplicate PATH. It upgrades to the test MSI, checks old/new product registration and retained PATH ownership, then uninstalls in a `finally` block. It checks removal of files, shortcut, metadata, and exact restoration of the original PATH. A second install/uninstall models a pre-existing user-managed PATH entry and verifies it is never claimed or removed. It changes no current-process environment and never manually deletes application directories. MSI logs are stored under the runner's temporary directory. This smoke script deliberately rejects developer machines and self-hosted runners. It is not invoked by the current Linux-only workflow. Graphical/PTY startup, upgrade rollback, and retained user state require the manual checks below.

Windows MSI compilation, install/upgrade/uninstall, and clean-VM acceptance were
not run during this Linux-only preparation. The current automated release matrix
does not include Windows targets.

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

Both MSI and executable remain unsigned; there is no signing infrastructure in this slice. SmartScreen/security prompts may warn, and the installer does not weaken or bypass them. WiX/package validation does not establish trust or SmartScreen reputation. Signing, MSIX, Debian/RPM packaging, macOS app/DMG packaging, package-manager publication, and auto-update are deferred. Windows ARM64 and clean-VM graphical/upgrade checks are only confirmed after they actually run successfully; configuration alone is not validation evidence.
