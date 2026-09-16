use terminal_core::{TerminalDimensions, TerminalParser, TerminalReply, TerminalState};

fn state() -> TerminalState {
    TerminalState::new(TerminalDimensions::new(12, 12).unwrap())
}

fn reply_bytes(reply: TerminalReply) -> Vec<u8> {
    reply.as_bytes().as_slice().to_vec()
}

#[test]
fn cpr_is_chunk_safe_at_every_input_boundary_and_across_calls() {
    let input = b"\x1b[6n";
    for split in 0..=input.len() {
        let mut parser = TerminalParser::new();
        let mut terminal = state();
        terminal.set_cursor_position(4, 9).unwrap();

        parser.advance(&mut terminal, &input[..split]).unwrap();
        parser.advance(&mut terminal, &input[split..]).unwrap();

        assert_eq!(
            terminal.take_reply().map(reply_bytes),
            Some(b"\x1b[5;10R".to_vec()),
            "split {split}"
        );
        assert_eq!(terminal.take_reply(), None);
    }

    let mut parser = TerminalParser::new();
    let mut terminal = state();
    parser.advance(&mut terminal, b"\x1b[6n").unwrap();
    terminal.set_cursor_position(10, 11).unwrap();
    parser.advance(&mut terminal, b"\x1b[6n").unwrap();
    assert_eq!(
        terminal.take_reply().map(reply_bytes),
        Some(b"\x1b[1;1R".to_vec())
    );
    assert_eq!(
        terminal.take_reply().map(reply_bytes),
        Some(b"\x1b[11;12R".to_vec())
    );
}

#[test]
fn multiple_and_interleaved_da1_and_cpr_queries_keep_exact_fifo_order() {
    let mut parser = TerminalParser::new();
    let mut terminal = state();
    terminal.set_cursor_position(4, 9).unwrap();

    parser
        .advance(&mut terminal, b"\x1b[c\x1b[6n\x1b[0c\x1b[6n")
        .unwrap();

    assert_eq!(
        terminal.take_reply().map(reply_bytes),
        Some(b"\x1b[?1;0c".to_vec())
    );
    assert_eq!(
        terminal.take_reply().map(reply_bytes),
        Some(b"\x1b[5;10R".to_vec())
    );
    assert_eq!(
        terminal.take_reply().map(reply_bytes),
        Some(b"\x1b[?1;0c".to_vec())
    );
    assert_eq!(
        terminal.take_reply().map(reply_bytes),
        Some(b"\x1b[5;10R".to_vec())
    );
    assert_eq!(terminal.take_reply(), None);
}

#[test]
fn incomplete_or_malformed_dsr_does_not_reply_before_a_complete_valid_query() {
    let mut parser = TerminalParser::new();
    let mut terminal = state();

    parser.advance(&mut terminal, b"\x1b[").unwrap();
    assert_eq!(terminal.take_reply(), None);
    parser.advance(&mut terminal, b"6n").unwrap();
    assert_eq!(
        terminal.take_reply().map(reply_bytes),
        Some(b"\x1b[1;1R".to_vec())
    );

    for sequence in [b"\x1b[6:0n".as_slice(), b"\x1b[6;0n", b"\x1b[?6n"] {
        parser.advance(&mut terminal, sequence).unwrap();
        assert_eq!(terminal.take_reply(), None, "sequence {sequence:?}");
    }
}
