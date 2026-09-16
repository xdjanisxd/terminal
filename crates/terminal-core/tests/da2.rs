use terminal_core::{
    AutoWrapMode, CellColor, CharacterInsertionMode, CursorKeyMode, CursorVisibility, InverseVideo,
    ItalicStyle, TerminalDimensions, TerminalParser, TerminalReply, TerminalState, TextIntensity,
    UnderlineStyle,
};

const DA1: &[u8] = b"\x1b[?1;0c";
const DA2: &[u8] = b"\x1b[>0;0;0c";
const DSR_STATUS: &[u8] = b"\x1b[0n";
const CPR: &[u8] = b"\x1b[2;3R";

fn state() -> TerminalState {
    TerminalState::new(TerminalDimensions::new(6, 4).unwrap())
}

fn bytes(reply: TerminalReply) -> Vec<u8> {
    reply.as_bytes().as_slice().to_vec()
}

#[test]
fn secondary_device_attributes_has_a_fixed_bounded_conservative_encoding() {
    let reply = TerminalReply::SecondaryDeviceAttributes;
    assert_eq!(bytes(reply), DA2);
    assert_eq!(reply.as_bytes().as_slice().len(), 9);
    assert_eq!(bytes(TerminalReply::PrimaryDeviceAttributes), DA1);
    assert_eq!(bytes(TerminalReply::TerminalStatus), DSR_STATUS);
    assert_eq!(
        bytes(TerminalReply::CursorPosition { row: 2, column: 3 }),
        CPR
    );
}

#[test]
fn parser_recognizes_only_omitted_or_zero_da2_requests() {
    for request in [b"\x1b[>c".as_slice(), b"\x1b[>0c"] {
        let mut parser = TerminalParser::new();
        let mut terminal = state();
        parser.advance(&mut terminal, request).unwrap();
        assert_eq!(terminal.take_reply().map(bytes), Some(DA2.to_vec()));
    }

    for request in [
        b"\x1b[>1c".as_slice(),
        b"\x1b[>42c",
        b"\x1b[>0;0c",
        b"\x1b[>0:1c",
        b"\x1b[?c",
        b"\x1b[?0c",
        b"\x1b[=c",
        b"\x1b[=0c",
    ] {
        let mut parser = TerminalParser::new();
        let mut terminal = state();
        parser.advance(&mut terminal, request).unwrap();
        assert_eq!(terminal.take_reply(), None, "request {request:?}");
    }

    let input = b"A\x1b[>0cB";
    for split in 0..=input.len() {
        let mut parser = TerminalParser::new();
        let mut terminal = state();
        parser.advance(&mut terminal, &input[..split]).unwrap();
        parser.advance(&mut terminal, &input[split..]).unwrap();
        assert_eq!(terminal.take_reply().map(bytes), Some(DA2.to_vec()));
        assert_eq!(terminal.screen().cell(0, 0).unwrap().character(), 'A');
        assert_eq!(terminal.screen().cell(0, 1).unwrap().character(), 'B');
    }
}

#[test]
fn secondary_device_attributes_preserves_fifo_order_and_capacity() {
    let mut parser = TerminalParser::new();
    let mut terminal = state();
    terminal.set_cursor_position(1, 2).unwrap();
    parser
        .advance(&mut terminal, b"\x1b[>c\x1b[c\x1b[5n\x1b[6n\x1b[>0c")
        .unwrap();

    let expected = [DA2, DA1, DSR_STATUS, CPR, DA2];
    for reply in expected {
        assert_eq!(terminal.take_reply().map(bytes), Some(reply.to_vec()));
    }
    assert_eq!(terminal.take_reply(), None);

    for _ in 0..16 {
        assert!(terminal.request_secondary_device_attributes());
    }
    assert!(!terminal.request_secondary_device_attributes());
    assert_eq!(terminal.pending_reply_count(), 16);
    for _ in 0..16 {
        assert_eq!(terminal.take_reply().map(bytes), Some(DA2.to_vec()));
    }
}

#[test]
fn secondary_device_attributes_changes_only_pending_replies() {
    let mut parser = TerminalParser::new();
    let mut terminal = state();
    terminal.set_cursor_position(2, 3).unwrap();
    assert!(terminal.set_vertical_scrolling_margins(1, 2));
    terminal.set_cursor_position(2, 3).unwrap();
    terminal.set_foreground_color(CellColor::Indexed(196));
    terminal.set_text_intensity(TextIntensity::Faint);
    terminal.set_italic_style(ItalicStyle::Italic);
    terminal.set_underline_style(UnderlineStyle::Enabled);
    terminal.set_inverse_video(InverseVideo::Enabled);
    terminal.set_character_insertion(CharacterInsertionMode::Insert);
    terminal.set_auto_wrap(AutoWrapMode::Disabled);
    terminal.set_cursor_visibility(CursorVisibility::Hidden);
    terminal.set_cursor_key_mode(CursorKeyMode::Application);
    terminal.set_horizontal_tab_stop();
    let screen = terminal.screen().clone();
    let cursor = terminal.cursor();
    let rendition = *terminal.current_rendition();
    let modes = *terminal.terminal_modes();
    let input_modes = *terminal.input_modes();
    let margins = terminal.vertical_scrolling_margins();

    parser.advance(&mut terminal, b"\x1b[>c\x1b[").unwrap();

    assert_eq!(terminal.screen(), &screen);
    assert_eq!(terminal.cursor(), cursor);
    assert_eq!(*terminal.current_rendition(), rendition);
    assert_eq!(*terminal.terminal_modes(), modes);
    assert_eq!(*terminal.input_modes(), input_modes);
    assert_eq!(terminal.vertical_scrolling_margins(), margins);
    assert!(terminal.has_horizontal_tab_stop(3));
    assert_eq!(terminal.take_reply().map(bytes), Some(DA2.to_vec()));
}
