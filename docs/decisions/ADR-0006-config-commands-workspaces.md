# Declarative Configuration, Central Commands, and Workspaces

Status: Accepted

## Context

Keybindings, command palette actions, CLI control, hot reload, and reproducible workspace startup need consistent behavior without mixing persistent configuration with live session state.

## Decision

Use typed declarative TOML configuration with `serde` and `toml`, and `notify` for partial hot reload. Retain the last valid configuration after an invalid update. Use `lexopt` for basic CLI parsing. Resolve keybindings, palette actions, and CLI controls into one centralized command/action system. Treat workspace definitions as reproducible configuration for roots, tabs, pane layouts, and startup commands, not runtime restoration.

## Alternatives

- Lua or another scripting runtime: more flexible but unnecessary for known V1 needs.
- Separate behavior in each input/UI handler: initially direct but duplicates logic.
- Runtime session serialization: useful for restoration but explicitly outside V1.

## Consequences

Configuration remains inspectable and behavior has a single invocation path. Hot-reload validation and command routing require careful error reporting and tests. The exact configuration schema and reloadable field set will be introduced incrementally.
