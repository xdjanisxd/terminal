use terminal_core::{
    CellAttributes, CursorShape, CursorStyle, Osc52Policy, ScreenKind, TerminalDimensions,
    TerminalParser, TerminalReply, TerminalState,
};

fn state() -> TerminalState {
    TerminalState::new(TerminalDimensions::new(16, 4).unwrap())
}

#[test]
fn canceled_osc_never_applies_payload_and_valid_input_recovers_at_every_split() {
    for payload in [
        b"2;canceled".as_slice(),
        b"7;file:///canceled",
        b"8;;https://canceled.example",
        b"52;c;aGVsbG8=",
        b"133;A",
    ] {
        for cancel in [0x18, 0x1a] {
            let mut bytes = b"\x1b]".to_vec();
            bytes.extend_from_slice(payload);
            bytes.push(cancel);
            bytes.extend_from_slice(b"\x1b[32mX\x1b[0m");
            for split in 0..=bytes.len() {
                let mut terminal = state();
                let mut parser = TerminalParser::with_osc52_policy(Osc52Policy::AllowWrite);
                parser.advance(&mut terminal, &bytes[..split]).unwrap();
                parser.advance(&mut terminal, &bytes[split..]).unwrap();
                assert_eq!(terminal.shell_title(), None, "{payload:?}/{cancel}/{split}");
                assert_eq!(terminal.working_directory_uri(), None);
                assert_eq!(terminal.shell_prompt_version(), 0);
                assert_eq!(terminal.take_osc52_write(), None);
                assert_eq!(terminal.target_at_viewport(0, 0), None);
                assert_eq!(terminal.screen().cell(0, 0).unwrap().character(), 'X');
            }
        }
    }
}

#[test]
fn cancellation_and_overflow_preserve_existing_metadata_and_hyperlink() {
    let mut terminal = state();
    let mut parser = TerminalParser::new();
    parser
        .advance(
            &mut terminal,
            b"\x1b]2;original\x07\x1b]7;file:///original\x07\x1b]8;;https://original.example\x07",
        )
        .unwrap();
    let version = terminal.shell_title_version();
    parser
        .advance(
            &mut terminal,
            b"\x1b]2;changed\x18\x1b]7;file:///changed\x1a\x1b]8;;\x18",
        )
        .unwrap();
    parser
        .advance(
            &mut terminal,
            format!("\x1b]8;;https://{}\x07X", "x".repeat(2048)).as_bytes(),
        )
        .unwrap();
    assert_eq!(terminal.shell_title(), Some("original"));
    assert_eq!(terminal.shell_title_version(), version);
    assert_eq!(terminal.working_directory_uri(), Some("file:///original"));
    assert_eq!(
        terminal.target_at_viewport(0, 0),
        Some("https://original.example")
    );
}

#[test]
fn saturated_osc_payloads_are_rejected_instead_of_applying_truncated_values() {
    let inputs = [
        format!("\x1b]7;file:///{}\x07", "x".repeat(4096)),
        format!("\x1b]8;;https://example/{}\x1b\\", "x".repeat(4096)),
        format!("\x1b]2;{}\x07", "x;".repeat(32)),
    ];
    for input in inputs {
        for chunk_size in [1, 7, input.len()] {
            let mut terminal = state();
            let mut parser = TerminalParser::new();
            for chunk in input.as_bytes().chunks(chunk_size) {
                parser.advance(&mut terminal, chunk).unwrap();
            }
            parser.advance(&mut terminal, b"X").unwrap();
            assert_eq!(terminal.working_directory_uri(), None);
            assert_eq!(terminal.shell_title(), None);
            assert_eq!(terminal.target_at_viewport(0, 0), None);
            assert_eq!(terminal.screen().cell(0, 0).unwrap().character(), 'X');
        }
    }
}

#[test]
fn osc_capacity_boundary_is_conservative_and_bel_and_st_still_work() {
    for terminator in [b"\x07".as_slice(), b"\x1b\\"] {
        for size in [1023, 1024, 1025] {
            let mut terminal = state();
            let mut parser = TerminalParser::new();
            // OSC storage excludes semicolon separators; includes the command.
            let uri = format!("file:///{}", "x".repeat(size - 9));
            let mut input = format!("\x1b]7;{uri}").into_bytes();
            input.extend_from_slice(terminator);
            for byte in &input {
                parser.advance(&mut terminal, &[*byte]).unwrap();
            }
            assert_eq!(
                terminal.working_directory_uri(),
                (size < 1024).then_some(uri.as_str())
            );
        }
        for fields in [15, 16, 17] {
            let title = vec!["x"; fields - 1].join(";");
            let mut input = format!("\x1b]2;{title}").into_bytes();
            input.extend_from_slice(terminator);
            let mut terminal = state();
            TerminalParser::new()
                .advance(&mut terminal, &input)
                .unwrap();
            assert_eq!(
                terminal.shell_title(),
                (fields < 16).then_some(title.as_str())
            );
        }
    }
}

#[test]
fn interrupted_parser_states_recover_without_rendition_or_screen_leaks() {
    let prefixes = [
        b"\x1b[31".as_slice(),
        b"\x1b[?25;",
        b"\x1b[1:2:3",
        b"\x1b#",
        b"\x1b]2;incomplete",
        b"\x1bP1;2$qignored",
        b"\x1bXignored",
        b"\x1b^ignored",
        b"\x1b_ignored",
    ];
    for prefix in prefixes {
        for cancel in [0x18, 0x1a] {
            let mut bytes = prefix.to_vec();
            bytes.push(cancel);
            bytes.extend_from_slice("\x1b[!p界e\u{301}\x1b[6n".as_bytes());
            for split in 0..=bytes.len() {
                let mut terminal = state();
                let mut parser = TerminalParser::new();
                parser.advance(&mut terminal, &bytes[..split]).unwrap();
                parser.advance(&mut terminal, &bytes[split..]).unwrap();
                assert_eq!(terminal.screen().cell(0, 0).unwrap().character(), '界');
                assert!(terminal.screen().cell(0, 1).unwrap().is_wide_continuation());
                let cell = terminal.screen().cell(0, 2).unwrap();
                assert_eq!(cell.character(), 'e');
                assert_eq!(cell.combining_marks(), ['\u{301}']);
                assert_eq!(cell.attributes(), &CellAttributes::default());
                assert_eq!(terminal.shell_title(), None);
                assert_eq!(
                    terminal.take_reply(),
                    Some(TerminalReply::CursorPosition { row: 1, column: 4 })
                );
            }
        }
        // ESC c must interrupt each parser state without a cancellation byte.
        let mut terminal = state();
        let mut parser = TerminalParser::new();
        parser.advance(&mut terminal, prefix).unwrap();
        parser.advance(&mut terminal, b"\x1bcX").unwrap();
        assert_eq!(terminal.screen().cell(0, 0).unwrap().character(), 'X');
        assert_eq!(terminal.active_screen(), ScreenKind::Primary);
    }
}

#[test]
fn repeated_resets_restore_latest_cursor_default_and_clear_keyboard_stacks_on_both_screens() {
    for reset in [b"\x1b[!p".as_slice(), b"\x1bc"] {
        for screen in [b"\x1b[?47h".as_slice(), b"\x1b[?47l"] {
            let mut terminal = state();
            let mut parser = TerminalParser::new();
            let configured = CursorStyle {
                shape: CursorShape::Underline,
                blinking: false,
            };
            parser
                .advance(&mut terminal, b"\x1b[>31u\x1b[?47h\x1b[>7u")
                .unwrap();
            parser.advance(&mut terminal, screen).unwrap();
            parser.advance(&mut terminal, b"\x1b[5 q\x1b[?25l").unwrap();
            terminal.set_cursor_style_default(configured);
            terminal.resize(TerminalDimensions::new(9, 2).unwrap());
            let generation = terminal.keyboard_generation();
            parser.advance(&mut terminal, reset).unwrap();
            parser.advance(&mut terminal, reset).unwrap();
            assert_ne!(terminal.keyboard_generation(), generation);
            assert_eq!(terminal.cursor_style(), configured);
            assert_eq!(terminal.terminal_modes(), &Default::default());
            assert_eq!(
                terminal.dimensions(),
                TerminalDimensions::new(9, 2).unwrap()
            );
            for switch in [b"\x1b[?47h".as_slice(), b"\x1b[?47l"] {
                parser.advance(&mut terminal, switch).unwrap();
                parser.advance(&mut terminal, b"\x1b[<u").unwrap();
                assert_eq!(terminal.input_modes().keyboard_flags(), 0);
                assert_eq!(terminal.cursor_style(), configured);
            }
        }
    }
}

#[test]
fn arbitrary_bytes_have_bounded_error_recovery_and_ris_resynchronizes() {
    for prefix in [
        b"".as_slice(),
        b"\x1b[",
        b"\x1b]",
        b"\x1bP",
        b"\x1b_",
        b"\xe2",
    ] {
        for byte in 0..=255 {
            let mut terminal = state();
            let mut parser = TerminalParser::new();
            parser.advance(&mut terminal, prefix).unwrap();
            let input = [byte, 0x18, 0x1b, b'c', b'X'];
            let mut remaining = input.as_slice();
            // An error consumes input or clears one pending UTF-8 prefix.
            // Two attempts per input byte plus completion is a finite bound.
            for _ in 0..=input.len() * 2 {
                match parser.advance(&mut terminal, remaining) {
                    Ok(()) => {
                        remaining = &[];
                        break;
                    }
                    Err(error) => remaining = &remaining[error.bytes_consumed()..],
                }
            }
            assert!(remaining.is_empty(), "{prefix:?}/{byte}");
            assert_eq!(terminal.active_screen(), ScreenKind::Primary);
            assert_eq!(terminal.screen().cell(0, 0).unwrap().character(), 'X');
            assert_eq!(terminal.current_rendition(), &CellAttributes::default());
            assert_eq!(terminal.cursor().column(), 1);
            assert_eq!(terminal.cursor().row(), 0);
        }
    }
}
