# Session Handoff

## Current state

`test/m3-pty-lifecycle-semantics` starts from the accepted portable-pty adapter state. This slice adds deterministic repository-helper lifecycle coverage without PTY workers, event channels, parser dispatch, renderer integration, input encoding, or shell policy.

`PortablePtySession` now records child exit after both natural completion and a termination request. Verified transitions are `Running -> Exited` for natural exit and `Running -> TerminationRequested -> Exited` when the child has not already exited; a fast backend may be observed as `Exited` immediately after `terminate`. `terminate` remains idempotent and rejects later write/resize operations from its first request. Known exit releases the adapter-owned master PTY, allowing the exclusively transferred reader to drain buffered raw bytes and subsequently observe EOF. EOF and child exit remain distinct project-owned primitives.

The helper accepts narrowly scoped `exit <code>`, `wait`, and `payload` modes. Tests use finite reads and a five-second deadline to verify numeric natural exit, stable lifecycle polling, post-exit write/resize rejection, idempotent termination, safe running resize and raw-write calls, buffered output after observed exit, and EOF. The raw input write test proves byte acceptance without a user shell; it does not claim a cross-platform raw echo round trip because the helper intentionally does not modify platform TTY line discipline.

## Validation

Focused lifecycle tests, the complete repository suite, GNU/MSVC target suites, and the structural Graphify refresh all pass locally. Native CI acceptance remains pending after the final edits.

## Next

Keep this branch on the same lifecycle acceptance slice until the full suite and native GitHub Actions matrix pass on Linux x86_64/ARM64, macOS x86_64/ARM64, and Windows x86_64/ARM64. Do not begin PTY workers or bounded event channels yet.
