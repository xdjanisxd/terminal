# Engineering Workflow

## Before implementation

1. Identify exact behavior and acceptance criteria.
2. Identify the owning crate/module and check dependency direction.
3. Read relevant ADRs and surrounding code.
4. Decide how the behavior will be tested.
5. Record architectural conflicts instead of silently bypassing them.
6. Avoid unrelated improvements; record separate technical debt in `.agent/TASKS.md`.

## Implementation

- Work in a small, reviewable change.
- Prefer a failing test before or alongside correctness-sensitive behavior.
- Implement the smallest design that satisfies current requirements without blocking known requirements.
- Keep resources bounded and platform differences behind narrow interfaces.
- Add a dependency only after applying `docs/CONVENTIONS.md` dependency checks.
- Add or supersede an ADR before changing a costly architectural decision.

## Completion gates

1. Run `cargo fmt --all -- --check`.
2. Run relevant unit and integration tests.
3. Run `cargo check --workspace --all-targets`.
4. Run `cargo clippy --workspace --all-targets -- -D warnings`.
5. Run supported-platform CI/release/compatibility checks when configured.
6. Inspect for architecture and scope violations.
7. Update `docs/CURRENT_STATE.md`.
8. Update `docs/ROADMAP.md` only when acceptance criteria are met.
9. Add or update ADRs when decisions changed.
10. Update `.agent/TASKS.md` and `.agent/SESSION.md`.

Report commands actually run, unsupported validation, remaining limitations, and the next smallest task. Never begin that next task without authorization.
