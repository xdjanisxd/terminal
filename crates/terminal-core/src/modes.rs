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

/// Controls how cursor-key presses will be encoded by a future input encoder.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CursorKeyMode {
    /// Cursor keys use their normal encoding. This is the terminal default.
    #[default]
    Normal,
    /// Cursor keys use application encoding.
    Application,
}

/// Input-related modes produced by terminal output and consumed by input encoding.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InputModes {
    cursor_keys: CursorKeyMode,
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
}
