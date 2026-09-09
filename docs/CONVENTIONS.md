# Conventions

## Engineering

- Keep changes scoped to the requested behavior and place responsibility in the owning crate/module.
- Prefer small cohesive modules, explicit ownership and state transitions, simple control flow, and project-owned interfaces at dependency boundaries.
- Keep configuration separate from runtime state and command resolution separate from input/UI handlers.
- Do not add async runtimes, global mutable state, speculative abstractions, excessive traits/macros, or unrelated refactors without demonstrated need.
- Use stable Rust, `rustfmt`, and Clippy. Avoid unnecessary cloning and allocation, but do not trade correctness or clarity for unmeasured optimization.

## Errors, logging, and unsafe code

- Use explicit library error types with `thiserror` and application-level context with `anyhow` when those crates are introduced.
- Use the `log` facade; do not expose sensitive terminal contents in logs by default.
- Avoid `unsafe`. Required significant unsafe blocks must include a `SAFETY` comment that states the upheld invariant.

## Dependencies

Before adding a crate, check the standard library, existing dependencies, maintenance, cross-platform support, dependency tree, startup and runtime cost, and whether a small clear implementation is safer. Add dependencies when they solve difficult, platform-specific, security-sensitive, or established problems. Record major technology changes in a new ADR before implementation.

## Testing

- Add regression tests with fixes and keep `terminal-core` independently testable.
- Test parsing across arbitrary byte boundaries, Unicode and cell-width behavior, cursor and mode transitions, scrolling, alternate screen, resize, scrollback, workspace layout, config validation, and keybindings as their implementations arrive.
- Cross-platform behavior is not established until exercised on each supported platform or authenticated CI.

## Required local gates

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
```

Add release builds, integration tests, fuzzing, compatibility probes, and benchmarks when their targets exist.

## Documentation

- Repository documentation overrides conversational context when they conflict.
- `CURRENT_STATE.md` describes implemented reality; `ROADMAP.md` tracks completion and future work.
- Create a new ADR when an accepted decision changes; mark the old ADR superseded rather than deleting or rewriting history.
- Update `.agent/SESSION.md` at the end of each meaningful session with only the context needed to resume.
