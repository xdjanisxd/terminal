use terminal_core::{
    AutoWrapMode, CharacterInsertionMode, CursorError, CursorKeyMode, CursorVisibility, PrintError,
    TerminalDimensions, TerminalState,
};

fn row_text(state: &TerminalState, row: usize) -> String {
    (0..state.dimensions().columns())
        .map(|column| state.screen().cell(row, column).unwrap().character())
        .collect()
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

    state.move_cursor(-10, 10);
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
