# Session Handoff

## Changed

- Added a std-only `terminal-core` mode model with typed enums, private storage, documented defaults, and explicit setters.
- `TerminalModes` owns terminal-global cursor visibility, auto-wrap, and insert/replace behavior.
- `InputModes` separately owns normal/application cursor-key state for later consumption by input encoding.
- Added 10 public-API mode tests covering defaults, both directions of every transition, repeated/idempotent changes, unrelated-mode independence, and input/global ownership separation.
- No numeric parser identifiers, parser, `vte`, PTY, renderer, input encoder, scrollback, alternate screen, SGR expansion, or external dependency was added.
- An initial per-screen origin-mode design was rejected during independent review and removed before completion.

## Validation

- All four required repository Cargo gates passed locally with `CARGO_BUILD_TARGET=x86_64-pc-windows-gnu`.
- Workspace tests passed: 29 `terminal-core` integration tests, including 10 mode tests; no failures.
- `cargo metadata --format-version 1 --no-deps` confirmed that `terminal-core` has zero dependencies.
- Static scan found no `unsafe`, forbidden architectural dependencies, parser identifiers, or out-of-scope subsystem references in the new mode module.
- Independent review passed after the origin-mode correction. It found no remaining blockers; its setter-isolation test suggestion was addressed.

## Compatibility-sensitive decisions

- Cursor visibility, auto-wrap, and insert/replace are terminal-global state.
- Application cursor-key mode is terminal-core state because terminal output controls it, but it is isolated in `InputModes` because a future input encoder consumes it.
- No screen-local mode is included in this slice. Screen-local cursor, margins, and wrap-pending values are state, not automatically mode flags.
- Origin mode is deferred until `TerminalState` can apply its cursor-home and scrolling-region effects atomically and define DECSC/DECRC plus 47/1047/1049 interactions.
- Line-feed/new-line mode, application keypad mode, cursor blinking, mouse/focus reporting, and other historical modes remain unmodeled until owning behavior requires them.

## Known limitations

- These types store mode values only; no terminal writing, reset operation, parser mapping, rendering, or input encoding consumes them yet.
- The local Git Bash environment cannot link the default MSVC target because Visual C++ build tools and Windows import libraries are unavailable; the installed Windows GNU target is used for linked local tests.
- This feature branch has not run remote CI yet.

## Next recommended task

Introduce a small parser-independent `TerminalState` facade that owns the active `ScreenGrid`, `TerminalModes`, and `InputModes`, then define atomic reset and semantic state operations with tests. Do not add `vte` in that slice.
