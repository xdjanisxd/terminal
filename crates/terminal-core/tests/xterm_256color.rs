use terminal_core::{CellColor, ScreenKind, TerminalDimensions, TerminalParser, TerminalState};

#[test]
fn xterm_256color_clear_colors_and_screen_round_trip() {
    let mut terminal = TerminalState::new(TerminalDimensions::new(80, 24).unwrap());
    let mut parser = TerminalParser::new();
    parser.advance(&mut terminal, b"primary").unwrap();
    // Relevant capabilities from the native xterm-256color terminfo entry:
    // clear, setaf/setab (256 colors), smcup/rmcup, smkx/rmkx.
    parser
        .advance(&mut terminal, b"\x1b[?1049h\x1b[22;0;0t\x1b[?1h\x1b=")
        .unwrap();
    assert_eq!(terminal.active_screen(), ScreenKind::Alternate);
    parser
        .advance(&mut terminal, b"old contents\x1b[H\x1b[2J\x1b[3J")
        .unwrap();
    assert_eq!(terminal.screen().cell(0, 0).unwrap().character(), ' ');
    parser
        .advance(&mut terminal, b"\x1b[38;5;196m\x1b[48;5;22mA")
        .unwrap();
    let attributes = terminal.screen().cell(0, 0).unwrap().attributes();
    assert_eq!(attributes.foreground(), CellColor::Indexed(196));
    assert_eq!(attributes.background(), CellColor::Indexed(22));
    parser
        .advance(&mut terminal, b"\x1b[?1l\x1b>\x1b[?1049l\x1b[23;0;0t")
        .unwrap();
    assert_eq!(terminal.active_screen(), ScreenKind::Primary);
    assert_eq!(terminal.screen().cell(0, 0).unwrap().character(), 'p');
}
