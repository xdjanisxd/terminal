
# Repository guide

## Priorities

Correctness > maintainability > performance > convenience.

## Architecture

- terminal-core owns terminal semantics.
- renderer never owns terminal semantics.
- config is separate from runtime state.
- platform differences stay behind project-owned interfaces.
- do not add async runtime without a concrete architectural need.

## Context loading

Do not read the entire documentation set.

Read only what the task requires:

Architecture/boundaries:
`docs/ARCHITECTURE.md`

Current implemented behavior:
`docs/CURRENT_STATE.md`

Milestones:
`docs/ROADMAP.md`

Repository rules:
`docs/CONVENTIONS.md`

Architectural decisions:
relevant `docs/decisions/ADR-*.md`

Active cross-session task:
`docs/active/CURRENT_TASK.md`, only when continuing existing work.

Do not preload these files.

## Workflow

Inspect relevant code before modifying it.

Prefer targeted searches and symbol-level inspection.

Avoid unrelated refactors.

Use repository skills for implementation, architecture review, validation, and task completion workflows.

## Source Navigation

Prefer the cheapest targeted source of information first:

1. `rg` / file-name search
2. symbol-level source inspection
3. Git history/diff when historical context matters
4. relevant project documentation
5. Graphify only for cross-module dependency, ownership, or blast-radius questions

Do not read generated Graphify JSON/HTML files for ordinary code navigation.
Do not recursively inspect `target/` or historical `graphify-out/` snapshots.

## Validation

Run focused tests while implementing.

Before completion, use the repository validation skill.

## Graphify

Use Graphify only when relationships or blast radius are unclear.
Do not use it for simple symbol lookup.

Prefer targeted Graphify queries over reading generated graph files.

Repository docs define intended architecture.
Graphify describes implemented architecture.
