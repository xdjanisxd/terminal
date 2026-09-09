# Session Handoff

## Changed

- Added parser-independent `TerminalState::print_character`, `carriage_return`, `line_feed`, and `backspace` semantic operations.
- Printable output accepts only single-cell ASCII space through tilde, rejects controls and unsupported Unicode explicitly, writes default attributes, and supports current fixed-width replace and insert modes.
- Added private delayed-wrap state: printing in the final column leaves the cursor there, and the next printable character wraps before writing when auto-wrap is enabled.
- Auto-wrap-disabled output remains at and replaces the final column. Disabling auto-wrap cancels a pending wrap.
- Line feed preserves the cursor column and delayed-wrap state; at the bottom row it scrolls the active fixed-size screen up by one row and blanks the new bottom row without scrollback.
- Carriage return moves to column zero. Backspace moves left without erasing or reverse wrapping.
- Successful cursor positioning, relative cursor movement, carriage return, screen clear, resize, reset, and disabling auto-wrap cancel delayed wrap where applicable.
- Added crate-private grid helpers for fixed-row insertion and one-row in-screen scrolling; public callers still receive no mutable path into `TerminalState`-owned state.
- Horizontal tab was explicitly deferred because correct terminal behavior requires mutable tab-stop state, including default stops and reset/resize behavior.
- No parser, `vte`, dependency, PTY, renderer, scrollback, alternate screen, CSI/SGR handling, or event framework was added.

## Validation

- `cargo fmt --all -- --check` passed with `CARGO_BUILD_TARGET=x86_64-pc-windows-gnu`.
- `cargo check --workspace --all-targets` passed with the Windows GNU target.
- `cargo test --workspace --all-targets` passed: 64 unit/integration tests.
- `cargo test --workspace --doc` passed: three compile-fail ownership doctests.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `cargo tree -p terminal-core` confirmed `terminal-core` has zero dependencies.
- `git diff --check` passed.
- Independent re-review found no remaining blocker after unsupported width-sensitive Unicode was changed from one-cell approximation to explicit rejection.
- Only Windows GNU and Windows MSVC Rust targets are installed locally; linked tests use GNU because local Git Bash lacks the MSVC linker. Cross-platform behavior still depends on configured Windows, Linux, and macOS CI.

## Decisions

- Delayed wrapping is required to distinguish a character just written in the final column from the next printable character that triggers wrapping.
- Bottom-row line feed performs only full-screen in-screen scrolling until scrolling margins and scrollback exist.
- Insert mode is implemented only for accepted one-cell ASCII by shifting the row right and dropping its final cell.
- Control characters cannot enter cells through the semantic print API; adapters must call dedicated semantic control operations. Non-ASCII input returns an explicit unsupported-character error until Unicode width is modeled.
- `ROADMAP.md` was unchanged because the existing M1 parser and parser-testing acceptance items are not complete.

## Known limitations

- Printable semantics currently support ASCII space through tilde only; combining characters, wide characters, other Unicode, active rendition attributes, and grapheme behavior are deferred.
- Horizontal tab is deferred until mutable tab stops are modeled.
- There is no line-feed/new-line mode, reverse wrap, scrolling region, scrollback, parser, or parser-specific behavior.
- The local feature branch has not run remote cross-platform CI.

## Next recommended task

Integrate a minimal incremental `vte` adapter behind a project-owned interface, translating printable input and basic executed controls into these `TerminalState` operations. Test arbitrary byte chunk boundaries and keep CSI, SGR, PTY, rendering, and broader compatibility behavior out of that slice.
