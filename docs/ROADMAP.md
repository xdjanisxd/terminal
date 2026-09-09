# Roadmap

Completion marks require tested acceptance criteria, not code presence alone.

## M0: Repository foundation

- [x] Initialize a valid Cargo workspace
- [x] Create dependency-free structural crates for the documented component boundaries
- [x] Create required product and engineering documentation
- [x] Record accepted initial architectural decisions
- [x] Create agent startup, workflow, task, and handoff files
- [x] Add the MIT license and package metadata
- [x] Retain internal `terminal-*` names without blocking on public branding
- [x] Record Windows, Linux, and macOS x86_64/ARM64 targets and an evidence-based process for deciding OS support floors
- [x] Configure all required Cargo quality gates on native CI runners for all six OS/architecture combinations
- [x] Confirm the first authenticated CI run succeeds on all six configured runners

## M1: Terminal core foundation

- [x] Define bounded cell, attribute, screen-grid, and cursor models as the first independently tested `terminal-core` behavior
- [x] Add terminal-global and input-related modes with explicit typed state transitions
- [x] Introduce a parser-independent `TerminalState` facade for atomic terminal operations
- [x] Integrate incremental `vte` parsing behind project-owned interfaces
- [x] Test printable text, arbitrary input chunking, malformed input, and bounded parser state

## M2: Terminal compatibility

- [ ] Implement and test commonly used cursor, erase, scroll, SGR, mode, and reply behavior
- [ ] Implement Unicode combining and wide-cell invariants
- [ ] Implement primary/alternate screens, resize behavior, and bounded scrollback
- [ ] Validate against representative modern shells and TUIs

## M3: PTY and session lifecycle

- [ ] Define the project-owned PTY contract
- [ ] Integrate `portable-pty` for Windows, Linux, and macOS
- [ ] Add deterministic lifecycle, resize, EOF, exit, and termination tests
- [ ] Connect PTY workers through bounded event-driven communication

## M4: Window and renderer

- [ ] Create the `winit` application shell and `wgpu` surface lifecycle
- [ ] Implement font discovery, shaping/rasterization, fallback, and bounded glyph caching
- [ ] Render terminal backgrounds, glyphs, decorations, and cursor
- [ ] Add damage-driven redraw and DPI/resize handling

## Early integration checkpoint

Run this checkpoint after the basic PTY, parser/`TerminalState`, window, primitive renderer, and keyboard path exist. It proves only the minimum end-to-end shell path:

```text
PTY -> parser -> TerminalState -> primitive renderer
keyboard -> PTY
```

- [ ] Open a basic local shell
- [ ] Display shell text through the complete output path
- [ ] Send keyboard input to the shell
- [ ] Execute simple commands and render their output
- [ ] Keep later terminal compatibility, workspace, configuration, and UI features out of this checkpoint

## M5: Input and configuration

- [ ] Add mode-aware keyboard, mouse, focus, and paste encoding
- [ ] Add selection, clipboard, OSC 52 policy, and safe clickable targets
- [ ] Add typed TOML config, validation, themes, keybindings, and partial hot reload
- [ ] Route keybindings and command palette through centralized commands

## M6: Workspaces and release readiness

- [ ] Add tabs, panes, layout, and reproducible workspace definitions
- [ ] Add project roots, startup commands, command palette, and basic CLI control
- [ ] Establish compatibility, fuzzing, and performance baselines
- [ ] Package and validate supported Windows, Linux, and macOS releases
