use terminal_core::{TerminalDimensions, TerminalParser, TerminalState};

fn session() -> (TerminalParser, TerminalState) {
    (
        TerminalParser::new(),
        TerminalState::new(TerminalDimensions::new(8, 2).unwrap()),
    )
}

#[test]
fn osc_0_and_2_update_and_clear_the_shell_title() {
    let (mut parser, mut terminal) = session();
    parser
        .advance(&mut terminal, b"\x1b]0;Build; debug\x07")
        .unwrap();
    assert_eq!(terminal.shell_title(), Some("Build; debug"));
    parser
        .advance(&mut terminal, b"\x1b]2;Editor\x1b\\")
        .unwrap();
    assert_eq!(terminal.shell_title(), Some("Editor"));
    parser.advance(&mut terminal, b"\x1b]2;\x07").unwrap();
    assert_eq!(terminal.shell_title(), None);
}

#[test]
fn osc_7_preserves_the_latest_valid_file_uri() {
    let (mut parser, mut terminal) = session();
    parser
        .advance(&mut terminal, b"\x1b]7;file://host/home/user%20name\x07")
        .unwrap();
    assert_eq!(
        terminal.working_directory_uri(),
        Some("file://host/home/user%20name")
    );
    parser
        .advance(&mut terminal, b"\x1b]7;file:///C:/Work%3BNotes\x1b\\")
        .unwrap();
    assert_eq!(
        terminal.working_directory_uri(),
        Some("file:///C:/Work%3BNotes")
    );
}

#[test]
fn invalid_or_unsupported_osc_does_not_replace_reported_metadata() {
    let (mut parser, mut terminal) = session();
    parser
        .advance(&mut terminal, b"\x1b]2;Good\x07\x1b]7;file:///good\x07")
        .unwrap();
    for bytes in [
        b"\x1b]1;Icon\x07".as_slice(),
        b"\x1b]2;\xff\x07",
        b"\x1b]7;https://host/path\x07",
        b"\x1b]7;file://host/invalid%GG\x07",
        b"\x1b]7;file://host/invalid%00\x07",
        b"\x1b]7;file://host/invalid%FF\x07",
        b"\x1b]7;file://host/with space\x07",
        b"\x1b]7;file://host\x07",
        b"\x1b]7;file://bad_host/path\x07",
    ] {
        parser.advance(&mut terminal, bytes).unwrap();
        assert_eq!(terminal.shell_title(), Some("Good"));
        assert_eq!(terminal.working_directory_uri(), Some("file:///good"));
    }
    let oversized = format!("\x1b]2;{}\x07", "x".repeat(513));
    parser.advance(&mut terminal, oversized.as_bytes()).unwrap();
    assert_eq!(terminal.shell_title(), Some("Good"));
}

#[test]
fn fragmented_reports_and_sessions_remain_independent() {
    let (mut first_parser, mut first) = session();
    let (mut second_parser, mut second) = session();
    first_parser.advance(&mut first, b"\x1b]0;Fir").unwrap();
    first_parser
        .advance(&mut first, b"st\x07\x1b]7;file:///first\x07")
        .unwrap();
    second_parser
        .advance(&mut second, b"\x1b]2;Second\x07\x1b]7;file:///second\x07")
        .unwrap();
    assert_eq!(first.shell_title(), Some("First"));
    assert_eq!(first.working_directory_uri(), Some("file:///first"));
    assert_eq!(second.shell_title(), Some("Second"));
    assert_eq!(second.working_directory_uri(), Some("file:///second"));
}
