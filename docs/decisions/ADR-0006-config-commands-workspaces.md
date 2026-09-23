# Declarative Configuration, Central Commands, and Workspaces

Status: Accepted

## Context

Keybindings, command palette actions, CLI control, hot reload, and reproducible workspace startup need consistent behavior without mixing persistent configuration with live session state.

## Decision

Use typed declarative TOML configuration with `serde` and `toml`. The app checks the small config file every 500 ms on a dedicated standard thread and sends a change event to the main loop for partial hot reload. This handles edits and file replacement with no additional watcher dependency. Retain the last valid configuration after an invalid update. Use `lexopt` for basic CLI parsing. Resolve keybindings, palette actions, and CLI controls into one centralized command/action system. Treat workspace definitions as reproducible configuration for roots, tabs, pane layouts, and startup commands, not runtime restoration.

## Alternatives

- Lua or another scripting runtime: more flexible but unnecessary for known V1 needs.
- Separate behavior in each input/UI handler: initially direct but duplicates logic.
- Runtime session serialization: useful for restoration but explicitly outside V1.
- `notify` for config changes: event-driven and efficient, but adds platform-specific watcher behavior for one small file. Reconsider it if polling becomes costly or the watched configuration set grows.

## Consequences

Configuration remains inspectable and behavior has a single invocation path. Hot-reload validation and command routing require careful error reporting and tests. The exact configuration schema and reloadable field set will be introduced incrementally.
