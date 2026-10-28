//! Platform key identities and routing; wire semantics live in terminal-core.
use terminal_core::{
    InputModes, KeyboardEvent, KeyboardEventKind, KeyboardInput, KeyboardKey, KeyboardModifiers,
    ScreenKind, encode_kitty_key,
};
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::{
    event::{ElementState, KeyEvent},
    keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey},
};

#[derive(Clone, Copy)]
struct Context {
    screen: ScreenKind,
    modes: InputModes,
    generation: u64,
}
impl From<&terminal_core::TerminalState> for Context {
    fn from(terminal: &terminal_core::TerminalState) -> Self {
        Self {
            screen: terminal.active_screen(),
            modes: *terminal.input_modes(),
            generation: terminal.keyboard_generation(),
        }
    }
}

#[derive(Clone, Copy)]
struct Forwarded {
    physical: PhysicalKey,
    key: KeyboardKey,
    shifted: Option<char>,
    base: Option<char>,
    screen: ScreenKind,
    generation: u64,
}

pub(super) struct KeyboardState {
    held: [Option<Forwarded>; 64],
    modifier_keys: u8,
    consumed: [Option<PhysicalKey>; 64],
    active: bool,
}
impl Default for KeyboardState {
    fn default() -> Self {
        Self {
            held: [None; 64],
            modifier_keys: 0,
            consumed: [None; 64],
            active: false,
        }
    }
}
impl KeyboardState {
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }
    pub(super) fn begin_press(&mut self, physical: PhysicalKey, repeat: bool) -> bool {
        self.active = true;
        if repeat {
            return self.consumed.contains(&Some(physical));
        }
        self.take(physical);
        if !self.consumed.contains(&Some(physical))
            && let Some(slot) = self.consumed.iter_mut().find(|key| key.is_none())
        {
            *slot = Some(physical);
        }
        false
    }
    fn unconsume(&mut self, physical: PhysicalKey) {
        for key in &mut self.consumed {
            if *key == Some(physical) {
                *key = None;
            }
        }
    }
    fn take(&mut self, physical: PhysicalKey) -> Option<Forwarded> {
        self.held
            .iter_mut()
            .find(|entry| entry.is_some_and(|held| held.physical == physical))
            .and_then(Option::take)
    }
    pub(super) fn encode(
        &mut self,
        event: &KeyEvent,
        modifiers: ModifiersState,
        terminal: &terminal_core::TerminalState,
    ) -> Option<KeyboardInput> {
        let modes = *terminal.input_modes();
        if modes.keyboard_flags() == 0 {
            if self.active {
                self.clear();
            }
            return None;
        }
        self.active = true;
        let unmodified = event.key_without_modifiers();
        self.encode_parts(
            &unmodified,
            &event.logical_key,
            event.physical_key,
            event.text_with_all_modifiers(),
            event.state,
            event.repeat,
            modifiers,
            Context::from(terminal),
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn encode_parts(
        &mut self,
        unmodified: &Key,
        logical: &Key,
        physical: PhysicalKey,
        text: Option<&str>,
        state: ElementState,
        repeat: bool,
        modifiers: ModifiersState,
        context: Context,
    ) -> Option<KeyboardInput> {
        let Context {
            screen,
            modes,
            generation,
        } = context;
        let release = state == ElementState::Released;
        self.unconsume(physical);
        let mut mods = u8::from(modifiers.shift_key())
            | (u8::from(modifiers.alt_key()) << 1)
            | (u8::from(modifiers.control_key()) << 2)
            | (u8::from(modifiers.super_key()) << 3);
        // Winit can deliver ModifiersChanged after the modifier's own key event.
        if let Some((side, flag)) = modifier_side(physical) {
            if release {
                self.modifier_keys &= !(1 << side);
            } else {
                self.modifier_keys |= 1 << side;
            }
            let pair = (1 << (side % 4)) | (1 << (side % 4 + 4));
            if self.modifier_keys & pair != 0 {
                mods |= flag;
            } else {
                mods &= !flag;
            }
        }
        let previous = if release { self.take(physical) } else { None };
        if release
            && (previous.is_none()
                || previous
                    .is_some_and(|held| held.screen != screen || held.generation != generation))
        {
            return Some(KeyboardInput::default());
        }
        if modes.keyboard_flags() == 0 {
            return None;
        }
        let key = previous
            .map(|held| held.key)
            .or_else(|| key_identity(unmodified, logical, physical))
            .or_else(|| {
                text.filter(|s| !s.is_empty() && s.chars().all(|c| !c.is_control()))
                    .map(|_| KeyboardKey::Character('\0'))
            })?;
        let shifted = previous.and_then(|held| held.shifted).or_else(|| {
            if modifiers.shift_key() {
                character(logical)
            } else {
                None
            }
        });
        let base = previous
            .and_then(|held| held.base)
            .or_else(|| base_layout(physical));
        let kind = if release {
            KeyboardEventKind::Release
        } else if repeat {
            KeyboardEventKind::Repeat
        } else {
            KeyboardEventKind::Press
        };
        let output = encode_kitty_key(
            modes,
            KeyboardEvent {
                key,
                shifted,
                base_layout: base,
                modifiers: KeyboardModifiers(mods),
                kind,
                text,
            },
        );
        if !release
            && output
                .as_ref()
                .is_some_and(|bytes| bytes.as_slice().starts_with(b"\x1b["))
        {
            self.take(physical);
            if let Some(slot) = self.held.iter_mut().find(|entry| entry.is_none()) {
                *slot = Some(Forwarded {
                    physical,
                    key,
                    shifted,
                    base,
                    screen,
                    generation,
                });
            }
        }
        output
    }
}

fn character(key: &Key) -> Option<char> {
    let Key::Character(text) = key else {
        return None;
    };
    let mut chars = text.chars();
    let first = chars.next()?;
    (chars.next().is_none() && !first.is_control()).then_some(first)
}

fn modifier_side(physical: PhysicalKey) -> Option<(u8, u8)> {
    let PhysicalKey::Code(code) = physical else {
        return None;
    };
    Some(match code {
        KeyCode::ShiftLeft => (0, 1),
        KeyCode::ControlLeft => (1, 4),
        KeyCode::AltLeft => (2, 2),
        KeyCode::SuperLeft => (3, 8),
        KeyCode::ShiftRight => (4, 1),
        KeyCode::ControlRight => (5, 4),
        KeyCode::AltRight => (6, 2),
        KeyCode::SuperRight => (7, 8),
        _ => return None,
    })
}

fn key_identity(unmodified: &Key, logical: &Key, physical: PhysicalKey) -> Option<KeyboardKey> {
    if let Some((side, _)) = modifier_side(physical) {
        let code = [57441, 57442, 57443, 57444, 57447, 57448, 57449, 57450][side as usize];
        return Some(KeyboardKey::Functional(code));
    }
    if let PhysicalKey::Code(code) = physical {
        let keypad = match code {
            KeyCode::Numpad0 => Some(57399),
            KeyCode::Numpad1 => Some(57400),
            KeyCode::Numpad2 => Some(57401),
            KeyCode::Numpad3 => Some(57402),
            KeyCode::Numpad4 => Some(57403),
            KeyCode::Numpad5 => Some(57404),
            KeyCode::Numpad6 => Some(57405),
            KeyCode::Numpad7 => Some(57406),
            KeyCode::Numpad8 => Some(57407),
            KeyCode::Numpad9 => Some(57408),
            KeyCode::NumpadDecimal => Some(57409),
            KeyCode::NumpadDivide => Some(57410),
            KeyCode::NumpadMultiply => Some(57411),
            KeyCode::NumpadSubtract => Some(57412),
            KeyCode::NumpadAdd => Some(57413),
            KeyCode::NumpadEnter => Some(57414),
            KeyCode::NumpadEqual => Some(57415),
            KeyCode::NumpadComma => Some(57416),
            KeyCode::NumpadClear => Some(57427),
            _ => None,
        };
        if let Some(mut code) = keypad {
            // NumLock-off keypad navigation retains a distinct keypad identity.
            if let Key::Named(named) = logical {
                code = match named {
                    NamedKey::ArrowLeft => 57417,
                    NamedKey::ArrowRight => 57418,
                    NamedKey::ArrowUp => 57419,
                    NamedKey::ArrowDown => 57420,
                    NamedKey::PageUp => 57421,
                    NamedKey::PageDown => 57422,
                    NamedKey::Home => 57423,
                    NamedKey::End => 57424,
                    NamedKey::Insert => 57425,
                    NamedKey::Delete => 57426,
                    _ => code,
                };
            }
            return Some(KeyboardKey::Functional(code));
        }
    }
    if let Some(c) = character(unmodified) {
        return Some(KeyboardKey::Character(c));
    }
    let named = match unmodified {
        Key::Named(named) => named,
        _ => match logical {
            Key::Named(named) => named,
            _ => return None,
        },
    };
    Some(KeyboardKey::Functional(match named {
        NamedKey::Escape => 57344,
        NamedKey::Enter => 57345,
        NamedKey::Tab => 57346,
        NamedKey::Backspace => 57347,
        NamedKey::Insert => 57348,
        NamedKey::Delete => 57349,
        NamedKey::ArrowLeft => 57350,
        NamedKey::ArrowRight => 57351,
        NamedKey::ArrowUp => 57352,
        NamedKey::ArrowDown => 57353,
        NamedKey::PageUp => 57354,
        NamedKey::PageDown => 57355,
        NamedKey::Home => 57356,
        NamedKey::End => 57357,
        NamedKey::CapsLock => 57358,
        NamedKey::ScrollLock => 57359,
        NamedKey::NumLock => 57360,
        NamedKey::PrintScreen => 57361,
        NamedKey::Pause => 57362,
        NamedKey::ContextMenu => 57363,
        NamedKey::F1 => 57364,
        NamedKey::F2 => 57365,
        NamedKey::F3 => 57366,
        NamedKey::F4 => 57367,
        NamedKey::F5 => 57368,
        NamedKey::F6 => 57369,
        NamedKey::F7 => 57370,
        NamedKey::F8 => 57371,
        NamedKey::F9 => 57372,
        NamedKey::F10 => 57373,
        NamedKey::F11 => 57374,
        NamedKey::F12 => 57375,
        NamedKey::F13 => 57376,
        NamedKey::F14 => 57377,
        NamedKey::F15 => 57378,
        NamedKey::F16 => 57379,
        NamedKey::F17 => 57380,
        NamedKey::F18 => 57381,
        NamedKey::F19 => 57382,
        NamedKey::F20 => 57383,
        NamedKey::F21 => 57384,
        NamedKey::F22 => 57385,
        NamedKey::F23 => 57386,
        NamedKey::F24 => 57387,
        NamedKey::F25 => 57388,
        NamedKey::F26 => 57389,
        NamedKey::F27 => 57390,
        NamedKey::F28 => 57391,
        NamedKey::F29 => 57392,
        NamedKey::F30 => 57393,
        NamedKey::F31 => 57394,
        NamedKey::F32 => 57395,
        NamedKey::F33 => 57396,
        NamedKey::F34 => 57397,
        NamedKey::F35 => 57398,
        NamedKey::MediaPlay => 57428,
        NamedKey::MediaPause => 57429,
        NamedKey::MediaPlayPause => 57430,
        NamedKey::MediaStop => 57432,
        NamedKey::MediaFastForward => 57433,
        NamedKey::MediaRewind => 57434,
        NamedKey::MediaTrackNext => 57435,
        NamedKey::MediaTrackPrevious => 57436,
        NamedKey::MediaRecord => 57437,
        NamedKey::AudioVolumeDown => 57438,
        NamedKey::AudioVolumeUp => 57439,
        NamedKey::AudioVolumeMute => 57440,
        NamedKey::Hyper => 57445,
        NamedKey::Meta => 57446,
        NamedKey::AltGraph => 57453,
        _ => return None,
    }))
}

fn base_layout(physical: PhysicalKey) -> Option<char> {
    let PhysicalKey::Code(code) = physical else {
        return None;
    };
    Some(match code {
        KeyCode::KeyA => 'a',
        KeyCode::KeyB => 'b',
        KeyCode::KeyC => 'c',
        KeyCode::KeyD => 'd',
        KeyCode::KeyE => 'e',
        KeyCode::KeyF => 'f',
        KeyCode::KeyG => 'g',
        KeyCode::KeyH => 'h',
        KeyCode::KeyI => 'i',
        KeyCode::KeyJ => 'j',
        KeyCode::KeyK => 'k',
        KeyCode::KeyL => 'l',
        KeyCode::KeyM => 'm',
        KeyCode::KeyN => 'n',
        KeyCode::KeyO => 'o',
        KeyCode::KeyP => 'p',
        KeyCode::KeyQ => 'q',
        KeyCode::KeyR => 'r',
        KeyCode::KeyS => 's',
        KeyCode::KeyT => 't',
        KeyCode::KeyU => 'u',
        KeyCode::KeyV => 'v',
        KeyCode::KeyW => 'w',
        KeyCode::KeyX => 'x',
        KeyCode::KeyY => 'y',
        KeyCode::KeyZ => 'z',
        KeyCode::Digit0 => '0',
        KeyCode::Digit1 => '1',
        KeyCode::Digit2 => '2',
        KeyCode::Digit3 => '3',
        KeyCode::Digit4 => '4',
        KeyCode::Digit5 => '5',
        KeyCode::Digit6 => '6',
        KeyCode::Digit7 => '7',
        KeyCode::Digit8 => '8',
        KeyCode::Digit9 => '9',
        KeyCode::Space => ' ',
        KeyCode::Backquote => '`',
        KeyCode::Minus => '-',
        KeyCode::Equal => '=',
        KeyCode::BracketLeft => '[',
        KeyCode::BracketRight => ']',
        KeyCode::Backslash => '\\',
        KeyCode::Semicolon => ';',
        KeyCode::Quote => '\'',
        KeyCode::Comma => ',',
        KeyCode::Period => '.',
        KeyCode::Slash => '/',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_core::{TerminalDimensions, TerminalParser, TerminalState};
    fn terminal(flags: u16) -> TerminalState {
        let mut state = TerminalState::new(TerminalDimensions::new(80, 24).unwrap());
        TerminalParser::new()
            .advance(&mut state, format!("\x1b[={flags}u").as_bytes())
            .unwrap();
        state
    }
    fn character_key(s: &str) -> Key {
        Key::Character(s.into())
    }
    fn run(
        state: &mut KeyboardState,
        terminal: &TerminalState,
        code: KeyCode,
        key: &Key,
        modifiers: ModifiersState,
        released: bool,
        repeat: bool,
    ) -> Vec<u8> {
        state
            .encode_parts(
                key,
                key,
                PhysicalKey::Code(code),
                None,
                if released {
                    ElementState::Released
                } else {
                    ElementState::Pressed
                },
                repeat,
                modifiers,
                Context::from(terminal),
            )
            .unwrap()
            .as_slice()
            .to_vec()
    }
    #[test]
    fn kitty_layout_identity_keeps_control_i_and_m_distinct() {
        let terminal = terminal(1);
        let mut state = KeyboardState::default();
        for (code, key, mods, expected) in [
            (
                KeyCode::KeyI,
                character_key("i"),
                ModifiersState::CONTROL,
                b"\x1b[105;5u".as_slice(),
            ),
            (
                KeyCode::Tab,
                Key::Named(NamedKey::Tab),
                ModifiersState::empty(),
                b"\t",
            ),
            (
                KeyCode::KeyM,
                character_key("m"),
                ModifiersState::CONTROL,
                b"\x1b[109;5u",
            ),
            (
                KeyCode::Enter,
                Key::Named(NamedKey::Enter),
                ModifiersState::empty(),
                b"\r",
            ),
        ] {
            assert_eq!(
                run(&mut state, &terminal, code, &key, mods, false, false),
                expected
            );
        }
    }

    #[test]
    fn kitty_modified_horizontal_arrows_preserve_complete_press_and_release_sequences() {
        for flags in [1, 3] {
            for application_cursor in [false, true] {
                let mut terminal = terminal(flags);
                if application_cursor {
                    TerminalParser::new()
                        .advance(&mut terminal, b"\x1b[?1h")
                        .unwrap();
                }
                for (code, named, press, release) in [
                    (
                        KeyCode::ArrowLeft,
                        NamedKey::ArrowLeft,
                        b"\x1b[1;6D".as_slice(),
                        b"\x1b[1;6:3D".as_slice(),
                    ),
                    (
                        KeyCode::ArrowRight,
                        NamedKey::ArrowRight,
                        b"\x1b[1;6C".as_slice(),
                        b"\x1b[1;6:3C".as_slice(),
                    ),
                ] {
                    let mut state = KeyboardState::default();
                    let key = Key::Named(named);
                    let modifiers = ModifiersState::CONTROL | ModifiersState::SHIFT;
                    assert_eq!(
                        run(&mut state, &terminal, code, &key, modifiers, false, false),
                        press
                    );
                    assert_eq!(
                        run(&mut state, &terminal, code, &key, modifiers, true, false),
                        if flags == 3 { release } else { b"" }
                    );
                }
            }
        }
    }
    #[test]
    fn kitty_release_requires_forwarded_press_and_survives_repeat() {
        let terminal = terminal(11);
        let mut state = KeyboardState::default();
        let key = character_key("a");
        assert!(
            run(
                &mut state,
                &terminal,
                KeyCode::KeyA,
                &key,
                ModifiersState::empty(),
                true,
                false
            )
            .is_empty()
        );
        assert_eq!(
            run(
                &mut state,
                &terminal,
                KeyCode::KeyA,
                &key,
                ModifiersState::empty(),
                false,
                false
            ),
            b"\x1b[97u"
        );
        assert_eq!(
            run(
                &mut state,
                &terminal,
                KeyCode::KeyA,
                &key,
                ModifiersState::empty(),
                false,
                true
            ),
            b"\x1b[97;1:2u"
        );
        assert_eq!(
            run(
                &mut state,
                &terminal,
                KeyCode::KeyA,
                &key,
                ModifiersState::empty(),
                true,
                false
            ),
            b"\x1b[97;1:3u"
        );
    }
    #[test]
    fn kitty_modal_shortcuts_keep_repeat_and_release_ownership() {
        let terminal = terminal(11);
        let mut state = KeyboardState::default();
        let physical = PhysicalKey::Code(KeyCode::KeyA);
        assert!(!state.begin_press(physical, false));
        // UI consumed the press; no encoder call. A later repeat stays consumed.
        assert!(state.begin_press(physical, true));
        assert!(
            run(
                &mut state,
                &terminal,
                KeyCode::KeyA,
                &character_key("a"),
                ModifiersState::empty(),
                true,
                false
            )
            .is_empty()
        );
        assert!(!state.begin_press(physical, false));
        assert_eq!(
            run(
                &mut state,
                &terminal,
                KeyCode::KeyA,
                &character_key("a"),
                ModifiersState::empty(),
                false,
                false
            ),
            b"\x1b[97u"
        );
        assert!(!state.begin_press(physical, true));
    }

    #[test]
    fn kitty_search_shortcut_consumes_repeat_and_release() {
        let terminal = terminal(3);
        let mut state = KeyboardState::default();
        let physical = PhysicalKey::Code(KeyCode::KeyF);
        assert!(!state.begin_press(physical, false));
        assert!(state.begin_press(physical, true));
        assert!(
            run(
                &mut state,
                &terminal,
                KeyCode::KeyF,
                &character_key("f"),
                ModifiersState::CONTROL,
                true,
                false,
            )
            .is_empty()
        );
    }
    #[test]
    fn kitty_held_state_cannot_leak_across_screens_resets_or_focus() {
        for transition in [
            b"\x1b[?47h\x1b[=11u\x1b[?47l".as_slice(),
            b"\x1b[!p\x1b[=11u",
            b"\x1bc\x1b[=11u",
        ] {
            let mut terminal = terminal(11);
            let mut state = KeyboardState::default();
            let key = character_key("a");
            run(
                &mut state,
                &terminal,
                KeyCode::KeyA,
                &key,
                ModifiersState::empty(),
                false,
                false,
            );
            TerminalParser::new()
                .advance(&mut terminal, transition)
                .unwrap();
            assert!(
                run(
                    &mut state,
                    &terminal,
                    KeyCode::KeyA,
                    &key,
                    ModifiersState::empty(),
                    true,
                    false
                )
                .is_empty()
            );
        }
        let terminal = terminal(11);
        let mut state = KeyboardState::default();
        let key = character_key("a");
        run(
            &mut state,
            &terminal,
            KeyCode::KeyA,
            &key,
            ModifiersState::empty(),
            false,
            false,
        );
        state.clear();
        assert!(
            run(
                &mut state,
                &terminal,
                KeyCode::KeyA,
                &key,
                ModifiersState::empty(),
                true,
                false
            )
            .is_empty()
        );
    }
    #[test]
    fn kitty_modifier_own_event_has_post_event_state_with_both_sides() {
        let terminal = terminal(11);
        let mut state = KeyboardState::default();
        let key = Key::Named(NamedKey::Shift);
        assert_eq!(
            run(
                &mut state,
                &terminal,
                KeyCode::ShiftLeft,
                &key,
                ModifiersState::empty(),
                false,
                false
            ),
            b"\x1b[57441;2u"
        );
        assert_eq!(
            run(
                &mut state,
                &terminal,
                KeyCode::ShiftRight,
                &key,
                ModifiersState::SHIFT,
                false,
                false
            ),
            b"\x1b[57447;2u"
        );
        assert_eq!(
            run(
                &mut state,
                &terminal,
                KeyCode::ShiftLeft,
                &key,
                ModifiersState::SHIFT,
                true,
                false
            ),
            b"\x1b[57441;2:3u"
        );
        assert_eq!(
            run(
                &mut state,
                &terminal,
                KeyCode::ShiftRight,
                &key,
                ModifiersState::SHIFT,
                true,
                false
            ),
            b"\x1b[57447;1:3u"
        );
    }
    #[test]
    fn kitty_shifted_and_base_layout_key_and_keypad_identity() {
        let terminal = terminal(31);
        let mut state = KeyboardState::default();
        let bytes = state
            .encode_parts(
                &character_key("ж"),
                &character_key("Ж"),
                PhysicalKey::Code(KeyCode::KeyV),
                Some("Ж"),
                ElementState::Pressed,
                false,
                ModifiersState::SHIFT,
                Context::from(&terminal),
            )
            .unwrap();
        assert_eq!(bytes.as_slice(), b"\x1b[1078:1046:118;2;1046u");
        assert_eq!(
            run(
                &mut state,
                &terminal,
                KeyCode::NumpadEnter,
                &Key::Named(NamedKey::Enter),
                ModifiersState::empty(),
                false,
                false
            ),
            b"\x1b[57414u"
        );
        assert_eq!(
            run(
                &mut state,
                &terminal,
                KeyCode::Numpad4,
                &Key::Named(NamedKey::ArrowLeft),
                ModifiersState::empty(),
                false,
                false
            ),
            b"\x1b[57417u"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires installed Neovim; real PTY protocol acceptance, no GUI key injection"]
    fn kitty_neovim_native_pty_distinguishes_control_i_and_m() {
        use std::{
            ffi::OsString,
            time::{Duration, Instant},
        };
        use terminal_pty::{
            PortablePtyBackend, PtyBackend, PtyOutput, PtySize, PtySpawnConfig, PtyWorker,
            PtyWorkerEvent,
        };
        let root =
            std::env::temp_dir().join(format!("terminal-kitty-neovim-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let lua = root.join("keys.lua");
        let result = root.join("result.txt");
        std::fs::write(&lua, "_G.kitty_keys={}\nfor key,label in pairs({['<Tab>']='tab',['<C-I>']='ctrl_i',['<CR>']='enter',['<C-M>']='ctrl_m'}) do vim.keymap.set('n',key,function() table.insert(_G.kitty_keys,label) end) end\n").unwrap();
        let config = PtySpawnConfig::new("/usr/bin/nvim".into(), PtySize::new(24, 80).unwrap())
            .with_arguments([
                OsString::from("-u"),
                OsString::from("NONE"),
                OsString::from("-i"),
                OsString::from("NONE"),
                OsString::from("-n"),
                OsString::from("-c"),
                OsString::from(format!("luafile {}", lua.display())),
            ])
            .with_environment([
                (OsString::from("TERM"), OsString::from("xterm-256color")),
                (
                    OsString::from("NVIM_LOG_FILE"),
                    root.join("nvim.log").into_os_string(),
                ),
            ]);
        let mut worker =
            PtyWorker::start(PortablePtyBackend::new().spawn(config).unwrap()).unwrap();
        let mut app = crate::Application::default();
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut raw = Vec::new();
        while Instant::now() < deadline && app.terminal.input_modes().keyboard_flags() == 0 {
            if let Some(PtyWorkerEvent::Output(PtyOutput::Bytes(bytes))) =
                worker.recv_timeout(Duration::from_millis(30)).unwrap()
            {
                raw.extend_from_slice(&bytes);
                for reply in
                    crate::parse_terminal_output(&mut app.parser, &mut app.terminal, &bytes)
                {
                    worker.write(reply).unwrap();
                }
            }
        }
        std::fs::write(root.join("startup.bin"), &raw).unwrap();
        assert_ne!(
            app.terminal.input_modes().keyboard_flags(),
            0,
            "Neovim must negotiate Kitty; raw output in {}",
            root.display()
        );
        for (code, key, mods) in [
            (
                KeyCode::Tab,
                Key::Named(NamedKey::Tab),
                ModifiersState::empty(),
            ),
            (KeyCode::KeyI, character_key("i"), ModifiersState::CONTROL),
            (
                KeyCode::Enter,
                Key::Named(NamedKey::Enter),
                ModifiersState::empty(),
            ),
            (KeyCode::KeyM, character_key("m"), ModifiersState::CONTROL),
        ] {
            let bytes = run(
                &mut app.keyboard,
                &app.terminal,
                code,
                &key,
                mods,
                false,
                false,
            );
            eprintln!("Neovim Kitty input {code:?}: {bytes:?}");
            worker.write_input(&bytes).unwrap();
        }
        worker
            .write(
                format!(
                    ":lua vim.fn.writefile(_G.kitty_keys, '{}'); vim.cmd('qa!')\r",
                    result.display()
                )
                .into_bytes(),
            )
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(8);
        while Instant::now() < deadline
            && (!result.exists()
                || app.terminal.active_screen() != ScreenKind::Primary
                || app.terminal.input_modes().keyboard_flags() != 0)
        {
            if let Some(PtyWorkerEvent::Output(PtyOutput::Bytes(bytes))) =
                worker.recv_timeout(Duration::from_millis(30)).unwrap()
            {
                raw.extend_from_slice(&bytes);
                for reply in
                    crate::parse_terminal_output(&mut app.parser, &mut app.terminal, &bytes)
                {
                    worker.write(reply).unwrap();
                }
            }
        }
        worker.shutdown_and_join().unwrap();
        std::fs::write(root.join("session.bin"), raw).unwrap();
        assert_eq!(app.terminal.active_screen(), ScreenKind::Primary);
        assert_eq!(app.terminal.input_modes().keyboard_flags(), 0);
        assert_eq!(
            std::fs::read_to_string(&result).unwrap(),
            "tab\nctrl_i\nenter\nctrl_m\n"
        );
        eprintln!(
            "Neovim native PTY identities passed; evidence {}",
            root.display()
        );
    }
}
