# Tasks

## Next

- [ ] Define a narrow SGR slice around project-owned current rendition state and the existing `CellColor` model; avoid adding unrelated style flags or a generic command framework.

## Later

- [x] Start M2 with a narrow typed cursor/erase semantic slice through `TerminalState`; keep protocol numeric identifiers inside the parser adapter.
- [x] Connect standard IRM and private DECAWM/DECTCEM parser dispatch to existing `TerminalState` setters; keep unsupported modes as no-ops and protocol identifiers private.
- [x] Add terminal-owned mutable horizontal tab stops with HT, HTS, current/all TBC, bounded movement, and explicit resize/reset behavior.
- [ ] Add origin mode only with scrolling-region state, cursor homing, saved-state behavior, and primary/alternate-screen semantics coordinated atomically by `TerminalState`.
- [ ] Defer line-feed/new-line mode and application keypad mode until their behavior is required by parser/input milestones; do not grow a speculative mode catalog.
- [ ] Extend cell content for combining and wide-character invariants during the Unicode compatibility slice; do not assume the current single-`char` representation is final.
- [ ] Decide resize reflow behavior during the compatibility milestone; the current top-left preservation rule does not perform terminal line reflow.
- [ ] Determine minimum OS versions from pinned stack requirements and real platform validation when those dependencies are introduced; record the result in a new ADR.

## Deferred by scope

- Plugin system, embedded AI, SSH manager, IDE features, runtime session restoration, and large scripting/UI frameworks remain outside V1.
