use terminal_core::{MAX_PENDING_REPLIES, TerminalDimensions, TerminalParser, TerminalState};

fn reply_bytes(state: &mut TerminalState) -> Vec<u8> {
    state.take_reply().unwrap().as_bytes().as_slice().to_vec()
}

#[test]
fn alternate_scroll_set_reset_and_status_query_match_current_mode() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(8, 2).unwrap());

    parser.advance(&mut state, b"\x1b[?1007$p").unwrap();
    assert_eq!(reply_bytes(&mut state), b"\x1b[?1007;2$y");
    assert!(!state.alternate_scroll());

    parser
        .advance(&mut state, b"\x1b[?1007h\x1b[?1007$p")
        .unwrap();
    assert!(state.alternate_scroll());
    assert_eq!(reply_bytes(&mut state), b"\x1b[?1007;1$y");

    parser
        .advance(&mut state, b"\x1b[?1007l\x1b[?1007$p")
        .unwrap();
    assert!(!state.alternate_scroll());
    assert_eq!(reply_bytes(&mut state), b"\x1b[?1007;2$y");
}

#[test]
fn alternate_scroll_is_reset_by_decstr_and_terminal_reset() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(8, 2).unwrap());
    parser.advance(&mut state, b"\x1b[?1007h\x1b[!p").unwrap();
    assert!(!state.alternate_scroll());

    parser.advance(&mut state, b"\x1b[?1007h\x1bc").unwrap();
    assert!(!state.alternate_scroll());
}

#[test]
fn alternate_scroll_status_uses_the_existing_bounded_reply_queue() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(8, 2).unwrap());
    for _ in 0..MAX_PENDING_REPLIES {
        assert!(state.request_alternate_scroll_status());
    }
    parser.advance(&mut state, b"\x1b[?1007$p").unwrap();
    assert_eq!(state.pending_reply_count(), MAX_PENDING_REPLIES);
    for _ in 0..MAX_PENDING_REPLIES {
        assert_eq!(reply_bytes(&mut state), b"\x1b[?1007;2$y");
    }
    assert!(state.take_reply().is_none());
}
