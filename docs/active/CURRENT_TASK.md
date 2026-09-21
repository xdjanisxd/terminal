# Current Task

## Goal

Close the M3 bounded PTY worker communication acceptance item by obtaining
reproducible native GitHub Actions evidence for the existing worker slice on
Linux x86_64/ARM64, macOS x86_64/ARM64, and Windows x86_64/ARM64.

## Branch

Current working branch.

## Current state

The existing six-runner workflow covers the exact acceptance scope. A failed
Windows x86_64 run exposed a test ordering race: shutdown disconnected events
and legitimately requested termination before the test verified accepted queued
writes. The worker implementation remains unchanged; the test now waits for
those writes before shutdown.

## Important decisions

Use the existing CI structure where possible; this task is acceptance evidence,
not a PTY feature expansion.

## Files touched

- `docs/active/CURRENT_TASK.md`
- `crates/terminal-pty/tests/worker.rs`

## Validation already performed

`cargo fmt --all -- --check` and `cargo test -p terminal-pty --test worker
-- --test-threads=1` pass locally.

## Remaining validation

Run the exact worker acceptance on all six native targets and retain
reproducible run evidence.

## Next step

Push the minimal test correction and verify its six-runner native CI run.
