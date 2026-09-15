use terminal_core::{
    Cell, CellColor, TerminalDimensions, TerminalParser, TerminalState, TextIntensity,
    VerticalScrollingMargins,
};

fn row_text(state: &TerminalState, row: usize) -> String {
    (0..state.dimensions().columns())
        .map(|column| state.screen().cell(row, column).unwrap().character())
        .collect()
}

fn parse(input: &[u8]) -> TerminalState {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(2, 6).unwrap());
    parser.advance(&mut state, input).unwrap();
    state
}

#[test]
fn parser_routes_decstbm_followed_by_index_and_reverse_index() {
    let index = parse(b"ab\r\ncd\r\nef\r\ngh\r\nij\r\nkl\x1b[2;5r\x1b[5;2H\x1bD");
    assert_eq!(
        (0..6).map(|row| row_text(&index, row)).collect::<Vec<_>>(),
        ["ab", "ef", "gh", "ij", "  ", "kl"]
    );
    assert_eq!(
        index.vertical_scrolling_margins(),
        VerticalScrollingMargins::new(1, 4, 6).unwrap()
    );

    let reverse = parse(b"ab\r\ncd\r\nef\r\ngh\r\nij\r\nkl\x1b[2;5r\x1b[2;2H\x1bM");
    assert_eq!(
        (0..6)
            .map(|row| row_text(&reverse, row))
            .collect::<Vec<_>>(),
        ["ab", "  ", "cd", "ef", "gh", "kl"]
    );
    assert_eq!(
        reverse.vertical_scrolling_margins(),
        VerticalScrollingMargins::new(1, 4, 6).unwrap()
    );
}

#[test]
fn parser_region_scrolls_preserve_printable_neighbors() {
    let state = parse(b"0A\r\n1B\r\n2C\r\n3D\r\n4E\r\n5F\x1b[2;5r\x1b[5;1Hq\x1bDr");
    assert_eq!(row_text(&state, 0), "0A");
    assert_eq!(row_text(&state, 1), "2C");
    assert_eq!(row_text(&state, 2), "3D");
    assert_eq!(row_text(&state, 3), "qE");
    assert_eq!(row_text(&state, 4), " r");
    assert_eq!(row_text(&state, 5), "5F");
}

#[test]
fn parser_index_and_reverse_index_with_margins_are_chunk_safe() {
    for input in [
        b"ab\r\ncd\r\nef\r\ngh\r\nij\r\nkl\x1b[2;5r\x1b[5;2H\x1bDXY".as_slice(),
        b"ab\r\ncd\r\nef\r\ngh\r\nij\r\nkl\x1b[2;5r\x1b[2;2H\x1bMXY",
    ] {
        let expected = parse(input);
        for split in 0..=input.len() {
            let mut parser = TerminalParser::new();
            let mut state = TerminalState::new(TerminalDimensions::new(2, 6).unwrap());
            parser.advance(&mut state, &input[..split]).unwrap();
            parser.advance(&mut state, &input[split..]).unwrap();
            assert_eq!(state.screen(), expected.screen());
            assert_eq!(state.cursor(), expected.cursor());
            assert_eq!(
                state.vertical_scrolling_margins(),
                expected.vertical_scrolling_margins()
            );
        }

        let mut parser = TerminalParser::new();
        let mut state = TerminalState::new(TerminalDimensions::new(2, 6).unwrap());
        for byte in input {
            parser
                .advance(&mut state, std::slice::from_ref(byte))
                .unwrap();
        }
        assert_eq!(state.screen(), expected.screen());
        assert_eq!(state.cursor(), expected.cursor());
    }
}

#[test]
fn active_rendition_survives_parser_driven_region_scroll() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(2, 6).unwrap());
    parser
        .advance(
            &mut state,
            b"\x1b[38;5;196;48;5;22;1m\x1b[2;5r\x1b[4;1HX\x1b[5;1H\x1bD",
        )
        .unwrap();

    let moved = state.screen().cell(2, 0).unwrap();
    assert_eq!(moved.character(), 'X');
    assert_eq!(moved.attributes().foreground(), CellColor::Indexed(196));
    assert_eq!(moved.attributes().background(), CellColor::Indexed(22));
    assert_eq!(moved.attributes().intensity(), TextIntensity::Bold);
    for column in 0..2 {
        assert_eq!(state.screen().cell(4, column), Some(&Cell::default()));
    }
    parser.advance(&mut state, b"Y").unwrap();
    assert_eq!(state.screen().cell(4, 0).unwrap().character(), 'Y');
    assert_eq!(
        state.current_rendition().foreground(),
        CellColor::Indexed(196)
    );
}
