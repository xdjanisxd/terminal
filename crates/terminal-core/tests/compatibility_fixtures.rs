use terminal_core::{
    CellColor, CursorKeyMode, CursorVisibility, InverseVideo, ScreenKind, TerminalDimensions,
    TerminalParser, TerminalReply, TerminalState, VerticalScrollingMargins,
};

fn row_text(state: &TerminalState, row: usize) -> String {
    (0..state.dimensions().columns())
        .map(|column| state.screen().cell(row, column).unwrap().character())
        .collect()
}

fn assert_row(state: &TerminalState, row: usize, expected: &str) {
    assert_eq!(
        row_text(state, row),
        format!("{expected:<width$}", width = state.dimensions().columns()),
        "row {row}"
    );
}

#[test]
fn shell_prompt_redraw_fixture_preserves_supported_output_and_ignores_deferred_title() {
    // Representative prompt redraw: OSC title setting is intentionally unsupported, while
    // SGR, CR, EL, LF, and cursor visibility are all part of the current compatibility slice.
    let fixture = b"\x1b]0;dev@box\x07\x1b[?25l\x1b[1;32mdev@box\x1b[0m:\x1b[34m~\x1b[0m$ echo hi\r\x1b[2K\x1b[1;32mdev@box\x1b[0m:\x1b[34m~\x1b[0m$ echo hi\r\nhi\r\n\x1b[1;32mdev@box\x1b[0m:\x1b[34m~\x1b[0m$ \x1b[?25h";
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(24, 5).unwrap());

    for chunk in fixture.chunks(7) {
        parser.advance(&mut state, chunk).unwrap();
    }

    assert_row(&state, 0, "dev@box:~$ echo hi");
    assert_row(&state, 1, "hi");
    assert_row(&state, 2, "dev@box:~$ ");
    assert_eq!(
        state.screen().cell(0, 0).unwrap().attributes().foreground(),
        CellColor::Indexed(2)
    );
    assert_eq!(
        state.screen().cell(0, 8).unwrap().attributes().foreground(),
        CellColor::Indexed(4)
    );
    assert_eq!(
        state.terminal_modes().cursor_visibility(),
        CursorVisibility::Visible
    );
    assert_eq!(state.pending_reply_count(), 0);
}

#[test]
fn fullscreen_editor_fixture_uses_alternate_screen_and_restores_the_prompt() {
    // Representative full-screen editor: alternate screen, clear/home, truecolor status line,
    // CUP, inverse cursor cell, and cursor visibility are all already supported.
    let fixture = b"\x1b[?1049h\x1b[?25l\x1b[2J\x1b[H\x1b[48;2;20;20;20m\x1b[38;2;200;200;200m file.txt \x1b[0m\r\nline one\r\nline two\x1b[3;5H\x1b[7mX\x1b[0m";
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(12, 4).unwrap());
    parser.advance(&mut state, b"shell$ ").unwrap();

    for chunk in fixture.chunks(5) {
        parser.advance(&mut state, chunk).unwrap();
    }

    assert_eq!(state.active_screen(), ScreenKind::Alternate);
    assert_row(&state, 0, " file.txt ");
    assert_row(&state, 1, "line one");
    assert_row(&state, 2, "lineXtwo");
    assert_eq!(
        state.screen().cell(0, 0).unwrap().attributes().background(),
        CellColor::Rgb {
            red: 20,
            green: 20,
            blue: 20,
        }
    );
    assert_eq!(
        state.screen().cell(2, 4).unwrap().attributes().inverse(),
        InverseVideo::Enabled
    );
    assert_eq!(
        state.terminal_modes().cursor_visibility(),
        CursorVisibility::Hidden
    );

    parser.advance(&mut state, b"\x1b[?25h\x1b[?1049l").unwrap();
    assert_eq!(state.active_screen(), ScreenKind::Primary);
    assert_row(&state, 0, "shell$ ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 7));
    assert_eq!(
        state.terminal_modes().cursor_visibility(),
        CursorVisibility::Visible
    );
}

#[test]
fn multiplexer_fixture_preserves_region_output_modes_and_reply_fifo() {
    // Representative multiplexer control traffic: DECCKM, cursor visibility, DECSTBM, CUP,
    // DA1, ANSI DSR status, and CPR are all part of the current compatibility slice.
    let fixture =
        b"\x1b[?1h\x1b[?25l\x1b[2;5r\x1b[4;1Hpane-1\r\npane-2\x1b[?1l\x1b[?25h\x1b[c\x1b[5n\x1b[6n";
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(12, 6).unwrap());

    for chunk in fixture.chunks(4) {
        parser.advance(&mut state, chunk).unwrap();
    }

    assert_row(&state, 3, "pane-1");
    assert_row(&state, 4, "pane-2");
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::new(1, 4, 6).unwrap()
    );
    assert_eq!(state.input_modes().cursor_keys(), CursorKeyMode::Normal);
    assert_eq!(
        state.terminal_modes().cursor_visibility(),
        CursorVisibility::Visible
    );
    assert_eq!(
        state.take_reply(),
        Some(TerminalReply::PrimaryDeviceAttributes)
    );
    assert_eq!(state.take_reply(), Some(TerminalReply::TerminalStatus));
    assert_eq!(
        state.take_reply(),
        Some(TerminalReply::CursorPosition { row: 5, column: 7 })
    );
    assert_eq!(state.take_reply(), None);
}

#[test]
fn shell_edit_fixture_handles_carriage_return_backspace_and_erase() {
    // A shell edits an input line, then replaces it with a completed command and output.
    let fixture = b"PS C:\\work> git statu\x08s\r\x1b[2KPS C:\\work> git status\r\nOn branch main\r\nPS C:\\work> ";
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(32, 4).unwrap());

    for byte in fixture {
        parser
            .advance(&mut state, std::slice::from_ref(byte))
            .unwrap();
    }

    assert_row(&state, 0, "PS C:\\work> git status");
    assert_row(&state, 1, "On branch main");
    assert_row(&state, 2, "PS C:\\work> ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (2, 12));
}

#[test]
fn tui_progress_fixture_updates_rows_without_damaging_neighbors() {
    // A progress TUI updates two status rows by cursor addressing and line erasure.
    let fixture = b"\x1b[?25l\x1b[Hheader\x1b[2;1Hdownload 10%\x1b[3;1Hwaiting\x1b[2;1H\x1b[2Kdownload 100%\x1b[3;1H\x1b[2Kcomplete\x1b[?25h";
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(20, 4).unwrap());

    for chunk in fixture.chunks(3) {
        parser.advance(&mut state, chunk).unwrap();
    }

    assert_row(&state, 0, "header");
    assert_row(&state, 1, "download 100%");
    assert_row(&state, 2, "complete");
    assert_eq!(
        state.terminal_modes().cursor_visibility(),
        CursorVisibility::Visible
    );
}
