# Terminal

Terminal is a keyboard-focused, GPU-rendered terminal emulator with tabs, split panes, and reusable workspaces. It is built in Rust for Windows, Linux, and macOS. The app is under active development; build it from source today.

## Features

- Tabs, nested horizontal and vertical splits, a searchable tab picker, directional pane focus and resizing, and per-tab pane zoom.
- Independent PTYs per pane, terminal scrollback with a draggable scrollbar, literal scrollback search, and mouse selection.
- A searchable command palette, configurable physical-key shortcuts, project shortcuts in the palette, and custom tab titles.
- Configurable fonts, runtime font sizing, terminal and ANSI colors, and semantic colors for app UI, search, and scrollbars.
- Shell-reported titles and PowerShell current-directory tracking.
- Saved Workspaces that recreate layouts, working directories, and explicit per-pane shell startup commands.

## Build from source

Install a current stable Rust toolchain with Cargo. Native graphics, window-system, and font support must be available on the build host. The repository runs CI on Windows, Linux, and macOS; packaged installers are still planned.

```sh
git clone https://github.com/xdjanisxd/terminal.git
cd terminal
cargo build --release -p terminal-app
cargo run --release -p terminal-app
```

The executable is `target/release/terminal-app` (`target/release/terminal-app.exe` on Windows).

## Configuration

The default file is `%APPDATA%\terminal\config.toml` on Windows and `${XDG_CONFIG_HOME:-$HOME/.config}/terminal/config.toml` on Linux and macOS. `TERMINAL_CONFIG` selects a different file. A missing file uses built-in defaults; the app does not create it. Copy [config.example.toml](config.example.toml) to get a complete, annotated starting point, and see the [configuration reference](docs/CONFIG.md) for validation, paths, and reload behavior.

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

The application and cross-platform CI exist, but distribution packaging and release artifacts are future work. See [current state](docs/CURRENT_STATE.md) and the [roadmap](docs/ROADMAP.md) for details. Repository validation uses:

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo test --workspace --doc
cargo clippy --workspace --all-targets -- -D warnings
```

## License

MIT; see [LICENSE](LICENSE).
