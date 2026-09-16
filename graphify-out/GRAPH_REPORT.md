# Graph Report - terminal  (2026-09-16)

## Corpus Check
- 74 files · ~31,022 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 813 nodes · 1221 edges · 80 communities (59 shown, 11 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `c1cbfb7e`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- SemanticPerformer
- TerminalReplyBytes
- ScreenGrid
- TerminalDimensions
- grid.rs
- CellAttributes
- da1_parser.rs
- TerminalState
- assert_default_cell
- TerminalModes
- region_ind_ri.rs
- HorizontalTabStops
- region_su_sd_parser.rs
- Architecture
- cpr_parser.rs
- insert_delete_lines.rs
- assert_observable_state_eq
- region_su_sd.rs
- scrolling_margins.rs
- insert_delete_lines_parser.rs
- rendition.rs
- CursorError
- region_nel_parser.rs
- Roadmap
- delete_characters.rs
- decckm_parser.rs
- decstbm.rs
- insert_characters.rs
- VerticalScrollingMargins
- insert_characters_parser.rs
- labeled_scroll_state
- region_ind_ri_parser.rs
- Conventions
- Engineering Workflow
- DimensionsError
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
- delete_characters_parser.rs
- filled_state
- row_text
- erase_characters.rs
- erase_characters_parser.rs
- TerminalReply
- cpr.rs
- region_nel.rs
- truecolor.rs
- da1.rs
- dsr_status_parser.rs
- da2.rs
- dsr_status.rs
- terminal-app
- terminal-config
- terminal-core
- terminal-platform
- terminal-pty
- terminal-renderer
- terminal-workspace

## God Nodes (most connected - your core abstractions)
1. `TerminalState` - 122 edges
2. `ScreenGrid` - 28 edges
3. `CellAttributes` - 24 edges
4. `TerminalDimensions` - 17 edges
5. `assert_observable_state_eq()` - 16 edges
6. `Cell` - 13 edges
7. `TerminalReply` - 13 edges
8. `parse_in_chunks()` - 13 edges
9. `TerminalModes` - 12 edges
10. `HorizontalTabStops` - 12 edges

## Surprising Connections (you probably didn't know these)
- `rgb()` --references--> `CellColor`  [EXTRACTED]
  crates/terminal-core/tests/truecolor.rs → crates/terminal-core/src/cell.rs
- `TerminalState` --references--> `CellAttributes`  [EXTRACTED]
  crates/terminal-core/src/state.rs → crates/terminal-core/src/cell.rs
- `ScreenGrid` --references--> `Cell`  [EXTRACTED]
  crates/terminal-core/src/grid.rs → crates/terminal-core/src/cell.rs
- `cells()` --references--> `Cell`  [EXTRACTED]
  crates/terminal-core/tests/region_nel.rs → crates/terminal-core/src/cell.rs
- `ScreenGrid` --references--> `Cursor`  [EXTRACTED]
  crates/terminal-core/src/grid.rs → crates/terminal-core/src/cursor.rs

## Import Cycles
- None detected.

## Communities (80 total, 11 thin omitted)

### Community 2 - "SemanticPerformer"
Cohesion: 0.07
Nodes (22): default_one(), ExtendedColorChannel, Default, Display, Error, Formatter, Option, Result (+14 more)

### Community 4 - "ScreenGrid"
Cohesion: 0.18
Nodes (6): delete_cells_shifts_complete_row_cells_left_and_clamps(), row_text(), Option, String, Vec, ScreenGrid

### Community 5 - "TerminalDimensions"
Cohesion: 0.13
Nodes (6): Cursor, offset_clamped(), Self, TerminalDimensions, NonZeroUsize, Parser

### Community 6 - "grid.rs"
Cohesion: 0.39
Nodes (7): labeled_grid(), region_scroll_clamps_counts_and_handles_one_row_regions(), region_scroll_preserves_complete_cells_and_uses_default_exposed_rows(), region_scroll_supports_ranges_touching_either_screen_edge(), Self, ScrollDirection, scrolls_middle_region_up_and_down_without_touching_outside_rows()

### Community 7 - "CellAttributes"
Cohesion: 0.10
Nodes (9): Cell, CellAttributes, CellColor, InverseVideo, ItalicStyle, Default, Self, TextIntensity (+1 more)

### Community 8 - "da1_parser.rs"
Cohesion: 0.26
Nodes (11): da1_queries_are_chunk_safe_at_every_input_boundary(), incomplete_malformed_private_and_non_primary_da_forms_do_not_reply(), multiple_queries_in_one_stream_and_separate_calls_are_preserved(), parser_csi_c_and_csi_zero_c_produce_exact_da1_reply(), parser_da1_at_capacity_is_bounded_and_does_not_overwrite_pending_replies(), printable_text_neighbors_da1_without_becoming_reply_input(), reply_bytes(), row_text() (+3 more)

### Community 10 - "TerminalState"
Cohesion: 0.10
Nodes (9): clear_screen_clears_owned_screen_content(), CursorMovement, EraseDirection, EraseRegion, reset_clears_existing_screen_content(), resize_preserves_overlap_and_blanks_new_cells_through_facade(), Result, Self (+1 more)

### Community 11 - "assert_default_cell"
Cohesion: 0.32
Nodes (8): assert_default_cell(), bottom_index_scrolls_styled_cells_and_preserves_terminal_state(), bottom_next_line_scrolls_up_and_preserves_rendition_and_modes(), erase_in_display_supports_all_directions_without_moving_the_cursor(), full_screen_scroll_moves_complete_cells_and_preserves_rendition_and_modes(), scroll_down_moves_attributes_and_clears_new_top_rows_with_default_cells(), styled_control_state(), top_reverse_index_scrolls_styled_cells_and_clears_default_top_row()

### Community 12 - "TerminalModes"
Cohesion: 0.15
Nodes (6): AutoWrapMode, CharacterInsertionMode, CursorKeyMode, CursorVisibility, InputModes, TerminalModes

### Community 13 - "region_ind_ri.rs"
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

### Community 17 - "cpr_parser.rs"
Cohesion: 0.43
Nodes (6): cpr_is_chunk_safe_at_every_input_boundary_and_across_calls(), incomplete_or_malformed_dsr_does_not_reply_before_a_complete_valid_query(), multiple_and_interleaved_da1_and_cpr_queries_keep_exact_fifo_order(), reply_bytes(), Vec, state()

### Community 18 - "insert_delete_lines.rs"
Cohesion: 0.29
Nodes (10): delete_lines_is_cursor_relative_within_the_active_region(), insert_lines_is_cursor_relative_within_the_active_region(), labeled_state(), line_operations_are_no_ops_outside_margins_and_use_full_screen_defaults(), line_operations_clamp_to_the_cursor_to_bottom_subregion(), line_operations_leave_margins_intact(), line_operations_preserve_complete_cells_canonical_blanks_and_unrelated_state(), rows() (+2 more)

### Community 19 - "assert_observable_state_eq"
Cohesion: 0.28
Nodes (13): assert_observable_state_eq(), byte_at_a_time_matches_one_shot_input(), cnl_and_cpl_are_chunk_safe_and_preserve_printable_input(), every_split_point_matches_one_shot_input(), ind_ri_nel_are_chunk_safe_at_every_byte_boundary(), indexed_color_and_malformed_groups_are_chunk_safe_at_every_boundary(), many_chunk_sizes_match_one_shot_input(), parse_in_chunks() (+5 more)

### Community 20 - "region_su_sd.rs"
Cohesion: 0.27
Nodes (10): full_screen_margins_keep_established_su_sd_behavior(), labeled_state(), region_scroll_handles_one_row_and_regions_at_screen_edges(), region_scroll_preserves_complete_cells_and_unrelated_terminal_state(), region_scroll_zero_is_no_op_and_large_counts_clear_only_active_region(), rows(), String, Vec (+2 more)

### Community 21 - "scrolling_margins.rs"
Cohesion: 0.21
Nodes (5): custom_margins_bound_su_sd_and_nel(), labeled_state(), row_text(), String, setting_margins_does_not_change_existing_cells()

### Community 22 - "insert_delete_lines_parser.rs"
Cohesion: 0.29
Nodes (9): labeled_state(), parse(), parser_il_dl_are_chunk_invariant_and_incomplete_or_malformed_input_is_safe(), parser_il_dl_clamp_counts_and_preserve_printable_neighbors(), parser_il_dl_preserve_cursor_and_existing_cell_model(), parser_il_dl_respect_cursor_relative_decstbm_subregions(), rows(), String (+1 more)

### Community 24 - "rendition.rs"
Cohesion: 0.30
Nodes (10): current_rendition_uses_documented_defaults(), faint_intensity_is_typed_idempotent_and_preserved_by_resize(), foreground_and_background_channels_change_independently(), inverse_video_can_be_set_and_cleared_independently(), italic_can_be_set_and_cleared_independently(), printed_cells_snapshot_the_current_rendition_without_retroactive_changes(), state(), terminal_reset_restores_rendition_defaults() (+2 more)

### Community 25 - "CursorError"
Cohesion: 0.25
Nodes (6): CursorError, Display, Error, Formatter, Result, Result

### Community 26 - "region_nel_parser.rs"
Cohesion: 0.38
Nodes (9): assert_observable_state_eq(), incomplete_and_malformed_nel_escapes_keep_safe_existing_behavior(), labeled_state(), parse(), parser_region_nel_is_chunk_safe_at_every_input_boundary(), parser_routes_decstbm_followed_by_nel_through_active_region(), printable_text_before_and_after_nel_uses_region_semantics(), row_text() (+1 more)

### Community 27 - "Roadmap"
Cohesion: 0.20
Nodes (9): Early integration checkpoint, M0: Repository foundation, M1: Terminal core foundation, M2: Terminal compatibility, M3: PTY and session lifecycle, M4: Window and renderer, M5: Input and configuration, M6: Workspaces and release readiness (+1 more)

### Community 28 - "delete_characters.rs"
Cohesion: 0.36
Nodes (8): delete_characters_cancels_delayed_wrap_like_ich_and_erase(), delete_characters_is_independent_of_typing_insert_mode_and_preserves_state(), delete_characters_moves_complete_cells_and_fills_with_canonical_blanks(), delete_characters_shifts_only_the_current_row_and_clamps_counts(), insert_and_delete_characters_compose_as_bounded_current_row_edits(), labeled_state(), row(), String

### Community 29 - "decckm_parser.rs"
Cohesion: 0.29
Nodes (3): decckm_changes_only_cursor_key_mode_and_preserves_terminal_state(), row_text(), String

### Community 30 - "decstbm.rs"
Cohesion: 0.28
Nodes (3): decstbm_accepts_full_explicit_and_partial_default_forms(), decstbm_defaults_remain_valid_on_a_single_row_screen(), parse()

### Community 31 - "insert_characters.rs"
Cohesion: 0.36
Nodes (8): insert_characters_cancels_delayed_wrap_like_other_current_row_editing_operations(), insert_characters_does_not_depend_on_scrolling_margins(), insert_characters_is_independent_of_typing_insert_mode_and_preserves_state(), insert_characters_shifts_only_the_current_row_and_clamps_counts(), inserted_character_cells_are_canonical_blanks_and_moved_cells_keep_attributes(), labeled_state(), row(), String

### Community 32 - "VerticalScrollingMargins"
Cohesion: 0.25
Nodes (3): Option, Self, VerticalScrollingMargins

### Community 33 - "insert_characters_parser.rs"
Cohesion: 0.43
Nodes (7): labeled_state(), parse(), parser_dispatches_ich_with_omitted_zero_one_and_multiple_counts(), parser_ich_clamps_at_the_right_edge_and_preserves_printable_neighbors(), parser_ich_is_chunk_invariant_and_incomplete_or_unsupported_shapes_are_safe(), row(), String

### Community 34 - "labeled_scroll_state"
Cohesion: 0.25
Nodes (8): incomplete_and_malformed_index_controls_do_not_mutate_unrelated_state(), labeled_scroll_state(), malformed_or_extra_su_sd_parameters_are_safe_no_ops(), parser_dispatches_index_reverse_index_and_next_line(), parser_sd_uses_one_for_omitted_and_zero_counts(), parser_su_and_sd_clamp_explicit_and_oversized_counts(), parser_su_uses_one_for_omitted_and_zero_counts(), su_sd_sequences_are_chunk_safe_at_every_byte_boundary()

### Community 35 - "region_ind_ri_parser.rs"
Cohesion: 0.36
Nodes (6): parse(), parser_index_and_reverse_index_with_margins_are_chunk_safe(), parser_region_scrolls_preserve_printable_neighbors(), parser_routes_decstbm_followed_by_index_and_reverse_index(), row_text(), String

### Community 36 - "Conventions"
Cohesion: 0.25
Nodes (7): Conventions, Dependencies, Documentation, Engineering, Errors, logging, and unsafe code, Required local gates, Testing

### Community 37 - "Engineering Workflow"
Cohesion: 0.29
Nodes (6): Before implementation, Completion gates, Context loading, Engineering Workflow, Implementation, Reporting

### Community 38 - "DimensionsError"
Cohesion: 0.29
Nodes (6): DimensionsError, Display, Error, Formatter, Result, Self

### Community 39 - "Terminal Workspace Application"
Cohesion: 0.29
Nodes (6): Current validation, License, Priorities, Repository map, Target platforms, Terminal Workspace Application

### Community 40 - "Session Handoff"
Cohesion: 0.33
Nodes (5): Current state, Important limitations, Next, Session Handoff, Validation

### Community 41 - "Current State"
Cohesion: 0.33
Nodes (5): Current State, Known issues, Missing, Partial, Working

### Community 42 - "Rust and Cargo Workspace"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, Rust and Cargo Workspace

### Community 43 - "Component Boundaries and Dependency Direction"
Cohesion: 0.33
Nodes (5): Alternatives, Component Boundaries and Dependency Direction, Consequences, Context, Decision

### Community 44 - "Native Window, GPU Rendering, and Font Stack"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, Native Window, GPU Rendering, and Font Stack

### Community 45 - "VTE Parsing with Project-Owned Terminal State"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, VTE Parsing with Project-Owned Terminal State

### Community 46 - "PTY Boundary, Process Model, and Threading"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, PTY Boundary, Process Model, and Threading

### Community 47 - "Declarative Configuration, Central Commands, and Workspaces"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, Declarative Configuration, Central Commands, and Workspaces

### Community 48 - "V1 Scope Boundaries"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, V1 Scope Boundaries

### Community 49 - "License, Internal Naming, and Target Architectures"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, License, Internal Naming, and Target Architectures

### Community 50 - "Product"
Cohesion: 0.33
Nodes (5): Goal, Priorities, Product, V1 non-goals, V1 scope

### Community 51 - "Engineering Agent Contract"
Cohesion: 0.40
Nodes (4): Context efficiency, Engineering Agent Contract, Non-negotiable boundaries, Start every engineering session

### Community 52 - "Tasks"
Cohesion: 0.33
Nodes (5): Deferred by scope, Later, Later, Next, Tasks

### Community 53 - "Performance"
Cohesion: 0.40
Nodes (4): Initial goals, Measurements to add with implementations, Performance, Rules

### Community 54 - "delete_characters_parser.rs"
Cohesion: 0.43
Nodes (7): labeled_state(), parse(), parser_dch_clamps_at_the_right_edge_and_preserves_printable_neighbors(), parser_dch_is_chunk_invariant_and_incomplete_or_unsupported_shapes_are_safe(), parser_dispatches_dch_with_omitted_zero_one_and_multiple_counts(), row(), String

### Community 56 - "filled_state"
Cohesion: 0.22
Nodes (9): cursor_next_and_previous_line_do_not_use_scrolling_control_semantics(), cursor_next_and_previous_line_preserve_cells_state_and_screen_bounds(), erase_in_line_supports_all_directions_without_moving_the_cursor(), filled_state(), full_screen_scroll_down_normalizes_count_to_screen_height(), full_screen_scroll_up_normalizes_count_to_screen_height(), horizontal_tab_resolves_delayed_wrap_without_modifying_cells_or_modes(), row_text() (+1 more)

### Community 57 - "row_text"
Cohesion: 0.67
Nodes (3): row_text(), String, successive_mode_sequences_preserve_unrelated_state()

### Community 61 - "erase_characters.rs"
Cohesion: 0.36
Nodes (8): erase_characters_cancels_delayed_wrap_and_ignores_scrolling_margins(), erase_characters_clears_only_the_bounded_current_row_range(), erase_characters_does_not_shift_cells_like_delete_characters(), erase_characters_is_independent_of_typing_insert_mode_and_preserves_state(), erase_characters_uses_canonical_blanks_without_changing_untouched_cells(), labeled_state(), row(), String

### Community 62 - "erase_characters_parser.rs"
Cohesion: 0.43
Nodes (7): labeled_state(), parse(), parser_dispatches_ech_with_omitted_zero_one_and_multiple_counts(), parser_ech_clamps_at_the_right_edge_and_preserves_printable_neighbors(), parser_ech_is_chunk_invariant_and_incomplete_or_unsupported_shapes_are_safe(), row(), String

### Community 63 - "TerminalReply"
Cohesion: 0.24
Nodes (7): PendingReplies, Default, Option, TerminalReply, Option, bytes(), Vec

### Community 65 - "region_nel.rs"
Cohesion: 0.36
Nodes (10): cells(), full_screen_nel_keeps_established_bottom_scroll_behavior(), labeled_state(), nel_at_bottom_margin_scrolls_only_region_and_preserves_cells_and_state(), nel_inside_custom_region_moves_down_without_scrolling_and_cancels_delayed_wrap(), nel_on_single_row_region_clears_only_that_row(), nel_outside_region_moves_bounded_without_scrolling_or_margin_clamping(), rows() (+2 more)

### Community 68 - "truecolor.rs"
Cohesion: 0.43
Nodes (7): malformed_truecolor_groups_are_atomic_and_colon_forms_stay_unsupported(), rgb(), state(), truecolor_cells_snapshot_and_move_with_complete_attributes(), truecolor_foreground_and_background_are_exact_and_preserve_styles(), truecolor_is_chunk_safe_and_preserves_unrelated_terminal_state_and_replies(), truecolor_minimum_maximum_replacement_and_resets_work_per_channel()

### Community 70 - "dsr_status_parser.rs"
Cohesion: 0.43
Nodes (6): bytes(), parser_dispatches_status_at_every_chunk_boundary(), Vec, state(), status_preserves_order_with_cpr_and_da1(), unsupported_dsr_forms_remain_noops()

### Community 71 - "da2.rs"
Cohesion: 0.53
Nodes (4): parser_recognizes_only_omitted_or_zero_da2_requests(), secondary_device_attributes_changes_only_pending_replies(), secondary_device_attributes_preserves_fifo_order_and_capacity(), state()

## Knowledge Gaps
- **96 isolated node(s):** `terminal-app`, `terminal-config`, `terminal-platform`, `terminal-renderer`, `terminal-core` (+91 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 379 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **11 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `TerminalState` connect `TerminalState` to `SemanticPerformer`, `ScreenGrid`, `CellAttributes`, `da1_parser.rs`, `assert_default_cell`, `TerminalModes`, `region_ind_ri.rs`, `HorizontalTabStops`, `region_su_sd_parser.rs`, `cpr_parser.rs`, `insert_delete_lines.rs`, `assert_observable_state_eq`, `region_su_sd.rs`, `scrolling_margins.rs`, `insert_delete_lines_parser.rs`, `rendition.rs`, `region_nel_parser.rs`, `delete_characters.rs`, `decckm_parser.rs`, `decstbm.rs`, `insert_characters.rs`, `VerticalScrollingMargins`, `insert_characters_parser.rs`, `labeled_scroll_state`, `region_ind_ri_parser.rs`, `delete_characters_parser.rs`, `filled_state`, `row_text`, `erase_characters.rs`, `erase_characters_parser.rs`, `TerminalReply`, `region_nel.rs`, `truecolor.rs`, `dsr_status_parser.rs`, `da2.rs`?**
  _High betweenness centrality (0.565) - this node is a cross-community bridge._
- **Why does `ScreenGrid` connect `ScreenGrid` to `TerminalDimensions`, `grid.rs`, `CellAttributes`, `TerminalState`, `CursorError`?**
  _High betweenness centrality (0.072) - this node is a cross-community bridge._
- **Why does `TerminalReply` connect `TerminalReply` to `cpr.rs`, `TerminalReplyBytes`, `da1.rs`, `dsr_status_parser.rs`, `da1_parser.rs`, `dsr_status.rs`, `cpr_parser.rs`?**
  _High betweenness centrality (0.053) - this node is a cross-community bridge._
- **What connects `terminal-app`, `terminal-config`, `terminal-platform` to the rest of the system?**
  _96 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `tests/parser.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.038461538461538464 - nodes in this community are weakly interconnected._
- **Should `tests/state.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.0392156862745098 - nodes in this community are weakly interconnected._
- **Should `SemanticPerformer` be split into smaller, more focused modules?**
  _Cohesion score 0.07317073170731707 - nodes in this community are weakly interconnected._