use terminal_core::{CellColor, TerminalDimensions, TerminalParser, TerminalState, TextIntensity};

#[test]
fn ncurses_bold_default_foreground_is_chunk_safe_and_does_not_select_a_color() {
    // Captured ncurses rendition pattern: charset selection, bold on, then sgr0.
    // SGR 1 selects intensity; it does not request the theme's cyan or blue.
    let input = b"\x1b[39;49mKeys:  \x1b(B\x1b[0;1mH\x1b(B\x1b[melp";
    for split in 0..=input.len() {
        let mut state = TerminalState::new(TerminalDimensions::new(20, 2).unwrap());
        let mut parser = TerminalParser::new();
        parser.advance(&mut state, &input[..split]).unwrap();
        parser.advance(&mut state, &input[split..]).unwrap();
        for column in 0..11 {
            let cell = state.screen().cell(0, column).unwrap();
            assert_eq!(cell.attributes().foreground(), CellColor::Default);
            assert_eq!(cell.attributes().background(), CellColor::Default);
            assert_eq!(
                cell.attributes().intensity(),
                if column == 7 {
                    TextIntensity::Bold
                } else {
                    TextIntensity::Normal
                },
                "split {split}, column {column}"
            );
        }
        assert_eq!(state.screen().cell(0, 7).unwrap().character(), 'H');
        assert_eq!(state.pending_reply_count(), 0);
    }
}

#[test]
fn ncurses_color_pair_and_bold_reset_remain_independent() {
    let mut state = TerminalState::new(TerminalDimensions::new(10, 2).unwrap());
    let mut parser = TerminalParser::new();
    parser
        .advance(&mut state, b"\x1b[31m\x1b[1mA\x1b[22mB\x1b[39mC")
        .unwrap();
    for (column, color, intensity) in [
        (0, CellColor::Indexed(1), TextIntensity::Bold),
        (1, CellColor::Indexed(1), TextIntensity::Normal),
        (2, CellColor::Default, TextIntensity::Normal),
    ] {
        let attrs = state.screen().cell(0, column).unwrap().attributes();
        assert_eq!(attrs.foreground(), color);
        assert_eq!(attrs.intensity(), intensity);
    }
}
