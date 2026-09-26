# Configuration

The app reads `config.toml` at startup. It does not create the file; when it is
missing, built-in defaults provide one local-shell tab, default bindings, a
default font and theme, and no project palette entries. The complete annotated
[example](../config.example.toml) reproduces those defaults.

## Location and loading

| Platform | Default path |
| --- | --- |
| Windows | `%APPDATA%\terminal\config.toml` |
| Linux and macOS | `$XDG_CONFIG_HOME/terminal/config.toml` when set; otherwise `$HOME/.config/terminal/config.toml` |

`TERMINAL_CONFIG` overrides the full file path. Relative project paths in
the file resolve against the directory containing that file. The same rules
apply to a file loaded with `--workspace`. See [CLI options](#cli-options).
If neither platform base-directory environment variable is available, the
app uses `./terminal` as its storage directory.

TOML is parsed into a typed schema. Unknown fields, malformed TOML, invalid
colors, unsupported or duplicate key chords, unknown commands, invalid font
values, and invalid workspace definitions reject the whole file. Errors name
the file and relevant source location or field. On a reload error the app
reports the error to stderr and retains the last valid configuration. A
missing file restores defaults for live-reloaded settings.

The app reloads valid changes to theme, bindings, and project palette entries
while running. Font settings and startup workspace definitions are applied
when the app starts; restart after editing those. Runtime font shortcuts change
the current size without editing the file, and reset returns to the configured
startup size. Existing running panes are not rebuilt when the file changes.

## Font

`[font]` has two optional fields:

| Field | Value | Default |
| --- | --- | --- |
| `family` | Name of an installed monospace font | Platform-selected monospace font |
| `size` | Integer logical pixels from 1 through 256 | `16` |

The renderer scales the size for display DPI. A missing or proportional family
can prevent renderer initialization. The font shortcuts adjust size during the
current run; editing `[font]` requires a restart.

## Theme

All `[theme]` fields are optional. Colors are six-digit `#RRGGBB` strings.
The terminal color fields are `foreground`, `background`, `cursor`,
`selection_foreground`, `selection_background`, and `ansi`. The `ansi`
array contains exactly 16 colors for indexes 0–15. Omitted values use the
built-in defaults shown in the [example](../config.example.toml).

The optional semantic app UI fields are `ui_background`, `ui_foreground`,
`ui_selected_background`, `ui_muted`, `ui_accent`, and `ui_border`. They
control app-owned surfaces and text, selected rows, muted text, accents and
focus borders, and borders. Their fallback and override relationships are:

| Field | Fallback or related use |
| --- | --- |
| `ui_background` | Defaults to terminal `background`. |
| `ui_foreground` | Defaults to terminal `foreground`; when set, also supplies selected UI text and the scrollbar thumb hover color. |
| `ui_selected_background` | Defaults to `selection_background`; also supplies active search color. |
| `ui_muted` | Supplies muted UI text and the scrollbar thumb. |
| `ui_accent` | Defaults to `selection_foreground`; supplies focus and search accents. |
| `ui_border` | Supplies borders, scrollbar track, and scrollbar track hover color. |

Any remaining UI values keep renderer defaults. The more specific optional
`search_match_background`, `search_active_background`,
`scrollbar_thumb`, and `scrollbar_thumb_hover` fields override their
related semantic UI colors. The example includes every theme field.

## Key bindings and commands

Omitting `bindings` retains all built-in shortcuts in the
[README table](../README.md#default-keyboard-shortcuts). **Any
`[[bindings]]` entries replace the entire built-in list.** Include every
default shortcut you still want; `bindings = []` disables them all. Each
entry has a `key` and a `command`. The older `action` field is an alias
for `command`.

A key chord puts optional `Ctrl` (or `Control`), `Shift`, and `Alt` in
any order before the physical key, separated by `+`. Modifier and key names
are case-insensitive. Supported keys are A–Z, 0–9, `=`/`Equal`,
`-`/`Minus`, `PageUp`, `PageDown`, `Home`, `End`, `Insert`,
`Delete`, `Backspace`, `Tab`, `Space`, `ArrowLeft`,
`ArrowRight`, `ArrowUp`, and `ArrowDown`. Plain letters and digits need a
modifier so typing them still reaches the PTY. Exact modifier sets matter;
duplicate chords are rejected. `Ctrl+F` opens scrollback search through app
input handling rather than a configurable default binding.

The complete command names accepted in a binding are:

| Area | Commands |
| --- | --- |
| Clipboard and scrollback | `copy`, `paste`, `page_up`, `page_down` |
| Font | `increase_font_size`, `decrease_font_size`, `reset_font_size` |
| Pickers and workspaces | `open_palette`, `open_tab_picker`, `save_current_workspace`, `set_pane_startup_command`, `clear_pane_startup_command`, `open_workspace_picker`, `delete_workspace`, `open_target` |
| Tabs and panes | `new_tab`, `split_horizontal`, `split_vertical`, `next_tab`, `previous_tab`, `next_pane`, `previous_pane`, `focus_pane_left`, `focus_pane_right`, `focus_pane_up`, `focus_pane_down`, `resize_pane_left`, `resize_pane_right`, `resize_pane_up`, `resize_pane_down`, `toggle_pane_zoom`, `close_pane`, `rename_tab` |

These are command names, not all default shortcuts. The example has every
default binding with a short comment. The command palette also lists app
commands and project actions. `rename_tab` prompts for a local title;
palette and prompt input do not reach the PTY.

## Startup workspace

An optional `[workspace]` defines the layout created at app startup. It is
distinct from app-managed Saved Workspaces. Without it, startup creates one
local-shell tab. Its fields are:

| Level | Fields |
| --- | --- |
| `[workspace]` | Optional `project_root`, `active_tab` (default `0`), required nonempty `tabs` when present |
| `[[workspace.tabs]]` | Required `title` and `layout`; optional `custom_title`, `project_root`, `active_pane` (default `0`) |
| Pane layout (`kind = "pane"`) | Required `session`; optional `project_root`, `startup_command` |
| Split layout (`kind = "split"`) | Required `axis`, `first`, and `second`; optional `first_share` (default `500000`) |

`active_tab` and `active_pane` are zero-based and must refer to existing
tabs and panes. Pane indices follow the first child and then the second child
recursively. A split's `axis` is `vertical` (left and right) or
`horizontal` (top and bottom). Each child can be another split or a pane.
`first_share` is the first child's share in millionths, from 1 through
999999. The app rejects empty tab lists, missing sessions or split children,
and invalid focus indices or share values.

`project_root` may appear on the workspace, a tab, or a pane. A pane root
wins over its tab root, which wins over the workspace root. Relative roots
resolve against the config file's directory. The effective root becomes the
pane PTY's initial working directory.

A pane's `session = "local_shell"` starts the normal local shell in a fresh
PTY. A direct-command session uses a `command` table with required
`program` and optional `args` (default `[]`), and launches the program
directly without shell interpolation:

```toml
[workspace]
project_root = "../repo"

[[workspace.tabs]]
title = "Development"
active_pane = 0
layout = { kind = "split", axis = "vertical", first_share = 500000, first = { kind = "pane", session = "local_shell", startup_command = "nvim" }, second = { kind = "pane", session = { command = { program = "cargo", args = ["run"] } } } }
```

`startup_command` is an explicit, nonempty, single-line shell command for a
local-shell pane. It is sent inside the fresh shell after shell startup; it
does not convert the pane into a direct-command session. Once that program
exits, the shell remains usable. It is not inferred from a running process.
A direct-command session keeps its existing direct launch behavior.

## Project palette entries

Each `[[projects]]` entry has a nonempty, unique `name` and a nonempty
`path`. The path may be relative to the config file's directory. Entries
add **New Tab: NAME**, **Split Horizontal: NAME**, and **Split Vertical:
NAME** actions to the command palette. These launch fresh local shells rooted
at that project; a new tab owns its root, and a split records it on its new
pane. The default is no project entries.

## Saved Workspaces and shell directories

**Save Current Workspace** stores a reusable template in app-managed
`workspaces.toml` in the same platform config directory. Saved Workspaces are
not defined in `config.toml`, and `TERMINAL_CONFIG` does not move their
storage. Arrange panes, set their working directories, optionally use **Set
Pane Startup Command**, then save and name the workspace. **Clear Pane Startup
Command** removes the explicit command for future launches without changing
the running process. Reopening through **Open Workspace** creates fresh PTYs
and shells. Layout, focus, known directories, and explicit startup commands
can be reused; terminal contents, scrollback, live processes, shell history,
editor buffers, and SSH state are not restored. The app never inspects the
process tree to infer a command.

On Windows, the normal PowerShell profile remains enabled. The app's
session-local prompt integration reports the shell's current directory via
OSC 7, allowing Saved Workspaces to capture CWD changes. This integration is
currently PowerShell-specific; Bash, Zsh, and Fish CWD reporting is not
implemented.

## CLI options

`terminal-app --project-root DIRECTORY` starts with that workspace-level
root. `terminal-app --workspace FILE.toml` loads a config-shaped TOML file
containing `[workspace]`; it may also contain font, theme, bindings, and
projects. The options can be combined: `--project-root` replaces the
workspace-level root, while explicit tab and pane roots still win. `--help`
prints usage. Invalid paths or arguments fail before the window opens.
