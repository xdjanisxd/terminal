---
Name: implement-terminal-feature
description: Implement a non-trivial terminal behavior or roadmap feature in this Rust terminal emulator. Use when runtime behavior, parser/state semantics, PTY behavior, or focused feature tests must change. Do not use for documentation-only or simple cleanup tasks.
---

# Implement Terminal Feature

Use this workflow for non-trivial terminal feature implementation.

## 1. Understand

If ownership, dependency direction, public API impact, or concurrency boundaries are unclear, use the `architecture-check` skill before planning.

Read the requested task.

Consult only project documentation relevant to the task.

Inspect:

- the owning crate and module
- relevant implementation
- relevant existing tests
- public APIs involved
- neighboring terminal behavior

Determine:

- what component owns the behavior
- what invariants must remain unchanged
- what existing behavior could regress

Do not begin implementation until the ownership and expected behavior are
understood.

## 2. Plan

Create a concise implementation plan.

Identify:

- files likely to change
- state changes
- parser or dispatch changes, if applicable
- tests required
- edge cases
- possible regressions

Prefer the smallest coherent implementation.

Do not turn the task into a broader refactor.

## 3. Implement

Implement only the requested behavior and required supporting changes.

Preserve existing architectural boundaries.

Avoid:

- unrelated cleanup
- speculative abstractions
- unnecessary public API expansion
- unnecessary dependencies
- unrelated formatting changes

Follow existing project patterns when they remain appropriate.

## 4. Test incrementally

Run the narrowest relevant test target first.

Examples:

`cargo test -p terminal-core <relevant-test>`

or:

`cargo test -p terminal-pty <relevant-test>`

Expand to crate-level testing after focused tests pass.

Do not run the entire workspace after every small edit.

## 5. Review implementation

Before full validation, inspect the resulting diff.

Check:

- requested semantics are implemented
- tests prove the intended behavior
- unrelated state is preserved
- edge cases are covered
- no unrelated code changed
- no architecture boundary was bypassed

Fix discovered issues before continuing.

## 6. Validate

Use the `validate-rust-change` skill.

## 7. Finish

After validation succeds, use the 'finish-task' skill.
