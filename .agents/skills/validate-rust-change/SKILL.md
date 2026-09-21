---
name: validate-rust-change
description: Perform final validation and regression review after a Rust implementation change is complete. Use when runtime code, tests, Cargo configuration, or Rust build behavior changed. Do not use as the primary workflow for documentation-only changes.
---

# Validate Rust Change

Use this only when implementation is believed to be complete.

## Inspect changes

Inspect:

`git status`

`git diff`

Determine exactly which files changed and why.

Look for:

- unintended modifications
- unrelated refactors
- accidental formatting churn
- generated files
- debug code
- temporary files

## Dependency review

If `Cargo.toml` or `Cargo.lock` changed:

- determine why
- verify the dependency change is necessary
- report it explicitly

Unexpected dependency changes are a failure.

## Rust review

Check changed code for:

- unnecessary clones
- unnecessary allocations
- avoidable copies
- unnecessary public API expansion
- new `unsafe`
- panic-prone logic
- concurrency hazards
- nondeterministic tests

Do not invent speculative optimizations.

## Validation

Run:

`cargo fmt --all -- --check`

`cargo check --workspace --all-targets`

`cargo test --workspace --all-targets`

`cargo test --workspace --doc`

`cargo clippy --workspace --all-targets -- -D warnings`

`git diff --check`

All commands must pass before reporting successful validation.

If a command fails because of the implementation, fix the issue and rerun the
relevant validation.

Do not hide or ignore failures.

## Final diff review

After validation, inspect:

`git status --short`

`git diff --stat`

and the human-authored diff:

`git diff -- . ':(exclude)graphify-out/**'`

Verify that fixes made during validation did not introduce unrelated changes.

If Graphify output changed, confirm only that the refresh was intentional.

Do not consume raw generated Graphify diffs.

Report the exact validation performed and any limitations.

Do not update project documentation as part of this skill.

After successful validation, return control to the calling workflow or use `finish-task` when the task is ready to close.
