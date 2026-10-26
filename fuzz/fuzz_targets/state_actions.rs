#![no_main]

use libfuzzer_sys::fuzz_target;
use terminal_core::{MAX_SCROLLBACK_ROWS, TerminalDimensions, TerminalState};

fuzz_target!(|data: &[u8]| {
    if data.len() > 4_096 {
        return;
    }
    let mut state = TerminalState::new(TerminalDimensions::new(12, 5).unwrap());
    for command in data.chunks(3) {
        let row = usize::from(*command.get(1).unwrap_or(&0)) % state.dimensions().rows();
        let column = usize::from(*command.get(2).unwrap_or(&0)) % state.dimensions().columns();
        let value = *command.get(1).unwrap_or(&0);
        match command[0] % 13 {
            0 => state.index(),
            1 => {
                state.page_up();
            }
            2 => {
                state.page_down();
            }
            3 => {
                state.begin_selection(row, column);
            }
            4 => {
                state.extend_selection(row, column);
            }
            5 => {
                let _ = state.selected_text();
            }
            6 => {
                let columns = usize::from(value) % 32 + 1;
                let rows = usize::from(*command.get(2).unwrap_or(&0)) % 16 + 1;
                state.resize(TerminalDimensions::new(columns, rows).unwrap());
            }
            7 => state.reverse_index(),
            8 => {
                state.scroll_viewport_rows(i32::from(value) - 128);
            }
            9 => state.switch_to_alternate_screen(),
            10 => state.switch_to_primary_screen(),
            11 => match value % 3 {
                0 => state.reset(),
                1 => state.soft_reset(),
                _ => state.hard_reset(),
            },
            _ => {
                let character = ['a', '界', '\u{0301}', '👩', '\u{fe0f}'][usize::from(value) % 5];
                let _ = state.print_character(character);
            }
        }
        assert!(state.cursor().row() < state.dimensions().rows());
        assert!(state.cursor().column() < state.dimensions().columns());
        assert!(state.scrollback_len() <= MAX_SCROLLBACK_ROWS);
        assert!(state.viewport_offset() <= state.scrollback_len());
    }
    for row in 0..state.dimensions().rows() {
        let stored_columns = state.viewport_row_columns(row);
        assert!(stored_columns <= state.dimensions().columns());
        assert!(state.viewport_wrap_columns(row) <= state.dimensions().columns());
        for column in 0..stored_columns {
            assert!(state.viewport_cell(row, column).is_some());
        }
    }
});
