use terminal_core::{
    AutoWrapMode, CellAttributes, CellColor, CharacterInsertionMode, CursorKeyMode,
    CursorVisibility, InverseVideo, ItalicStyle, PrintError, TerminalDimensions, TerminalParser,
    TerminalParserError, TerminalState, TextIntensity, UnderlineStyle,
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
    assert_eq!(actual.current_rendition(), expected.current_rendition());
    for column in 0..actual.dimensions().columns() {
        assert_eq!(
            actual.has_horizontal_tab_stop(column),
            expected.has_horizontal_tab_stop(column),
            "tab-stop mismatch at column {column}"
        );
    }

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
fn routes_supported_cursor_csi_with_defaults_zeroes_and_large_parameters() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(8, 6).unwrap());

    parser.advance(&mut state, b"\x1b[4;5H").unwrap();
    assert_eq!((state.cursor().row(), state.cursor().column()), (3, 4));
    parser.advance(&mut state, b"\x1b[A\x1b[0C").unwrap();
    assert_eq!((state.cursor().row(), state.cursor().column()), (2, 5));
    parser.advance(&mut state, b"\x1b[2B\x1b[3D").unwrap();
    assert_eq!((state.cursor().row(), state.cursor().column()), (4, 2));
    parser.advance(&mut state, b"\x1b[0;0f").unwrap();
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 0));
    parser.advance(&mut state, b"\x1b[6d\x1b[8G").unwrap();
    assert_eq!((state.cursor().row(), state.cursor().column()), (5, 7));
    parser
        .advance(&mut state, b"\x1b[65535A\x1b[65535D")
        .unwrap();
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 0));
}

#[test]
fn routes_supported_erase_csi_and_preserves_cursor() {
    let cases = [
        (b"\x1b[K".as_slice(), ["abcde", "fg   ", "klmno"]),
        (b"\x1b[0K".as_slice(), ["abcde", "fg   ", "klmno"]),
        (b"\x1b[1K".as_slice(), ["abcde", "   ij", "klmno"]),
        (b"\x1b[2K".as_slice(), ["abcde", "     ", "klmno"]),
        (b"\x1b[J".as_slice(), ["abcde", "fg   ", "     "]),
        (b"\x1b[0J".as_slice(), ["abcde", "fg   ", "     "]),
        (b"\x1b[1J".as_slice(), ["     ", "   ij", "klmno"]),
        (b"\x1b[2J".as_slice(), ["     ", "     ", "     "]),
    ];

    for (sequence, expected) in cases {
        let mut parser = TerminalParser::new();
        let mut state = TerminalState::new(TerminalDimensions::new(5, 3).unwrap());
        parser
            .advance(&mut state, b"abcde\x1b[2;1Hfghij\x1b[3;1Hklmno\x1b[2;3H")
            .unwrap();

        parser.advance(&mut state, sequence).unwrap();

        for (row, expected_row) in expected.into_iter().enumerate() {
            assert_eq!(row_text(&state, row), expected_row, "sequence {sequence:?}");
        }
        assert_eq!((state.cursor().row(), state.cursor().column()), (1, 2));
        assert_eq!(
            state.screen().cell(1, 2).unwrap().attributes(),
            &terminal_core::CellAttributes::default()
        );
    }
}

#[test]
fn supported_csi_is_chunk_safe_at_every_byte_boundary() {
    let input = b"ab\x1b[2D!\x1b[3;4H#\x1b[1A\x1b[0C\x1b[K\x1b[2JZ";
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
fn multiple_successive_csi_sequences_mix_with_printable_output() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(6, 3).unwrap());

    parser
        .advance(
            &mut state,
            b"abc\x1b[2D!\x1b[2B\x1b[6G?\x1b[1A\x1b[2K\x1b[1GX",
        )
        .unwrap();

    assert_eq!(row_text(&state, 0), "a!c   ");
    assert_eq!(row_text(&state, 1), "X     ");
    assert_eq!(row_text(&state, 2), "     ?");
}

#[test]
fn unsupported_csi_is_ignored_without_corrupting_state() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(5, 3).unwrap());

    parser
        .advance(
            &mut state,
            b"\x1b[3;3H\x1b[3J\x1b[1;2A\x1b[1:2D\x1b[1;2;3H\x1b[?2Jc",
        )
        .unwrap();

    assert_eq!(row_text(&state, 0), "     ");
    assert_eq!(row_text(&state, 1), "     ");
    assert_eq!(row_text(&state, 2), "  c  ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (2, 3));
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

    parser.advance(&mut state, b"a\x0bb\x07c").unwrap();

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

#[test]
fn standard_mode_sequences_control_insert_and_replace_modes() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(6, 1).unwrap());

    parser.advance(&mut state, b"\x1b[4h").unwrap();
    assert_eq!(
        state.terminal_modes().character_insertion(),
        CharacterInsertionMode::Insert
    );

    parser.advance(&mut state, b"\x1b[4l").unwrap();
    assert_eq!(
        state.terminal_modes().character_insertion(),
        CharacterInsertionMode::Replace
    );
}

#[test]
fn private_mode_sequences_control_auto_wrap_and_cursor_visibility() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(6, 1).unwrap());

    parser.advance(&mut state, b"\x1b[?7l\x1b[?25l").unwrap();
    assert_eq!(state.terminal_modes().auto_wrap(), AutoWrapMode::Disabled);
    assert_eq!(
        state.terminal_modes().cursor_visibility(),
        CursorVisibility::Hidden
    );

    parser.advance(&mut state, b"\x1b[?7h\x1b[?25h").unwrap();
    assert_eq!(state.terminal_modes().auto_wrap(), AutoWrapMode::Enabled);
    assert_eq!(
        state.terminal_modes().cursor_visibility(),
        CursorVisibility::Visible
    );
}

#[test]
fn mode_sequences_handle_multiple_and_mixed_supported_parameters() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(6, 1).unwrap());

    parser.advance(&mut state, b"\x1b[?7;999;25l").unwrap();
    assert_eq!(state.terminal_modes().auto_wrap(), AutoWrapMode::Disabled);
    assert_eq!(
        state.terminal_modes().cursor_visibility(),
        CursorVisibility::Hidden
    );

    parser.advance(&mut state, b"\x1b[3;4;999h").unwrap();
    assert_eq!(
        state.terminal_modes().character_insertion(),
        CharacterInsertionMode::Insert
    );

    parser.advance(&mut state, b"\x1b[?25;7h").unwrap();
    assert_eq!(state.terminal_modes().auto_wrap(), AutoWrapMode::Enabled);
    assert_eq!(
        state.terminal_modes().cursor_visibility(),
        CursorVisibility::Visible
    );
}

#[test]
fn omitted_unsupported_and_wrong_private_modes_are_safe_no_ops() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(6, 1).unwrap());
    state.set_cursor_key_mode(CursorKeyMode::Application);
    parser.advance(&mut state, b"abc").unwrap();
    let dimensions = state.dimensions();
    let cursor = state.cursor();

    parser
        .advance(
            &mut state,
            b"\x1b[h\x1b[l\x1b[999h\x1b[999l\x1b[7l\x1b[25l\x1b[?4h",
        )
        .unwrap();

    assert_eq!(state.dimensions(), dimensions);
    assert_eq!(state.cursor(), cursor);
    assert_eq!(row_text(&state, 0), "abc   ");
    assert_eq!(state.terminal_modes(), &Default::default());
    assert_eq!(
        state.input_modes().cursor_keys(),
        CursorKeyMode::Application
    );
}

#[test]
fn parser_controlled_insert_mode_changes_printable_output_semantics() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(6, 1).unwrap());

    parser
        .advance(&mut state, b"abcde\x1b[1;2H\x1b[4hX\x1b[4lY")
        .unwrap();

    assert_eq!(row_text(&state, 0), "aXYcde");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 3));
    assert_eq!(
        state.terminal_modes().character_insertion(),
        CharacterInsertionMode::Replace
    );
}

#[test]
fn parser_controlled_auto_wrap_clears_and_does_not_create_pending_wrap() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(3, 2).unwrap());

    parser
        .advance(&mut state, b"abc\x1b[?7lX\x1b[?7hY")
        .unwrap();

    assert_eq!(row_text(&state, 0), "abY");
    assert_eq!(row_text(&state, 1), "   ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 2));

    parser.advance(&mut state, b"Z").unwrap();
    assert_eq!(row_text(&state, 1), "Z  ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 1));
}

#[test]
fn parser_mode_dispatch_is_chunk_safe_at_every_byte_boundary() {
    let input = b"ab\x1b[4hX\x1b[4lY\x1b[?7;25lZ\x1b[?999;25;7h!";
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
fn successive_mode_sequences_preserve_unrelated_state() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(6, 2).unwrap());
    state.set_cursor_key_mode(CursorKeyMode::Application);
    parser.advance(&mut state, b"abc\x1b[2;3H").unwrap();
    let dimensions = state.dimensions();
    let cursor = state.cursor();
    let cells = [row_text(&state, 0), row_text(&state, 1)];

    parser
        .advance(
            &mut state,
            b"\x1b[4h\x1b[?7l\x1b[?25l\x1b[4l\x1b[?7h\x1b[?25h",
        )
        .unwrap();

    assert_eq!(state.dimensions(), dimensions);
    assert_eq!(state.cursor(), cursor);
    assert_eq!([row_text(&state, 0), row_text(&state, 1)], cells);
    assert_eq!(
        state.input_modes().cursor_keys(),
        CursorKeyMode::Application
    );
}

#[test]
fn malformed_private_and_subparameter_mode_forms_do_not_panic_or_change_modes() {
    let inputs: &[&[u8]] = &[
        b"\x1b[?",
        b"\x1b[?7",
        b"\x1b[?7:h",
        b"\x1b[4:1h",
        b"\x1b[??7h",
        b"\x1b[>7h",
    ];

    for input in inputs {
        let mut parser = TerminalParser::new();
        let mut state = TerminalState::new(TerminalDimensions::new(4, 1).unwrap());
        parser.advance(&mut state, input).unwrap();
        assert_eq!(
            state.terminal_modes(),
            &Default::default(),
            "input {input:?}"
        );
    }
}

#[test]
fn parses_horizontal_tab_with_mixed_printable_input_in_one_chunk() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(12, 1).unwrap());

    parser.advance(&mut state, b"abc\tX").unwrap();

    assert_eq!(row_text(&state, 0), "abc     X   ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 9));
}

#[test]
fn parser_sets_and_clears_horizontal_tab_stops() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(20, 1).unwrap());
    state.set_cursor_visibility(CursorVisibility::Hidden);
    state.set_auto_wrap(AutoWrapMode::Disabled);
    state.set_character_insertion(CharacterInsertionMode::Insert);
    state.set_cursor_key_mode(CursorKeyMode::Application);
    let terminal_modes = *state.terminal_modes();
    let input_modes = *state.input_modes();

    parser
        .advance(
            &mut state,
            b"\x1b[6G\x1bH\x1b[1G\tX\x1b[6G\x1b[g\x1b[1G\tY\x1b[3g\x1b[1G\tZ",
        )
        .unwrap();

    assert!((0..20).all(|column| !state.has_horizontal_tab_stop(column)));
    assert_eq!(row_text(&state, 0), "     X  Y          Z");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 19));
    assert_eq!(*state.terminal_modes(), terminal_modes);
    assert_eq!(*state.input_modes(), input_modes);
}

#[test]
fn parser_horizontal_tab_resolves_delayed_wrap() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(9, 2).unwrap());

    parser.advance(&mut state, b"abcdefghi\tX").unwrap();

    assert_eq!(row_text(&state, 0), "abcdefghX");
    assert_eq!(row_text(&state, 1), "         ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 8));
}

#[test]
fn parser_tab_sequences_are_chunk_safe_at_every_byte_boundary() {
    let input = b"ab\tC\x1b[6G\x1bH\x1b[1G\tX\x1b[6G\x1b[g\x1b[1G\tY\x1b[3g\x1b[1G\tZ";
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
fn unsupported_malformed_and_incomplete_tab_forms_are_safe_no_ops() {
    let inputs: &[&[u8]] = &[
        b"\x1b#H",
        b"\x1b[1g",
        b"\x1b[0;3g",
        b"\x1b[?3g",
        b"\x1b[3:0g",
        b"\x1b[",
        b"\x1b",
    ];

    for input in inputs {
        let mut parser = TerminalParser::new();
        let mut state = TerminalState::new(TerminalDimensions::new(20, 1).unwrap());
        state.clear_all_horizontal_tab_stops();
        state.set_cursor_position(0, 5).unwrap();
        state.set_horizontal_tab_stop();

        parser.advance(&mut state, input).unwrap();

        assert_eq!(
            (0..20)
                .filter(|&column| state.has_horizontal_tab_stop(column))
                .collect::<Vec<_>>(),
            vec![5],
            "input {input:?}"
        );
    }
}

#[test]
fn sgr_style_set_and_clear_pairs_are_independent() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(4, 1).unwrap());

    parser.advance(&mut state, b"\x1b[1;3;4;7m").unwrap();
    assert_eq!(state.current_rendition().intensity(), TextIntensity::Bold);
    assert_eq!(state.current_rendition().italic(), ItalicStyle::Italic);
    assert_eq!(
        state.current_rendition().underline(),
        UnderlineStyle::Enabled
    );
    assert_eq!(state.current_rendition().inverse(), InverseVideo::Enabled);

    parser.advance(&mut state, b"\x1b[22m").unwrap();
    assert_eq!(state.current_rendition().intensity(), TextIntensity::Normal);
    assert_eq!(state.current_rendition().italic(), ItalicStyle::Italic);
    parser.advance(&mut state, b"\x1b[23m").unwrap();
    assert_eq!(state.current_rendition().italic(), ItalicStyle::Upright);
    assert_eq!(
        state.current_rendition().underline(),
        UnderlineStyle::Enabled
    );
    parser.advance(&mut state, b"\x1b[24m").unwrap();
    assert_eq!(
        state.current_rendition().underline(),
        UnderlineStyle::Disabled
    );
    assert_eq!(state.current_rendition().inverse(), InverseVideo::Enabled);
    parser.advance(&mut state, b"\x1b[27m").unwrap();
    assert_eq!(state.current_rendition(), &CellAttributes::default());
}

#[test]
fn sgr_maps_all_standard_and_bright_foreground_colors() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(4, 1).unwrap());

    for (code, index) in (30..=37).zip(0..=7) {
        parser
            .advance(&mut state, format!("\x1b[{code}m").as_bytes())
            .unwrap();
        assert_eq!(
            state.current_rendition().foreground(),
            CellColor::Indexed(index)
        );
    }
    for (code, index) in (90..=97).zip(8..=15) {
        parser
            .advance(&mut state, format!("\x1b[{code}m").as_bytes())
            .unwrap();
        assert_eq!(
            state.current_rendition().foreground(),
            CellColor::Indexed(index)
        );
    }
}

#[test]
fn sgr_maps_all_standard_and_bright_background_colors() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(4, 1).unwrap());

    for (code, index) in (40..=47).zip(0..=7) {
        parser
            .advance(&mut state, format!("\x1b[{code}m").as_bytes())
            .unwrap();
        assert_eq!(
            state.current_rendition().background(),
            CellColor::Indexed(index)
        );
    }
    for (code, index) in (100..=107).zip(8..=15) {
        parser
            .advance(&mut state, format!("\x1b[{code}m").as_bytes())
            .unwrap();
        assert_eq!(
            state.current_rendition().background(),
            CellColor::Indexed(index)
        );
    }
}

#[test]
fn sgr_default_color_codes_restore_only_the_selected_channel() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(4, 1).unwrap());

    parser.advance(&mut state, b"\x1b[1;31;104m").unwrap();
    parser.advance(&mut state, b"\x1b[39m").unwrap();
    assert_eq!(state.current_rendition().foreground(), CellColor::Default);
    assert_eq!(
        state.current_rendition().background(),
        CellColor::Indexed(12)
    );
    assert_eq!(state.current_rendition().intensity(), TextIntensity::Bold);

    parser.advance(&mut state, b"\x1b[49m").unwrap();
    assert_eq!(state.current_rendition().background(), CellColor::Default);
    assert_eq!(state.current_rendition().intensity(), TextIntensity::Bold);
}

#[test]
fn sgr_zero_bare_and_empty_parameters_reset_the_complete_rendition() {
    for reset in [b"\x1b[0m".as_slice(), b"\x1b[m", b"\x1b[;m"] {
        let mut parser = TerminalParser::new();
        let mut state = TerminalState::new(TerminalDimensions::new(4, 1).unwrap());
        parser.advance(&mut state, b"\x1b[1;3;4;7;35;106m").unwrap();

        parser.advance(&mut state, reset).unwrap();

        assert_eq!(state.current_rendition(), &CellAttributes::default());
    }
}

#[test]
fn sgr_parameters_apply_in_order_including_empty_parameters() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(4, 1).unwrap());

    parser
        .advance(&mut state, b"\x1b[31;39;92;44;49;103;1;22;1m")
        .unwrap();
    assert_eq!(
        state.current_rendition().foreground(),
        CellColor::Indexed(10)
    );
    assert_eq!(
        state.current_rendition().background(),
        CellColor::Indexed(11)
    );
    assert_eq!(state.current_rendition().intensity(), TextIntensity::Bold);

    parser.advance(&mut state, b"\x1b[1;;3m").unwrap();
    assert_eq!(state.current_rendition().intensity(), TextIntensity::Normal);
    assert_eq!(state.current_rendition().italic(), ItalicStyle::Italic);
    assert_eq!(state.current_rendition().foreground(), CellColor::Default);
    assert_eq!(state.current_rendition().background(), CellColor::Default);
}

#[test]
fn unsupported_sgr_parameters_and_subparameters_are_individual_no_ops() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(4, 1).unwrap());
    parser.advance(&mut state, b"\x1b[32;44m").unwrap();
    let initial = *state.current_rendition();

    parser
        .advance(&mut state, b"\x1b[2;5;8;9;38;48;58;999m")
        .unwrap();
    assert_eq!(state.current_rendition(), &initial);

    parser
        .advance(&mut state, b"\x1b[1;38:2:255:0:0;3m")
        .unwrap();
    assert_eq!(state.current_rendition().intensity(), TextIntensity::Bold);
    assert_eq!(state.current_rendition().italic(), ItalicStyle::Italic);
    assert_eq!(
        state.current_rendition().foreground(),
        CellColor::Indexed(2)
    );
    assert_eq!(
        state.current_rendition().background(),
        CellColor::Indexed(4)
    );
}

#[test]
fn parser_printing_snapshots_rendition_without_changing_existing_cells() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(5, 1).unwrap());

    parser
        .advance(&mut state, b"a\x1b[1;3;4;7;91;104mb\x1b[0mc")
        .unwrap();

    let default = CellAttributes::default();
    assert_eq!(state.screen().cell(0, 0).unwrap().attributes(), &default);
    let styled = state.screen().cell(0, 1).unwrap().attributes();
    assert_eq!(styled.intensity(), TextIntensity::Bold);
    assert_eq!(styled.italic(), ItalicStyle::Italic);
    assert_eq!(styled.underline(), UnderlineStyle::Enabled);
    assert_eq!(styled.inverse(), InverseVideo::Enabled);
    assert_eq!(styled.foreground(), CellColor::Indexed(9));
    assert_eq!(styled.background(), CellColor::Indexed(12));
    assert_eq!(state.screen().cell(0, 2).unwrap().attributes(), &default);
}

#[test]
fn representative_sgr_stream_is_chunk_safe_at_every_byte_boundary() {
    let input = b"a\x1b[1;31;44mb\x1b[22;39;104mc\x1b[0md\t\x1b[?25l!";
    let expected = parse_in_chunks(input, input.len());

    for split in 0..=input.len() {
        let mut parser = TerminalParser::new();
        let mut state = TerminalState::new(TerminalDimensions::new(12, 4).unwrap());
        parser.advance(&mut state, &input[..split]).unwrap();
        parser.advance(&mut state, &input[split..]).unwrap();
        assert_observable_state_eq(&state, &expected);
    }
}
