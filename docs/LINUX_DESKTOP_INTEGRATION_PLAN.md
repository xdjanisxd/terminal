# Linux desktop integration

Branch: `feat/linux-desktop-integration`. Do not commit or push.

Status: implementation and verification complete. Native observations and remaining
limitations are recorded in [the installation report](LINUX_DESKTOP_INTEGRATION.md#verification-record).

## Goal

Integrate Terminal into Linux application menus, desktop search, and taskbars/docks
using freedesktop conventions, existing branding, and a simple root-free user
installation. Desktop launches must resolve the installed executable independently
of an interactive shell's PATH or working directory. Preserve configuration,
shell selection, PTY initialization, and workspace startup behavior.

## Findings

- The requested branch is already checked out and the initial working tree is clean.
- Existing assets use `io.github.xdjanisxd.terminal`: a desktop entry and eight
  hicolor PNG sizes under `assets/branding/linux/share/`.
- The desktop entry currently uses `Exec=terminal` and `TryExec=terminal`, requiring
  PATH lookup. Its name, categories, icon name, and StartupWMClass already exist.
- `crates/app/src/branding.rs` sets the title and Windows icons, but does not set
  Linux Wayland app ID or X11 class. The branding README currently claims it does.
- Linux portable archives already include the branding share tree. They have no
  install/uninstall mechanism.
- Configuration uses TERMINAL_CONFIG or XDG_CONFIG_HOME/HOME. Unix default shell
  selection uses SHELL, falling back to `/bin/sh`; preserve these decisions.
- The current session reports KDE on Wayland, with an X display also exposed.
  desktop-file-validate, update-desktop-database, gtk-update-icon-cache,
  kbuildsycoca6, gio, gdbus, and xprop are available. An exposed X display does not
  establish that a separate native X11 desktop session is available.

## Plan

1. Confirm window creation, release archive tests, branding generator checks, and
   startup path handling before editing. Consult primary freedesktop/XDG and winit
   documentation for desktop entry escaping and native identity details.
2. Reuse the existing desktop entry and icons. Set matching Wayland app ID and
   X11 class through Linux-specific winit attributes in the existing branding
   module; keep platform details out of terminal-core and the renderer. Add useful
   desktop search keywords without changing the application name or identifier.
3. Add a small user install/uninstall script using ordinary Linux tools. Install
   a supplied built binary as `$HOME/.local/bin/terminal`; install the launcher and
   icons under `${XDG_DATA_HOME:-$HOME/.local/share}/applications` and
   `icons/hicolor/<size>x<size>/apps`. Respect absolute XDG overrides and handle
   invalid relative values according to XDG conventions. Require no sudo.
4. Generate the installed desktop entry from the committed asset with a correctly
   escaped absolute Exec and TryExec, without a shell wrapper or sourcing shell
   profiles. Use an explicit home working directory for desktop launches so they
   do not depend on the launcher's inherited directory. Configured workspace roots
   and shell behavior remain authoritative. Handle spaces and desktop entry
   reserved characters in installation paths.
5. Support repeat installation and removal of only Terminal-owned files. Preserve
   configuration and Saved Workspaces; do not delete shared icon directories.
   Diagnose missing inputs before changing installed files, avoid partial binary
   replacement, and guard against replacing an unrelated existing executable.
   Refresh relevant desktop/icon caches when the tools exist, without making
   optional cache tools mandatory dependencies.
6. Make installation available from both the source checkout and Linux portable
   archives using the same script and assets. Adjust package manifests and their
   existing tests only as needed. Keep branding generator validation aligned.
7. Add `docs/LINUX_DESKTOP_INTEGRATION.md` with build/install/uninstall commands,
   XDG destinations, update behavior, menu/search and pinning checks, troubleshooting,
   and the distinction between desktop-session and interactive-shell environments.
   Update README and branding documentation to describe the actual behavior.
8. Validate the installer in temporary HOME/XDG directories, including alternate
   data roots, paths with spaces/reserved characters, missing binary, reinstall,
   uninstall, and preservation of unrelated files/config. Validate generated
   desktop entries and verify direct executable resolution with a restricted PATH
   and unrelated working directory. Use focused tests for escaping and destructive
   removal boundaries rather than mirroring every script operation.
9. Run appropriate Rust checks for the native identity change, existing branding
   and Linux packaging checks, script syntax checks, desktop-file-validate, and a
   release build. Run broader repository checks where required by project guidance.
10. Install and launch on the current KDE Wayland desktop. Observe application
    menu/search discovery, icon, window identity/grouping, pinning and relaunch,
    then verify config and shell startup. Exercise the X11 backend when practical
    and distinguish XWayland observations from a native X11 session. Verify GNOME
    directly only if an actual session is available; otherwise report it untested
    and document a GNOME acceptance checklist. Record uninstall behavior and any
    menu/cache refresh limitations.
11. Audit all requirements and write the report: files changed, exact working
    installation/uninstallation commands, observed native results, and remaining
    limitations. Complete the goal only after implementation and required
    verification are finished; no commit or push.

## Expected change scope

- Existing Linux desktop entry and `crates/app/src/branding.rs`.
- A new user-level Linux installer/uninstaller script and focused validation.
- `scripts/package_release.py` and its tests for shipping the script.
- Branding generator checks if affected by desktop entry metadata.
- README, branding README, and Linux installation/verification documentation.

No new artwork, packaging framework, terminal semantics, config format, or shell
selection changes are planned. Final file names and commands will be recorded
after implementation rather than presented as already available.

## Acceptance and limitations

The installed launcher must validate, resolve the binary without user PATH setup,
reuse the hicolor icon, and match the window identity. User installation/removal
must work without root and preserve user data. Native KDE checks must be backed by
observations; format compliance alone is not evidence of successful GNOME pinning.
Desktop launches inherit the graphical session environment, so variables defined
only in interactive shell startup files are not guaranteed. Do not change shell
selection to conceal that difference. Actual writes to the user's installation
directories and native launches may require sandbox approval during execution.

## Delivered files

- Added `scripts/install_linux.py` and `scripts/test_install_linux.py`.
- Added `docs/LINUX_DESKTOP_INTEGRATION.md` and this goal/plan document.
- Changed `crates/app/src/branding.rs`, the existing Linux desktop entry,
  `scripts/package_release.py`, `scripts/test_package_release.py`, `README.md`,
  and `assets/branding/README.md`.

All eleven plan steps are complete within the available KDE Wayland environment.
The report distinguishes native observations from unavailable GNOME/native X11
sessions and records the pre-existing Wayland graphics teardown crash. No commit
or push was performed.
