use terminal_core::{
    AutoWrapMode, Cell, CellColor, CharacterInsertionMode, CursorKeyMode, CursorVisibility,
    InverseVideo, ItalicStyle, TerminalDimensions, TerminalState, TextIntensity, UnderlineStyle,
    VerticalScrollingMargins,
};

type TerminalOperation = fn(&mut TerminalState);
type OutsideRegionCase = (TerminalOperation, (usize, usize), (usize, usize));
type DelayedWrapCase = (TerminalOperation, usize, usize);

fn labeled_state() -> TerminalState {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 6).unwrap());
    for (row, text) in ["ab", "cd", "ef", "gh", "ij", "kl"].into_iter().enumerate() {
        state.set_cursor_position(row, 0).unwrap();
        for character in text.chars() {
            state.print_character(character).unwrap();
        }
    }
    assert!(state.set_vertical_scrolling_margins(1, 4));
    state
}

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

#[test]
fn index_moves_inside_region_and_scrolls_only_at_bottom_margin() {
    let mut moving = labeled_state();
    moving.set_cursor_position(2, 1).unwrap();
    let before = rows(&moving);
    moving.index();
    assert_eq!((moving.cursor().row(), moving.cursor().column()), (3, 1));
    assert_eq!(rows(&moving), before);

    let mut scrolling = labeled_state();
    scrolling.set_cursor_position(4, 1).unwrap();
    scrolling.index();
    assert_eq!(
        (scrolling.cursor().row(), scrolling.cursor().column()),
        (4, 1)
    );
    assert_eq!(rows(&scrolling), ["ab", "ef", "gh", "ij", "  ", "kl"]);
    assert_eq!(
        scrolling.vertical_scrolling_margins(),
        VerticalScrollingMargins::new(1, 4, 6).unwrap()
    );
}

#[test]
fn reverse_index_moves_inside_region_and_scrolls_only_at_top_margin() {
    let mut moving = labeled_state();
    moving.set_cursor_position(3, 1).unwrap();
    let before = rows(&moving);
    moving.reverse_index();
    assert_eq!((moving.cursor().row(), moving.cursor().column()), (2, 1));
    assert_eq!(rows(&moving), before);

    let mut scrolling = labeled_state();
    scrolling.set_cursor_position(1, 1).unwrap();
    scrolling.reverse_index();
    assert_eq!(
        (scrolling.cursor().row(), scrolling.cursor().column()),
        (1, 1)
    );
    assert_eq!(rows(&scrolling), ["ab", "  ", "cd", "ef", "gh", "kl"]);
    assert_eq!(
        scrolling.vertical_scrolling_margins(),
        VerticalScrollingMargins::new(1, 4, 6).unwrap()
    );
}

#[test]
fn index_and_reverse_index_move_outside_region_without_scrolling_or_clamping() {
    let cases: &[OutsideRegionCase] = &[
        (TerminalState::index, (0, 1), (1, 1)),
        (TerminalState::reverse_index, (1, 1), (0, 1)),
        (TerminalState::reverse_index, (5, 1), (4, 1)),
        (TerminalState::index, (4, 1), (5, 1)),
        (TerminalState::reverse_index, (0, 1), (0, 1)),
        (TerminalState::index, (5, 1), (5, 1)),
    ];

    for (operation, start, expected) in cases {
        let mut state = TerminalState::new(TerminalDimensions::new(2, 6).unwrap());
        for (row, text) in ["ab", "cd", "ef", "gh", "ij", "kl"].into_iter().enumerate() {
            state.set_cursor_position(row, 0).unwrap();
            for character in text.chars() {
                state.print_character(character).unwrap();
            }
        }
        assert!(state.set_vertical_scrolling_margins(2, 3));
        state.set_cursor_position(start.0, start.1).unwrap();
        let before = rows(&state);
        operation(&mut state);
        assert_eq!((state.cursor().row(), state.cursor().column()), *expected);
        assert_eq!(rows(&state), before);
    }
}

#[test]
fn crossing_into_region_by_index_or_reverse_index_does_not_scroll() {
    let mut by_index = labeled_state();
    by_index.set_cursor_position(0, 1).unwrap();
    let before = rows(&by_index);
    by_index.index();
    assert_eq!(
        (by_index.cursor().row(), by_index.cursor().column()),
        (1, 1)
    );
    assert_eq!(rows(&by_index), before);

    let mut by_reverse_index = labeled_state();
    by_reverse_index.set_cursor_position(5, 1).unwrap();
    let before = rows(&by_reverse_index);
    by_reverse_index.reverse_index();
    assert_eq!(
        (
            by_reverse_index.cursor().row(),
            by_reverse_index.cursor().column()
        ),
        (4, 1)
    );
    assert_eq!(rows(&by_reverse_index), before);
}

#[test]
fn index_and_reverse_index_clear_only_a_single_row_region() {
    for (operation, _) in [
        (TerminalState::index as fn(&mut TerminalState), true),
        (
            TerminalState::reverse_index as fn(&mut TerminalState),
            false,
        ),
    ] {
        let mut state = labeled_state();
        assert!(state.set_vertical_scrolling_margins(2, 2));
        state.set_cursor_position(2, 1).unwrap();
        operation(&mut state);
        assert_eq!(rows(&state), ["ab", "cd", "  ", "gh", "ij", "kl"]);
        assert_eq!((state.cursor().row(), state.cursor().column()), (2, 1));
    }
}

#[test]
fn region_boundary_scrolling_preserves_styled_cells_and_unrelated_state() {
    for (operation, is_index) in [
        (TerminalState::index as fn(&mut TerminalState), true),
        (
            TerminalState::reverse_index as fn(&mut TerminalState),
            false,
        ),
    ] {
        let mut state = labeled_state();
        state.set_text_intensity(TextIntensity::Bold);
        state.set_italic_style(ItalicStyle::Italic);
        state.set_underline_style(UnderlineStyle::Enabled);
        state.set_inverse_video(InverseVideo::Enabled);
        state.set_foreground_color(CellColor::Indexed(196));
        state.set_background_color(CellColor::Indexed(22));
        state.set_cursor_visibility(CursorVisibility::Hidden);
        state.set_character_insertion(CharacterInsertionMode::Insert);
        state.set_auto_wrap(AutoWrapMode::Disabled);
        state.set_cursor_key_mode(CursorKeyMode::Application);
        state.set_cursor_position(3, 0).unwrap();
        state.print_character('X').unwrap();
        let styled = *state.screen().cell(3, 0).unwrap();
        let rendition = *state.current_rendition();
        let modes = *state.terminal_modes();
        let input_modes = *state.input_modes();
        state.set_cursor_position(0, 1).unwrap();
        state.set_horizontal_tab_stop();
        let margins = state.vertical_scrolling_margins();
        let boundary = if is_index {
            margins.bottom()
        } else {
            margins.top()
        };
        state.set_cursor_position(boundary, 1).unwrap();

        operation(&mut state);

        let moved_row = if is_index { 2 } else { 4 };
        assert_eq!(state.screen().cell(moved_row, 0), Some(&styled));
        let exposed_row = if is_index {
            margins.bottom()
        } else {
            margins.top()
        };
        for column in 0..2 {
            assert_eq!(
                state.screen().cell(exposed_row, column),
                Some(&Cell::default())
            );
        }
        assert_eq!(*state.current_rendition(), rendition);
        assert_eq!(*state.terminal_modes(), modes);
        assert_eq!(*state.input_modes(), input_modes);
        assert!(state.has_horizontal_tab_stop(1));
        assert_eq!(state.vertical_scrolling_margins(), margins);
        state.print_character('Y').unwrap();
        assert_eq!(
            state.screen().cell(boundary, 1).unwrap().attributes(),
            &rendition
        );
    }
}

#[test]
fn index_and_reverse_index_cancel_delayed_wrap_for_move_scroll_and_outside_cases() {
    let cases: &[DelayedWrapCase] = &[
        (TerminalState::index, 2, 3),
        (TerminalState::index, 4, 4),
        (TerminalState::index, 0, 1),
        (TerminalState::reverse_index, 3, 2),
        (TerminalState::reverse_index, 1, 1),
        (TerminalState::reverse_index, 5, 4),
    ];

    for (operation, start_row, expected_row) in cases {
        let mut state = TerminalState::new(TerminalDimensions::new(2, 6).unwrap());
        assert!(state.set_vertical_scrolling_margins(1, 4));
        state.set_cursor_position(*start_row, 0).unwrap();
        state.print_character('x').unwrap();
        state.print_character('y').unwrap();
        operation(&mut state);
        state.print_character('Z').unwrap();
        assert_eq!(
            (state.cursor().row(), state.cursor().column()),
            (*expected_row, 1)
        );
        assert_eq!(
            state.screen().cell(*expected_row, 1).unwrap().character(),
            'Z'
        );
    }
}

#[test]
fn full_screen_margins_preserve_previous_index_and_reverse_index_behavior() {
    let mut index = labeled_state();
    assert!(index.set_vertical_scrolling_margins(0, 5));
    index.set_cursor_position(5, 1).unwrap();
    index.index();
    assert_eq!(rows(&index), ["cd", "ef", "gh", "ij", "kl", "  "]);
    assert_eq!((index.cursor().row(), index.cursor().column()), (5, 1));

    let mut reverse = labeled_state();
    assert!(reverse.set_vertical_scrolling_margins(0, 5));
    reverse.set_cursor_position(0, 1).unwrap();
    reverse.reverse_index();
    assert_eq!(rows(&reverse), ["  ", "ab", "cd", "ef", "gh", "ij"]);
    assert_eq!((reverse.cursor().row(), reverse.cursor().column()), (0, 1));
}
