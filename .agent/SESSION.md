# Session Handoff

## Current state

The margin-aware IND/RI M2 slice is complete.

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
* private CSI mode translation for standard IRM and DEC-private DECAWM/DECTCEM
* parser-controlled insert/replace output and delayed-wrap cancellation through existing `TerminalState` semantics
* terminal-owned mutable horizontal tab stops with conventional eight-column defaults
* bounded HT movement, HTS, and current/all TBC through `TerminalState`
* explicit tab-stop resize and reset behavior
* typed current rendition for intensity, italic, underline, inverse, foreground, and background
* new printable cells snapshot current rendition without retroactively changing existing cells
* SGR reset, style set/clear, ANSI standard/bright colors, and per-channel defaults through `TerminalState`
* ordered SGR parameter handling with bare/empty reset behavior and safe unsupported-parameter no-ops
* semicolon-form `38;5;n` and `48;5;n` indexed colors across the full 0..=255 range
* bounded logical extended-color groups that prevent malformed, truecolor, or unknown-selector payloads from leaking into unrelated SGR operations
* bounded full-screen scroll up/down through `TerminalState`, with counts clamped to visible height
* complete-cell movement with canonical default blank rows and preserved cursor, modes, rendition, and delayed wrap
* private CSI translation for SU/SD with omitted and zero counts normalized to one
* project-owned IND, RI, and NEL semantics with full-screen edge scrolling through existing bounded primitives
* direct ESC D/M/E parser translation through `TerminalState`
* delayed-wrap cancellation for IND, RI, and NEL while preserving modes, tab stops, and current rendition
* project-owned inclusive zero-based `VerticalScrollingMargins`, including safe one-row representation
* full-screen margin defaults on creation and reset, with full-new-height margins after grow or shrink resize
* atomic parser-independent margin replacement through `TerminalState`, including screen-origin cursor homing and delayed-wrap cancellation on success
* private DECSTBM translation with omitted/zero defaults, one-based validation before conversion, and safe no-ops for equal/reversed/out-of-bounds ranges, extra scalar parameters, and colon/subparameter groups
* bounded inclusive `ScreenGrid` region-scroll up/down primitives with count clamping, complete-cell movement, default exposed rows, and untouched outside rows
* margin-aware IND and RI movement and boundary scrolling through `TerminalState`, including one-row regions and bounded movement outside custom margins
* full-screen IND/RI compatibility plus unchanged full-screen SU/SD and NEL semantics

`terminal-core` currently depends only on `vte`, with default features disabled.

## Validation

Local required Cargo gates pass on `x86_64-pc-windows-gnu`.

Current terminal-core coverage includes 194 unit/integration tests plus four ownership doctests.

Structural Graphify data refreshed locally in code-only mode to 603 nodes and 815 edges across 58 communities. Semantic enrichment was unavailable because no supported LLM API key is configured.

The targeted Graphify, architecture, dependency-direction, static-security, direct diff, and independent delegated reviews found no issues.

Remote CI has not run for this unpushed slice and remains the merge gate.

## Important limitations

* Printable content is currently ASCII U+0020..U+007E only.
* CSI mode support is intentionally limited to standard IRM and DEC-private DECAWM/DECTCEM; all other mode identifiers remain no-ops.
* Horizontal tabs are cursor-only: they do not write tab characters or spaces into cells. HT cancels delayed wrap and remains on the final column when no later stop exists.
* SGR supports reset, bold, italic, underline, inverse, ANSI 16 colors, semicolon-form indexed foreground/background colors, and per-channel defaults. Truecolor, colon color forms, and additional styles remain deferred.
* IND and RI consult the active vertical scrolling margins; SU, SD, and NEL remain intentionally full-screen. Region-aware SU/SD and NEL, origin mode, insert/delete lines, alternate-screen behavior, and history remain deferred.
* Unicode combining/wide-cell behavior is deferred.
* No PTY, renderer, scrollback, alternate screen, or terminal replies exist yet.
* Resize still uses top-left preservation with no line reflow.

See:

* ADR-0004 for parser ownership
* `docs/ARCHITECTURE.md` for terminal-core boundaries
* `docs/ROADMAP.md` for milestone state

## Next

1. Make explicit SU and SD region-aware through the bounded inclusive grid region-scroll primitive; keep NEL, origin mode, IL/DL, scrollback, and alternate screens separate.

