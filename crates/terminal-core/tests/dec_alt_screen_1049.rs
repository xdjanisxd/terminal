use terminal_core::{
    AutoWrapMode, CellColor, CellOccupancy, CharacterInsertionMode, CursorKeyMode,
    CursorVisibility, ScreenKind, TerminalDimensions, TerminalParser, TerminalReply, TerminalState,
    TextIntensity, VerticalScrollingMargins,
};

fn state() -> TerminalState {
    TerminalState::new(TerminalDimensions::new(6, 4).unwrap())
}

fn cursor(state: &TerminalState) -> (usize, usize) {
    (state.cursor().row(), state.cursor().column())
}

fn row_text(state: &TerminalState, row: usize) -> String {
    (0..state.dimensions().columns())
        .map(|column| state.screen().cell(row, column).unwrap().character())
        .collect()
}

#[test]
fn dec_private_mode_1049_saves_primary_resets_alternate_and_restores_primary() {
    let mut parser = TerminalParser::new();
    let mut state = state();
    state.set_text_intensity(TextIntensity::Bold);
    state.set_foreground_color(CellColor::Indexed(196));
    state.print_character('p').unwrap();
    assert!(state.set_vertical_scrolling_margins(1, 3));
    state.set_cursor_position(2, 4).unwrap();

    parser.advance(&mut state, b"\x1b[?1049h").unwrap();

    assert_eq!(state.active_screen(), ScreenKind::Alternate);
    assert_eq!(row_text(&state, 0), "      ");
    assert_eq!(cursor(&state), (0, 0));
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::full_screen(4)
    );

    state.set_text_intensity(TextIntensity::Faint);
    state.set_foreground_color(CellColor::Indexed(22));
    state.print_character('a').unwrap();
    parser.advance(&mut state, b"\x1b[?1049l").unwrap();

    assert_eq!(state.active_screen(), ScreenKind::Primary);
    assert_eq!(row_text(&state, 0), "p     ");
    assert_eq!(cursor(&state), (2, 4));
    assert_eq!(state.current_rendition().intensity(), TextIntensity::Bold);
    assert_eq!(
        state.current_rendition().foreground(),
        CellColor::Indexed(196)
    );
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::new(1, 3, 4).unwrap()
    );
}

#[test]
fn repeated_1049_entry_while_alternate_is_active_is_idempotent_and_preserves_primary_save() {
    let mut parser = TerminalParser::new();
    let mut state = state();
    state.set_cursor_position(2, 5).unwrap();
    state.save_cursor();
    state.set_cursor_position(1, 3).unwrap();

    parser.advance(&mut state, b"\x1b[?1049h").unwrap();
    state.print_character('a').unwrap();
    state.set_cursor_position(3, 4).unwrap();
    parser.advance(&mut state, b"\x1b[?1049h").unwrap();

    assert_eq!(state.active_screen(), ScreenKind::Alternate);
    assert_eq!(row_text(&state, 0), "a     ");
    assert_eq!(cursor(&state), (3, 4));

    parser
        .advance(&mut state, b"\x1b[?1049l\x1b[?1049l")
        .unwrap();
    assert_eq!(state.active_screen(), ScreenKind::Primary);
    assert_eq!(cursor(&state), (1, 3));
}

#[test]
fn dec_private_mode_1049_preserves_global_state_and_primary_wide_combining_cells() {
    let mut parser = TerminalParser::new();
    let mut state = state();
    state.print_character('界').unwrap();
    state.print_character('\u{0301}').unwrap();
    state.set_character_insertion(CharacterInsertionMode::Insert);
    state.set_auto_wrap(AutoWrapMode::Disabled);
    state.set_cursor_visibility(CursorVisibility::Hidden);
    state.set_cursor_key_mode(CursorKeyMode::Application);
    state.set_cursor_position(0, 5).unwrap();
    state.set_horizontal_tab_stop();
    assert!(state.request_primary_device_attributes());

    parser
        .advance(&mut state, b"\x1b[?1049h\x1b[?1049l")
        .unwrap();

    assert_eq!(
        state.screen().cell(0, 0).unwrap().occupancy(),
        CellOccupancy::WideLead
    );
    assert_eq!(
        state.screen().cell(0, 0).unwrap().combining_marks(),
        ['\u{0301}']
    );
    assert_eq!(
        state.terminal_modes().character_insertion(),
        CharacterInsertionMode::Insert
    );
    assert_eq!(state.terminal_modes().auto_wrap(), AutoWrapMode::Disabled);
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

#[test]
fn dec_private_mode_1049_resets_stale_alternate_wide_combining_content() {
    let mut parser = TerminalParser::new();
    let mut state = state();
    parser.advance(&mut state, b"\x1b[?47h").unwrap();
    state.print_character('界').unwrap();
    state.print_character('\u{0301}').unwrap();
    parser.advance(&mut state, b"\x1b[?47l\x1b[?1049h").unwrap();

    assert_eq!(state.active_screen(), ScreenKind::Alternate);
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
}

#[test]
fn dec_private_mode_1049_restore_clamps_after_resize_and_reset_clears_saved_state() {
    let mut parser = TerminalParser::new();
    let mut state = state();
    state.set_cursor_position(3, 5).unwrap();

    parser.advance(&mut state, b"\x1b[?1049h").unwrap();
    state.resize(TerminalDimensions::new(3, 2).unwrap());
    parser.advance(&mut state, b"\x1b[?1049l").unwrap();
    assert_eq!(state.active_screen(), ScreenKind::Primary);
    assert_eq!(cursor(&state), (1, 2));

    parser.advance(&mut state, b"\x1b[?1049h").unwrap();
    state.reset();
    assert_eq!(state.active_screen(), ScreenKind::Primary);
    state.set_cursor_position(1, 1).unwrap();
    parser.advance(&mut state, b"\x1b[?1049l").unwrap();
    assert_eq!(cursor(&state), (1, 1));
}

#[test]
fn dec_private_mode_1049_is_chunk_safe_and_isolated_from_other_private_mode_forms() {
    for split in 1..b"\x1b[?1049h\x1b[?1049l".len() {
        let mut parser = TerminalParser::new();
        let mut state = state();
        state.set_cursor_position(2, 4).unwrap();
        let input = b"\x1b[?1049h\x1b[?1049l";
        parser.advance(&mut state, &input[..split]).unwrap();
        parser.advance(&mut state, &input[split..]).unwrap();
        assert_eq!(state.active_screen(), ScreenKind::Primary);
        assert_eq!(cursor(&state), (2, 4));
    }

    let mut parser = TerminalParser::new();
    let mut state = state();
    state.set_cursor_position(2, 4).unwrap();
    parser.advance(&mut state, b"\x1b[?1049").unwrap();
    assert_eq!(state.active_screen(), ScreenKind::Primary);
    parser
        .advance(
            &mut state,
            b"\x1b[1049h\x1b[1049l\x1b[>1049h\x1b[?999;1049h\x1b[?1049;999l",
        )
        .unwrap();
    assert_eq!(state.active_screen(), ScreenKind::Primary);
    assert_eq!(cursor(&state), (2, 4));
}
