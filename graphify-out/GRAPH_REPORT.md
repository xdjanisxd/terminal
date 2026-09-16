# Graph Report - terminal  (2026-09-16)

## Corpus Check
- cluster-only mode — file stats not available

## Summary
- 736 nodes · 1011 edges · 79 communities (46 shown, 22 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `fc083005`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- PendingReplies
- SemanticPerformer
- ScreenGrid
- TerminalDimensions
- TerminalState
- CellAttributes
- da1_parser.rs
- assert_default_cell
- TerminalModes
- region_ind_ri.rs
- HorizontalTabStops
- region_su_sd_parser.rs
- Architecture
- truecolor.rs
- .dimensions
- assert_observable_state_eq
- region_su_sd.rs
- scrolling_margins.rs
- rendition.rs
- region_nel_parser.rs
- Roadmap
- decckm_parser.rs
- decstbm.rs
- VerticalScrollingMargins
- src/state.rs
- labeled_scroll_state
- region_ind_ri_parser.rs
- Conventions
- Engineering Workflow
- terminal-core/src/lib.rs
- .print_character
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
- row_text
- Option
- Default
- Display
- Error
- Formatter
- Option
- Result
- Self
- Cursor
- CursorError
- TerminalDimensions
- VerticalScrollingMargins
- String
- String
- terminal-app
- terminal-config
- terminal-core
- terminal-platform
- terminal-pty
- terminal-renderer
- terminal-workspace

## God Nodes (most connected - your core abstractions)
1. `TerminalState` - 75 edges
2. `ScreenGrid` - 23 edges
3. `CellAttributes` - 22 edges
4. `assert_observable_state_eq()` - 14 edges
5. `Architecture` - 12 edges
6. `parse_in_chunks()` - 11 edges
7. `TerminalModes` - 10 edges
8. `HorizontalTabStops` - 10 edges
9. `PendingReplies` - 10 edges
10. `TerminalReply` - 10 edges

## Surprising Connections (you probably didn't know these)
- `labeled_state()` --references--> `TerminalState`  [EXTRACTED]
  crates/terminal-core/tests/region_ind_ri.rs → crates/terminal-core/src/state.rs
- `row_text()` --references--> `TerminalState`  [EXTRACTED]
  crates/terminal-core/tests/region_ind_ri.rs → crates/terminal-core/src/state.rs
- `rows()` --references--> `TerminalState`  [EXTRACTED]
  crates/terminal-core/tests/region_ind_ri.rs → crates/terminal-core/src/state.rs
- `labeled_state()` --references--> `TerminalState`  [EXTRACTED]
  crates/terminal-core/tests/region_su_sd_parser.rs → crates/terminal-core/src/state.rs
- `parse()` --references--> `TerminalState`  [EXTRACTED]
  crates/terminal-core/tests/region_su_sd_parser.rs → crates/terminal-core/src/state.rs

## Import Cycles
- None detected.

## Communities (79 total, 22 thin omitted)

### Community 2 - "PendingReplies"
Cohesion: 0.07
Nodes (17): PendingReplies, Default, Option, Self, TerminalReply, TerminalReplyBytes, Option, cpr_is_chunk_safe_at_every_input_boundary_and_across_calls() (+9 more)

### Community 3 - "SemanticPerformer"
Cohesion: 0.08
Nodes (19): default_one(), ExtendedColorChannel, TerminalState, SemanticPerformer, SemanticPerformer<'a>, simple_csi_parameters(), TerminalParser, TerminalParserError (+11 more)

### Community 4 - "ScreenGrid"
Cohesion: 0.13
Nodes (18): Cell, labeled_grid(), region_scroll_clamps_counts_and_handles_one_row_regions(), region_scroll_preserves_complete_cells_and_uses_default_exposed_rows(), region_scroll_supports_ranges_touching_either_screen_edge(), row_text(), Cursor, CursorError (+10 more)

### Community 5 - "TerminalDimensions"
Cohesion: 0.09
Nodes (16): Cursor, CursorError, offset_clamped(), Display, Error, Formatter, Result, Self (+8 more)

### Community 6 - "TerminalState"
Cohesion: 0.08
Nodes (12): AutoWrapMode, CellAttributes, CharacterInsertionMode, TerminalState, CursorKeyMode, CursorVisibility, InputModes, InverseVideo (+4 more)

### Community 7 - "CellAttributes"
Cohesion: 0.14
Nodes (9): Cell, CellAttributes, CellColor, InverseVideo, ItalicStyle, Default, Self, TextIntensity (+1 more)

### Community 8 - "da1_parser.rs"
Cohesion: 0.16
Nodes (21): da1_queries_are_chunk_safe_at_every_input_boundary(), incomplete_malformed_private_and_non_primary_da_forms_do_not_reply(), multiple_queries_in_one_stream_and_separate_calls_are_preserved(), parser_csi_c_and_csi_zero_c_produce_exact_da1_reply(), parser_da1_at_capacity_is_bounded_and_does_not_overwrite_pending_replies(), printable_text_neighbors_da1_without_becoming_reply_input(), reply_bytes(), row_text() (+13 more)

### Community 10 - "assert_default_cell"
Cohesion: 0.17
Nodes (16): assert_default_cell(), bottom_index_scrolls_styled_cells_and_preserves_terminal_state(), bottom_next_line_scrolls_up_and_preserves_rendition_and_modes(), erase_in_display_supports_all_directions_without_moving_the_cursor(), erase_in_line_supports_all_directions_without_moving_the_cursor(), filled_state(), full_screen_scroll_down_normalizes_count_to_screen_height(), full_screen_scroll_moves_complete_cells_and_preserves_rendition_and_modes() (+8 more)

### Community 11 - "TerminalModes"
Cohesion: 0.25
Nodes (6): AutoWrapMode, CharacterInsertionMode, CursorKeyMode, CursorVisibility, InputModes, TerminalModes

### Community 12 - "region_ind_ri.rs"
Cohesion: 0.27
Nodes (12): crossing_into_region_by_index_or_reverse_index_does_not_scroll(), full_screen_margins_preserve_previous_index_and_reverse_index_behavior(), index_and_reverse_index_clear_only_a_single_row_region(), index_and_reverse_index_move_outside_region_without_scrolling_or_clamping(), index_moves_inside_region_and_scrolls_only_at_bottom_margin(), labeled_state(), region_boundary_scrolling_preserves_styled_cells_and_unrelated_state(), reverse_index_moves_inside_region_and_scrolls_only_at_top_margin() (+4 more)

### Community 14 - "HorizontalTabStops"
Cohesion: 0.17
Nodes (4): HorizontalTabStops, Option, Self, Vec

### Community 15 - "region_su_sd_parser.rs"
Cohesion: 0.27
Nodes (11): decstbm_followed_by_su_or_sd_uses_active_region_and_default_count(), labeled_state(), malformed_extra_and_subparameter_shapes_remain_safe_no_ops_with_margins(), parse(), parser_region_su_sd_are_safe_across_every_input_split(), parser_su_sd_preserve_active_sgr_and_moved_cell_attributes(), printable_text_neighbors_region_scroll_without_parser_side_mutation(), row_text() (+3 more)

### Community 16 - "Architecture"
Cohesion: 0.15
Nodes (12): Architecture, Commands and workspaces, Conceptual components, Dependency direction, Initial terminal-core model invariants, Mode ownership, Parser boundary, Resource and trust boundaries (+4 more)

### Community 17 - "truecolor.rs"
Cohesion: 0.24
Nodes (9): CellColor, malformed_truecolor_groups_are_atomic_and_colon_forms_stay_unsupported(), rgb(), TerminalState, state(), truecolor_cells_snapshot_and_move_with_complete_attributes(), truecolor_foreground_and_background_are_exact_and_preserve_styles(), truecolor_is_chunk_safe_and_preserves_unrelated_terminal_state_and_replies() (+1 more)

### Community 18 - ".dimensions"
Cohesion: 0.24
Nodes (5): reset_clears_existing_screen_content(), resize_preserves_overlap_and_blanks_new_cells_through_facade(), Self, CursorError, TerminalDimensions

### Community 19 - "assert_observable_state_eq"
Cohesion: 0.30
Nodes (12): assert_observable_state_eq(), byte_at_a_time_matches_one_shot_input(), every_split_point_matches_one_shot_input(), ind_ri_nel_are_chunk_safe_at_every_byte_boundary(), indexed_color_and_malformed_groups_are_chunk_safe_at_every_boundary(), many_chunk_sizes_match_one_shot_input(), parse_in_chunks(), parser_mode_dispatch_is_chunk_safe_at_every_byte_boundary() (+4 more)

### Community 20 - "region_su_sd.rs"
Cohesion: 0.27
Nodes (10): full_screen_margins_keep_established_su_sd_behavior(), labeled_state(), region_scroll_handles_one_row_and_regions_at_screen_edges(), region_scroll_preserves_complete_cells_and_unrelated_terminal_state(), region_scroll_zero_is_no_op_and_large_counts_clear_only_active_region(), rows(), String, Vec (+2 more)

### Community 21 - "scrolling_margins.rs"
Cohesion: 0.21
Nodes (5): custom_margins_bound_su_sd_and_nel(), labeled_state(), row_text(), String, setting_margins_does_not_change_existing_cells()

### Community 23 - "rendition.rs"
Cohesion: 0.33
Nodes (9): current_rendition_uses_documented_defaults(), foreground_and_background_channels_change_independently(), inverse_video_can_be_set_and_cleared_independently(), italic_can_be_set_and_cleared_independently(), printed_cells_snapshot_the_current_rendition_without_retroactive_changes(), state(), terminal_reset_restores_rendition_defaults(), text_intensity_can_be_set_and_cleared_independently() (+1 more)

### Community 24 - "region_nel_parser.rs"
Cohesion: 0.38
Nodes (9): assert_observable_state_eq(), incomplete_and_malformed_nel_escapes_keep_safe_existing_behavior(), labeled_state(), parse(), parser_region_nel_is_chunk_safe_at_every_input_boundary(), parser_routes_decstbm_followed_by_nel_through_active_region(), printable_text_before_and_after_nel_uses_region_semantics(), row_text() (+1 more)

### Community 25 - "Roadmap"
Cohesion: 0.20
Nodes (9): Early integration checkpoint, M0: Repository foundation, M1: Terminal core foundation, M2: Terminal compatibility, M3: PTY and session lifecycle, M4: Window and renderer, M5: Input and configuration, M6: Workspaces and release readiness (+1 more)

### Community 26 - "decckm_parser.rs"
Cohesion: 0.25
Nodes (4): decckm_changes_only_cursor_key_mode_and_preserves_terminal_state(), row_text(), String, TerminalState

### Community 27 - "decstbm.rs"
Cohesion: 0.28
Nodes (3): decstbm_accepts_full_explicit_and_partial_default_forms(), decstbm_defaults_remain_valid_on_a_single_row_screen(), parse()

### Community 28 - "VerticalScrollingMargins"
Cohesion: 0.29
Nodes (3): Option, Self, VerticalScrollingMargins

### Community 29 - "src/state.rs"
Cohesion: 0.29
Nodes (5): clear_screen_clears_owned_screen_content(), CursorMovement, EraseDirection, EraseRegion, HorizontalTabStops

### Community 30 - "labeled_scroll_state"
Cohesion: 0.25
Nodes (8): incomplete_and_malformed_index_controls_do_not_mutate_unrelated_state(), labeled_scroll_state(), malformed_or_extra_su_sd_parameters_are_safe_no_ops(), parser_dispatches_index_reverse_index_and_next_line(), parser_sd_uses_one_for_omitted_and_zero_counts(), parser_su_and_sd_clamp_explicit_and_oversized_counts(), parser_su_uses_one_for_omitted_and_zero_counts(), su_sd_sequences_are_chunk_safe_at_every_byte_boundary()

### Community 31 - "region_ind_ri_parser.rs"
Cohesion: 0.36
Nodes (6): parse(), parser_index_and_reverse_index_with_margins_are_chunk_safe(), parser_region_scrolls_preserve_printable_neighbors(), parser_routes_decstbm_followed_by_index_and_reverse_index(), row_text(), String

### Community 32 - "Conventions"
Cohesion: 0.25
Nodes (7): Conventions, Dependencies, Documentation, Engineering, Errors, logging, and unsafe code, Required local gates, Testing

### Community 33 - "Engineering Workflow"
Cohesion: 0.29
Nodes (6): Before implementation, Completion gates, Context loading, Engineering Workflow, Implementation, Reporting

### Community 34 - "terminal-core/src/lib.rs"
Cohesion: 0.29
Nodes (4): Cursor, Parser, ScreenGrid, VerticalScrollingMargins

### Community 35 - ".print_character"
Cohesion: 0.33
Nodes (5): PrintError, Display, Error, Formatter, Result

### Community 36 - "Terminal Workspace Application"
Cohesion: 0.29
Nodes (6): Current validation, License, Priorities, Repository map, Target platforms, Terminal Workspace Application

### Community 37 - "Session Handoff"
Cohesion: 0.33
Nodes (5): Current state, Important limitations, Next, Session Handoff, Validation

### Community 38 - "Current State"
Cohesion: 0.33
Nodes (5): Current State, Known issues, Missing, Partial, Working

### Community 39 - "Rust and Cargo Workspace"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, Rust and Cargo Workspace

### Community 40 - "Component Boundaries and Dependency Direction"
Cohesion: 0.33
Nodes (5): Alternatives, Component Boundaries and Dependency Direction, Consequences, Context, Decision

### Community 41 - "Native Window, GPU Rendering, and Font Stack"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, Native Window, GPU Rendering, and Font Stack

### Community 42 - "VTE Parsing with Project-Owned Terminal State"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, VTE Parsing with Project-Owned Terminal State

### Community 43 - "PTY Boundary, Process Model, and Threading"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, PTY Boundary, Process Model, and Threading

### Community 44 - "Declarative Configuration, Central Commands, and Workspaces"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, Declarative Configuration, Central Commands, and Workspaces

### Community 45 - "V1 Scope Boundaries"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, V1 Scope Boundaries

### Community 46 - "License, Internal Naming, and Target Architectures"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, License, Internal Naming, and Target Architectures

### Community 47 - "Product"
Cohesion: 0.33
Nodes (5): Goal, Priorities, Product, V1 non-goals, V1 scope

### Community 48 - "Engineering Agent Contract"
Cohesion: 0.40
Nodes (4): Context efficiency, Engineering Agent Contract, Non-negotiable boundaries, Start every engineering session

### Community 49 - "Tasks"
Cohesion: 0.40
Nodes (4): Deferred by scope, Later, Next, Tasks

### Community 50 - "Performance"
Cohesion: 0.40
Nodes (4): Initial goals, Measurements to add with implementations, Performance, Rules

## Knowledge Gaps
- **95 isolated node(s):** `Commands and workspaces`, `Conceptual components`, `Dependency direction`, `Initial terminal-core model invariants`, `Mode ownership` (+90 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 375 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **22 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `TerminalState` connect `TerminalState` to `PendingReplies`, `.print_character`, `terminal-core/src/lib.rs`, `da1_parser.rs`, `region_ind_ri.rs`, `.cursor`, `region_su_sd_parser.rs`, `truecolor.rs`, `.dimensions`, `region_su_sd.rs`, `scrolling_margins.rs`, `rendition.rs`, `region_nel_parser.rs`, `decstbm.rs`, `src/state.rs`, `region_ind_ri_parser.rs`?**
  _High betweenness centrality (0.223) - this node is a cross-community bridge._
- **Why does `row_text()` connect `row_text` to `tests/parser.rs`, `da1_parser.rs`, `assert_observable_state_eq`?**
  _High betweenness centrality (0.089) - this node is a cross-community bridge._
- **Why does `row_text()` connect `da1_parser.rs` to `TerminalState`?**
  _High betweenness centrality (0.075) - this node is a cross-community bridge._
- **What connects `Commands and workspaces`, `Conceptual components`, `Dependency direction` to the rest of the system?**
  _95 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `tests/parser.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.04081632653061224 - nodes in this community are weakly interconnected._
- **Should `tests/state.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.04081632653061224 - nodes in this community are weakly interconnected._
- **Should `PendingReplies` be split into smaller, more focused modules?**
  _Cohesion score 0.06707317073170732 - nodes in this community are weakly interconnected._