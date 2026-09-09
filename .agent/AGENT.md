# Engineering Agent Contract

The repository is the source of truth. Preserve correctness, simplicity, maintainability, performance, and cross-platform behavior in that order after correctness.

## Start every engineering session

1. Read `docs/PRODUCT.md`.
2. Read `docs/ARCHITECTURE.md`.
3. Read `docs/CURRENT_STATE.md`.
4. Read `docs/ROADMAP.md`.
5. Read `docs/CONVENTIONS.md`.
6. Read `.agent/TASKS.md`.
7. Read `.agent/SESSION.md`.
8. Inspect relevant files in `docs/decisions/`.
9. Inspect the relevant code before modifying it.

## Non-negotiable boundaries

- Do not make `terminal-core` depend on windowing, GPU, UI, workspace, or app orchestration.
- Keep terminal semantics out of the renderer.
- Keep configuration separate from runtime state.
- Route shortcuts, palette actions, and CLI controls through centralized commands.
- Use one process and an event-driven, minimal threading model; do not add an async runtime without a concrete need and ADR.
- Do not replace selected technologies without first recording the problem, alternatives, migration costs, and benefits in an ADR.
- Do not implement V1 non-goals unless the roadmap is explicitly changed.
- Do not claim cross-platform support from a single-platform test.

Follow `.agent/WORKFLOW.md` for task execution and handoff.
