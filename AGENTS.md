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

During implementation:

- Prefer the smallest command that can answer the current question.
- Scope commands to the affected crate, module, file, symbol, or test whenever possible.
- Do not run workspace-wide validation after every change.
- Do not repeat successful commands unless relevant code changed afterward.
- Prefer quiet or summarized command output when successful output carries no useful information.
- On failure, inspect only the diagnostics needed to identify and fix the problem.
- Do not dump large files, directories, diffs, logs, or generated output into context unless required.

## Source Navigation

Prefer the cheapest targeted source of information first:

1. `rg` / file-name search
2. symbol-level source inspection
3. Git history/diff when historical context matters
4. relevant project documentation
5. Graphify only for cross-module dependency, ownership, or blast-radius questions

Search narrowly before broadening scope.

Prefer:

- `rg` scoped to likely crates/directories over repository-wide searches.
- exact symbols or distinctive identifiers over broad keywords.
- file/range inspection over reading complete large files.
- `git diff --stat` or `git status --short` before inspecting full diffs.
- path-scoped or file-scoped diffs when only part of the repository changed.

Broaden a search only when the targeted search is insufficient.

Do not recursively search generated or dependency directories unless explicitly required, including:

- `target/`
- historical `graphify-out/` snapshots
- `.git/`

Do not read generated Graphify JSON/HTML files for ordinary code navigation.

## Validation

Run focused tests while implementing.

Prefer validation in increasing scope:

1. directly affected test or test module
2. affected crate
3. affected workspace subset when necessary
4. full repository validation at completion

Use quiet output for successful validation when practical.

Do not rerun an unchanged validation step solely to reproduce a successful result.

Before completion, use the repository validation skill.

Full workspace validation belongs at the completion boundary unless the task specifically requires it earlier.

## Git and diff inspection

Start with:

- `git status --short`
- `git diff --stat`

Inspect only changed paths relevant to the task before requesting the full diff.

Use full repository diffs only when needed for final review or when the change spans enough files that scoped inspection would be misleading.

Do not repeatedly inspect unchanged diffs.

## Terminal output discipline

Treat terminal output as context with a cost.

Prefer commands and flags that suppress routine success output while preserving warnings and errors.

For commands that may produce large output:

- narrow the command scope first;
- use quiet modes when they preserve useful diagnostics;
- capture or filter output when only a small portion is relevant;
- avoid printing generated artifacts or large machine-readable files;
- do not print complete logs merely to confirm success.

If a command fails, focus subsequent inspection on the failing component rather than rerunning broad commands with maximum output.

## Graphify

Use Graphify only when relationships or blast radius are unclear.

Do not use it for simple symbol lookup.

Prefer targeted Graphify queries over reading generated graph files.

Repository docs define intended architecture.

Graphify describes implemented architecture.

## Model delegation

Use only GPT-6 Sol and GPT-6 Luna.

GPT-6 Sol is the primary agent. Keep architecture decisions, planning,
implementation, complex debugging, cross-crate reasoning, and final
integration in Sol.

Prefer GPT-6 Luna for bounded auxiliary work such as:

- targeted searches and source inspection;
- focused and completion validation;
- test execution, `cargo check`, `cargo clippy`, and formatting checks;
- status, diff statistics, and scoped diff inspection;
- checking explicit acceptance criteria.

Do not delegate trivial work when transferring context would cost more
than doing it directly.

Give Luna only the context required for its task. Luna must not make
architecture or ownership decisions.

If Luna encounters a non-trivial failure, architectural ambiguity, or
a change requiring broader reasoning, return the relevant findings to
Sol instead of attempting speculative fixes.

Sol remains responsible for interpreting delegated results and deciding
whether the task is complete.
