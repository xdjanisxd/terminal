# Session Handoff

## Current state

The narrow M2 ANSI DSR/CPR slice is complete.

`TerminalParser` recognizes only standard `CSI 6 n` and delegates to `TerminalState::request_cursor_position_report()`. `TerminalState` captures its current absolute zero-based cursor coordinates as one-based `u16` values at query time and queues `TerminalReply::CursorPosition` through the existing fixed-capacity `PendingReplies` FIFO. `TerminalReply` now provides fixed 12-byte-capacity wire encoding for DA1 and dynamic CPR values; no heap allocation, extra queue, output abstraction, or PTY/session dependency was added.

CPR is `ESC [ <row> ; <column> R`, remains absolute with DECSTBM margins active, preserves terminal state and delayed wrap, retains captured coordinates across reset/resize, and is FIFO-interleaved with DA1. Unsupported DSR parameters and private DSR forms do not reply.

## Validation

Local Windows GNU validation passed:

* `cargo fmt --all -- --check`
* `cargo test --workspace --all-targets` (234 unit/integration tests)
* `cargo test --workspace --doc` (four compile-fail ownership doctests)
* `cargo check --workspace --all-targets`
* `cargo clippy --workspace --all-targets -- -D warnings`
* `git diff --check`
* source-only dependency/static scans (no PTY, async runtime, event bus, or unsafe Rust)

Structural Graphify was refreshed code-only to 712 nodes, 987 edges, and 75 communities. Targeted paths confirm `SemanticPerformer::csi_dispatch` reaches `TerminalState::request_cursor_position_report`, which reaches the existing `PendingReplies`. Semantic enrichment remains unavailable because no supported LLM API key is configured.

## Important limitations

* Only ANSI `CSI 6 n` is implemented; operating-status DSR, private DSR/CPR, and origin mode remain deferred.
* CPR reports absolute screen coordinates; DECSTBM does not change the coordinate origin.
* Unicode combining/wide cells, alternate screens, scrollback, PTY/session integration, renderer behavior, IL/DL, truecolor, and further compatibility behavior remain deferred.

## Next

Reassess the still-open M2 item “Implement and test remaining commonly used scroll, SGR, mode, and reply behavior” and select one high-value narrow compatibility slice. Do not advance to Unicode/wide cells, alternate screens/scrollback, or M3 PTY work.