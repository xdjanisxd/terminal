# Tasks

## Next

- [ ] Push the foundation to a remote and require the first CI run to pass on all six native OS/architecture runners before beginning M1.
- [ ] Begin M1 with bounded terminal cell, attribute, grid, and cursor primitives plus focused unit tests; do not add parsing or rendering in that task.

## Later

- [ ] Determine minimum OS versions from pinned stack requirements and real platform validation when those dependencies are introduced; record the result in a new ADR.
- [ ] Resolve renderer fallback, operational defaults, telemetry policy, and compatibility-version matrix before their owning implementation or release gate.

## Deferred by scope

- Plugin system, embedded AI, SSH manager, IDE features, runtime session restoration, and large scripting/UI frameworks remain outside V1.
