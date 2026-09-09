# Session Handoff

## Changed

- Added parser-independent `TerminalState` in `crates/terminal-core/src/state.rs` and exported it from `lib.rs`.
- `TerminalState` privately owns one active `ScreenGrid`, `TerminalModes`, and `InputModes`.
- Added controlled operations for absolute/relative cursor movement, screen clear, resize, supported terminal mode changes, and cursor-key mode changes.
- Added a project-owned `reset` that preserves dimensions, clears the screen, homes the cursor, and restores all currently supported modes to defaults; it is not DECSTR or RIS.
- Screen and mode accessors return immutable references; compile-fail doctests enforce that state-owned values cannot be mutated through those accessors.
- Lower-level model APIs were not restricted because independently owned models remain testable and the facade exposes no mutable path to its owned values.
- Added 12 facade tests: nine public integration tests and three internal delegation/content tests, plus three compile-fail ownership doctests.
- No parser, `vte`, PTY, renderer, input encoder, scrollback, alternate screen, or dependency was added.

## Validation

- All required local Cargo gates passed with `CARGO_BUILD_TARGET=x86_64-pc-windows-gnu`.
- `terminal-core` passed 41 unit/integration tests and three compile-fail doctests.
- Cargo metadata confirmed zero `terminal-core` dependencies.
- Independent review passed with no security, logic, architecture, scope, or compatibility blockers; all three reported coverage gaps were addressed.

## Decisions

- `TerminalState` is the semantic mutation boundary for future parser adapters.
- Read-only screen/mode access is explicit; callers receive no mutable references into state-owned values.
- Reset is a project-owned model reset, not an approximation of DECSTR or RIS.
- Existing `ScreenGrid`, `TerminalModes`, and `InputModes` mutation APIs remain public for independent model use and tests; they cannot bypass ownership inside `TerminalState`.

## Known limitations

- No printable-character operation or parser adapter exists yet.
- Reset reconstructs the grid at existing dimensions; this is simple and correct but may briefly allocate replacement storage before old storage is released.
- The local Git Bash environment cannot link the default MSVC target; linked local tests use the installed Windows GNU target.
- This feature branch has not run remote CI.

## Next recommended task

Integrate a minimal incremental `vte` adapter behind a project-owned interface and route printable-character behavior through `TerminalState`, with arbitrary chunk-boundary tests. Do not add CSI, SGR, PTY, rendering, or broader compatibility behavior in that slice.
