use terminal_core::{
    CellColor, CellOccupancy, EraseDirection, EraseRegion, InverseVideo, ItalicStyle,
    MAX_COMBINING_MARKS, PrintError, TerminalDimensions, TerminalParser, TerminalState,
    TextIntensity, UnderlineStyle,
};

const ACUTE: char = '\u{0301}';
const DOT_BELOW: char = '\u{0323}';

fn state(columns: usize, rows: usize) -> TerminalState {
    TerminalState::new(TerminalDimensions::new(columns, rows).unwrap())
}

fn marks(state: &TerminalState, row: usize, column: usize) -> &[char] {
    state.screen().cell(row, column).unwrap().combining_marks()
}

fn assert_valid_cells(state: &TerminalState) {
    for row in 0..state.dimensions().rows() {
        for column in 0..state.dimensions().columns() {
            let cell = state.screen().cell(row, column).unwrap();
            match cell.occupancy() {
                CellOccupancy::Single => {}
                CellOccupancy::WideLead => {
                    assert!(column + 1 < state.dimensions().columns());
                    assert_eq!(
                        state.screen().cell(row, column + 1).unwrap().occupancy(),
                        CellOccupancy::WideContinuation
                    );
                }
                CellOccupancy::WideContinuation => {
                    assert!(column > 0);
                    assert_eq!(
                        state.screen().cell(row, column - 1).unwrap().occupancy(),
                        CellOccupancy::WideLead
                    );
                    assert!(cell.combining_marks().is_empty());
                }
            }
            if *cell == terminal_core::Cell::default() {
                assert!(cell.combining_marks().is_empty());
            }
        }
    }
}

#[test]
fn narrow_base_owns_ordered_marks_without_cursor_or_rendition_changes() {
    let mut state = state(5, 1);
    state.set_text_intensity(TextIntensity::Bold);
    state.set_italic_style(ItalicStyle::Italic);
    state.set_underline_style(UnderlineStyle::Enabled);
    state.set_inverse_video(InverseVideo::Enabled);
    state.set_foreground_color(CellColor::Indexed(196));
    state.set_background_color(CellColor::Indexed(22));
    state.print_character('a').unwrap();
    state.reset_rendition();

    state.print_character(ACUTE).unwrap();
    state.print_character(DOT_BELOW).unwrap();

    let base = state.screen().cell(0, 0).unwrap();
    assert_eq!(base.character(), 'a');
    assert_eq!(base.occupancy(), CellOccupancy::Single);
    assert_eq!(base.combining_marks(), [ACUTE, DOT_BELOW]);
    assert_eq!(base.attributes().intensity(), TextIntensity::Bold);
    assert_eq!(base.attributes().italic(), ItalicStyle::Italic);
    assert_eq!(base.attributes().underline(), UnderlineStyle::Enabled);
    assert_eq!(base.attributes().inverse(), InverseVideo::Enabled);
    assert_eq!(base.attributes().foreground(), CellColor::Indexed(196));
    assert_eq!(base.attributes().background(), CellColor::Indexed(22));
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 1));
    assert_valid_cells(&state);
}

#[test]
fn wide_base_owns_marks_and_continuation_remains_metadata_only() {
    let mut state = state(5, 1);
    state.print_character('界').unwrap();
    state.print_character(ACUTE).unwrap();
    state.print_character(DOT_BELOW).unwrap();

    let lead = state.screen().cell(0, 0).unwrap();
    assert_eq!(lead.occupancy(), CellOccupancy::WideLead);
    assert_eq!(lead.combining_marks(), [ACUTE, DOT_BELOW]);
    assert_eq!(
        state.screen().cell(0, 1).unwrap().occupancy(),
        CellOccupancy::WideContinuation
    );
    assert!(marks(&state, 0, 1).is_empty());
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 1));
    assert_valid_cells(&state);
}

#[test]
fn combining_at_right_margin_attaches_before_delayed_wrap_resolves() {
    let mut narrow = state(3, 2);
    narrow.set_cursor_position(0, 2).unwrap();
    narrow.print_character('a').unwrap();
    narrow.print_character(ACUTE).unwrap();
    assert_eq!(marks(&narrow, 0, 2), [ACUTE]);
    assert_eq!((narrow.cursor().row(), narrow.cursor().column()), (0, 2));
    narrow.print_character('b').unwrap();
    assert_eq!(narrow.screen().cell(1, 0).unwrap().character(), 'b');

    let mut wide = state(4, 2);
    wide.set_cursor_position(0, 2).unwrap();
    wide.print_character('界').unwrap();
    wide.print_character(ACUTE).unwrap();
    assert_eq!(marks(&wide, 0, 2), [ACUTE]);
    assert_eq!((wide.cursor().row(), wide.cursor().column()), (0, 3));
    wide.print_character('b').unwrap();
    assert_eq!(wide.screen().cell(1, 0).unwrap().character(), 'b');
    assert_valid_cells(&narrow);
    assert_valid_cells(&wide);
}

#[test]
fn no_base_or_erased_base_ignores_marks_without_mutation() {
    let mut state = state(3, 1);
    state.print_character(ACUTE).unwrap();
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 0));
    assert_eq!(
        state.screen().cell(0, 0),
        Some(&terminal_core::Cell::default())
    );

    state.print_character('a').unwrap();
    state.set_cursor_position(0, 0).unwrap();
    state.print_character(ACUTE).unwrap();
    assert!(marks(&state, 0, 0).is_empty());

    state.set_cursor_position(0, 0).unwrap();
    state.erase_characters(1);
    state.set_cursor_position(0, 1).unwrap();
    state.print_character(ACUTE).unwrap();
    assert_eq!(
        state.screen().cell(0, 0),
        Some(&terminal_core::Cell::default())
    );
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 1));
    assert_valid_cells(&state);
}

#[test]
fn editing_and_resize_move_or_clear_complete_combining_payloads() {
    let mut state = state(6, 3);
    state.print_character('a').unwrap();
    state.print_character(ACUTE).unwrap();
    state.set_cursor_position(0, 0).unwrap();
    state.insert_characters(1);
    assert_eq!(marks(&state, 0, 1), [ACUTE]);
    state.set_cursor_position(0, 0).unwrap();
    state.delete_characters(1);
    assert_eq!(marks(&state, 0, 0), [ACUTE]);
    state.erase_characters(1);
    assert!(marks(&state, 0, 0).is_empty());

    state.set_cursor_position(1, 2).unwrap();
    state.print_character('界').unwrap();
    state.print_character(ACUTE).unwrap();
    state.scroll_up(1);
    assert_eq!(marks(&state, 0, 2), [ACUTE]);
    state.resize(TerminalDimensions::new(5, 3).unwrap());
    assert_eq!(marks(&state, 0, 2), [ACUTE]);
    state.resize(TerminalDimensions::new(3, 3).unwrap());
    assert_eq!(
        state.screen().cell(0, 2),
        Some(&terminal_core::Cell::default())
    );
    assert_valid_cells(&state);
}

#[test]
fn erase_overwrite_and_reset_clear_combining_payloads() {
    let mut state = state(5, 2);
    state.print_character('a').unwrap();
    state.print_character(ACUTE).unwrap();
    state.set_cursor_position(0, 0).unwrap();
    state.print_character('b').unwrap();
    assert!(marks(&state, 0, 0).is_empty());

    state.set_cursor_position(1, 1).unwrap();
    state.print_character('界').unwrap();
    state.print_character(ACUTE).unwrap();
    state.set_cursor_position(1, 2).unwrap();
    state.erase(EraseRegion::Line, EraseDirection::StartToCursor);
    assert_eq!(
        state.screen().cell(1, 1),
        Some(&terminal_core::Cell::default())
    );
    assert_eq!(
        state.screen().cell(1, 2),
        Some(&terminal_core::Cell::default())
    );

    state.reset();
    for row in 0..state.dimensions().rows() {
        for column in 0..state.dimensions().columns() {
            assert!(marks(&state, row, column).is_empty());
        }
    }
    assert_valid_cells(&state);
}

#[test]
fn parser_is_chunk_safe_for_base_and_combining_bytes() {
    let input = "a\u{0301}\u{0323}界\u{0301}".as_bytes();
    let mut expected_parser = TerminalParser::new();
    let mut expected = state(8, 1);
    expected_parser.advance(&mut expected, input).unwrap();

    for split in 0..=input.len() {
        let mut parser = TerminalParser::new();
        let mut actual = state(8, 1);
        parser.advance(&mut actual, &input[..split]).unwrap();
        parser.advance(&mut actual, &input[split..]).unwrap();
        assert_eq!(actual.screen(), expected.screen(), "split {split}");
        assert_eq!(actual.cursor(), expected.cursor(), "split {split}");
    }
}

#[test]
fn wide_combining_payloads_survive_shifts_and_clear_with_their_pair() {
    let mut state = state(6, 1);
    state.set_cursor_position(0, 1).unwrap();
    state.print_character('界').unwrap();
    state.print_character(ACUTE).unwrap();

    state.set_cursor_position(0, 0).unwrap();
    state.insert_characters(1);
    assert_eq!(marks(&state, 0, 2), [ACUTE]);
    assert_valid_cells(&state);

    state.set_cursor_position(0, 0).unwrap();
    state.delete_characters(1);
    assert_eq!(marks(&state, 0, 1), [ACUTE]);
    assert_valid_cells(&state);

    state.set_cursor_position(0, 1).unwrap();
    state.erase_characters(1);
    assert_eq!(
        state.screen().cell(0, 1),
        Some(&terminal_core::Cell::default())
    );
    assert_eq!(
        state.screen().cell(0, 2),
        Some(&terminal_core::Cell::default())
    );
    assert_valid_cells(&state);
}

#[test]
fn vertical_moves_preserve_narrow_and_wide_combining_payloads() {
    let mut state = state(5, 3);
    state.set_cursor_position(1, 0).unwrap();
    state.print_character('a').unwrap();
    state.print_character(ACUTE).unwrap();
    state.set_cursor_position(2, 1).unwrap();
    state.print_character('界').unwrap();
    state.print_character(DOT_BELOW).unwrap();

    state.scroll_up(1);
    assert_eq!(marks(&state, 0, 0), [ACUTE]);
    assert_eq!(marks(&state, 1, 1), [DOT_BELOW]);
    assert_valid_cells(&state);

    state.set_cursor_position(0, 0).unwrap();
    state.insert_lines(1);
    assert_eq!(marks(&state, 1, 0), [ACUTE]);
    state.delete_lines(1);
    assert_eq!(marks(&state, 0, 0), [ACUTE]);
    assert_valid_cells(&state);
}

#[test]
fn combining_capacity_is_bounded_without_mutating_the_base_or_cursor() {
    let mut state = state(3, 1);
    state.print_character('a').unwrap();
    for _ in 0..MAX_COMBINING_MARKS {
        state.print_character(ACUTE).unwrap();
    }
    let cursor = state.cursor();
    let marks_before = marks(&state, 0, 0).to_vec();

    assert_eq!(
        state.print_character(DOT_BELOW),
        Err(PrintError::CombiningMarkOverflow(DOT_BELOW))
    );
    assert_eq!(marks(&state, 0, 0), marks_before);
    assert_eq!(state.cursor(), cursor);
    assert_valid_cells(&state);
}
