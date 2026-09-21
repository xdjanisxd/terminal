# Graph Report - terminal  (2026-09-18)

## Corpus Check
- cluster-only mode — file stats not available

## Summary
- 1202 nodes · 2013 edges · 121 communities (76 shown, 36 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS · INFERRED: 2 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `c848fac0`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- CellAttributes
- ScreenGrid
- ScreenSet
- TerminalState
- SemanticPerformer
- TestSession
- TerminalDimensions
- .cursor
- saved_cursor.rs
- PtySpawnConfig
- PtyWorker
- src/state.rs
- tests/scrollback.rs
- TerminalModes
- terminal-pty/src/lib.rs
- combining_marks.rs
- region_ind_ri.rs
- PtyError
- Scrollback
- HorizontalTabStops
- assert_observable_state_eq
- region_su_sd_parser.rs
- PtyLifecycle
- Architecture
- da1_parser.rs
- insert_delete_lines.rs
- region_su_sd.rs
- rendition.rs
- scrolling_margins.rs
- PortablePtySession
- TerminalReplyBytes
- dec_alt_screen_1049.rs
- insert_delete_lines_parser.rs
- screen_resize_scrollback_audit.rs
- TerminalReply
- dec_alt_screen_1047.rs
- region_nel_parser.rs
- wide_cells.rs
- Roadmap
- dec_alt_screen_47.rs
- decstbm.rs
- delete_characters.rs
- erase_characters.rs
- insert_characters.rs
- screen_switching.rs
- filled_state
- contract.rs
- VerticalScrollingMargins
- cpr.rs
- da2.rs
- decckm_parser.rs
- delete_characters_parser.rs
- erase_characters_parser.rs
- insert_characters_parser.rs
- labeled_scroll_state
- region_ind_ri_parser.rs
- assert_default_cell
- Conventions
- Engineering Workflow
- da1.rs
- Terminal Workspace Application
- Tasks
- PrintError
- dsr_status.rs
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
- Session Handoff
- .scrollback_row
- cpr_parser.rs
- dsr_status_parser.rs
- Performance
- row_text
- main
- Box
- Default
- Cell
- CellAttributes
- Cursor
- ScreenGrid
- TerminalDimensions
- VerticalScrollingMargins
- Display
- Error
- Formatter
- ScreenGrid
- Result
- Vec
- CursorError
- IntoIterator
- Item
- Mutex
- Option
- terminal-app
- terminal-config
- terminal-core
- terminal-platform
- terminal-pty
- terminal-renderer
- terminal-workspace
- ScreenKind
- ScreenSet
- Self
- Send

## God Nodes (most connected - your core abstractions)
1. `TerminalState` - 153 edges
2. `ScreenGrid` - 34 edges
3. `ScreenSet` - 26 edges
4. `CellAttributes` - 23 edges
5. `PtyWorker` - 19 edges
6. `PortablePtySession` - 18 edges
7. `ScreenState` - 17 edges
8. `Cell` - 16 edges
9. `PtySpawnConfig` - 16 edges
10. `PtyError` - 16 edges

## Surprising Connections (you probably didn't know these)
- `cursor()` --references--> `TerminalState`  [EXTRACTED]
  crates/terminal-core/tests/saved_cursor.rs → crates/terminal-core/src/state.rs
- `cursor()` --references--> `TerminalState`  [EXTRACTED]
  crates/terminal-core/tests/dec_alt_screen_1049.rs → crates/terminal-core/src/state.rs
- `rgb()` --references--> `CellColor`  [EXTRACTED]
  crates/terminal-core/tests/truecolor.rs → crates/terminal-core/src/cell.rs
- `cells()` --references--> `TerminalState`  [EXTRACTED]
  crates/terminal-core/tests/region_nel.rs → crates/terminal-core/src/state.rs
- `labeled_state()` --references--> `TerminalState`  [EXTRACTED]
  crates/terminal-core/tests/region_nel.rs → crates/terminal-core/src/state.rs

## Import Cycles
- None detected.

## Communities (121 total, 36 thin omitted)

### Community 0 - "CellAttributes"
Cohesion: 0.07
Nodes (28): Cell, CellAttributes, CellColor, CellOccupancy, InverseVideo, ItalicStyle, Default, Result (+20 more)

### Community 3 - "ScreenGrid"
Cohesion: 0.11
Nodes (21): CombiningMarkAttachment, delete_cells_shifts_complete_row_cells_left_and_clamps(), labeled_grid(), region_scroll_clamps_counts_and_handles_one_row_regions(), region_scroll_preserves_complete_cells_and_uses_default_exposed_rows(), region_scroll_supports_ranges_touching_either_screen_edge(), row_text(), Cell (+13 more)

### Community 4 - "ScreenSet"
Cohesion: 0.09
Nodes (16): Cell, CellAttributes, Option, Self, SavedCursor, ScreenKind, ScreenSet, ScreenState (+8 more)

### Community 5 - "TerminalState"
Cohesion: 0.05
Nodes (13): AutoWrapMode, CellColor, CharacterInsertionMode, CellAttributes, TerminalState, CursorKeyMode, CursorVisibility, InputModes (+5 more)

### Community 6 - "SemanticPerformer"
Cohesion: 0.08
Nodes (19): default_one(), ExtendedColorChannel, Display, Error, Formatter, Option, Result, Self (+11 more)

### Community 7 - "TestSession"
Cohesion: 0.10
Nodes (25): PtySession, bounded_event_queue_backpressures_without_dropping_terminal_events(), dropping_the_controller_terminates_and_joins_the_session(), explicit_termination_emits_each_terminal_event_once_and_join_rejects_later_commands(), full_command_queue_rejects_the_newest_command_without_dropping_queued_commands(), helper_worker(), Arc, Box (+17 more)

### Community 8 - "TerminalDimensions"
Cohesion: 0.09
Nodes (16): Cursor, CursorError, offset_clamped(), Display, Error, Formatter, Result, Self (+8 more)

### Community 9 - ".cursor"
Cohesion: 0.12
Nodes (3): Cursor, CursorError, VerticalScrollingMargins

### Community 10 - "saved_cursor.rs"
Cohesion: 0.13
Nodes (26): cells(), cursor(), dec_private_mode_1047_entry_resets_alternate_saved_cursor_and_exit_does_not_restore_primary(), dec_private_mode_47_round_trip_does_not_overwrite_either_saved_cursor_slot(), latest_save_wins_and_repeated_restore_is_stable(), reset_returns_both_saved_cursor_slots_to_the_uninitialized_no_op_state(), restore_before_save_is_a_safe_no_op(), restore_clamps_saved_coordinates_after_shrink_and_keeps_them_valid_after_grow() (+18 more)

### Community 11 - "PtySpawnConfig"
Cohesion: 0.17
Nodes (12): PortablePtyBackend, PtyBackend, PtySpawnConfig, Default, IntoIterator, Item, Self, OsString (+4 more)

### Community 13 - "PtyWorker"
Cohesion: 0.27
Nodes (6): PtyWorker, PtyWorkerError, Result, Drop, Formatter, JoinHandle

### Community 14 - "src/state.rs"
Cohesion: 0.15
Nodes (11): clear_screen_clears_owned_screen_content(), CursorMovement, EraseDirection, EraseRegion, PrintableWidth, reset_clears_existing_screen_content(), resize_preserves_overlap_and_blanks_new_cells_through_facade(), Self (+3 more)

### Community 15 - "tests/scrollback.rs"
Cohesion: 0.28
Nodes (15): alternate_scrolling_does_not_append_primary_history_and_switches_preserve_it(), captured_cells_preserve_combining_wide_occupancy_and_rendition(), full_screen_nel_and_wrapped_print_capture_in_chronological_order(), full_screen_su_captures_each_displaced_row_and_restricted_su_does_not(), primary_full_screen_index_captures_the_displaced_top_row(), primary_scrollback_starts_empty_and_alternate_has_no_independent_history(), reset_clears_primary_history_and_resize_retains_capture_time_widths(), row_text() (+7 more)

### Community 16 - "TerminalModes"
Cohesion: 0.25
Nodes (6): AutoWrapMode, CharacterInsertionMode, CursorKeyMode, CursorVisibility, InputModes, TerminalModes

### Community 17 - "terminal-pty/src/lib.rs"
Cohesion: 0.41
Nodes (13): AtomicBool, handle_command(), PtyCommand, PtyWorkerEvent, request_termination(), Arc, Receiver, SyncSender (+5 more)

### Community 18 - "combining_marks.rs"
Cohesion: 0.36
Nodes (13): assert_valid_cells(), combining_at_right_margin_attaches_before_delayed_wrap_resolves(), combining_capacity_is_bounded_without_mutating_the_base_or_cursor(), editing_and_resize_move_or_clear_complete_combining_payloads(), erase_overwrite_and_reset_clear_combining_payloads(), marks(), narrow_base_owns_ordered_marks_without_cursor_or_rendition_changes(), no_base_or_erased_base_ignores_marks_without_mutation() (+5 more)

### Community 19 - "region_ind_ri.rs"
Cohesion: 0.27
Nodes (12): crossing_into_region_by_index_or_reverse_index_does_not_scroll(), full_screen_margins_preserve_previous_index_and_reverse_index_behavior(), index_and_reverse_index_clear_only_a_single_row_region(), index_and_reverse_index_move_outside_region_without_scrolling_or_clamping(), index_moves_inside_region_and_scrolls_only_at_bottom_margin(), labeled_state(), region_boundary_scrolling_preserves_styled_cells_and_unrelated_state(), reverse_index_moves_inside_region_and_scrolls_only_at_top_margin() (+4 more)

### Community 20 - "PtyError"
Cohesion: 0.21
Nodes (5): PtyError, PtySize, PtySizeError, Display, Error

### Community 21 - "Scrollback"
Cohesion: 0.23
Nodes (8): evicts_the_oldest_row_when_at_capacity(), row_is_valid(), Cell, Option, Self, Vec, Scrollback, VecDeque

### Community 22 - "HorizontalTabStops"
Cohesion: 0.17
Nodes (4): HorizontalTabStops, Option, Self, Vec

### Community 23 - "assert_observable_state_eq"
Cohesion: 0.28
Nodes (13): assert_observable_state_eq(), byte_at_a_time_matches_one_shot_input(), cnl_and_cpl_are_chunk_safe_and_preserve_printable_input(), every_split_point_matches_one_shot_input(), ind_ri_nel_are_chunk_safe_at_every_byte_boundary(), indexed_color_and_malformed_groups_are_chunk_safe_at_every_boundary(), many_chunk_sizes_match_one_shot_input(), parse_in_chunks() (+5 more)

### Community 24 - "region_su_sd_parser.rs"
Cohesion: 0.27
Nodes (11): decstbm_followed_by_su_or_sd_uses_active_region_and_default_count(), labeled_state(), malformed_extra_and_subparameter_shapes_remain_safe_no_ops_with_margins(), parse(), parser_region_su_sd_are_safe_across_every_input_split(), parser_su_sd_preserve_active_sgr_and_moved_cell_attributes(), printable_text_neighbors_region_scroll_without_parser_side_mutation(), row_text() (+3 more)

### Community 25 - "PtyLifecycle"
Cohesion: 0.22
Nodes (5): PtyExitStatus, PtyLifecycle, PtyOutput, Option, Vec

### Community 26 - "Architecture"
Cohesion: 0.15
Nodes (12): Architecture, Commands and workspaces, Conceptual components, Dependency direction, Initial terminal-core model invariants, Mode ownership, Parser boundary, Resource and trust boundaries (+4 more)

### Community 27 - "da1_parser.rs"
Cohesion: 0.26
Nodes (11): da1_queries_are_chunk_safe_at_every_input_boundary(), incomplete_malformed_private_and_non_primary_da_forms_do_not_reply(), multiple_queries_in_one_stream_and_separate_calls_are_preserved(), parser_csi_c_and_csi_zero_c_produce_exact_da1_reply(), parser_da1_at_capacity_is_bounded_and_does_not_overwrite_pending_replies(), printable_text_neighbors_da1_without_becoming_reply_input(), reply_bytes(), row_text() (+3 more)

### Community 28 - "insert_delete_lines.rs"
Cohesion: 0.29
Nodes (10): delete_lines_is_cursor_relative_within_the_active_region(), insert_lines_is_cursor_relative_within_the_active_region(), labeled_state(), line_operations_are_no_ops_outside_margins_and_use_full_screen_defaults(), line_operations_clamp_to_the_cursor_to_bottom_subregion(), line_operations_leave_margins_intact(), line_operations_preserve_complete_cells_canonical_blanks_and_unrelated_state(), rows() (+2 more)

### Community 29 - "region_su_sd.rs"
Cohesion: 0.27
Nodes (10): full_screen_margins_keep_established_su_sd_behavior(), labeled_state(), region_scroll_handles_one_row_and_regions_at_screen_edges(), region_scroll_preserves_complete_cells_and_unrelated_terminal_state(), region_scroll_zero_is_no_op_and_large_counts_clear_only_active_region(), rows(), String, Vec (+2 more)

### Community 30 - "rendition.rs"
Cohesion: 0.30
Nodes (10): current_rendition_uses_documented_defaults(), faint_intensity_is_typed_idempotent_and_preserved_by_resize(), foreground_and_background_channels_change_independently(), inverse_video_can_be_set_and_cleared_independently(), italic_can_be_set_and_cleared_independently(), printed_cells_snapshot_the_current_rendition_without_retroactive_changes(), state(), terminal_reset_restores_rendition_defaults() (+2 more)

### Community 31 - "scrolling_margins.rs"
Cohesion: 0.21
Nodes (5): custom_margins_bound_su_sd_and_nel(), labeled_state(), row_text(), String, setting_margins_does_not_change_existing_cells()

### Community 32 - "PortablePtySession"
Cohesion: 0.29
Nodes (10): Child, PortableOutputReader, PortablePtySession, PtyOutputReader, Box, Mutex, Send, MasterPty (+2 more)

### Community 34 - "dec_alt_screen_1049.rs"
Cohesion: 0.29
Nodes (10): cursor(), dec_private_mode_1049_is_chunk_safe_and_isolated_from_other_private_mode_forms(), dec_private_mode_1049_preserves_global_state_and_primary_wide_combining_cells(), dec_private_mode_1049_resets_stale_alternate_wide_combining_content(), dec_private_mode_1049_restore_clamps_after_resize_and_reset_clears_saved_state(), dec_private_mode_1049_saves_primary_resets_alternate_and_restores_primary(), repeated_1049_entry_while_alternate_is_active_is_idempotent_and_preserves_primary_save(), row_text() (+2 more)

### Community 35 - "insert_delete_lines_parser.rs"
Cohesion: 0.29
Nodes (9): labeled_state(), parse(), parser_il_dl_are_chunk_invariant_and_incomplete_or_malformed_input_is_safe(), parser_il_dl_clamp_counts_and_preserve_printable_neighbors(), parser_il_dl_preserve_cursor_and_existing_cell_model(), parser_il_dl_respect_cursor_relative_decstbm_subregions(), rows(), String (+1 more)

### Community 37 - "screen_resize_scrollback_audit.rs"
Cohesion: 0.35
Nodes (10): primary_history_retains_chronological_mixed_width_rows_across_resizes(), resize_during_1049_clamps_saved_cursor_without_mutating_primary_history(), resize_never_mutates_historical_wide_or_combining_cells_and_reset_clears_history(), resize_preserves_primary_history_through_47_and_1047_round_trips(), row_text(), Cell, state(), write_row() (+2 more)

### Community 38 - "TerminalReply"
Cohesion: 0.24
Nodes (8): PendingReplies, Default, Option, TerminalReply, reply_bytes(), Vec, bytes(), Vec

### Community 39 - "dec_alt_screen_1047.rs"
Cohesion: 0.33
Nodes (9): dec_private_mode_1047_clears_the_resized_alternate_and_terminal_reset_is_unchanged(), dec_private_mode_1047_clears_wide_combining_alternate_cells_but_not_primary_cells(), dec_private_mode_1047_entry_resets_the_alternate_screen_locally(), dec_private_mode_1047_is_distinct_from_47_and_rejects_malformed_mode_forms(), dec_private_mode_1047_preserves_terminal_global_state(), dec_private_mode_1047_repeated_entry_clears_again_and_exit_is_idempotent(), row_text(), String (+1 more)

### Community 40 - "region_nel_parser.rs"
Cohesion: 0.38
Nodes (9): assert_observable_state_eq(), incomplete_and_malformed_nel_escapes_keep_safe_existing_behavior(), labeled_state(), parse(), parser_region_nel_is_chunk_safe_at_every_input_boundary(), parser_routes_decstbm_followed_by_nel_through_active_region(), printable_text_before_and_after_nel_uses_region_semantics(), row_text() (+1 more)

### Community 41 - "wide_cells.rs"
Cohesion: 0.51
Nodes (9): assert_valid_wide_cells(), erase_editing_resize_reset_and_vertical_moves_preserve_wide_cell_structure(), final_full_width_write_uses_delayed_wrap_and_partial_erase_repairs_the_pair(), insert_mode_wide_writes_do_not_leave_orphans_at_the_right_edge(), row_movement_primitives_preserve_valid_wide_pairs(), state(), wide_character_at_final_column_wraps_only_when_auto_wrap_is_enabled(), wide_printable_uses_a_lead_and_continuation_with_rendition_snapshot() (+1 more)

### Community 42 - "Roadmap"
Cohesion: 0.20
Nodes (9): Early integration checkpoint, M0: Repository foundation, M1: Terminal core foundation, M2: Terminal compatibility, M3: PTY and session lifecycle, M4: Window and renderer, M5: Input and configuration, M6: Workspaces and release readiness (+1 more)

### Community 43 - "dec_alt_screen_47.rs"
Cohesion: 0.39
Nodes (8): dec_private_mode_47_handles_mixed_parameters_and_isolates_neighboring_modes(), dec_private_mode_47_is_chunk_invariant_and_keeps_printable_text_on_active_screen(), dec_private_mode_47_is_idempotent_and_preserves_shared_state(), dec_private_mode_47_preserves_wide_combining_cells_across_resize_and_reset(), dec_private_mode_47_selects_independent_screens_without_clearing(), row_text(), String, state()

### Community 44 - "decstbm.rs"
Cohesion: 0.28
Nodes (3): decstbm_accepts_full_explicit_and_partial_default_forms(), decstbm_defaults_remain_valid_on_a_single_row_screen(), parse()

### Community 45 - "delete_characters.rs"
Cohesion: 0.36
Nodes (8): delete_characters_cancels_delayed_wrap_like_ich_and_erase(), delete_characters_is_independent_of_typing_insert_mode_and_preserves_state(), delete_characters_moves_complete_cells_and_fills_with_canonical_blanks(), delete_characters_shifts_only_the_current_row_and_clamps_counts(), insert_and_delete_characters_compose_as_bounded_current_row_edits(), labeled_state(), row(), String

### Community 46 - "erase_characters.rs"
Cohesion: 0.36
Nodes (8): erase_characters_cancels_delayed_wrap_and_ignores_scrolling_margins(), erase_characters_clears_only_the_bounded_current_row_range(), erase_characters_does_not_shift_cells_like_delete_characters(), erase_characters_is_independent_of_typing_insert_mode_and_preserves_state(), erase_characters_uses_canonical_blanks_without_changing_untouched_cells(), labeled_state(), row(), String

### Community 47 - "insert_characters.rs"
Cohesion: 0.36
Nodes (8): insert_characters_cancels_delayed_wrap_like_other_current_row_editing_operations(), insert_characters_does_not_depend_on_scrolling_margins(), insert_characters_is_independent_of_typing_insert_mode_and_preserves_state(), insert_characters_shifts_only_the_current_row_and_clamps_counts(), inserted_character_cells_are_canonical_blanks_and_moved_cells_keep_attributes(), labeled_state(), row(), String

### Community 48 - "screen_switching.rs"
Cohesion: 0.36
Nodes (8): margins_and_pending_wrap_are_screen_local(), modes_rendition_tabs_and_replies_are_shared_across_switching(), row_text(), String, starts_primary_and_switches_between_independent_contents_and_cursors(), state(), switching_is_idempotent_and_reset_blanks_both_screens_on_primary(), wide_combining_primary_payload_survives_round_trip_and_resize_updates_both_screens()

### Community 49 - "filled_state"
Cohesion: 0.22
Nodes (9): cursor_next_and_previous_line_do_not_use_scrolling_control_semantics(), cursor_next_and_previous_line_preserve_cells_state_and_screen_bounds(), erase_in_display_supports_all_directions_without_moving_the_cursor(), filled_state(), full_screen_scroll_down_normalizes_count_to_screen_height(), full_screen_scroll_up_normalizes_count_to_screen_height(), horizontal_tab_resolves_delayed_wrap_without_modifying_cells_or_modes(), row_text() (+1 more)

### Community 50 - "contract.rs"
Cohesion: 0.28
Nodes (3): contains_subsequence(), helper_session(), portable_backend_streams_helper_output_without_assuming_read_boundaries()

### Community 51 - "VerticalScrollingMargins"
Cohesion: 0.29
Nodes (3): Option, Self, VerticalScrollingMargins

### Community 53 - "da2.rs"
Cohesion: 0.36
Nodes (6): bytes(), parser_recognizes_only_omitted_or_zero_da2_requests(), Vec, secondary_device_attributes_changes_only_pending_replies(), secondary_device_attributes_preserves_fifo_order_and_capacity(), state()

### Community 54 - "decckm_parser.rs"
Cohesion: 0.29
Nodes (3): decckm_changes_only_cursor_key_mode_and_preserves_terminal_state(), row_text(), String

### Community 55 - "delete_characters_parser.rs"
Cohesion: 0.43
Nodes (7): labeled_state(), parse(), parser_dch_clamps_at_the_right_edge_and_preserves_printable_neighbors(), parser_dch_is_chunk_invariant_and_incomplete_or_unsupported_shapes_are_safe(), parser_dispatches_dch_with_omitted_zero_one_and_multiple_counts(), row(), String

### Community 56 - "erase_characters_parser.rs"
Cohesion: 0.43
Nodes (7): labeled_state(), parse(), parser_dispatches_ech_with_omitted_zero_one_and_multiple_counts(), parser_ech_clamps_at_the_right_edge_and_preserves_printable_neighbors(), parser_ech_is_chunk_invariant_and_incomplete_or_unsupported_shapes_are_safe(), row(), String

### Community 57 - "insert_characters_parser.rs"
Cohesion: 0.43
Nodes (7): labeled_state(), parse(), parser_dispatches_ich_with_omitted_zero_one_and_multiple_counts(), parser_ich_clamps_at_the_right_edge_and_preserves_printable_neighbors(), parser_ich_is_chunk_invariant_and_incomplete_or_unsupported_shapes_are_safe(), row(), String

### Community 58 - "labeled_scroll_state"
Cohesion: 0.25
Nodes (8): incomplete_and_malformed_index_controls_do_not_mutate_unrelated_state(), labeled_scroll_state(), malformed_or_extra_su_sd_parameters_are_safe_no_ops(), parser_dispatches_index_reverse_index_and_next_line(), parser_sd_uses_one_for_omitted_and_zero_counts(), parser_su_and_sd_clamp_explicit_and_oversized_counts(), parser_su_uses_one_for_omitted_and_zero_counts(), su_sd_sequences_are_chunk_safe_at_every_byte_boundary()

### Community 59 - "region_ind_ri_parser.rs"
Cohesion: 0.36
Nodes (6): parse(), parser_index_and_reverse_index_with_margins_are_chunk_safe(), parser_region_scrolls_preserve_printable_neighbors(), parser_routes_decstbm_followed_by_index_and_reverse_index(), row_text(), String

### Community 60 - "assert_default_cell"
Cohesion: 0.32
Nodes (8): assert_default_cell(), bottom_index_scrolls_styled_cells_and_preserves_terminal_state(), bottom_next_line_scrolls_up_and_preserves_rendition_and_modes(), erase_in_line_supports_all_directions_without_moving_the_cursor(), full_screen_scroll_moves_complete_cells_and_preserves_rendition_and_modes(), scroll_down_moves_attributes_and_clears_new_top_rows_with_default_cells(), styled_control_state(), top_reverse_index_scrolls_styled_cells_and_clears_default_top_row()

### Community 61 - "Conventions"
Cohesion: 0.25
Nodes (7): Conventions, Dependencies, Documentation, Engineering, Errors, logging, and unsafe code, Required local gates, Testing

### Community 62 - "Engineering Workflow"
Cohesion: 0.29
Nodes (6): Before implementation, Completion gates, Context loading, Engineering Workflow, Implementation, Reporting

### Community 64 - "Terminal Workspace Application"
Cohesion: 0.29
Nodes (6): Current validation, License, Priorities, Repository map, Target platforms, Terminal Workspace Application

### Community 65 - "Tasks"
Cohesion: 0.33
Nodes (5): Completed in this slice, Deferred by scope, Later, Next, Tasks

### Community 68 - "Current State"
Cohesion: 0.33
Nodes (5): Current State, Known issues, Missing, Partial, Working

### Community 69 - "Rust and Cargo Workspace"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, Rust and Cargo Workspace

### Community 70 - "Component Boundaries and Dependency Direction"
Cohesion: 0.33
Nodes (5): Alternatives, Component Boundaries and Dependency Direction, Consequences, Context, Decision

### Community 71 - "Native Window, GPU Rendering, and Font Stack"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, Native Window, GPU Rendering, and Font Stack

### Community 72 - "VTE Parsing with Project-Owned Terminal State"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, VTE Parsing with Project-Owned Terminal State

### Community 73 - "PTY Boundary, Process Model, and Threading"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, PTY Boundary, Process Model, and Threading

### Community 74 - "Declarative Configuration, Central Commands, and Workspaces"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, Declarative Configuration, Central Commands, and Workspaces

### Community 75 - "V1 Scope Boundaries"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, V1 Scope Boundaries

### Community 76 - "License, Internal Naming, and Target Architectures"
Cohesion: 0.33
Nodes (5): Alternatives, Consequences, Context, Decision, License, Internal Naming, and Target Architectures

### Community 77 - "Product"
Cohesion: 0.33
Nodes (5): Goal, Priorities, Product, V1 non-goals, V1 scope

### Community 78 - "Engineering Agent Contract"
Cohesion: 0.40
Nodes (4): Context efficiency, Engineering Agent Contract, Non-negotiable boundaries, Start every engineering session

### Community 79 - "Session Handoff"
Cohesion: 0.40
Nodes (4): Current state, Next, Session Handoff, Validation

### Community 80 - ".scrollback_row"
Cohesion: 0.40
Nodes (3): Cell, Option, TerminalReply

### Community 81 - "cpr_parser.rs"
Cohesion: 0.70
Nodes (4): cpr_is_chunk_safe_at_every_input_boundary_and_across_calls(), incomplete_or_malformed_dsr_does_not_reply_before_a_complete_valid_query(), multiple_and_interleaved_da1_and_cpr_queries_keep_exact_fifo_order(), state()

### Community 82 - "dsr_status_parser.rs"
Cohesion: 0.70
Nodes (4): parser_dispatches_status_at_every_chunk_boundary(), state(), status_preserves_order_with_cpr_and_da1(), unsupported_dsr_forms_remain_noops()

### Community 83 - "Performance"
Cohesion: 0.40
Nodes (4): Initial goals, Measurements to add with implementations, Performance, Rules

### Community 84 - "row_text"
Cohesion: 0.67
Nodes (3): row_text(), String, successive_mode_sequences_preserve_unrelated_state()

## Knowledge Gaps
- **95 isolated node(s):** `terminal-app`, `terminal-config`, `terminal-core`, `terminal-platform`, `terminal-pty` (+90 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 455 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **36 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `TerminalState` connect `TerminalState` to `CellAttributes`, `ScreenSet`, `SemanticPerformer`, `.cursor`, `saved_cursor.rs`, `src/state.rs`, `tests/scrollback.rs`, `combining_marks.rs`, `region_ind_ri.rs`, `assert_observable_state_eq`, `region_su_sd_parser.rs`, `da1_parser.rs`, `insert_delete_lines.rs`, `region_su_sd.rs`, `rendition.rs`, `scrolling_margins.rs`, `dec_alt_screen_1049.rs`, `insert_delete_lines_parser.rs`, `dec_alt_screen_1047.rs`, `region_nel_parser.rs`, `wide_cells.rs`, `dec_alt_screen_47.rs`, `decstbm.rs`, `delete_characters.rs`, `erase_characters.rs`, `insert_characters.rs`, `screen_switching.rs`, `filled_state`, `da2.rs`, `decckm_parser.rs`, `delete_characters_parser.rs`, `erase_characters_parser.rs`, `insert_characters_parser.rs`, `labeled_scroll_state`, `region_ind_ri_parser.rs`, `assert_default_cell`, `PrintError`, `.scrollback_row`, `cpr_parser.rs`, `dsr_status_parser.rs`, `row_text`?**
  _High betweenness centrality (0.582) - this node is a cross-community bridge._
- **Why does `PrintError` connect `PrintError` to `PtyError`, `SemanticPerformer`, `src/state.rs`?**
  _High betweenness centrality (0.160) - this node is a cross-community bridge._
- **Why does `SemanticPerformer` connect `SemanticPerformer` to `PrintError`, `TerminalState`?**
  _High betweenness centrality (0.056) - this node is a cross-community bridge._
- **What connects `terminal-app`, `terminal-config`, `terminal-core` to the rest of the system?**
  _95 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `CellAttributes` be split into smaller, more focused modules?**
  _Cohesion score 0.06666666666666667 - nodes in this community are weakly interconnected._
- **Should `tests/parser.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.0392156862745098 - nodes in this community are weakly interconnected._
- **Should `tests/state.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.0392156862745098 - nodes in this community are weakly interconnected._