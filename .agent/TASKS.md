# Tasks

## Next

- [ ] Add a narrow parser mode-dispatch slice only for already-modeled insert/replace, auto-wrap, and cursor-visibility state; keep numeric identifiers private and do not grow the mode catalog speculatively.

## Later

- [x] Start M2 with a narrow typed cursor/erase semantic slice through `TerminalState`; keep protocol numeric identifiers inside the parser adapter.
- [ ] Add origin mode only with scrolling-region state, cursor homing, saved-state behavior, and primary/alternate-screen semantics coordinated atomically by `TerminalState`.
- [ ] Defer line-feed/new-line mode and application keypad mode until their behavior is required by parser/input milestones; do not grow a speculative mode catalog.
- [ ] Add horizontal-tab behavior only with terminal-owned mutable tab stops, including default stops and resize/reset behavior; do not approximate it as permanently fixed eight-column movement.
- [ ] Extend cell content for combining and wide-character invariants during the Unicode compatibility slice; do not assume the current single-`char` representation is final.
- [ ] Decide resize reflow behavior during the compatibility milestone; the current top-left preservation rule does not perform terminal line reflow.
- [ ] Determine minimum OS versions from pinned stack requirements and real platform validation when those dependencies are introduced; record the result in a new ADR.

## Deferred by scope

- Plugin system, embedded AI, SSH manager, IDE features, runtime session restoration, and large scripting/UI frameworks remain outside V1.
