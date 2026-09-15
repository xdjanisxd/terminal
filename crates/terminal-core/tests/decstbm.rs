use terminal_core::{
    AutoWrapMode, CellColor, CharacterInsertionMode, CursorKeyMode, CursorVisibility,
    TerminalDimensions, TerminalParser, TerminalState, TextIntensity, VerticalScrollingMargins,
};

fn parse(sequence: &[u8], columns: usize, rows: usize) -> TerminalState {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(columns, rows).unwrap());
    parser.advance(&mut state, sequence).unwrap();
    state
}

#[test]
fn decstbm_accepts_full_explicit_and_partial_default_forms() {
    let cases: &[(&[u8], usize, usize)] = &[
        (b"\x1b[r", 0, 5),
        (b"\x1b[1;6r", 0, 5),
        (b"\x1b[2;5r", 1, 4),
        (b"\x1b[;5r", 0, 4),
        (b"\x1b[2;r", 1, 5),
        (b"\x1b[;r", 0, 5),
        (b"\x1b[0;5r", 0, 4),
        (b"\x1b[2;0r", 1, 5),
        (b"\x1b[0;0r", 0, 5),
    ];

    for (sequence, top, bottom) in cases {
        let state = parse(sequence, 8, 6);
        assert_eq!(
            state.vertical_scrolling_margins(),
            VerticalScrollingMargins::new(*top, *bottom, 6).unwrap(),
            "sequence {sequence:?}"
        );
        assert_eq!((state.cursor().row(), state.cursor().column()), (0, 0));
    }
}

#[test]
fn valid_decstbm_homes_cursor_cancels_wrap_and_preserves_unrelated_state() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(10, 6).unwrap());
    state.set_cursor_visibility(CursorVisibility::Hidden);
    state.set_auto_wrap(AutoWrapMode::Enabled);
    state.set_character_insertion(CharacterInsertionMode::Insert);
    state.set_cursor_key_mode(CursorKeyMode::Application);
    state.set_text_intensity(TextIntensity::Bold);
    state.set_foreground_color(CellColor::Indexed(196));
    state.set_cursor_position(3, 5).unwrap();
    state.set_horizontal_tab_stop();
    state.set_cursor_position(0, 0).unwrap();
    for character in "abcdefghij".chars() {
        state.print_character(character).unwrap();
    }
    let terminal_modes = *state.terminal_modes();
    let input_modes = *state.input_modes();
    let rendition = *state.current_rendition();

    parser.advance(&mut state, b"\x1b[2;5r").unwrap();

    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 0));
    assert_eq!(*state.terminal_modes(), terminal_modes);
    assert_eq!(*state.input_modes(), input_modes);
    assert_eq!(*state.current_rendition(), rendition);
    assert!(state.has_horizontal_tab_stop(5));
    state.print_character('X').unwrap();
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 1));
    assert_eq!(state.screen().cell(0, 0).unwrap().character(), 'X');
}

#[test]
fn invalid_decstbm_is_an_atomic_no_op_and_does_not_home_cursor() {
    let invalid: &[&[u8]] = &[
        b"\x1b[3;3r",
        b"\x1b[4;3r",
        b"\x1b[7;5r",
        b"\x1b[7;8r",
        b"\x1b[2;7r",
        b"\x1b[65535;65535r",
        b"\x1b[999999999999999999999;5r",
        b"\x1b[2;5;6r",
        b"\x1b[2:3;5r",
        b"\x1b[2;5:6r",
    ];

    for sequence in invalid {
        let mut parser = TerminalParser::new();
        let mut state = TerminalState::new(TerminalDimensions::new(8, 6).unwrap());
        parser.advance(&mut state, b"\x1b[2;5r").unwrap();
        state.set_cursor_position(3, 4).unwrap();
        let margins = state.vertical_scrolling_margins();
        let cursor = state.cursor();

        parser.advance(&mut state, sequence).unwrap();

        assert_eq!(state.vertical_scrolling_margins(), margins, "{sequence:?}");
        assert_eq!(state.cursor(), cursor, "{sequence:?}");
    }
}

#[test]
fn invalid_decstbm_preserves_delayed_wrap() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(2, 3).unwrap());
    state.print_character('a').unwrap();
    state.print_character('b').unwrap();

    parser.advance(&mut state, b"\x1b[2;2r").unwrap();
    state.print_character('X').unwrap();

    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 1));
    assert_eq!(state.screen().cell(1, 0).unwrap().character(), 'X');
}

#[test]
fn incomplete_decstbm_has_no_effect_until_terminated() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(8, 6).unwrap());
    state.set_cursor_position(3, 4).unwrap();

    parser.advance(&mut state, b"\x1b[2;5").unwrap();
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::full_screen(6)
    );
    assert_eq!((state.cursor().row(), state.cursor().column()), (3, 4));

    parser.advance(&mut state, b"r").unwrap();
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::new(1, 4, 6).unwrap()
    );
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 0));
}

#[test]
fn decstbm_is_chunk_safe_at_every_byte_boundary() {
    let input = b"abc\x1b[2;5rX\x1b[;4rY\x1b[rZ";
    let mut expected_parser = TerminalParser::new();
    let mut expected = TerminalState::new(TerminalDimensions::new(12, 6).unwrap());
    expected_parser.advance(&mut expected, input).unwrap();

    for split in 0..=input.len() {
        let mut parser = TerminalParser::new();
        let mut state = TerminalState::new(TerminalDimensions::new(12, 6).unwrap());
        parser.advance(&mut state, &input[..split]).unwrap();
        parser.advance(&mut state, &input[split..]).unwrap();
        assert_eq!(
            state.vertical_scrolling_margins(),
            expected.vertical_scrolling_margins()
        );
        assert_eq!(state.cursor(), expected.cursor());
        assert_eq!(state.terminal_modes(), expected.terminal_modes());
        assert_eq!(state.input_modes(), expected.input_modes());
        assert_eq!(state.current_rendition(), expected.current_rendition());
        for row in 0..state.dimensions().rows() {
            for column in 0..state.dimensions().columns() {
                assert_eq!(
                    state.screen().cell(row, column),
                    expected.screen().cell(row, column)
                );
            }
        }
    }
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(12, 6).unwrap());
    for byte in input {
        parser
            .advance(&mut state, std::slice::from_ref(byte))
            .unwrap();
    }
    assert_eq!(
        state.vertical_scrolling_margins(),
        expected.vertical_scrolling_margins()
    );
    assert_eq!(state.cursor(), expected.cursor());
}

#[test]
fn decstbm_defaults_remain_valid_on_a_single_row_screen() {
    for sequence in [b"\x1b[r".as_slice(), b"\x1b[;r", b"\x1b[1;1r"] {
        let state = parse(sequence, 8, 1);
        assert_eq!(
            state.vertical_scrolling_margins(),
            VerticalScrollingMargins::full_screen(1),
            "sequence {sequence:?}"
        );
    }
}
