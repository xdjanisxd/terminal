use terminal_core::{
    Cell, CellOccupancy, MAX_SCROLLBACK_ROWS, TerminalDimensions, TerminalParser, TerminalState,
};

fn state(columns: usize, rows: usize, text: &str) -> TerminalState {
    let mut state = TerminalState::new(TerminalDimensions::new(columns, rows).unwrap());
    TerminalParser::new()
        .advance(&mut state, text.as_bytes())
        .unwrap();
    state
}

fn resize(state: &mut TerminalState, columns: usize, rows: usize) {
    state.resize(TerminalDimensions::new(columns, rows).unwrap());
}

/// Extract logical lines through the public viewport, including history and
/// wrap widths. This checks what renderer/search/selection clients can see.
fn text(state: &mut TerminalState) -> String {
    let offset = state.viewport_offset();
    let history = state.scrollback_len();
    let mut result = String::new();
    for index in 0..history + state.dimensions().rows() {
        state.return_to_live_viewport();
        let row = if index < history {
            state.scroll_viewport_rows((history - index) as i32);
            0
        } else {
            index - history
        };
        let wrapped = state.viewport_wrap_columns(row);
        let cells = (0..state.dimensions().columns())
            .map(|column| {
                state
                    .viewport_cell(row, column)
                    .copied()
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        let len = if wrapped != 0 {
            wrapped
        } else {
            cells
                .iter()
                .rposition(|cell| *cell != Cell::default())
                .map_or(0, |i| i + 1)
        };
        for cell in &cells[..len] {
            if !cell.is_wide_continuation() {
                result.push(cell.character());
                result.extend(cell.combining_marks());
            }
        }
        if wrapped == 0 {
            result.push('\n');
        }
    }
    state.return_to_live_viewport();
    state.scroll_viewport_rows(offset as i32);
    result.trim_end_matches('\n').to_owned()
}

fn valid_cells(state: &TerminalState) {
    let check = |row: &[Cell]| {
        for (column, cell) in row.iter().enumerate() {
            match cell.occupancy() {
                CellOccupancy::WideLead => {
                    assert!(row.get(column + 1).unwrap().is_wide_continuation())
                }
                CellOccupancy::WideContinuation => {
                    assert!(column > 0 && row[column - 1].is_wide_lead())
                }
                CellOccupancy::Single => {}
            }
        }
    };
    for row in 0..state.scrollback_len() {
        check(state.scrollback_row(row).unwrap());
    }
    for row in 0..state.dimensions().rows() {
        check(
            &(0..state.dimensions().columns())
                .map(|col| *state.screen().cell(row, col).unwrap())
                .collect::<Vec<_>>(),
        );
    }
}

#[test]
fn emoji_widths_and_text_survive_scrollback_reflow() {
    let mut state = state(8, 2, "A🚀B 🚀C\r\nnext 🚀 line\r\nend");
    assert!(state.scrollback_len() > 0);
    let expected = text(&mut state);
    for columns in [4, 12, 3, 8] {
        resize(&mut state, columns, 2);
        assert_eq!(text(&mut state), expected, "columns={columns}");
        valid_cells(&state);
        for row in 0..state.scrollback_len() {
            for cell in state.scrollback_row(row).unwrap() {
                if cell.character() == '🚀' {
                    assert!(cell.is_wide_lead());
                }
            }
        }
    }
}

#[test]
fn hard_breaks_soft_wraps_attributes_links_spaces_and_combining_survive_round_trips() {
    let input = "abcdefghijk\r\n\r\n\x1b[1;3;4;38;2;1;2;3m界e\u{301}  \x1b[0m\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\";
    let mut state = state(8, 8, input);
    let expected = text(&mut state);
    let styled = *state.screen().cell(3, 0).unwrap().attributes();
    for columns in [3, 1, 2, 9, 4, 12, 1, 8] {
        resize(&mut state, columns, 16);
        assert_eq!(text(&mut state), expected, "width {columns}");
        valid_cells(&state);
        let mut found_style = false;
        let mut found_link = false;
        state.scroll_viewport_rows(i32::MAX);
        for _ in 0..state.scrollback_len() + 16 {
            for row in 0..16 {
                for col in 0..columns {
                    let cell = state.viewport_cell(row, col).unwrap();
                    if cell.character() == '界' {
                        assert_eq!(*cell.attributes(), styled);
                        found_style = true;
                    }
                    found_link |= state.target_at_viewport(row, col) == Some("https://example.com");
                }
            }
            state.scroll_viewport_rows(-1);
        }
        assert!(found_style && found_link);
    }
}

#[test]
fn cursor_insertion_and_delayed_wrap_follow_the_text() {
    let mut state = state(4, 3, "abcdef");
    resize(&mut state, 2, 3);
    assert_eq!((state.cursor().row(), state.cursor().column()), (2, 1));
    state.print_character('g').unwrap();
    assert_eq!(text(&mut state), "abcdefg");
    resize(&mut state, 7, 3);
    state.print_character('\u{301}').unwrap();
    state.print_character('h').unwrap();
    assert_eq!(text(&mut state), "abcdefg\u{301}h");
}

#[test]
fn saved_cursor_and_inactive_primary_reflow_independently_of_alternate() {
    let mut state = state(6, 4, "abcdefghi");
    state.set_cursor_position(0, 1).unwrap();
    state.save_cursor();
    state.set_cursor_position(1, 3).unwrap();
    state.enter_alternate_screen_1047();
    for c in "TUI".chars() {
        state.print_character(c).unwrap();
    }
    resize(&mut state, 3, 4);
    assert_eq!(state.screen().cell(0, 0).unwrap().character(), 'T');
    assert_eq!(state.screen().cell(0, 2).unwrap().character(), 'I');
    state.leave_alternate_screen_1047();
    state.restore_cursor();
    state.print_character('Z').unwrap();
    assert_eq!(text(&mut state), "aZcdefghi");
}

#[test]
fn mouse_keyboard_and_backward_selection_keep_logical_endpoints() {
    for backward in [false, true] {
        let mut state = state(6, 5, "abcdefghijklmnop");
        let (start, end) = if backward {
            ((2, 2), (0, 2))
        } else {
            ((0, 2), (2, 2))
        };
        state.begin_selection(start.0, start.1);
        state.extend_selection(end.0, end.1);
        let expected = state.selected_text().unwrap();
        for columns in [3, 1, 9, 6] {
            resize(&mut state, columns, 8);
            assert_eq!(state.selected_text().unwrap(), expected);
        }
    }
    let mut state = state(6, 5, "abcd界efgh");
    state.extend_keyboard_selection(false, false);
    state.extend_keyboard_selection(false, false);
    let expected = state.selected_text().unwrap();
    for columns in [1, 3, 10] {
        resize(&mut state, columns, 8);
        assert_eq!(state.selected_text().unwrap(), expected);
    }
}

#[test]
fn viewport_stays_on_its_text_and_live_view_stays_live() {
    let mut state = state(6, 2, "abcdefghijklmnopqrstuvwxyz");
    state.scroll_viewport_rows(2);
    assert_eq!(state.viewport_cell(0, 0).unwrap().character(), 'g');
    resize(&mut state, 3, 2);
    assert_eq!(state.viewport_cell(0, 0).unwrap().character(), 'g');
    resize(&mut state, 6, 2);
    assert_eq!(state.viewport_cell(0, 0).unwrap().character(), 'g');
    state.return_to_live_viewport();
    resize(&mut state, 2, 3);
    assert_eq!(state.viewport_offset(), 0);
}

#[test]
fn height_only_resize_moves_rows_between_history_and_grid() {
    let mut state = state(4, 3, "one\r\ntwo\r\ntri");
    let expected = text(&mut state);
    for rows in [1, 2, 6, 1, 3] {
        resize(&mut state, 4, rows);
        assert_eq!(text(&mut state), expected);
        assert!(state.cursor().row() < rows);
    }
}

#[test]
fn one_column_printing_and_repeated_narrowing_keep_wide_payloads() {
    let mut state = state(1, 3, "界\u{301}語x");
    assert_eq!(text(&mut state), "界\u{301}語x");
    for columns in [2, 1, 4, 1, 8] {
        resize(&mut state, columns, 4);
        valid_cells(&state);
        assert_eq!(text(&mut state), "界\u{301}語x");
    }
}

#[test]
fn history_eviction_is_bounded_and_discarded_selection_is_cleared() {
    let mut state = state(4, 2, "");
    let mut parser = TerminalParser::new();
    for _ in 0..MAX_SCROLLBACK_ROWS + 2 {
        parser.advance(&mut state, b"abcd\r\n").unwrap();
    }
    state.scroll_viewport_rows(i32::MAX);
    state.select_word(0, 0);
    resize(&mut state, 1, 1);
    assert_eq!(state.scrollback_len(), MAX_SCROLLBACK_ROWS);
    assert_eq!(state.selected_text(), None);
    assert!(state.viewport_offset() <= state.scrollback_len());
    valid_cells(&state);
    resize(&mut state, 4, 2);
    state.return_to_live_viewport();
    state.print_character('Z').unwrap();
    assert_eq!(
        state
            .screen()
            .cell(state.cursor().row(), 0)
            .unwrap()
            .character(),
        'Z'
    );
}

#[test]
fn unchanged_resize_preserves_pending_wrap_selection_and_alternate_contents() {
    let mut state = state(2, 2, "ab");
    state.select_word(0, 0);
    resize(&mut state, 2, 2);
    assert_eq!(state.selected_text().as_deref(), Some("ab"));
    state.print_character('c').unwrap();
    assert_eq!(text(&mut state), "abc");
    state.enter_alternate_screen_1047();
    for c in "abcd".chars() {
        state.print_character(c).unwrap();
    }
    state.select_word(0, 0);
    resize(&mut state, 3, 3);
    assert_eq!(state.selected_text().as_deref(), Some("ab"));
    assert_eq!(state.screen().cell(0, 0).unwrap().character(), 'a');
    assert_eq!(state.screen().cell(1, 0).unwrap().character(), 'c');
    assert_eq!(state.viewport_offset(), 0);
}

#[test]
fn shrinking_with_content_below_cursor_keeps_the_cursor_on_its_text() {
    let mut state = state(120, 100, &("x".repeat(119) + "\r\n").repeat(99));
    state.set_cursor_position(0, 0).unwrap();
    resize(&mut state, 1, 1);
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 0));
    state.print_character('Z').unwrap();
    assert_eq!(state.screen().cell(0, 0).unwrap().character(), 'Z');
    assert_eq!(state.scrollback_len(), 0);
}

#[test]
fn selection_partly_evicted_before_reflow_keeps_its_retained_suffix() {
    let mut state = state(4, 2, "");
    let mut parser = TerminalParser::new();
    for _ in 0..MAX_SCROLLBACK_ROWS + 1 {
        parser.advance(&mut state, b"abcd\r\n").unwrap();
    }
    state.scroll_viewport_rows(i32::MAX);
    state.begin_selection(0, 0);
    state.extend_selection(1, 3);
    state.return_to_live_viewport();
    parser.advance(&mut state, b"abcd\r\n").unwrap();
    assert_eq!(state.selected_text().as_deref(), Some("abcd"));
    resize(&mut state, 8, 2);
    assert_eq!(state.selected_text().as_deref(), Some("abcd"));
}

#[test]
fn deterministic_resize_sequences_preserve_hard_lines_and_wide_wrap_gaps() {
    let input = "ab界e\u{301}f語ghi  \r\n\r\n  z界\r\nlast";
    for initial_width in [1, 2, 3, 4, 7, 12] {
        let mut state = state(initial_width, 4, input);
        let expected = text(&mut state);
        let mut seed = 12_345_u32;
        for _ in 0..60 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let columns = 1 + (seed as usize % 19);
            let rows = 1 + ((seed >> 8) as usize % 9);
            resize(&mut state, columns, rows);
            assert_eq!(
                text(&mut state),
                expected,
                "{initial_width} -> {columns}x{rows}"
            );
            valid_cells(&state);
        }
    }
}

#[test]
fn mouse_selection_retains_spaces_that_land_on_new_soft_boundaries() {
    let mut state = state(8, 4, "ab  cd界e\u{301}");
    state.begin_selection(0, 0);
    state.extend_selection(1, 0);
    let expected = state.selected_text().unwrap();
    assert_eq!(expected, "ab  cd界e\u{301}");
    for columns in [3, 1, 4, 8] {
        resize(&mut state, columns, 12);
        assert_eq!(state.selected_text().as_deref(), Some(expected.as_str()));
    }
}

#[test]
fn reflow_does_not_enable_delayed_wrap_when_auto_wrap_is_disabled() {
    let mut state = state(4, 3, "ab");
    state.set_auto_wrap(terminal_core::AutoWrapMode::Disabled);
    resize(&mut state, 2, 3);
    state.print_character('Z').unwrap();
    assert_eq!(state.cursor().row(), 0);
    assert_eq!(state.screen().cell(0, 0).unwrap().character(), 'a');
    assert_eq!(state.screen().cell(0, 1).unwrap().character(), 'Z');
}

#[test]
fn keyboard_selection_keeps_hard_breaks_and_explicit_spaces_without_erased_padding() {
    let mut state = state(8, 4, "ab  \r\ncd");
    for _ in 0..11 {
        state.extend_keyboard_selection(false, false);
    }
    assert_eq!(state.selected_text().as_deref(), Some("ab  \ncd"));
    for columns in [3, 1, 12, 8] {
        resize(&mut state, columns, 8);
        assert_eq!(state.selected_text().as_deref(), Some("ab  \ncd"));
    }
}
