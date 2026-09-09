use std::{error::Error, fmt};

use crate::{
    AutoWrapMode, Cell, CellAttributes, CharacterInsertionMode, Cursor, CursorKeyMode,
    CursorVisibility, InputModes, ScreenGrid, TerminalDimensions, TerminalModes,
};

/// Failure to print a character through the terminal semantic boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrintError {
    /// Control functions must use their dedicated semantic operation.
    ControlCharacter(char),
    /// Width-sensitive Unicode is deferred until cell width can be modeled.
    UnsupportedCharacter(char),
}

impl fmt::Display for PrintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ControlCharacter(character) => write!(
                formatter,
                "control character U+{:04X} is not printable terminal output",
                *character as u32
            ),
            Self::UnsupportedCharacter(character) => write!(
                formatter,
                "character U+{:04X} is outside the supported single-cell ASCII range",
                *character as u32
            ),
        }
    }
}

impl Error for PrintError {}

/// Parser-independent owner of the active terminal semantic state.
///
/// The facade exposes its screen and modes read-only. Mutations that affect the
/// owned state go through semantic methods on `TerminalState`.
///
/// ```compile_fail
/// use terminal_core::{TerminalDimensions, TerminalState};
///
/// let mut state = TerminalState::new(TerminalDimensions::new(80, 24).unwrap());
/// state.screen().clear();
/// ```
///
/// ```compile_fail
/// use terminal_core::{AutoWrapMode, TerminalDimensions, TerminalState};
///
/// let mut state = TerminalState::new(TerminalDimensions::new(80, 24).unwrap());
/// state
///     .terminal_modes()
///     .set_auto_wrap(AutoWrapMode::Disabled);
/// ```
///
/// ```compile_fail
/// use terminal_core::{CursorKeyMode, TerminalDimensions, TerminalState};
///
/// let mut state = TerminalState::new(TerminalDimensions::new(80, 24).unwrap());
/// state
///     .input_modes()
///     .set_cursor_keys(CursorKeyMode::Application);
/// ```
#[derive(Debug)]
pub struct TerminalState {
    screen: ScreenGrid,
    terminal_modes: TerminalModes,
    input_modes: InputModes,
    wrap_pending: bool,
}

impl TerminalState {
    /// Creates a terminal in its documented default state.
    pub fn new(dimensions: TerminalDimensions) -> Self {
        Self {
            screen: ScreenGrid::new(dimensions),
            terminal_modes: TerminalModes::default(),
            input_modes: InputModes::default(),
            wrap_pending: false,
        }
    }

    /// Returns the current validated screen dimensions.
    pub const fn dimensions(&self) -> TerminalDimensions {
        self.screen.dimensions()
    }

    /// Returns read-only access to the active screen.
    pub const fn screen(&self) -> &ScreenGrid {
        &self.screen
    }

    /// Returns the active screen's bounded cursor.
    pub const fn cursor(&self) -> Cursor {
        self.screen.cursor()
    }

    /// Moves the cursor to an in-bounds zero-based coordinate.
    pub fn set_cursor_position(
        &mut self,
        row: usize,
        column: usize,
    ) -> Result<(), crate::CursorError> {
        let result = self.screen.set_cursor_position(row, column);
        if result.is_ok() {
            self.wrap_pending = false;
        }
        result
    }

    /// Moves the cursor by signed deltas, clamping at screen edges.
    pub fn move_cursor(&mut self, row_delta: isize, column_delta: isize) {
        self.screen.move_cursor(row_delta, column_delta);
        self.wrap_pending = false;
    }

    /// Prints one supported single-cell ASCII character at the cursor.
    ///
    /// The current fixed-width model accepts ASCII space through tilde and
    /// stores each character in one cell with default attributes. Other
    /// Unicode is rejected until width and combining behavior can be modeled
    /// without approximation.
    pub fn print_character(&mut self, character: char) -> Result<(), PrintError> {
        if character.is_control() {
            return Err(PrintError::ControlCharacter(character));
        }
        if !character.is_ascii() {
            return Err(PrintError::UnsupportedCharacter(character));
        }

        if self.wrap_pending {
            self.wrap_before_print();
        }

        let cursor = self.cursor();
        let cell = Cell::new(character, CellAttributes::default());
        match self.terminal_modes.character_insertion() {
            CharacterInsertionMode::Replace => {
                *self
                    .screen
                    .cell_mut(cursor.row(), cursor.column())
                    .expect("terminal cursor is always in bounds") = cell;
            }
            CharacterInsertionMode::Insert => {
                self.screen.insert_cell(cursor.row(), cursor.column(), cell);
            }
        }

        if cursor.column() + 1 < self.dimensions().columns() {
            self.screen.move_cursor(0, 1);
        } else {
            self.wrap_pending = self.terminal_modes.auto_wrap() == AutoWrapMode::Enabled;
        }

        Ok(())
    }

    /// Moves the cursor to the first column of its current row.
    pub fn carriage_return(&mut self) {
        let row = self.cursor().row();
        self.screen
            .set_cursor_position(row, 0)
            .expect("first column is always in bounds");
        self.wrap_pending = false;
    }

    /// Moves down one row, scrolling the active screen at its bottom edge.
    ///
    /// This is an in-screen scroll only; discarded top-row cells are not kept
    /// as scrollback. The cursor column and any delayed-wrap condition are
    /// unchanged.
    pub fn line_feed(&mut self) {
        let cursor = self.cursor();
        if cursor.row() + 1 < self.dimensions().rows() {
            self.screen.move_cursor(1, 0);
        } else {
            self.screen.scroll_up_one_row();
        }
    }

    /// Moves left one column without erasing and without reverse wrapping.
    pub fn backspace(&mut self) {
        if self.cursor().column() > 0 {
            self.screen.move_cursor(0, -1);
            self.wrap_pending = false;
        }
    }

    /// Clears every active-screen cell without moving the cursor.
    pub fn clear_screen(&mut self) {
        self.screen.clear();
        self.wrap_pending = false;
    }

    /// Resizes the active screen using `ScreenGrid` preservation semantics.
    pub fn resize(&mut self, dimensions: TerminalDimensions) {
        self.screen.resize(dimensions);
        self.wrap_pending = false;
    }

    /// Returns read-only terminal-global modes.
    pub const fn terminal_modes(&self) -> &TerminalModes {
        &self.terminal_modes
    }

    /// Sets whether the cursor is visible.
    pub fn set_cursor_visibility(&mut self, visibility: CursorVisibility) {
        self.terminal_modes.set_cursor_visibility(visibility);
    }

    /// Sets the right-margin wrapping behavior.
    pub fn set_auto_wrap(&mut self, auto_wrap: AutoWrapMode) {
        self.terminal_modes.set_auto_wrap(auto_wrap);
        if auto_wrap == AutoWrapMode::Disabled {
            self.wrap_pending = false;
        }
    }

    /// Sets whether new characters replace or insert before existing cells.
    pub fn set_character_insertion(&mut self, insertion: CharacterInsertionMode) {
        self.terminal_modes.set_character_insertion(insertion);
    }

    /// Returns read-only input-related modes.
    pub const fn input_modes(&self) -> &InputModes {
        &self.input_modes
    }

    /// Sets how a future input encoder should encode cursor keys.
    pub fn set_cursor_key_mode(&mut self, cursor_keys: CursorKeyMode) {
        self.input_modes.set_cursor_keys(cursor_keys);
    }

    /// Restores this project-owned model to its initial state at the current dimensions.
    ///
    /// This clears the active screen, moves the cursor to `(0, 0)`, and restores all
    /// supported terminal and input modes to their documented defaults. Dimensions are
    /// preserved. This is not an implementation of DECSTR or RIS.
    pub fn reset(&mut self) {
        *self = Self::new(self.dimensions());
    }

    fn wrap_before_print(&mut self) {
        debug_assert_eq!(
            self.terminal_modes.auto_wrap(),
            AutoWrapMode::Enabled,
            "pending wrap is cleared when auto-wrap is disabled"
        );

        self.line_feed();
        self.carriage_return();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Cell, CellAttributes};

    #[test]
    fn reset_clears_existing_screen_content() {
        let dimensions = TerminalDimensions::new(2, 2).unwrap();
        let mut state = TerminalState::new(dimensions);
        *state.screen.cell_mut(1, 1).unwrap() = Cell::new('x', CellAttributes::default());

        state.reset();

        assert_eq!(state.screen.cell(1, 1), Some(&Cell::default()));
    }

    #[test]
    fn clear_screen_clears_owned_screen_content() {
        let dimensions = TerminalDimensions::new(2, 2).unwrap();
        let mut state = TerminalState::new(dimensions);
        *state.screen.cell_mut(0, 1).unwrap() = Cell::new('x', CellAttributes::default());

        state.clear_screen();

        assert_eq!(state.screen.cell(0, 1), Some(&Cell::default()));
    }

    #[test]
    fn resize_preserves_overlap_and_blanks_new_cells_through_facade() {
        let mut state = TerminalState::new(TerminalDimensions::new(2, 1).unwrap());
        *state.screen.cell_mut(0, 1).unwrap() = Cell::new('x', CellAttributes::default());

        state.resize(TerminalDimensions::new(3, 2).unwrap());

        assert_eq!(state.screen.cell(0, 1).unwrap().character(), 'x');
        assert_eq!(state.screen.cell(1, 2), Some(&Cell::default()));
    }
}
