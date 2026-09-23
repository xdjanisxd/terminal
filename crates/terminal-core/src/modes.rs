/// Controls whether the terminal cursor is presented.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CursorVisibility {
    /// The cursor is visible. This is the terminal default.
    #[default]
    Visible,
    /// The cursor is hidden.
    Hidden,
}

/// Controls whether printing at the right margin may continue on the next row.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AutoWrapMode {
    /// Right-margin wrapping is enabled. This is the terminal default.
    #[default]
    Enabled,
    /// Right-margin wrapping is disabled.
    Disabled,
}

/// Controls whether new characters replace cells or insert before them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CharacterInsertionMode {
    /// New characters replace existing cells. This is the terminal default.
    #[default]
    Replace,
    /// New characters shift existing cells to the right before insertion.
    Insert,
}

/// Modes with one value shared across all terminal screens.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerminalModes {
    cursor_visibility: CursorVisibility,
    auto_wrap: AutoWrapMode,
    character_insertion: CharacterInsertionMode,
}

impl TerminalModes {
    /// Returns the current cursor visibility.
    pub const fn cursor_visibility(self) -> CursorVisibility {
        self.cursor_visibility
    }

    /// Sets whether the cursor is visible.
    pub fn set_cursor_visibility(&mut self, visibility: CursorVisibility) {
        self.cursor_visibility = visibility;
    }

    /// Returns the current auto-wrap behavior.
    pub const fn auto_wrap(self) -> AutoWrapMode {
        self.auto_wrap
    }

    /// Sets the right-margin wrapping behavior.
    pub fn set_auto_wrap(&mut self, auto_wrap: AutoWrapMode) {
        self.auto_wrap = auto_wrap;
    }

    /// Returns the current character insertion behavior.
    pub const fn character_insertion(self) -> CharacterInsertionMode {
        self.character_insertion
    }

    /// Sets whether new characters replace or insert before existing cells.
    pub fn set_character_insertion(&mut self, insertion: CharacterInsertionMode) {
        self.character_insertion = insertion;
    }
}

/// Controls how cursor-key presses are encoded.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CursorKeyMode {
    /// Cursor keys use their normal encoding. This is the terminal default.
    #[default]
    Normal,
    /// Cursor keys use application encoding.
    Application,
}

/// Which pointer events the terminal application requests.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MouseTracking {
    #[default]
    Off,
    Press,
    Drag,
    Any,
}

/// Mouse coordinate and release wire format.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MouseEncoding {
    #[default]
    Legacy,
    Sgr,
}

/// Input-related modes produced by terminal output and consumed by input encoding.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InputModes {
    cursor_keys: CursorKeyMode,
    focus_reporting: bool,
    bracketed_paste: bool,
    mouse_press: bool,
    mouse_drag: bool,
    mouse_any: bool,
    mouse_encoding: MouseEncoding,
}

impl InputModes {
    /// Returns the cursor-key encoding mode.
    pub const fn cursor_keys(self) -> CursorKeyMode {
        self.cursor_keys
    }

    /// Sets the cursor-key encoding mode.
    pub fn set_cursor_keys(&mut self, cursor_keys: CursorKeyMode) {
        self.cursor_keys = cursor_keys;
    }

    pub const fn focus_reporting(self) -> bool {
        self.focus_reporting
    }
    pub fn set_focus_reporting(&mut self, enabled: bool) {
        self.focus_reporting = enabled;
    }
    pub const fn bracketed_paste(self) -> bool {
        self.bracketed_paste
    }
    pub fn set_bracketed_paste(&mut self, enabled: bool) {
        self.bracketed_paste = enabled;
    }
    pub const fn mouse_tracking(self) -> MouseTracking {
        if self.mouse_any {
            MouseTracking::Any
        } else if self.mouse_drag {
            MouseTracking::Drag
        } else if self.mouse_press {
            MouseTracking::Press
        } else {
            MouseTracking::Off
        }
    }
    pub fn set_mouse_press(&mut self, enabled: bool) {
        self.mouse_press = enabled;
    }
    pub fn set_mouse_drag(&mut self, enabled: bool) {
        self.mouse_drag = enabled;
    }
    pub fn set_mouse_any(&mut self, enabled: bool) {
        self.mouse_any = enabled;
    }
    pub const fn mouse_encoding(self) -> MouseEncoding {
        self.mouse_encoding
    }
    pub fn set_mouse_encoding(&mut self, encoding: MouseEncoding) {
        self.mouse_encoding = encoding;
    }
}
