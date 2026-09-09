use terminal_core::{
    AutoWrapMode, CharacterInsertionMode, CursorError, CursorKeyMode, CursorMovement,
    CursorVisibility, EraseDirection, EraseRegion, PrintError, TerminalDimensions, TerminalState,
};

fn row_text(state: &TerminalState, row: usize) -> String {
    (0..state.dimensions().columns())
        .map(|column| state.screen().cell(row, column).unwrap().character())
        .collect()
}

fn filled_state(columns: usize, rows: usize) -> TerminalState {
    let mut state = TerminalState::new(TerminalDimensions::new(columns, rows).unwrap());
    for row in 0..rows {
        state.set_cursor_position(row, 0).unwrap();
        for offset in 0..columns {
            state
                .print_character(char::from(
                    b'a' + u8::try_from((row * columns + offset) % 26).unwrap(),
                ))
                .unwrap();
        }
    }
    state
}

fn assert_default_cell(state: &TerminalState, row: usize, column: usize) {
    let cell = state.screen().cell(row, column).unwrap();
    assert_eq!(cell, &terminal_core::Cell::default());
    assert_eq!(cell.attributes(), &terminal_core::CellAttributes::default());
}

#[test]
fn typed_cursor_movements_cover_relative_and_absolute_semantics() {
    let mut state = TerminalState::new(TerminalDimensions::new(8, 6).unwrap());
    state.set_cursor_position(3, 4).unwrap();

    state.move_cursor(CursorMovement::Up(2));
    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 4));
    state.move_cursor(CursorMovement::Down(3));
    assert_eq!((state.cursor().row(), state.cursor().column()), (4, 4));
    state.move_cursor(CursorMovement::Forward(2));
    assert_eq!((state.cursor().row(), state.cursor().column()), (4, 6));
    state.move_cursor(CursorMovement::Backward(5));
    assert_eq!((state.cursor().row(), state.cursor().column()), (4, 1));
    state.move_cursor(CursorMovement::Position { row: 2, column: 3 });
    assert_eq!((state.cursor().row(), state.cursor().column()), (2, 3));
    state.move_cursor(CursorMovement::HorizontalAbsolute(7));
    assert_eq!((state.cursor().row(), state.cursor().column()), (2, 7));
    state.move_cursor(CursorMovement::VerticalAbsolute(5));
    assert_eq!((state.cursor().row(), state.cursor().column()), (5, 7));
}

#[test]
fn typed_cursor_movements_clamp_zero_and_extreme_amounts_to_screen_bounds() {
    let mut state = TerminalState::new(TerminalDimensions::new(8, 6).unwrap());
    state.set_cursor_position(2, 3).unwrap();

    state.move_cursor(CursorMovement::Up(0));
    state.move_cursor(CursorMovement::Forward(0));
    assert_eq!((state.cursor().row(), state.cursor().column()), (2, 3));

    state.move_cursor(CursorMovement::Up(usize::MAX));
    state.move_cursor(CursorMovement::Backward(usize::MAX));
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 0));
    state.move_cursor(CursorMovement::Down(usize::MAX));
    state.move_cursor(CursorMovement::Forward(usize::MAX));
    assert_eq!((state.cursor().row(), state.cursor().column()), (5, 7));

    state.move_cursor(CursorMovement::Position {
        row: usize::MAX,
        column: usize::MAX,
    });
    assert_eq!((state.cursor().row(), state.cursor().column()), (5, 7));
}

#[test]
fn every_typed_cursor_operation_cancels_delayed_wrap() {
    let movements = [
        CursorMovement::Up(1),
        CursorMovement::Down(1),
        CursorMovement::Forward(1),
        CursorMovement::Backward(1),
        CursorMovement::Position { row: 0, column: 1 },
        CursorMovement::HorizontalAbsolute(1),
        CursorMovement::VerticalAbsolute(0),
    ];

    for movement in movements {
        let mut state = TerminalState::new(TerminalDimensions::new(2, 2).unwrap());
        state.print_character('a').unwrap();
        state.print_character('b').unwrap();

        state.move_cursor(movement);
        let cursor_before_print = state.cursor();
        state.print_character('x').unwrap();

        assert_eq!(
            state
                .screen()
                .cell(cursor_before_print.row(), cursor_before_print.column())
                .unwrap()
                .character(),
            'x',
            "movement {movement:?} left delayed wrap pending"
        );
    }
}

#[test]
fn erase_in_line_supports_all_directions_without_moving_the_cursor() {
    let cases = [
        (EraseDirection::CursorToEnd, "ab   "),
        (EraseDirection::StartToCursor, "   de"),
        (EraseDirection::EntireRegion, "     "),
    ];

    for (direction, expected) in cases {
        let mut state = filled_state(5, 2);
        state.set_cursor_position(0, 2).unwrap();
        let dimensions = state.dimensions();
        let modes = *state.terminal_modes();
        let input_modes = *state.input_modes();

        state.erase(EraseRegion::Line, direction);

        assert_eq!(row_text(&state, 0), expected);
        assert_eq!(row_text(&state, 1), "fghij");
        assert_eq!((state.cursor().row(), state.cursor().column()), (0, 2));
        assert_eq!(state.dimensions(), dimensions);
        assert_eq!(*state.terminal_modes(), modes);
        assert_eq!(*state.input_modes(), input_modes);
        assert_default_cell(&state, 0, 2);
    }
}

#[test]
fn erase_in_display_supports_all_directions_without_moving_the_cursor() {
    let cases = [
        (EraseDirection::CursorToEnd, ["abc", "d  ", "   "]),
        (EraseDirection::StartToCursor, ["   ", "  f", "ghi"]),
        (EraseDirection::EntireRegion, ["   ", "   ", "   "]),
    ];

    for (direction, expected) in cases {
        let mut state = filled_state(3, 3);
        state.set_cursor_position(1, 1).unwrap();
        let dimensions = state.dimensions();
        let modes = *state.terminal_modes();
        let input_modes = *state.input_modes();

        state.erase(EraseRegion::Display, direction);

        for (row, expected_row) in expected.into_iter().enumerate() {
            assert_eq!(row_text(&state, row), expected_row);
        }
        assert_eq!((state.cursor().row(), state.cursor().column()), (1, 1));
        assert_eq!(state.dimensions(), dimensions);
        assert_eq!(*state.terminal_modes(), modes);
        assert_eq!(*state.input_modes(), input_modes);
        assert_default_cell(&state, 1, 1);
    }
}

#[test]
fn erase_cancels_delayed_wrap_without_moving_the_cursor() {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 2).unwrap());
    state.print_character('a').unwrap();
    state.print_character('b').unwrap();

    state.erase(EraseRegion::Line, EraseDirection::CursorToEnd);
    state.print_character('x').unwrap();

    assert_eq!(row_text(&state, 0), "ax");
    assert_eq!(row_text(&state, 1), "  ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 1));
}

#[test]
fn prints_a_character_at_the_cursor_and_advances_one_column() {
    let mut state = TerminalState::new(TerminalDimensions::new(3, 2).unwrap());
    state.set_cursor_position(1, 0).unwrap();

    state.print_character('x').unwrap();

    assert_eq!(row_text(&state, 1), "x  ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 1));
}

#[test]
fn prints_sequential_characters_in_replace_mode() {
    let mut state = TerminalState::new(TerminalDimensions::new(4, 1).unwrap());

    for character in "abc".chars() {
        state.print_character(character).unwrap();
    }

    assert_eq!(row_text(&state, 0), "abc ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 3));
}

#[test]
fn rejects_control_characters_without_mutating_state() {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 1).unwrap());

    assert_eq!(
        state.print_character('\n'),
        Err(PrintError::ControlCharacter('\n'))
    );

    assert_eq!(row_text(&state, 0), "  ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 0));
}

#[test]
fn rejects_unicode_whose_cell_width_is_not_modeled() {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 1).unwrap());

    assert_eq!(
        state.print_character('界'),
        Err(PrintError::UnsupportedCharacter('界'))
    );
    assert_eq!(
        state.print_character('\u{0301}'),
        Err(PrintError::UnsupportedCharacter('\u{0301}'))
    );

    assert_eq!(row_text(&state, 0), "  ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 0));
}

#[test]
fn accepts_ascii_boundaries_and_rejects_delete() {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 1).unwrap());

    state.print_character(' ').unwrap();
    state.print_character('~').unwrap();

    assert_eq!(row_text(&state, 0), " ~");
    assert_eq!(
        state.print_character('\u{007f}'),
        Err(PrintError::ControlCharacter('\u{007f}'))
    );
    assert_eq!(row_text(&state, 0), " ~");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 1));
}

#[test]
fn final_column_enters_delayed_wrap_until_the_next_print() {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 2).unwrap());

    state.print_character('a').unwrap();
    state.print_character('b').unwrap();

    assert_eq!(row_text(&state, 0), "ab");
    assert_eq!(row_text(&state, 1), "  ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 1));

    state.print_character('c').unwrap();

    assert_eq!(row_text(&state, 0), "ab");
    assert_eq!(row_text(&state, 1), "c ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 1));
}

#[test]
fn disabled_auto_wrap_replaces_the_final_column_without_wrapping() {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 2).unwrap());
    state.set_auto_wrap(AutoWrapMode::Disabled);

    for character in "abc".chars() {
        state.print_character(character).unwrap();
    }

    assert_eq!(row_text(&state, 0), "ac");
    assert_eq!(row_text(&state, 1), "  ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 1));
}

#[test]
fn disabling_auto_wrap_cancels_an_existing_pending_wrap() {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 2).unwrap());
    state.print_character('a').unwrap();
    state.print_character('b').unwrap();

    state.set_auto_wrap(AutoWrapMode::Disabled);
    state.print_character('c').unwrap();

    assert_eq!(row_text(&state, 0), "ac");
    assert_eq!(row_text(&state, 1), "  ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 1));
}

#[test]
fn wrapping_from_the_final_row_scrolls_only_the_active_screen() {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 2).unwrap());

    for character in "abcde".chars() {
        state.print_character(character).unwrap();
    }

    assert_eq!(row_text(&state, 0), "cd");
    assert_eq!(row_text(&state, 1), "e ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 1));
}

#[test]
fn carriage_return_moves_to_the_first_column_without_changing_row() {
    let mut state = TerminalState::new(TerminalDimensions::new(3, 2).unwrap());
    state.set_cursor_position(1, 2).unwrap();

    state.carriage_return();

    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 0));
}

#[test]
fn carriage_return_cancels_a_pending_wrap() {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 2).unwrap());
    state.print_character('a').unwrap();
    state.print_character('b').unwrap();

    state.carriage_return();
    state.print_character('c').unwrap();

    assert_eq!(row_text(&state, 0), "cb");
    assert_eq!(row_text(&state, 1), "  ");
}

#[test]
fn line_feed_moves_down_without_changing_column() {
    let mut state = TerminalState::new(TerminalDimensions::new(3, 2).unwrap());
    state.set_cursor_position(0, 1).unwrap();

    state.line_feed();

    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 1));
}

#[test]
fn line_feed_preserves_pending_wrap_until_the_next_print() {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 3).unwrap());
    state.print_character('a').unwrap();
    state.print_character('b').unwrap();

    state.line_feed();
    state.print_character('c').unwrap();

    assert_eq!(row_text(&state, 0), "ab");
    assert_eq!(row_text(&state, 1), "  ");
    assert_eq!(row_text(&state, 2), "c ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (2, 1));
}

#[test]
fn bottom_line_feed_preserves_pending_wrap_for_a_later_print() {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 2).unwrap());
    state.set_cursor_position(1, 0).unwrap();
    state.print_character('a').unwrap();
    state.print_character('b').unwrap();

    state.line_feed();
    state.print_character('c').unwrap();

    assert_eq!(row_text(&state, 0), "  ");
    assert_eq!(row_text(&state, 1), "c ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 1));
}

#[test]
fn line_feed_at_the_bottom_scrolls_the_screen_and_keeps_the_column() {
    let mut state = TerminalState::new(TerminalDimensions::new(3, 2).unwrap());
    state.print_character('a').unwrap();
    state.line_feed();
    state.carriage_return();
    state.print_character('b').unwrap();

    state.line_feed();

    assert_eq!(row_text(&state, 0), "b  ");
    assert_eq!(row_text(&state, 1), "   ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 1));
}

#[test]
fn backspace_moves_left_without_erasing_and_stops_at_the_margin() {
    let mut state = TerminalState::new(TerminalDimensions::new(3, 1).unwrap());
    state.print_character('a').unwrap();
    state.print_character('b').unwrap();

    state.backspace();

    assert_eq!(row_text(&state, 0), "ab ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 1));

    state.backspace();
    state.backspace();
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 0));
}

#[test]
fn backspace_cancels_pending_wrap_before_the_next_print() {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 2).unwrap());
    state.print_character('a').unwrap();
    state.print_character('b').unwrap();

    state.backspace();
    state.print_character('c').unwrap();

    assert_eq!(row_text(&state, 0), "cb");
    assert_eq!(row_text(&state, 1), "  ");
}

#[test]
fn backspace_at_the_margin_is_a_no_op_even_when_wrap_is_pending() {
    let mut state = TerminalState::new(TerminalDimensions::new(1, 2).unwrap());
    state.print_character('a').unwrap();

    state.backspace();
    state.print_character('b').unwrap();

    assert_eq!(row_text(&state, 0), "a");
    assert_eq!(row_text(&state, 1), "b");
    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 0));
}

#[test]
fn insert_mode_shifts_the_current_row_right_and_drops_the_last_cell() {
    let mut state = TerminalState::new(TerminalDimensions::new(4, 1).unwrap());
    for character in "abcd".chars() {
        state.print_character(character).unwrap();
    }
    state.set_cursor_position(0, 1).unwrap();
    state.set_character_insertion(CharacterInsertionMode::Insert);

    state.print_character('x').unwrap();

    assert_eq!(row_text(&state, 0), "axbc");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 2));
}

#[test]
fn insert_mode_at_the_final_column_replaces_then_delays_wrap() {
    let mut state = TerminalState::new(TerminalDimensions::new(3, 2).unwrap());
    for character in "abc".chars() {
        state.print_character(character).unwrap();
    }
    state.set_cursor_position(0, 2).unwrap();
    state.set_character_insertion(CharacterInsertionMode::Insert);

    state.print_character('x').unwrap();
    state.print_character('y').unwrap();

    assert_eq!(row_text(&state, 0), "abx");
    assert_eq!(row_text(&state, 1), "y  ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 1));
}

#[test]
fn clear_screen_cancels_pending_wrap_without_moving_the_cursor() {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 2).unwrap());
    state.print_character('a').unwrap();
    state.print_character('b').unwrap();

    state.clear_screen();
    state.print_character('c').unwrap();

    assert_eq!(row_text(&state, 0), " c");
    assert_eq!(row_text(&state, 1), "  ");
}

#[test]
fn reset_discards_pending_wrap_and_restores_the_origin() {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 2).unwrap());
    state.print_character('a').unwrap();
    state.print_character('b').unwrap();

    state.reset();
    state.print_character('c').unwrap();

    assert_eq!(row_text(&state, 0), "c ");
    assert_eq!(row_text(&state, 1), "  ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 1));
}

#[test]
fn resize_cancels_pending_wrap() {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 2).unwrap());
    state.print_character('a').unwrap();
    state.print_character('b').unwrap();

    state.resize(TerminalDimensions::new(3, 2).unwrap());
    state.print_character('c').unwrap();

    assert_eq!(row_text(&state, 0), "ac ");
    assert_eq!(row_text(&state, 1), "   ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 2));
}

#[test]
fn constructs_state_from_validated_dimensions_with_documented_defaults() {
    let dimensions = TerminalDimensions::new(80, 24).unwrap();
    let state = TerminalState::new(dimensions);

    assert_eq!(state.dimensions(), dimensions);
    assert_eq!(state.screen().dimensions(), dimensions);
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 0));
    assert_eq!(
        state.terminal_modes().cursor_visibility(),
        CursorVisibility::Visible
    );
    assert_eq!(state.terminal_modes().auto_wrap(), AutoWrapMode::Enabled);
    assert_eq!(
        state.terminal_modes().character_insertion(),
        CharacterInsertionMode::Replace
    );
    assert_eq!(state.input_modes().cursor_keys(), CursorKeyMode::Normal);
}

#[test]
fn coordinates_absolute_and_relative_cursor_operations() {
    let mut state = TerminalState::new(TerminalDimensions::new(3, 2).unwrap());

    state.set_cursor_position(1, 1).unwrap();
    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 1));

    assert_eq!(
        state.set_cursor_position(2, 1),
        Err(CursorError::RowOutOfBounds { row: 2, rows: 2 })
    );
    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 1));

    state.move_cursor(CursorMovement::Up(10));
    state.move_cursor(CursorMovement::Forward(10));
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 2));
}

#[test]
fn clears_the_active_screen_without_moving_the_cursor() {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 2).unwrap());
    state.set_cursor_position(1, 1).unwrap();

    state.clear_screen();

    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 1));
    assert_eq!(
        state.screen().cell(0, 0),
        Some(&terminal_core::Cell::default())
    );
    assert_eq!(
        state.screen().cell(1, 1),
        Some(&terminal_core::Cell::default())
    );
}

#[test]
fn resizes_the_active_screen_and_keeps_cursor_bounded() {
    let mut state = TerminalState::new(TerminalDimensions::new(3, 3).unwrap());
    state.set_cursor_position(2, 2).unwrap();
    let resized = TerminalDimensions::new(2, 2).unwrap();

    state.resize(resized);

    assert_eq!(state.dimensions(), resized);
    assert_eq!(state.screen().dimensions(), resized);
    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 1));
}

#[test]
fn changes_supported_terminal_modes_through_the_facade() {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 2).unwrap());

    state.set_cursor_visibility(CursorVisibility::Hidden);
    state.set_auto_wrap(AutoWrapMode::Disabled);
    state.set_character_insertion(CharacterInsertionMode::Insert);

    assert_eq!(
        state.terminal_modes().cursor_visibility(),
        CursorVisibility::Hidden
    );
    assert_eq!(state.terminal_modes().auto_wrap(), AutoWrapMode::Disabled);
    assert_eq!(
        state.terminal_modes().character_insertion(),
        CharacterInsertionMode::Insert
    );
}

#[test]
fn changes_cursor_key_mode_through_the_facade() {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 2).unwrap());

    state.set_cursor_key_mode(CursorKeyMode::Application);

    assert_eq!(
        state.input_modes().cursor_keys(),
        CursorKeyMode::Application
    );
}

#[test]
fn reset_restores_initial_supported_state_at_current_dimensions() {
    let dimensions = TerminalDimensions::new(4, 3).unwrap();
    let mut state = TerminalState::new(dimensions);
    state.set_cursor_position(2, 3).unwrap();
    state.set_cursor_visibility(CursorVisibility::Hidden);
    state.set_auto_wrap(AutoWrapMode::Disabled);
    state.set_character_insertion(CharacterInsertionMode::Insert);
    state.set_cursor_key_mode(CursorKeyMode::Application);

    state.reset();

    assert_eq!(state.dimensions(), dimensions);
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 0));
    assert_eq!(
        state.terminal_modes(),
        &terminal_core::TerminalModes::default()
    );
    assert_eq!(state.input_modes(), &terminal_core::InputModes::default());
}

#[test]
fn reset_is_idempotent() {
    let dimensions = TerminalDimensions::new(2, 2).unwrap();
    let mut state = TerminalState::new(dimensions);
    state.set_cursor_position(1, 1).unwrap();
    state.set_auto_wrap(AutoWrapMode::Disabled);

    state.reset();
    state.reset();

    assert_eq!(state.dimensions(), dimensions);
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 0));
    assert_eq!(
        state.terminal_modes(),
        &terminal_core::TerminalModes::default()
    );
    assert_eq!(state.input_modes(), &terminal_core::InputModes::default());
}

#[test]
fn resize_then_reset_keeps_resized_dimensions_and_restores_invariants() {
    let mut state = TerminalState::new(TerminalDimensions::new(4, 4).unwrap());
    state.set_cursor_position(3, 3).unwrap();
    let resized = TerminalDimensions::new(2, 3).unwrap();
    state.resize(resized);
    state.set_cursor_position(2, 1).unwrap();
    state.set_cursor_visibility(CursorVisibility::Hidden);

    state.reset();

    assert_eq!(state.dimensions(), resized);
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 0));
    assert_eq!(
        state.terminal_modes(),
        &terminal_core::TerminalModes::default()
    );
    assert_eq!(state.input_modes(), &terminal_core::InputModes::default());
}
