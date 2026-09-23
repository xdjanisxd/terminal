use base64::Engine;
use terminal_core::{Osc52Policy, TerminalDimensions, TerminalParser, TerminalState};

fn state() -> TerminalState {
    TerminalState::new(TerminalDimensions::new(8, 2).unwrap())
}

#[test]
fn osc52_is_denied_by_default_and_never_answers_queries() {
    let mut parser = TerminalParser::new();
    let mut terminal = state();
    parser
        .advance(&mut terminal, b"\x1b]52;c;aGVsbG8=\x07")
        .unwrap();
    assert_eq!(terminal.take_osc52_write(), None);

    let mut parser = TerminalParser::with_osc52_policy(Osc52Policy::AllowWrite);
    parser.advance(&mut terminal, b"\x1b]52;c;?\x07").unwrap();
    assert_eq!(terminal.take_osc52_write(), None);
    assert_eq!(terminal.pending_reply_count(), 0);
}

#[test]
fn osc52_allow_write_accepts_only_bounded_valid_clipboard_text() {
    let mut parser = TerminalParser::with_osc52_policy(Osc52Policy::AllowWrite);
    let mut terminal = state();
    parser.advance(&mut terminal, b"\x1b]52;c;aGVs").unwrap();
    assert_eq!(terminal.take_osc52_write(), None);
    parser.advance(&mut terminal, b"bG8=\x1b\\").unwrap();
    assert_eq!(terminal.take_osc52_write().as_deref(), Some("hello"));

    for sequence in [
        b"\x1b]52;p;aGVsbG8=\x07".as_slice(),
        b"\x1b]52;c;@@@\x07",
        b"\x1b]52;c;AA==\x07",
        b"\x1b]52;c;//8=\x07",
    ] {
        parser.advance(&mut terminal, sequence).unwrap();
        assert_eq!(terminal.take_osc52_write(), None);
    }
    let boundary = base64::engine::general_purpose::STANDARD.encode(vec![b'a'; 512]);
    let accepted = format!("\x1b]52;c;{boundary}\x07");
    parser.advance(&mut terminal, accepted.as_bytes()).unwrap();
    assert_eq!(terminal.take_osc52_write(), Some("a".repeat(512)));

    let oversized = format!(
        "\x1b]52;c;{}\x07",
        base64::engine::general_purpose::STANDARD.encode(vec![b'a'; 513])
    );
    parser.advance(&mut terminal, oversized.as_bytes()).unwrap();
    assert_eq!(terminal.take_osc52_write(), None);
}
