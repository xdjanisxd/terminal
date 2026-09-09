use crate::{
    AutoWrapMode, CharacterInsertionMode, Cursor, CursorKeyMode, CursorVisibility, InputModes,
    ScreenGrid, TerminalDimensions, TerminalModes,
};

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
}

impl TerminalState {
    /// Creates a terminal in its documented default state.
    pub fn new(dimensions: TerminalDimensions) -> Self {
        Self {
            screen: ScreenGrid::new(dimensions),
            terminal_modes: TerminalModes::default(),
            input_modes: InputModes::default(),
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
        self.screen.set_cursor_position(row, column)
    }

    /// Moves the cursor by signed deltas, clamping at screen edges.
    pub fn move_cursor(&mut self, row_delta: isize, column_delta: isize) {
        self.screen.move_cursor(row_delta, column_delta);
    }

    /// Clears every active-screen cell without moving the cursor.
    pub fn clear_screen(&mut self) {
        self.screen.clear();
    }

    /// Resizes the active screen using `ScreenGrid` preservation semantics.
    pub fn resize(&mut self, dimensions: TerminalDimensions) {
        self.screen.resize(dimensions);
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
