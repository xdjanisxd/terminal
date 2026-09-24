# Configuration

`terminal-config` owns the typed defaults, TOML parsing, and validation. The app loads the file at startup, retains the last valid configuration, resolves configured keys to typed commands, and passes colors to the renderer. Terminal state and command execution remain outside the config crate.

The config file is `%APPDATA%\terminal\config.toml` on Windows and `${XDG_CONFIG_HOME:-$HOME/.config}/terminal/config.toml` elsewhere. `TERMINAL_CONFIG` overrides the full path. A missing file uses defaults; the app does not create one.

```toml
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

All theme fields are optional and independently fall back to defaults. If `bindings` is present, it replaces the entire default list; `bindings = []` disables all shortcuts. The defaults are Ctrl+Shift+C/V for copy/paste, PageUp/PageDown for local scrollback, Ctrl+Shift+P for the command palette, Ctrl+Shift+T for a new tab, Ctrl+Shift+E/O for vertical/horizontal splits, Ctrl+Tab and Ctrl+Shift+Tab for tab switching, Ctrl+Shift+ArrowRight/ArrowLeft for pane switching, and Ctrl+Shift+W for closing a pane. Chords use physical keys: a letter A–Z, digit 0–9, PageUp, PageDown, Home, End, Insert, Delete, Tab, ArrowLeft, or ArrowRight, optionally preceded by Ctrl, Shift, and/or Alt. Modifiers must match exactly. Plain letters and digits need a modifier so ordinary text still reaches the PTY. Commands are `copy`, `paste`, `page_up`, `page_down`, `open_palette`, `new_tab`, `split_horizontal`, `split_vertical`, `next_tab`, `previous_tab`, `next_pane`, `previous_pane`, and `close_pane`. The older `action` field remains accepted as an alias for `command`. The app executes commands in one dispatcher.

The palette lists copy, paste, scrollback, and workspace commands from the app command registry. Type to filter names (case insensitive), use Up/Down to select, Enter to run, or Escape to close. Palette input is consumed locally and does not reach the PTY. `open_palette` is omitted from its own list. The palette is drawn over the top terminal rows without changing terminal contents. If custom bindings replace the defaults, include an `open_palette` binding to retain the shortcut.

Unknown fields or commands, malformed TOML, invalid colors, incomplete ANSI palettes, unsupported chords, and duplicate chords reject the whole file. Errors include the file path and either TOML's source location or the field/binding index. On a reload error, the app writes the error to stderr and keeps the last valid config.

The app checks the file contents every 500 ms and posts a change event to its main loop. Theme and keybindings apply to the next input/redraw without restarting the PTY. Removing the file restores defaults. A short lived invalid file during editing can produce an error; a later valid save is applied. An open palette retains its query; changed bindings affect the next keypress outside the palette.

## Workspace definitions

The default workspace is one tab with one local shell pane. An optional `[workspace]` table declares ordered tabs and a recursive pane layout. Each `pane` leaf starts a fresh local shell session; `split` nodes record `horizontal` or `vertical` structure. `active_tab` and each tab's `active_pane` are zero-based indexes. Pane indexes follow leaves in first-then-second traversal order. The app rejects empty tab lists and out-of-range focus indexes.

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

Definitions reproduce the tab, split, focus, and session structure on startup. They do not restore live process state or terminal contents. The running workspace keeps its live state when the config file changes; restart the app to apply changes to `[workspace]`. The active pane uses the full terminal window and the title shows its tab and pane position. Split geometry and simultaneous pane rendering remain future UI work.

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
