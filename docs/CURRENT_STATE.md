# Current State

## Working

- Valid Rust workspace, MIT metadata, and required repository knowledge system
- Cross-platform GitHub Actions gates verified on Windows, Linux, and macOS for x86_64 and ARM64
- Project-owned `terminal-core` models for bounded dimensions, cells, typed colors and rendition attributes, row-major screen storage, and a grid-owned bounded cursor
- Zero-based checked cell access, default-cell clearing, bounded inclusive region-scroll primitives, and top-left-preserving resize with cursor clamping
- Typed terminal-global modes for cursor visibility, auto-wrap, and insert/replace behavior
- Separate typed input-related state for normal/application cursor keys
- Parser-independent `TerminalState` owning one active `ScreenGrid`, current rendition, `TerminalModes`, `InputModes`, bounded mutable horizontal tab stops, bounded typed vertical scrolling margins, and a fixed-capacity FIFO of project-owned pending terminal replies for primary DA1 and captured ANSI CPR responses
- Controlled state operations for cursor movement, margin-aware SU/SD, IND/RI, and NEL, horizontal tabs and tab-stop mutation, vertical scrolling-margin updates, current rendition, bounded DA1 and absolute ANSI CPR reply generation with FIFO consumption, clearing, resizing, supported mode changes, and a project-owned reset
- Project-owned typed cursor movement and inclusive erase-region operations, with all absolute and relative movement clamped to the active screen
- Parser-independent printable-character, carriage-return, line-feed, index, reverse-index, next-line, backspace, horizontal-tab, region-aware-scroll, typed scrolling-margin, and typed rendition operations routed through `TerminalState`
- Delayed right-margin wrapping, auto-wrap suppression, bounded region-aware SU/SD, IND/RI, and NEL scrolling, fixed-screen bottom scrolling, and fixed-width insert/replace output behavior
- Incremental project-owned `TerminalParser` adapter encapsulating `vte` and preserving parser state across input chunks
- Parser routing for printable input, CR, LF, BS, HT, HTS, IND, RI, NEL, primary DA1 queries `CSI c`/`CSI 0 c`, ANSI cursor-position DSR `CSI 6 n`, TBC, CUU/CUD/CUF/CUB, CUP/HVP, CHA/VPA, ED, EL, region-aware SU/SD, DECSTBM, IRM, DECAWM, DECTCEM, and SGR styles, defaults, ANSI 16 colors, and semicolon-form indexed foreground/background colors; unsupported parser actions and parameters are ignored without approximation
- DECSTBM omitted/zero defaults, one-based validation before zero-based conversion, cursor homing and delayed-wrap cancellation on success, and atomic no-op behavior for invalid ranges, extra parameters, and subparameter forms
- Bounded grouped handling for extended-color SGR: indexed selectors consume one index, unsupported truecolor consumes three payload parameters, and unknown selectors consume the remaining CSI parameters so payload cannot leak into unrelated SGR
- Bounded, resumable semantic-error reporting with exact consumed-byte counts
- Bounded 1,024-byte OSC parser storage by compiling `vte` without its default `std` feature
- Two hundred thirty-four `terminal-core` unit/integration tests plus four compile-fail ownership tests

## Partial

- `TerminalState::reset` restores the screen model to initial state at existing dimensions, including full-screen vertical margins, while preserving already-generated pending replies; it is explicitly not DECSTR or RIS
- `Cell` stores one Unicode scalar plus foreground/background colors and the narrow bold, italic, underline, and inverse style set; combining characters, wide-cell continuations, additional styles, and grapheme behavior are not modeled yet
- Printable output accepts only single-cell ASCII space through tilde and snapshots current rendition attributes; all other Unicode remains explicitly deferred
- The parser recognizes broader VTE syntax incrementally, but only printable input, CR, LF, BS, HT, HTS, IND, RI, NEL, primary DA1, ANSI DSR cursor-position queries, the supported cursor/erase/region-aware-scroll/DECSTBM CSI subset, IRM/DECAWM/DECTCEM, and the documented style, default, ANSI 16-color, and semicolon-form indexed-color SGR subset currently have terminal semantics
- Resize preserves the top-left rectangular intersection, resets vertical scrolling margins to the full new screen height, and does not reflow text; final terminal resize/reflow semantics remain future compatibility work
- Public branding and minimum operating-system versions remain intentionally deferred as documented in ADR-0008

## Missing

- Remaining CSI/SGR style semantics and modes, truecolor and colon-form color semantics, OSC/DCS semantics, secondary/tertiary DA replies, other DSR/CPR forms, saved cursor state, origin mode, and alternate screens
- PTY, renderer, input encoding, configuration, workspace, and platform implementations
- Scrollback and compatibility/performance baselines

## Known issues

- The local Git Bash environment lacks the MSVC build tools required to link default-target tests; linked local validation uses the installed Windows GNU target
