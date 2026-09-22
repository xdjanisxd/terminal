use terminal_core::{
    AutoWrapMode, CellColor, CellOccupancy, CharacterInsertionMode, TerminalDimensions,
    TerminalState,
};

fn state(columns: usize, rows: usize) -> TerminalState {
    TerminalState::new(TerminalDimensions::new(columns, rows).unwrap())
}

fn assert_valid_wide_cells(state: &TerminalState) {
    for row in 0..state.dimensions().rows() {
        for column in 0..state.dimensions().columns() {
            let cell = state.screen().cell(row, column).unwrap();
            match cell.occupancy() {
                CellOccupancy::Single => {}
                CellOccupancy::WideLead => {
                    assert!(column + 1 < state.dimensions().columns());
                    assert_eq!(
                        state.screen().cell(row, column + 1).unwrap().occupancy(),
                        CellOccupancy::WideContinuation
                    );
                }
                CellOccupancy::WideContinuation => {
                    assert!(column > 0);
                    assert_eq!(
                        state.screen().cell(row, column - 1).unwrap().occupancy(),
                        CellOccupancy::WideLead
                    );
                }
            }
        }
    }
}

#[test]
fn wide_printable_uses_a_lead_and_continuation_with_rendition_snapshot() {
    let mut state = state(6, 1);
    state.set_foreground_color(CellColor::Indexed(196));
    state.set_cursor_position(0, 2).unwrap();

    state.print_character('界').unwrap();

    let lead = state.screen().cell(0, 2).unwrap();
    assert_eq!(lead.character(), '界');
    assert_eq!(lead.occupancy(), CellOccupancy::WideLead);
    assert_eq!(lead.attributes().foreground(), CellColor::Indexed(196));
    assert_eq!(
        state.screen().cell(0, 3).unwrap().occupancy(),
        CellOccupancy::WideContinuation
    );
    assert_eq!(state.cursor().column(), 4);
    assert_valid_wide_cells(&state);
}

#[test]
fn wide_character_at_final_column_wraps_only_when_auto_wrap_is_enabled() {
    let mut enabled = state(4, 2);
    enabled.set_cursor_position(0, 3).unwrap();
    enabled.print_character('界').unwrap();
    assert_eq!(
        enabled.screen().cell(1, 0).unwrap().occupancy(),
        CellOccupancy::WideLead
    );
    assert_eq!(
        enabled.screen().cell(1, 1).unwrap().occupancy(),
        CellOccupancy::WideContinuation
    );
    assert_eq!((enabled.cursor().row(), enabled.cursor().column()), (1, 2));
    assert_valid_wide_cells(&enabled);

    let mut disabled = state(4, 2);
    disabled.set_auto_wrap(AutoWrapMode::Disabled);
    disabled.set_cursor_position(0, 3).unwrap();
    disabled.print_character('界').unwrap();
    assert_eq!(disabled.screen().cell(0, 3).unwrap().character(), ' ');
    assert_eq!(
        (disabled.cursor().row(), disabled.cursor().column()),
        (0, 3)
    );
    assert_valid_wide_cells(&disabled);
}

#[test]
fn wide_write_repairs_overwritten_halves_and_delayed_wrap_before_writing() {
    let mut state = state(6, 2);
    state.set_cursor_position(0, 2).unwrap();
    state.print_character('界').unwrap();
    state.set_cursor_position(0, 2).unwrap();
    state.print_character('x').unwrap();
    assert_eq!(state.screen().cell(0, 2).unwrap().character(), 'x');
    assert_eq!(state.screen().cell(0, 3).unwrap().character(), ' ');
    assert_valid_wide_cells(&state);

    state.set_cursor_position(0, 2).unwrap();
    state.print_character('界').unwrap();
    state.set_cursor_position(0, 3).unwrap();
    state.print_character('y').unwrap();
    assert_eq!(state.screen().cell(0, 2).unwrap().character(), ' ');
    assert_eq!(state.screen().cell(0, 3).unwrap().character(), 'y');
    assert_valid_wide_cells(&state);

    state.set_cursor_position(0, 5).unwrap();
    state.print_character('z').unwrap();
    state.print_character('界').unwrap();
    assert_eq!(
        state.screen().cell(1, 0).unwrap().occupancy(),
        CellOccupancy::WideLead
    );
    assert_valid_wide_cells(&state);
}

#[test]
fn printing_after_a_wide_cell_does_not_overwrite_its_continuation() {
    let mut state = state(6, 1);
    state.set_cursor_position(0, 2).unwrap();

    state.print_character('界').unwrap();
    state.print_character('x').unwrap();

    assert_eq!(state.screen().cell(0, 2).unwrap().character(), '界');
    assert_eq!(
        state.screen().cell(0, 3).unwrap().occupancy(),
        CellOccupancy::WideContinuation
    );
    assert_eq!(state.screen().cell(0, 4).unwrap().character(), 'x');
    assert_eq!(state.cursor().column(), 5);
    assert_valid_wide_cells(&state);
}

#[test]
fn erase_editing_resize_reset_and_vertical_moves_preserve_wide_cell_structure() {
    let mut state = state(6, 3);
    state.set_cursor_position(1, 2).unwrap();
    state.print_character('界').unwrap();
    state.set_cursor_position(1, 2).unwrap();
    state.erase_characters(1);
    assert_valid_wide_cells(&state);
    assert_eq!(state.screen().cell(1, 2).unwrap().character(), ' ');
    assert_eq!(state.screen().cell(1, 3).unwrap().character(), ' ');

    state.set_cursor_position(1, 2).unwrap();
    state.print_character('界').unwrap();
    state.set_cursor_position(1, 3).unwrap();
    state.delete_characters(1);
    assert_valid_wide_cells(&state);

    state.set_cursor_position(1, 2).unwrap();
    state.print_character('界').unwrap();
    state.set_cursor_position(1, 2).unwrap();
    state.insert_characters(1);
    assert_valid_wide_cells(&state);

    state.set_cursor_position(1, 2).unwrap();
    state.print_character('界').unwrap();
    state.scroll_up(1);
    assert_valid_wide_cells(&state);
    state.resize(TerminalDimensions::new(3, 3).unwrap());
    assert_valid_wide_cells(&state);
    state.reset();
    assert!(
        (0..state.dimensions().rows()).all(|row| (0..state.dimensions().columns()).all(|column| {
            state.screen().cell(row, column).unwrap().occupancy() == CellOccupancy::Single
        }))
    );
}

#[test]
fn insert_mode_wide_writes_do_not_leave_orphans_at_the_right_edge() {
    let mut state = state(5, 1);
    state.set_character_insertion(CharacterInsertionMode::Insert);
    state.set_cursor_position(0, 3).unwrap();
    state.print_character('界').unwrap();
    assert_valid_wide_cells(&state);
}

#[test]
fn final_full_width_write_uses_delayed_wrap_and_partial_erase_repairs_the_pair() {
    let mut state = state(4, 2);
    state.set_cursor_position(0, 2).unwrap();
    state.print_character('界').unwrap();
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 3));

    state.print_character('x').unwrap();
    assert_eq!(state.screen().cell(1, 0).unwrap().character(), 'x');
    assert_valid_wide_cells(&state);

    state.set_cursor_position(1, 1).unwrap();
    state.print_character('界').unwrap();
    state.set_cursor_position(1, 2).unwrap();
    state.erase(
        terminal_core::EraseRegion::Line,
        terminal_core::EraseDirection::StartToCursor,
    );
    assert_eq!(state.screen().cell(1, 1).unwrap().character(), ' ');
    assert_eq!(state.screen().cell(1, 2).unwrap().character(), ' ');
    assert_valid_wide_cells(&state);
}

#[test]
fn row_movement_primitives_preserve_valid_wide_pairs() {
    let mut state = state(5, 4);
    state.set_cursor_position(1, 1).unwrap();
    state.print_character('界').unwrap();
    state.insert_lines(1);
    assert_valid_wide_cells(&state);
    state.delete_lines(1);
    assert_valid_wide_cells(&state);
    state.scroll_up(1);
    assert_valid_wide_cells(&state);
    state.scroll_down(1);
    assert_valid_wide_cells(&state);

    state.set_cursor_position(2, 1).unwrap();
    state.print_character('界').unwrap();
    state.index();
    assert_valid_wide_cells(&state);
    state.reverse_index();
    assert_valid_wide_cells(&state);
}
