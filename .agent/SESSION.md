# Session Handoff

## Current state

`feat/m2-saved-cursor-foundation` adds explicit parser-independent `TerminalState::save_cursor` and `restore_cursor` operations. Each project-owned `ScreenState` owns one optional bounded `SavedCursor` slot. A slot captures only its screen cursor coordinates and the terminal current rendition at save time; it does not capture cells, delayed wrap, margins, tabs, terminal/input modes, or replies.

Restore before save is a safe no-op. A successful restore affects only the active screen: it clamps saved coordinates to current dimensions, restores captured rendition, and cancels delayed wrap. The stored coordinates are not normalized during resize, so later growth can recover the original saved coordinates. Full reset clears both slots. `?47` selection preserves slots. `?1047 h` recreates Alternate and clears its slot; `?1047 l` selects Primary without restore. No parser dispatch, `?1049`, DECSC/DECRC, scrollback, reflow, renderer, PTY, or input work exists.

## Validation

Focused saved-cursor coverage verifies basic/repeated save and restore, no-op uninitialized restore, screen isolation, `?47` preservation, `?1047` Alternate-slot reset/exit behavior, resize clamping/growth, reset, rendition capture, delayed-wrap exclusion, and preservation of cells/margins/tabs/modes/replies. Full workspace format, tests, doctests (including ownership compile-fail tests), check, Clippy, GNU/MSVC target tests, Graphify structural refresh, and `git diff --check` passed. No repository-specific static/security/dependency-direction tool is configured beyond the Cargo/CI gates; no dependency was added. Graphify semantic enrichment and relabeling were unavailable without an LLM API key; the code-only structural refresh completed.

## Next

Add the narrow DEC `?1049` parser/state semantics using this saved-cursor foundation, without scrollback or reflow.
