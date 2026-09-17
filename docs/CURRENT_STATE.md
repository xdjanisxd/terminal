# Current State

## Working

- Valid Rust workspace, MIT metadata, and required repository knowledge system
- Cross-platform GitHub Actions gates verified on Windows, Linux, and macOS for x86_64 and ARM64
- Project-owned `terminal-core` models for bounded dimensions, cells with typed single/wide-lead/wide-continuation occupancy, bounded ordered width-zero attachment payloads on printable bases, typed colors and rendition attributes including normal/bold/faint intensity, row-major screen storage, and a grid-owned bounded cursor
- Zero-based checked cell access, default-cell clearing, bounded inclusive region-scroll primitives, and top-left-preserving resize with cursor clamping
- Typed terminal-global modes for cursor visibility, auto-wrap, and insert/replace behavior
- Separate typed input-related state for normal/application cursor keys
- Parser-independent `TerminalState` owns one active `ScreenGrid`, current rendition, terminal/input modes, bounded tab stops and scrolling margins, and a fixed-capacity FIFO of project-owned pending replies for DA1, DA2, ANSI DSR status, and captured ANSI CPR responses
- Controlled state operations for bounded CUU/CUD/CUF/CUB, CNL/CPL, CHA, VPA, CUP/HVP cursor movement; margin-aware SU/SD, IND/RI, and NEL; horizontal tabs and tab-stop mutation; vertical scrolling-margin updates; current rendition; bounded DA1; fixed ANSI DSR status; and absolute ANSI CPR reply generation with FIFO consumption, clearing, resizing, supported mode changes, and a project-owned reset
- Project-owned typed cursor movement and inclusive erase-region operations, with all absolute and relative movement clamped to the active screen
- `TerminalState` owns independent primary and alternate project screen buffers with atomic typed switching; each buffer retains grid/cursor, scrolling margins, and delayed-wrap state, while rendition, tabs, terminal/input modes, and replies remain shared.
- Parser-independent printable-character, carriage-return, line-feed, index, reverse-index, next-line, bounded cursor-next/previous-line, backspace, horizontal-tab, region-aware-scroll, typed scrolling-margin, and typed rendition operations routed through `TerminalState`
- Delayed right-margin wrapping, auto-wrap suppression, bounded region-aware SU/SD, IND/RI, and NEL scrolling, fixed-screen bottom scrolling, and fixed-width insert/replace output behavior
- Incremental project-owned `TerminalParser` adapter encapsulating `vte` and preserving parser state across input chunks
- Parser routing for printable input, CR, LF, BS, HT, HTS, IND, RI, NEL, primary DA1 queries `CSI c`/`CSI 0 c`, ANSI DSR status `CSI 5 n` and cursor-position `CSI 6 n` queries, TBC, CUU/CUD/CUF/CUB, CNL/CPL, CUP/HVP, CHA/VPA, bounded current-row ICH/DCH/ECH, ED, EL, cursor-relative IL/DL, region-aware SU/SD, DECSTBM, non-clearing DEC private alternate-screen mode 47 switching, IRM, private DECCKM/DECAWM/DECTCEM, and SGR reset, bold, faint, italic, underline, inverse, defaults, ANSI 16 colors, semicolon-form indexed foreground/background colors, and semicolon-form exact RGB foreground/background colors; unsupported parser actions and parameters are ignored without approximation
- IL/DL `CSI Ps L`/`CSI Ps M` use the cursor-to-bottom portion of the active scrolling region, leaving rows above the cursor and outside margins unchanged; blank lines use canonical default cells and delayed wrap is preserved
- Bounded current-row ICH `CSI Ps @` and DCH `CSI Ps P` use generic grid cell insertion/deletion primitives, shift complete cells within the current row with final-column clipping, fill canonical default blank cells, and cancel delayed wrap without moving the cursor
- Bounded current-row ECH `CSI Ps X` reuses the grid range-clear primitive to replace complete cells with canonical default blanks without shifting neighboring cells; it preserves cursor coordinates and cancels delayed wrap
- DECCKM `CSI ? 1 h`/`CSI ? 1 l` selects the project-owned application/normal cursor-key mode through `TerminalState`; byte encoding for cursor-key input remains deferred to M5
- DECSTBM omitted/zero defaults, one-based validation before zero-based conversion, cursor homing and delayed-wrap cancellation on success, and atomic no-op behavior for invalid ranges, extra parameters, and subparameter forms
- Bounded grouped handling for extended-color SGR: indexed selectors consume one index, semicolon-form truecolor consumes exactly three scalar components and validates all before mutation, and unknown selectors consume the remaining CSI parameters so payload cannot leak into unrelated SGR
- DA2 uses fixed `ESC [ > 0 ; 0 ; 0 c`: Pp=0 retains DA1's conservative VT100-class identity, Pv=0 avoids exposing package/release versions, and Pc=0 claims no optional hardware features
- Unicode combining and wide-cell invariant scope is complete: widths 0/1/2 are safely classified, width-zero scalars attach in order to a valid preceding base or are ignored without mutation, and central...[truncated]
- Three hundred twenty-three `terminal-core` unit/integration tests plus four compile-fail ownership tests

## Partial

- `TerminalState::reset` restores the screen model to initial state at existing dimensions, including full-screen vertical margins, while preserving already-generated pending replies; it is explicitly not DECSTR or RIS
- `Cell` stores one Unicode scalar, its rendition snapshot, and up to eight ordered width-zero scalars in a single or wide-leading cell; a distinct continuation cell stores no duplicate character, rendition, or combining payload. Width-zero output attaches only to the preceding logical printable base without moving the cursor or resolving delayed wrap; without a base it is ignored, and overflow returns `PrintError::CombiningMarkOverflow` without mutation. Grid mutation normalizes every row so each lead is immediately followed by one continuation and continuations never stand alone; grapheme segmentation, normalization, ZWJ handling, and renderer shaping remain deferred.
- Printable output supports width-one and width-two Unicode scalars through the project-owned `CellOccupancy` model and `unicode-width`; width-zero classification uses the narrow attachment policy above rather than claiming full Unicode grapheme correctness, while malformed UTF-8 replacement characters remain errors
- Faint is represented in terminal state and captured by cells, but visual dimming remains deferred with renderer implementation
- The parser recognizes broader VTE syntax incrementally, but only printable input, CR, LF, BS, HT, HTS, IND, RI, NEL, primary DA1, ANSI DSR cursor-position queries, the supported cursor/erase/region-aware-scroll/DECSTBM CSI subset, IRM/DECAWM/DECTCEM, and the documented style, default, ANSI 16-color, semicolon-form indexed-color, and semicolon-form truecolor SGR subsets currently have terminal semantics
- Resize preserves the top-left rectangular intersection, resets vertical scrolling margins to the full new screen height, and does not reflow text; final terminal resize/reflow semantics remain future compatibility work
- Public branding and minimum operating-system versions remain intentionally deferred as documented in ADR-0008

## Missing

- Remaining CSI/SGR style semantics and modes, colon-form color semantics, OSC/DCS semantics, secondary/tertiary DA replies, other DSR/CPR forms, saved cursor state, origin mode, and alternate screens
- PTY, renderer, input encoding, configuration, workspace, and platform implementations
- Scrollback and compatibility/performance baselines

## Known issues

- No local validation limitation is currently recorded: the x86_64-pc-windows-gnu and x86_64-pc-windows-msvc linked workspace suites passed for this slice.
