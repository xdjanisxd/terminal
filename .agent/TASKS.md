# Tasks

## Next

- [ ] Audit the remaining primary/alternate-screen, resize, and scrollback parent before choosing a narrow resize/reflow policy or a read-only viewport model; do not add renderer UI or shell/TUI validation yet.

##...[truncated]

## Completed in this slice

- [x] Add bounded Primary-only scrollback capture for whole-screen upward scrolling, exact `Cell` preservation, 10,000-row FIFO eviction, reset clearing, capture-time width retention across resize, and no viewport/reflow behavior.
- [x] Add DEC `?1049` parser dispatch through dedicated `TerminalState` save-Primary/reset-Alternate/switch/restore-Primary semantics, with idempotent Alternate entry and no new saved fields.
- [x] Add project-owned per-screen saved-cursor state with explicit `TerminalState` save/restore semantics: cursor coordinates and current rendition only, uninitialized restore no-op, restore-time coordinate clamping, `?47` preservation, `?1047` Alternate-slot reset, and no parser dispatch.
- [x] Add DEC `?1047` parser dispatch through `TerminalState` for alternate-screen-local grid/cursor/margins/delayed-wrap reset on entry, Primary selection on exit, and no saved cursor or global state reset.
- [x] Add DEC `?47` parser dispatch through `TerminalState` for non-clearing primary/alternate screen selection, preserving screen-local and shared state while leaving `?1047` unsupported at that slice.
- [x] Add width-zero combining-mark attachment to narrow and wide base cells with bounded ordered storage, no cursor advancement, delayed-wrap preservation, and no standalone mark cells.
- [x] Add typed wide-cell lead/continuation representation, bounded grid repair, width-two output, and no-orphan resize/edit invariants; defer width-zero combining behavior.

## Later

- [x] Add typed SGR faint intensity with ordered `1`/`2`/`22` semantics through the existing rendition model.
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
- [x] Audit Unicode width classifications, width-zero/variation-selector attachment policy, payload bounds, repair paths, and public ownership; close the Unicode parent without claiming grapheme or rendering...[truncated]
- [ ] Decide resize reflow behavior during the compatibility milestone; the current top-left preservation rule does not perform terminal line reflow.
- [ ] Determine minimum OS versions from pinned stack requirements and real platform validation when those dependencies are introduced; record the result in a new ADR.

## Deferred by scope

- Plugin system, embedded AI, SSH manager, IDE features, runtime session restoration, and large scripting/UI frameworks remain outside V1.
