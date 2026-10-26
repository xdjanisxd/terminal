# Linux release readiness

Audit date: 2026-10-26. Candidate: `0.2.2-rc.1`. No release was published.
The clean task branch starts at integrated `master` commit `3c731b5`.

## Verified and resolved

- [x] Version metadata is centralized in `[workspace.package]`; all seven
  workspace crates, Cargo.lock entries, and the executable report
  `0.2.2-rc.1`. The version follows the latest existing tag, `v0.2.1`.
- [x] Standalone `terminal --version` and `terminal -V` return before startup
  diagnostics, event-loop/window creation, GPU initialization, and shell spawn.
  Packaging smoke removes display variables and asserts both flags do not
  create the opt-in startup diagnostic log.
- [x] Formatting, workspace check, all-target tests, doc tests, and Clippy
  passed with Rust 1.99.0: 898 tests passed, 31 ignored; 4 doc tests passed.
- [x] Locked Linux x86_64 release build passed for
  `x86_64-unknown-linux-gnu`.
- [x] `python3 scripts/test_install_linux.py`: 10 tests passed.
  `python3 scripts/test_package_release.py`: passed.
  Branding check passed for all 10 platform icon assets with Pillow 12.1.0.
- [x] `scripts/package_release.py` created the portable archive. Repeating the
  package step produced identical bytes. The archive contains the executable,
  README, example config, license, installer, Linux desktop guide, canonical
  desktop entry, and eight icons; package checks verify file manifest, modes,
  bytes, executable permission, and `--help`, `--version`, and `-V` smoke.
- [x] RC artifact:
  `dist/terminal-linux-x86_64-0.2.2-rc.1.tar.gz`.
  Checksum sidecar:
  `dist/terminal-linux-x86_64-0.2.2-rc.1.tar.gz.sha256`.
  Size: 5,530,191 bytes. SHA-256:
  `1d423bfcd958bbd03f2e55c98fc25fbadc66b73b4e00f7f9308af96b44734459`.
  `sha256sum -c` passed. Published workflow asset names remain stable and omit
  the version; the local RC artifact name includes it.
- [x] Isolated clean install, upgrade from a release binary built from the
  existing `v0.2.1` tag, and uninstall passed with temporary HOME/XDG
  config/data/cache paths. Upgrade installed the RC executable and retained
  config, workspace, and unrelated sentinels. The
  desktop entry validated; all eight installed PNG icons had expected sizes.
  Uninstall removed owned files and retained user data.
- [x] Automated `gio launch` coverage passed the launcher's exact working
  directory, including spaces and Unicode, to a shell stub. The native KDE
  CWD test also passed for direct and installed launches.
- [x] Native Linux Bash/Zsh lifecycle acceptance passed 12 scenarios covering
  normal and nonzero exit, startup exit, tabs, splits, mixed layouts, shell
  reaping, integration-file cleanup, and Saved Workspace preservation.
- [x] The release-profile Bash/Zsh lifecycle and direct/installed CWD tests
  passed on KDE Wayland. Seven native GPU renderer tests passed in both debug
  and release profiles with fixture-backed JetBrains Mono and installed Noto
  Color Emoji fonts.
- [x] One-cycle release resource stress passed for one pane, three panes, three
  tabs, and two passes over 32 MiB of Unicode output. All app/shell processes
  exited. Peak RSS was 152,988 KiB; measured idle CPU medians were 0–1.99%.
  These bounded samples are not a soak or universal performance claim.
- [x] Direct ELF dependencies resolved with `ldd`: `libgcc_s.so.1`, `libm.so.6`,
  `libc.so.6`, and the ELF loader. No missing direct library was reported.
  ELF symbol inspection found a `GLIBC_2.39` ABI floor.

## Performance and stability

The existing seven-run CPU benchmark gate was run twice. Both runs reported
threshold exceedances, but neither is a valid candidate regression result:
the baseline was captured at load average `1.0`, while these runs recorded
`3.79` and `4.66`. Parser and renderer source are identical to the benchmark
baseline commit; this candidate changes package metadata and the early CLI
version path only. No performance change was made and no regression is
confirmed. Repeat the comparison on a quiet host before making a performance
claim. Existing stress/lifecycle evidence is not a multi-hour soak.

The three-layout startup/idle benchmark reached first frame and shell output,
then hit one KWin scripted-close timeout on its first sample. A release close
probe and the subsequent four-scenario stress run both closed cleanly with the
same KWin close path; no shutdown hang reproduced. The timing dataset is
incomplete and is not reported as a pass.

## Runtime and compatibility

This archive is a dynamically linked x86_64 GNU/Linux ELF. Its versioned symbol
requirements include `GLIBC_2.39`; systems with older glibc cannot run it. The
application also needs a supported Wayland or X11 session, graphics driver and
window-system libraries, Vulkan/GPU support, and installed fonts. `ldd` reports
direct ELF dependencies; it does not enumerate libraries loaded dynamically by
the windowing and graphics backends.

The candidate was built and exercised on Debian 13 x86_64, KDE Plasma Wayland,
Intel Iris Xe, and Vulkan. The GitHub workflow configures Ubuntu 24.04 builds,
but that is not interactive runtime evidence. No support claim is made for
older distributions, GNOME, standalone X11, or untested GPU/driver combinations.
The archive is portable, not a Debian package. The installer uses Python 3.11+
standard library and needs no root access.

## Outstanding release blockers

- [ ] Before publishing a stable release, complete the interactive KDE checks
  below for launcher/KRunner discovery, task-manager icon/grouping, and Dolphin
  **Open Terminal Here** behavior. Automated checks do not prove those UI paths.

No automated package, workspace, CLI, or native lifecycle blocker is confirmed.

## Non-blocking limitations

- Terminal implements a bounded VT/xterm subset, not full xterm/VTE semantics;
  see [current state](CURRENT_STATE.md) and [configuration](CONFIG.md).
- Emoji and some grapheme rendering depend on installed fonts and supported
  font formats.
- Compatibility is verified on Debian 13 x86_64 only. The glibc symbol floor
  is an ABI requirement, not proof of support for every distribution meeting it.
- The CPU benchmark gate was inconclusive under elevated host load; no timing
  claim is made from those runs.
- The local archive is unsigned. Its checksum sidecar detects accidental
  corruption but is not a signed authenticity proof.

## Manual acceptance in a disposable KDE user session

Do not install this candidate over a developer's existing Terminal. Use a
disposable KDE user account/session with the candidate archive copied into a
working directory:

```sh
sha256sum -c terminal-linux-x86_64-0.2.2-rc.1.tar.gz.sha256
mkdir terminal-linux-x86_64-0.2.2-rc.1
tar -xzf terminal-linux-x86_64-0.2.2-rc.1.tar.gz -C terminal-linux-x86_64-0.2.2-rc.1
cd terminal-linux-x86_64-0.2.2-rc.1
./terminal --version
./terminal -V
python3 install_linux.py install
desktop-file-validate "$HOME/.local/share/applications/io.github.xdjanisxd.terminal.desktop"
kbuildsycoca6 --noincremental
"$HOME/.local/bin/terminal" --version
```

Then use the application menu and KRunner to launch Terminal; check its task
manager grouping/icon and shell prompt. In Dolphin, open a directory with spaces
and Unicode using **Open Terminal Here**, then run `pwd`. Repeat with a configured
project root and confirm that root keeps precedence. In the terminal, run:

```sh
printf '\033[38;2;255;80;60mRGB foreground\033[48;2;10;30;50m RGB background\033[0m 👩‍💻 é 日本語\n'
seq 1 1000
```

Exercise resize/reflow, tabs, splits, and valid then invalid configuration
reload. Create `config.toml` and save one workspace in the disposable account.
Close panes and the final window; confirm shells exit and no child remains.
Uninstall and verify user data remains:

```sh
python3 install_linux.py uninstall
test -f "$HOME/.config/terminal/config.toml"
test -f "$HOME/.config/terminal/workspaces.toml"
```

Use the actual config/workspace paths created in the disposable account if they
differ from these defaults. Remove manually pinned launcher favorites only if
KDE retains one after uninstall.
