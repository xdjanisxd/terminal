# Session Handoff

## Current state

`feat/m2-bounded-scrollback-foundation` adds project-owned bounded scrollback only to Primary `ScreenState`. It retains up to 10,000 chronological complete `Vec<Cell>` rows; at capacity, the oldest row is evicted before the newest is kept. `TerminalState::scrollback_len` and `scrollback_row` provide immutable inspection only.

Capture occurs only before Primary whole-visible-screen upward scrolling: bottom-edge line-feed/IND, NEL via index, delayed-wrap output that invokes the same path, and deliberate full-screen `SU`. Restricted DECSTBM scrolling, IL/DL, RI/SD and every downward scroll, erase/edit operations, and all Alternate operations do not append. `?47`, `?1047`, and `?1049` preserve Primary history; Alternate has no hidden history. Full reset clears history.

Captured rows preserve exact `Cell` values, including attributes, combining payloads, and wide lead/continuation metadata. Resize leaves stored rows at their capture-time widths with no crop, padding, reflow, or rewrap; future viewport/render work must handle mixed historical widths. No viewport state, renderer behavior, PTY integration, shell/TUI validation, or reflow was added.

## Validation

Focused scrollback tests cover primary capture/order, capacity eviction, full-screen `SU`, restricted-region exclusion, IL/DL/downward exclusion, bottom-edge IND/NEL, auto-wrap, Alternate isolation, `?47`/`?1047`/`?1049` preservation, reset, resize, and wide/combining/rendition retention. Full workspace gates and cross-target tests remain required before final handoff. No dependency was added.

## Next

Audit the still-open primary/alternate-screen, resize, and bounded-scrollback parent to decide whether the next narrow blocker is resize/reflow policy, a read-only viewport model needed for validation, or another history invariant. Do not add renderer UI or move to representative shell/TUI validation until that parent is explicitly audited.
