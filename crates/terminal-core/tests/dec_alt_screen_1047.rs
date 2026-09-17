use terminal_core::{
    AutoWrapMode, CellColor, CellOccupancy, CharacterInsertionMode, CursorKeyMode,
    CursorVisibility, ScreenKind, TerminalDimensions, TerminalParser, TerminalReply, TerminalState,
    VerticalScrollingMargins,
};

fn state() -> TerminalState {
    TerminalState::new(TerminalDimensions::new(6, 3).unwrap())
}

fn row_text(state: &TerminalState, row: usize) -> String {
    (0..state.dimensions().columns())
        .map(|column| state.screen().cell(row, column).unwrap().character())
        .collect()
}

#[test]
fn dec_private_mode_1047_entry_resets_the_alternate_screen_locally() {
    let mut parser = TerminalParser::new();
    let mut state = state();
    state.print_character('p').unwrap();
    assert!(state.set_vertical_scrolling_margins(1, 2));
    state.set_cursor_position(2, 4).unwrap();

    parser.advance(&mut state, b"\x1b[?47h").unwrap();
    state.print_character('a').unwrap();
    assert!(state.set_vertical_scrolling_margins(0, 1));
    state.set_cursor_position(2, 5).unwrap();
    state.print_character('w').unwrap();
    parser.advance(&mut state, b"\x1b[?47l").unwrap();

    parser.advance(&mut state, b"\x1b[?1047h").unwrap();

    assert_eq!(state.active_screen(), ScreenKind::Alternate);
    assert_eq!(row_text(&state, 0), "      ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 0));
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::full_screen(3)
    );
    state.print_character('x').unwrap();
    assert_eq!(state.screen().cell(0, 0).unwrap().character(), 'x');

    parser.advance(&mut state, b"\x1b[?1047l").unwrap();
    assert_eq!(state.active_screen(), ScreenKind::Primary);
    assert_eq!(row_text(&state, 0), "p     ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (2, 4));
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::new(1, 2, 3).unwrap()
    );
}

#[test]
fn dec_private_mode_1047_repeated_entry_clears_again_and_exit_is_idempotent() {
    let mut parser = TerminalParser::new();
    let mut state = state();

    parser.advance(&mut state, b"\x1b[?1047h").unwrap();
    state.print_character('a').unwrap();
    parser.advance(&mut state, b"\x1b[?1047h").unwrap();
    assert_eq!(state.active_screen(), ScreenKind::Alternate);
    assert_eq!(row_text(&state, 0), "      ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 0));

    parser
        .advance(&mut state, b"\x1b[?1047l\x1b[?1047l")
        .unwrap();
    assert_eq!(state.active_screen(), ScreenKind::Primary);
}

#[test]
fn dec_private_mode_1047_preserves_terminal_global_state() {
    let mut parser = TerminalParser::new();
    let mut state = state();
    state.set_foreground_color(CellColor::Indexed(196));
    state.set_character_insertion(CharacterInsertionMode::Insert);
    state.set_auto_wrap(AutoWrapMode::Disabled);
    state.set_cursor_visibility(CursorVisibility::Hidden);
    state.set_cursor_key_mode(CursorKeyMode::Application);
    state.clear_all_horizontal_tab_stops();
    assert!(state.request_primary_device_attributes());

    let rendition = *state.current_rendition();
    let terminal_modes = *state.terminal_modes();
    let input_modes = *state.input_modes();
    parser
        .advance(&mut state, b"\x1b[?1047h\x1b[?1047l")
        .unwrap();

    assert_eq!(*state.current_rendition(), rendition);
    assert_eq!(*state.terminal_modes(), terminal_modes);
    assert_eq!(*state.input_modes(), input_modes);
    assert!((0..state.dimensions().columns()).all(|column| !state.has_horizontal_tab_stop(column)));
    assert_eq!(
        state.take_reply(),
        Some(TerminalReply::PrimaryDeviceAttributes)
    );
}

#[test]
fn dec_private_mode_1047_clears_wide_combining_alternate_cells_but_not_primary_cells() {
    let mut parser = TerminalParser::new();
    let mut state = state();
    state.print_character('界').unwrap();
    state.print_character('\u{0301}').unwrap();

    parser.advance(&mut state, b"\x1b[?47h").unwrap();
    state.print_character('界').unwrap();
    state.print_character('\u{0301}').unwrap();
    parser.advance(&mut state, b"\x1b[?47l\x1b[?1047h").unwrap();

    assert_eq!(state.screen().cell(0, 0).unwrap().character(), ' ');
    assert_eq!(
        state.screen().cell(0, 0).unwrap().occupancy(),
        CellOccupancy::Single
    );
    assert!(
        state
            .screen()
            .cell(0, 0)
            .unwrap()
            .combining_marks()
            .is_empty()
    );

    parser.advance(&mut state, b"\x1b[?1047l").unwrap();
    assert_eq!(
        state.screen().cell(0, 0).unwrap().occupancy(),
        CellOccupancy::WideLead
    );
    assert_eq!(
        state.screen().cell(0, 0).unwrap().combining_marks(),
        ['\u{0301}']
    );
}

#[test]
fn dec_private_mode_1047_is_distinct_from_47_and_rejects_malformed_mode_forms() {
    let mut parser = TerminalParser::new();
    let mut state = state();

    parser.advance(&mut state, b"\x1b[?47h").unwrap();
    state.print_character('a').unwrap();
    parser.advance(&mut state, b"\x1b[?47l\x1b[?47h").unwrap();
    assert_eq!(row_text(&state, 0), "a     ");

    parser.advance(&mut state, b"\x1b[?47l\x1b[?1047h").unwrap();
    assert_eq!(row_text(&state, 0), "      ");
    parser.advance(&mut state, b"\x1b[?1047l").unwrap();
    assert_eq!(state.active_screen(), ScreenKind::Primary);

    parser
        .advance(
            &mut state,
            b"\x1b[1047h\x1b[1047l\x1b[?999;1047h\x1b[?1047;999l",
        )
        .unwrap();
    assert_eq!(state.active_screen(), ScreenKind::Primary);
}

#[test]
fn dec_private_mode_1047_clears_the_resized_alternate_and_terminal_reset_is_unchanged() {
    let mut parser = TerminalParser::new();
    let mut state = state();
    parser.advance(&mut state, b"\x1b[?47h").unwrap();
    state.print_character('a').unwrap();
    state.resize(TerminalDimensions::new(4, 2).unwrap());
    parser.advance(&mut state, b"\x1b[?1047h").unwrap();

    assert_eq!(state.dimensions(), TerminalDimensions::new(4, 2).unwrap());
    assert_eq!(row_text(&state, 0), "    ");
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::full_screen(2)
    );

    state.reset();
    assert_eq!(state.active_screen(), ScreenKind::Primary);
    assert_eq!(row_text(&state, 0), "    ");
    state.switch_to_alternate_screen();
    assert_eq!(row_text(&state, 0), "    ");
}
