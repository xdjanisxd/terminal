# V1 Scope Boundaries

Status: Accepted

## Context

A terminal emulator already has substantial correctness, platform, rendering, and compatibility risk. Adjacent IDE and platform-management features would dilute that work.

## Decision

V1 remains a keyboard-first terminal and reproducible workspace application. Defer plugins and marketplaces, embedded AI, SSH management, file explorer, built-in Git UI/editor/Docker UI, runtime session restoration, large scripting runtimes, Electron, WebView UI, React, Qt, and large GUI frameworks. System tools may run normally inside terminal sessions; that does not make them built-in product features.

## Alternatives

- Build a terminal-centered IDE or extensible platform from the start.
- Add a plugin system early to defer product decisions to extensions.

## Consequences

Engineering effort stays focused on terminal correctness, cross-platform PTY behavior, rendering, and bounded resources. Future inclusion of a deferred capability requires an explicit roadmap change and, when architecturally significant, a new ADR; accepted history is not deleted.
