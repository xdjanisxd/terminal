# Session Handoff

## Current state

`feat/m2-dec-alt-screen-47` adds narrow DEC private mode 47 parser dispatch through `TerminalState`: `CSI ? 47 h` selects Alternate and `CSI ? 47 l` selects Primary. Switching is pure typed `ScreenKind` selection; it preserves both buffers, per-screen cursor/margins/delayed-wrap, and shared rendition, tab stops, terminal/input modes, and pending replies.

`?1047` and `?1049` remain unsupported. No clearing, cursor save/restore, scrollback, reflow, renderer, PTY, or input work exists in this slice. Resize still updates both buffers with top-left preservation; reset recreates blank buffers and selects Primary.

## Validation

Focused DEC `?47` parser tests cover switching, repeated/mixed modes, chunk boundaries, incomplete CSI, isolation, wide/combining cells, resize, and reset. Full workspace format, tests, doctests, check, Clippy, GNU/MSVC target tests, repository static/dependency checks, Graphify refresh, and `git diff --check` passed.

## Next

Add one narrow DEC `?1047` alternate-screen parser/state slice through the established switching facade, keeping saved-cursor semantics and scrollback separate.
