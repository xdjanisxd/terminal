# Architecture

## Status

This document defines intended boundaries. The repository currently contains a project-owned incremental parser adapter over `vte` and a `TerminalState` facade with printable, basic control, cursor-movement, erase, narrow mode, and bounded terminal-reply semantics, including primary DA1 and ANSI cursor-position reports, but no PTY, renderer, or application behavior.

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
TerminalState pending replies -> explicit caller consumption -> future PTY writer
input -> command resolver or terminal encoder -> PTY writer
```

The renderer consumes terminal snapshots/state and damage information; it does not own terminal semantics. Platform-specific implementations remain behind narrow project-owned interfaces.

## Initial terminal-core model invariants

- `TerminalDimensions` is non-zero and rejects per-axis and total-cell resource limits.
- `ScreenGrid` uses zero-based `(row, column)` coordinates over row-major storage; checked access returns `None` outside the grid. A `Cell` has project-owned `Single`, `WideLead`, or `WideContinuation` occupancy. At every stable grid boundary, a lead is followed immediately by exactly one continuation, a continuation is preceded immediately by its lead, and neither can appear at a row edge without its pair. The lead alone holds the character and rendition; generic bounded horizontal edits normalize clipped or split pairs to canonical blanks.
- A screen grid owns its cursor so cursor mutations and resize keep it inside current dimensions.
- Clearing writes default blank cells without moving the cursor.
- Resize preserves the top-left rectangular intersection, blanks newly exposed cells, discards cells outside the new rectangle, and clamps the cursor. It does not yet implement terminal line reflow.
- Line feed at the bottom row scrolls the active fixed-size grid up by one row and blanks the new bottom row. Discarded cells are not retained because scrollback is not modeled yet.

## Mode ownership

- `TerminalModes` owns values shared across the terminal: cursor visibility, auto-wrap, and insert/replace behavior. Defaults are visible, enabled, and replace.
- `InputModes` separately owns output-controlled state consumed by future keyboard encoding. Application cursor keys default to normal encoding.
- Cursor, cells, delayed-wrap state, and typed vertical scrolling margins are screen-local state, not mode flags. `TerminalState` keeps separate values in each typed primary/alternate `ScreenKind` buffer; terminal-global and input-related modes remain shared.
- Origin mode is intentionally deferred. Setting or resetting it must atomically coordinate cursor homing, scrolling margins, saved cursor state, and primary/alternate-screen behavior through `TerminalState`.
- The parser adapter translates supported protocol parameters into typed project-owned operations. Numeric VT/xterm identifiers are not exposed by the core model.
- Primary and alternate screens own separate screen state. Protocol-specific save/restore behavior must be modeled explicitly rather than implied by buffer switching.

## TerminalState boundary

- `TerminalState` privately owns a `ScreenSet` of independent primary and alternate `ScreenGrid` buffers, current rendition, `TerminalModes`, `InputModes`, horizontal tab-stop state bounded by the validated column count, and a fixed-capacity FIFO of pending project-owned terminal replies.
- Callers receive immutable screen, rendition, mode, and scrolling-margin access plus explicit FIFO reply consumption. Printable output, carriage return, line feed, index/reverse-index/next-line controls, backspace, region-aware scroll up/down, horizontal-tab movement and tab-stop mutation, scrolling-margin replacement, typed cursor movement, typed erase, rendition changes, clearing, resizing, mode transitions, and reply generation use semantic methods on `TerminalState`.
- Parser adapters translate protocol input into `TerminalState` operations rather than mutating owned low-level models directly.
- Low-level model types remain independently constructible and testable, but the facade exposes no mutable reference to its owned values.
- The current project-owned reset preserves dimensions and already-generated pending replies, recreates both blank screen buffers with Primary active, restores current rendition and supported modes to defaults, restores horizontal tab stops every eight columns beginning at zero-based column 8, and restores full-screen vertical scrolling margins. Resize also preserves pending replies. This reset is not DECSTR or RIS.
- `TerminalReply` is a non-exhaustive project-owned reply representation. `TerminalState` retains at most 16 replies in FIFO order; consumption removes the oldest reply. At capacity, a new reply is rejected without panic or replacement, and consuming a reply frees one slot. DA1 emits the exact seven-byte response `ESC [ ? 1 ; 0 c`, identifying a conservative VT100-class terminal with no optional capability codes rather than advertising unsupported features. ANSI cursor-position reports capture the current absolute zero-based screen cursor as bounded one-based row and column values at request time and encode as `ESC [ <row> ; <column> R`; active scrolling margins do not alter those coordinates.
- DA2 is fixed at `ESC [ > 0 ; 0 ; 0 c`: Pp=0 matches DA1's conservative VT100-class policy, Pv=0 is a stable compatibility value rather than a package version, and Pc=0 advertises no optional hardware capabilities.
- Horizontal tab stops are mutable terminal state rather than fixed arithmetic. HT skips a stop at the current column, moves to the next later stop, or clamps to the final column when no later stop exists. HT never modifies cells or modes and cancels delayed wrap. Resize preserves stops in surviving columns, discards truncated stops, and gives newly exposed columns conventional default stops.
- `VerticalScrollingMargins` is an inclusive zero-based typed region that permits `top == bottom` so one-row screens remain representable, while rejecting reversed or out-of-screen bounds. `TerminalState` initializes and resets it to the full screen; every resize resets it to the full new height rather than preserving or clamping a custom region. A successful semantic margin update homes the cursor at screen origin and cancels delayed wrap; rejected updates are atomic no-ops.
- `ScreenGrid` owns a distinct bounded inclusive region-scroll primitive. It receives explicit validated row bounds, remains independent of `TerminalState` and active-margin ownership, clamps counts to region height before calculating one overlapping copy, clears only the selected region at or above that height, moves complete `Cell` values without allocation, and fills newly exposed rows with `Cell::default()`. `TerminalState` translates active `VerticalScrollingMargins` into explicit grid regions for IND, RI, SU, and SD. NEL composes the same region-aware index operation with carriage return, so it inherits bounded movement and region scrolling without separate boundary logic. Cursor visibility, modes, current rendition, tab stops, and margins are preserved; SU/SD preserve delayed wrap, while IND, RI, and NEL cancel it.
- `TerminalState` owns two independent project `ScreenState` buffers selected by typed `ScreenKind`. Each buffer owns its grid/cursor, vertical scrolling margins, and delayed-wrap state. Current rendition, horizontal tab stops, terminal modes, input modes, and reply FIFO remain shared terminal-global state. Switching is atomic and does not copy or merge cells; resize applies existing grid semantics to both buffers and reset recreates both blank buffers with Primary active.
- Printing uses VT-style delayed wrapping: writing the final column leaves the cursor there and records a private pending wrap; the next printable character wraps before it is written when auto-wrap is enabled. Disabling auto-wrap keeps output at the final column. Typed cursor movement, IND, RI, NEL, horizontal tab, carriage return, erase, clearing, resize, and reset cancel a pending wrap. Line feed preserves pending wrap as well as the cursor column; backspace cancels it only when the cursor can move left.
- `CellAttributes` is both the typed current-rendition value and the immutable snapshot stored with each printed cell. It separates normal/bold intensity, upright/italic style, underline, inverse video, foreground, and background without parser identifiers. Color values reuse `CellColor`; default, all 256 indexed values, and exact RGB values are representable and used by the supported SGR forms.
- Printing classifies Unicode scalar width through `unicode-width` at the `TerminalState` semantic boundary. Width-one output snapshots the current rendition into one `Single` cell; width-two output uses one `WideLead` with exactly one adjacent `WideContinuation`, and the lead alone carries the scalar, rendition, and up to eight ordered width-zero attachments. Width-zero scalars attach to the preceding logical printable base without changing its rendition, cursor, or delayed-wrap state; missing bases are ignored and capacity overflow is a project-owned error without mutation. Grid-owned repair normalizes clipped or intersected pairs. This is not grapheme segmentation, normalization, ZWJ composition, variation-selector shaping, or renderer shaping. Scrollback remains a separate future state model.

## Parser boundary

- `TerminalParser` is the public project-owned incremental byte interface. It owns parser state across calls and exposes no `vte` parser, callback, parameter, or action type.
- A private `vte::Perform` implementation translates printable characters, CR, LF, BS, HT, HTS, IND, RI, NEL, primary DA1, secondary DA2, ANSI DSR status, ANSI DSR cursor-position queries, and the supported cursor/erase/scroll/margin/mode/TBC/SGR CSI subset into `TerminalState` semantic methods only. It never mutates grids, screen-set storage, cell attributes, tab-stop storage, scrolling-margin storage, reply storage, or mode models directly, and it performs no host or PTY I/O. DECCKM selects the typed input-related cursor-key mode only; DEC `?47` selects the existing typed primary/alternate screen through `TerminalState` without clearing or saved-cursor behavior; DEC `?1047` invokes one `TerminalState` entry operation that resets only the alternate grid/cursor, margins, and delayed-wrap before selecting it, while its exit selects Primary without saved-cursor behavior; a future input encoder consumes input state, so this parser path emits no key bytes.
- The supported CSI subset includes primary DA queries `CSI c` and `CSI 0 c`, ANSI DSR terminal-status query `CSI 5 n`, ANSI DSR cursor-position query `CSI 6 n`, CUU, CUD, CUF, CUB, CUP/HVP, CHA, VPA, bounded current-row ICH/DCH/ECH, ED modes 0-2, EL modes 0-2, cursor-relative IL/DL, region-aware SU/SD, DECSTBM, TBC current/all, standard IRM, private DECCKM/DECAWM/DECTCEM/non-clearing alternate-screen `?47`/alternate-resetting `?1047`, and SGR reset, bold, italic, underline, inverse, ANSI standard/bright, `38;5;n`/`48;5;n` indexed, and semicolon-form `38;2;r;g;b`/`48;2;r;g;b` exact RGB foreground/background colors, plus channel-default restoration. IL/DL use a `TerminalState`-owned subregion from the current cursor row through the active bottom margin, and are no-ops outside the active margins; their parser path only normalizes the single CSI count and invokes that semantic method. ICH and DCH use `TerminalState`-owned current-row operations that delegate bounded complete-cell shifting and canonical default-blank fill to `ScreenGrid`; both preserve cursor coordinates while cancelling delayed wrap. ECH uses a `TerminalState`-owned current-row operation that delegates its bounded range clear to `ScreenGrid`, preserving cells after the erased range and cursor coordinates while cancelling delayed wrap. Nonzero, extra, subparameter, unsupported private-prefixed, unsupported secondary, and tertiary DA forms remain safe no-ops. CSI parameters and numeric protocol identifiers remain private to the adapter; cursor and scroll operations normalize omitted and zero parameters, and cursor and DECSTBM operations convert one-based coordinates to project-owned zero-based semantics. DECSTBM defaults omitted or zero top to row 1 and omitted or zero bottom to the screen height, rejects equal or reversed multi-row bounds, out-of-range values, extra scalar parameters, and subparameter groups atomically, and permits the full one-row region as the single-row invariant exception.
- SGR parameters are applied in order. Bare SGR and empty scalar parameters are reset operations, matching `vte` parameter dispatch and terminal convention. Semicolon-form indexed colors consume their selector and index as one logical group. Semicolon-form truecolor consumes exactly three scalar RGB payload parameters and validates all three before changing either channel, so incomplete, grouped, or out-of-range payloads cannot partially mutate rendition or leak their group payload into unrelated SGR. Unknown extended-color selectors consume the remainder of that CSI callback because their payload length is undefined; this prevents payload values from being reinterpreted as unrelated SGR. Other unsupported scalar parameters and all colon/subparameter groups are ignored individually.
- Unsupported ESC, OSC, DCS, executed controls, and unsupported or malformed CSI callbacks are no-ops. The parser still consumes them incrementally so input following a complete unsupported sequence returns to normal parsing without an approximation of that sequence's behavior.
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
