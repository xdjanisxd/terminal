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
        match command[0] % 7 {
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
            _ => {
                let columns = column + 1;
                let rows = row + 1;
                state.resize(TerminalDimensions::new(columns, rows).unwrap());
            }
        }
        assert!(state.cursor().row() < state.dimensions().rows());
        assert!(state.cursor().column() < state.dimensions().columns());
        assert!(state.scrollback_len() <= MAX_SCROLLBACK_ROWS);
        assert!(state.viewport_offset() <= state.scrollback_len());
    }
});
