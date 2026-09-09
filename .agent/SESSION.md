# Session Handoff

## Changed

- Added public project-owned `TerminalParser` and `TerminalParserError` types in `terminal-core`.
- `TerminalParser` privately owns `vte::Parser` state and preserves it across every `advance` call; no `vte` type appears in the public terminal-core API.
- Added a private `vte::Perform` implementation that routes printable characters, CR, LF, and BS only through `TerminalState` semantic methods.
- CSI, ESC dispatch, OSC, DCS, and all other executed controls are intentionally ignored without approximating behavior.
- Added resumable semantic-error handling: parsing stops before callbacks following the first rejected semantic action can mutate state, and `TerminalParserError` reports the chunk-relative consumed-byte count plus the underlying project-owned `PrintError`.
- Added `vte` 0.15.0 only to `terminal-core`, with default features disabled. Its OSC storage is therefore a fixed 1,024-byte `ArrayVec` rather than an unbounded `Vec`.
- Added 20 parser integration tests covering printable/control routing, successive chunks, every split point, every fixed chunk size, byte-at-a-time input, persistent CSI and UTF-8 parser state, unsupported CSI/OSC/DCS/controls, malformed and incomplete escapes, oversized OSC input, unsupported Unicode, resumable semantic errors, and malformed partial UTF-8 callback termination.
- Marked both remaining M1 roadmap acceptance items complete. No ADR was needed because ADR-0004 already selects this boundary and dependency.

## Validation

- `cargo fmt --all -- --check` passed with `CARGO_BUILD_TARGET=x86_64-pc-windows-gnu`.
- `cargo check --workspace --all-targets` passed.
- `cargo test --workspace --all-targets` passed: 84 unit/integration tests.
- `cargo test --workspace --doc` passed: three compile-fail ownership doctests.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `cargo doc -p terminal-core --no-deps` passed; generated public `TerminalParser` documentation contains no `vte` type signature.
- `cargo tree -p terminal-core -e features` contains `vte 0.15.0`, `arrayvec 0.7.8`, and `memchr 2.8.3`, with no optional `vte` features enabled.
- `cargo tree --workspace --invert vte` confirmed `terminal-core` is the only direct `vte` consumer.
- `git diff --check` passed.
- Independent re-review confirmed the malformed partial UTF-8 error-termination fix and found no remaining blocker.
- Only Windows GNU and Windows MSVC Rust targets are installed locally; linked tests use GNU because local Git Bash lacks the MSVC linker. Cross-platform behavior still depends on configured Windows, Linux, and macOS CI.

## Decisions

- The public parser API accepts raw byte slices and a mutable `TerminalState`; `vte::Parser`, `vte::Perform`, parameters, and dispatch types remain private implementation details.
- Input is advanced one byte at a time through the incremental `vte` parser. This makes first-error consumption exact and resumable without allocating per byte; parser state still spans chunks.
- Error storage is bounded to one error because `Perform::terminated` stops parser progress before callbacks following the first semantic failure can mutate state. The reported offset identifies the untouched suffix and can be zero when malformed partial UTF-8 must surface a replacement-character error before reprocessing the current byte.
- Unsupported parser actions are consumed as syntax but have no terminal semantic effect. Once a complete unsupported sequence ends, following printable input is handled normally.
- `vte` default features stay disabled to preserve bounded OSC parser state. Adding the `ansi` or `std` feature requires a concrete need and a resource-bound review.
- `ROADMAP.md` now marks M1 complete because incremental parsing, printable input, arbitrary chunking, malformed/incomplete input, and bounded parser state all have direct tests.

## Known limitations

- Printable semantics still accept only ASCII space through tilde; other Unicode becomes an explicit resumable parser error.
- CSI, SGR, cursor/erase escape sequences, DECSET/DECRST, ESC dispatch, OSC, DCS, terminal replies, and unsupported C0/C1 controls have no semantic effect.
- Parser errors require callers to use `bytes_consumed` when resuming the rejected chunk's unconsumed suffix; zero-byte progress is valid after a malformed UTF-8 prefix from an earlier chunk.
- No PTY, renderer, scrollback, or alternate screen was added.
- This feature branch has not run remote cross-platform CI.

## Next recommended task

Run authenticated CI for the completed M1 branch and merge only after all six configured runners pass. After M1 closes, start M2 with a narrow typed cursor/erase semantic slice through `TerminalState`, keeping numeric protocol identifiers inside the parser adapter.
