//! Encode input against modes selected by terminal output.

use crate::{CursorKeyMode, InputModes, MouseEncoding, MouseTracking};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CursorKey {
    Up,
    Down,
    Right,
    Left,
}

pub fn encode_cursor_key(modes: InputModes, key: CursorKey) -> &'static [u8] {
    match (modes.cursor_keys(), key) {
        (CursorKeyMode::Normal, CursorKey::Up) => b"\x1b[A",
        (CursorKeyMode::Normal, CursorKey::Down) => b"\x1b[B",
        (CursorKeyMode::Normal, CursorKey::Right) => b"\x1b[C",
        (CursorKeyMode::Normal, CursorKey::Left) => b"\x1b[D",
        (CursorKeyMode::Application, CursorKey::Up) => b"\x1bOA",
        (CursorKeyMode::Application, CursorKey::Down) => b"\x1bOB",
        (CursorKeyMode::Application, CursorKey::Right) => b"\x1bOC",
        (CursorKeyMode::Application, CursorKey::Left) => b"\x1bOD",
    }
}

pub fn encode_focus(modes: InputModes, focused: bool) -> Option<&'static [u8]> {
    modes
        .focus_reporting()
        .then_some(if focused { b"\x1b[I" } else { b"\x1b[O" })
}

pub fn encode_paste(modes: InputModes, text: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(text.len() + if modes.bracketed_paste() { 12 } else { 0 });
    if modes.bracketed_paste() {
        bytes.extend_from_slice(b"\x1b[200~");
    }
    bytes.extend_from_slice(text.as_bytes());
    if modes.bracketed_paste() {
        bytes.extend_from_slice(b"\x1b[201~");
    }
    bytes
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

impl MouseButton {
    fn code(self) -> u8 {
        match self {
            Self::Left => 0,
            Self::Middle => 1,
            Self::Right => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseEvent {
    Press(MouseButton),
    Release(MouseButton),
    Move(Option<MouseButton>),
    WheelUp,
    WheelDown,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MouseModifiers {
    pub shift: bool,
    pub alt: bool,
    pub control: bool,
}

/// Coordinates are zero-based terminal cells. Out-of-range legacy reports are omitted.
pub fn encode_mouse(
    modes: InputModes,
    event: MouseEvent,
    column: usize,
    row: usize,
    modifiers: MouseModifiers,
) -> Option<Vec<u8>> {
    let tracking = modes.mouse_tracking();
    if tracking == MouseTracking::Off {
        return None;
    }
    let (base, release) = match event {
        MouseEvent::Press(button) => (button.code(), false),
        MouseEvent::Release(button) => (button.code(), true),
        MouseEvent::Move(button) if tracking == MouseTracking::Any => {
            (button.map_or(3, MouseButton::code) + 32, false)
        }
        MouseEvent::Move(Some(button)) if tracking == MouseTracking::Drag => {
            (button.code() + 32, false)
        }
        MouseEvent::Move(_) => return None,
        MouseEvent::WheelUp => (64, false),
        MouseEvent::WheelDown => (65, false),
    };
    let flags = (u8::from(modifiers.shift) * 4)
        | (u8::from(modifiers.alt) * 8)
        | (u8::from(modifiers.control) * 16);
    let code = base | flags;
    match modes.mouse_encoding() {
        MouseEncoding::Sgr => {
            let column = column.checked_add(1)?;
            let row = row.checked_add(1)?;
            let suffix = if release { 'm' } else { 'M' };
            Some(format!("\x1b[<{code};{column};{row}{suffix}").into_bytes())
        }
        MouseEncoding::Legacy => {
            let column = u8::try_from(column.checked_add(33)?).ok()?;
            let row = u8::try_from(row.checked_add(33)?).ok()?;
            let code = if release { 3 | flags } else { code };
            Some(vec![0x1b, b'[', b'M', code + 32, column, row])
        }
    }
}
