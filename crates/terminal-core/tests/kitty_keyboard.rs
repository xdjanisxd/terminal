use terminal_core::*;

fn terminal(flags: u16) -> TerminalState {
    let mut state = TerminalState::new(TerminalDimensions::new(80, 24).unwrap());
    feed(&mut state, format!("\x1b[={flags}u").as_bytes());
    state
}
fn feed(state: &mut TerminalState, bytes: &[u8]) {
    TerminalParser::new().advance(state, bytes).unwrap();
}
fn event(key: KeyboardKey, mods: u8, kind: KeyboardEventKind) -> KeyboardEvent<'static> {
    KeyboardEvent {
        key,
        shifted: None,
        base_layout: None,
        modifiers: KeyboardModifiers(mods),
        kind,
        text: None,
    }
}
fn encoded(flags: u16, event: KeyboardEvent<'_>) -> Vec<u8> {
    encode_kitty_key(*terminal(flags).input_modes(), event)
        .unwrap()
        .as_slice()
        .to_vec()
}
use KeyboardEventKind::{Press, Release, Repeat};
use KeyboardKey::{Character as C, Functional as F};

#[test]
fn query_flags_and_device_attributes_preserve_reply_order_and_fragmentation() {
    let mut state = terminal(0);
    let mut parser = TerminalParser::new();
    for byte in b"\x1b[?u\x1b[c\x1b[=65535u\x1b[?u" {
        parser.advance(&mut state, &[*byte]).unwrap();
    }
    for expected in [b"\x1b[?0u".as_slice(), b"\x1b[?1;0c", b"\x1b[?31u"] {
        assert_eq!(state.take_reply().unwrap().as_bytes().as_slice(), expected);
    }
    assert_eq!(state.pending_reply_count(), 0);
}

#[test]
fn nested_push_restores_set_value_and_modes_apply_only_to_current_value() {
    let mut state = terminal(1);
    for (request, expected) in [
        ("\x1b[>3u", 3),
        ("\x1b[=4;2u", 7),
        ("\x1b[=2;3u", 5),
        ("\x1b[>u", 0),
        ("\x1b[<u", 5),
        ("\x1b[<1u", 1),
        ("\x1b[<9u", 0),
    ] {
        feed(&mut state, request.as_bytes());
        assert_eq!(state.input_modes().keyboard_flags(), expected);
    }
}

#[test]
fn stack_overflow_evicts_oldest_saved_value_and_underflow_resets() {
    let mut state = terminal(0);
    for i in 1..=20 {
        feed(&mut state, format!("\x1b[>{i}u").as_bytes());
    }
    feed(&mut state, b"\x1b[<16u");
    assert_eq!(state.input_modes().keyboard_flags(), 4);
    feed(&mut state, b"\x1b[<u");
    assert_eq!(state.input_modes().keyboard_flags(), 0);
}

#[test]
fn screens_and_nested_exit_restore_independent_modes() {
    let mut state = terminal(1);
    feed(&mut state, b"\x1b[>3u\x1b[?1049h\x1b[>11u\x1b[>31u\x1b[<u");
    assert_eq!(state.input_modes().keyboard_flags(), 11);
    feed(&mut state, b"\x1b[<u\x1b[?1049l");
    assert_eq!(state.input_modes().keyboard_flags(), 3);
    feed(&mut state, b"\x1b[<u");
    assert_eq!(state.input_modes().keyboard_flags(), 1);
    feed(&mut state, b"\x1b[?47h");
    assert_eq!(state.input_modes().keyboard_flags(), 0);
}

#[test]
fn reset_invalidates_held_events_and_both_screen_stacks() {
    for reset in [b"\x1b[!p".as_slice(), b"\x1bc"] {
        let mut state = terminal(31);
        let before = state.keyboard_generation();
        feed(&mut state, b"\x1b[?47h\x1b[>7u");
        feed(&mut state, reset);
        assert_eq!(state.input_modes().keyboard_flags(), 0);
        assert_ne!(state.keyboard_generation(), before);
        feed(&mut state, b"\x1b[?47l\x1b[<u");
        assert_eq!(state.input_modes().keyboard_flags(), 0);
    }
}

#[test]
fn malformed_requests_are_ignored_and_queries_are_bounded() {
    let mut state = terminal(1);
    for bytes in [
        b"\x1b[=3;9u".as_slice(),
        b"\x1b[>3;2u",
        b"\x1b[=3:2u",
        b"\x1b[<1;2u",
    ] {
        feed(&mut state, bytes);
        assert_eq!(state.input_modes().keyboard_flags(), 1);
    }
    for _ in 0..100 {
        feed(&mut state, b"\x1b[?u");
    }
    assert_eq!(state.pending_reply_count(), MAX_PENDING_REPLIES);
}

#[test]
fn tab_and_enter_remain_distinct_from_control_letters() {
    for (key, mods, expected) in [
        (F(57346), 0, b"\t".as_slice()),
        (C('i'), 4, b"\x1b[105;5u"),
        (F(57345), 0, b"\r"),
        (C('m'), 4, b"\x1b[109;5u"),
        (F(57346), 5, b"\x1b[9;6u"),
        (F(57344), 0, b"\x1b[27u"),
        (F(57347), 4, b"\x1b[127;5u"),
    ] {
        assert_eq!(encoded(1, event(key, mods, Press)), expected);
    }
}

#[test]
fn every_modifier_combination_and_unicode_identity_is_encoded() {
    for mods in 0..=255 {
        assert_eq!(
            encoded(8, event(C('ж'), mods, Press)),
            format!(
                "\x1b[1078{}u",
                if mods == 0 {
                    String::new()
                } else {
                    format!(";{}", u16::from(mods) + 1)
                }
            )
            .as_bytes()
        );
    }
}

#[test]
fn event_reporting_preserves_recovery_keys_and_text_without_report_all() {
    for key in [F(57345), F(57346), F(57347)] {
        assert!(encoded(3, event(key, 0, Release)).is_empty());
        assert_eq!(
            encoded(3, event(key, 0, Repeat)),
            vec![match key {
                F(57345) => 13,
                F(57346) => 9,
                _ => 127,
            }]
        );
    }
    assert_eq!(encoded(3, event(C('i'), 4, Repeat)), b"\x1b[105;5:2u");
    assert_eq!(encoded(3, event(C('i'), 4, Release)), b"\x1b[105;5:3u");
    assert!(encoded(1, event(C('i'), 4, Release)).is_empty());
    assert!(encoded(3, event(C('a'), 0, Release)).is_empty());
    assert_eq!(encoded(11, event(F(57345), 0, Release)), b"\x1b[13;1:3u");
    assert_eq!(encoded(11, event(C('a'), 0, Repeat)), b"\x1b[97;1:2u");
}

#[test]
fn canonical_function_key_and_keypad_codes_including_f3() {
    for (key, expected) in [
        (57352, b"\x1b[A".as_slice()),
        (57364, b"\x1b[P"),
        (57366, b"\x1b[13~"),
        (57375, b"\x1b[24~"),
        (57376, b"\x1b[57376u"),
        (57398, b"\x1b[57398u"),
        (57414, b"\x1b[57414u"),
        (57427, b"\x1b[E"),
    ] {
        assert_eq!(encoded(1, event(F(key), 0, Press)), expected);
    }
    assert_eq!(encoded(3, event(F(57352), 5, Release)), b"\x1b[1;6:3A");
    assert!(encoded(3, event(F(57441), 1, Press)).is_empty());
    assert_eq!(encoded(11, event(F(57441), 1, Press)), b"\x1b[57441;2u");
}

#[test]
fn alternates_and_associated_text_obey_flags_and_exclude_release_text() {
    let mut e = event(C('ж'), 5, Press);
    e.shifted = Some('Ж');
    e.base_layout = Some('v');
    e.text = Some("Ж");
    assert_eq!(encoded(31, e), b"\x1b[1078:1046:118;6;1046u");
    e.kind = Release;
    assert_eq!(encoded(31, e), b"\x1b[1078:1046:118;6:3u");
    e.kind = Press;
    e.modifiers = KeyboardModifiers(4);
    assert_eq!(encoded(5, e), b"\x1b[1078::118;5u");
    e.text = Some("\x01");
    assert_eq!(encoded(24, e), b"\x1b[1078;5u");
    e.text = Some("e\u{301}");
    e.modifiers = KeyboardModifiers(0);
    assert_eq!(encoded(24, e), b"\x1b[1078;1;101:769u");
    let long = "界".repeat(100);
    e.text = Some(&long);
    let bytes = encoded(24, e);
    assert_eq!(bytes.iter().filter(|c| **c == b':').count(), 99);
    assert!(bytes.ends_with(b"30028u"));
}

#[test]
fn inactive_protocol_and_pure_alternate_flag_leave_legacy_untouched() {
    for flags in [0, 4, 16] {
        for key in [C('a'), C('i'), F(57345), F(57364)] {
            assert!(
                encode_kitty_key(*terminal(flags).input_modes(), event(key, 4, Press)).is_none()
            );
        }
    }
}

#[test]
fn printable_keypad_text_stays_utf8_until_all_keys_are_requested() {
    let mut e = event(F(57400), 0, Press);
    e.text = Some("1");
    assert_eq!(encoded(1, e), b"1");
    assert_eq!(encoded(3, e), b"1");
    assert_eq!(encoded(11, e), b"\x1b[57400u");
    assert_eq!(encoded(27, e), b"\x1b[57400;1;49u");
    e.kind = Release;
    e.text = None;
    assert!(encoded(3, e).is_empty());
    assert_eq!(encoded(11, e), b"\x1b[57400;1:3u");
}
