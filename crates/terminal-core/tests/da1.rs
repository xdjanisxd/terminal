use terminal_core::{
    AutoWrapMode, CellColor, CharacterInsertionMode, CursorKeyMode, CursorVisibility, InverseVideo,
    ItalicStyle, MAX_PENDING_REPLIES, TerminalDimensions, TerminalParser, TerminalReply,
    TerminalState, TextIntensity, UnderlineStyle, VerticalScrollingMargins,
};

const EXPECTED_DA1: &[u8] = b"\x1b[?1;0c";

fn reply_bytes(reply: TerminalReply) -> Vec<u8> {
    reply.as_bytes().as_slice().to_vec()
}

#[test]
fn primary_device_attributes_queues_exact_conservative_reply() {
    let mut state = TerminalState::new(TerminalDimensions::new(3, 4).unwrap());

    assert!(state.request_primary_device_attributes());

    assert_eq!(state.pending_reply_count(), 1);
    assert_eq!(
        state.take_reply().map(reply_bytes),
        Some(EXPECTED_DA1.to_vec())
    );
    assert_eq!(state.pending_reply_count(), 0);
    assert_eq!(state.take_reply(), None);
}

#[test]
fn pending_replies_are_fifo_bounded_and_never_overwritten() {
    let mut state = TerminalState::new(TerminalDimensions::new(1, 1).unwrap());

    for expected_count in 1..=MAX_PENDING_REPLIES {
        assert!(state.request_primary_device_attributes());
        assert_eq!(state.pending_reply_count(), expected_count);
    }

    assert!(!state.request_primary_device_attributes());
    assert_eq!(state.pending_reply_count(), MAX_PENDING_REPLIES);

    assert_eq!(
        state.take_reply().map(reply_bytes),
        Some(EXPECTED_DA1.to_vec())
    );
    assert!(state.request_primary_device_attributes());
    assert_eq!(state.pending_reply_count(), MAX_PENDING_REPLIES);

    for _ in 0..MAX_PENDING_REPLIES {
        assert_eq!(
            state.take_reply().map(reply_bytes),
            Some(EXPECTED_DA1.to_vec())
        );
    }
    assert_eq!(state.take_reply(), None);
}

#[test]
fn da1_generation_and_consumption_preserve_terminal_state_and_delayed_wrap() {
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
    let mut parser = TerminalParser::new();

    parser.advance(&mut state, b"\x1b[c").unwrap();
    assert_eq!(state.screen(), &screen);
    assert_eq!(state.cursor(), cursor);
    assert_eq!(*state.current_rendition(), rendition);
    assert_eq!(*state.terminal_modes(), terminal_modes);
    assert_eq!(*state.input_modes(), input_modes);
    assert_eq!(state.vertical_scrolling_margins(), margins);
    assert!(state.has_horizontal_tab_stop(1));

    assert_eq!(
        state.take_reply().map(reply_bytes),
        Some(EXPECTED_DA1.to_vec())
    );
    assert_eq!(state.screen(), &screen);
    assert_eq!(state.cursor(), cursor);
    assert_eq!(*state.current_rendition(), rendition);
    assert_eq!(*state.terminal_modes(), terminal_modes);
    assert_eq!(*state.input_modes(), input_modes);
    assert_eq!(state.vertical_scrolling_margins(), margins);
    assert!(state.has_horizontal_tab_stop(1));

    state.print_character('X').unwrap();
    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 1));
    assert_eq!(state.screen().cell(1, 0).unwrap().character(), 'X');
    assert_eq!(state.screen().cell(1, 0).unwrap().attributes(), &rendition);
}

#[test]
fn reset_and_resize_preserve_already_generated_replies() {
    let mut state = TerminalState::new(TerminalDimensions::new(3, 4).unwrap());
    assert!(state.request_primary_device_attributes());
    assert!(state.request_primary_device_attributes());

    state.reset();

    assert_eq!(state.pending_reply_count(), 2);
    assert_eq!(
        state.take_reply().map(reply_bytes),
        Some(EXPECTED_DA1.to_vec())
    );

    state.resize(TerminalDimensions::new(5, 2).unwrap());

    assert_eq!(state.pending_reply_count(), 1);
    assert_eq!(
        state.take_reply().map(reply_bytes),
        Some(EXPECTED_DA1.to_vec())
    );
    assert_eq!(state.take_reply(), None);
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::full_screen(2)
    );
}
