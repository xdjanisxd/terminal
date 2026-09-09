# Session Handoff

## Current state

M1 terminal-core foundation is complete locally.

Implemented:

* bounded screen/cell/cursor model
* terminal and input mode models
* `TerminalState` semantic boundary
* printable ASCII terminal semantics
* CR, LF, BS
* delayed auto-wrap and bounded in-screen scrolling
* incremental project-owned `TerminalParser`
* private `vte` integration
* chunk-safe parser state and resumable semantic errors

`terminal-core` currently depends only on `vte`, with default features disabled.

## Validation

Local required Cargo gates pass on `x86_64-pc-windows-gnu`.

Current terminal-core coverage includes 84 unit/integration tests plus ownership doctests.

Remote CI for the current M1 branch still needs to pass on all six configured platform runners before merge.

## Important limitations

* Printable content is currently ASCII U+0020..U+007E only.
* CSI/SGR cursor and erase semantics are not implemented yet.
* Unicode combining/wide-cell behavior is deferred.
* No PTY, renderer, scrollback, alternate screen, or terminal replies exist yet.
* Resize still uses top-left preservation with no line reflow.

See:

* ADR-0004 for parser ownership
* `docs/ARCHITECTURE.md` for terminal-core boundaries
* `docs/ROADMAP.md` for milestone state

## Next

1. Run authenticated cross-platform CI for the M1 branch.
2. Merge after all six jobs pass.
3. Begin M2 with typed cursor and erase semantics through `TerminalState`.

