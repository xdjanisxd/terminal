use terminal_core::{
    AutoWrapMode, CharacterInsertionMode, CursorError, CursorKeyMode, CursorVisibility,
    TerminalDimensions, TerminalState,
};

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
