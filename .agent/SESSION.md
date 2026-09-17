# Session Handoff

## Current state

`feat/m3-pty-contract` defines the first M3 project-owned PTY boundary without a backend. `terminal-pty` now owns validated character-cell size, owned mechanism-only spawn configuration, raw byte/EOF/exit events, project-owned lifecycle/error types, and narrow `PtyBackend`/`PtySession` traits. Output is arbitrary undecoded bytes; empty chunks are omitted; EOF and exit are distinct; termination is explicit/idempotent; no real child, portable-pty dependency, threads, queues, parser coupling, or renderer coupling exists.

Primary owns the only 10,000-row scrollback. Resize never mutates it: rows retain exact capture-time widths, complete cells, combining payloads, and wide metadata. There is no reflow, crop/pad, logical-line reconstruction, viewport offset, renderer composition, navigation, persistence, or Alternate history. `?47`, `?1047`, and `?1049` resize round trips preserve Primary history; `?1049` restore correctly clamps saved cursor coordinates and restores saved rendition. Full reset clears Primary history.

A narrow Clippy correction was required in the completed scrollback capture path: restricted scrolls now return directly to the unchanged grid primitive before the Primary full-screen capture branch. It does not change capture policy or state behavior.

## Validation

All repository gates passed after the scoped correction: format, workspace all-target tests, doctests, check, Clippy with `-D warnings`, GNU/MSVC target suites, and `git diff --check`. No dependency changes were made.

Graphify completed a code-only structural incremental refresh and clustering: 1,047 nodes, 1,626 edges, 106 communities. Documentation semantic enrichment was not performed because no semantic LLM/API key was supplied; this was non-blocking. Targeted graph output placed `TerminalState`, `ScreenSet`, `ScreenState`, `ScreenGrid`, `Scrollback`, parser, and relevant tests in the expected ownership paths, with no parser/renderer ownership of history.

## Next

Run the remaining complete validation after the scoped Clippy correction, then begin the roadmap's representative modern shell/TUI validation only once PTY and renderer integration make such validation meaningful. Do not add viewport UI, history navigation, or reflow as a prerequisite for the closed terminal-core parent.
