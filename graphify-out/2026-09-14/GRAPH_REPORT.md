# Graph Report - terminal  (2026-09-14)

## Corpus Check
- 47 files · ~17,510 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 500 nodes · 660 edges · 48 communities (31 shown, 7 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `771dcad0`
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
- labeled_scroll_state

## God Nodes (most connected - your core abstractions)
1. `TerminalState` - 53 edges
2. `CellAttributes` - 24 edges
3. `ScreenGrid` - 23 edges
4. `TerminalDimensions` - 17 edges
5. `Cell` - 12 edges
6. `TerminalModes` - 12 edges
7. `HorizontalTabStops` - 12 edges
8. `assert_observable_state_eq()` - 12 edges
9. `Architecture` - 12 edges
10. `Cursor` - 10 edges

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

## Communities (48 total, 7 thin omitted)

### Community 0 - "ScreenGrid"
Cohesion: 0.05
Nodes (25): Cell, Default, Self, Cursor, CursorError, offset_clamped(), Display, Error (+17 more)

### Community 3 - "TerminalState"
Cohesion: 0.14
Nodes (9): clear_screen_clears_owned_screen_content(), CursorMovement, EraseDirection, EraseRegion, reset_clears_existing_screen_content(), resize_preserves_overlap_and_blanks_new_cells_through_facade(), Result, Self (+1 more)

### Community 4 - "SemanticPerformer"
Cohesion: 0.07
Nodes (23): default_one(), ExtendedColorChannel, Default, Display, Error, Formatter, Option, Result (+15 more)

### Community 5 - "CellAttributes"
Cohesion: 0.12
Nodes (6): CellAttributes, CellColor, InverseVideo, ItalicStyle, TextIntensity, UnderlineStyle

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
Cohesion: 0.36
Nodes (10): assert_observable_state_eq(), byte_at_a_time_matches_one_shot_input(), every_split_point_matches_one_shot_input(), indexed_color_and_malformed_groups_are_chunk_safe_at_every_boundary(), many_chunk_sizes_match_one_shot_input(), parse_in_chunks(), parser_mode_dispatch_is_chunk_safe_at_every_byte_boundary(), parser_tab_sequences_are_chunk_safe_at_every_byte_boundary() (+2 more)

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
Cohesion: 0.29
Nodes (8): assert_default_cell(), erase_in_display_supports_all_directions_without_moving_the_cursor(), erase_in_line_supports_all_directions_without_moving_the_cursor(), filled_state(), full_screen_scroll_down_normalizes_count_to_screen_height(), full_screen_scroll_moves_complete_cells_and_preserves_rendition_and_modes(), full_screen_scroll_up_normalizes_count_to_screen_height(), scroll_down_moves_attributes_and_clears_new_top_rows_with_default_cells()

### Community 32 - "row_text"
Cohesion: 0.67
Nodes (3): row_text(), String, successive_mode_sequences_preserve_unrelated_state()

### Community 33 - "row_text"
Cohesion: 0.67
Nodes (3): horizontal_tab_resolves_delayed_wrap_without_modifying_cells_or_modes(), row_text(), String

### Community 47 - "labeled_scroll_state"
Cohesion: 0.33
Nodes (6): labeled_scroll_state(), malformed_or_extra_su_sd_parameters_are_safe_no_ops(), parser_sd_uses_one_for_omitted_and_zero_counts(), parser_su_and_sd_clamp_explicit_and_oversized_counts(), parser_su_uses_one_for_omitted_and_zero_counts(), su_sd_sequences_are_chunk_safe_at_every_byte_boundary()

## Knowledge Gaps
- **95 isolated node(s):** `terminal-app`, `terminal-config`, `terminal-platform`, `terminal-renderer`, `terminal-core` (+90 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 292 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **7 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `TerminalState` connect `TerminalState` to `ScreenGrid`, `row_text`, `row_text`, `SemanticPerformer`, `CellAttributes`, `TerminalModes`, `HorizontalTabStops`, `rendition.rs`, `assert_observable_state_eq`, `labeled_scroll_state`, `assert_default_cell`?**
  _High betweenness centrality (0.355) - this node is a cross-community bridge._
- **Why does `ScreenGrid` connect `ScreenGrid` to `TerminalState`?**
  _High betweenness centrality (0.075) - this node is a cross-community bridge._
- **Why does `CellAttributes` connect `CellAttributes` to `ScreenGrid`, `TerminalState`?**
  _High betweenness centrality (0.051) - this node is a cross-community bridge._
- **What connects `terminal-app`, `terminal-config`, `terminal-platform` to the rest of the system?**
  _95 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `ScreenGrid` be split into smaller, more focused modules?**
  _Cohesion score 0.05028248587570622 - nodes in this community are weakly interconnected._
- **Should `tests/state.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.044444444444444446 - nodes in this community are weakly interconnected._
- **Should `tests/parser.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.041666666666666664 - nodes in this community are weakly interconnected._