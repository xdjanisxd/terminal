# Product

## Goal

Build a lightweight, fast, keyboard-first, GPU-accelerated terminal emulator and workspace application for Windows, Linux, and macOS. It is a terminal and workspace tool, not an IDE.

## Priorities

1. Correct terminal behavior
2. Low startup latency
3. Low unnecessary CPU usage
4. Cross-platform consistency
5. Simple and maintainable architecture
6. Keyboard-first usability
7. Minimal unnecessary dependencies
8. Configurability

## V1 scope

- Modern ANSI/VT terminal behavior, Unicode, true color, mouse protocols, and compatibility with modern shells and TUIs
- Configurable bounded scrollback, selection, clipboard, OSC 52 policy, clickable URLs, and clickable file paths
- Tabs, horizontal and vertical splits, pane focus, and optional mouse resizing
- A centralized command system used by keyboard shortcuts and a command palette
- Declarative TOML configuration, themes, font fallback, Nerd Fonts, optional ligatures, and safe partial hot reload
- Reproducible project-aware workspace definitions with roots, tabs, pane layouts, and startup commands
- Basic CLI control

## V1 non-goals

- Plugin system or marketplace
- Embedded AI
- SSH manager
- File explorer, Git UI, editor, or Docker UI
- Runtime session restoration
- Large scripting runtime
- Electron, WebView UI, React, Qt, or another large GUI framework

Workspace definitions describe reproducible startup configuration; they are not runtime session persistence.
