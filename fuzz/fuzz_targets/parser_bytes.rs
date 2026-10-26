#![no_main]

use libfuzzer_sys::fuzz_target;
use terminal_core::{
    MAX_PENDING_REPLIES, MAX_SCROLLBACK_ROWS, TerminalDimensions, TerminalParser, TerminalState,
};

fn check_state(state: &TerminalState) {
    assert!(state.cursor().row() < state.dimensions().rows());
    assert!(state.cursor().column() < state.dimensions().columns());
    assert!(state.scrollback_len() <= MAX_SCROLLBACK_ROWS);
    assert!(state.viewport_offset() <= state.scrollback_len());
    assert!(state.pending_reply_count() <= MAX_PENDING_REPLIES);
}

fn replay(data: &[u8], chunk_size: usize) {
    let mut state = TerminalState::new(TerminalDimensions::new(20, 8).unwrap());
    let mut parser = TerminalParser::new();
    for chunk in data.chunks(chunk_size) {
        // Invalid or unsupported semantic operations may return an error. The state must
        // remain usable after every chunk, including one that reports an error.
        let _ = parser.advance(&mut state, chunk);
        check_state(&state);
    }
    for row in 0..state.dimensions().rows() {
        for column in 0..state.dimensions().columns() {
            assert!(state.viewport_cell(row, column).is_some());
        }
    }
}

fn replay_variable_chunks(data: &[u8]) {
    let mut state = TerminalState::new(TerminalDimensions::new(20, 8).unwrap());
    let mut parser = TerminalParser::new();
    let mut offset = 0;
    while offset < data.len() {
        let chunk_size = usize::from(data[offset] % 31) + 1;
        let end = (offset + chunk_size).min(data.len());
        let _ = parser.advance(&mut state, &data[offset..end]);
        check_state(&state);
        offset = end;
    }
    for row in 0..state.dimensions().rows() {
        for column in 0..state.dimensions().columns() {
            assert!(state.viewport_cell(row, column).is_some());
        }
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() > 4_096 {
        return;
    }
    // Replay at fixed and data-derived boundaries so parser state crosses arbitrary calls.
    for chunk_size in [1, 2, 7, 32, data.first().map_or(1, |byte| usize::from(byte % 31) + 1)] {
        replay(data, chunk_size);
    }
    replay_variable_chunks(data);
});
