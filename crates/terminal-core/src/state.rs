use std::{error::Error, fmt};

use unicode_width::UnicodeWidthChar;

use crate::reply::PendingReplies;
use crate::tabs::HorizontalTabStops;
use crate::{
    AutoWrapMode, Cell, CellAttributes, CellColor, CharacterInsertionMode, Cursor, CursorKeyMode,
    CursorVisibility, InputModes, InverseVideo, ItalicStyle, ScreenGrid, TerminalDimensions,
    TerminalModes, TerminalReply, TextIntensity, UnderlineStyle, VerticalScrollingMargins,
};

/// Failure to print a character through the terminal semantic boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrintError {
    /// Control functions must use their dedicated semantic operation.
    ControlCharacter(char),
    /// Width-zero characters remain deferred until combining behavior is modeled.
    UnsupportedZeroWidthCharacter(char),
    /// Character width is not representable by the current one- or two-cell model.
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
            Self::UnsupportedZeroWidthCharacter(character) => write!(
                formatter,
                "width-zero character U+{:04X} requires deferred combining behavior",
                *character as u32
            ),
            Self::UnsupportedCharacter(character) => write!(
                formatter,
                "character U+{:04X} has unsupported terminal cell width",
                *character as u32
            ),
        }
    }
}

impl Error for PrintError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrintableWidth {
    One,
    Two,
}

impl PrintableWidth {
    fn classify(character: char) -> Result<Self, PrintError> {
        if character.is_control() {
            return Err(PrintError::ControlCharacter(character));
        }
        if character == '\u{FFFD}' {
            return Err(PrintError::UnsupportedCharacter(character));
        }

        match character.width() {
            Some(1) => Ok(Self::One),
            Some(2) => Ok(Self::Two),
            Some(0) => Err(PrintError::UnsupportedZeroWidthCharacter(character)),
            _ => Err(PrintError::UnsupportedCharacter(character)),
        }
    }
}

/// Parser-independent cursor movement over zero-based terminal coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CursorMovement {
    Up(usize),
    Down(usize),
    Forward(usize),
    Backward(usize),
    Position { row: usize, column: usize },
    HorizontalAbsolute(usize),
    VerticalAbsolute(usize),
}

/// The terminal area affected by an erase operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EraseRegion {
    Display,
    Line,
}

/// Direction of an inclusive erase relative to the cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EraseDirection {
    CursorToEnd,
    StartToCursor,
    EntireRegion,
}

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
/// ```compile_fail
/// use terminal_core::{TerminalDimensions, TerminalState, VerticalScrollingMargins};
///
/// let mut state = TerminalState::new(TerminalDimensions::new(80, 24).unwrap());
/// state.vertical_scrolling_margins = VerticalScrollingMargins::full_screen(24);
/// ```
#[derive(Debug)]
pub struct TerminalState {
    screen: ScreenGrid,
    terminal_modes: TerminalModes,
    input_modes: InputModes,
    current_rendition: CellAttributes,
    horizontal_tab_stops: HorizontalTabStops,
    vertical_scrolling_margins: VerticalScrollingMargins,
    pending_replies: PendingReplies,
    wrap_pending: bool,
}

impl TerminalState {
    /// Creates a terminal in its documented default state.
    pub fn new(dimensions: TerminalDimensions) -> Self {
        Self {
            screen: ScreenGrid::new(dimensions),
            terminal_modes: TerminalModes::default(),
            input_modes: InputModes::default(),
            current_rendition: CellAttributes::default(),
            horizontal_tab_stops: HorizontalTabStops::new(dimensions.columns()),
            vertical_scrolling_margins: VerticalScrollingMargins::full_screen(dimensions.rows()),
            pending_replies: PendingReplies::default(),
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

    /// Returns the inclusive zero-based vertical scrolling margins.
    pub const fn vertical_scrolling_margins(&self) -> VerticalScrollingMargins {
        self.vertical_scrolling_margins
    }

    /// Returns the number of replies waiting for caller consumption.
    pub const fn pending_reply_count(&self) -> usize {
        self.pending_replies.len()
    }

    /// Removes and returns the oldest pending terminal reply.
    pub fn take_reply(&mut self) -> Option<TerminalReply> {
        self.pending_replies.pop()
    }

    /// Queues the conservative primary Device Attributes response.
    ///
    /// Returns `false` without replacing an older reply when the fixed-capacity
    /// reply queue is full.
    #[must_use]
    pub fn request_primary_device_attributes(&mut self) -> bool {
        self.pending_replies
            .push(TerminalReply::PrimaryDeviceAttributes)
    }

    /// Queues the conservative fixed secondary Device Attributes response.
    ///
    /// Returns `false` without replacing an older reply when the fixed-capacity
    /// reply queue is full.
    #[must_use]
    pub fn request_secondary_device_attributes(&mut self) -> bool {
        self.pending_replies
            .push(TerminalReply::SecondaryDeviceAttributes)
    }

    /// Queues the fixed ANSI terminal-status response.
    ///
    /// Returns `false` without replacing an older reply when the fixed-capacity
    /// reply queue is full.
    #[must_use]
    pub fn request_terminal_status(&mut self) -> bool {
        self.pending_replies.push(TerminalReply::TerminalStatus)
    }

    /// Queues a cursor-position report using the current absolute screen coordinates.
    ///
    /// The reply captures one-based coordinates at request time. It returns
    /// `false` without changing terminal state when the fixed-capacity reply
    /// queue is full.
    #[must_use]
    pub fn request_cursor_position_report(&mut self) -> bool {
        let cursor = self.cursor();
        let row = u16::try_from(cursor.row() + 1)
            .expect("validated terminal dimensions fit cursor-position reply coordinates");
        let column = u16::try_from(cursor.column() + 1)
            .expect("validated terminal dimensions fit cursor-position reply coordinates");
        self.pending_replies
            .push(TerminalReply::CursorPosition { row, column })
    }

    /// Replaces the vertical scrolling margins when they are valid for the screen.
    ///
    /// A successful update homes the cursor at the screen origin and cancels
    /// delayed wrap. Invalid input leaves all terminal state unchanged. The
    /// representation permits a one-row region so one-row screens remain valid.
    pub fn set_vertical_scrolling_margins(&mut self, top: usize, bottom: usize) -> bool {
        let Some(margins) = VerticalScrollingMargins::new(top, bottom, self.dimensions().rows())
        else {
            return false;
        };

        self.vertical_scrolling_margins = margins;
        self.screen
            .set_cursor_position(0, 0)
            .expect("screen origin is always in bounds");
        self.wrap_pending = false;
        true
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

    /// Applies a typed cursor movement, clamping at screen edges.
    pub fn move_cursor(&mut self, movement: CursorMovement) {
        let cursor = self.cursor();
        let dimensions = self.dimensions();
        let maximum_row = dimensions.rows() - 1;
        let maximum_column = dimensions.columns() - 1;
        let (row, column) = match movement {
            CursorMovement::Up(amount) => (cursor.row().saturating_sub(amount), cursor.column()),
            CursorMovement::Down(amount) => (
                cursor.row().saturating_add(amount).min(maximum_row),
                cursor.column(),
            ),
            CursorMovement::Forward(amount) => (
                cursor.row(),
                cursor.column().saturating_add(amount).min(maximum_column),
            ),
            CursorMovement::Backward(amount) => {
                (cursor.row(), cursor.column().saturating_sub(amount))
            }
            CursorMovement::Position { row, column } => {
                (row.min(maximum_row), column.min(maximum_column))
            }
            CursorMovement::HorizontalAbsolute(column) => {
                (cursor.row(), column.min(maximum_column))
            }
            CursorMovement::VerticalAbsolute(row) => (row.min(maximum_row), cursor.column()),
        };

        self.screen
            .set_cursor_position(row, column)
            .expect("clamped semantic cursor position is always in bounds");
        self.wrap_pending = false;
    }

    /// Moves down by a bounded count and returns to the first column without scrolling.
    ///
    /// This is CNL-style explicit cursor positioning, not NEL: it uses the
    /// ordinary screen bounds even when scrolling margins are active. Zero is
    /// normalized to one for callers using terminal control-function counts.
    pub fn cursor_next_line(&mut self, rows: usize) {
        self.move_cursor(CursorMovement::Down(rows.max(1)));
        self.carriage_return();
    }

    /// Moves up by a bounded count and returns to the first column without scrolling.
    ///
    /// This is CPL-style explicit cursor positioning, not RI: it uses the
    /// ordinary screen bounds even when scrolling margins are active. Zero is
    /// normalized to one for callers using terminal control-function counts.
    pub fn cursor_previous_line(&mut self, rows: usize) {
        self.move_cursor(CursorMovement::Up(rows.max(1)));
        self.carriage_return();
    }

    /// Prints one Unicode scalar whose display width is one or two cells.
    ///
    /// Width-zero combining behavior remains deferred. A two-cell character that
    /// cannot fit at the right margin wraps before writing only when auto-wrap is
    /// enabled; otherwise it is ignored without changing the grid or cursor.
    pub fn print_character(&mut self, character: char) -> Result<(), PrintError> {
        let width = PrintableWidth::classify(character)?;

        if self.wrap_pending {
            self.wrap_before_print();
        }

        let cursor = self.cursor();
        let columns = self.dimensions().columns();
        if width == PrintableWidth::Two && cursor.column() + 1 >= columns {
            if self.terminal_modes.auto_wrap() == AutoWrapMode::Disabled {
                return Ok(());
            }
            self.wrap_before_print();
        }

        let cursor = self.cursor();
        match (self.terminal_modes.character_insertion(), width) {
            (CharacterInsertionMode::Replace, PrintableWidth::One) => self.screen.write_single(
                cursor.row(),
                cursor.column(),
                character,
                self.current_rendition,
            ),
            (CharacterInsertionMode::Replace, PrintableWidth::Two) => self.screen.write_wide(
                cursor.row(),
                cursor.column(),
                character,
                self.current_rendition,
            ),
            (CharacterInsertionMode::Insert, PrintableWidth::One) => self.screen.insert_cells(
                cursor.row(),
                cursor.column(),
                1,
                Cell::new(character, self.current_rendition),
            ),
            (CharacterInsertionMode::Insert, PrintableWidth::Two) => self.screen.insert_wide(
                cursor.row(),
                cursor.column(),
                character,
                self.current_rendition,
            ),
        }

        let final_column = columns - 1;
        if width == PrintableWidth::Two && cursor.column() + 1 == final_column {
            self.screen.move_cursor(0, 1);
            self.wrap_pending = self.terminal_modes.auto_wrap() == AutoWrapMode::Enabled;
        } else if cursor.column() < final_column {
            self.screen.move_cursor(0, 1);
        } else {
            self.wrap_pending = self.terminal_modes.auto_wrap() == AutoWrapMode::Enabled;
        }

        Ok(())
    }

    /// Inserts canonical blank cells from the cursor through the final column.
    ///
    /// The count is normalized so zero inserts one cell, then clamped by the
    /// bounded grid width. This explicit current-row editing operation preserves
    /// the cursor but cancels delayed wrap, matching erase and other cursor edits.
    pub fn insert_characters(&mut self, count: usize) {
        let cursor = self.cursor();
        self.screen
            .insert_cells(cursor.row(), cursor.column(), count.max(1), Cell::default());
        self.wrap_pending = false;
    }

    /// Deletes cells from the cursor through the final column.
    ///
    /// The count is normalized so zero deletes one cell, then clamped by the
    /// bounded grid width. This explicit current-row editing operation preserves
    /// the cursor but cancels delayed wrap, matching ICH and erase.
    pub fn delete_characters(&mut self, count: usize) {
        let cursor = self.cursor();
        self.screen
            .delete_cells(cursor.row(), cursor.column(), count.max(1));
        self.wrap_pending = false;
    }

    /// Erases canonical blank cells beginning at the cursor without shifting cells.
    ///
    /// The count is normalized so zero erases one cell, then clamped by the
    /// bounded current-row suffix. This explicit current-row editing operation
    /// preserves the cursor but cancels delayed wrap, matching existing erase,
    /// ICH, and DCH behavior.
    pub fn erase_characters(&mut self, count: usize) {
        let cursor = self.cursor();
        let columns = self.dimensions().columns();
        let start = cursor.row() * columns + cursor.column();
        let width = count.max(1).min(columns - cursor.column());
        self.screen.erase_cells(start, start + width);
        self.wrap_pending = false;
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
            self.screen.scroll_up(1);
        }
    }

    /// Advances one row, scrolling the active region at its bottom margin.
    ///
    /// Outside the scrolling region, movement remains bounded by the screen and
    /// never scrolls. The cursor column is preserved and delayed wrap is cancelled.
    pub fn index(&mut self) {
        let cursor_row = self.cursor().row();
        if cursor_row == self.vertical_scrolling_margins.bottom() {
            self.screen
                .scroll_region_up(self.vertical_scrolling_margins, 1);
        } else if cursor_row + 1 < self.dimensions().rows() {
            self.screen.move_cursor(1, 0);
        }
        self.wrap_pending = false;
    }

    /// Moves up one row, scrolling the active region at its top margin.
    ///
    /// Outside the scrolling region, movement remains bounded by the screen and
    /// never scrolls. The cursor column is preserved and delayed wrap is cancelled.
    pub fn reverse_index(&mut self) {
        let cursor_row = self.cursor().row();
        if cursor_row == self.vertical_scrolling_margins.top() {
            self.screen
                .scroll_region_down(self.vertical_scrolling_margins, 1);
        } else if cursor_row > 0 {
            self.screen.move_cursor(-1, 0);
        }
        self.wrap_pending = false;
    }

    /// Advances one row using index semantics and returns to the first column.
    pub fn next_line(&mut self) {
        self.index();
        self.carriage_return();
    }

    /// Scrolls the active vertical scrolling region upward without moving the cursor.
    ///
    /// The count is clamped to the region height. Newly exposed rows use the
    /// canonical default blank cell. Current rendition and delayed wrap are
    /// preserved.
    pub fn scroll_up(&mut self, rows: usize) {
        self.screen
            .scroll_region_up(self.vertical_scrolling_margins, rows);
    }

    /// Scrolls the active vertical scrolling region downward without moving the cursor.
    ///
    /// The count is clamped to the region height. Newly exposed rows use the
    /// canonical default blank cell. Current rendition and delayed wrap are
    /// preserved.
    pub fn scroll_down(&mut self, rows: usize) {
        self.screen
            .scroll_region_down(self.vertical_scrolling_margins, rows);
    }

    /// Inserts blank lines from the cursor through the bottom active margin.
    ///
    /// This is a no-op when the cursor is outside the active scrolling region.
    /// The bounded subregion keeps rows above the cursor unchanged; its count
    /// is clamped and exposed rows use canonical default blank cells. Delayed
    /// wrap is preserved because this operation does not move the cursor.
    pub fn insert_lines(&mut self, rows: usize) {
        let cursor_row = self.cursor().row();
        let margins = self.vertical_scrolling_margins;
        if cursor_row < margins.top() || cursor_row > margins.bottom() {
            return;
        }

        let affected =
            VerticalScrollingMargins::new(cursor_row, margins.bottom(), self.dimensions().rows())
                .expect("cursor and active bottom margin define an in-bounds subregion");
        self.screen.scroll_region_down(affected, rows);
    }

    /// Deletes lines from the cursor through the bottom active margin.
    ///
    /// This is a no-op when the cursor is outside the active scrolling region.
    /// The bounded subregion keeps rows above the cursor unchanged; its count
    /// is clamped and exposed rows use canonical default blank cells. Delayed
    /// wrap is preserved because this operation does not move the cursor.
    pub fn delete_lines(&mut self, rows: usize) {
        let cursor_row = self.cursor().row();
        let margins = self.vertical_scrolling_margins;
        if cursor_row < margins.top() || cursor_row > margins.bottom() {
            return;
        }

        let affected =
            VerticalScrollingMargins::new(cursor_row, margins.bottom(), self.dimensions().rows())
                .expect("cursor and active bottom margin define an in-bounds subregion");
        self.screen.scroll_region_up(affected, rows);
    }

    /// Moves left one column without erasing and without reverse wrapping.
    pub fn backspace(&mut self) {
        if self.cursor().column() > 0 {
            self.screen.move_cursor(0, -1);
            self.wrap_pending = false;
        }
    }

    /// Moves to the next horizontal tab stop or the final column.
    ///
    /// A stop at the current column is skipped. The operation changes neither
    /// cells nor modes and cancels any delayed right-margin wrap.
    pub fn horizontal_tab(&mut self) {
        let cursor = self.cursor();
        let final_column = self.dimensions().columns() - 1;
        let column = self
            .horizontal_tab_stops
            .next_after(cursor.column())
            .unwrap_or(final_column);
        self.screen
            .set_cursor_position(cursor.row(), column)
            .expect("tab target is always in bounds");
        self.wrap_pending = false;
    }

    /// Returns whether an in-bounds zero-based column has a horizontal tab stop.
    pub fn has_horizontal_tab_stop(&self, column: usize) -> bool {
        self.horizontal_tab_stops.has(column)
    }

    /// Sets a horizontal tab stop at the current cursor column.
    pub fn set_horizontal_tab_stop(&mut self) {
        self.horizontal_tab_stops.set(self.cursor().column());
    }

    /// Clears the horizontal tab stop at the current cursor column.
    pub fn clear_horizontal_tab_stop(&mut self) {
        self.horizontal_tab_stops.clear(self.cursor().column());
    }

    /// Clears all horizontal tab stops.
    pub fn clear_all_horizontal_tab_stops(&mut self) {
        self.horizontal_tab_stops.clear_all();
    }

    /// Clears every active-screen cell without moving the cursor.
    pub fn clear_screen(&mut self) {
        self.screen.clear();
        self.wrap_pending = false;
    }

    /// Erases default blank cells in the selected region without moving the cursor.
    pub fn erase(&mut self, region: EraseRegion, direction: EraseDirection) {
        let cursor = self.cursor();
        let columns = self.dimensions().columns();
        let cursor_index = cursor.row() * columns + cursor.column();
        let (region_start, region_end) = match region {
            EraseRegion::Display => (0, self.dimensions().cell_count()),
            EraseRegion::Line => {
                let start = cursor.row() * columns;
                (start, start + columns)
            }
        };
        let (start, end) = match direction {
            EraseDirection::CursorToEnd => (cursor_index, region_end),
            EraseDirection::StartToCursor => (region_start, cursor_index + 1),
            EraseDirection::EntireRegion => (region_start, region_end),
        };

        self.screen.erase_cells(start, end);
        self.wrap_pending = false;
    }

    /// Resizes the active screen, horizontal tab stops, and scrolling margins.
    ///
    /// Existing stops in surviving columns are preserved. Stops beyond a
    /// shrunken width are discarded, while newly exposed columns receive the
    /// conventional default stops. Vertical scrolling margins reset to the
    /// full height of the resized screen.
    pub fn resize(&mut self, dimensions: TerminalDimensions) {
        self.screen.resize(dimensions);
        self.horizontal_tab_stops.resize(dimensions.columns());
        self.vertical_scrolling_margins = VerticalScrollingMargins::full_screen(dimensions.rows());
        self.wrap_pending = false;
    }

    /// Returns the current rendition copied into newly printed cells.
    pub const fn current_rendition(&self) -> &CellAttributes {
        &self.current_rendition
    }

    /// Restores the complete current rendition to documented defaults.
    pub fn reset_rendition(&mut self) {
        self.current_rendition = CellAttributes::default();
    }

    /// Sets the current text intensity.
    pub fn set_text_intensity(&mut self, intensity: TextIntensity) {
        self.current_rendition.set_intensity(intensity);
    }

    /// Sets the current italic style.
    pub fn set_italic_style(&mut self, italic: ItalicStyle) {
        self.current_rendition.set_italic(italic);
    }

    /// Sets the current underline style.
    pub fn set_underline_style(&mut self, underline: UnderlineStyle) {
        self.current_rendition.set_underline(underline);
    }

    /// Sets whether current output uses inverse video.
    pub fn set_inverse_video(&mut self, inverse: InverseVideo) {
        self.current_rendition.set_inverse(inverse);
    }

    /// Sets the current foreground color.
    pub fn set_foreground_color(&mut self, foreground: CellColor) {
        self.current_rendition.set_foreground(foreground);
    }

    /// Sets the current background color.
    pub fn set_background_color(&mut self, background: CellColor) {
        self.current_rendition.set_background(background);
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
    /// supported terminal and input modes to their documented defaults. Dimensions and
    /// already-generated pending replies are preserved. This is not an implementation
    /// of DECSTR or RIS.
    pub fn reset(&mut self) {
        let pending_replies = std::mem::take(&mut self.pending_replies);
        *self = Self::new(self.dimensions());
        self.pending_replies = pending_replies;
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
