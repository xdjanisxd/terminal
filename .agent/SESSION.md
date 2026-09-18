# Session Handoff

## Current state

`feat/m3-bounded-pty-workers` starts from native-accepted PTY lifecycle coverage. This slice adds deterministic bounded PTY worker communication without parser dispatch, `TerminalState` mutation, renderer integration, input encoding, or shell policy.

`PtyWorker` owns two std-only threads: a session thread owns the live `PtySession`, the eight-slot command receiver, lifecycle polling, write/resize/terminate handling, and its reader-thread join; a dedicated reader thread owns the exclusive `PtyOutputReader`. The controller owns an eight-slot command sender, eight-slot event receiver, and deterministic worker join. Reader chunks are raw and capped at 1,024 bytes per event. A full event queue blocks the reader rather than dropping bytes, EOF, exit, or errors; a full command queue rejects the newest command with `PtyWorkerError::CommandQueueFull`.

EOF and `Exited` remain separate events with no delivery-order guarantee. Reader EOF is emitted once on `read() == 0`; child exit is emitted once by lifecycle polling at 20 ms intervals. Controller channel loss or event-receiver loss terminates the child; `shutdown_and_join` and controller `Drop` discard unread events, request termination, and join both threads. The worker boundary exposes only project-owned commands, events, and errors; `portable-pty`, channel, parser, terminal-core, renderer, and application types do not leak across it.

## Validation

Focused worker tests and complete local workspace validation are required after final edits, followed by GNU/MSVC target suites. Native CI acceptance for this worker slice remains pending.

## Next

Keep this branch on bounded PTY worker acceptance until the exact commit passes the full native GitHub Actions matrix on Linux x86_64/ARM64, macOS x86_64/ARM64, and Windows x86_64/ARM64. Do not begin parser/event routing, renderer work, input encoding, or shell policy yet.
