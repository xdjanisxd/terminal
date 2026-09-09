# Architecture

## Status

This document defines intended boundaries. The repository currently contains a project-owned incremental parser adapter over `vte` and a `TerminalState` facade with printable, basic control, cursor-movement, and erase semantics, but no PTY, renderer, or application behavior.

## Conceptual components

- `app`: entry point, `winit` event loop, lifecycle, command dispatch, and component wiring
- `terminal-core`: parser integration, terminal state, grid, cursor, modes, scrollback, selection, and terminal semantics
- `terminal-pty`: PTY abstraction, child lifecycle, and input/output communication
- `renderer`: `wgpu` resources, terminal drawing, fonts, shaping/rasterization, and bounded glyph atlas/cache
- `workspace`: tabs, panes, layout tree, workspace definitions, and startup commands
- `config`: TOML loading, defaults, validation, keybindings, themes, and partial hot reload
- `platform`: clipboard, paths, and narrowly scoped operating-system behavior

The current internal packages are `terminal-app`, `terminal-core`, `terminal-pty`, `terminal-renderer`, `terminal-workspace`, `terminal-config`, and `terminal-platform`. These internal names are retained while public branding remains undecided; public naming does not block development.

## Dependency direction

`terminal-core` must not depend on `winit`, `wgpu`, UI, workspace state, or application orchestration. It must remain independently testable.

Primary data flow:

```text
raw terminal bytes -> TerminalParser -> private vte parser/callbacks -> TerminalState -> renderer
input -> command resolver or terminal encoder -> PTY writer
```

The renderer consumes terminal snapshots/state and damage information; it does not own terminal semantics. Platform-specific implementations remain behind narrow project-owned interfaces.

## Initial terminal-core model invariants

- `TerminalDimensions` is non-zero and rejects per-axis and total-cell resource limits.
- `ScreenGrid` uses zero-based `(row, column)` coordinates over row-major storage; checked access returns `None` outside the grid.
- A screen grid owns its cursor so cursor mutations and resize keep it inside current dimensions.
- Clearing writes default blank cells without moving the cursor.
- Resize preserves the top-left rectangular intersection, blanks newly exposed cells, discards cells outside the new rectangle, and clamps the cursor. It does not yet implement terminal line reflow.
- Line feed at the bottom row scrolls the active fixed-size grid up by one row and blanks the new bottom row. Discarded cells are not retained because scrollback is not modeled yet.

## Mode ownership

- `TerminalModes` owns values shared across the terminal: cursor visibility, auto-wrap, and insert/replace behavior. Defaults are visible, enabled, and replace.
- `InputModes` separately owns output-controlled state consumed by future keyboard encoding. Application cursor keys default to normal encoding.
- The current subset contains no screen-local mode. Cursor, cells, delayed-wrap state, and future margins are screen-local state, but are not interchangeable with mode flags. The single-screen `TerminalState` currently stores delayed-wrap state privately; future screen-buffer ownership must keep separate delayed-wrap state per primary or alternate screen.
- Origin mode is intentionally deferred. Setting or resetting it must atomically coordinate cursor homing, scrolling margins, saved cursor state, and future primary/alternate-screen behavior through `TerminalState`.
- The parser adapter translates supported protocol parameters into typed project-owned operations. Numeric VT/xterm identifiers are not exposed by the core model.
- Future primary and alternate screens own separate screen state. Terminal-global and input-related modes remain owned once by `TerminalState`; protocol-specific save/restore behavior must be modeled explicitly rather than implied by buffer switching.

## TerminalState boundary

- `TerminalState` privately owns one active `ScreenGrid`, `TerminalModes`, and `InputModes`.
- Callers receive immutable screen and mode access. Printable output, carriage return, line feed, backspace, typed cursor movement, typed erase, clearing, resizing, and mode transitions use semantic methods on `TerminalState`.
- Parser adapters translate protocol input into `TerminalState` operations rather than mutating owned low-level models directly.
- Low-level model types remain independently constructible and testable, but the facade exposes no mutable reference to its owned values.
- The current project-owned reset preserves dimensions, clears the active screen, homes the cursor, and restores supported modes to defaults. It is not DECSTR or RIS.
- Printing uses VT-style delayed wrapping: writing the final column leaves the cursor there and records a private pending wrap; the next printable character wraps before it is written when auto-wrap is enabled. Disabling auto-wrap keeps output at the final column. Typed cursor movement, carriage return, erase, clearing, resize, and reset cancel a pending wrap. Line feed preserves pending wrap as well as the cursor column; backspace cancels it only when the cursor can move left.
- Printing currently accepts only ASCII space through tilde, whose one-cell width the model can guarantee without external Unicode data. Insert mode shifts those fixed-width cells right within the current row and discards the final cell. Other Unicode is rejected rather than approximating combining or wide-character behavior.

## Parser boundary

- `TerminalParser` is the public project-owned incremental byte interface. It owns parser state across calls and exposes no `vte` parser, callback, parameter, or action type.
- A private `vte::Perform` implementation translates printable characters, CR, LF, BS, and the supported cursor/erase CSI subset into `TerminalState` semantic methods only. It never mutates grids or mode models directly.
- The supported CSI subset is CUU, CUD, CUF, CUB, CUP/HVP, CHA, VPA, ED modes 0-2, and EL modes 0-2. CSI parameters remain private `vte` values until the adapter normalizes omitted and zero parameters and converts one-based absolute coordinates to project-owned zero-based semantics.
- ESC, OSC, DCS, unsupported executed controls, and unsupported or malformed CSI callbacks are no-ops. The parser still consumes them incrementally so input following a complete unsupported sequence returns to normal parsing without an approximation of that sequence's behavior.
- `vte` is compiled without default features so OSC collection uses a fixed 1,024-byte buffer rather than an unbounded `Vec`. Excess unsupported OSC payload is discarded by the parser.
- `TerminalParser::advance` processes without per-byte allocation and uses `vte` termination checks to stop before callbacks following the first `TerminalState` semantic error can mutate state. Its project-owned error reports both that semantic error and the number of bytes consumed, allowing the caller to resume with the unconsumed suffix without losing later errors silently. The count can be zero when malformed UTF-8 retained from a prior chunk is rejected before the current byte is reprocessed.

## Runtime model

Use one process. The main thread owns the `winit` event loop, input, windows, and render scheduling. PTY reads and child communication run on worker threads and wake the event loop when work is available. Start with `std::thread` and `std::sync`; add an async runtime or more workers only for a demonstrated architectural need. Avoid polling and request redraws only on state changes.

## Technical direction

- Window/input: `winit`
- GPU rendering: `wgpu`
- PTY abstraction: `portable-pty`
- Escape parsing: `vte`
- Terminal state: project-owned implementation
- Fonts: `fontdb` plus `swash`
- Configuration: `serde`, `toml`, and `notify`
- CLI: `lexopt`
- Errors: `thiserror` in libraries and `anyhow` at application boundaries
- Logging: `log`

Dependencies are not added until the owning crate and tested behavior require them. Replacing a selected technology requires an ADR before implementation.

## Commands and workspaces

Keyboard bindings, command palette actions, and CLI control resolve to centralized commands such as `terminal.new_tab`, `pane.split_vertical`, and `config.reload`. UI handlers must not duplicate feature behavior.

Configuration and runtime state remain separate. Invalid hot-reloaded configuration must leave the last valid configuration active and report a useful error.

## Resource and trust boundaries

Bound scrollback, parser string payloads, channels, terminal dimensions, and glyph caches. Treat terminal output, OSC/DCS/APC data, hyperlinks, titles, clipboard requests, and startup commands as untrusted input. OSC 52 and clickable targets require explicit policy. Spawn programs with executable and argument fields, not interpolated command strings.
