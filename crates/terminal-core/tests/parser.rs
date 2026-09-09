use terminal_core::{
    PrintError, TerminalDimensions, TerminalParser, TerminalParserError, TerminalState,
};

fn row_text(state: &TerminalState, row: usize) -> String {
    (0..state.dimensions().columns())
        .map(|column| state.screen().cell(row, column).unwrap().character())
        .collect()
}

fn assert_observable_state_eq(actual: &TerminalState, expected: &TerminalState) {
    assert_eq!(actual.dimensions(), expected.dimensions());
    assert_eq!(actual.cursor(), expected.cursor());
    assert_eq!(actual.terminal_modes(), expected.terminal_modes());
    assert_eq!(actual.input_modes(), expected.input_modes());

    for row in 0..actual.dimensions().rows() {
        for column in 0..actual.dimensions().columns() {
            assert_eq!(
                actual.screen().cell(row, column),
                expected.screen().cell(row, column),
                "cell mismatch at ({row}, {column})"
            );
        }
    }
}

fn parse_in_chunks(input: &[u8], chunk_size: usize) -> TerminalState {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(12, 4).unwrap());

    for chunk in input.chunks(chunk_size) {
        parser.advance(&mut state, chunk).unwrap();
    }

    state
}

#[test]
fn parses_simple_printable_ascii_stream() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(5, 2).unwrap());

    parser.advance(&mut state, b"abc").unwrap();

    assert_eq!(row_text(&state, 0), "abc  ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 3));
}

#[test]
fn routes_carriage_return_to_terminal_state() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(4, 2).unwrap());

    parser.advance(&mut state, b"abc\rX").unwrap();

    assert_eq!(row_text(&state, 0), "Xbc ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 1));
}

#[test]
fn routes_line_feed_to_terminal_state_without_implying_carriage_return() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(4, 2).unwrap());

    parser.advance(&mut state, b"ab\nX").unwrap();

    assert_eq!(row_text(&state, 0), "ab  ");
    assert_eq!(row_text(&state, 1), "  X ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 3));
}

#[test]
fn routes_backspace_to_terminal_state_without_erasing() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(4, 1).unwrap());

    parser.advance(&mut state, b"ab\x08X").unwrap();

    assert_eq!(row_text(&state, 0), "aX  ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 2));
}

#[test]
fn parses_mixed_printable_and_supported_control_input() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(4, 3).unwrap());

    parser.advance(&mut state, b"ab\rX\nY\x08Z").unwrap();

    assert_eq!(row_text(&state, 0), "Xb  ");
    assert_eq!(row_text(&state, 1), " Z  ");
    assert_eq!(row_text(&state, 2), "    ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 2));
}

#[test]
fn byte_at_a_time_matches_one_shot_input() {
    let input = b"ab\rX\nY\x08Z";
    let expected = parse_in_chunks(input, input.len());
    let actual = parse_in_chunks(input, 1);

    assert_observable_state_eq(&actual, &expected);
}

#[test]
fn every_split_point_matches_one_shot_input() {
    let input = b"ab\rX\nY\x08Z\x1b[31m!\x1b]0;title\x07?\x1bPqpayload\x1b\\#";
    let expected = parse_in_chunks(input, input.len());

    for split in 0..=input.len() {
        let mut parser = TerminalParser::new();
        let mut state = TerminalState::new(TerminalDimensions::new(12, 4).unwrap());
        parser.advance(&mut state, &input[..split]).unwrap();
        parser.advance(&mut state, &input[split..]).unwrap();

        assert_observable_state_eq(&state, &expected);
    }
}

#[test]
fn many_chunk_sizes_match_one_shot_input() {
    let input = b"ab\rX\nY\x08Z\x1b[31m!\x1b]0;title\x07?\x1bPqpayload\x1b\\#";
    let expected = parse_in_chunks(input, input.len());

    for chunk_size in 1..=input.len() {
        let actual = parse_in_chunks(input, chunk_size);
        assert_observable_state_eq(&actual, &expected);
    }
}

#[test]
fn supports_multiple_successive_chunks() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(6, 2).unwrap());

    parser.advance(&mut state, b"ab").unwrap();
    parser.advance(&mut state, b"cd").unwrap();
    parser.advance(&mut state, b"\rX").unwrap();

    assert_eq!(row_text(&state, 0), "Xbcd  ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 1));
}

#[test]
fn parser_state_persists_across_csi_chunk_boundaries() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(6, 1).unwrap());

    parser.advance(&mut state, b"\x1b[").unwrap();
    parser.advance(&mut state, b"31mA").unwrap();

    assert_eq!(row_text(&state, 0), "A     ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 1));
}

#[test]
fn unsupported_csi_is_ignored_without_corrupting_state() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(5, 1).unwrap());

    parser.advance(&mut state, b"ab\x1b[2Jc").unwrap();

    assert_eq!(row_text(&state, 0), "abc  ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 3));
}

#[test]
fn unsupported_osc_and_dcs_are_ignored_without_corrupting_state() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(5, 1).unwrap());

    parser
        .advance(
            &mut state,
            b"a\x1b]0;ignored title\x07b\x1bPqignored payload\x1b\\c",
        )
        .unwrap();

    assert_eq!(row_text(&state, 0), "abc  ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 3));
}

#[test]
fn unsupported_controls_are_ignored_without_approximation() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(4, 1).unwrap());

    parser.advance(&mut state, b"a\tb\x07c").unwrap();

    assert_eq!(row_text(&state, 0), "abc ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 3));
}

#[test]
fn malformed_or_incomplete_escape_input_does_not_panic_or_mutate_state() {
    let inputs: &[&[u8]] = &[
        b"\x1b",
        b"\x1b[31",
        b"\x1b]0;unterminated",
        b"\x1bPq unterminated",
    ];

    for input in inputs {
        let mut parser = TerminalParser::new();
        let mut state = TerminalState::new(TerminalDimensions::new(4, 1).unwrap());
        parser.advance(&mut state, input).unwrap();

        assert_eq!(row_text(&state, 0), "    ");
        assert_eq!((state.cursor().row(), state.cursor().column()), (0, 0));
    }
}

#[test]
fn oversized_unsupported_osc_is_bounded_and_does_not_corrupt_state() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(4, 1).unwrap());
    let mut input = b"\x1b]0;".to_vec();
    input.extend(std::iter::repeat_n(b'x', 4096));
    input.extend_from_slice(b"\x07A");

    parser.advance(&mut state, &input).unwrap();

    assert_eq!(row_text(&state, 0), "A   ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 1));
}

#[test]
fn unsupported_unicode_returns_a_bounded_error_with_consumed_bytes() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(5, 1).unwrap());
    let input = "A界B".as_bytes();

    let error = parser.advance(&mut state, input).unwrap_err();

    assert_eq!(error.bytes_consumed(), 4);
    assert_eq!(
        error.semantic_error(),
        PrintError::UnsupportedCharacter('界')
    );
    assert_eq!(row_text(&state, 0), "A    ");

    parser
        .advance(&mut state, &input[error.bytes_consumed()..])
        .unwrap();
    assert_eq!(row_text(&state, 0), "AB   ");
}

#[test]
fn unsupported_unicode_error_is_reported_when_split_codepoint_completes() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(4, 1).unwrap());
    let bytes = "界".as_bytes();

    parser.advance(&mut state, &bytes[..2]).unwrap();
    let error = parser.advance(&mut state, &bytes[2..]).unwrap_err();

    assert_eq!(error.bytes_consumed(), 1);
    assert_eq!(
        error.semantic_error(),
        PrintError::UnsupportedCharacter('界')
    );
    assert_eq!(row_text(&state, 0), "    ");
}

#[test]
fn malformed_partial_utf8_surfaces_error_before_reprocessing_current_byte() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(4, 1).unwrap());

    parser.advance(&mut state, &[0xE2]).unwrap();
    let error = parser.advance(&mut state, b"(").unwrap_err();

    assert_eq!(error.bytes_consumed(), 0);
    assert_eq!(
        error.semantic_error(),
        PrintError::UnsupportedCharacter('\u{FFFD}')
    );
    assert_eq!(row_text(&state, 0), "    ");

    parser.advance(&mut state, b"(").unwrap();
    assert_eq!(row_text(&state, 0), "(   ");
}

#[test]
fn malformed_partial_utf8_does_not_execute_reprocessed_control_after_error() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(4, 1).unwrap());
    parser.advance(&mut state, b"A").unwrap();
    parser.advance(&mut state, &[0xE2]).unwrap();

    let error = parser.advance(&mut state, b"\r").unwrap_err();

    assert_eq!(error.bytes_consumed(), 0);
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 1));

    parser.advance(&mut state, b"\r").unwrap();
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 0));
}

#[test]
fn parser_error_is_project_owned_and_exposes_standard_error_source() {
    fn assert_error(error: &dyn std::error::Error) {
        assert!(error.source().is_some());
    }

    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(2, 1).unwrap());
    let error: TerminalParserError = parser.advance(&mut state, "é".as_bytes()).unwrap_err();

    assert_error(&error);
    assert_eq!(error.bytes_consumed(), 2);
    assert_eq!(
        error.semantic_error(),
        PrintError::UnsupportedCharacter('é')
    );
}
