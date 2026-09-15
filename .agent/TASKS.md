# Tasks

## Next

- [ ] Make explicit SU and SD consult the active vertical scrolling margins through the bounded region-scroll primitive, while keeping NEL, origin mode, IL/DL, scrollback, and alternate screens separate.

## Later

- [x] Make IND and RI region-aware at active vertical margins through a bounded grid region-scroll primitive while preserving outside-region movement, full-screen defaults, and NEL/SU/SD behavior.

- [x] Introduce typed bounded vertical scrolling margins and DECSTBM parser/state semantics with full-screen defaults, cursor homing, atomic invalid-input handling, and reset/resize invariants.

- [x] Add full-screen IND/RI/NEL through `TerminalState`, reusing bounded SU/SD edge scrolling and cancelling delayed wrap.

- [x] Add bounded full-screen SU/SD through `TerminalState`, with clamped counts, cursor and delayed-wrap preservation, complete-cell movement, and canonical default blank rows.

- [x] Add bounded SGR indexed-color parsing for `38;5;n` and `48;5;n` using the existing `CellColor::Indexed` model; truecolor, colon color forms, and unrelated styles remain deferred.

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
