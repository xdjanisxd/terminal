use terminal_core::{
    AutoWrapMode, Cell, CellColor, CharacterInsertionMode, CursorKeyMode, CursorVisibility,
    InverseVideo, ItalicStyle, TerminalDimensions, TerminalState, TextIntensity, UnderlineStyle,
    VerticalScrollingMargins,
};

type ScrollOperation = fn(&mut TerminalState, usize);

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
fn su_scrolls_only_custom_region_for_one_and_multiple_rows() {
    for (count, expected) in [
        (1, ["ab", "ef", "gh", "ij", "  ", "kl"]),
        (2, ["ab", "gh", "ij", "  ", "  ", "kl"]),
    ] {
        let mut state = labeled_state(1, 4);
        state.set_cursor_position(5, 1).unwrap();
        let cursor = state.cursor();
        state.scroll_up(count);
        assert_eq!(rows(&state), expected);
        assert_eq!(state.cursor(), cursor);
    }
}

#[test]
fn sd_scrolls_only_custom_region_for_one_and_multiple_rows() {
    for (count, expected) in [
        (1, ["ab", "  ", "cd", "ef", "gh", "kl"]),
        (2, ["ab", "  ", "  ", "cd", "ef", "kl"]),
    ] {
        let mut state = labeled_state(1, 4);
        state.set_cursor_position(0, 1).unwrap();
        let cursor = state.cursor();
        state.scroll_down(count);
        assert_eq!(rows(&state), expected);
        assert_eq!(state.cursor(), cursor);
    }
}

#[test]
fn region_scroll_zero_is_no_op_and_large_counts_clear_only_active_region() {
    for operation in [
        TerminalState::scroll_up as ScrollOperation,
        TerminalState::scroll_down as ScrollOperation,
    ] {
        let mut zero = labeled_state(1, 4);
        let before = rows(&zero);
        operation(&mut zero, 0);
        assert_eq!(rows(&zero), before);

        for count in [4, 5, usize::MAX] {
            let mut state = labeled_state(1, 4);
            operation(&mut state, count);
            assert_eq!(rows(&state), ["ab", "  ", "  ", "  ", "  ", "kl"]);
        }
    }
}

#[test]
fn region_scroll_handles_one_row_and_regions_at_screen_edges() {
    for operation in [
        TerminalState::scroll_up as ScrollOperation,
        TerminalState::scroll_down as ScrollOperation,
    ] {
        let mut one_row = labeled_state(2, 2);
        one_row.set_cursor_position(5, 1).unwrap();
        let cursor = one_row.cursor();
        operation(&mut one_row, 1);
        assert_eq!(rows(&one_row), ["ab", "cd", "  ", "gh", "ij", "kl"]);
        assert_eq!(one_row.cursor(), cursor);

        let mut starts_at_zero = labeled_state(0, 2);
        operation(&mut starts_at_zero, 1);
        let expected =
            if std::ptr::fn_addr_eq(operation, TerminalState::scroll_up as ScrollOperation) {
                ["cd", "ef", "  ", "gh", "ij", "kl"]
            } else {
                ["  ", "ab", "cd", "gh", "ij", "kl"]
            };
        assert_eq!(rows(&starts_at_zero), expected);

        let mut ends_at_last = labeled_state(3, 5);
        operation(&mut ends_at_last, 1);
        let expected =
            if std::ptr::fn_addr_eq(operation, TerminalState::scroll_up as ScrollOperation) {
                ["ab", "cd", "ef", "ij", "kl", "  "]
            } else {
                ["ab", "cd", "ef", "  ", "gh", "ij"]
            };
        assert_eq!(rows(&ends_at_last), expected);
    }
}

#[test]
fn region_scroll_preserves_complete_cells_and_unrelated_terminal_state() {
    for (operation, moved_row, exposed_row) in [
        (TerminalState::scroll_up as ScrollOperation, 2, 4),
        (TerminalState::scroll_down as ScrollOperation, 4, 1),
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
        state.set_auto_wrap(AutoWrapMode::Disabled);
        state.set_cursor_key_mode(CursorKeyMode::Application);
        state.set_cursor_position(3, 0).unwrap();
        state.print_character('X').unwrap();
        let styled = *state.screen().cell(3, 0).unwrap();
        state.set_cursor_position(0, 1).unwrap();
        state.set_horizontal_tab_stop();
        state.set_cursor_position(2, 1).unwrap();

        let cursor = state.cursor();
        let rendition = *state.current_rendition();
        let terminal_modes = *state.terminal_modes();
        let input_modes = *state.input_modes();
        let margins = state.vertical_scrolling_margins();

        operation(&mut state, 1);

        assert_eq!(state.screen().cell(moved_row, 0), Some(&styled));
        for column in 0..2 {
            assert_eq!(
                state.screen().cell(exposed_row, column),
                Some(&Cell::default())
            );
        }
        assert_eq!(state.cursor(), cursor);
        assert_eq!(*state.current_rendition(), rendition);
        assert_eq!(*state.terminal_modes(), terminal_modes);
        assert_eq!(*state.input_modes(), input_modes);
        assert_eq!(state.vertical_scrolling_margins(), margins);
        assert!(state.has_horizontal_tab_stop(1));

        state.print_character('Y').unwrap();
        assert_eq!(state.screen().cell(2, 1).unwrap().attributes(), &rendition);
    }
}

#[test]
fn region_scroll_preserves_existing_delayed_wrap_behavior() {
    for operation in [
        TerminalState::scroll_up as ScrollOperation,
        TerminalState::scroll_down as ScrollOperation,
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
fn full_screen_margins_keep_established_su_sd_behavior() {
    let mut up = labeled_state(0, 5);
    up.scroll_up(2);
    assert_eq!(rows(&up), ["ef", "gh", "ij", "kl", "  ", "  "]);

    let mut down = labeled_state(0, 5);
    down.scroll_down(2);
    assert_eq!(rows(&down), ["  ", "  ", "ab", "cd", "ef", "gh"]);
    assert_eq!(
        down.vertical_scrolling_margins(),
        VerticalScrollingMargins::full_screen(6)
    );
}
