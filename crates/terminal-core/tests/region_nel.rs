use terminal_core::{
    AutoWrapMode, Cell, CellColor, CharacterInsertionMode, CursorKeyMode, CursorVisibility,
    InverseVideo, ItalicStyle, TerminalDimensions, TerminalState, TextIntensity, UnderlineStyle,
    VerticalScrollingMargins,
};

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

fn rows(state: &TerminalState) -> Vec<String> {
    (0..state.dimensions().rows())
        .map(|row| {
            (0..state.dimensions().columns())
                .map(|column| state.screen().cell(row, column).unwrap().character())
                .collect()
        })
        .collect()
}

fn cells(state: &TerminalState) -> Vec<Cell> {
    (0..state.dimensions().rows())
        .flat_map(|row| {
            (0..state.dimensions().columns())
                .map(move |column| *state.screen().cell(row, column).unwrap())
        })
        .collect()
}

#[test]
fn nel_inside_custom_region_moves_down_without_scrolling_and_cancels_delayed_wrap() {
    let mut state = labeled_state();
    assert!(state.set_vertical_scrolling_margins(1, 4));
    state.set_cursor_position(2, 2).unwrap();
    state.print_character('X').unwrap();
    let before = cells(&state);

    state.next_line();

    assert_eq!(cells(&state), before);
    assert_eq!((state.cursor().row(), state.cursor().column()), (3, 0));
    state.print_character('Y').unwrap();
    assert_eq!(state.screen().cell(3, 0).unwrap().character(), 'Y');
}

#[test]
fn nel_at_bottom_margin_scrolls_only_region_and_preserves_cells_and_state() {
    let mut state = labeled_state();
    assert!(state.set_vertical_scrolling_margins(1, 4));
    state.set_foreground_color(CellColor::Indexed(1));
    state.set_background_color(CellColor::Indexed(22));
    state.set_text_intensity(TextIntensity::Bold);
    state.set_italic_style(ItalicStyle::Italic);
    state.set_underline_style(UnderlineStyle::Enabled);
    state.set_inverse_video(InverseVideo::Enabled);
    state.set_character_insertion(CharacterInsertionMode::Insert);
    state.set_auto_wrap(AutoWrapMode::Enabled);
    state.set_cursor_visibility(CursorVisibility::Hidden);
    state.set_cursor_key_mode(CursorKeyMode::Application);
    state.set_cursor_position(3, 0).unwrap();
    state.print_character('X').unwrap();
    let ansi_foreground_indexed_background = *state.screen().cell(3, 0).unwrap();
    state.set_foreground_color(CellColor::Indexed(196));
    state.set_background_color(CellColor::Indexed(4));
    state.print_character('W').unwrap();
    let indexed_foreground_ansi_background = *state.screen().cell(3, 1).unwrap();
    state.set_horizontal_tab_stop();
    state.set_cursor_position(4, 2).unwrap();
    state.print_character('Z').unwrap();
    let outside_above = (0..3)
        .map(|column| *state.screen().cell(0, column).unwrap())
        .collect::<Vec<_>>();
    let outside_below = (0..3)
        .map(|column| *state.screen().cell(5, column).unwrap())
        .collect::<Vec<_>>();
    let rendition = *state.current_rendition();
    let terminal_modes = *state.terminal_modes();
    let input_modes = *state.input_modes();
    let margins = state.vertical_scrolling_margins();

    state.next_line();

    assert_eq!(
        state.screen().cell(2, 0),
        Some(&ansi_foreground_indexed_background)
    );
    assert_eq!(
        state.screen().cell(2, 1),
        Some(&indexed_foreground_ansi_background)
    );
    assert_eq!(
        (0..3)
            .map(|column| *state.screen().cell(0, column).unwrap())
            .collect::<Vec<_>>(),
        outside_above
    );
    assert_eq!(
        (0..3)
            .map(|column| *state.screen().cell(5, column).unwrap())
            .collect::<Vec<_>>(),
        outside_below
    );
    for column in 0..3 {
        assert_eq!(state.screen().cell(4, column), Some(&Cell::default()));
    }
    assert_eq!((state.cursor().row(), state.cursor().column()), (4, 0));
    assert_eq!(*state.current_rendition(), rendition);
    assert_eq!(*state.terminal_modes(), terminal_modes);
    assert_eq!(*state.input_modes(), input_modes);
    assert_eq!(state.vertical_scrolling_margins(), margins);
    assert!(state.has_horizontal_tab_stop(2));

    state.print_character('Y').unwrap();
    assert_eq!(state.screen().cell(4, 0).unwrap().attributes(), &rendition);
}

#[test]
fn nel_outside_region_moves_bounded_without_scrolling_or_margin_clamping() {
    for (start_row, expected_row) in [(0, 1), (4, 5), (5, 5)] {
        let mut state = labeled_state();
        assert!(state.set_vertical_scrolling_margins(1, 3));
        state.set_cursor_position(start_row, 2).unwrap();
        state.print_character('Q').unwrap();
        let before = cells(&state);

        state.next_line();

        assert_eq!(cells(&state), before, "start row {start_row}");
        assert_eq!(
            (state.cursor().row(), state.cursor().column()),
            (expected_row, 0),
            "start row {start_row}"
        );
        state.print_character('X').unwrap();
        assert_eq!(
            state.screen().cell(expected_row, 0).unwrap().character(),
            'X',
            "start row {start_row}"
        );
    }
}

#[test]
fn nel_on_single_row_region_clears_only_that_row() {
    let mut state = labeled_state();
    assert!(state.set_vertical_scrolling_margins(2, 2));
    let outside = [
        rows(&state)[0].clone(),
        rows(&state)[1].clone(),
        rows(&state)[3].clone(),
        rows(&state)[4].clone(),
        rows(&state)[5].clone(),
    ];
    state.set_cursor_position(2, 2).unwrap();
    state.print_character('X').unwrap();

    state.next_line();

    assert_eq!(rows(&state)[2], "   ");
    assert_eq!(
        [
            rows(&state)[0].clone(),
            rows(&state)[1].clone(),
            rows(&state)[3].clone(),
            rows(&state)[4].clone(),
            rows(&state)[5].clone(),
        ],
        outside
    );
    assert_eq!((state.cursor().row(), state.cursor().column()), (2, 0));
}

#[test]
fn full_screen_nel_keeps_established_bottom_scroll_behavior() {
    let mut state = labeled_state();
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::full_screen(6)
    );
    state.set_cursor_position(5, 2).unwrap();

    state.next_line();

    assert_eq!(rows(&state), ["bbb", "ccc", "ddd", "eee", "fff", "   "]);
    assert_eq!((state.cursor().row(), state.cursor().column()), (5, 0));
}
