# Rust and Cargo Workspace

Status: Accepted

## Context

The product requires native cross-platform behavior, low startup and idle overhead, predictable resource ownership, and independently testable components.

## Decision

Use stable Rust and a Cargo workspace. Create dependency-free structural packages for the accepted component boundaries so standard workspace checks are executable before feature implementation. Keep them at version `0.0.0` and non-publishable during bootstrap. Workspace packages inherit the MIT license selected in ADR-0008.

## Alternatives

- C or C++: broad native access but greater memory-safety and build-complexity cost.
- Go: simple tooling but a less direct fit for the selected native GPU and event-loop ecosystem.
- Electron/WebView stacks: conflict with native footprint, startup, and explicit product constraints.

## Consequences

Rust supports explicit ownership, native APIs, and a shared cross-platform codebase. Compile times and platform toolchain setup require ongoing attention. The public product name and minimum supported Rust version remain to be decided before release; neither blocks the current internal package layout.
