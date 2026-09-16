use terminal_core::{
    AutoWrapMode, Cell, CellColor, CharacterInsertionMode, CursorKeyMode, CursorVisibility,
    InverseVideo, ItalicStyle, TerminalDimensions, TerminalReply, TerminalState, TextIntensity,
    UnderlineStyle,
};

fn labeled_state() -> TerminalState {
    let mut state = TerminalState::new(TerminalDimensions::new(5, 3).unwrap());
    for (row, text) in ["abcde", "fghij", "klmno"].into_iter().enumerate() {
        state.set_cursor_position(row, 0).unwrap();
        for character in text.chars() {
            state.print_character(character).unwrap();
        }
    }
    state
}

fn row(state: &TerminalState, row: usize) -> String {
    (0..state.dimensions().columns())
        .map(|column| state.screen().cell(row, column).unwrap().character())
        .collect()
}

#[test]
fn delete_characters_shifts_only_the_current_row_and_clamps_counts() {
    for (column, count, expected) in [
        (0, 0, "ghij "),
        (0, 1, "ghij "),
        (2, 2, "fgj  "),
        (2, 3, "fg   "),
        (2, usize::MAX, "fg   "),
        (4, 1, "fghi "),
    ] {
        let mut state = labeled_state();
        state.set_cursor_position(1, column).unwrap();
        let cursor = state.cursor();
        state.delete_characters(count);

        assert_eq!(row(&state, 1), expected, "column {column}, count {count}");
        assert_eq!(row(&state, 0), "abcde");
        assert_eq!(row(&state, 2), "klmno");
        assert_eq!(state.cursor(), cursor);
    }
}

#[test]
fn delete_characters_moves_complete_cells_and_fills_with_canonical_blanks() {
    let mut state = labeled_state();
    state.set_text_intensity(TextIntensity::Bold);
    state.set_italic_style(ItalicStyle::Italic);
    state.set_underline_style(UnderlineStyle::Enabled);
    state.set_inverse_video(InverseVideo::Enabled);
    state.set_foreground_color(CellColor::Indexed(196));
    state.set_background_color(CellColor::Indexed(22));
    state.set_cursor_position(1, 3).unwrap();
    state.print_character('X').unwrap();
    let styled = *state.screen().cell(1, 3).unwrap();
    state.set_cursor_position(1, 1).unwrap();

    state.delete_characters(2);

    assert_eq!(state.screen().cell(1, 1), Some(&styled));
    assert_eq!(state.screen().cell(1, 3), Some(&Cell::default()));
    assert_eq!(state.screen().cell(1, 4), Some(&Cell::default()));
    assert_eq!(state.screen().cell(1, 0).unwrap().character(), 'f');
}

#[test]
fn delete_characters_is_independent_of_typing_insert_mode_and_preserves_state() {
    for insertion in [
        CharacterInsertionMode::Replace,
        CharacterInsertionMode::Insert,
    ] {
        let mut state = labeled_state();
        state.set_text_intensity(TextIntensity::Bold);
        state.set_cursor_visibility(CursorVisibility::Hidden);
        state.set_auto_wrap(AutoWrapMode::Disabled);
        state.set_character_insertion(insertion);
        state.set_cursor_key_mode(CursorKeyMode::Application);
        assert!(state.set_vertical_scrolling_margins(1, 2));
        state.set_cursor_position(1, 2).unwrap();
        state.set_horizontal_tab_stop();
        assert!(state.request_cursor_position_report());

        let cursor = state.cursor();
        let rendition = *state.current_rendition();
        let terminal_modes = *state.terminal_modes();
        let input_modes = *state.input_modes();
        let margins = state.vertical_scrolling_margins();
        state.delete_characters(2);

        assert_eq!(row(&state, 1), "fgj  ");
        assert_eq!(state.cursor(), cursor);
        assert_eq!(*state.current_rendition(), rendition);
        assert_eq!(*state.terminal_modes(), terminal_modes);
        assert_eq!(*state.input_modes(), input_modes);
        assert_eq!(state.vertical_scrolling_margins(), margins);
        assert!(state.has_horizontal_tab_stop(2));
        assert_eq!(
            state.take_reply(),
            Some(TerminalReply::CursorPosition { row: 2, column: 3 })
        );
    }
}

#[test]
fn delete_characters_cancels_delayed_wrap_like_ich_and_erase() {
    let mut state = labeled_state();
    state.set_cursor_position(1, 4).unwrap();
    state.print_character('X').unwrap();
    state.delete_characters(1);

    state.print_character('Y').unwrap();

    assert_eq!(state.screen().cell(1, 4).unwrap().character(), 'Y');
    assert_eq!(state.screen().cell(2, 0).unwrap().character(), 'k');
}

#[test]
fn insert_and_delete_characters_compose_as_bounded_current_row_edits() {
    let mut inserted_then_deleted = labeled_state();
    inserted_then_deleted.set_cursor_position(1, 2).unwrap();
    inserted_then_deleted.insert_characters(1);
    inserted_then_deleted.delete_characters(1);
    assert_eq!(row(&inserted_then_deleted, 1), "fghi ");

    let mut deleted_then_inserted = labeled_state();
    deleted_then_inserted.set_cursor_position(1, 2).unwrap();
    deleted_then_inserted.delete_characters(1);
    deleted_then_inserted.insert_characters(1);
    assert_eq!(row(&deleted_then_inserted, 1), "fg ij");
}
