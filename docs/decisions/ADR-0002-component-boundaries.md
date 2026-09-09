# Component Boundaries and Dependency Direction

Status: Accepted

## Context

Terminal semantics, platform I/O, rendering, workspace state, configuration, and application orchestration evolve and test differently. Coupling them would make correctness tests and platform maintenance harder.

## Decision

Separate responsibilities into `app` (`terminal-app`), `terminal-core`, `terminal-pty`, `renderer` (`terminal-renderer`), `workspace` (`terminal-workspace`), `config` (`terminal-config`), and `platform` (`terminal-platform`). `terminal-core` owns terminal semantics and must not depend on `winit`, `wgpu`, UI, workspace, or application orchestration. The renderer consumes state/snapshots and damage rather than owning semantics. External implementations stay behind narrow project-owned boundaries.

## Alternatives

- One application crate: simpler initially but encourages boundary erosion and slow, coupled tests.
- A highly generic plugin/service architecture: flexible but speculative and unnecessarily complex for V1.

## Consequences

Core behavior can be tested without GPU or OS integration, and platform differences remain localized. Crate boundaries add some coordination and require care to avoid cyclic dependencies. The current internal `terminal-*` names are retained; public branding remains open and does not block development.
