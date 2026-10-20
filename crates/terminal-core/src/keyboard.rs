//! Kitty keyboard protocol semantics, independent of the windowing backend.
use crate::InputModes;

pub const DISAMBIGUATE: u16 = 1;
pub const REPORT_EVENTS: u16 = 2;
pub const REPORT_ALTERNATES: u16 = 4;
pub const REPORT_ALL: u16 = 8;
pub const REPORT_TEXT: u16 = 16;
const SUPPORTED: u16 = 31;

/// Each screen has a current value and at most sixteen saved values.
#[derive(Debug, Default)]
pub(crate) struct KeyboardStack {
    saved: [u16; 16],
    len: usize,
    flags: u16,
}
impl KeyboardStack {
    pub(crate) fn flags(&self) -> u16 {
        self.flags
    }
    pub(crate) fn request(&mut self, action: u8, flags: u16, mode: u16) -> bool {
        match action {
            b'>' => {
                if self.len == self.saved.len() {
                    self.saved.copy_within(1.., 0);
                    self.len -= 1;
                }
                self.saved[self.len] = self.flags;
                self.len += 1;
                self.flags = flags & SUPPORTED;
            }
            b'<' => {
                let count = usize::from(flags.max(1));
                if count > self.len {
                    self.len = 0;
                    self.flags = 0;
                } else {
                    self.len -= count;
                    self.flags = self.saved[self.len];
                }
            }
            b'=' => match mode {
                1 => self.flags = flags & SUPPORTED,
                2 => self.flags |= flags & SUPPORTED,
                3 => self.flags &= !flags,
                _ => return false,
            },
            _ => return false,
        }
        true
    }
}

/// A layout character, or a functional key's canonical Kitty identity.
/// Functional identities use the specification's PUA values (57344 onward),
/// including Escape/Enter/Tab/Backspace at 57344..=57347. Wire aliases belong
/// to the encoder, never the platform adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyboardKey {
    Character(char),
    Functional(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyboardEventKind {
    Press,
    Repeat,
    Release,
}

/// Bit positions follow Kitty: Shift, Alt, Ctrl, Super, Hyper, Meta, Caps, Num.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KeyboardModifiers(pub u8);

#[derive(Clone, Copy, Debug)]
pub struct KeyboardEvent<'a> {
    pub key: KeyboardKey,
    pub shifted: Option<char>,
    pub base_layout: Option<char>,
    pub modifiers: KeyboardModifiers,
    pub kind: KeyboardEventKind,
    pub text: Option<&'a str>,
}

/// Common events stay inline. Long composed text can grow without truncation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyboardInput {
    inline: [u8; 64],
    len: usize,
    overflow: Option<Vec<u8>>,
}
impl Default for KeyboardInput {
    fn default() -> Self {
        Self {
            inline: [0; 64],
            len: 0,
            overflow: None,
        }
    }
}
impl KeyboardInput {
    pub fn as_slice(&self) -> &[u8] {
        self.overflow.as_deref().unwrap_or(&self.inline[..self.len])
    }
    fn push(&mut self, byte: u8) {
        if let Some(bytes) = &mut self.overflow {
            bytes.push(byte);
        } else if self.len < self.inline.len() {
            self.inline[self.len] = byte;
            self.len += 1;
        } else {
            let mut bytes = Vec::with_capacity(128);
            bytes.extend_from_slice(&self.inline);
            bytes.push(byte);
            self.overflow = Some(bytes);
        }
    }
    fn extend(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.push(*byte);
        }
    }
    fn decimal(&mut self, mut value: u32) {
        let mut digits = [0; 10];
        let mut start = digits.len();
        loop {
            start -= 1;
            digits[start] = b'0' + (value % 10) as u8;
            value /= 10;
            if value == 0 {
                break;
            }
        }
        self.extend(&digits[start..]);
    }
}

fn wire_key(key: KeyboardKey) -> (u32, u8) {
    let code = match key {
        KeyboardKey::Character(c) => return (c as u32, b'u'),
        KeyboardKey::Functional(code) => code,
    };
    match code {
        57344 => (27, b'u'),
        57345 => (13, b'u'),
        57346 => (9, b'u'),
        57347 => (127, b'u'),
        57348 => (2, b'~'),
        57349 => (3, b'~'),
        57350 => (1, b'D'),
        57351 => (1, b'C'),
        57352 => (1, b'A'),
        57353 => (1, b'B'),
        57354 => (5, b'~'),
        57355 => (6, b'~'),
        57356 => (1, b'H'),
        57357 => (1, b'F'),
        57364 => (1, b'P'),
        57365 => (1, b'Q'),
        // CSI R conflicts with cursor-position replies.
        57366 => (13, b'~'),
        57367 => (1, b'S'),
        57368 => (15, b'~'),
        57369 => (17, b'~'),
        57370 => (18, b'~'),
        57371 => (19, b'~'),
        57372 => (20, b'~'),
        57373 => (21, b'~'),
        57374 => (23, b'~'),
        57375 => (24, b'~'),
        57427 => (1, b'E'),
        _ => (u32::from(code), b'u'),
    }
}

/// None preserves the existing legacy encoder. Some(empty) suppresses an event.
/// No allocation is performed for ordinary key reports or short UTF-8 input.
pub fn encode_kitty_key(modes: InputModes, event: KeyboardEvent<'_>) -> Option<KeyboardInput> {
    let flags = modes.keyboard_flags();
    if flags == 0 {
        return None;
    }
    let all = flags & REPORT_ALL != 0;
    let disambiguate = flags & DISAMBIGUATE != 0 || all;
    let events = flags & REPORT_EVENTS != 0;
    let release = event.kind == KeyboardEventKind::Release;
    let mut output = KeyboardInput::default();
    if release && !events {
        return Some(output);
    }
    let functional = match event.key {
        KeyboardKey::Functional(c) => Some(c),
        _ => None,
    };
    if functional.is_some_and(|c| c >= 57441) && !all {
        return Some(output);
    }
    // Recovery keys remain usable at a shell prompt unless report-all is set.
    let recovery = functional.is_some_and(|c| (57345..=57347).contains(&c));
    let mods = event.modifiers.0;
    if recovery && !all && mods & 63 == 0 {
        if !release {
            output.push(wire_key(event.key).0 as u8);
        }
        return Some(output);
    }
    if recovery && !all && release {
        return Some(output);
    }
    let text = event
        .text
        .filter(|s| !s.is_empty() && s.chars().all(|c| !c.is_control()));
    let character = matches!(event.key, KeyboardKey::Character(_));
    let text_key = character
        || functional.is_some_and(|c| (57399..=57413).contains(&c) || (57415..=57416).contains(&c));
    if release && !all && text_key && mods & 62 == 0 {
        return Some(output);
    }
    // Text-producing events keep UTF-8 reporting unless report-all is enabled.
    if !all
        && text_key
        && mods & 62 == 0
        && let Some(text) = text
    {
        if !release {
            output.extend(text.as_bytes());
        }
        return Some(output);
    }
    let encode = all
        || (disambiguate && (functional.is_some() || mods & 62 != 0))
        || (events
            && ((functional.is_some() && !recovery && !text_key) || (character && mods & 62 != 0)));
    if !encode {
        if release {
            return Some(output);
        }
        return None;
    }
    let (number, suffix) = wire_key(event.key);
    output.extend(b"\x1b[");
    let alternates = flags & REPORT_ALTERNATES != 0 && suffix == b'u';
    let shifted = event.shifted.filter(|_| alternates && mods & 1 != 0);
    let base = event.base_layout.filter(|c| {
        alternates
            && Some(*c as u32)
                != match event.key {
                    KeyboardKey::Character(c) => Some(c as u32),
                    _ => None,
                }
    });
    let event_type = if events {
        match event.kind {
            KeyboardEventKind::Press => 1,
            KeyboardEventKind::Repeat => 2,
            KeyboardEventKind::Release => 3,
        }
    } else {
        1
    };
    let modifier = u32::from(mods) + 1;
    let associated = if all && flags & REPORT_TEXT != 0 && !release {
        text
    } else {
        None
    };
    if number != 1
        || suffix == b'u'
        || suffix == b'~'
        || modifier != 1
        || event_type != 1
        || associated.is_some()
    {
        output.decimal(number);
    }
    if shifted.is_some() || base.is_some() {
        output.push(b':');
        if let Some(c) = shifted {
            output.decimal(c as u32);
        }
        if let Some(c) = base {
            output.push(b':');
            output.decimal(c as u32);
        }
    }
    if modifier != 1 || event_type != 1 || associated.is_some() {
        output.push(b';');
        output.decimal(modifier);
        if event_type != 1 {
            output.push(b':');
            output.decimal(event_type);
        }
    }
    if let Some(text) = associated {
        output.push(b';');
        for (index, c) in text.chars().enumerate() {
            if index != 0 {
                output.push(b':');
            }
            output.decimal(c as u32);
        }
    }
    output.push(suffix);
    Some(output)
}
