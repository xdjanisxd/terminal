# Engineering Workflow

## Context loading

Begin with the smallest useful context.

Always start from:

* `.agent/SESSION.md`
* `.agent/TASKS.md`
* `docs/CURRENT_STATE.md`

Load architecture documents, ADRs, roadmap sections, conventions, source files, or Graphify context only when required by the task.

Avoid loading unrelated files or entire documentation sets preemptively.

## Before implementation

1. Identify exact behavior and acceptance criteria.
2. Identify the owning crate/module and dependency direction.
3. Inspect the relevant code.
4. Read only relevant ADRs or architecture sections.
5. Decide how the behavior will be tested.
6. Record architectural conflicts instead of silently bypassing them.
7. Avoid unrelated improvements; record separate technical debt in `.agent/TASKS.md`.

## Implementation

* Work in a small, reviewable change.
* Prefer a failing test before or alongside correctness-sensitive behavior.
* Implement the smallest design that satisfies current requirements.
* Keep resources bounded and platform differences behind narrow interfaces.
* Add a dependency only after applying `docs/CONVENTIONS.md` dependency checks.
* Add or supersede an ADR before changing a costly architectural decision.
* Use Graphify only when structural context or blast-radius analysis is useful.

## Completion gates

Run:

1. `cargo fmt --all -- --check`
2. relevant unit and integration tests
3. `cargo check --workspace --all-targets`
4. `cargo clippy --workspace --all-targets -- -D warnings`
5. supported-platform CI/release/compatibility checks when configured

Then:

6. inspect for architecture and scope violations
7. update `docs/CURRENT_STATE.md`
8. update `docs/ROADMAP.md` only when acceptance criteria are met
9. update ADRs only when decisions changed
10. update `.agent/TASKS.md`
11. update `.agent/SESSION.md`

## Reporting

Report only:

1. What changed
2. Tests and validation
3. Important decisions or limitations
4. Documentation updated
5. Recommended next task

Keep reports concise unless there is a blocker or architecture change.

Do not reproduce full command logs, test lists, diffs, or dependency trees unless they are relevant to a problem.

Never begin the recommended next task without authorization.

After material structural or public-API changes, run:

graphify . --update

Then use Graphify only for targeted architecture checks.
Do not treat graph warnings or high-degree nodes as problems
without comparing them against docs/ARCHITECTURE.md and ADRs.
