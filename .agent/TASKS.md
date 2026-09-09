# Tasks

## Next

- [ ] Add the next M1 slice: introduce a parser-independent `TerminalState` facade that owns the active screen, `TerminalModes`, and `InputModes`, and defines atomic semantic operations. Do not add `vte` in that slice.

## Later

- [ ] Integrate incremental `vte` parsing behind project-owned interfaces only after the `TerminalState` operation contract and tests are established.
- [ ] Add origin mode only with scrolling-region state, cursor homing, saved-state behavior, and primary/alternate-screen semantics coordinated atomically by `TerminalState`.
- [ ] Defer line-feed/new-line mode and application keypad mode until their behavior is required by the parser/input milestones; do not grow a speculative mode catalog.
- [ ] Extend cell content for combining and wide-character invariants during the Unicode compatibility slice; do not assume the current single-`char` representation is final.
- [ ] Decide resize reflow behavior during the compatibility milestone; the current top-left preservation rule does not perform terminal line reflow.
- [ ] Determine minimum OS versions from pinned stack requirements and real platform validation when those dependencies are introduced; record the result in a new ADR.

## Deferred by scope

- Plugin system, embedded AI, SSH manager, IDE features, runtime session restoration, and large scripting/UI frameworks remain outside V1.
