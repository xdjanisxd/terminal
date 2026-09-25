# Configuration

`terminal-config` owns the typed defaults, TOML parsing, and validation. The app loads the file at startup, retains the last valid configuration, resolves configured keys to typed commands, and passes font settings and colors to the renderer. Terminal state and command execution remain outside the config crate.

The config file is `%APPDATA%\terminal\config.toml` on Windows and `${XDG_CONFIG_HOME:-$HOME/.config}/terminal/config.toml` elsewhere. `TERMINAL_CONFIG` overrides the full path. A missing file uses defaults; the app does not create one.

The complete, ready-to-copy example is [`config.example.toml`](../config.example.toml) at the repository root. Copy it to the config file location above, then edit the values you want. Omitted font and theme fields use their individual defaults; omitting `[workspace]`, `bindings`, or `projects` uses the default single-pane workspace, default shortcuts, or no projects, respectively. The example lists the actual defaults and valid values in comments.

```toml
[font]
# Omit family to use the platform-selected monospace font.
# family = "Cascadia Mono"
size = 16

[theme]
foreground = "#e6e6e6"
background = "#000000"
cursor = "#e6e6e6"
selection_foreground = "#ffffff"
selection_background = "#2e59a6"
# Optional: exactly 16 #RRGGBB strings for ANSI colors 0 through 15.
# ansi = ["#000000", ...]

[[bindings]]
key = "Ctrl+Shift+C"
command = "copy"

[[bindings]]
key = "Ctrl+Shift+V"
command = "paste"

[[bindings]]
key = "PageUp"
command = "page_up"

[[bindings]]
key = "PageDown"
command = "page_down"

[[bindings]]
key = "Ctrl+Shift+P"
command = "open_palette"
```

All theme fields are optional and independently fall back to defaults. If `bindings` is present, it replaces the entire default list; `bindings = []` disables all shortcuts. Default commands cover copy/paste, scrollback, font size, tabs, panes, pane zoom, and directional pane resize. Alt+Shift+ArrowLeft/Right/Up/Down resize panes; Ctrl+Shift+ArrowLeft/Right switch panes. Chords use physical keys: a letter A–Z, digit 0–9, Equal, Minus, PageUp, PageDown, Home, End, Insert, Delete, Backspace, Tab, or ArrowLeft/Right/Up/Down, optionally preceded by Ctrl, Shift, and/or Alt. Modifiers must match exactly. Plain letters and digits need a modifier so ordinary text still reaches the PTY. For example, `key = "Ctrl+Shift+Backspace"` with `command = "close_pane"` binds that physical chord; unbound Backspace still goes to the terminal. Command names match the snake_case variants in `config.example.toml`. The older `action` field remains accepted as an alias for `command`. The app executes commands in one dispatcher.

`toggle_pane_zoom` expands the focused pane to the workspace viewport for its tab. The split layout and all pane sessions stay in place; background panes continue receiving PTY output. Toggling again restores the exact split geometry. Zoom belongs to each running tab and is not saved in workspace definitions. Switching tabs preserves each tab's zoom state. Focusing another pane while zoomed shows that pane full size. Splitting while zoomed keeps zoom active on the new pane; closing a pane while zoomed keeps zoom active on the newly focused pane. Closing a tab discards its zoom state.

The palette lists copy, paste, scrollback, and workspace commands from the app command registry, including all four pane resize commands. **Rename Tab** opens a local title prompt; type a title and press Enter to save, Escape to cancel, or leave the title empty to restore the tab's configured/generated title. Each `[[projects]]` entry also adds **New Tab: NAME**, **Split Horizontal: NAME**, and **Split Vertical: NAME** actions. Type to filter names (case insensitive), use Up/Down to select, Enter to run, or Escape to close. Palette and rename-prompt input are consumed locally and do not reach the PTY. Custom titles stay with their tab while the app is running and are not written into the startup workspace definition. `open_palette` is omitted from its own list. The palette is drawn over the top terminal rows without changing terminal contents. If custom bindings replace the defaults, include an `open_palette` binding to retain the shortcut.

`resize_pane_left`, `resize_pane_right`, `resize_pane_up`, and `resize_pane_down` move the nearest split boundary controlling the focused pane in the requested direction. This lets either side of a split grow or shrink without changing focus, and opposite commands reverse each other while geometry permits. Defaults are Alt+Shift+ArrowLeft/Right/Up/Down; these do not overlap the existing Ctrl+Shift pane switching shortcuts. Each horizontal invocation moves the boundary by at most two cell widths, and each vertical invocation by at most one cell height. The last step may be smaller to retain at least 10 columns or 3 rows per pane, including panes inside nested splits. When either side has reached its minimum or while the tab is zoomed, the command does nothing. Resize ratios belong to the running tab; the startup layout definition still records split structure only. Pane sessions stay in place, and changed geometry updates terminal grids and PTY sizes.

Unknown fields or commands, malformed TOML, invalid colors, incomplete ANSI palettes, unsupported chords, and duplicate chords reject the whole file. Errors include the file path and either TOML's source location or the field/binding index. On a reload error, the app writes the error to stderr and keeps the last valid config.

The app checks the file contents every 500 ms and posts a change event to its main loop. Theme, keybindings, and project palette entries apply after a valid reload without restarting the PTY. Removing the file restores defaults for those live settings. A short lived invalid file during editing can produce an error; a later valid save is applied. An open palette retains its query; changed bindings affect the next keypress outside the palette. Editing font settings and workspace definitions requires an app restart. Runtime font shortcuts resize the renderer and terminal grids without writing `config.toml`; reset returns to the configured `[font].size` captured at startup.

## Font settings

`font.family` selects an installed monospace family by name. If omitted, the renderer uses its platform monospace selection. A missing or proportional family fails renderer initialization. `font.size` is an integer in logical pixels from 1 to 256; it defaults to 16. The renderer scales that size with window DPI when calculating cell metrics and grid dimensions. Fallback faces are selected automatically and cannot be configured. Font family and size are applied when the renderer starts; restart the app after editing `[font]`.

## Workspace definitions

The default workspace is one tab with one local shell pane. An optional `[workspace]` table declares ordered tabs and a recursive pane layout. Each `pane` leaf starts an independent PTY session; `split` nodes record `horizontal` or `vertical` structure. `active_tab` and each tab's `active_pane` are zero-based indexes. Pane indexes follow leaves in first-then-second traversal order. The app rejects empty tab lists and out-of-range focus indexes.

```toml
[workspace]
active_tab = 0

[[workspace.tabs]]
title = "Development"
active_pane = 1
layout = { kind = "split", axis = "vertical", first = { kind = "pane", session = "local_shell" }, second = { kind = "pane", session = "local_shell" } }

[[workspace.tabs]]
title = "Notes"
layout = { kind = "pane", session = "local_shell" }
```

An optional `project_root` can be set on `[workspace]`, `[[workspace.tabs]]`, or a `pane` leaf. The pane root takes precedence over the tab root, then the workspace root. Relative roots resolve from the configuration file's directory. A root sets that pane's PTY working directory. The default workspace uses `local_shell`; each explicit pane requires a `session`. A command session starts the specified program directly with its argument array, with no shell interpolation:

```toml
[workspace]
project_root = "../repo"

[[workspace.tabs]]
title = "Server"
[workspace.tabs.layout]
kind = "pane"
[workspace.tabs.layout.session.command]
program = "cargo"
args = ["run", "--bin", "server"]

[[projects]]
name = "Repository"
path = "../repo"
```

`[[projects]]` names must be unique and paths must be nonempty. Their paths also resolve from the configuration file's directory. Palette project actions start a fresh local shell in the selected project root; a new tab owns its root, while a split records its root on the new pane. Ordinary new tab and split commands inherit the applicable workspace or tab root.

Definitions reproduce the tab, split, focus, root, and startup command structure on startup. They do not restore live process state or terminal contents. The running workspace keeps its live state when the config file changes; restart the app to apply changes to `[workspace]`. Project palette entries update after a valid config reload. The active tab's split panes render simultaneously, and the title shows its focused tab and pane position.

## CLI

`terminal-app --project-root DIRECTORY` opens the default workspace in that directory. `terminal-app --workspace FILE.toml` loads a config-shaped TOML file that contains `[workspace]`; it can also include `[[projects]]`, font, theme, and bindings. The two options can be combined. `--project-root` replaces the workspace-level root from the file, while explicit tab and pane roots retain their precedence. `--help` prints usage. Invalid paths and arguments fail before the window opens. There is no external control of an already-running app.

## Manual smoke

1. Set `TERMINAL_CONFIG` to a temporary TOML file and start `terminal-app`. Try the default copy/paste, PageUp/PageDown, and Ctrl+Shift+P shortcuts.
2. Change `[theme] background` and `foreground` to visibly different colors and save. Check that blank cells and text repaint without restarting the shell.
3. Replace the bindings list with `key = "Alt+PageUp"` and `command = "page_down"` in a `[[bindings]]` entry. Check that Alt+PageUp runs the new command and the old PageUp binding no longer runs.
4. Save an invalid color, verify the error on stderr and that the prior theme and bindings remain active. Correct the file and verify the new values apply.
5. Remove the file and verify defaults return.
6. Open the palette with Ctrl+Shift+P, type `page down`, choose it with Enter, and confirm scrollback moves without sending the query to the shell. Reopen it, try Up/Down selection and Escape, and confirm the terminal content remains intact.
7. Start with no `[workspace]`. Create a tab with Ctrl+Shift+T and a split with Ctrl+Shift+E. Run a different command in each pane; switch with Ctrl+Shift+ArrowLeft/ArrowRight and Ctrl+Tab/Ctrl+Shift+Tab. Check that each pane retains its own prompt and output, and that the window title identifies the focused tab and pane.
8. Close a pane with Ctrl+Shift+W and verify its session ends while the remaining pane stays usable. Closing the sole pane of the sole tab leaves that terminal running.
9. Add the two-tab workspace definition above and restart. Check that its tab and pane counts and initial focus match the file. Edit only `[workspace]` while running and verify the current sessions remain intact; restart to apply the new definition.
10. On Windows, create a temporary workspace file with a `[workspace] project_root` pointing to an existing directory, a pane command using `program = "cmd.exe"` and `args = ["/K", "cd"]`, and a `[[projects]]` entry pointing to a second directory. Run `cargo run -q -p terminal-app -- --workspace PATH_TO_FILE`. The command should print the first directory and leave an interactive prompt.
11. Open Ctrl+Shift+P and use **New Tab: NAME**, **Split Horizontal: NAME**, and **Split Vertical: NAME**. Run `cd` in each created shell and check that it prints the second directory. Check that all panes remain responsive, output continues in an unfocused pane, focus and cursor follow pane switching, and Close Pane leaves the others running. Run `cargo run -q -p terminal-app -- --project-root PATH_TO_DIRECTORY` separately and check the initial shell directory with `cd`.
