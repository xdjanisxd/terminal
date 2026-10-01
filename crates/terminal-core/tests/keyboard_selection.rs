use terminal_core::{TerminalDimensions, TerminalParser, TerminalState};

fn state(columns: usize, rows: usize, text: &str) -> TerminalState {
    let mut state = TerminalState::new(TerminalDimensions::new(columns, rows).unwrap());
    TerminalParser::new()
        .advance(&mut state, text.as_bytes())
        .unwrap();
    state
}

#[test]
fn character_selection_contracts_reverses_and_does_not_move_terminal_cursor() {
    let mut terminal = state(10, 2, "abc");
    terminal.set_cursor_position(0, 1).unwrap();
    let cursor = terminal.cursor();
    terminal.extend_keyboard_selection(false, false);
    assert_eq!(terminal.selected_text().as_deref(), Some("a"));
    assert!(terminal.is_selected(0, 0));
    assert!(!terminal.is_selected(0, 1));
    terminal.extend_keyboard_selection(true, false);
    assert_eq!(terminal.selected_text(), None);
    terminal.extend_keyboard_selection(true, false);
    assert_eq!(terminal.selected_text().as_deref(), Some("b"));
    terminal.extend_keyboard_selection(true, false);
    assert_eq!(terminal.selected_text().as_deref(), Some("bc"));
    terminal.extend_keyboard_selection(false, false);
    assert_eq!(terminal.selected_text().as_deref(), Some("b"));
    assert_eq!(terminal.cursor(), cursor);
}

#[test]
fn cursor_inside_wide_cell_anchors_at_whole_character() {
    let mut terminal = state(8, 2, "a界b");
    terminal.set_cursor_position(0, 2).unwrap();
    terminal.extend_keyboard_selection(true, false);
    assert_eq!(terminal.selected_text().as_deref(), Some("界"));
    terminal.extend_keyboard_selection(false, false);
    assert_eq!(terminal.selected_text(), None);
    terminal.extend_keyboard_selection(false, false);
    assert_eq!(terminal.selected_text().as_deref(), Some("a"));
}

#[test]
fn hard_rows_and_soft_wraps_have_distinct_copy_text() {
    for (text, expected) in [("abc\r\ndef", "c\nd"), ("abcdef", "cd")] {
        let mut terminal = state(3, 2, text);
        terminal.set_cursor_position(1, 1).unwrap();
        terminal.extend_keyboard_selection(false, false);
        terminal.extend_keyboard_selection(false, false);
        assert_eq!(terminal.selected_text().as_deref(), Some(expected));
    }
}

#[test]
fn pending_wrap_and_wide_combining_cells_are_atomic() {
    let mut terminal = state(3, 2, "a界\u{301}");
    terminal.extend_keyboard_selection(false, false);
    assert_eq!(terminal.selected_text().as_deref(), Some("界\u{301}"));
    assert!(terminal.is_selected(0, 1));
    assert!(terminal.is_selected(0, 2));
    terminal.extend_keyboard_selection(false, false);
    assert_eq!(terminal.selected_text().as_deref(), Some("a界\u{301}"));
    terminal.extend_keyboard_selection(true, false);
    assert_eq!(terminal.selected_text().as_deref(), Some("界\u{301}"));
    let mut terminal = state(12, 2, "界e\u{301}");
    terminal.extend_keyboard_selection(false, false);
    assert_eq!(terminal.selected_text().as_deref(), Some("e\u{301}"));
    terminal.extend_keyboard_selection(false, false);
    assert_eq!(terminal.selected_text().as_deref(), Some("界e\u{301}"));
}

#[test]
fn words_reuse_mouse_classes_for_punctuation_and_paths() {
    for text in [
        "foo_bar",
        "foo-bar",
        "C:\\Users\\gifted\\project",
        "/usr/local/bin",
        "hello.world",
    ] {
        let mut terminal = state(64, 2, text);
        terminal.extend_keyboard_selection(false, true);
        assert_eq!(terminal.selected_text().as_deref(), Some(text));
        terminal.extend_keyboard_selection(true, true);
        assert_eq!(terminal.selected_text(), None);
        terminal.select_word(0, 0);
        assert_eq!(terminal.selected_text().as_deref(), Some(text));
    }
    let mut terminal = state(32, 2, "--flag=value");
    for expected in ["value", "=value", "--flag=value"] {
        terminal.extend_keyboard_selection(false, true);
        assert_eq!(terminal.selected_text().as_deref(), Some(expected));
    }
    let mut terminal = state(16, 2, "foo bar");
    terminal.extend_keyboard_selection(false, true);
    assert_eq!(terminal.selected_text().as_deref(), Some("bar"));
    terminal.extend_keyboard_selection(false, true);
    assert_eq!(terminal.selected_text().as_deref(), Some("foo bar"));
}

#[test]
fn word_selection_crosses_soft_wrap_and_scrollback_keeps_absolute_endpoints() {
    let mut terminal = state(3, 2, "abcdefg");
    assert_eq!(terminal.scrollback_len(), 1);
    terminal.page_up();
    terminal.extend_keyboard_selection(false, true);
    assert_eq!(terminal.selected_text().as_deref(), Some("abcdefg"));
    terminal.return_to_live_viewport();
    assert_eq!(terminal.selected_text().as_deref(), Some("abcdefg"));
    terminal.begin_selection(0, 0);
    terminal.extend_selection(1, 0);
    assert_eq!(terminal.selected_text().as_deref(), Some("def\ng"));
}

#[test]
fn mouse_and_keyboard_share_one_selection_and_alternate_is_rejected() {
    let mut terminal = state(12, 2, "foo bar");
    terminal.select_word(0, 0);
    terminal.extend_keyboard_selection(true, false);
    assert_eq!(terminal.selected_text().as_deref(), Some("foo "));
    terminal.begin_selection(0, 4);
    terminal.extend_selection(0, 6);
    assert_eq!(terminal.selected_text().as_deref(), Some("bar"));
    terminal.switch_to_alternate_screen();
    assert!(!terminal.extend_keyboard_selection(false, false));
    assert_eq!(terminal.selected_text(), None);
}

#[test]
fn forward_words_and_wide_pre_wrap_padding_copy_exactly() {
    let mut terminal = state(16, 2, "foo bar");
    terminal.set_cursor_position(0, 0).unwrap();
    terminal.extend_keyboard_selection(true, true);
    assert_eq!(terminal.selected_text().as_deref(), Some("foo"));
    terminal.extend_keyboard_selection(true, true);
    assert_eq!(terminal.selected_text().as_deref(), Some("foo bar"));
    let mut terminal = state(3, 2, "ab界\u{301}");
    terminal.extend_keyboard_selection(false, false);
    terminal.extend_keyboard_selection(false, false);
    assert_eq!(terminal.selected_text().as_deref(), Some("b界\u{301}"));
    terminal.extend_keyboard_selection(true, false);
    terminal.extend_keyboard_selection(true, false);
    assert_eq!(terminal.selected_text(), None);
    let mut terminal = state(3, 2, "abc");
    terminal.extend_keyboard_selection(false, false);
    terminal.extend_keyboard_selection(true, false);
    assert_eq!(terminal.selected_text(), None);
}

#[test]
fn reporting_requests_release_shortcut_ownership_and_restore_nested_flags() {
    let mut terminal = state(8, 2, "");
    let mut parser = TerminalParser::new();
    for (bytes, enabled) in [
        ("\x1b[>1u", true),
        ("\x1b[>0u", false),
        ("\x1b[<u", true),
        ("\x1b[<u", false),
        ("\x1b[=1u", true),
        ("\x1b[=2;2u", true),
        ("\x1b[=1;3u", true),
        ("\x1b[=2;3u", false),
        ("\x1b[>4;2m", true),
        ("\x1b[>4;0m", false),
    ] {
        parser.advance(&mut terminal, bytes.as_bytes()).unwrap();
        assert_eq!(
            terminal.input_modes().keyboard_reporting_requested(),
            enabled
        );
    }
}

#[test]
fn reporting_ownership_is_screen_local() {
    let mut terminal = state(8, 2, "");
    let mut parser = TerminalParser::new();
    parser
        .advance(&mut terminal, b"\x1b[>1u\x1b[?1049h")
        .unwrap();
    assert!(!terminal.input_modes().keyboard_reporting_requested());
    parser
        .advance(&mut terminal, b"\x1b[>2u\x1b[?1049l")
        .unwrap();
    assert!(terminal.input_modes().keyboard_reporting_requested());
    parser.advance(&mut terminal, b"\x1b[<u").unwrap();
    assert!(!terminal.input_modes().keyboard_reporting_requested());
    parser.advance(&mut terminal, b"\x1b[?47h").unwrap();
    assert!(terminal.input_modes().keyboard_reporting_requested());
    parser.advance(&mut terminal, b"\x1b[?47l").unwrap();
    assert!(!terminal.input_modes().keyboard_reporting_requested());
}
