# Terminal

Terminal is a keyboard-focused, GPU-rendered terminal emulator with tabs, split panes, and reusable workspaces. It is built in Rust for Windows, Linux, and macOS. Portable archives are configured for GitHub Releases; source builds are also supported.

## Features

- Tabs, nested horizontal and vertical splits, a searchable tab picker, directional pane focus and resizing, and per-tab pane zoom.
- Independent PTYs per pane, terminal scrollback with a draggable scrollbar, literal scrollback search, and mouse selection.
- A searchable command palette, configurable physical-key shortcuts, project shortcuts in the palette, and custom tab titles.
- Configurable fonts, runtime font sizing, terminal and ANSI colors, and semantic colors for app UI, search, and scrollbars.
- Shell-reported titles and PowerShell current-directory tracking.
- Saved Workspaces that recreate layouts, working directories, and explicit per-pane shell startup commands.

## Installation

Download a portable archive from [GitHub Releases](https://github.com/xdjanisxd/terminal/releases) for your operating system and CPU. Each archive contains the `terminal` executable, this README, `config.example.toml`, and the MIT license. Extract it to a directory you keep; you can run the executable there or put it on PATH. You do not need to copy `config.example.toml`: Terminal uses built-in defaults when no user config exists.

### Windows

Download `terminal-windows-x86_64.zip` for x64 Windows or `terminal-windows-aarch64.zip` for Windows ARM64. Extract the ZIP, then run `terminal.exe`. You can move the executable to a permanent directory and add that directory to PATH as described below.

### Linux

Download `terminal-linux-x86_64.tar.gz` for x64 Linux or `terminal-linux-aarch64.tar.gz` for ARM64 Linux. Extract and run it, for example:

```sh
tar -xzf terminal-linux-x86_64.tar.gz
./terminal
```

The Linux archive uses the GNU libc target and needs compatible system graphics, window-system, and font libraries. It is not a fully static binary; see [release requirements](docs/RELEASE.md).

### macOS

Download `terminal-macos-x86_64.tar.gz` for Intel or `terminal-macos-aarch64.tar.gz` for Apple Silicon. Extract the archive with `tar -xzf <archive-name>`, then run `./terminal` from Terminal. This command-line application is currently unsigned and not notarized. macOS Gatekeeper may warn or block it; see [Apple's guidance for opening an unverified app](https://support.apple.com/guide/mac-help/open-a-mac-app-from-an-unidentified-developer-mh40616/mac).

### Build from source

Install a current stable Rust toolchain with Cargo. Native graphics, window-system, and font support must be available on the build host.

```sh
git clone https://github.com/xdjanisxd/terminal.git
cd terminal
cargo build --release -p terminal-app
cargo run --release -p terminal-app
```

The executable is `target/release/terminal` (`target/release/terminal.exe` on Windows).

### Add Terminal to PATH

**Windows:** Keep `terminal.exe` in a stable directory such as `%LOCALAPPDATA%\Programs\Terminal`. Open **Edit environment variables for your account**, edit the user **Path**, and add that directory. Open a new terminal session and verify with `terminal --help`.

**Linux:** For a user-local install without `sudo`:

```sh
mkdir -p ~/.local/bin
install -m 755 terminal ~/.local/bin/terminal
```

If `~/.local/bin` is not on PATH, add `export PATH="$HOME/.local/bin:$PATH"` to your shell startup file (`~/.bashrc` for Bash or `~/.zshrc` for Zsh), then reopen the shell or source that file. For a system-wide install instead, run `sudo install -m 755 terminal /usr/local/bin/terminal`.

**macOS:** Install to your user-local directory with `mkdir -p ~/.local/bin` and `install -m 755 terminal ~/.local/bin/terminal`. If needed, add `export PATH="$HOME/.local/bin:$PATH"` to `~/.zshrc`, then reopen the shell or run `source ~/.zshrc`. A common system-wide alternative is `sudo install -m 755 terminal /usr/local/bin/terminal`. Run `terminal --help` from a new shell to check PATH.

## Configuration

The default file is `%APPDATA%\terminal\config.toml` on Windows and `${XDG_CONFIG_HOME:-$HOME/.config}/terminal/config.toml` on Linux and macOS. `TERMINAL_CONFIG` selects a different file. A missing file uses built-in defaults; the app does not create it. To customize Terminal, create a config at that path using [config.example.toml](config.example.toml) as a reference. See the [configuration reference](docs/CONFIG.md) for validation, paths, and reload behavior.

Adding any `[[bindings]]` entries replaces the entire built-in shortcut list. Keep every default binding you still want.

## Default keyboard shortcuts

These are the bindings in `terminal-config` on this branch. Commands without a default shortcut remain available in the command palette or through custom bindings.

| Shortcut | Action | Description |
| --- | --- | --- |
| `Ctrl+Shift+C` | `copy` | Copy the selection. |
| `Ctrl+Shift+V` | `paste` | Paste from the clipboard. |
| `PageUp` | `page_up` | Scroll one page up. |
| `PageDown` | `page_down` | Scroll one page down. |
| `Ctrl+=` | `increase_font_size` | Increase font size. |
| `Ctrl+Shift+=` | `increase_font_size` | Increase font size. |
| `Ctrl+-` | `decrease_font_size` | Decrease font size. |
| `Ctrl+0` | `reset_font_size` | Reset to the configured size. |
| `Ctrl+Shift+P` | `open_palette` | Open the command palette. |
| `Ctrl+Shift+Space` | `open_tab_picker` | Open the searchable tab picker. |
| `Ctrl+Shift+T` | `new_tab` | Open a new tab. |
| `Ctrl+Shift+E` | `split_vertical` | Split into left and right panes. |
| `Ctrl+Shift+O` | `split_horizontal` | Split into top and bottom panes. |
| `Ctrl+Tab` | `next_tab` | Focus the next tab. |
| `Ctrl+Shift+Tab` | `previous_tab` | Focus the previous tab. |
| `Ctrl+Shift+ArrowRight` | `next_pane` | Focus the next pane in traversal order. |
| `Ctrl+Shift+ArrowLeft` | `previous_pane` | Focus the previous pane in traversal order. |
| `Ctrl+Shift+H` | `focus_pane_left` | Focus the pane to the left. |
| `Ctrl+Shift+L` | `focus_pane_right` | Focus the pane to the right. |
| `Ctrl+Shift+K` | `focus_pane_up` | Focus the pane above. |
| `Ctrl+Shift+J` | `focus_pane_down` | Focus the pane below. |
| `Ctrl+Alt+H` | `resize_pane_left` | Move the nearest split boundary left. |
| `Ctrl+Alt+L` | `resize_pane_right` | Move the nearest split boundary right. |
| `Ctrl+Alt+K` | `resize_pane_up` | Move the nearest split boundary up. |
| `Ctrl+Alt+J` | `resize_pane_down` | Move the nearest split boundary down. |
| `Ctrl+Shift+Z` | `toggle_pane_zoom` | Zoom or restore the focused pane. |
| `Ctrl+Shift+W` | `close_pane` | Close the focused pane. |

`Ctrl+F` opens scrollback search through the app's input handling; it is not a configurable default binding.

## Saved Workspaces

1. Arrange tabs and splits, then change each pane to the desired working directory.
2. Optionally focus a local-shell pane and choose **Set Pane Startup Command** from the command palette. Enter a command such as `nvim` or `npm run dev`. Setting it does not run it now; **Clear Pane Startup Command** removes it.
3. Choose **Save Current Workspace**, name it, and later use **Open Workspace** to reopen it.

Saved Workspaces are reusable templates stored in the app's `workspaces.toml`, separate from `config.toml`. Reopening creates fresh PTYs and processes. It restores the layout, known pane directories, and explicitly configured startup commands, but not terminal contents, scrollback, live processes, shell history, editor buffers, or SSH state. A startup command runs inside a new local shell, so the shell remains available after the command exits. The app never infers commands from running processes.

## Shell integration

On Windows, local PowerShell sessions keep the user's normal profile and add a session-local prompt hook that reports the current filesystem directory with OSC 7. Saved Workspaces use that report to capture pane directories. Shell titles can also appear in the app UI. Directory reporting for Bash, Zsh, and Fish is not implemented here.

## Themes and customization

There are no required built-in theme presets. Configure terminal foreground, background, cursor and selection colors, the 16-color ANSI palette, optional semantic app UI colors, and specific search or scrollbar overrides under `[theme]`. Choose an installed monospace family and base font size under `[font]`; the font shortcuts adjust size for the current run. [config.example.toml](config.example.toml) lists every field and its fallback.

## Project status and development

Portable release packaging is configured for Windows, Linux, and macOS. Installer formats and signing are future work. See [release engineering](docs/RELEASE.md), [current state](docs/CURRENT_STATE.md), and the [roadmap](docs/ROADMAP.md) for details. Repository validation uses:

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo test --workspace --doc
cargo clippy --workspace --all-targets -- -D warnings
```

## License

MIT; see [LICENSE](LICENSE).
