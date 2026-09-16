# Graph Report - terminal  (2026-09-15)

## Corpus Check
- cluster-only mode — file stats not available

## Summary
- 603 nodes · 815 edges · 58 communities (37 shown, 11 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `ddd7b4ce`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- TerminalState
- TerminalDimensions
- SemanticPerformer
- ScreenGrid
- CellAttributes
- TerminalModes
- region_ind_ri.rs
- HorizontalTabStops
- Architecture
- scrolling_margins.rs
- assert_observable_state_eq
- rendition.rs
- Roadmap
- decstbm.rs
- labeled_scroll_state
- region_ind_ri_parser.rs
- assert_default_cell
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
- filled_state
- row_text
- row_text
- Option
- Display
- Error
- Formatter
- terminal-app
- terminal-config
- terminal-core
- terminal-platform
- terminal-pty
- terminal-renderer
- terminal-workspace

## God Nodes (most connected - your core abstractions)
1. `TerminalState` - 68 edges
2. `ScreenGrid` - 27 edges
3. `CellAttributes` - 22 edges
4. `assert_observable_state_eq()` - 14 edges
5. `Architecture` - 12 edges
6. `parse_in_chunks()` - 11 edges
7. `HorizontalTabStops` - 10 edges
8. `state()` - 10 edges
9. `TerminalDimensions` - 10 edges
10. `SemanticPerformer` - 10 edges

## Surprising Connections (you probably didn't know these)
- `SemanticPerformer` --references--> `TerminalState`  [EXTRACTED]
  crates/terminal-core/src/parser.rs → crates/terminal-core/src/state.rs
- `TerminalState` --references--> `ScreenGrid`  [EXTRACTED]
  crates/terminal-core/src/state.rs → crates/terminal-core/src/grid.rs
- `parse()` --references--> `TerminalState`  [EXTRACTED]
  crates/terminal-core/tests/decstbm.rs → crates/terminal-core/src/state.rs
- `assert_observable_state_eq()` --references--> `TerminalState`  [EXTRACTED]
  crates/terminal-core/tests/parser.rs → crates/terminal-core/src/state.rs
- `labeled_scroll_state()` --references--> `TerminalState`  [EXTRACTED]
  crates/terminal-core/tests/parser.rs → crates/terminal-core/src/state.rs

## Import Cycles
- None detected.

## Communities (58 total, 11 thin omitted)

### Community 0 - "TerminalState"
Cohesion: 0.05
Nodes (27): AutoWrapMode, CellAttributes, CellColor, CharacterInsertionMode, clear_screen_clears_owned_screen_content(), CursorMovement, EraseDirection, EraseRegion (+19 more)

### Community 3 - "TerminalDimensions"
Cohesion: 0.06
Nodes (20): Cursor, CursorError, offset_clamped(), Display, Error, Formatter, Result, Self (+12 more)

### Community 4 - "SemanticPerformer"
Cohesion: 0.08
Nodes (21): default_one(), ExtendedColorChannel, Default, Display, Error, Formatter, Option, Result (+13 more)

### Community 5 - "ScreenGrid"
Cohesion: 0.12
Nodes (18): Cell, labeled_grid(), region_scroll_clamps_counts_and_handles_one_row_regions(), region_scroll_preserves_complete_cells_and_uses_default_exposed_rows(), region_scroll_supports_ranges_touching_either_screen_edge(), row_text(), Cursor, CursorError (+10 more)

### Community 6 - "CellAttributes"
Cohesion: 0.14
Nodes (9): Cell, CellAttributes, CellColor, InverseVideo, ItalicStyle, Default, Self, TextIntensity (+1 more)

### Community 8 - "TerminalModes"
Cohesion: 0.25
Nodes (6): AutoWrapMode, CharacterInsertionMode, CursorKeyMode, CursorVisibility, InputModes, TerminalModes

### Community 9 - "region_ind_ri.rs"
Cohesion: 0.27
Nodes (12): crossing_into_region_by_index_or_reverse_index_does_not_scroll(), full_screen_margins_preserve_previous_index_and_reverse_index_behavior(), index_and_reverse_index_clear_only_a_single_row_region(), index_and_reverse_index_move_outside_region_without_scrolling_or_clamping(), index_moves_inside_region_and_scrolls_only_at_bottom_margin(), labeled_state(), region_boundary_scrolling_preserves_styled_cells_and_unrelated_state(), reverse_index_moves_inside_region_and_scrolls_only_at_top_margin() (+4 more)

### Community 10 - "HorizontalTabStops"
Cohesion: 0.17
Nodes (4): HorizontalTabStops, Option, Self, Vec

### Community 11 - "Architecture"
Cohesion: 0.15
Nodes (12): Architecture, Commands and workspaces, Conceptual components, Dependency direction, Initial terminal-core model invariants, Mode ownership, Parser boundary, Resource and trust boundaries (+4 more)

### Community 12 - "scrolling_margins.rs"
Cohesion: 0.21
Nodes (5): custom_margins_do_not_change_full_screen_su_sd_or_nel(), labeled_state(), row_text(), String, setting_margins_does_not_change_existing_cells()

### Community 14 - "assert_observable_state_eq"
Cohesion: 0.33
Nodes (11): assert_observable_state_eq(), byte_at_a_time_matches_one_shot_input(), every_split_point_matches_one_shot_input(), ind_ri_nel_are_chunk_safe_at_every_byte_boundary(), indexed_color_and_malformed_groups_are_chunk_safe_at_every_boundary(), many_chunk_sizes_match_one_shot_input(), parse_in_chunks(), parser_mode_dispatch_is_chunk_safe_at_every_byte_boundary() (+3 more)

### Community 15 - "rendition.rs"
Cohesion: 0.33
Nodes (9): current_rendition_uses_documented_defaults(), foreground_and_background_channels_change_independently(), inverse_video_can_be_set_and_cleared_independently(), italic_can_be_set_and_cleared_independently(), printed_cells_snapshot_the_current_rendition_without_retroactive_changes(), state(), terminal_reset_restores_rendition_defaults(), text_intensity_can_be_set_and_cleared_independently() (+1 more)

### Community 16 - "Roadmap"
Cohesion: 0.20
Nodes (9): Early integration checkpoint, M0: Repository foundation, M1: Terminal core foundation, M2: Terminal compatibility, M3: PTY and session lifecycle, M4: Window and renderer, M5: Input and configuration, M6: Workspaces and release readiness (+1 more)

### Community 17 - "decstbm.rs"
Cohesion: 0.28
Nodes (3): decstbm_accepts_full_explicit_and_partial_default_forms(), decstbm_defaults_remain_valid_on_a_single_row_screen(), parse()

### Community 18 - "labeled_scroll_state"
Cohesion: 0.25
Nodes (8): incomplete_and_malformed_index_controls_do_not_mutate_unrelated_state(), labeled_scroll_state(), malformed_or_extra_su_sd_parameters_are_safe_no_ops(), parser_dispatches_index_reverse_index_and_next_line(), parser_sd_uses_one_for_omitted_and_zero_counts(), parser_su_and_sd_clamp_explicit_and_oversized_counts(), parser_su_uses_one_for_omitted_and_zero_counts(), su_sd_sequences_are_chunk_safe_at_every_byte_boundary()

### Community 19 - "region_ind_ri_parser.rs"
Cohesion: 0.36
Nodes (6): parse(), parser_index_and_reverse_index_with_margins_are_chunk_safe(), parser_region_scrolls_preserve_printable_neighbors(), parser_routes_decstbm_followed_by_index_and_reverse_index(), row_text(), String

### Community 20 - "assert_default_cell"
Cohesion: 0.32
Nodes (8): assert_default_cell(), bottom_index_scrolls_styled_cells_and_preserves_terminal_state(), bottom_next_line_scrolls_up_and_preserves_rendition_and_modes(), erase_in_display_supports_all_directions_without_moving_the_cursor(), full_screen_scroll_moves_complete_cells_and_preserves_rendition_and_modes(), scroll_down_moves_attributes_and_clears_new_top_rows_with_default_cells(), styled_control_state(), top_reverse_index_scrolls_styled_cells_and_clears_default_top_row()

### Community 21 - "Conventions"
Cohesion: 0.25
Nodes (7): Conventions, Dependencies, Documentation, Engineering, Errors, logging, and unsafe code, Required local gates, Testing

### Community 22 - "Engineering Workflow"
Cohesion: 0.29
Nodes (6): Before implementation, Completion gates, Context loading, Engineering Workflow, Implementation, Reporting

### Community 23 - "Terminal Workspace Application"
Cohesion: 0.29
Nodes (6): Current validation, License, Priorities, Repository map, Target platforms, Terminal Workspace Application

### Community 24 - "Session Handoff"
Cohesion: 0.33
Nodes (5): Current state, Important limitations, Next, Session Handoff, Validation

### Community 25 - "Current State"
Cohesion: 0.33
Nodes (5): Current State, Known issues, Missing, Partial, Working

### Community 26 - "Rust and Cargo Workspace"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, Rust and Cargo Workspace

### Community 27 - "Component Boundaries and Dependency Direction"
Cohesion: 0.33
Nodes (5): Alternatives, Component Boundaries and Dependency Direction, Consequences, Context, Decision

### Community 28 - "Native Window, GPU Rendering, and Font Stack"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, Native Window, GPU Rendering, and Font Stack

### Community 29 - "VTE Parsing with Project-Owned Terminal State"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, VTE Parsing with Project-Owned Terminal State

### Community 30 - "PTY Boundary, Process Model, and Threading"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, PTY Boundary, Process Model, and Threading

### Community 31 - "Declarative Configuration, Central Commands, and Workspaces"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, Declarative Configuration, Central Commands, and Workspaces

### Community 32 - "V1 Scope Boundaries"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, V1 Scope Boundaries

### Community 33 - "License, Internal Naming, and Target Architectures"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, License, Internal Naming, and Target Architectures

### Community 34 - "Product"
Cohesion: 0.33
Nodes (5): Goal, Priorities, Product, V1 non-goals, V1 scope

### Community 35 - "Engineering Agent Contract"
Cohesion: 0.40
Nodes (4): Context efficiency, Engineering Agent Contract, Non-negotiable boundaries, Start every engineering session

### Community 36 - "Tasks"
Cohesion: 0.40
Nodes (4): Deferred by scope, Later, Next, Tasks

### Community 37 - "Performance"
Cohesion: 0.40
Nodes (4): Initial goals, Measurements to add with implementations, Performance, Rules

### Community 38 - "filled_state"
Cohesion: 0.50
Nodes (4): erase_in_line_supports_all_directions_without_moving_the_cursor(), filled_state(), full_screen_scroll_down_normalizes_count_to_screen_height(), full_screen_scroll_up_normalizes_count_to_screen_height()

### Community 39 - "row_text"
Cohesion: 0.67
Nodes (3): row_text(), String, successive_mode_sequences_preserve_unrelated_state()

### Community 40 - "row_text"
Cohesion: 0.67
Nodes (3): horizontal_tab_resolves_delayed_wrap_without_modifying_cells_or_modes(), row_text(), String

## Knowledge Gaps
- **95 isolated node(s):** `Commands and workspaces`, `Conceptual components`, `Dependency direction`, `Initial terminal-core model invariants`, `Mode ownership` (+90 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 333 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **11 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `TerminalState` connect `TerminalState` to `SemanticPerformer`, `ScreenGrid`, `filled_state`, `row_text`, `row_text`, `region_ind_ri.rs`, `scrolling_margins.rs`, `assert_observable_state_eq`, `rendition.rs`, `decstbm.rs`, `labeled_scroll_state`, `region_ind_ri_parser.rs`, `assert_default_cell`?**
  _High betweenness centrality (0.369) - this node is a cross-community bridge._
- **Why does `ScreenGrid` connect `ScreenGrid` to `TerminalState`, `TerminalDimensions`?**
  _High betweenness centrality (0.176) - this node is a cross-community bridge._
- **Why does `Cell` connect `CellAttributes` to `TerminalDimensions`?**
  _High betweenness centrality (0.054) - this node is a cross-community bridge._
- **What connects `Commands and workspaces`, `Conceptual components`, `Dependency direction` to the rest of the system?**
  _95 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `TerminalState` be split into smaller, more focused modules?**
  _Cohesion score 0.052429667519181586 - nodes in this community are weakly interconnected._
- **Should `tests/parser.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.04081632653061224 - nodes in this community are weakly interconnected._
- **Should `tests/state.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.041666666666666664 - nodes in this community are weakly interconnected._