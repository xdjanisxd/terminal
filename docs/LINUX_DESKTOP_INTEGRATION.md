# Linux desktop installation

Terminal uses the application/icon ID `io.github.xdjanisxd.terminal` on Wayland
and X11. Its desktop entry and hicolor icons support freedesktop application
menus, search, and launcher matching in KDE Plasma, GNOME, and other desktops.

## Install and update

Python 3.11 or later is required for the installer; it uses only the standard
library. No root privileges or shell PATH changes are required.

From an extracted Linux release archive:

```sh
python3 install_linux.py install
```

From the source checkout, build with the repository's supported Rust toolchain:

```sh
cargo build --release -p terminal-app
python3 scripts/install_linux.py install --binary target/release/terminal
```

Run the same command with a newer binary/archive to update. Close and reopen
Terminal to use the new binary. An already running instance is preserved.

The installer writes:

- `$HOME/.local/bin/terminal` (a copy, independent of the checkout/archive).
- `${XDG_DATA_HOME:-$HOME/.local/share}/applications/io.github.xdjanisxd.terminal.desktop`.
- `${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/<size>x<size>/apps/io.github.xdjanisxd.terminal.png`.
- An ownership record at `${XDG_DATA_HOME:-$HOME/.local/share}/terminal/desktop-install.json`.

An empty or relative XDG_DATA_HOME falls back to `$HOME/.local/share`, following
the [XDG specification](https://specifications.freedesktop.org/basedir/latest/).
Use the same HOME/XDG_DATA_HOME when updating or uninstalling. An alternate data
directory must also be visible to the desktop session's XDG search paths.

The installed launcher uses an absolute executable path and starts in HOME.
Paths containing spaces, quotes, backslashes, and dollar signs are
escaped according to the
[desktop entry specification](https://specifications.freedesktop.org/desktop-entry/latest/exec-variables.html).
HOME containing `=`, `%`, newline, or carriage return is unsupported (`=` is
forbidden by the specification; GLib cannot resolve an escaped percent sign in
the executable name). The installer
refuses to overwrite unrelated or modified destination files, including symlinks.
Resolve a reported conflict by backing up/moving that file before retrying.

## Uninstall

From a checkout or extracted release respectively:

```sh
python3 scripts/install_linux.py uninstall
python3 install_linux.py uninstall
```

Use only the command appropriate to your location. Uninstall removes recorded
Terminal files and leaves configuration, Saved Workspaces, unrelated icons, and
shared directories intact. Modified installed files cause removal to stop before
deleting anything. Keep an installer script until removal is complete.

## Verify on your desktop

1. Search for **Terminal** in the application menu or desktop overview/KRunner.
   Look for the project icon; other terminal emulators may share the display name.
2. Launch from that result. Confirm a working shell, your existing configuration,
   tabs/splits, and any configured workspace startup commands.
3. In KDE, pin the running application using the task manager's **Pin to Task
   Manager** action. In GNOME, add it to Favorites from the application overview.
   Close all Terminal windows and relaunch from the pinned icon. Check that the
   window groups with that launcher and keeps the project icon.
4. After uninstall, confirm that the menu/search entry disappears. Remove any
   manually pinned favorite if the desktop retains it.

For command-line diagnostics (substitute an absolute XDG_DATA_HOME if set):

```sh
desktop-file-validate "$HOME/.local/share/applications/io.github.xdjanisxd.terminal.desktop"
cd /tmp
env PATH=/usr/bin:/bin gio launch "$HOME/.local/share/applications/io.github.xdjanisxd.terminal.desktop"
```

The installer refreshes desktop/icon caches when the relevant tools exist.
Desktops may refresh asynchronously; log out and back in if an entry stays stale.
Installation does not change the preferred system terminal emulator.

The graphical session must provide its ordinary HOME/display/session environment
and installed graphics libraries. Variables set only in interactive shell startup
files are not inherited by menu launches. Configuration lookup, explicit shell
configuration, SHELL (otherwise `/bin/sh`), and startup commands retain their
existing behavior. Use an absolute configured shell path if the shell is installed
outside the graphical session PATH. No login-shell wrapper is introduced.

## Verification record

Verified on 2026-10-09, KDE Plasma/KWin 6.3.6 under Wayland:

- Source and installed launchers pass `desktop-file-validate`. GLib resolves the
  installed entry and icon. A real `gio launch` test starts from an unrelated
  directory with `PATH=/usr/bin:/bin`, including an installation path containing
  spaces, quotes, dollar signs, backticks, and backslashes; its shell stub reports
  the expected HOME working directory.
- KDE's native application-menu and `krunner_services` search models list Terminal
  with its desktop file and project icon. Triggering the menu entry launches the
  installed build with a working shell and the existing configuration's appearance.
- KWin reports Wayland resourceClass and desktopFileName as
  `io.github.xdjanisxd.terminal`. Temporary pinning shows the project icon in the
  Plasma task manager; the task manager's launcher model relaunches the build
  after all verification windows close. The original panel configuration was
  restored. These checks exercised KDE models and window metadata; a manual
  end-to-end mouse interaction/grouping check is still useful.
- Forcing winit's X11 backend through XWayland produces both WM_CLASS strings as
  `io.github.xdjanisxd.terminal`, and KWin resolves the same desktopFileName.
  This is XWayland verification, not a separate native X11 desktop session.
- Real user uninstall removes the binary, entry, and icons; the native KDE search
  result disappears. Existing user-data checksums remain unchanged. Reinstallation
  restores the application. The prior unowned binary was preserved at
  `~/.local/bin/terminal.before-desktop-integration` before installing.
- Installer ownership/lifecycle/escaping tests, release packaging tests, a real
  archive extraction/binary smoke/install/uninstall check, branding regeneration
  check, workspace Rust tests/doctests, formatting, and Clippy passed. Validation
  used an isolated Rust 1.99.0 toolchain because this machine's Rust 1.85.1 cannot
  build the existing wayland-protocols dependency.

Remaining limitations: no GNOME session or standalone X11 desktop is installed,
so GNOME Favorites and those sessions were not directly tested. GLib launcher
validation provides a compatibility check, not evidence of GNOME dock behavior.
Native Wayland test windows crash during graphics teardown when closed on this
machine (Mesa/wgpu EGL stack, exit 139). The preserved pre-integration binary
reproduces the same crash; renderer shutdown was deliberately left outside this
integration change. Shell/config selection and startup implementations were not
changed; every user-specific workspace startup command was not manually exercised.
