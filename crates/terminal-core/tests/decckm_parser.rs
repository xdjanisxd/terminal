use terminal_core::{
    AutoWrapMode, CellColor, CharacterInsertionMode, CursorKeyMode, CursorVisibility,
    TerminalDimensions, TerminalParser, TerminalReply, TerminalState,
};

fn row_text(state: &TerminalState, row: usize) -> String {
    (0..state.dimensions().columns())
        .map(|column| state.screen().cell(row, column).unwrap().character())
        .collect()
}

#[test]
fn decckm_private_mode_one_transitions_application_and_normal_cursor_keys() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(8, 2).unwrap());

    parser.advance(&mut state, b"before\x1b[?1hafter").unwrap();
    assert_eq!(
        state.input_modes().cursor_keys(),
        CursorKeyMode::Application
    );
    assert_eq!(row_text(&state, 0), "beforeaf");
    assert_eq!(row_text(&state, 1), "ter     ");

    parser.advance(&mut state, b"\x1b[?1l").unwrap();
    assert_eq!(state.input_modes().cursor_keys(), CursorKeyMode::Normal);

    parser
        .advance(&mut state, b"\x1b[?1l\x1b[?1h\x1b[?1h")
        .unwrap();
    assert_eq!(
        state.input_modes().cursor_keys(),
        CursorKeyMode::Application
    );
}

#[test]
fn decckm_private_mode_one_is_chunk_invariant_and_incomplete_sequences_are_safe() {
    let input = b"a\x1b[?1hB\x1b[?1lC\x1b[?1h";
    let mut expected_parser = TerminalParser::new();
    let mut expected = TerminalState::new(TerminalDimensions::new(8, 2).unwrap());
    expected_parser.advance(&mut expected, input).unwrap();

    for split in 0..=input.len() {
        let mut parser = TerminalParser::new();
        let mut actual = TerminalState::new(TerminalDimensions::new(8, 2).unwrap());
        parser.advance(&mut actual, &input[..split]).unwrap();
        parser.advance(&mut actual, &input[split..]).unwrap();
        assert_eq!(
            actual.input_modes(),
            expected.input_modes(),
            "split {split}"
        );
        assert_eq!(actual.cursor(), expected.cursor(), "split {split}");
        assert_eq!(
            row_text(&actual, 0),
            row_text(&expected, 0),
            "split {split}"
        );
        assert_eq!(
            row_text(&actual, 1),
            row_text(&expected, 1),
            "split {split}"
        );
    }

    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(4, 1).unwrap());
    parser.advance(&mut state, b"\x1b[?1").unwrap();
    assert_eq!(state.input_modes().cursor_keys(), CursorKeyMode::Normal);
}

#[test]
fn decckm_handles_private_parameters_independently_in_parser_order() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(4, 1).unwrap());

    parser.advance(&mut state, b"\x1b[?1;999h").unwrap();
    assert_eq!(
        state.input_modes().cursor_keys(),
        CursorKeyMode::Application
    );

    parser.advance(&mut state, b"\x1b[?999;1l").unwrap();
    assert_eq!(state.input_modes().cursor_keys(), CursorKeyMode::Normal);

    parser
        .advance(&mut state, b"\x1b[?999;1h\x1b[?1;999l")
        .unwrap();
    assert_eq!(state.input_modes().cursor_keys(), CursorKeyMode::Normal);
}

#[test]
fn decckm_isolated_from_standard_and_unrelated_private_modes() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(4, 1).unwrap());
    state.set_cursor_key_mode(CursorKeyMode::Application);

    parser
        .advance(&mut state, b"\x1b[1h\x1b[1l\x1b[?999h\x1b[?999l\x1b[?1:h")
        .unwrap();

    assert_eq!(
        state.input_modes().cursor_keys(),
        CursorKeyMode::Application
    );
    assert_eq!(state.terminal_modes().auto_wrap(), AutoWrapMode::Enabled);
    assert_eq!(
        state.terminal_modes().character_insertion(),
        CharacterInsertionMode::Replace
    );
    assert_eq!(
        state.terminal_modes().cursor_visibility(),
        CursorVisibility::Visible
    );
}

#[test]
fn decckm_changes_only_cursor_key_mode_and_preserves_terminal_state() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(8, 4).unwrap());
    parser.advance(&mut state, b"content").unwrap();
    state.set_cursor_position(2, 7).unwrap();
    state.print_character('Z').unwrap();
    state.set_foreground_color(CellColor::Indexed(196));
    state.set_vertical_scrolling_margins(1, 3);
    state.set_cursor_position(2, 7).unwrap();
    state.print_character('Z').unwrap();
    state.clear_all_horizontal_tab_stops();
    state.set_auto_wrap(AutoWrapMode::Enabled);
    state.set_character_insertion(CharacterInsertionMode::Insert);
    state.set_cursor_visibility(CursorVisibility::Hidden);
    assert!(state.request_cursor_position_report());

    let cursor = state.cursor();
    let rendition = *state.current_rendition();
    let margins = state.vertical_scrolling_margins();
    let terminal_modes = *state.terminal_modes();
    let cells: Vec<_> = (0..state.dimensions().rows())
        .map(|row| row_text(&state, row))
        .collect();

    parser.advance(&mut state, b"\x1b[?1h").unwrap();

    assert_eq!(
        state.input_modes().cursor_keys(),
        CursorKeyMode::Application
    );
    assert_eq!(state.cursor(), cursor);
    assert_eq!(*state.current_rendition(), rendition);
    assert_eq!(state.vertical_scrolling_margins(), margins);
    assert_eq!(*state.terminal_modes(), terminal_modes);
    assert!((0..state.dimensions().columns()).all(|column| !state.has_horizontal_tab_stop(column)));
    assert_eq!(
        (0..state.dimensions().rows())
            .map(|row| row_text(&state, row))
            .collect::<Vec<_>>(),
        cells
    );
    assert_eq!(
        state.take_reply(),
        Some(TerminalReply::CursorPosition { row: 3, column: 8 })
    );

    state.print_character('Y').unwrap();
    assert_eq!(state.cursor().row(), 3);
    assert_eq!(state.cursor().column(), 1);
    assert_eq!(state.screen().cell(3, 0).unwrap().character(), 'Y');
}
