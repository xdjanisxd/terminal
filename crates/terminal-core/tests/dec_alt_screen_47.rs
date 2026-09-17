use terminal_core::{
    AutoWrapMode, CellColor, CellOccupancy, CharacterInsertionMode, CursorKeyMode,
    CursorVisibility, ScreenKind, TerminalDimensions, TerminalParser, TerminalReply, TerminalState,
    VerticalScrollingMargins,
};

const ACUTE: char = '\u{0301}';

fn state() -> TerminalState {
    TerminalState::new(TerminalDimensions::new(6, 3).unwrap())
}

fn row_text(state: &TerminalState, row: usize) -> String {
    (0..state.dimensions().columns())
        .map(|column| state.screen().cell(row, column).unwrap().character())
        .collect()
}

#[test]
fn dec_private_mode_47_selects_independent_screens_without_clearing() {
    let mut parser = TerminalParser::new();
    let mut state = state();
    state.print_character('p').unwrap();
    assert!(state.set_vertical_scrolling_margins(1, 2));
    state.set_cursor_position(2, 4).unwrap();

    parser.advance(&mut state, b"\x1b[?47h").unwrap();
    assert_eq!(state.active_screen(), ScreenKind::Alternate);
    assert_eq!(row_text(&state, 0), "      ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 0));
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::full_screen(3)
    );

    state.print_character('a').unwrap();
    assert!(state.set_vertical_scrolling_margins(0, 1));
    state.set_cursor_position(1, 2).unwrap();
    parser.advance(&mut state, b"\x1b[?47l").unwrap();
    assert_eq!(state.active_screen(), ScreenKind::Primary);
    assert_eq!(row_text(&state, 0), "p     ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (2, 4));
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::new(1, 2, 3).unwrap()
    );

    parser.advance(&mut state, b"\x1b[?47h").unwrap();
    assert_eq!(state.active_screen(), ScreenKind::Alternate);
    assert_eq!(row_text(&state, 0), "a     ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 2));
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::new(0, 1, 3).unwrap()
    );
}

#[test]
fn dec_private_mode_47_is_idempotent_and_preserves_shared_state() {
    let mut parser = TerminalParser::new();
    let mut state = state();
    state.set_foreground_color(CellColor::Indexed(196));
    state.set_character_insertion(CharacterInsertionMode::Insert);
    state.set_auto_wrap(AutoWrapMode::Disabled);
    state.set_cursor_visibility(CursorVisibility::Hidden);
    state.set_cursor_key_mode(CursorKeyMode::Application);
    state.clear_all_horizontal_tab_stops();
    assert!(state.request_primary_device_attributes());
    state.set_cursor_position(0, 5).unwrap();
    state.print_character('p').unwrap();

    let rendition = *state.current_rendition();
    let terminal_modes = *state.terminal_modes();
    let input_modes = *state.input_modes();
    parser
        .advance(&mut state, b"\x1b[?47h\x1b[?47h\x1b[?47l\x1b[?47l")
        .unwrap();

    assert_eq!(state.active_screen(), ScreenKind::Primary);
    assert_eq!(row_text(&state, 0), "     p");
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
fn dec_private_mode_47_is_chunk_invariant_and_keeps_printable_text_on_active_screen() {
    let input = b"p\x1b[?47ha\x1b[?47lq";
    let mut expected_parser = TerminalParser::new();
    let mut expected = state();
    expected_parser.advance(&mut expected, input).unwrap();
    let expected_primary = row_text(&expected, 0);
    expected.switch_to_alternate_screen();
    let expected_alternate = row_text(&expected, 0);

    for split in 0..=input.len() {
        let mut parser = TerminalParser::new();
        let mut actual = state();
        parser.advance(&mut actual, &input[..split]).unwrap();
        parser.advance(&mut actual, &input[split..]).unwrap();
        assert_eq!(actual.active_screen(), ScreenKind::Primary, "split {split}");
        assert_eq!(row_text(&actual, 0), expected_primary, "split {split}");

        actual.switch_to_alternate_screen();
        assert_eq!(row_text(&actual, 0), expected_alternate, "split {split}");
    }

    let mut parser = TerminalParser::new();
    let mut incomplete = state();
    parser.advance(&mut incomplete, b"\x1b[?47").unwrap();
    assert_eq!(incomplete.active_screen(), ScreenKind::Primary);
}

#[test]
fn dec_private_mode_47_handles_mixed_parameters_and_isolates_neighboring_modes() {
    let mut parser = TerminalParser::new();
    let mut state = state();

    parser.advance(&mut state, b"\x1b[?47;999h").unwrap();
    assert_eq!(state.active_screen(), ScreenKind::Alternate);
    parser.advance(&mut state, b"\x1b[?999;47l").unwrap();
    assert_eq!(state.active_screen(), ScreenKind::Primary);
    parser
        .advance(&mut state, b"\x1b[?999;47h\x1b[?47;999l")
        .unwrap();
    assert_eq!(state.active_screen(), ScreenKind::Primary);

    parser
        .advance(
            &mut state,
            b"\x1b[47h\x1b[47l\x1b[?1047h\x1b[?1047l\x1b[?1049h\x1b[?1049l\x1b[?999h",
        )
        .unwrap();
    assert_eq!(state.active_screen(), ScreenKind::Primary);
}

#[test]
fn dec_private_mode_47_preserves_wide_combining_cells_across_resize_and_reset() {
    let mut parser = TerminalParser::new();
    let mut state = state();
    state.print_character('界').unwrap();
    state.print_character(ACUTE).unwrap();
    parser.advance(&mut state, b"\x1b[?47h").unwrap();
    state.print_character('界').unwrap();
    state.print_character(ACUTE).unwrap();
    state.resize(TerminalDimensions::new(4, 2).unwrap());

    parser.advance(&mut state, b"\x1b[?47l").unwrap();
    assert_eq!(
        state.screen().cell(0, 0).unwrap().occupancy(),
        CellOccupancy::WideLead
    );
    assert_eq!(
        state.screen().cell(0, 0).unwrap().combining_marks(),
        [ACUTE]
    );
    assert_eq!(
        state.screen().cell(0, 1).unwrap().occupancy(),
        CellOccupancy::WideContinuation
    );
    parser.advance(&mut state, b"\x1b[?47h").unwrap();
    assert_eq!(
        state.screen().cell(0, 0).unwrap().occupancy(),
        CellOccupancy::WideLead
    );
    assert_eq!(
        state.screen().cell(0, 0).unwrap().combining_marks(),
        [ACUTE]
    );

    state.reset();
    assert_eq!(state.active_screen(), ScreenKind::Primary);
    assert_eq!(row_text(&state, 0), "    ");
    state.switch_to_alternate_screen();
    assert_eq!(row_text(&state, 0), "    ");
}
