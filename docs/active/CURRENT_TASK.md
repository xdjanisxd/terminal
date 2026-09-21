# Current Task

## Goal

Create the minimal project-owned `winit` application shell and `wgpu` surface
lifecycle for M4 without adding terminal rendering.

## Branch

`feat/m3-bounded-pty-workers`

## Current state

Implementation and deterministic validation are complete. Local build verifies
the app/renderer pair, but native window/surface execution and the six-runner
CI matrix have not yet accepted this uncommitted change.

## Important decisions

`app` owns event-loop orchestration and `renderer` owns GPU/surface lifecycle;
`terminal-core` remains independent. `wgpu` 25 and `winit` 0.30 use `pollster`
only for blocking initialization, not an async runtime.

## Files touched

- `docs/active/CURRENT_TASK.md`
- `Cargo.lock`
- `crates/app/Cargo.toml`
- `crates/app/src/main.rs`
- `crates/renderer/Cargo.toml`
- `crates/renderer/src/lib.rs`

## Validation already performed

Focused renderer tests, local app/renderer checks, and full workspace fmt,
check, tests, doc tests, Clippy, and diff validation pass.

## Remaining validation

Native execution and the existing six-runner CI matrix for this change.

## Next step

Commit/push the validated change, then inspect the six native CI job results.
