# License, Internal Naming, and Target Architectures

Status: Accepted

## Context

The repository needs a license and an explicit release architecture matrix before cross-platform quality gates are established. The public product name and minimum operating-system versions are not yet supported by implementation evidence.

## Decision

Use the MIT license. Retain the current internal `terminal-*` Cargo package names; choosing a public product or installed binary name does not block development.

Target these native architectures:

- Windows: x86_64 and ARM64
- macOS: x86_64 and ARM64
- Linux: x86_64 and ARM64

Do not choose minimum operating-system versions speculatively. When `winit`, `wgpu`, `portable-pty`, `fontdb`, `swash`, platform SDKs, and native platform validation are introduced, determine the support floor from their actual requirements and verified behavior. Record the resulting product support matrix in a new ADR. CI runner image versions are validation environments, not declarations of minimum product support.

## Alternatives

- Proprietary licensing: inconsistent with the selected open license.
- Apache-2.0 or dual licensing: valid options but not the chosen policy.
- Support only x86_64 initially: simpler CI but contrary to the required architecture matrix.
- Guess minimum OS versions now: creates unsupported promises before the selected stack is integrated.

## Consequences

All workspace packages inherit MIT metadata and the license text is distributed in the repository. CI must cover six native OS/architecture combinations where hosted runners are available. Public branding and exact operating-system floors remain deliberately unresolved without blocking implementation.
