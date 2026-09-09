# Tasks

## Next

- [ ] Integrate a minimal incremental `vte` adapter behind a project-owned interface and translate printable input plus basic executed controls into `TerminalState` operations. Test arbitrary input chunking and keep CSI, SGR, PTY, rendering, and broader compatibility behavior out of that slice.

## Later

- [ ] Add bounded malformed/incomplete parser-state tests before expanding escape-sequence coverage.
- [ ] Add origin mode only with scrolling-region state, cursor homing, saved-state behavior, and primary/alternate-screen semantics coordinated atomically by `TerminalState`.
- [ ] Defer line-feed/new-line mode and application keypad mode until their behavior is required by parser/input milestones; do not grow a speculative mode catalog.
- [ ] Add horizontal-tab behavior only with terminal-owned mutable tab stops, including default stops and resize/reset behavior; do not approximate it as permanently fixed eight-column movement.
- [ ] Extend cell content for combining and wide-character invariants during the Unicode compatibility slice; do not assume the current single-`char` representation is final.
- [ ] Decide resize reflow behavior during the compatibility milestone; the current top-left preservation rule does not perform terminal line reflow.
- [ ] Determine minimum OS versions from pinned stack requirements and real platform validation when those dependencies are introduced; record the result in a new ADR.

## Deferred by scope

- Plugin system, embedded AI, SSH manager, IDE features, runtime session restoration, and large scripting/UI frameworks remain outside V1.
