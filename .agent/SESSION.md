# Session Handoff

## Current state

`feat/m2-dec-alt-screen-1049` adds DEC private `CSI ? 1049 h`/`l` parser dispatch through dedicated `TerminalState` operations. From Primary, `?1049 h` saves the existing Primary cursor/rendition slot, resets Alternate with the established `?1047` screen-local policy, and selects Alternate. When Alternate is already active, repeated `?1049 h` is an idempotent no-op, preserving both Alternate contents and the saved Primary state. `?1049 l` selects Primary and uses the existing Primary saved-cursor restore, including rendition restoration, restore-time coordinate clamping, and delayed-wrap cancellation.

`?47` remains pure non-clearing selection. `?1047` continues to reset/select Alternate on entry and select Primary without restore on exit. `?1049` does not add saved fields or change cells, margins, tabs, terminal/input modes, replies, renderer, PTY, input, scrollback, or reflow behavior. Full reset recreates both screens, clears saved slots, selects Primary, and makes a later `?1049 l` safe.

## Validation

Focused `?1049` tests cover entry/exit, rendition restoration, repeated entry/exit, `?47`/`?1047` distinction, stale Alternate wide/combining clearing, Primary wide/combining preservation, global-state isolation, parser chunking/malformed forms, resize clamping, and reset. Full workspace format, tests, doctests (including ownership compile-fail tests), check, Clippy, GNU/MSVC target tests, Graphify structural refresh, and `git diff --check` passed. No repository-specific static/security/dependency-direction tool is configured beyond the Cargo/CI gates; no dependency was added. Graphify semantic enrichment and relabeling were unavailable without an LLM API key; the code-only structural refresh completed.

## Next

Add a bounded primary-screen scrollback foundation that remains separate from reflow and renderer behavior.
