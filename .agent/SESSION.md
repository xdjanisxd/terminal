# Session Handoff

## Current state

`feat/m2-dec-alt-screen-1047` adds narrow DEC private mode 1047 parser dispatch through `TerminalState`: `CSI ? 1047 h` resets the Alternate screen's grid/cursor, vertical margins, and delayed-wrap at current dimensions, then selects it. `CSI ? 1047 l` selects Primary without clearing either Primary or restoring saved state.

DEC `?47` remains pure non-clearing typed screen selection. The Primary buffer and all shared state (rendition, tabs, terminal/input modes, and pending replies) remain untouched by `?1047`. `?1049` remains unsupported. No saved cursor, scrollback, reflow, renderer, PTY, or input work exists.

## Validation

Focused `?1047` tests cover screen-local reset, repeated entry, exit idempotence, shared-state isolation, `?47` distinction, `?1049` isolation, wide/combining clearing, resize, and full reset regression. Full workspace format, tests, doctests, check, Clippy, GNU/MSVC target tests, repository static/dependency checks, Graphify refresh, and `git diff --check` passed. Independent review was unavailable because its delegated model failed to start.

## Next

Add a narrow saved-cursor foundation without `?1049` parser dispatch or scrollback, providing explicit state for later alternate-screen semantics.
