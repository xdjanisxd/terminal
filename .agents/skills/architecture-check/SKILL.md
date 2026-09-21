---
name: architecture-check
description: Review architectural ownership, dependency direction, public API impact, concurrency boundaries, and blast radius for structural changes in this Rust terminal emulator. Use when crate responsibilities, dependencies, PTY/threading behavior, public APIs, or architectural boundaries may change. Do not use for routine local implementation changes with clear ownership.
---

# Architecture Check

Use this skill only when a change may affect project architecture or when ownership is unclear.

Do not perform a full repository architecture review for ordinary local changes.

## 1. Identify the architectural question

Determine exactly what may change.

Examples:

- ownership of terminal behavior
- crate dependency direction
- public API surface
- PTY/process lifecycle
- threading or synchronization
- renderer/state responsibilities
- platform abstraction
- new dependency or runtime
- cross-crate data flow

State the architectural question narrowly.

## 2. Inspect existing ownership

Inspect the relevant source code first.

Determine:

- which crate currently owns the behavior
- which types expose the behavior
- which crates depend on those types
- whether an existing abstraction already represents the required boundary

Prefer targeted symbol and source inspection over broad repository reading.

## 3. Read only relevant architecture documentation

Consult only what is needed:

- `docs/ARCHITECTURE.md` for component ownership and dependency direction
- relevant files under `docs/decisions/` for accepted architectural decisions
- `docs/CURRENT_STATE.md` when implemented behavior must be confirmed
- `docs/PERFORMANCE.md` only when performance constraints are relevant

Do not preload all documentation or all ADRs.

## 4. Check dependency and blast radius

Use targeted source search and Git first.

If cross-module relationships, ownership, or blast radius remain unclear, use the existing Graphify graph through targeted query commands.

Prefer:

`graphify query "<specific architectural question>" --budget 800`

Use:

`graphify path "<source-node>" "<target-node>"`

when the question is specifically about the connection between two known components.

Use:

`graphify explain "<node>"`

when additional context about one graph node is required.

Keep Graphify questions narrow and concrete.

Examples:

- what depends on `PtyWorker`?
- what connects `TerminalParser` to `TerminalState`?
- what is the dependency path from `terminal-app` to `terminal-pty`?
- what is the blast radius of changing `PendingReplies`?

Do not open or read complete `graphify-out/graph.json`, HTML output, or historical graph snapshots during normal analysis.

If the graph is stale relative to recent structural changes, refresh it before relying on its result.

## 5. Evaluate the proposed change

Check whether the change:

- preserves existing ownership boundaries
- introduces reverse or unexpected dependencies
- leaks platform-specific behavior into portable crates
- moves terminal semantics outside `terminal-core`
- moves PTY/process lifecycle outside `terminal-pty`
- gives the renderer ownership of terminal semantics
- unnecessarily expands public APIs
- introduces an unnecessary dependency or runtime
- creates avoidable coupling
- changes concurrency assumptions

Prefer using an existing boundary over creating a new abstraction.

## 6. Decide the minimum architectural action

Choose the smallest architectural response that satisfies the task.

Possible outcomes:

- no architectural change is required
- use an existing abstraction
- make a small internal boundary adjustment
- introduce a narrowly scoped interface
- update an existing ADR
- create a new ADR because a durable architectural decision changed

Do not create an ADR for ordinary implementation details.

## 7. Report

Report briefly:

- current owner
- affected boundary
- relevant dependency or API impact
- whether an architectural change is required
- relevant ADRs consulted
- Graphify usage, if any

If the existing architecture already supports the change, say so and return to the implementation workflow.
