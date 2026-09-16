use terminal_core::{
    Cell, TerminalDimensions, TerminalParser, TerminalState, VerticalScrollingMargins,
};

fn rows(state: &TerminalState) -> Vec<String> {
    (0..state.dimensions().rows())
        .map(|row| {
            (0..state.dimensions().columns())
                .map(|column| state.screen().cell(row, column).unwrap().character())
                .collect()
        })
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
fn parser_dispatches_il_and_dl_with_omitted_zero_one_and_multiple_counts() {
    for sequence in [
        b"\x1b[3;1H\x1b[L".as_slice(),
        b"\x1b[3;1H\x1b[0L",
        b"\x1b[3;1H\x1b[1L",
    ] {
        assert_eq!(rows(&parse(sequence)), ["ab", "cd", "  ", "ef", "gh", "ij"]);
    }
    assert_eq!(
        rows(&parse(b"\x1b[3;1H\x1b[2L")),
        ["ab", "cd", "  ", "  ", "ef", "gh"]
    );

    for sequence in [
        b"\x1b[3;1H\x1b[M".as_slice(),
        b"\x1b[3;1H\x1b[0M",
        b"\x1b[3;1H\x1b[1M",
    ] {
        assert_eq!(rows(&parse(sequence)), ["ab", "cd", "gh", "ij", "kl", "  "]);
    }
    assert_eq!(
        rows(&parse(b"\x1b[3;1H\x1b[2M")),
        ["ab", "cd", "ij", "kl", "  ", "  "]
    );
}

#[test]
fn parser_il_dl_respect_cursor_relative_decstbm_subregions() {
    let cases = [
        (
            b"\x1b[2;5r\x1b[2;1H\x1b[L".as_slice(),
            ["ab", "  ", "cd", "ef", "gh", "kl"],
        ),
        (
            b"\x1b[2;5r\x1b[3;1H\x1b[L",
            ["ab", "cd", "  ", "ef", "gh", "kl"],
        ),
        (
            b"\x1b[2;5r\x1b[5;1H\x1b[L",
            ["ab", "cd", "ef", "gh", "  ", "kl"],
        ),
        (
            b"\x1b[2;5r\x1b[3;1H\x1b[M",
            ["ab", "cd", "gh", "ij", "  ", "kl"],
        ),
        (
            b"\x1b[2;5r\x1b[5;1H\x1b[M",
            ["ab", "cd", "ef", "gh", "  ", "kl"],
        ),
    ];
    for (input, expected) in cases {
        let state = parse(input);
        assert_eq!(rows(&state), expected);
        assert_eq!(
            state.vertical_scrolling_margins(),
            VerticalScrollingMargins::new(1, 4, 6).unwrap()
        );
    }

    for input in [
        b"\x1b[2;5r\x1b[1;1H\x1b[L".as_slice(),
        b"\x1b[2;5r\x1b[6;1H\x1b[M",
    ] {
        assert_eq!(rows(&parse(input)), ["ab", "cd", "ef", "gh", "ij", "kl"]);
    }
}

#[test]
fn parser_il_dl_clamp_counts_and_preserve_printable_neighbors() {
    let insert = parse(b"\x1b[2;5r\x1b[3;1HP\x1b[999999999999999999999LQ");
    assert_eq!(rows(&insert), ["ab", "cd", " Q", "  ", "  ", "kl"]);

    let delete = parse(b"\x1b[2;5r\x1b[3;1HP\x1b[999999999999999999999MQ");
    assert_eq!(rows(&delete), ["ab", "cd", " Q", "  ", "  ", "kl"]);
}

#[test]
fn parser_il_dl_are_chunk_invariant_and_incomplete_or_malformed_input_is_safe() {
    for input in [
        b"\x1b[2;5r\x1b[3Lafter".as_slice(),
        b"\x1b[2;5r\x1b[3Mafter",
    ] {
        let expected = parse(input);
        for split in 0..=input.len() {
            let mut parser = TerminalParser::new();
            let mut state = labeled_state();
            parser.advance(&mut state, &input[..split]).unwrap();
            parser.advance(&mut state, &input[split..]).unwrap();
            assert_eq!(state.screen(), expected.screen(), "split {split}");
            assert_eq!(state.cursor(), expected.cursor(), "split {split}");
        }
    }

    let mut parser = TerminalParser::new();
    let mut incomplete = labeled_state();
    parser.advance(&mut incomplete, b"\x1b[3").unwrap();
    assert_eq!(rows(&incomplete), ["ab", "cd", "ef", "gh", "ij", "kl"]);

    for input in [
        b"\x1b[1;2L".as_slice(),
        b"\x1b[1:2M",
        b"\x1b[?1L",
        b"\x1b[?1M",
    ] {
        let state = parse(input);
        assert_eq!(rows(&state), ["ab", "cd", "ef", "gh", "ij", "kl"]);
    }
}

#[test]
fn parser_il_dl_preserve_cursor_and_existing_cell_model() {
    let mut parser = TerminalParser::new();
    let mut state = labeled_state();
    parser
        .advance(
            &mut state,
            b"\x1b[2;5r\x1b[38;5;196m\x1b[4;1HX\x1b[3;2H\x1b[L",
        )
        .unwrap();
    assert_eq!(state.cursor().row(), 2);
    assert_eq!(state.cursor().column(), 1);
    assert_eq!(state.screen().cell(4, 0).unwrap().character(), 'X');
    for column in 0..2 {
        assert_eq!(state.screen().cell(2, column), Some(&Cell::default()));
    }
}
