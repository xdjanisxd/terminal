---
name: finish-task
description: Finalize a completed repository task after implementation and validation by reconciling project documentation, active-task state, and the final concise handoff report. Use only when the implementation is complete or work must be handed off to another session. Do not use during normal implementation.
---

# Finish Task

Use this skill only after implementation work is complete and relevant validation has been performed, or when unfinished work must be handed off across sessions.

Do not modify implementation merely to make documentation easier to describe.

## 1. Determine completion state

Decide whether the task is:

- complete and validated
- complete with an explicit validation limitation
- incomplete and requires handoff

Do not describe incomplete work as complete.

## 2. Reconcile documentation

Update documentation only when repository reality changed.

### Current state

Update `docs/CURRENT_STATE.md` when implemented behavior, supported semantics, runtime structure, or component capabilities changed.

Record the resulting state, not a chronological work log.

Do not add implementation trivia already obvious from the code.

### Roadmap

Update `docs/ROADMAP.md` only when:

- a roadmap item was completed
- milestone status changed
- project sequencing materially changed

Do not use the roadmap as a changelog.

### Architecture decisions

Create or update an ADR under `docs/decisions/` only when a durable architectural decision changed.

Examples include:

- component ownership
- dependency direction
- public architectural interface
- threading model
- backend strategy
- persistent project-wide constraint

Do not create ADRs for ordinary feature implementation details.

## 3. Handle active task state

Use `docs/active/CURRENT_TASK.md` only for cross-session continuity.

### If the task is complete

Reset it to:

```markdown
# Current Task

No active task.
```

### If work remains

Keep the file concise and include only:

- goal
- branch
- current state
- important task-specific decisions
- files touched
- validation already performed
- remaining validation
- single best next step

Do not duplicate information already stored in architecture docs, ADRs, or source code.

## 4. Inspect final repository state

Inspect:

`git status`

`git diff`

Confirm:

- only intended files changed
- documentation matches implemented reality
- temporary files are absent
- generated output was not added accidentally
- active task state is correct

Do not create commits unless explicitly requested.

## 5. Final report

Provide a concise completion report containing:

1. what changed
2. tests and validation performed
3. important decisions or limitations
4. documentation or architectural updates
5. recommended next task, only when it is evident from the roadmap or active work

Do not repeat the full diff.

Do not claim validation that was not actually performed.

Keep the report suitable for use as a future session handoff.
