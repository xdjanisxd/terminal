use terminal_core::{
    MAX_PENDING_REPLIES, MAX_SCROLLBACK_ROWS, TerminalDimensions, TerminalParser, TerminalState,
};

fn assert_bounds(state: &TerminalState) {
    assert!(state.cursor().row() < state.dimensions().rows());
    assert!(state.cursor().column() < state.dimensions().columns());
    assert!(state.scrollback_len() <= MAX_SCROLLBACK_ROWS);
    assert!(state.viewport_offset() <= state.scrollback_len());
    assert!(state.pending_reply_count() <= MAX_PENDING_REPLIES);
}

fn next_byte(seed: &mut u32) -> u8 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    (*seed >> 24) as u8
}

#[test]
fn parser_handles_deterministic_byte_streams_at_multiple_chunk_boundaries() {
    for case in 0..32u32 {
        let mut seed = case + 1;
        let bytes: Vec<u8> = (0..256).map(|_| next_byte(&mut seed)).collect();
        for chunk_size in [1, 7, 64] {
            let mut parser = TerminalParser::new();
            let mut state = TerminalState::new(TerminalDimensions::new(20, 8).unwrap());
            for chunk in bytes.chunks(chunk_size) {
                let _ = parser.advance(&mut state, chunk);
                assert_bounds(&state);
                for row in 0..state.dimensions().rows() {
                    for column in 0..state.dimensions().columns() {
                        assert!(state.viewport_cell(row, column).is_some());
                    }
                }
            }
        }
    }
}

#[test]
fn state_navigation_selection_and_resize_handle_deterministic_actions() {
    for case in 0..32u32 {
        let mut seed = case + 1;
        let mut state = TerminalState::new(TerminalDimensions::new(12, 5).unwrap());
        for _ in 0..256 {
            let action = next_byte(&mut seed) % 7;
            let row = usize::from(next_byte(&mut seed)) % state.dimensions().rows();
            let column = usize::from(next_byte(&mut seed)) % state.dimensions().columns();
            match action {
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
                _ => state.resize(TerminalDimensions::new(column + 1, row + 1).unwrap()),
            }
            assert_bounds(&state);
        }
    }
}
