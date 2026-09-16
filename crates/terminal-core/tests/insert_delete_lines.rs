use terminal_core::{
    AutoWrapMode, Cell, CellColor, CharacterInsertionMode, CursorKeyMode, CursorVisibility,
    InverseVideo, ItalicStyle, TerminalDimensions, TerminalReply, TerminalState, TextIntensity,
    UnderlineStyle, VerticalScrollingMargins,
};

type LineOperation = fn(&mut TerminalState, usize);

fn labeled_state(top: usize, bottom: usize) -> TerminalState {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 6).unwrap());
    for (row, text) in ["ab", "cd", "ef", "gh", "ij", "kl"].into_iter().enumerate() {
        state.set_cursor_position(row, 0).unwrap();
        for character in text.chars() {
            state.print_character(character).unwrap();
        }
    }
    assert!(state.set_vertical_scrolling_margins(top, bottom));
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

#[test]
fn insert_lines_is_cursor_relative_within_the_active_region() {
    let cases = [
        (1, 1, ["ab", "  ", "cd", "ef", "gh", "kl"]),
        (2, 1, ["ab", "cd", "  ", "ef", "gh", "kl"]),
        (3, 1, ["ab", "cd", "ef", "  ", "gh", "kl"]),
    ];
    for (cursor_row, count, expected) in cases {
        let mut state = labeled_state(1, 4);
        state.set_cursor_position(cursor_row, 1).unwrap();
        let cursor = state.cursor();
        state.insert_lines(count);
        assert_eq!(rows(&state), expected, "cursor row {cursor_row}");
        assert_eq!(state.cursor(), cursor);
    }
}

#[test]
fn delete_lines_is_cursor_relative_within_the_active_region() {
    let cases = [
        (1, 1, ["ab", "ef", "gh", "ij", "  ", "kl"]),
        (2, 1, ["ab", "cd", "gh", "ij", "  ", "kl"]),
        (3, 1, ["ab", "cd", "ef", "ij", "  ", "kl"]),
        (4, 1, ["ab", "cd", "ef", "gh", "  ", "kl"]),
    ];
    for (cursor_row, count, expected) in cases {
        let mut state = labeled_state(1, 4);
        state.set_cursor_position(cursor_row, 1).unwrap();
        let cursor = state.cursor();
        state.delete_lines(count);
        assert_eq!(rows(&state), expected, "cursor row {cursor_row}");
        assert_eq!(state.cursor(), cursor);
    }
}

#[test]
fn line_operations_clamp_to_the_cursor_to_bottom_subregion() {
    for operation in [
        TerminalState::insert_lines as LineOperation,
        TerminalState::delete_lines as LineOperation,
    ] {
        let mut state = labeled_state(1, 4);
        state.set_cursor_position(3, 0).unwrap();
        let before = rows(&state);
        operation(&mut state, 0);
        assert_eq!(rows(&state), before);

        for count in [2, 3, usize::MAX] {
            let mut state = labeled_state(1, 4);
            state.set_cursor_position(3, 0).unwrap();
            operation(&mut state, count);
            assert_eq!(rows(&state), ["ab", "cd", "ef", "  ", "  ", "kl"]);
        }
    }
}

#[test]
fn line_operations_are_no_ops_outside_margins_and_use_full_screen_defaults() {
    for operation in [
        TerminalState::insert_lines as LineOperation,
        TerminalState::delete_lines as LineOperation,
    ] {
        for cursor_row in [0, 5] {
            let mut state = labeled_state(1, 4);
            state.set_cursor_position(cursor_row, 1).unwrap();
            let before = rows(&state);
            operation(&mut state, 1);
            assert_eq!(rows(&state), before, "cursor row {cursor_row}");
        }
    }

    let mut insert = labeled_state(0, 5);
    insert.set_cursor_position(3, 0).unwrap();
    insert.insert_lines(2);
    assert_eq!(rows(&insert), ["ab", "cd", "ef", "  ", "  ", "gh"]);

    let mut delete = labeled_state(0, 5);
    delete.set_cursor_position(3, 0).unwrap();
    delete.delete_lines(2);
    assert_eq!(rows(&delete), ["ab", "cd", "ef", "kl", "  ", "  "]);
}

#[test]
fn line_operations_preserve_complete_cells_canonical_blanks_and_unrelated_state() {
    for (operation, source_row, moved_row, blank_rows) in [
        (TerminalState::insert_lines as LineOperation, 2, 3, [2, 2]),
        (TerminalState::delete_lines as LineOperation, 3, 2, [4, 4]),
    ] {
        let mut state = labeled_state(1, 4);
        state.set_text_intensity(TextIntensity::Bold);
        state.set_italic_style(ItalicStyle::Italic);
        state.set_underline_style(UnderlineStyle::Enabled);
        state.set_inverse_video(InverseVideo::Enabled);
        state.set_foreground_color(CellColor::Indexed(196));
        state.set_background_color(CellColor::Indexed(22));
        state.set_cursor_visibility(CursorVisibility::Hidden);
        state.set_character_insertion(CharacterInsertionMode::Insert);
        state.set_auto_wrap(AutoWrapMode::Enabled);
        state.set_cursor_key_mode(CursorKeyMode::Application);
        state.set_cursor_position(source_row, 0).unwrap();
        state.print_character('X').unwrap();
        let styled = *state.screen().cell(source_row, 0).unwrap();
        state.set_cursor_position(2, 1).unwrap();
        state.set_horizontal_tab_stop();
        assert!(state.request_cursor_position_report());

        let cursor = state.cursor();
        let rendition = *state.current_rendition();
        let terminal_modes = *state.terminal_modes();
        let input_modes = *state.input_modes();
        let margins = state.vertical_scrolling_margins();
        operation(&mut state, 1);

        assert_eq!(state.screen().cell(moved_row, 0), Some(&styled));
        for row in blank_rows {
            for column in 0..2 {
                assert_eq!(state.screen().cell(row, column), Some(&Cell::default()));
            }
        }
        assert_eq!(state.cursor(), cursor);
        assert_eq!(*state.current_rendition(), rendition);
        assert_eq!(*state.terminal_modes(), terminal_modes);
        assert_eq!(*state.input_modes(), input_modes);
        assert_eq!(state.vertical_scrolling_margins(), margins);
        assert!(state.has_horizontal_tab_stop(1));
        assert_eq!(
            state.take_reply(),
            Some(TerminalReply::CursorPosition { row: 3, column: 2 })
        );
    }
}

#[test]
fn line_operations_preserve_delayed_wrap() {
    for operation in [
        TerminalState::insert_lines as LineOperation,
        TerminalState::delete_lines as LineOperation,
    ] {
        let mut state = TerminalState::new(TerminalDimensions::new(2, 6).unwrap());
        assert!(state.set_vertical_scrolling_margins(1, 4));
        state.set_cursor_position(2, 0).unwrap();
        state.print_character('a').unwrap();
        state.print_character('b').unwrap();
        let cursor = state.cursor();

        operation(&mut state, 1);

        assert_eq!(state.cursor(), cursor);
        state.print_character('X').unwrap();
        assert_eq!(state.screen().cell(3, 0).unwrap().character(), 'X');
        assert_eq!((state.cursor().row(), state.cursor().column()), (3, 1));
    }
}

#[test]
fn line_operations_leave_margins_intact() {
    let mut state = labeled_state(1, 4);
    state.set_cursor_position(2, 0).unwrap();
    state.insert_lines(1);
    state.delete_lines(1);
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::new(1, 4, 6).unwrap()
    );
}
