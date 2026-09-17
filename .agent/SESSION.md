# Session Handoff

## Current state

`feat/m2-screen-state-foundation` adds project-owned primary and alternate screen buffers behind `TerminalState`. `ScreenKind` selects the active buffer atomically.

Per-screen state: `ScreenGrid` (and cursor), vertical scrolling margins, and delayed-wrap state. Shared state: current rendition, horizontal tab stops, terminal modes (IRM, DECAWM, DECTCEM), input cursor-key mode, and pending replies. Both buffers resize with the existing top-left-preserving policy; reset recreates blank buffers and selects Primary.

No DEC alternate-screen parser dispatch, saved-cursor semantics, scrollback, or reflow exists.

## Validation

All workspace format, tests, doctests, check, Clippy, GNU/MSVC target tests, and `git diff --check` passed.

## Next

Add one carefully selected DEC alternate-screen parser mode family to the established switching facade, without saved-cursor or scrollback behavior.
