#![no_main]

use libfuzzer_sys::fuzz_target;
use terminal_core::{
    MAX_PENDING_REPLIES, MAX_SCROLLBACK_ROWS, TerminalDimensions, TerminalParser, TerminalState,
};

fuzz_target!(|data: &[u8]| {
    if data.len() > 4_096 {
        return;
    }
    let mut state = TerminalState::new(TerminalDimensions::new(20, 8).unwrap());
    let mut parser = TerminalParser::new();
    for chunk in data.chunks(7) {
        // Invalid or unsupported semantic operations may return an error. The state must
        // remain usable after every chunk, including one that reports an error.
        let _ = parser.advance(&mut state, chunk);
        assert!(state.cursor().row() < state.dimensions().rows());
        assert!(state.cursor().column() < state.dimensions().columns());
        assert!(state.scrollback_len() <= MAX_SCROLLBACK_ROWS);
        assert!(state.viewport_offset() <= state.scrollback_len());
        assert!(state.pending_reply_count() <= MAX_PENDING_REPLIES);
        for row in 0..state.dimensions().rows() {
            for column in 0..state.dimensions().columns() {
                assert!(state.viewport_cell(row, column).is_some());
            }
        }
    }
});
