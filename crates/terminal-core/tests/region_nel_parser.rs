use terminal_core::{TerminalDimensions, TerminalParser, TerminalState, VerticalScrollingMargins};

fn labeled_state() -> TerminalState {
    let mut state = TerminalState::new(TerminalDimensions::new(3, 6).unwrap());
    for (row, text) in ["aaa", "bbb", "ccc", "ddd", "eee", "fff"]
        .into_iter()
        .enumerate()
    {
        state.set_cursor_position(row, 0).unwrap();
        for character in text.chars() {
            state.print_character(character).unwrap();
        }
    }
    state
}

fn row_text(state: &TerminalState, row: usize) -> String {
    (0..state.dimensions().columns())
        .map(|column| state.screen().cell(row, column).unwrap().character())
        .collect()
}

fn parse(input: &[u8]) -> TerminalState {
    let mut parser = TerminalParser::new();
    let mut state = labeled_state();
    parser.advance(&mut state, input).unwrap();
    state
}

fn assert_observable_state_eq(actual: &TerminalState, expected: &TerminalState) {
    assert_eq!(actual.screen(), expected.screen());
    assert_eq!(actual.cursor(), expected.cursor());
    assert_eq!(actual.current_rendition(), expected.current_rendition());
    assert_eq!(actual.terminal_modes(), expected.terminal_modes());
    assert_eq!(actual.input_modes(), expected.input_modes());
    assert_eq!(
        actual.vertical_scrolling_margins(),
        expected.vertical_scrolling_margins()
    );
    for column in 0..actual.dimensions().columns() {
        assert_eq!(
            actual.has_horizontal_tab_stop(column),
            expected.has_horizontal_tab_stop(column)
        );
    }
}

#[test]
fn parser_routes_decstbm_followed_by_nel_through_active_region() {
    let state = parse(b"\x1b[2;5r\x1b[5;3H\x1bE");

    assert_eq!(
        (0..6).map(|row| row_text(&state, row)).collect::<Vec<_>>(),
        ["aaa", "ccc", "ddd", "eee", "   ", "fff"]
    );
    assert_eq!((state.cursor().row(), state.cursor().column()), (4, 0));
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::new(1, 4, 6).unwrap()
    );
}

#[test]
fn parser_region_nel_is_chunk_safe_at_every_input_boundary() {
    let input = b"\x1b[2;5r\x1b[5;3H\x1bEP";
    let expected = parse(input);

    for split in 0..=input.len() {
        let mut parser = TerminalParser::new();
        let mut state = labeled_state();
        parser.advance(&mut state, &input[..split]).unwrap();
        parser.advance(&mut state, &input[split..]).unwrap();
        assert_observable_state_eq(&state, &expected);
    }

    let mut parser = TerminalParser::new();
    let mut state = labeled_state();
    for byte in input {
        parser
            .advance(&mut state, std::slice::from_ref(byte))
            .unwrap();
    }
    assert_observable_state_eq(&state, &expected);
}

#[test]
fn printable_text_before_and_after_nel_uses_region_semantics() {
    let state = parse(b"\x1b[2;5r\x1b[5;1HP\x1bEQ");

    assert_eq!(row_text(&state, 0), "aaa");
    assert_eq!(row_text(&state, 1), "ccc");
    assert_eq!(row_text(&state, 2), "ddd");
    assert_eq!(row_text(&state, 3), "Pee");
    assert_eq!(row_text(&state, 4), "Q  ");
    assert_eq!(row_text(&state, 5), "fff");
    assert_eq!((state.cursor().row(), state.cursor().column()), (4, 1));
}

#[test]
fn incomplete_and_malformed_nel_escapes_keep_safe_existing_behavior() {
    let mut parser = TerminalParser::new();
    let mut incomplete = labeled_state();
    let before = labeled_state();
    parser.advance(&mut incomplete, b"\x1b").unwrap();
    assert_observable_state_eq(&incomplete, &before);

    parser.advance(&mut incomplete, b"E").unwrap();
    assert_eq!(
        (incomplete.cursor().row(), incomplete.cursor().column()),
        (5, 0)
    );
    assert_eq!(row_text(&incomplete, 5), "   ");

    let mut malformed_parser = TerminalParser::new();
    let mut malformed = labeled_state();
    malformed.set_cursor_position(2, 0).unwrap();
    malformed_parser
        .advance(&mut malformed, b"\x1b#EX")
        .unwrap();
    assert_eq!(row_text(&malformed, 2), "Xcc");
    assert_eq!(
        (malformed.cursor().row(), malformed.cursor().column()),
        (2, 1)
    );
}
