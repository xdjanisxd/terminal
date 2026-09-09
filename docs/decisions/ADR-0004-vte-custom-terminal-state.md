# VTE Parsing with Project-Owned Terminal State

Status: Accepted

## Context

Modern terminal compatibility requires incremental escape-sequence parsing while preserving project control over screen state, invariants, scrollback, selection, resource limits, and testability.

## Decision

Use the `vte` crate as the byte-stream escape parser and implement terminal screen/state semantics in `terminal-core`. Feed PTY bytes incrementally through the parser into a project-owned `TerminalState`. Keep parser dependency types behind project-owned interfaces.

## Alternatives

- Implement the parser and state entirely in-house: greater control but unnecessary parser risk and cost.
- Adopt a complete external terminal model: faster initial coverage but less control over architecture, semantics, and resource policy.

## Consequences

The project reuses a focused parser while retaining independently testable terminal semantics. The custom model carries substantial compatibility responsibility and must grow incrementally through regression, chunk-boundary, Unicode, fuzz, and real-application tests.
