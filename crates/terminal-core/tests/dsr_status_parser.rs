use terminal_core::{TerminalDimensions, TerminalParser, TerminalReply, TerminalState};

const STATUS: &[u8] = b"\x1b[0n";

fn state() -> TerminalState {
    TerminalState::new(TerminalDimensions::new(12, 12).unwrap())
}

fn bytes(reply: TerminalReply) -> Vec<u8> {
    reply.as_bytes().as_slice().to_vec()
}

#[test]
fn parser_dispatches_status_at_every_chunk_boundary() {
    let input = b"before\x1b[5nafter";
    for split in 0..=input.len() {
        let mut parser = TerminalParser::new();
        let mut terminal = state();
        parser.advance(&mut terminal, &input[..split]).unwrap();
        parser.advance(&mut terminal, &input[split..]).unwrap();
        assert_eq!(terminal.take_reply().map(bytes).as_deref(), Some(STATUS));
        assert_eq!(terminal.take_reply(), None);
    }
}

#[test]
fn status_preserves_order_with_cpr_and_da1() {
    let mut parser = TerminalParser::new();
    let mut terminal = state();
    terminal.set_cursor_position(4, 9).unwrap();
    parser
        .advance(&mut terminal, b"\x1b[5n\x1b[6n\x1b[c\x1b[6n\x1b[5n")
        .unwrap();
    for expected in [
        STATUS,
        b"\x1b[5;10R".as_slice(),
        b"\x1b[?1;0c",
        b"\x1b[5;10R",
        STATUS,
    ] {
        assert_eq!(terminal.take_reply().map(bytes).as_deref(), Some(expected));
    }
}

#[test]
fn unsupported_dsr_forms_remain_noops() {
    let mut parser = TerminalParser::new();
    let mut terminal = state();
    parser.advance(&mut terminal, b"\x1b[").unwrap();
    assert_eq!(terminal.take_reply(), None);
    for sequence in [
        b"\x1b[0n".as_slice(),
        b"\x1b[1n",
        b"\x1b[4n",
        b"\x1b[7n",
        b"\x1b[?5n",
        b"\x1b[>5n",
        b"\x1b[5:0n",
        b"\x1b[5;0n",
    ] {
        parser.advance(&mut terminal, sequence).unwrap();
        assert_eq!(terminal.take_reply(), None, "sequence {sequence:?}");
    }
}
