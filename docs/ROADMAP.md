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

- [x] Implement and test the first typed cursor-movement and erase compatibility slice
- [x] Connect parser dispatch for the existing insert/replace, auto-wrap, and cursor-visibility modes
- [x] Add terminal-owned horizontal tab stops with HT, HTS, TBC, resize, and reset semantics
- [x] Add project-owned current rendition with basic style and 16-color SGR parsing
- [x] Add bounded semicolon-form SGR indexed foreground/background colors with grouped malformed-payload handling
- [x] Add bounded full-screen SU/SD with cursor, rendition, attribute, and delayed-wrap preservation
- [x] Add full-screen IND/RI/NEL through `TerminalState` with bounded edge scrolling and delayed-wrap cancellation
- [x] Add typed bounded vertical scrolling margins and DECSTBM parser/state semantics with full-screen reset/resize behavior
- [x] Make IND and RI region-aware through a bounded inclusive grid region-scroll primitive
- [x] Make explicit SU and SD use the active vertical scrolling margins through the bounded inclusive grid region-scroll primitive
- [x] Make NEL compose region-aware index semantics with carriage return while preserving full-screen compatibility
- [x] Add bounded primary DA1 query/reply semantics through `TerminalState` without PTY integration
- [x] Implement and test remaining commonly used scroll, SGR, mode, and reply behavior
  - [x] Add ANSI cursor-position DSR (`CSI 6 n`) and bounded absolute CPR replies through `TerminalState`
  - [x] Add ANSI DSR terminal-status query (`CSI 5 n`) and bounded `CSI 0 n` reply
  - [x] Add semicolon-form truecolor foreground/background SGR through the existing project-owned rendition model
  - [x] Add DECCKM parser dispatch to the existing typed application/normal cursor-key mode without input encoding
  - [x] Add bounded IL/DL line insertion/deletion within the cursor-to-bottom portion of the active scrolling region
  - [x] Add bounded ICH cell insertion on the current row through project-owned grid/state operations
  - [x] Add bounded DCH cell deletion on the current row through project-owned grid/state operations
  - [x] Add bounded ECH current-row cell erasure without shifting neighboring cells
  - [x] Add bounded CNL/CPL cursor-next/previous-line movement with column reset and no scrolling
  - [x] Add typed SGR faint intensity with ordered `1`/`2`/`22` semantics through the existing rendition model
  - [x] Add conservative fixed DA2 secondary-device-attributes replies through the existing bounded reply FIFO
- [x] Implement Unicode combining and wide-cell invariants
  - [x] Add typed wide-cell lead/continuation representation and bounded grid/write invariants
  - [x] Add width-zero combining-mark attachment to narrow and wide base cells without cursor advancement
- [x] Implement primary/alternate screens, resize behavior, and bounded scrollback
  - [x] Add project-owned primary/alternate screen buffers with atomic TerminalState switching and independent screen-local state
  - [x] Add DEC `?47` parser dispatch for non-clearing primary/alternate screen switching
  - [x] Add DEC `?1047` alternate-screen entry/exit with explicit alternate clear/reset semantics and no saved cursor
  - [x] Add project-owned per-screen saved-cursor state with explicit `TerminalState` save/restore semantics
  - [x] Add DEC `?1049` alternate-screen save/reset/switch/restore semantics through project-owned saved-cursor state
  - [x] Add bounded Primary-screen scrollback capture for full-screen upward scrolling with deterministic eviction
  - [x] Audit and test resize coherence across both screens, saved cursor slots, tabs, margins, and mixed-width Primary history
- [ ] Validate against representative modern shells and TUIs

## M3: PTY and session lifecycle

- [x] Define the project-owned PTY contract
- [x] Integrate `portable-pty` for Windows, Linux, and macOS
  - Native GitHub Actions run 35320057146 for commit `8d8e32668fe6ee41770514e5b64871d520a04bc5` passed the adapter smoke test on Linux x86_64/ARM64, macOS x86_64/ARM64, and Windows x86_64/ARM64.
- [x] Add deterministic lifecycle, resize, EOF, exit, and termination tests
  - Native GitHub Actions run 35329910411 for commit `c848fac066693eee537e6986265b5c873256f8d6` passed the lifecycle suite on Linux x86_64/ARM64, macOS x86_64/ARM64, and Windows x86_64/ARM64.
- [x] Connect PTY workers through bounded event-driven communication
  - Native GitHub Actions run 35593337297 for commit `c1e65824c14f970f192745493ff68a9104954b0e` passed the worker slice on Linux x86_64/ARM64, macOS x86_64/ARM64, and Windows x86_64/ARM64.

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
