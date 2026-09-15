use terminal_core::{
    AutoWrapMode, CellAttributes, CellColor, CharacterInsertionMode, CursorKeyMode,
    CursorVisibility, TerminalDimensions, TerminalState, VerticalScrollingMargins,
};

type ScrollCase = (
    &'static str,
    fn(&mut TerminalState),
    [&'static str; 4],
    (usize, usize),
);

fn row_text(state: &TerminalState, row: usize) -> String {
    (0..state.dimensions().columns())
        .map(|column| state.screen().cell(row, column).unwrap().character())
        .collect()
}

fn labeled_state() -> TerminalState {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 4).unwrap());
    for (row, text) in ["ab", "cd", "ef", "gh"].into_iter().enumerate() {
        state.set_cursor_position(row, 0).unwrap();
        for character in text.chars() {
            state.print_character(character).unwrap();
        }
    }
    state
}

#[test]
fn scrolling_margins_are_typed_zero_based_and_default_to_the_full_screen() {
    let state = TerminalState::new(TerminalDimensions::new(8, 6).unwrap());

    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::new(0, 5, 6).unwrap()
    );
    assert_eq!(state.vertical_scrolling_margins().top(), 0);
    assert_eq!(state.vertical_scrolling_margins().bottom(), 5);
}

#[test]
fn typed_scrolling_margins_enforce_order_and_screen_bounds() {
    assert_eq!(
        VerticalScrollingMargins::new(0, 0, 1).unwrap(),
        VerticalScrollingMargins::full_screen(1)
    );
    assert!(VerticalScrollingMargins::new(0, 3, 4).is_some());
    assert!(VerticalScrollingMargins::new(1, 1, 4).is_some());
    assert!(VerticalScrollingMargins::new(2, 1, 4).is_none());
    assert!(VerticalScrollingMargins::new(0, 4, 4).is_none());
    assert!(VerticalScrollingMargins::new(usize::MAX, usize::MAX, 4).is_none());
}

#[test]
fn setting_valid_margins_is_atomic_homes_cursor_and_preserves_unrelated_state() {
    let mut state = TerminalState::new(TerminalDimensions::new(20, 6).unwrap());
    state.set_cursor_visibility(CursorVisibility::Hidden);
    state.set_auto_wrap(AutoWrapMode::Disabled);
    state.set_character_insertion(CharacterInsertionMode::Insert);
    state.set_cursor_key_mode(CursorKeyMode::Application);
    state.set_foreground_color(CellColor::Indexed(196));
    state.set_background_color(CellColor::Indexed(22));
    state.set_cursor_position(3, 5).unwrap();
    state.set_horizontal_tab_stop();
    let terminal_modes = *state.terminal_modes();
    let input_modes = *state.input_modes();
    let rendition = *state.current_rendition();

    assert!(state.set_vertical_scrolling_margins(1, 4));

    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::new(1, 4, 6).unwrap()
    );
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 0));
    assert_eq!(*state.terminal_modes(), terminal_modes);
    assert_eq!(*state.input_modes(), input_modes);
    assert_eq!(*state.current_rendition(), rendition);
    assert!(state.has_horizontal_tab_stop(5));
}

#[test]
fn rejected_margin_updates_preserve_the_previous_region_and_cursor() {
    let mut state = TerminalState::new(TerminalDimensions::new(8, 6).unwrap());
    assert!(state.set_vertical_scrolling_margins(1, 4));
    state.set_cursor_position(3, 2).unwrap();
    let expected = state.vertical_scrolling_margins();
    let cursor = state.cursor();

    for (top, bottom) in [(4, 3), (0, 6), (6, 6), (usize::MAX, usize::MAX)] {
        assert!(!state.set_vertical_scrolling_margins(top, bottom));
        assert_eq!(state.vertical_scrolling_margins(), expected);
        assert_eq!(state.cursor(), cursor);
    }
}

#[test]
fn setting_margins_cancels_delayed_wrap() {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 4).unwrap());
    state.print_character('a').unwrap();
    state.print_character('b').unwrap();

    assert!(state.set_vertical_scrolling_margins(1, 2));
    state.print_character('X').unwrap();

    assert_eq!(row_text(&state, 0), "Xb");
    assert_eq!(row_text(&state, 1), "  ");
}

#[test]
fn reset_and_resize_restore_full_screen_margins() {
    let mut state = TerminalState::new(TerminalDimensions::new(8, 6).unwrap());
    assert!(state.set_vertical_scrolling_margins(1, 4));
    state.reset();
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::full_screen(6)
    );

    assert!(state.set_vertical_scrolling_margins(1, 4));
    state.resize(TerminalDimensions::new(8, 9).unwrap());
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::full_screen(9)
    );

    assert!(state.set_vertical_scrolling_margins(2, 7));
    state.resize(TerminalDimensions::new(8, 3).unwrap());
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::full_screen(3)
    );

    state.resize(TerminalDimensions::new(8, 1).unwrap());
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::full_screen(1)
    );
}

#[test]
fn custom_margins_do_not_yet_change_full_screen_scrolling_controls() {
    let cases: &[ScrollCase] = &[
        (
            "SU",
            |state| state.scroll_up(1),
            ["cd", "ef", "gh", "  "],
            (2, 1),
        ),
        (
            "SD",
            |state| state.scroll_down(1),
            ["  ", "ab", "cd", "ef"],
            (2, 1),
        ),
        (
            "IND",
            |state| state.index(),
            ["cd", "ef", "gh", "  "],
            (3, 1),
        ),
        (
            "RI",
            |state| state.reverse_index(),
            ["  ", "ab", "cd", "ef"],
            (0, 1),
        ),
        (
            "NEL",
            |state| state.next_line(),
            ["cd", "ef", "gh", "  "],
            (3, 0),
        ),
    ];

    for (name, operation, expected, expected_cursor) in cases {
        let mut state = labeled_state();
        assert!(state.set_vertical_scrolling_margins(1, 2));
        let start = match *name {
            "IND" | "NEL" => (3, 1),
            "RI" => (0, 1),
            _ => (2, 1),
        };
        state.set_cursor_position(start.0, start.1).unwrap();

        operation(&mut state);

        for (row, expected_text) in expected.iter().enumerate() {
            assert_eq!(row_text(&state, row), *expected_text, "{name}, row {row}");
        }
        assert_eq!(
            (state.cursor().row(), state.cursor().column()),
            *expected_cursor,
            "{name}"
        );
        assert_eq!(
            state.vertical_scrolling_margins(),
            VerticalScrollingMargins::new(1, 2, 4).unwrap()
        );
    }
}

#[test]
fn setting_margins_does_not_change_existing_cells() {
    let mut state = labeled_state();
    let before = [
        row_text(&state, 0),
        row_text(&state, 1),
        row_text(&state, 2),
        row_text(&state, 3),
    ];

    assert!(state.set_vertical_scrolling_margins(1, 2));

    assert_eq!(
        [
            row_text(&state, 0),
            row_text(&state, 1),
            row_text(&state, 2),
            row_text(&state, 3),
        ],
        before
    );
    assert_eq!(state.current_rendition(), &CellAttributes::default());
}
