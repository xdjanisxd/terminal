use terminal_core::{
    Cell, CellColor, CellOccupancy, ScreenKind, TerminalDimensions, TerminalParser, TerminalState,
    TextIntensity, VerticalScrollingMargins,
};

fn state(columns: usize, rows: usize) -> TerminalState {
    TerminalState::new(TerminalDimensions::new(columns, rows).unwrap())
}

fn write_row(state: &mut TerminalState, row: usize, text: &str) {
    state.set_cursor_position(row, 0).unwrap();
    for character in text.chars() {
        state.print_character(character).unwrap();
    }
}

fn row_text(row: &[Cell]) -> String {
    row.iter().map(|cell| cell.character()).collect()
}

#[test]
fn primary_history_retains_chronological_mixed_width_rows_across_resizes() {
    let mut state = state(6, 2);
    write_row(&mut state, 0, "first!");
    write_row(&mut state, 1, "second");
    state.set_cursor_position(1, 0).unwrap();
    state.index();

    state.resize(TerminalDimensions::new(4, 3).unwrap());
    write_row(&mut state, 0, "next");
    state.set_cursor_position(2, 0).unwrap();
    state.index();

    state.resize(TerminalDimensions::new(2, 1).unwrap());

    assert_eq!(state.scrollback_len(), 2);
    assert_eq!(state.scrollback_row(0).unwrap().len(), 6);
    assert_eq!(row_text(state.scrollback_row(0).unwrap()), "first!");
    assert_eq!(state.scrollback_row(1).unwrap().len(), 4);
    assert_eq!(row_text(state.scrollback_row(1).unwrap()), "next");
    assert_eq!(state.dimensions(), TerminalDimensions::new(2, 1).unwrap());
}

#[test]
fn resize_preserves_primary_history_through_47_and_1047_round_trips() {
    let mut parser = TerminalParser::new();
    let mut state = state(4, 3);
    write_row(&mut state, 0, "hist");
    state.set_cursor_position(2, 0).unwrap();
    state.index();
    state.set_cursor_position(1, 2).unwrap();
    state.set_horizontal_tab_stop();
    state.set_cursor_position(1, 3).unwrap();
    state.set_text_intensity(TextIntensity::Bold);

    parser.advance(&mut state, b"\x1b[?47h").unwrap();
    state.resize(TerminalDimensions::new(3, 2).unwrap());
    parser.advance(&mut state, b"\x1b[?47l").unwrap();
    assert_eq!(state.active_screen(), ScreenKind::Primary);
    assert_eq!(state.scrollback_len(), 1);
    assert_eq!(row_text(state.scrollback_row(0).unwrap()), "hist");
    assert_eq!(state.cursor().row(), 1);
    assert_eq!(state.cursor().column(), 2);
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::full_screen(2)
    );
    assert!(state.has_horizontal_tab_stop(2));
    assert_eq!(state.current_rendition().intensity(), TextIntensity::Bold);

    parser.advance(&mut state, b"\x1b[?1047h").unwrap();
    state.resize(TerminalDimensions::new(2, 3).unwrap());
    state.set_cursor_position(2, 0).unwrap();
    state.index();
    parser.advance(&mut state, b"\x1b[?1047l").unwrap();

    assert_eq!(state.active_screen(), ScreenKind::Primary);
    assert_eq!(state.scrollback_len(), 1);
    assert_eq!(row_text(state.scrollback_row(0).unwrap()), "hist");
    assert_eq!(state.dimensions(), TerminalDimensions::new(2, 3).unwrap());
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::full_screen(3)
    );
}

#[test]
fn resize_during_1049_clamps_saved_cursor_without_mutating_primary_history() {
    let mut parser = TerminalParser::new();
    let mut state = state(5, 3);
    write_row(&mut state, 0, "hist!");
    state.set_cursor_position(2, 0).unwrap();
    state.index();
    state.set_cursor_position(2, 4).unwrap();
    state.set_foreground_color(CellColor::Indexed(196));

    parser.advance(&mut state, b"\x1b[?1049h").unwrap();
    state.resize(TerminalDimensions::new(3, 2).unwrap());
    state.set_foreground_color(CellColor::Indexed(22));
    state.set_cursor_position(1, 0).unwrap();
    state.index();
    parser.advance(&mut state, b"\x1b[?1049l").unwrap();

    assert_eq!(state.active_screen(), ScreenKind::Primary);
    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 2));
    assert_eq!(
        state.current_rendition().foreground(),
        CellColor::Indexed(196)
    );
    assert_eq!(state.scrollback_len(), 1);
    assert_eq!(state.scrollback_row(0).unwrap().len(), 5);
    assert_eq!(row_text(state.scrollback_row(0).unwrap()), "hist!");
}

#[test]
fn resize_never_mutates_historical_wide_or_combining_cells_and_reset_clears_history() {
    let mut state = state(5, 2);
    state.print_character('界').unwrap();
    state.print_character('\u{301}').unwrap();
    write_row(&mut state, 1, "other");
    state.set_cursor_position(1, 0).unwrap();
    state.index();

    state.resize(TerminalDimensions::new(3, 3).unwrap());
    state.resize(TerminalDimensions::new(2, 1).unwrap());

    let row = state.scrollback_row(0).unwrap();
    assert_eq!(row.len(), 5);
    assert_eq!(row[0].occupancy(), CellOccupancy::WideLead);
    assert_eq!(row[0].character(), '界');
    assert_eq!(row[0].combining_marks(), ['\u{301}']);
    assert_eq!(row[1].occupancy(), CellOccupancy::WideContinuation);
    assert!(row[1].combining_marks().is_empty());

    state.reset();
    assert_eq!(state.scrollback_len(), 0);
}
