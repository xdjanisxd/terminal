# Current State

## Working

- Valid Rust workspace, MIT metadata, and required repository knowledge system
- Cross-platform GitHub Actions gates verified on Windows, Linux, and macOS for x86_64 and ARM64
- Dependency-free `terminal-core` models for bounded dimensions, cells, colors/attributes, row-major screen storage, and a grid-owned bounded cursor
- Zero-based checked cell access, default-cell clearing, and top-left-preserving resize with cursor clamping
- Nineteen public-API model tests covering construction, resource boundaries, indexing, access, cursor bounds, clearing, and resize behavior

## Partial

- `Cell` stores one Unicode scalar and foreground/background colors; combining characters, wide-cell continuations, style attributes, and grapheme behavior are not modeled yet
- Resize preserves the top-left rectangular intersection and does not reflow text; final terminal resize/reflow semantics remain future compatibility work
- Public branding and minimum operating-system versions remain intentionally deferred as documented in ADR-0008

## Missing

- Terminal modes, `TerminalState`, escape parsing, `vte`, scrollback, selection, and terminal replies
- PTY, renderer, input, configuration, workspace, and platform implementations
- Compatibility and performance baselines

## Known issues

- The local Git Bash environment lacks the MSVC build tools required to link default-target tests; linked local validation uses the installed Windows GNU target
