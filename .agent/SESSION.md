# Session Handoff

## Current state

`feat/m3-portable-pty-adapter` now contains the concrete portable-pty adapter behind the completed project-owned PTY contract. `PortablePtyBackend` maps owned spawn configuration and validated cell dimensions to the native portable-pty system. `PortablePtySession` owns the master PTY, child handle, writer, pre-acquired single reader capability, and persisted lifecycle state. No portable-pty, anyhow, or std::io type appears in public project signatures.

`take_output_reader` transfers the only raw reader exactly once; a repeated call deterministically returns `PtyError::NotRunning` without creating a competing reader. The reader returns arbitrary undecoded bytes and maps failures to project-owned `PtyError::ReadFailed`. A zero-byte read is raw output EOF and remains separate from independently observed `PtyLifecycle::Exited`. Future workers may turn those primitives into `PtyOutput::Bytes`, `PtyOutput::Eof`, and `PtyOutput::Exited`; no workers, channels, parser integration, renderer integration, or shell policy has been added.

Write and character-cell resize are allowed only while `Running`. Lifecycle polling records a known exit; after exit, write and resize return `PtyError::NotRunning`. `terminate` changes lifecycle to `TerminationRequested` before dropping the writer and requesting backend termination; repeated termination is idempotent. Drop is only a backend safety net.

## Validation

Before the final adapter corrections, all requested repository gates passed on the existing committed tree: format, workspace all-target tests, doctests, check, Clippy with `-D warnings`, GNU/MSVC target workspace suites, and `git diff --check`.

The focused post-edit Windows runtime smoke test now passes against a repository-owned helper executable. It accumulates raw PTY bytes across bounded reads until a marker subsequence occurs, handles ConPTY's test-only cursor-position query without using a shell, verifies exclusive reader extraction, observes exit with a bounded deadline, and confirms post-exit write/resize rejection. Complete final validation and the Graphify refresh passed. Native CI acceptance remains pending.

## Next

Finish the same `feat/m3-portable-pty-adapter` branch: run the final complete validation and structural refresh, then push only when authorized so the existing native GitHub Actions matrix can provide Linux/macOS runtime acceptance. Do not begin the separate deterministic lifecycle/resize/EOF/exit/termination roadmap slice unless that acceptance evidence succeeds.
