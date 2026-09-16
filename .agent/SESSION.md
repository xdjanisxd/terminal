# Session Handoff

## Current state

The narrow M2 DEC private application cursor-key mode (DECCKM) slice is complete on `feat/m2-terminal-compatibility`.

`TerminalParser` routes `CSI ? 1 h` and `CSI ? 1 l` through `TerminalState::set_cursor_key_mode` to the existing project-owned `InputModes`/`CursorKeyMode` state. Default and reset remain `Normal`; resize preserves the mode. Private parameters retain the existing ordered independent dispatch, so unsupported neighbors do not block parameter 1. Standard CSI mode 1, unknown private modes, malformed subparameters, and incomplete private CSI remain safe no-ops.

The change is terminal mode state only. No keyboard/input byte encoder, SS3/CSI arrow-key generation, PTY, renderer, workspace, public API, or dependency was added; use by input encoding remains deferred to M5.

## Validation

Local Windows GNU validation passed:

* `cargo fmt --all -- --check`
* `cargo test --workspace --all-targets` (245 unit/integration tests)
* `cargo test --workspace --doc` (four compile-fail ownership doctests)
* `cargo check --workspace --all-targets`
* `cargo clippy --workspace --all-targets -- -D warnings`
* `git diff --check`
* dependency/static scans (no PTY, async runtime, event bus, renderer dependency, or unsafe Rust)

Structural Graphify was refreshed code-only to 736 nodes, 1,011 edges, and 79 communities. Targeted architecture checks confirm `TerminalParser` reaches `CursorKeyMode` only through `TerminalState::set_cursor_key_mode`, with no input, PTY, renderer, or dependency coupling. Semantic enrichment remains unavailable because no supported LLM API key is configured. An independent read-only review found no blocking or high-severity issue; its suggested combined foreground/background CSI coverage was added and validated.

## Important limitations

* DECCKM is parser-selectable mode state only; actual cursor-key byte encoding remains deferred to M5.
* Unicode combining/wide cells, origin mode, IL/DL, alternate screens, scrollback, PTY/session integration, and renderer behavior remain deferred.

## Roadmap audit

A complete M0–M2 roadmap consistency audit found all recorded completed milestone claims supported by current implementation, tests, architecture boundaries, and retained CI evidence. The earliest retained authenticated GitHub Actions CI run (`34327139362`) completed successfully on all six configured native OS/architecture jobs. The M2 parent compatibility item remains intentionally open; combining/wide-cell behavior, alternate screens/scrollback, and representative shell/TUI validation remain absent/deferred.

## Next

Select one narrow M2 child within “Implement and test remaining commonly used scroll, SGR, mode, and reply behavior”: add IL/DL as bounded active scrolling-region operations. Do not advance to Unicode/wide cells, alternate screens/scrollback, or M3 PTY work.