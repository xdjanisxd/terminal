use terminal_core::{TerminalDimensions, TerminalParser, TerminalState};

fn row(state: &TerminalState, row: usize) -> String {
    (0..state.dimensions().columns())
        .map(|column| state.screen().cell(row, column).unwrap().character())
        .collect()
}

fn labeled_state() -> TerminalState {
    let mut state = TerminalState::new(TerminalDimensions::new(5, 3).unwrap());
    let mut parser = TerminalParser::new();
    parser
        .advance(&mut state, b"abcde\r\nfghij\r\nklmno")
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
fn parser_dispatches_ich_with_omitted_zero_one_and_multiple_counts() {
    for sequence in [
        b"\x1b[2;3H\x1b[@".as_slice(),
        b"\x1b[2;3H\x1b[0@",
        b"\x1b[2;3H\x1b[1@",
    ] {
        let state = parse(sequence);
        assert_eq!(row(&state, 1), "fg hi");
        assert_eq!((state.cursor().row(), state.cursor().column()), (1, 2));
    }

    let state = parse(b"\x1b[2;3H\x1b[2@");
    assert_eq!(row(&state, 1), "fg  h");
    assert_eq!(row(&state, 0), "abcde");
    assert_eq!(row(&state, 2), "klmno");
}

#[test]
fn parser_ich_clamps_at_the_right_edge_and_preserves_printable_neighbors() {
    let state = parse(b"\x1b[2;4HP\x1b[999999999999999999999@Q");
    assert_eq!(row(&state, 1), "fghPQ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 4));

    let state = parse(b"\x1b[2;5H\x1b[@");
    assert_eq!(row(&state, 1), "fghi ");
}

#[test]
fn parser_ich_is_chunk_invariant_and_incomplete_or_unsupported_shapes_are_safe() {
    let input = b"before\x1b[2;3H\x1b[2@after";
    let expected = parse(input);
    for split in 0..=input.len() {
        let mut parser = TerminalParser::new();
        let mut state = labeled_state();
        parser.advance(&mut state, &input[..split]).unwrap();
        parser.advance(&mut state, &input[split..]).unwrap();
        assert_eq!(state.screen(), expected.screen(), "split {split}");
        assert_eq!(state.cursor(), expected.cursor(), "split {split}");
    }

    let mut parser = TerminalParser::new();
    let mut incomplete = labeled_state();
    parser.advance(&mut incomplete, b"\x1b[2").unwrap();
    assert_eq!(row(&incomplete, 1), "fghij");

    for input in [
        b"\x1b[2;3H\x1b[1;2@".as_slice(),
        b"\x1b[2;3H\x1b[1:2@",
        b"\x1b[2;3H\x1b[?1@",
    ] {
        let state = parse(input);
        assert_eq!(row(&state, 1), "fghij");
    }
}
