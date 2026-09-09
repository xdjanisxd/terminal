# Architecture

## Status

This document defines intended boundaries. The repository currently contains a dependency-free `TerminalState` facade over screen and mode models, but no parser, PTY, renderer, or application behavior.

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
PTY bytes -> vte parser -> TerminalState -> renderer
input -> command resolver or terminal encoder -> PTY writer
```

The renderer consumes terminal snapshots/state and damage information; it does not own terminal semantics. Platform-specific implementations remain behind narrow project-owned interfaces.

## Initial terminal-core model invariants

- `TerminalDimensions` is non-zero and rejects per-axis and total-cell resource limits.
- `ScreenGrid` uses zero-based `(row, column)` coordinates over row-major storage; checked access returns `None` outside the grid.
- A screen grid owns its cursor so cursor mutations and resize keep it inside current dimensions.
- Clearing writes default blank cells without moving the cursor.
- Resize preserves the top-left rectangular intersection, blanks newly exposed cells, discards cells outside the new rectangle, and clamps the cursor. It does not yet implement terminal line reflow.

## Mode ownership

- `TerminalModes` owns values shared across the terminal: cursor visibility, auto-wrap, and insert/replace behavior. Defaults are visible, enabled, and replace.
- `InputModes` separately owns output-controlled state consumed by future keyboard encoding. Application cursor keys default to normal encoding.
- The current subset contains no screen-local mode. Cursor, cells, and future margins/wrap-pending state are screen-local state, but are not interchangeable with mode flags.
- Origin mode is intentionally deferred. Setting or resetting it must atomically coordinate cursor homing, scrolling margins, saved cursor state, and future primary/alternate-screen behavior through `TerminalState`.
- A future parser translates protocol numeric identifiers into typed project-owned operations. Numeric VT/xterm mode identifiers are not exposed by the core model.
- Future primary and alternate screens own separate screen state. Terminal-global and input-related modes remain owned once by `TerminalState`; protocol-specific save/restore behavior must be modeled explicitly rather than implied by buffer switching.

## TerminalState boundary

- `TerminalState` privately owns one active `ScreenGrid`, `TerminalModes`, and `InputModes`.
- Callers receive immutable screen and mode access. Cursor movement, clearing, resizing, and mode transitions use semantic methods on `TerminalState`.
- Future parser adapters translate protocol input into `TerminalState` operations rather than mutating owned low-level models directly.
- Low-level model types remain independently constructible and testable, but the facade exposes no mutable reference to its owned values.
- The current project-owned reset preserves dimensions, clears the active screen, homes the cursor, and restores supported modes to defaults. It is not DECSTR or RIS.

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
