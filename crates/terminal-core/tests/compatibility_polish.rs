use terminal_core::{
    AutoWrapMode, CellAttributes, CellColor, CharacterInsertionMode, CursorKeyMode, CursorMovement,
    CursorVisibility, ScreenKind, TerminalDimensions, TerminalParser, TerminalReply, TerminalState,
    TextIntensity, UnderlineStyle,
};

fn state() -> TerminalState {
    TerminalState::new(TerminalDimensions::new(12, 6).unwrap())
}

fn feed(state: &mut TerminalState, bytes: &[u8]) {
    TerminalParser::new().advance(state, bytes).unwrap();
}

fn cursor(state: &TerminalState) -> (usize, usize) {
    (state.cursor().row(), state.cursor().column())
}

#[test]
fn colon_colors_and_attributes_are_ordered_and_chunk_safe() {
    let bytes = b"\x1b[1;38:2::255:20:30;48:5:200;3;4:1;8;9mA\x1b[22;23;4:0;28;29mB\x1b[0mC";
    for split in 0..=bytes.len() {
        let mut state = state();
        let mut parser = TerminalParser::new();
        parser.advance(&mut state, &bytes[..split]).unwrap();
        parser.advance(&mut state, &bytes[split..]).unwrap();
        let a = state.screen().cell(0, 0).unwrap().attributes();
        assert_eq!(
            a.foreground(),
            CellColor::Rgb {
                red: 255,
                green: 20,
                blue: 30
            }
        );
        assert_eq!(a.background(), CellColor::Indexed(200));
        assert_eq!(a.intensity(), TextIntensity::Bold);
        assert_eq!(a.underline(), UnderlineStyle::Enabled);
        assert!(a.concealed() && a.strikethrough());
        let b = state.screen().cell(0, 1).unwrap().attributes();
        assert!(!b.concealed() && !b.strikethrough());
        assert_eq!(b.underline(), UnderlineStyle::Disabled);
        assert_eq!(b.intensity(), TextIntensity::Normal);
        assert_eq!(
            state.screen().cell(0, 2).unwrap().attributes(),
            &CellAttributes::default()
        );
    }
    for color in [b"\x1b[38:2:1:2:3m".as_slice(), b"\x1b[38:2:0:1:2:3m"] {
        let mut state = state();
        feed(&mut state, color);
        assert_eq!(
            state.current_rendition().foreground(),
            CellColor::Rgb {
                red: 1,
                green: 2,
                blue: 3
            }
        );
    }
}

#[test]
fn invalid_colon_groups_are_atomic_and_do_not_consume_following_attributes() {
    for group in [
        "38:2:256:1:9",
        "48:2:1:256:9",
        "38:2:1:2",
        "38:2:1:2:3:4",
        "48:5:256",
        "38:7:1:9",
        "4:3",
    ] {
        let mut state = state();
        feed(&mut state, format!("\x1b[32;44;{group};3m").as_bytes());
        assert_eq!(
            state.current_rendition().foreground(),
            CellColor::Indexed(2),
            "{group}"
        );
        assert_eq!(
            state.current_rendition().background(),
            CellColor::Indexed(4),
            "{group}"
        );
        assert_eq!(state.current_rendition().intensity(), TextIntensity::Normal);
        assert!(!state.current_rendition().strikethrough());
        assert_eq!(
            state.current_rendition().italic(),
            terminal_core::ItalicStyle::Italic
        );
    }
}

#[test]
fn decom_homes_addresses_clamps_and_reports_relative_rows() {
    let mut state = state();
    feed(&mut state, b"\x1b[2;5r\x1b[?6h");
    assert!(state.terminal_modes().origin());
    assert_eq!(cursor(&state), (1, 0));
    feed(&mut state, b"\x1b[2;3H\x1b[6n");
    assert_eq!(cursor(&state), (2, 2));
    assert_eq!(
        state.take_reply(),
        Some(TerminalReply::CursorPosition { row: 2, column: 3 })
    );
    feed(&mut state, b"\x1b[999;999f");
    assert_eq!(cursor(&state), (4, 11));
    feed(&mut state, b"\x1b[999A");
    assert_eq!(cursor(&state), (1, 11));
    feed(&mut state, b"\x1b[999E");
    assert_eq!(cursor(&state), (4, 0));
    feed(&mut state, b"\x1b[999F");
    assert_eq!(cursor(&state), (1, 0));
    feed(&mut state, b"\x1b[3d\x1b[4G");
    assert_eq!(cursor(&state), (3, 3));
    feed(&mut state, b"\x1b[3;4r");
    assert_eq!(cursor(&state), (2, 0));
    feed(&mut state, b"\x1b[?6l\x1b[6n");
    assert!(!state.terminal_modes().origin());
    assert_eq!(cursor(&state), (0, 0));
    assert_eq!(
        state.take_reply(),
        Some(TerminalReply::CursorPosition { row: 1, column: 1 })
    );
    state.set_origin_mode(true);
    state.move_cursor(CursorMovement::VerticalAbsolute(usize::MAX));
    assert_eq!(state.cursor().row(), 3);
}

#[test]
fn decom_save_restore_and_1049_preserve_primary_and_scrollback() {
    let mut state = state();
    feed(&mut state, b"history\r\n1\r\n2\r\n3\r\n4\r\n5\r\n6");
    let history = state.scrollback_len();
    assert!(history > 0);
    feed(
        &mut state,
        b"\x1b[2;5r\x1b[?6h\x1b[2;3H\x1b[3m\x1b7\x1b[?6l\x1b[0m\x1b8",
    );
    assert_eq!(cursor(&state), (2, 2));
    assert!(state.terminal_modes().origin());
    let primary = state.screen().clone();
    let rendition = *state.current_rendition();
    feed(&mut state, b"\x1b[?1049h\x1b[?6lALT\x1b7\x1b[?1049l");
    assert_eq!(state.active_screen(), ScreenKind::Primary);
    assert_eq!(state.screen(), &primary);
    assert_eq!(state.current_rendition(), &rendition);
    assert!(state.terminal_modes().origin());
    assert_eq!(state.scrollback_len(), history);
    // Restoring a saved origin cursor after changed margins clamps to those margins.
    feed(&mut state, b"\x1b[4;5r\x1b8");
    assert_eq!(cursor(&state), (3, 2));
    state.resize(TerminalDimensions::new(2, 1).unwrap());
    feed(&mut state, b"\x1b8\x1b[999d");
    assert_eq!(cursor(&state), (0, 1));
    // Shrinking moves rows above the cursor into history and rewraps text.
    assert_eq!(state.scrollback_len(), 7);
}

#[test]
fn decom_wrap_and_index_scroll_only_inside_margins() {
    let mut state = state();
    feed(
        &mut state,
        b"TOP\x1b[6;1HBOTTOM\x1b[2;5r\x1b[?6h\x1b[4;12HAB\x1bM",
    );
    assert_eq!(state.screen().cell(0, 0).unwrap().character(), 'T');
    assert_eq!(state.screen().cell(5, 0).unwrap().character(), 'B');
    assert_eq!(state.scrollback_len(), 0);
    assert_eq!(cursor(&state), (3, 1));
    // Repeated DECOM requests home and cancel pending wrap.
    feed(&mut state, b"\x1b[4;12HX\x1b[?6hY");
    assert_eq!(state.screen().cell(1, 0).unwrap().character(), 'Y');
    assert_eq!(state.scrollback_len(), 0);
}

#[test]
fn decstr_preserves_buffers_history_selection_tabs_cursor_and_pending_replies() {
    let mut state = state();
    feed(&mut state, b"history\r\n1\r\n2\r\n3\r\n4\r\n5\r\n6\x1b[?1049hALT\x1b[2;5r\x1b[?6h\x1b[2;12H\x1b[1;3;8;9mX\x1b7\x1b[?25l\x1b[?7l\x1b[4h\x1b[?1h\x1b[?2004h\x1b[5n");
    state.clear_all_horizontal_tab_stops();
    state.set_horizontal_tab_stop();
    assert!(state.begin_selection(0, 0));
    assert!(state.extend_selection(0, 2));
    let selection = state.selected_text();
    let screen = state.screen().clone();
    let before = cursor(&state);
    feed(&mut state, b"\x1b[!p");
    assert_eq!(state.active_screen(), ScreenKind::Alternate);
    assert_eq!(state.screen(), &screen);
    assert_eq!(cursor(&state), before);
    assert_eq!(state.selected_text(), selection);
    assert!(state.has_horizontal_tab_stop(11));
    assert!(!state.has_horizontal_tab_stop(8));
    assert_eq!(state.current_rendition(), &CellAttributes::default());
    assert_eq!(state.terminal_modes().auto_wrap(), AutoWrapMode::Enabled);
    assert_eq!(
        state.terminal_modes().character_insertion(),
        CharacterInsertionMode::Replace
    );
    assert_eq!(
        state.terminal_modes().cursor_visibility(),
        CursorVisibility::Visible
    );
    assert!(!state.terminal_modes().origin());
    assert_eq!(state.vertical_scrolling_margins().top(), 0);
    assert_eq!(state.vertical_scrolling_margins().bottom(), 5);
    assert_eq!(state.input_modes().cursor_keys(), CursorKeyMode::Normal);
    assert!(state.input_modes().bracketed_paste());
    assert_eq!(state.take_reply(), Some(TerminalReply::TerminalStatus));
    feed(&mut state, b"Y");
    assert_eq!(
        state.screen().cell(before.0, before.1).unwrap().character(),
        'Y'
    );
    feed(&mut state, b"\x1b8");
    assert_eq!(cursor(&state), (0, 0));
    feed(&mut state, b"\x1b[?1049l");
    assert!(state.scrollback_len() > 0);
    assert_eq!(state.screen().cell(0, 0).unwrap().character(), '1');
}

#[test]
fn ris_clears_both_buffers_history_selection_saved_state_and_input_modes() {
    let mut state = state();
    feed(&mut state, b"\x1b]2;title\x07\x1b]7;file:///tmp\x07history\r\n1\r\n2\r\n3\r\n4\r\n5\r\n6\x1b[?1049hALT\x1b7\x1b[?6h\x1b[?2004h\x1b[?1003h\x1b[1;8;9m\x1b[5n");
    state.begin_selection(0, 0);
    state.clear_all_horizontal_tab_stops();
    let dimensions = state.dimensions();
    feed(&mut state, b"\x1bc");
    assert_eq!(state.dimensions(), dimensions);
    assert_eq!(state.active_screen(), ScreenKind::Primary);
    assert_eq!(state.scrollback_len(), 0);
    assert_eq!(state.viewport_offset(), 0);
    assert_eq!(state.selected_text(), None);
    assert_eq!(state.terminal_modes(), &Default::default());
    assert_eq!(state.input_modes(), &Default::default());
    assert_eq!(state.current_rendition(), &CellAttributes::default());
    assert_eq!(state.shell_title(), Some("title"));
    assert_eq!(state.working_directory_uri(), Some("file:///tmp"));
    assert_eq!(state.take_reply(), Some(TerminalReply::TerminalStatus));
    assert!(state.has_horizontal_tab_stop(8));
    for switch in [b"\x1b[?47h".as_slice(), b"\x1b[?47l"] {
        feed(&mut state, switch);
        for row in 0..dimensions.rows() {
            for column in 0..dimensions.columns() {
                assert_eq!(state.screen().cell(row, column).unwrap().character(), ' ');
            }
        }
        feed(&mut state, b"\x1b[3;4H\x1b8");
        assert_eq!(cursor(&state), (2, 3)); // uninitialized saved slot
    }
}

#[test]
fn reset_and_origin_sequences_work_at_every_byte_boundary() {
    let bytes =
        b"primary\x1b[2;5r\x1b[?6h\x1b[2;3H\x1b7\x1b[?1049hALT\x1b[!p\x1b[?1049l\x1b8\x1bcZ\x1b[6n";
    let mut expected = state();
    feed(&mut expected, bytes);
    for split in 0..=bytes.len() {
        let mut parser = TerminalParser::new();
        let mut state = state();
        parser.advance(&mut state, &bytes[..split]).unwrap();
        parser.advance(&mut state, &bytes[split..]).unwrap();
        assert_eq!(state.screen(), expected.screen());
        assert_eq!(state.current_rendition(), expected.current_rendition());
        assert_eq!(state.terminal_modes(), expected.terminal_modes());
        assert_eq!(
            state.take_reply(),
            Some(TerminalReply::CursorPosition { row: 1, column: 2 })
        );
    }
}

#[test]
fn malformed_mode_reset_and_save_forms_do_not_change_state() {
    let mut state = state();
    feed(&mut state, b"abc\x1b[1;8;9m");
    let screen = state.screen().clone();
    let rendition = *state.current_rendition();
    feed(
        &mut state,
        b"\x1b[6h\x1b[?6:1h\x1b[1!p\x1b[0;0!p\x1b[0:0!p\x1b[?0!p\x1b#c\x1b#7\x1b#8",
    );
    assert_eq!(state.screen(), &screen);
    assert_eq!(state.current_rendition(), &rendition);
    assert!(!state.terminal_modes().origin());
}

#[test]
fn deferred_underline_colors_consume_payload_without_changing_text_styles() {
    let mut state = state();
    feed(&mut state, b"\x1b[58;2;1;8;9;3m");
    assert_eq!(state.current_rendition().intensity(), TextIntensity::Normal);
    assert!(!state.current_rendition().concealed());
    assert!(!state.current_rendition().strikethrough());
    assert_eq!(
        state.current_rendition().italic(),
        terminal_core::ItalicStyle::Italic
    );
    feed(&mut state, b"\x1b[58;5;4m");
    assert_eq!(
        state.current_rendition().underline(),
        UnderlineStyle::Disabled
    );
}

#[test]
fn decstr_preserves_scrolled_primary_viewport_and_selection() {
    let mut state = state();
    feed(&mut state, b"history\r\n1\r\n2\r\n3\r\n4\r\n5\r\n6");
    state.page_up();
    state.begin_selection(0, 0);
    state.extend_selection(0, 3);
    let viewport = state.viewport_offset();
    assert!(viewport > 0);
    let selection = state.selected_text();
    let history = state.scrollback_row(0).unwrap().to_vec();
    let screen = state.screen().clone();
    feed(&mut state, b"\x1b[0!p");
    assert_eq!(state.viewport_offset(), viewport);
    assert_eq!(state.selected_text(), selection);
    assert_eq!(state.scrollback_row(0).unwrap(), history);
    assert_eq!(state.screen(), &screen);
}
