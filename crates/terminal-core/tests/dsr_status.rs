use terminal_core::{
    AutoWrapMode, CharacterInsertionMode, CursorKeyMode, CursorVisibility, MAX_PENDING_REPLIES,
    TerminalDimensions, TerminalReply, TerminalState,
};

const STATUS: &[u8] = b"\x1b[0n";
const DA1: &[u8] = b"\x1b[?1;0c";

fn bytes(reply: TerminalReply) -> Vec<u8> {
    reply.as_bytes().as_slice().to_vec()
}

#[test]
fn terminal_status_encodes_exactly_and_is_bounded() {
    let reply = TerminalReply::TerminalStatus;
    assert_eq!(reply.as_bytes().as_slice(), STATUS);
    assert_eq!(reply.as_bytes().as_slice().len(), 4);
    assert_eq!(
        TerminalReply::PrimaryDeviceAttributes.as_bytes().as_slice(),
        DA1
    );
    assert_eq!(
        TerminalReply::CursorPosition {
            row: 12,
            column: 20
        }
        .as_bytes()
        .as_slice(),
        b"\x1b[12;20R"
    );
}

#[test]
fn status_queries_are_fifo_bounded_and_interleave_with_existing_replies() {
    let mut state = TerminalState::new(TerminalDimensions::new(12, 12).unwrap());
    state.set_cursor_position(4, 9).unwrap();
    assert!(state.request_terminal_status());
    assert!(state.request_cursor_position_report());
    assert!(state.request_primary_device_attributes());
    assert_eq!(state.take_reply().map(bytes).as_deref(), Some(STATUS));
    assert_eq!(
        state.take_reply().map(bytes).as_deref(),
        Some(b"\x1b[5;10R".as_slice())
    );
    assert_eq!(state.take_reply().map(bytes).as_deref(), Some(DA1));

    for _ in 0..MAX_PENDING_REPLIES {
        assert!(state.request_terminal_status());
    }
    assert!(!state.request_terminal_status());
    assert_eq!(state.pending_reply_count(), MAX_PENDING_REPLIES);
    assert_eq!(state.take_reply().map(bytes).as_deref(), Some(STATUS));
}

#[test]
fn status_query_preserves_terminal_state_and_delayed_wrap() {
    let mut state = TerminalState::new(TerminalDimensions::new(3, 3).unwrap());
    state.set_character_insertion(CharacterInsertionMode::Insert);
    state.set_auto_wrap(AutoWrapMode::Enabled);
    state.set_cursor_visibility(CursorVisibility::Hidden);
    state.set_cursor_key_mode(CursorKeyMode::Application);
    assert!(state.set_vertical_scrolling_margins(1, 2));
    state.set_cursor_position(0, 1).unwrap();
    state.set_horizontal_tab_stop();
    state.set_cursor_position(0, 2).unwrap();
    state.print_character('Q').unwrap();
    let screen = state.screen().clone();
    let cursor = state.cursor();
    let modes = *state.terminal_modes();
    let input_modes = *state.input_modes();
    let margins = state.vertical_scrolling_margins();

    assert!(state.request_terminal_status());
    assert_eq!(state.screen(), &screen);
    assert_eq!(state.cursor(), cursor);
    assert_eq!(*state.terminal_modes(), modes);
    assert_eq!(*state.input_modes(), input_modes);
    assert_eq!(state.vertical_scrolling_margins(), margins);
    assert!(state.has_horizontal_tab_stop(1));
    assert_eq!(state.take_reply().map(bytes).as_deref(), Some(STATUS));

    state.print_character('X').unwrap();
    assert_eq!(state.screen().cell(1, 0).unwrap().character(), 'X');
}
