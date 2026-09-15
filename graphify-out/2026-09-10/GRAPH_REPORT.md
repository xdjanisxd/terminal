# Graph Report - terminal  (2026-09-10)

## Corpus Check
- 47 files · ~15,660 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 472 nodes · 611 edges · 47 communities (30 shown, 7 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `0bf84bea`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- ScreenGrid
- TerminalState
- SemanticPerformer
- CellAttributes
- TerminalModes
- HorizontalTabStops
- Architecture
- rendition.rs
- Roadmap
- assert_observable_state_eq
- Conventions
- Engineering Workflow
- Terminal Workspace Application
- Session Handoff
- Current State
- Rust and Cargo Workspace
- Component Boundaries and Dependency Direction
- Native Window, GPU Rendering, and Font Stack
- VTE Parsing with Project-Owned Terminal State
- PTY Boundary, Process Model, and Threading
- Declarative Configuration, Central Commands, and Workspaces
- V1 Scope Boundaries
- License, Internal Naming, and Target Architectures
- Product
- Engineering Agent Contract
- Tasks
- Performance
- assert_default_cell
- row_text
- row_text
- terminal-app
- terminal-config
- terminal-core
- terminal-platform
- terminal-pty
- terminal-renderer
- terminal-workspace

## God Nodes (most connected - your core abstractions)
1. `TerminalState` - 50 edges
2. `CellAttributes` - 24 edges
3. `ScreenGrid` - 21 edges
4. `TerminalDimensions` - 17 edges
5. `Cell` - 12 edges
6. `TerminalModes` - 12 edges
7. `HorizontalTabStops` - 12 edges
8. `Architecture` - 12 edges
9. `Cursor` - 10 edges
10. `SemanticPerformer` - 10 edges

## Surprising Connections (you probably didn't know these)
- `TerminalState` --references--> `CellAttributes`  [EXTRACTED]
  crates/terminal-core/src/state.rs → crates/terminal-core/src/cell.rs
- `TerminalState` --references--> `ScreenGrid`  [EXTRACTED]
  crates/terminal-core/src/state.rs → crates/terminal-core/src/grid.rs
- `TerminalState` --references--> `TerminalModes`  [EXTRACTED]
  crates/terminal-core/src/state.rs → crates/terminal-core/src/modes.rs
- `TerminalState` --references--> `InputModes`  [EXTRACTED]
  crates/terminal-core/src/state.rs → crates/terminal-core/src/modes.rs
- `SemanticPerformer` --references--> `TerminalState`  [EXTRACTED]
  crates/terminal-core/src/parser.rs → crates/terminal-core/src/state.rs

## Import Cycles
- None detected.

## Communities (47 total, 7 thin omitted)

### Community 0 - "ScreenGrid"
Cohesion: 0.05
Nodes (21): Cell, Default, Cursor, offset_clamped(), Formatter, Result, Self, DimensionsError (+13 more)

### Community 3 - "TerminalState"
Cohesion: 0.12
Nodes (13): CursorError, Display, Error, clear_screen_clears_owned_screen_content(), CursorMovement, EraseDirection, EraseRegion, reset_clears_existing_screen_content() (+5 more)

### Community 4 - "SemanticPerformer"
Cohesion: 0.09
Nodes (19): default_one(), Default, Display, Error, Formatter, Option, Result, Self (+11 more)

### Community 5 - "CellAttributes"
Cohesion: 0.11
Nodes (7): CellAttributes, CellColor, InverseVideo, ItalicStyle, Self, TextIntensity, UnderlineStyle

### Community 6 - "TerminalModes"
Cohesion: 0.15
Nodes (6): AutoWrapMode, CharacterInsertionMode, CursorKeyMode, CursorVisibility, InputModes, TerminalModes

### Community 8 - "HorizontalTabStops"
Cohesion: 0.17
Nodes (4): HorizontalTabStops, Option, Self, Vec

### Community 9 - "Architecture"
Cohesion: 0.15
Nodes (12): Architecture, Commands and workspaces, Conceptual components, Dependency direction, Initial terminal-core model invariants, Mode ownership, Parser boundary, Resource and trust boundaries (+4 more)

### Community 11 - "rendition.rs"
Cohesion: 0.33
Nodes (9): current_rendition_uses_documented_defaults(), foreground_and_background_channels_change_independently(), inverse_video_can_be_set_and_cleared_independently(), italic_can_be_set_and_cleared_independently(), printed_cells_snapshot_the_current_rendition_without_retroactive_changes(), state(), terminal_reset_restores_rendition_defaults(), text_intensity_can_be_set_and_cleared_independently() (+1 more)

### Community 12 - "Roadmap"
Cohesion: 0.20
Nodes (9): Early integration checkpoint, M0: Repository foundation, M1: Terminal core foundation, M2: Terminal compatibility, M3: PTY and session lifecycle, M4: Window and renderer, M5: Input and configuration, M6: Workspaces and release readiness (+1 more)

### Community 13 - "assert_observable_state_eq"
Cohesion: 0.39
Nodes (9): assert_observable_state_eq(), byte_at_a_time_matches_one_shot_input(), every_split_point_matches_one_shot_input(), many_chunk_sizes_match_one_shot_input(), parse_in_chunks(), parser_mode_dispatch_is_chunk_safe_at_every_byte_boundary(), parser_tab_sequences_are_chunk_safe_at_every_byte_boundary(), representative_sgr_stream_is_chunk_safe_at_every_byte_boundary() (+1 more)

### Community 14 - "Conventions"
Cohesion: 0.25
Nodes (7): Conventions, Dependencies, Documentation, Engineering, Errors, logging, and unsafe code, Required local gates, Testing

### Community 15 - "Engineering Workflow"
Cohesion: 0.29
Nodes (6): Before implementation, Completion gates, Context loading, Engineering Workflow, Implementation, Reporting

### Community 16 - "Terminal Workspace Application"
Cohesion: 0.29
Nodes (6): Current validation, License, Priorities, Repository map, Target platforms, Terminal Workspace Application

### Community 17 - "Session Handoff"
Cohesion: 0.33
Nodes (5): Current state, Important limitations, Next, Session Handoff, Validation

### Community 18 - "Current State"
Cohesion: 0.33
Nodes (5): Current State, Known issues, Missing, Partial, Working

### Community 19 - "Rust and Cargo Workspace"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, Rust and Cargo Workspace

### Community 20 - "Component Boundaries and Dependency Direction"
Cohesion: 0.33
Nodes (5): Alternatives, Component Boundaries and Dependency Direction, Consequences, Context, Decision

### Community 21 - "Native Window, GPU Rendering, and Font Stack"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, Native Window, GPU Rendering, and Font Stack

### Community 22 - "VTE Parsing with Project-Owned Terminal State"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, VTE Parsing with Project-Owned Terminal State

### Community 23 - "PTY Boundary, Process Model, and Threading"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, PTY Boundary, Process Model, and Threading

### Community 24 - "Declarative Configuration, Central Commands, and Workspaces"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, Declarative Configuration, Central Commands, and Workspaces

### Community 25 - "V1 Scope Boundaries"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, V1 Scope Boundaries

### Community 26 - "License, Internal Naming, and Target Architectures"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, License, Internal Naming, and Target Architectures

### Community 27 - "Product"
Cohesion: 0.33
Nodes (5): Goal, Priorities, Product, V1 non-goals, V1 scope

### Community 28 - "Engineering Agent Contract"
Cohesion: 0.40
Nodes (4): Context efficiency, Engineering Agent Contract, Non-negotiable boundaries, Start every engineering session

### Community 29 - "Tasks"
Cohesion: 0.40
Nodes (4): Deferred by scope, Later, Next, Tasks

### Community 30 - "Performance"
Cohesion: 0.40
Nodes (4): Initial goals, Measurements to add with implementations, Performance, Rules

### Community 31 - "assert_default_cell"
Cohesion: 0.67
Nodes (4): assert_default_cell(), erase_in_display_supports_all_directions_without_moving_the_cursor(), erase_in_line_supports_all_directions_without_moving_the_cursor(), filled_state()

### Community 32 - "row_text"
Cohesion: 0.67
Nodes (3): row_text(), String, successive_mode_sequences_preserve_unrelated_state()

### Community 33 - "row_text"
Cohesion: 0.67
Nodes (3): horizontal_tab_resolves_delayed_wrap_without_modifying_cells_or_modes(), row_text(), String

## Knowledge Gaps
- **95 isolated node(s):** `terminal-app`, `terminal-config`, `terminal-platform`, `terminal-renderer`, `terminal-core` (+90 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 284 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **7 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `TerminalState` connect `TerminalState` to `ScreenGrid`, `row_text`, `row_text`, `SemanticPerformer`, `CellAttributes`, `TerminalModes`, `HorizontalTabStops`, `rendition.rs`, `assert_observable_state_eq`, `assert_default_cell`?**
  _High betweenness centrality (0.333) - this node is a cross-community bridge._
- **Why does `ScreenGrid` connect `ScreenGrid` to `TerminalState`?**
  _High betweenness centrality (0.069) - this node is a cross-community bridge._
- **Why does `CellAttributes` connect `CellAttributes` to `ScreenGrid`, `TerminalState`?**
  _High betweenness centrality (0.052) - this node is a cross-community bridge._
- **What connects `terminal-app`, `terminal-config`, `terminal-platform` to the rest of the system?**
  _95 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `ScreenGrid` be split into smaller, more focused modules?**
  _Cohesion score 0.05370101596516691 - nodes in this community are weakly interconnected._
- **Should `tests/state.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.045454545454545456 - nodes in this community are weakly interconnected._
- **Should `tests/parser.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.05 - nodes in this community are weakly interconnected._