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

The installed launcher uses an absolute executable path and preserves the
launcher's process working directory. This lets Dolphin's **Open Terminal Here**
directory reach the initial shell; when no explicit project, workspace, or pane
root is configured, PTY children inherit that directory. Configured roots retain
their existing precedence. The PTY adapter passes the process directory
explicitly when no root is configured because its backend otherwise defaults to
HOME. Paths containing spaces, quotes, backslashes, and dollar signs are escaped
according to the
[desktop entry specification](https://specifications.freedesktop.org/desktop-entry/latest/exec-variables.html).
HOME containing `=`, `%`, newline, or carriage return is unsupported (`=` is
forbidden by the specification; GLib cannot resolve an escaped percent sign in
the executable name). The installer
refuses to overwrite unrelated or modified destination files, including symlinks.
Resolve a reported conflict by backing up/moving that file before retrying.

The canonical desktop entry is `io.github.xdjanisxd.terminal.desktop`.
If `terminal.desktop` exists in the user application directory, the installer
warns and preserves it, including dangling symlinks. It has no ownership record
for that legacy name; matching names or executable paths do not prove ownership.
Review duplicate launchers manually and select the canonical entry in KDE.

## KDE default terminal and command launches

KDE's `Terminal=true` applications are launched through KIO's
`DesktopExecParser`. It reads `[General] TerminalApplication` from `kdeglobals`,
expands the application's desktop `Exec` field and file placeholders, then
prepends the configured terminal and `-e`. Vim's `Exec=vim %F` therefore produces
`terminal -e /usr/bin/vim /path/to/file.txt` on the verified system. Konsole gets
additional special handling; arbitrary terminal emulators receive `-e` without
Konsole-specific title/icon arguments. KDE may explicitly supply `/bin/sh -c`
for desktop commands containing shell syntax; Terminal executes that supplied
program directly. See [KIO's implementation](https://invent.kde.org/frameworks/kio/-/blob/master/src/core/desktopexecparser.cpp).

Previously Terminal rejected `-e` during CLI parsing and exited before creating
a window. It now accepts `-e`, `--execute`, and `--` followed by an executable
and separate arguments. See [command execution](../README.md#command-execution)
for options, working directory, quoting, and workspace behavior. No extra `-e`
belongs in Terminal's own desktop entry: its `Exec` remains a normal Terminal
launch, with `Terminal=false` to avoid recursive terminal launching.

Verified on 2026-10-13 with KDE Wayland, kioclient 6.3.4 and KIO 6.13.0:

- An isolated KDE `text/plain=vim.desktop` association opened a `.txt` file with
  spaces, Turkish characters, and Japanese characters. Captured `/proc` argv
  confirmed `terminal -e /usr/bin/vim ... <exact path>`. Vim recorded the exact
  opened filename, then its exit closed Terminal.
- Native integration tests exercised all three delimiters, exact arguments
  (including empty arguments and literal shell metacharacters), PTY attachment,
  working directory, zero/nonzero exit, child reaping, and unchanged saved data.
  Vim and Neovim opened Unicode/space paths in real Wayland windows and closed
  them after editor exit. A missing executable closed without shell fallback.
- `terminal -e btop` rendered its interactive process monitor in a real Wayland
  window. SIGINT to btop exited it and closed Terminal; the child was reaped.
  Normal Zsh startup rendered an interactive prompt and closed cleanly. The
  existing native Bash/Zsh lifecycle suite passed for tabs, splits, startup
  commands, and unsuccessful exits.

The KDE association test used temporary settings and a Vim desktop entry with
only test commands added to record the opened path and quit automatically; live
user associations and desktop entries were preserved. These are automated native
checks, not a physical mouse double-click test. Native execution was tested on
Linux; equivalent Windows/macOS command launches were not tested. The user's
installed binary is separate from the build: run the installer after building
to update it for everyday KDE launches.

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

### Dolphin acceptance

1. Install the updated build and select Terminal as KDE's default terminal
   application.
2. In Dolphin, navigate to a local directory whose path contains spaces and
   non-ASCII characters, then choose **Tools → Open Terminal** (Shift+F4) or
   **Open Terminal Here** from the context menu.
3. Run `pwd` and confirm it prints the directory shown in Dolphin.
4. Repeat with an explicitly configured project, workspace, or pane root and
   confirm that configured root still takes precedence.
5. Launch Terminal normally from a shell in another directory and confirm the
   initial shell inherits that directory. Then check a saved workspace and a
   direct `terminal -e pwd` launch to confirm their existing launch rules remain
   intact.

Focused launch-directory regression verification on 2026-10-21:

- Source and installed launchers pass `desktop-file-validate`. GLib resolves the
  installed entry and icon. A real `gio launch` test starts from an unrelated
  directory with `PATH=/usr/bin:/bin`, including an installation path containing
  spaces, quotes, dollar signs, backticks, and backslashes and a launch directory
  containing spaces and Unicode; its shell stub reports the launcher's inherited
  process working directory.
- PTY contract coverage reads the actual child CWD with no configured root and
  with an explicit Unicode/space root. App shell integration checks Bash and Zsh
  with no root, and the ignored native application test covers direct and
  installed executable launches.
- The native application test was compiled but not run here: Dolphin and KWin are
  installed, but neither is running in the current session.

Earlier KDE integration verification on 2026-10-09, KDE Plasma/KWin 6.3.6 under
Wayland:
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
