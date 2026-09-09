# Tasks

## Next

- [ ] Add the next M1 slice: define terminal modes and their explicit state transitions in `terminal-core`; keep the slice std-only and parser-independent.

## Later

- [ ] Introduce a project-owned `TerminalState` facade and integrate incremental `vte` parsing only after its model contract and tests are defined.
- [ ] Extend cell content for combining and wide-character invariants during the Unicode compatibility slice; do not assume the current single-`char` representation is final.
- [ ] Decide resize reflow behavior during the compatibility milestone; the current top-left preservation rule does not perform terminal line reflow.
- [ ] Determine minimum OS versions from pinned stack requirements and real platform validation when those dependencies are introduced; record the result in a new ADR.

## Deferred by scope

- Plugin system, embedded AI, SSH manager, IDE features, runtime session restoration, and large scripting/UI frameworks remain outside V1.
