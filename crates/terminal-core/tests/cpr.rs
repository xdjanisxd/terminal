use terminal_core::{
    AutoWrapMode, CellColor, CharacterInsertionMode, CursorKeyMode, CursorVisibility, InverseVideo,
    ItalicStyle, MAX_PENDING_REPLIES, TerminalDimensions, TerminalParser, TerminalState,
    TextIntensity, UnderlineStyle,
};

const DA1: &[u8] = b"\x1b[?1;0c";
const CPR_HOME: &[u8] = b"\x1b[1;1R";

fn reply_bytes(reply: terminal_core::TerminalReply) -> Vec<u8> {
    reply.as_bytes().as_slice().to_vec()
}

#[test]
fn cursor_position_report_uses_one_based_coordinates_including_multi_digit_values() {
    let mut state = TerminalState::new(TerminalDimensions::new(20, 12).unwrap());

    assert!(state.request_cursor_position_report());
    state.set_cursor_position(4, 9).unwrap();
    assert!(state.request_cursor_position_report());
    state.set_cursor_position(11, 19).unwrap();
    assert!(state.request_cursor_position_report());

    assert_eq!(
        state.take_reply().map(reply_bytes).as_deref(),
        Some(CPR_HOME)
    );
    assert_eq!(
        state.take_reply().map(reply_bytes).as_deref(),
        Some(b"\x1b[5;10R".as_slice())
    );
    assert_eq!(
        state.take_reply().map(reply_bytes).as_deref(),
        Some(b"\x1b[12;20R".as_slice())
    );
}

#[test]
fn cpr_generation_preserves_terminal_state_and_delayed_wrap() {
    let mut state = TerminalState::new(TerminalDimensions::new(3, 4).unwrap());
    assert!(state.set_vertical_scrolling_margins(1, 2));
    state.set_foreground_color(CellColor::Indexed(196));
    state.set_background_color(CellColor::Indexed(22));
    state.set_text_intensity(TextIntensity::Bold);
    state.set_italic_style(ItalicStyle::Italic);
    state.set_underline_style(UnderlineStyle::Enabled);
    state.set_inverse_video(InverseVideo::Enabled);
    state.set_character_insertion(CharacterInsertionMode::Insert);
    state.set_auto_wrap(AutoWrapMode::Enabled);
    state.set_cursor_visibility(CursorVisibility::Hidden);
    state.set_cursor_key_mode(CursorKeyMode::Application);
    state.set_cursor_position(0, 1).unwrap();
    state.set_horizontal_tab_stop();
    state.set_cursor_position(0, 2).unwrap();
    state.print_character('Q').unwrap();

    let screen = state.screen().clone();
    let cursor = state.cursor();
    let rendition = *state.current_rendition();
    let terminal_modes = *state.terminal_modes();
    let input_modes = *state.input_modes();
    let margins = state.vertical_scrolling_margins();

    assert!(state.request_cursor_position_report());
    assert_eq!(state.screen(), &screen);
    assert_eq!(state.cursor(), cursor);
    assert_eq!(*state.current_rendition(), rendition);
    assert_eq!(*state.terminal_modes(), terminal_modes);
    assert_eq!(*state.input_modes(), input_modes);
    assert_eq!(state.vertical_scrolling_margins(), margins);
    assert!(state.has_horizontal_tab_stop(1));
    assert_eq!(
        state.take_reply().map(reply_bytes).as_deref(),
        Some(b"\x1b[1;3R".as_slice())
    );

    state.print_character('X').unwrap();
    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 1));
    assert_eq!(state.screen().cell(1, 0).unwrap().character(), 'X');
    assert_eq!(state.screen().cell(1, 0).unwrap().attributes(), &rendition);
}

#[test]
fn cpr_is_absolute_with_custom_margins_and_after_region_aware_scrolling() {
    let mut state = TerminalState::new(TerminalDimensions::new(5, 6).unwrap());
    assert!(state.set_vertical_scrolling_margins(1, 4));
    let margins = state.vertical_scrolling_margins();

    for (row, expected) in [
        (0, b"\x1b[1;3R".as_slice()),
        (2, b"\x1b[3;3R"),
        (5, b"\x1b[6;3R"),
    ] {
        state.set_cursor_position(row, 2).unwrap();
        assert!(state.request_cursor_position_report());
        assert_eq!(
            state.take_reply().map(reply_bytes).as_deref(),
            Some(expected)
        );
        assert_eq!(state.vertical_scrolling_margins(), margins);
    }

    state.set_cursor_position(4, 3).unwrap();
    state.index();
    assert_eq!((state.cursor().row(), state.cursor().column()), (4, 3));
    assert!(state.request_cursor_position_report());
    assert_eq!(
        state.take_reply().map(reply_bytes).as_deref(),
        Some(b"\x1b[5;4R".as_slice())
    );
    assert_eq!(state.vertical_scrolling_margins(), margins);
}

#[test]
fn mixed_replies_are_fifo_bounded_and_captured_before_reset_or_resize() {
    let mut state = TerminalState::new(TerminalDimensions::new(12, 12).unwrap());
    state.set_cursor_position(4, 9).unwrap();
    assert!(state.request_primary_device_attributes());
    assert!(state.request_cursor_position_report());
    assert!(state.request_primary_device_attributes());
    state.reset();
    state.resize(TerminalDimensions::new(20, 20).unwrap());

    assert_eq!(state.take_reply().map(reply_bytes).as_deref(), Some(DA1));
    assert_eq!(
        state.take_reply().map(reply_bytes).as_deref(),
        Some(b"\x1b[5;10R".as_slice())
    );
    assert_eq!(state.take_reply().map(reply_bytes).as_deref(), Some(DA1));

    for index in 0..MAX_PENDING_REPLIES {
        if index % 2 == 0 {
            assert!(state.request_primary_device_attributes());
        } else {
            state.set_cursor_position(index, index).unwrap();
            assert!(state.request_cursor_position_report());
        }
    }
    let cursor = state.cursor();
    assert!(!state.request_cursor_position_report());
    assert_eq!(state.cursor(), cursor);
    assert_eq!(state.pending_reply_count(), MAX_PENDING_REPLIES);
    assert_eq!(state.take_reply().map(reply_bytes).as_deref(), Some(DA1));
    state.set_cursor_position(19, 19).unwrap();
    assert!(state.request_cursor_position_report());
    for expected in [
        b"\x1b[2;2R".as_slice(),
        DA1,
        b"\x1b[4;4R",
        DA1,
        b"\x1b[6;6R",
        DA1,
        b"\x1b[8;8R",
        DA1,
        b"\x1b[10;10R",
        DA1,
        b"\x1b[12;12R",
        DA1,
        b"\x1b[14;14R",
        DA1,
        b"\x1b[16;16R",
    ] {
        assert_eq!(
            state.take_reply().map(reply_bytes).as_deref(),
            Some(expected)
        );
    }
    assert_eq!(
        state.take_reply().map(reply_bytes).as_deref(),
        Some(b"\x1b[20;20R".as_slice())
    );
}

#[test]
fn parser_cpr_preserves_absolute_position_and_does_not_reply_to_other_dsr_forms() {
    let mut parser = TerminalParser::new();
    let mut state = TerminalState::new(TerminalDimensions::new(20, 12).unwrap());

    parser
        .advance(&mut state, b"A\x1b[5C\x1b[3B\x1b[6nB")
        .unwrap();
    assert_eq!((state.cursor().row(), state.cursor().column()), (3, 7));
    assert_eq!(
        state.take_reply().map(reply_bytes).as_deref(),
        Some(b"\x1b[4;7R".as_slice())
    );
    assert_eq!(state.take_reply(), None);

    for sequence in [b"\x1b[0n".as_slice(), b"\x1b[7n", b"\x1b[?6n", b"\x1b[>6n"] {
        parser.advance(&mut state, sequence).unwrap();
        assert_eq!(state.take_reply(), None, "sequence {sequence:?}");
    }
}
