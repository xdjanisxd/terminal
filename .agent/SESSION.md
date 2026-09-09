# Session Handoff

## Current state

The first M2 terminal-compatibility slice is complete locally on `feat/m2-cursor-erase-semantics`.

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
* typed relative and absolute cursor movement through `TerminalState`
* typed display/line erase directions through `TerminalState`
* private CSI translation for CUU/CUD/CUF/CUB, CUP/HVP, CHA/VPA, ED, and EL

`terminal-core` currently depends only on `vte`, with default features disabled.

## Validation

Local required Cargo gates pass on `x86_64-pc-windows-gnu`.

Current terminal-core coverage includes 94 unit/integration tests plus ownership doctests.

Remote CI for the current M2 branch still needs to pass on all six configured platform runners before merge.

## Important limitations

* Printable content is currently ASCII U+0020..U+007E only.
* CSI support remains intentionally limited to the cursor and erase sequences in this slice; SGR, scrolling, mode dispatch, and replies are not implemented yet.
* Unicode combining/wide-cell behavior is deferred.
* No PTY, renderer, scrollback, alternate screen, or terminal replies exist yet.
* Resize still uses top-left preservation with no line reflow.

See:

* ADR-0004 for parser ownership
* `docs/ARCHITECTURE.md` for terminal-core boundaries
* `docs/ROADMAP.md` for milestone state

## Next

1. Run authenticated cross-platform CI for the M2 branch.
2. Merge after all six jobs pass.
3. Add a narrow mode-dispatch slice for the existing insert/replace, auto-wrap, and cursor-visibility model without adding speculative modes.

