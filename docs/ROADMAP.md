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
  - [x] Add DECCKM parser dispatch to the existing typed application/normal cursor-key mode
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
- [x] Validate against representative modern shells and TUIs
  - Deterministic `terminal-core` fixtures cover shell prompt redraw, full-screen editor alternate-screen behavior, and multiplexer-style region/mode/reply traffic without external runtime dependencies.

## M3: PTY and session lifecycle

- [x] Define the project-owned PTY contract
- [x] Integrate `portable-pty` for Windows, Linux, and macOS
  - Native GitHub Actions run 35320057146 for commit `8d8e32668fe6ee41770514e5b64871d520a04bc5` passed the adapter smoke test on Linux x86_64/ARM64, macOS x86_64/ARM64, and Windows x86_64/ARM64.
- [x] Add deterministic lifecycle, resize, EOF, exit, and termination tests
  - Native GitHub Actions run 35329910411 for commit `c848fac066693eee537e6986265b5c873256f8d6` passed the lifecycle suite on Linux x86_64/ARM64, macOS x86_64/ARM64, and Windows x86_64/ARM64.
- [x] Connect PTY workers through bounded event-driven communication
  - Native GitHub Actions run 35593337297 for commit `c1e65824c14f970f192745493ff68a9104954b0e` passed the worker slice on Linux x86_64/ARM64, macOS x86_64/ARM64, and Windows x86_64/ARM64.

## M4: Window and renderer

- [x] Create the `winit` application shell and `wgpu` surface lifecycle
  - Native GitHub Actions run 35596742589 for commit `b3e06bb98414e2992da37f3c004ca7c1ecc943c9` passed on Linux x86_64/ARM64, macOS x86_64/ARM64, and Windows x86_64/ARM64.
- [x] Implement font discovery, shaping/rasterization, fallback, and bounded glyph caching
  - Completed sub-slices: renderer-owned system discovery/loading, initial monospace-face selection, selected-face shaping, CPU alpha-mask rasterization, deterministic fallback selection, and bounded LRU glyph caching. The later rendering slice supplies bounded GPU atlas upload.
- [x] Render terminal backgrounds, glyphs, decorations, and cursor
  - `terminal-renderer` converts resolved terminal state into renderer-owned backgrounds, glyph alpha-mask quads, underlines, and cursor primitives, using a bounded GPU atlas and existing font fallback/cache pipeline. Windows visual smoke validation confirmed normal/wide/combining glyphs, non-default backgrounds, underlines, cursor, resize, and minimize/restore without corruption. Native GitHub Actions run 35699195505 passed the six-runner baseline; focused validation covers the subsequent wide-cell cursor-advance regression fix.
- [x] Add damage-driven redraw and DPI/resize handling
  - `terminal-app` coalesces invalidation and synchronizes the terminal grid to DPI-aware drawable size. Restore, fullscreen, and surface-recovery paths use one event-driven successor frame after the initial successful recovery present; diagnostics and Windows manual acceptance verify the recovery remains finite.
- [x] Add bounded scrollback viewport navigation
  - `terminal-core` owns the Primary-only history/viewport projection and bounds.
  - App-local PageUp/PageDown navigation never writes navigation keys to the PTY.
  - The renderer projects the selected viewport and hides the live cursor while scrolled back.
- [x] Add Primary-history mouse-wheel navigation and a visual scrollbar
  - The app converts line/pixel wheel motion to bounded row movement through the core viewport API; the renderer derives track/thumb geometry from history length, visible rows, and viewport offset.
  - Alternate screen has neither scrollback navigation nor scrollbar; thumb dragging, mouse reporting, selection, clipboard, config, and keybindings remain out of scope.

## Early integration checkpoint

Run this checkpoint after the basic PTY, parser/`TerminalState`, window, primitive renderer, and keyboard path exist. It proves only the minimum end-to-end shell path:

```text
PTY -> parser -> TerminalState -> primitive renderer
keyboard -> PTY
```

- [x] Open a basic local shell
- [x] Display shell text through the complete output path
- [x] Send keyboard input to the shell
- [x] Execute simple commands and render their output
- [x] Keep later terminal compatibility, workspace, configuration, and UI features out of this checkpoint

Completed on `feat/integration-basic-shell`: Windows manual acceptance covered
PowerShell, basic shell input/output, `echo`/`dir`, and structurally usable
Vim/Neovim rendering. Live resize is acceptable for this checkpoint, but
trails the pointer and temporarily scales text during a drag; retain that as
later performance/polish work.

## M5: Input and configuration

- [x] Add the first mode-aware input slice: cursor keys, focus, bracketed-paste encoding, and terminal mouse reporting (paste source remains for the clipboard task)
- [x] Add viewport selection, Windows clipboard copy/paste, and bounded OSC 52 write policy (denied by default)
- [ ] Add clickable activation after parsed target metadata and an explicit trust policy exist; current cells expose no targets
- [ ] Add typed TOML config, validation, themes, keybindings, and partial hot reload
- [ ] Route keybindings and command palette through centralized commands

## M6: Workspaces and release readiness

- [ ] Add tabs, panes, layout, and reproducible workspace definitions
- [ ] Add project roots, startup commands, command palette, and basic CLI control
- [ ] Establish compatibility, fuzzing, and performance baselines
- [ ] Dedicated input and frame-pacing performance task: investigate delayed typing/input, janky Backspace/delete, non-smooth PageUp/PageDown, live resize trailing the pointer (including temporary text scaling), and overall UI/frame pacing relative to mature terminals such as Alacritty
- [ ] Package and validate supported Windows, Linux, and macOS releases
