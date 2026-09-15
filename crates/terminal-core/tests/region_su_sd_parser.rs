use terminal_core::{
    Cell, CellColor, TerminalDimensions, TerminalParser, TerminalState, TextIntensity,
    VerticalScrollingMargins,
};

fn row_text(state: &TerminalState, row: usize) -> String {
    (0..state.dimensions().columns())
        .map(|column| state.screen().cell(row, column).unwrap().character())
        .collect()
}

fn rows(state: &TerminalState) -> Vec<String> {
    (0..state.dimensions().rows())
        .map(|row| row_text(state, row))
        .collect()
}

fn labeled_state() -> TerminalState {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(2, 6).unwrap());
    parser
        .advance(&mut state, b"ab\r\ncd\r\nef\r\ngh\r\nij\r\nkl")
        .unwrap();
    state
}

fn parse(input: &[u8]) -> TerminalState {
    let mut parser = TerminalParser::new();
    let mut state = labeled_state();
    parser.advance(&mut state, input).unwrap();
    state
}

#[test]
fn decstbm_followed_by_su_or_sd_uses_active_region_and_default_count() {
    for sequence in [b"\x1b[2;5r\x1b[S".as_slice(), b"\x1b[2;5r\x1b[0S"] {
        let state = parse(sequence);
        assert_eq!(rows(&state), ["ab", "ef", "gh", "ij", "  ", "kl"]);
        assert_eq!(
            state.vertical_scrolling_margins(),
            VerticalScrollingMargins::new(1, 4, 6).unwrap()
        );
    }

    for sequence in [b"\x1b[2;5r\x1b[T".as_slice(), b"\x1b[2;5r\x1b[0T"] {
        let state = parse(sequence);
        assert_eq!(rows(&state), ["ab", "  ", "cd", "ef", "gh", "kl"]);
        assert_eq!(
            state.vertical_scrolling_margins(),
            VerticalScrollingMargins::new(1, 4, 6).unwrap()
        );
    }
}

#[test]
fn parser_region_su_sd_counts_are_bounded_to_region_height() {
    for (sequence, expected) in [
        (
            b"\x1b[2;5r\x1b[2S".as_slice(),
            ["ab", "gh", "ij", "  ", "  ", "kl"],
        ),
        (b"\x1b[2;5r\x1b[4T", ["ab", "  ", "  ", "  ", "  ", "kl"]),
        (b"\x1b[2;5r\x1b[5S", ["ab", "  ", "  ", "  ", "  ", "kl"]),
        (
            b"\x1b[2;5r\x1b[999999999999999999999T",
            ["ab", "  ", "  ", "  ", "  ", "kl"],
        ),
    ] {
        assert_eq!(rows(&parse(sequence)), expected);
    }
}

#[test]
fn parser_region_su_sd_are_safe_across_every_input_split() {
    for input in [b"\x1b[2;5r\x1b[2SXY".as_slice(), b"\x1b[2;5r\x1b[3TXY"] {
        let expected = parse(input);
        for split in 0..=input.len() {
            let mut parser = TerminalParser::new();
            let mut state = labeled_state();
            parser.advance(&mut state, &input[..split]).unwrap();
            parser.advance(&mut state, &input[split..]).unwrap();
            assert_eq!(state.screen(), expected.screen(), "split {split}");
            assert_eq!(state.cursor(), expected.cursor(), "split {split}");
            assert_eq!(
                state.vertical_scrolling_margins(),
                expected.vertical_scrolling_margins(),
                "split {split}"
            );
        }

        let mut parser = TerminalParser::new();
        let mut state = labeled_state();
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
fn printable_text_neighbors_region_scroll_without_parser_side_mutation() {
    let up = parse(b"\x1b[2;5r\x1b[3;1HP\x1b[SQ");
    assert_eq!(rows(&up), ["ab", "Pf", "gQ", "ij", "  ", "kl"]);

    let down = parse(b"\x1b[2;5r\x1b[3;1HP\x1b[TQ");
    assert_eq!(rows(&down), ["ab", "  ", "cQ", "Pf", "gh", "kl"]);
}

#[test]
fn malformed_extra_and_subparameter_shapes_remain_safe_no_ops_with_margins() {
    for sequence in [
        b"\x1b[2;5r\x1b[1;2S".as_slice(),
        b"\x1b[2;5r\x1b[1:2S",
        b"\x1b[2;5r\x1b[2;T",
        b"\x1b[2;5r\x1b[2:3T",
    ] {
        let state = parse(sequence);
        assert_eq!(rows(&state), ["ab", "cd", "ef", "gh", "ij", "kl"]);
        assert_eq!(
            state.vertical_scrolling_margins(),
            VerticalScrollingMargins::new(1, 4, 6).unwrap()
        );
    }
}

#[test]
fn parser_su_sd_preserve_active_sgr_and_moved_cell_attributes() {
    for (action, moved_row, exposed_row) in [(b'S', 2, 4), (b'T', 4, 1)] {
        let mut parser = TerminalParser::new();
        let mut state = labeled_state();
        let mut input = b"\x1b[2;5r\x1b[38;5;196;48;5;22;1;3;4;7m\x1b[4;1HX\x1b[3;2H".to_vec();
        input.extend_from_slice(&[0x1b, b'[', action]);
        parser.advance(&mut state, &input).unwrap();

        let moved = state.screen().cell(moved_row, 0).unwrap();
        assert_eq!(moved.character(), 'X');
        assert_eq!(moved.attributes().foreground(), CellColor::Indexed(196));
        assert_eq!(moved.attributes().background(), CellColor::Indexed(22));
        assert_eq!(moved.attributes().intensity(), TextIntensity::Bold);
        for column in 0..2 {
            assert_eq!(
                state.screen().cell(exposed_row, column),
                Some(&Cell::default())
            );
        }

        parser.advance(&mut state, b"Y").unwrap();
        assert_eq!(state.screen().cell(2, 1).unwrap().character(), 'Y');
        assert_eq!(
            state.screen().cell(2, 1).unwrap().attributes(),
            state.current_rendition()
        );
    }
}
