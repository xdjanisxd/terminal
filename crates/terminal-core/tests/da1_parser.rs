use terminal_core::{
    MAX_PENDING_REPLIES, TerminalDimensions, TerminalParser, TerminalReply, TerminalState,
};

const EXPECTED_DA1: &[u8] = b"\x1b[?1;0c";

fn state() -> TerminalState {
    TerminalState::new(TerminalDimensions::new(4, 3).unwrap())
}

fn reply_bytes(reply: TerminalReply) -> Vec<u8> {
    reply.as_bytes().as_slice().to_vec()
}

fn row_text(state: &TerminalState, row: usize) -> String {
    (0..state.dimensions().columns())
        .map(|column| state.screen().cell(row, column).unwrap().character())
        .collect()
}

#[test]
fn parser_csi_c_and_csi_zero_c_produce_exact_da1_reply() {
    for sequence in [b"\x1b[c".as_slice(), b"\x1b[0c"] {
        let mut parser = TerminalParser::new();
        let mut terminal = state();

        parser.advance(&mut terminal, sequence).unwrap();

        assert_eq!(terminal.pending_reply_count(), 1, "sequence {sequence:?}");
        assert_eq!(
            terminal.take_reply().map(reply_bytes),
            Some(EXPECTED_DA1.to_vec()),
            "sequence {sequence:?}"
        );
        assert_eq!(terminal.take_reply(), None);
    }
}

#[test]
fn da1_queries_are_chunk_safe_at_every_input_boundary() {
    for input in [b"\x1b[c".as_slice(), b"\x1b[0c"] {
        for split in 0..=input.len() {
            let mut parser = TerminalParser::new();
            let mut terminal = state();
            parser.advance(&mut terminal, &input[..split]).unwrap();
            parser.advance(&mut terminal, &input[split..]).unwrap();

            assert_eq!(terminal.pending_reply_count(), 1, "split {split}");
            assert_eq!(
                terminal.take_reply().map(reply_bytes),
                Some(EXPECTED_DA1.to_vec())
            );
        }
    }
}

#[test]
fn printable_text_neighbors_da1_without_becoming_reply_input() {
    let mut parser = TerminalParser::new();
    let mut terminal = state();

    parser.advance(&mut terminal, b"A\x1b[cB").unwrap();

    assert_eq!(row_text(&terminal, 0), "AB  ");
    assert_eq!(
        (terminal.cursor().row(), terminal.cursor().column()),
        (0, 2)
    );
    assert_eq!(
        terminal.take_reply().map(reply_bytes),
        Some(EXPECTED_DA1.to_vec())
    );
    assert_eq!(terminal.take_reply(), None);
}

#[test]
fn multiple_queries_in_one_stream_and_separate_calls_are_preserved() {
    let mut parser = TerminalParser::new();
    let mut terminal = state();

    parser.advance(&mut terminal, b"\x1b[c\x1b[0c").unwrap();
    parser.advance(&mut terminal, b"\x1b[c").unwrap();

    assert_eq!(terminal.pending_reply_count(), 3);
    for _ in 0..3 {
        assert_eq!(
            terminal.take_reply().map(reply_bytes),
            Some(EXPECTED_DA1.to_vec())
        );
    }
    assert_eq!(terminal.take_reply(), None);
}

#[test]
fn incomplete_malformed_private_and_non_primary_da_forms_do_not_reply() {
    let mut parser = TerminalParser::new();
    let mut incomplete = state();
    parser.advance(&mut incomplete, b"\x1b[").unwrap();
    assert_eq!(incomplete.pending_reply_count(), 0);
    parser.advance(&mut incomplete, b"c").unwrap();
    assert_eq!(
        incomplete.take_reply().map(reply_bytes),
        Some(EXPECTED_DA1.to_vec())
    );

    for sequence in [
        b"\x1b[1c".as_slice(),
        b"\x1b[2c",
        b"\x1b[0;0c",
        b"\x1b[0:1c",
        b"\x1b[?c",
        b"\x1b[?0c",
        b"\x1b[>c",
        b"\x1b[>0c",
        b"\x1b[=c",
        b"\x1b[=0c",
    ] {
        let mut parser = TerminalParser::new();
        let mut terminal = state();
        parser.advance(&mut terminal, sequence).unwrap();
        assert_eq!(terminal.pending_reply_count(), 0, "sequence {sequence:?}");
    }
}

#[test]
fn parser_da1_at_capacity_is_bounded_and_does_not_overwrite_pending_replies() {
    let mut parser = TerminalParser::new();
    let mut terminal = state();
    for _ in 0..MAX_PENDING_REPLIES {
        assert!(terminal.request_primary_device_attributes());
    }

    parser.advance(&mut terminal, b"\x1b[c").unwrap();

    assert_eq!(terminal.pending_reply_count(), MAX_PENDING_REPLIES);
    for _ in 0..MAX_PENDING_REPLIES {
        assert_eq!(
            terminal.take_reply().map(reply_bytes),
            Some(EXPECTED_DA1.to_vec())
        );
    }
    assert_eq!(terminal.take_reply(), None);
}
