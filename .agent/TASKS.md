# Tasks

## Next

- [ ] Inspect the remaining M2 compatibility surface and select exactly one narrow, high-value slice; do not continue cursor-positioning commands by default.

## Later

- [x] Add bounded CNL/CPL cursor-next/previous-line movement with column reset and no scrolling.
- [x] Add ANSI DSR terminal-status query (`CSI 5 n`) and bounded `CSI 0 n` reply through the existing `TerminalReply` FIFO.
- [x] Add bounded ECH (`CSI Ps X`) current-row cell erasure without shifting neighboring cells.
- [x] Add bounded DCH (`CSI Ps P`) cell deletion on the current row through project-owned grid/state operations.
- [x] Add bounded ICH (`CSI Ps @`) cell insertion on the current row through project-owned grid/state operations.

- [x] Add bounded IL/DL line insertion/deletion within the cursor-to-bottom portion of the active scrolling region, preserving complete cells, canonical blank cells, delayed wrap, and unrelated terminal state.

- [x] Add DECCKM (`CSI ? 1 h`/`CSI ? 1 l`) parser dispatch through the existing typed `CursorKeyMode`, without keyboard/input encoding.

- [x] Add a narrow DSR/CPR slice that reuses the bounded `TerminalReply` FIFO for a cursor-position response without origin mode.

## Later

- [x] Add primary DA1 queries through `TerminalState` with a conservative fixed response, bounded FIFO storage, explicit caller consumption, and no PTY integration.

- [x] Make NEL respect active vertical scrolling margins by composing existing region-aware index and carriage-return semantics.

- [x] Make explicit SU and SD consult the active vertical scrolling margins through the bounded region-scroll primitive, while keeping NEL, origin mode, IL/DL, scrollback, and alternate screens separate.

- [x] Make IND and RI region-aware at active vertical margins through a bounded grid region-scroll primitive while preserving outside-region movement, full-screen defaults, and NEL/SU/SD behavior.

- [x] Introduce typed bounded vertical scrolling margins and DECSTBM parser/state semantics with full-screen defaults, cursor homing, atomic invalid-input handling, and reset/resize invariants.

- [x] Add full-screen IND/RI/NEL through `TerminalState`, reusing bounded SU/SD edge scrolling and cancelling delayed wrap.

- [x] Add bounded full-screen SU/SD through `TerminalState`, with clamped counts, cursor and delayed-wrap preservation, complete-cell movement, and canonical default blank rows.

- [x] Add semicolon-form SGR truecolor foreground/background support through the existing `CellColor::Rgb` rendition model, with bounded grouped malformed-payload handling.

- [x] Start M2 with a narrow typed cursor/erase semantic slice through `TerminalState`; keep protocol numeric identifiers inside the parser adapter.
- [x] Connect standard IRM and private DECAWM/DECTCEM parser dispatch to existing `TerminalState` setters; keep unsupported modes as no-ops and protocol identifiers private.
- [x] Add terminal-owned mutable horizontal tab stops with HT, HTS, current/all TBC, bounded movement, and explicit resize/reset behavior.
- [x] Add project-owned current rendition with SGR reset, bold, italic, underline, inverse, ANSI standard/bright colors, and per-channel default restoration.
- [ ] Add origin mode only with scrolling-region state, cursor homing, saved-state behavior, and primary/alternate-screen semantics coordinated atomically by `TerminalState`.
- [ ] Defer line-feed/new-line mode and application keypad mode until their behavior is required by parser/input milestones; do not grow a speculative mode catalog.
- [ ] Extend cell content for combining and wide-character invariants during the Unicode compatibility slice; do not assume the current single-`char` representation is final.
- [ ] Decide resize reflow behavior during the compatibility milestone; the current top-left preservation rule does not perform terminal line reflow.
- [ ] Determine minimum OS versions from pinned stack requirements and real platform validation when those dependencies are introduced; record the result in a new ADR.

## Deferred by scope

- Plugin system, embedded AI, SSH manager, IDE features, runtime session restoration, and large scripting/UI frameworks remain outside V1.
