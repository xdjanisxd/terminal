use terminal_core::{
    CursorKey, CursorKeyMode, MouseButton, MouseEncoding, MouseEvent, MouseModifiers,
    MouseTracking, TerminalDimensions, TerminalParser, TerminalState, encode_control_cursor_key,
    encode_cursor_key, encode_focus, encode_mouse, encode_paste,
};

fn terminal() -> (TerminalParser, TerminalState) {
    (
        TerminalParser::new(),
        TerminalState::new(TerminalDimensions::new(80, 24).unwrap()),
    )
}

#[test]
fn output_modes_drive_cursor_focus_and_paste_bytes() {
    let (mut parser, mut terminal) = terminal();
    let modes = *terminal.input_modes();
    assert_eq!(encode_cursor_key(modes, CursorKey::Up), b"\x1b[A");
    assert_eq!(encode_focus(modes, true), None);
    assert_eq!(encode_paste(modes, "a\nb"), b"a\nb");

    assert!(parser.advance(&mut terminal, b"\x1b[?1;1004;2004h").is_ok());
    let modes = *terminal.input_modes();
    assert_eq!(modes.cursor_keys(), CursorKeyMode::Application);
    for (key, suffix) in [
        (CursorKey::Up, b'A'),
        (CursorKey::Down, b'B'),
        (CursorKey::Right, b'C'),
        (CursorKey::Left, b'D'),
    ] {
        assert_eq!(encode_cursor_key(modes, key), &[b'\x1b', b'O', suffix]);
    }
    assert_eq!(encode_focus(modes, true), Some(b"\x1b[I".as_slice()));
    assert_eq!(encode_focus(modes, false), Some(b"\x1b[O".as_slice()));
    assert_eq!(encode_paste(modes, "a\nb"), b"\x1b[200~a\nb\x1b[201~");

    assert!(parser.advance(&mut terminal, b"\x1b[?1;1004;2004l").is_ok());
    let modes = *terminal.input_modes();
    assert_eq!(encode_cursor_key(modes, CursorKey::Left), b"\x1b[D");
    assert_eq!(encode_focus(modes, false), None);
    assert_eq!(encode_paste(modes, "x"), b"x");
}

#[test]
fn control_left_and_right_use_modified_csi_in_both_cursor_key_modes() {
    let (mut parser, mut terminal) = terminal();
    for application_mode in [false, true] {
        if application_mode {
            assert!(parser.advance(&mut terminal, b"\x1b[?1h").is_ok());
        }
        let modes = *terminal.input_modes();
        assert_eq!(
            encode_control_cursor_key(CursorKey::Left),
            Some(b"\x1b[1;5D".as_slice())
        );
        assert_eq!(
            encode_control_cursor_key(CursorKey::Right),
            Some(b"\x1b[1;5C".as_slice())
        );
        assert_eq!(encode_control_cursor_key(CursorKey::Up), None);
        assert_eq!(encode_control_cursor_key(CursorKey::Down), None);
        let prefix = if application_mode { b'O' } else { b'[' };
        assert_eq!(
            encode_cursor_key(modes, CursorKey::Left),
            &[0x1b, prefix, b'D']
        );
        assert_eq!(
            encode_cursor_key(modes, CursorKey::Right),
            &[0x1b, prefix, b'C']
        );
    }
}

#[test]
fn mouse_tracking_and_encodings_follow_private_modes() {
    let (mut parser, mut terminal) = terminal();
    let modifiers = MouseModifiers::default();
    assert_eq!(
        encode_mouse(
            *terminal.input_modes(),
            MouseEvent::WheelUp,
            0,
            0,
            modifiers
        ),
        None
    );

    assert!(parser.advance(&mut terminal, b"\x1b[?1000h").is_ok());
    let modes = *terminal.input_modes();
    assert_eq!(modes.mouse_tracking(), MouseTracking::Press);
    assert_eq!(
        encode_mouse(modes, MouseEvent::Press(MouseButton::Left), 0, 0, modifiers),
        Some(b"\x1b[M !!".to_vec())
    );
    assert_eq!(
        encode_mouse(
            modes,
            MouseEvent::Release(MouseButton::Left),
            0,
            0,
            modifiers
        ),
        Some(b"\x1b[M#!!".to_vec())
    );
    assert_eq!(
        encode_mouse(modes, MouseEvent::WheelUp, 0, 0, modifiers),
        Some(b"\x1b[M`!!".to_vec())
    );
    assert_eq!(
        encode_mouse(
            modes,
            MouseEvent::Move(Some(MouseButton::Left)),
            0,
            0,
            modifiers
        ),
        None
    );
    assert_eq!(
        encode_mouse(
            modes,
            MouseEvent::Press(MouseButton::Left),
            223,
            0,
            modifiers
        ),
        None
    );

    assert!(parser.advance(&mut terminal, b"\x1b[?1002;1006h").is_ok());
    let modes = *terminal.input_modes();
    assert_eq!(modes.mouse_tracking(), MouseTracking::Drag);
    assert_eq!(modes.mouse_encoding(), MouseEncoding::Sgr);
    assert_eq!(
        encode_mouse(modes, MouseEvent::Move(None), 4, 9, modifiers),
        None
    );
    assert_eq!(
        encode_mouse(
            modes,
            MouseEvent::Move(Some(MouseButton::Right)),
            4,
            9,
            modifiers
        ),
        Some(b"\x1b[<34;5;10M".to_vec())
    );
    assert_eq!(
        encode_mouse(
            modes,
            MouseEvent::Release(MouseButton::Right),
            4,
            9,
            modifiers
        ),
        Some(b"\x1b[<2;5;10m".to_vec())
    );
    assert_eq!(
        encode_mouse(
            modes,
            MouseEvent::WheelDown,
            4,
            9,
            MouseModifiers {
                shift: true,
                alt: true,
                control: true
            }
        ),
        Some(b"\x1b[<93;5;10M".to_vec())
    );

    assert!(parser.advance(&mut terminal, b"\x1b[?1003h").is_ok());
    assert_eq!(terminal.input_modes().mouse_tracking(), MouseTracking::Any);
    assert_eq!(
        encode_mouse(
            *terminal.input_modes(),
            MouseEvent::Move(None),
            4,
            9,
            modifiers
        ),
        Some(b"\x1b[<35;5;10M".to_vec())
    );
    assert!(parser.advance(&mut terminal, b"\x1b[?1003l").is_ok());
    assert_eq!(terminal.input_modes().mouse_tracking(), MouseTracking::Drag);
    assert!(parser.advance(&mut terminal, b"\x1b[?1002;1006l").is_ok());
    assert_eq!(
        terminal.input_modes().mouse_tracking(),
        MouseTracking::Press
    );
    assert_eq!(
        terminal.input_modes().mouse_encoding(),
        MouseEncoding::Legacy
    );
    assert!(parser.advance(&mut terminal, b"\x1b[?1000l").is_ok());
    assert_eq!(terminal.input_modes().mouse_tracking(), MouseTracking::Off);

    assert!(
        parser
            .advance(&mut terminal, b"\x1b[?1;1003;1004;1006;2004h")
            .is_ok()
    );
    terminal.reset();
    assert_eq!(*terminal.input_modes(), Default::default());
}
