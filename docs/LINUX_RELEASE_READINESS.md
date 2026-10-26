# Linux release readiness

Audit date: 2026-10-26. Scope: Linux release archive, per-user installer,
desktop integration, upgrades, and uninstall. No release was published.

## Verified on Debian 13 x86_64

- [x] Release branch starts at integrated master `01fa8a3`.
- [x] `cargo fmt --all -- --check`, workspace check, all-target tests, doc tests,
  and Clippy passed with Rust 1.99.0. Results: 898 tests passed, 31 ignored;
  4 doc tests passed; Clippy reported no issues.
- [x] `cargo build --locked --release -p terminal-app --bin terminal --target
  x86_64-unknown-linux-gnu --offline` passed.
- [x] `python3 scripts/test_install_linux.py`: 10 tests passed.
- [x] `python3 scripts/test_package_release.py`: passed.
- [x] `python scripts/generate_branding.py --check`: passed for 10 platform
  icon assets, using pinned Pillow 12.1.0.
- [x] Linux archive packaging and extracted-binary `--help` smoke passed.
  Repeated archive builds produced identical SHA-256:
  `a1c4187194d4a7a3e42c1728f24e85bf36f51f2e44d08108acd7af7e843eb64a`.
- [x] Isolated archive install, upgrade, and uninstall passed with HOME, config,
  data, and cache roots under a temporary directory. Paths included spaces and
  non-ASCII characters.
- [x] Installed binary launched with `--help` and had owner execute permission.
  `desktop-file-validate` accepted the installed entry. All eight installed
  icons were valid PNG files; branding validation checked their dimensions.
- [x] Upgrade replaced the installed binary. Sentinel `config.toml`, saved
  workspace data, and an unrelated file retained identical contents.
- [x] Uninstall removed the owned executable, desktop entry, and icons. It
  preserved user settings and the unrelated file.
- [x] `gio launch` of the installed desktop entry passed the launcher's exact
  working directory, including spaces and Unicode, to a shell stub.
- [x] Direct shared-library inspection found `libgcc_s.so.1`, `libm.so.6`,
  `libc.so.6`, and the ELF loader. `ldd` reported no missing libraries.
- [x] Debian 13 package output is a dynamically linked x86_64 GNU/Linux ELF.
  The release archive is a portable `.tar.gz`, not a Debian package.

The installer verifies ownership-record storage before the first application
file. This prevents an initial ownership-record permission failure from leaving
an untracked binary. A focused regression test covers this failure.

## Paths and ownership

- Installer executable path is `$HOME/.local/bin/terminal`.
- Desktop entry and icons use absolute `XDG_DATA_HOME`, or
  `$HOME/.local/share` when it is missing or relative.
- Installer does not read `XDG_BIN_HOME`. Configuration and saved workspace
  files remain under the application's existing `XDG_CONFIG_HOME` policy and
  are not installer-owned.
- Use the same HOME and XDG_DATA_HOME values to upgrade or uninstall. The
  installer preserves an unowned legacy `terminal.desktop` and warns about it.
- Destination conflicts, modified owned files, and symlinks stop install or
  uninstall before file removal. Shared directories remain in place.
- Ownership-record write failure was simulated in a regression test. A real
  read-only mount, ACL denial, or quota exhaustion was not exercised.

## Runtime libraries and compatibility

The direct ELF dependencies observed on this Debian 13 x86_64 build are the
GNU C library, math library, GCC support library, and ELF loader. The application
also needs working graphics, window-system, and font support at runtime. The
`--help` packaging smoke does not initialize those systems. Release CI targets
Ubuntu 24.04 runners; this audit does not establish compatibility with older
Debian/Ubuntu releases, other distributions, or every desktop environment.

The Cargo package version is `0.0.0`; the binary has no `--version` option.
Release tags identify published assets, but local binary version reporting is
not verified or provided by this audit.

## Desktop checks still needed

- [ ] On a live KDE session, search and launch Terminal from the application
  launcher/KRunner, then check task-manager grouping and icon.
- [ ] In Dolphin, use **Open Terminal Here** from a directory with spaces and
  non-ASCII characters. Run `pwd` and verify it matches Dolphin's directory.
- [ ] Repeat Dolphin launch with a configured project or workspace root and
  verify the configured root keeps precedence.
- [ ] Check launcher disappearance after uninstall and remove any manually
  pinned favorite if KDE retains it.

These interactive checks were unavailable in this run: the sandbox denied
access to the user session D-Bus, and no Dolphin, KWin, or Plasma shell process
was running. Automated desktop-entry validation and `gio launch` CWD coverage
do not prove KDE menu, task-manager, or Dolphin UI behavior.
