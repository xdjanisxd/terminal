# Configuration

`terminal-config` owns the typed defaults, TOML parsing, and validation. The app loads the file at startup, retains the last valid configuration, matches configured keys to app actions, and passes colors to the renderer. Terminal state and command execution remain outside the config crate.

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
action = "copy"

[[bindings]]
key = "Ctrl+Shift+V"
action = "paste"

[[bindings]]
key = "PageUp"
action = "page_up"

[[bindings]]
key = "PageDown"
action = "page_down"
```

All theme fields are optional and independently fall back to defaults. If `bindings` is present, it replaces the entire default list; `bindings = []` disables all four shortcuts. The defaults are Ctrl+Shift+C/V for copy/paste and PageUp/PageDown for local scrollback. Chords use physical keys: a letter A–Z, digit 0–9, PageUp, PageDown, Home, End, Insert, or Delete, optionally preceded by Ctrl, Shift, and/or Alt. Modifiers must match exactly. Plain letters and digits need a modifier so ordinary text still reaches the PTY. Actions are `copy`, `paste`, `page_up`, and `page_down`. The app executes them in one place; this model can feed the later centralized-command work.

Unknown fields or actions, malformed TOML, invalid colors, incomplete ANSI palettes, unsupported chords, and duplicate chords reject the whole file. Errors include the file path and either TOML's source location or the field/binding index. On a reload error, the app writes the error to stderr and keeps the last valid config.

The app checks the file contents every 500 ms and posts a change event to its main loop. Theme and keybindings apply to the next input/redraw without restarting the PTY. Removing the file restores defaults. A short lived invalid file during editing can produce an error; a later valid save is applied. Other runtime settings and commands are outside this slice.

## Manual smoke

1. Set `TERMINAL_CONFIG` to a temporary TOML file and start `terminal-app`. Try the default copy/paste and PageUp/PageDown shortcuts.
2. Change `[theme] background` and `foreground` to visibly different colors and save. Check that blank cells and text repaint without restarting the shell.
3. Replace the bindings list with `Alt+PageUp = page_down` in the TOML syntax above. Check that Alt+PageUp runs the new action and the old PageUp binding no longer runs.
4. Save an invalid color, verify the error on stderr and that the prior theme and bindings remain active. Correct the file and verify the new values apply.
5. Remove the file and verify defaults return.
