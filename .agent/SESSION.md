# Session Handoff

## Current state

`feat/m3-portable-pty-adapter` closes the M3 portable-pty adapter roadmap item. `PortablePtyBackend` maps owned spawn configuration and validated cell dimensions to the native portable-pty system. `PortablePtySession` owns the master PTY, child handle, writer, pre-acquired single reader capability, and persisted lifecycle state. No portable-pty, anyhow, or std::io type appears in public project signatures.

`take_output_reader` transfers the only raw reader exactly once; a repeated call deterministically returns `PtyError::NotRunning` without creating a competing reader. The reader returns arbitrary undecoded bytes and maps failures to project-owned `PtyError::ReadFailed`. A zero-byte read is raw output EOF and remains separate from independently observed `PtyLifecycle::Exited`. Future workers may turn those primitives into `PtyOutput::Bytes`, `PtyOutput::Eof`, and `PtyOutput::Exited`; no workers, channels, parser integration, renderer integration, or shell policy has been added.

Write and character-cell resize are allowed only while `Running`. Lifecycle polling records a known exit; after exit, write and resize return `PtyError::NotRunning`. `terminate` changes lifecycle to `TerminationRequested` before dropping the writer and requesting backend termination; repeated termination is idempotent. Drop is only a backend safety net.

## Validation

GitHub Actions run 35320057146 for commit `8d8e32668fe6ee41770514e5b64871d520a04bc5` completed successfully. Its `cargo test --workspace --all-targets` step ran the portable-pty adapter smoke test successfully on Linux x86_64, Linux ARM64, macOS x86_64, macOS ARM64, Windows x86_64, and Windows ARM64.

Local final validation also passed: format, workspace all-target tests, doctests, check, Clippy with `-D warnings`, and `git diff --check`. No production-code fix was required after CI.

## Next

Begin only the next M3 roadmap slice: Add deterministic lifecycle, resize, EOF, exit, and termination tests. Keep workers, channels, parser integration, renderer integration, input encoding, shell policy, and M4 work out of that slice.
