use terminal_core::{
    CursorKey, CursorKeyMode, EditingKey, MouseButton, MouseEncoding, MouseEvent, MouseModifiers,
    MouseTracking, TerminalDimensions, TerminalParser, TerminalState, encode_control_cursor_key,
    encode_cursor_key, encode_editing_key, encode_focus, encode_mouse, encode_paste,
};

fn terminal() -> (TerminalParser, TerminalState) {
    (
        TerminalParser::new(),
        TerminalState::new(TerminalDimensions::new(80, 24).unwrap()),
    )
}

#[test]
fn function_keys_use_xterm_sequences_for_all_modifiers_and_screen_modes() {
    use terminal_core::{FunctionKey::*, KeyModifiers, encode_function_key};
    let cases = [
        (F1, "\x1bOP", 1, 'P'),
        (F2, "\x1bOQ", 1, 'Q'),
        (F3, "\x1bOR", 1, 'R'),
        (F4, "\x1bOS", 1, 'S'),
        (F5, "\x1b[15~", 15, '~'),
        (F6, "\x1b[17~", 17, '~'),
        (F7, "\x1b[18~", 18, '~'),
        (F8, "\x1b[19~", 19, '~'),
        (F9, "\x1b[20~", 20, '~'),
        (F10, "\x1b[21~", 21, '~'),
        (F11, "\x1b[23~", 23, '~'),
        (F12, "\x1b[24~", 24, '~'),
    ];
    let (mut parser, mut terminal) = terminal();
    for modes in [
        b"\x1b[?1l".as_slice(),
        b"\x1b[?1h",
        b"\x1b[?1049h",
        b"\x1b[?1l",
        b"\x1b[?1049l",
    ] {
        parser.advance(&mut terminal, modes).unwrap();
        for (key, plain, number, suffix) in cases {
            for (bits, parameter) in [
                (0, 1),
                (1, 2),
                (2, 3),
                (3, 4),
                (4, 5),
                (5, 6),
                (6, 7),
                (7, 8),
            ] {
                let modifiers = KeyModifiers {
                    shift: bits & 1 != 0,
                    alt: bits & 2 != 0,
                    control: bits & 4 != 0,
                };
                let expected = if bits == 0 {
                    plain.to_owned()
                } else {
                    format!("\x1b[{number};{parameter}{suffix}")
                };
                assert_eq!(
                    encode_function_key(key, modifiers),
                    expected.as_bytes(),
                    "{key:?} modifiers={bits}"
                );
            }
        }
    }
}

#[test]
fn delete_home_and_end_use_conventional_sequences_in_both_cursor_modes() {
    let (mut parser, mut terminal) = terminal();
    let normal = *terminal.input_modes();
    assert_eq!(encode_editing_key(normal, EditingKey::Delete), b"\x1b[3~");
    assert_eq!(encode_editing_key(normal, EditingKey::Home), b"\x1b[H");
    assert_eq!(encode_editing_key(normal, EditingKey::End), b"\x1b[F");

    assert!(parser.advance(&mut terminal, b"\x1b[?1h").is_ok());
    let application = *terminal.input_modes();
    assert_eq!(
        encode_editing_key(application, EditingKey::Delete),
        b"\x1b[3~"
    );
    assert_eq!(encode_editing_key(application, EditingKey::Home), b"\x1b[H");
    assert_eq!(encode_editing_key(application, EditingKey::End), b"\x1b[F");

    // Later terminal state and parser chunking do not affect input encoding.
    assert!(parser.advance(&mut terminal, b"\x1b[?1l").is_ok());
    assert_eq!(
        encode_editing_key(*terminal.input_modes(), EditingKey::Delete),
        b"\x1b[3~"
    );
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
