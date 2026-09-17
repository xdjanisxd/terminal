use terminal_core::{
    AutoWrapMode, CellColor, CharacterInsertionMode, CursorKeyMode, CursorVisibility, ScreenKind,
    TerminalDimensions, TerminalParser, TerminalReply, TerminalState, VerticalScrollingMargins,
};

fn state() -> TerminalState {
    TerminalState::new(TerminalDimensions::new(6, 4).unwrap())
}

fn cursor(state: &TerminalState) -> (usize, usize) {
    (state.cursor().row(), state.cursor().column())
}

fn cells(state: &TerminalState) -> Vec<terminal_core::Cell> {
    let screen = state.screen();
    let dimensions = state.dimensions();
    (0..dimensions.rows())
        .flat_map(|row| {
            (0..dimensions.columns()).map(move |column| *screen.cell(row, column).unwrap())
        })
        .collect()
}

#[test]
fn save_move_restore_restores_the_active_cursor_coordinates_without_touching_cells() {
    let mut state = state();
    state.print_character('p').unwrap();
    state.set_cursor_position(2, 4).unwrap();
    let snapshot = cells(&state);

    state.save_cursor();
    state.set_cursor_position(0, 0).unwrap();
    state.restore_cursor();

    assert_eq!(cursor(&state), (2, 4));
    assert_eq!(cells(&state), snapshot);
}

#[test]
fn latest_save_wins_and_repeated_restore_is_stable() {
    let mut state = state();
    state.set_cursor_position(1, 1).unwrap();
    state.save_cursor();
    state.set_cursor_position(3, 5).unwrap();
    state.save_cursor();
    state.set_cursor_position(0, 0).unwrap();

    state.restore_cursor();
    assert_eq!(cursor(&state), (3, 5));
    state.set_cursor_position(2, 2).unwrap();
    state.restore_cursor();
    assert_eq!(cursor(&state), (3, 5));
}

#[test]
fn restore_before_save_is_a_safe_no_op() {
    let mut state = state();
    state.set_cursor_position(2, 3).unwrap();

    state.restore_cursor();

    assert_eq!(cursor(&state), (2, 3));
}

#[test]
fn saves_are_screen_local_and_switching_preserves_both_slots() {
    let mut state = state();
    state.set_cursor_position(1, 2).unwrap();
    state.save_cursor();

    state.switch_to_alternate_screen();
    state.set_cursor_position(3, 4).unwrap();
    state.save_cursor();
    state.set_cursor_position(0, 0).unwrap();
    state.restore_cursor();
    assert_eq!(state.active_screen(), ScreenKind::Alternate);
    assert_eq!(cursor(&state), (3, 4));

    state.switch_to_primary_screen();
    state.set_cursor_position(0, 0).unwrap();
    state.restore_cursor();
    assert_eq!(state.active_screen(), ScreenKind::Primary);
    assert_eq!(cursor(&state), (1, 2));
}

#[test]
fn dec_private_mode_47_round_trip_does_not_overwrite_either_saved_cursor_slot() {
    let mut parser = TerminalParser::new();
    let mut state = state();
    state.set_cursor_position(1, 2).unwrap();
    state.save_cursor();
    parser.advance(&mut state, b"\x1b[?47h").unwrap();
    state.set_cursor_position(3, 4).unwrap();
    state.save_cursor();
    parser.advance(&mut state, b"\x1b[?47l\x1b[?47h").unwrap();
    state.set_cursor_position(0, 0).unwrap();
    state.restore_cursor();
    assert_eq!(cursor(&state), (3, 4));

    parser.advance(&mut state, b"\x1b[?47l").unwrap();
    state.set_cursor_position(0, 0).unwrap();
    state.restore_cursor();
    assert_eq!(cursor(&state), (1, 2));
}

#[test]
fn dec_private_mode_1047_entry_resets_alternate_saved_cursor_and_exit_does_not_restore_primary() {
    let mut parser = TerminalParser::new();
    let mut state = state();
    state.set_cursor_position(2, 5).unwrap();
    state.save_cursor();
    parser.advance(&mut state, b"\x1b[?47h").unwrap();
    state.set_cursor_position(3, 4).unwrap();
    state.save_cursor();

    parser.advance(&mut state, b"\x1b[?1047h").unwrap();
    state.set_cursor_position(1, 1).unwrap();
    state.restore_cursor();
    assert_eq!(cursor(&state), (1, 1));

    parser.advance(&mut state, b"\x1b[?1047l").unwrap();
    assert_eq!(cursor(&state), (2, 5));
    state.set_cursor_position(0, 0).unwrap();
    state.restore_cursor();
    assert_eq!(cursor(&state), (2, 5));
}

#[test]
fn restore_clamps_saved_coordinates_after_shrink_and_keeps_them_valid_after_grow() {
    let mut state = state();
    state.set_cursor_position(3, 5).unwrap();
    state.save_cursor();
    state.resize(TerminalDimensions::new(3, 2).unwrap());
    state.set_cursor_position(0, 0).unwrap();
    state.restore_cursor();
    assert_eq!(cursor(&state), (1, 2));

    state.resize(TerminalDimensions::new(6, 4).unwrap());
    state.set_cursor_position(0, 0).unwrap();
    state.restore_cursor();
    assert_eq!(cursor(&state), (3, 5));
}

#[test]
fn reset_returns_both_saved_cursor_slots_to_the_uninitialized_no_op_state() {
    let mut state = state();
    state.set_cursor_position(2, 5).unwrap();
    state.save_cursor();
    state.switch_to_alternate_screen();
    state.set_cursor_position(3, 4).unwrap();
    state.save_cursor();

    state.reset();
    assert_eq!(state.active_screen(), ScreenKind::Primary);
    state.set_cursor_position(1, 1).unwrap();
    state.restore_cursor();
    assert_eq!(cursor(&state), (1, 1));
    state.switch_to_alternate_screen();
    state.set_cursor_position(1, 2).unwrap();
    state.restore_cursor();
    assert_eq!(cursor(&state), (1, 2));
}

#[test]
fn save_and_restore_capture_rendition_but_not_delayed_wrap_or_other_state() {
    let mut state = state();
    state.set_foreground_color(CellColor::Indexed(196));
    assert!(state.set_vertical_scrolling_margins(1, 3));
    state.set_auto_wrap(AutoWrapMode::Enabled);
    state.set_cursor_position(0, 5).unwrap();
    state.print_character('x').unwrap();
    state.set_character_insertion(CharacterInsertionMode::Insert);
    state.set_cursor_visibility(CursorVisibility::Hidden);
    state.set_cursor_key_mode(CursorKeyMode::Application);
    state.set_horizontal_tab_stop();
    assert!(state.request_primary_device_attributes());
    let snapshot = cells(&state);

    state.save_cursor();
    state.set_foreground_color(CellColor::Indexed(22));
    state.set_auto_wrap(AutoWrapMode::Disabled);
    assert!(state.set_vertical_scrolling_margins(0, 1));
    state.restore_cursor();

    assert_eq!(cursor(&state), (0, 5));
    assert_eq!(
        state.current_rendition().foreground(),
        CellColor::Indexed(196)
    );
    assert_eq!(cells(&state), snapshot);
    state.print_character('y').unwrap();
    assert_eq!(state.screen().cell(0, 5).unwrap().character(), 'y');
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::new(0, 1, 4).unwrap()
    );
    assert_eq!(
        state.terminal_modes().character_insertion(),
        CharacterInsertionMode::Insert
    );
    assert_eq!(
        state.terminal_modes().cursor_visibility(),
        CursorVisibility::Hidden
    );
    assert_eq!(
        state.input_modes().cursor_keys(),
        CursorKeyMode::Application
    );
    assert!(state.has_horizontal_tab_stop(5));
    assert_eq!(
        state.take_reply(),
        Some(TerminalReply::PrimaryDeviceAttributes)
    );
}
