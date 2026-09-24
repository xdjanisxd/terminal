use terminal_core::{
    Cell, CellColor, CellOccupancy, MAX_SCROLLBACK_ROWS, TerminalDimensions, TerminalState,
    TextIntensity,
};

fn state(columns: usize, rows: usize) -> TerminalState {
    TerminalState::new(TerminalDimensions::new(columns, rows).unwrap())
}

fn row_text(row: &[Cell]) -> String {
    row.iter().map(|cell| cell.character()).collect()
}

fn visible_row_text(state: &TerminalState, row: usize) -> String {
    (0..state.dimensions().columns())
        .map(|column| state.screen().cell(row, column).unwrap().character())
        .collect()
}

fn viewport_row_text(state: &TerminalState, row: usize) -> String {
    (0..state.dimensions().columns())
        .map(|column| {
            state
                .viewport_cell(row, column)
                .map_or(' ', |cell| cell.character())
        })
        .collect()
}

fn write_row(state: &mut TerminalState, row: usize, text: &str) {
    state.set_cursor_position(row, 0).unwrap();
    for character in text.chars() {
        state.print_character(character).unwrap();
    }
}

#[test]
fn primary_scrollback_starts_empty_and_alternate_has_no_independent_history() {
    let mut state = state(3, 2);

    assert_eq!(state.scrollback_len(), 0);
    assert_eq!(state.scrollback_row(0), None);

    state.switch_to_alternate_screen();
    write_row(&mut state, 0, "alt");
    state.set_cursor_position(1, 0).unwrap();
    state.index();

    assert_eq!(state.scrollback_len(), 0);
}

#[test]
fn primary_full_screen_index_captures_the_displaced_top_row() {
    let mut state = state(3, 2);
    write_row(&mut state, 0, "top");
    write_row(&mut state, 1, "bot");
    state.set_cursor_position(1, 2).unwrap();

    state.index();

    assert_eq!(state.scrollback_len(), 1);
    assert_eq!(row_text(state.scrollback_row(0).unwrap()), "top");
    assert_eq!(visible_row_text(&state, 0), "bot");
    assert_eq!(visible_row_text(&state, 1), "   ");
    assert_eq!(state.cursor().row(), 1);
    assert_eq!(state.cursor().column(), 2);
}

#[test]
fn full_screen_nel_and_wrapped_print_capture_in_chronological_order() {
    let mut state = state(2, 2);
    write_row(&mut state, 0, "aa");
    write_row(&mut state, 1, "bb");
    state.set_cursor_position(1, 1).unwrap();

    state.next_line();
    state.print_character('c').unwrap();
    state.print_character('d').unwrap();
    state.print_character('e').unwrap();

    assert_eq!(state.scrollback_len(), 2);
    assert_eq!(row_text(state.scrollback_row(0).unwrap()), "aa");
    assert_eq!(row_text(state.scrollback_row(1).unwrap()), "bb");
}

#[test]
fn full_screen_su_captures_each_displaced_row_and_restricted_su_does_not() {
    let mut state = state(2, 3);
    write_row(&mut state, 0, "aa");
    write_row(&mut state, 1, "bb");
    write_row(&mut state, 2, "cc");

    state.scroll_up(2);
    assert_eq!(state.scrollback_len(), 2);
    assert_eq!(row_text(state.scrollback_row(0).unwrap()), "aa");
    assert_eq!(row_text(state.scrollback_row(1).unwrap()), "bb");

    state.set_vertical_scrolling_margins(1, 2);
    state.scroll_up(1);
    assert_eq!(state.scrollback_len(), 2);
}

#[test]
fn scrollback_evicts_the_oldest_row_at_its_fixed_capacity() {
    let mut state = state(1, 1);
    for row in 0..=MAX_SCROLLBACK_ROWS {
        let character = char::from_u32(u32::from(b'A') + u32::try_from(row % 26).unwrap()).unwrap();
        write_row(&mut state, 0, &character.to_string());
        state.index();
    }

    assert_eq!(state.scrollback_len(), MAX_SCROLLBACK_ROWS);
    assert_eq!(
        state.scrollback_row(0).unwrap()[0].character(),
        char::from_u32(u32::from(b'A') + 1).unwrap()
    );
    assert_eq!(
        state.scrollback_row(MAX_SCROLLBACK_ROWS - 1).unwrap()[0].character(),
        char::from_u32(u32::from(b'A') + u32::try_from(MAX_SCROLLBACK_ROWS % 26).unwrap()).unwrap()
    );
}

#[test]
fn subregion_and_editing_or_downward_operations_do_not_capture_history() {
    let mut state = state(3, 4);
    for (row, text) in ["aaa", "bbb", "ccc", "ddd"].into_iter().enumerate() {
        write_row(&mut state, row, text);
    }
    state.set_vertical_scrolling_margins(1, 2);
    state.set_cursor_position(2, 0).unwrap();
    state.index();
    state.scroll_up(1);
    state.scroll_down(1);
    state.reverse_index();
    state.insert_lines(1);
    state.delete_lines(1);
    state.insert_characters(1);
    state.delete_characters(1);
    state.erase_characters(1);
    state.clear_screen();

    assert_eq!(state.scrollback_len(), 0);
}

#[test]
fn alternate_scrolling_does_not_append_primary_history_and_switches_preserve_it() {
    let mut state = state(2, 2);
    write_row(&mut state, 0, "p1");
    write_row(&mut state, 1, "p2");
    state.set_cursor_position(1, 0).unwrap();
    state.index();
    assert_eq!(state.scrollback_len(), 1);

    state.switch_to_alternate_screen();
    write_row(&mut state, 0, "a1");
    write_row(&mut state, 1, "a2");
    state.set_cursor_position(1, 0).unwrap();
    state.index();
    assert_eq!(state.scrollback_len(), 1);

    state.switch_to_primary_screen();
    assert_eq!(row_text(state.scrollback_row(0).unwrap()), "p1");

    state.enter_alternate_screen_1047();
    state.set_cursor_position(1, 0).unwrap();
    state.index();
    state.leave_alternate_screen_1047();
    assert_eq!(state.scrollback_len(), 1);

    state.enter_alternate_screen_1049();
    state.set_cursor_position(1, 0).unwrap();
    state.index();
    state.leave_alternate_screen_1049();
    assert_eq!(state.scrollback_len(), 1);
}

#[test]
fn captured_cells_preserve_combining_wide_occupancy_and_rendition() {
    let mut state = state(4, 2);
    state.set_foreground_color(CellColor::Rgb {
        red: 1,
        green: 2,
        blue: 3,
    });
    state.set_text_intensity(TextIntensity::Bold);
    state.print_character('e').unwrap();
    state.print_character('\u{301}').unwrap();
    state.print_character('界').unwrap();
    write_row(&mut state, 1, "next");
    state.set_cursor_position(1, 0).unwrap();

    state.index();

    let row = state.scrollback_row(0).unwrap();
    assert_eq!(row[0].character(), 'e');
    assert_eq!(row[0].combining_marks(), ['\u{301}']);
    assert_eq!(
        row[0].attributes().foreground(),
        CellColor::Rgb {
            red: 1,
            green: 2,
            blue: 3
        }
    );
    assert_eq!(row[0].attributes().intensity(), TextIntensity::Bold);
    assert_eq!(row[1].occupancy(), CellOccupancy::WideLead);
    assert_eq!(row[1].character(), '界');
    assert_eq!(row[2].occupancy(), CellOccupancy::WideContinuation);
    assert_eq!(row[2].combining_marks(), []);
}

#[test]
fn reset_clears_primary_history_and_resize_retains_capture_time_widths() {
    let mut state = state(4, 2);
    write_row(&mut state, 0, "wide");
    write_row(&mut state, 1, "row!");
    state.set_cursor_position(1, 0).unwrap();
    state.index();
    state.resize(TerminalDimensions::new(2, 3).unwrap());

    assert_eq!(state.scrollback_len(), 1);
    assert_eq!(state.scrollback_row(0).unwrap().len(), 4);
    assert_eq!(row_text(state.scrollback_row(0).unwrap()), "wide");

    state.reset();
    assert_eq!(state.scrollback_len(), 0);
}

#[test]
fn page_navigation_projects_history_and_returns_to_following_bottom() {
    let mut state = state(3, 2);
    write_row(&mut state, 0, "top");
    write_row(&mut state, 1, "bot");
    state.set_cursor_position(1, 0).unwrap();
    state.index();

    assert_eq!(state.viewport_offset(), 0);
    assert!(!state.page_down());
    assert!(state.page_up());
    assert_eq!(state.viewport_offset(), 1);
    assert_eq!(viewport_row_text(&state, 0), "top");
    assert_eq!(viewport_row_text(&state, 1), "bot");

    assert!(state.page_down());
    assert_eq!(state.viewport_offset(), 0);
    assert_eq!(viewport_row_text(&state, 0), "bot");
    assert_eq!(viewport_row_text(&state, 1), "   ");
}

#[test]
fn terminal_input_returns_scrolled_primary_to_live_bottom() {
    let mut state = state(3, 2);
    write_row(&mut state, 0, "old");
    write_row(&mut state, 1, "new");
    state.set_cursor_position(1, 0).unwrap();
    state.index();

    assert!(state.page_up());
    assert_eq!(viewport_row_text(&state, 0), "old");
    assert!(state.return_to_live_viewport());
    assert_eq!(state.viewport_offset(), 0);
    assert_eq!(viewport_row_text(&state, 0), "new");
    assert!(!state.return_to_live_viewport());
}

#[test]
fn alternate_input_leaves_primary_scrollback_position_unchanged() {
    let mut state = state(2, 2);
    write_row(&mut state, 0, "aa");
    state.set_cursor_position(1, 0).unwrap();
    state.index();
    assert!(state.page_up());
    state.switch_to_alternate_screen();
    assert!(!state.return_to_live_viewport());
    state.switch_to_primary_screen();
    assert_eq!(state.viewport_offset(), 1);
}

#[test]
fn output_preserves_a_scrolled_back_viewport_and_offset_is_bounded() {
    let mut state = state(3, 2);
    write_row(&mut state, 0, "one");
    write_row(&mut state, 1, "two");
    state.set_cursor_position(1, 0).unwrap();
    state.index();
    assert!(state.page_up());
    assert_eq!(viewport_row_text(&state, 0), "one");
    assert_eq!(viewport_row_text(&state, 1), "two");

    state.index();

    assert_eq!(state.scrollback_len(), 2);
    assert_eq!(state.viewport_offset(), 2);
    assert_eq!(viewport_row_text(&state, 0), "one");
    assert_eq!(viewport_row_text(&state, 1), "two");
    assert!(!state.page_up());
}

#[test]
fn resize_clamps_the_offset_and_reset_returns_the_viewport_to_bottom() {
    let mut state = state(2, 2);
    write_row(&mut state, 0, "aa");
    write_row(&mut state, 1, "bb");
    state.set_cursor_position(1, 0).unwrap();
    state.index();
    assert!(state.page_up());

    state.resize(TerminalDimensions::new(1, 3).unwrap());
    assert_eq!(state.viewport_offset(), 1);
    assert_eq!(state.viewport_cell(0, 0).unwrap().character(), 'a');
    assert_eq!(state.viewport_cell(0, 1), None);

    state.reset();
    assert_eq!(state.scrollback_len(), 0);
    assert_eq!(state.viewport_offset(), 0);
}

#[test]
fn alternate_screen_cannot_navigate_primary_history() {
    let mut state = state(2, 2);
    write_row(&mut state, 0, "aa");
    write_row(&mut state, 1, "bb");
    state.set_cursor_position(1, 0).unwrap();
    state.index();
    assert!(state.page_up());
    assert_eq!(state.viewport_offset(), 1);

    state.switch_to_alternate_screen();
    assert_eq!(state.viewport_offset(), 0);
    assert!(!state.page_up());
    state.switch_to_primary_screen();
    assert_eq!(state.viewport_offset(), 1);
    assert_eq!(viewport_row_text(&state, 0), "aa");
}

#[test]
fn row_scrolling_is_bounded_and_resumes_following_at_bottom() {
    let mut state = state(2, 2);
    write_row(&mut state, 0, "aa");
    write_row(&mut state, 1, "bb");
    state.set_cursor_position(1, 0).unwrap();
    state.index();
    state.index();
    assert_eq!(state.scrollback_len(), 2);

    assert!(state.scroll_viewport_rows(1));
    assert_eq!(state.viewport_offset(), 1);
    assert_eq!(viewport_row_text(&state, 0), "bb");
    assert!(state.scroll_viewport_rows(i32::MAX));
    assert_eq!(state.viewport_offset(), 2);
    assert!(!state.scroll_viewport_rows(1));

    state.index();
    assert_eq!(state.viewport_offset(), 3);
    assert!(state.scroll_viewport_rows(i32::MIN));
    assert_eq!(state.viewport_offset(), 0);
    assert!(!state.scroll_viewport_rows(-1));
    state.index();
    assert_eq!(state.viewport_offset(), 0);

    state.switch_to_alternate_screen();
    assert!(!state.scroll_viewport_rows(3));
    assert_eq!(state.viewport_offset(), 0);
    state.switch_to_primary_screen();
    assert_eq!(state.viewport_offset(), 0);
}
