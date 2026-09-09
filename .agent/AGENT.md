# Engineering Agent Contract

The repository is the source of truth.

Preserve correctness, simplicity, maintainability, performance, and cross-platform behavior.

## Start every engineering session

Read only:

1. `.agent/SESSION.md`
2. `.agent/TASKS.md`
3. `docs/CURRENT_STATE.md`

Then load additional context only when relevant to the current task.

Read:

* `docs/ARCHITECTURE.md` when changing component ownership, dependencies, data flow, concurrency, or subsystem boundaries.
* `docs/PRODUCT.md` when evaluating feature scope or product behavior.
* `docs/ROADMAP.md` when starting or completing milestones.
* `docs/CONVENTIONS.md` when repository-wide engineering rules are relevant or unclear.
* relevant files in `docs/decisions/` only when the task touches an accepted architectural decision.
* relevant source files before modifying them.

Do not reread all repository documentation or all ADRs on every task.

Start with the smallest useful context and expand only when necessary.

## Non-negotiable boundaries

* Do not make `terminal-core` depend on windowing, GPU, UI, workspace, or app orchestration.
* Keep terminal semantics out of the renderer.
* Keep configuration separate from runtime state.
* Route shortcuts, palette actions, and CLI controls through centralized commands.
* Use one process and an event-driven, minimal threading model.
* Do not add an async runtime without a concrete need and ADR.
* Do not replace selected technologies without first recording the problem, alternatives, migration costs, and benefits in an ADR.
* Do not implement V1 non-goals unless the roadmap is explicitly changed.
* Do not claim cross-platform support from a single-platform test.

## Context efficiency

Do not load entire files when a relevant symbol, section, ADR, or targeted search is sufficient.

When Graphify is available, use it selectively for:

* callers and dependents
* cross-module relationships
* dependency paths
* blast-radius analysis
* architecture-drift checks

Graphify describes the implementation that exists.

Repository documentation and accepted ADRs define the intended architecture.

If Graphify conflicts with architecture documentation, investigate the implementation rather than changing the architecture to match the graph.

Follow `.agent/WORKFLOW.md` for task execution and handoff.

