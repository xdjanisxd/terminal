# Architecture

## Status

This document defines intended boundaries. The repository currently contains dependency-free structural crates but no terminal implementation.

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
