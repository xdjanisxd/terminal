# Current State

## Working

- Valid Rust workspace, MIT metadata, and required repository knowledge system
- Cross-platform GitHub Actions gates verified on Windows, Linux, and macOS for x86_64 and ARM64
- Project-owned `terminal-core` models for bounded dimensions, cells, typed colors and rendition attributes, row-major screen storage, and a grid-owned bounded cursor
- Zero-based checked cell access, default-cell clearing, and top-left-preserving resize with cursor clamping
- Typed terminal-global modes for cursor visibility, auto-wrap, and insert/replace behavior
- Separate typed input-related state for normal/application cursor keys
- Parser-independent `TerminalState` owning one active `ScreenGrid`, current rendition, `TerminalModes`, `InputModes`, and bounded mutable horizontal tab stops
- Controlled state operations for cursor movement, horizontal tabs and tab-stop mutation, current rendition, clearing, resizing, supported mode changes, and a project-owned reset
- Project-owned typed cursor movement and inclusive erase-region operations, with all absolute and relative movement clamped to the active screen
- Parser-independent printable-character, carriage-return, line-feed, backspace, horizontal-tab, and typed rendition operations routed through `TerminalState`
- Delayed right-margin wrapping, auto-wrap suppression, fixed-screen bottom scrolling, and fixed-width insert/replace output behavior
- Incremental project-owned `TerminalParser` adapter encapsulating `vte` and preserving parser state across input chunks
- Parser routing for printable input, CR, LF, BS, HT, HTS, TBC, CUU/CUD/CUF/CUB, CUP/HVP, CHA/VPA, ED, EL, IRM, DECAWM, DECTCEM, and the basic 16-color/style SGR subset; unsupported parser actions and parameters are ignored without approximation
- Bounded, resumable semantic-error reporting with exact consumed-byte counts
- Bounded 1,024-byte OSC parser storage by compiling `vte` without its default `std` feature
- One hundred thirty-four `terminal-core` unit/integration tests plus three compile-fail ownership tests

## Partial

- `TerminalState::reset` restores the current model to initial state at existing dimensions; it is explicitly not DECSTR or RIS
- `Cell` stores one Unicode scalar plus foreground/background colors and the narrow bold, italic, underline, and inverse style set; combining characters, wide-cell continuations, additional styles, and grapheme behavior are not modeled yet
- Printable output accepts only single-cell ASCII space through tilde and snapshots current rendition attributes; all other Unicode remains explicitly deferred
- The parser recognizes broader VTE syntax incrementally, but only printable input, CR, LF, BS, horizontal-tab operations, the supported cursor/erase CSI subset, IRM/DECAWM/DECTCEM, and the documented basic SGR subset currently have terminal semantics
- Resize preserves the top-left rectangular intersection and does not reflow text; final terminal resize/reflow semantics remain future compatibility work
- Public branding and minimum operating-system versions remain intentionally deferred as documented in ADR-0008

## Missing

- Remaining CSI/SGR color and style semantics and modes, OSC/DCS semantics, terminal replies, scrolling margins, saved cursor state, origin mode, and alternate screens
- PTY, renderer, input encoding, configuration, workspace, and platform implementations
- Scrollback and compatibility/performance baselines

## Known issues

- The local Git Bash environment lacks the MSVC build tools required to link default-target tests; linked local validation uses the installed Windows GNU target
