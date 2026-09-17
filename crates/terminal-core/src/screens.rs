use std::ops::{Deref, DerefMut};

use crate::{CellAttributes, Cursor, ScreenGrid, TerminalDimensions, VerticalScrollingMargins};

/// Selects the terminal screen receiving semantic operations.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ScreenKind {
    /// The normal terminal screen, active by default.
    #[default]
    Primary,
    /// The independent alternate terminal screen.
    Alternate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SavedCursor {
    cursor: Cursor,
    rendition: CellAttributes,
}

#[derive(Debug)]
struct ScreenState {
    grid: ScreenGrid,
    vertical_scrolling_margins: VerticalScrollingMargins,
    wrap_pending: bool,
    saved_cursor: Option<SavedCursor>,
}

impl ScreenState {
    fn new(dimensions: TerminalDimensions) -> Self {
        Self {
            grid: ScreenGrid::new(dimensions),
            vertical_scrolling_margins: VerticalScrollingMargins::full_screen(dimensions.rows()),
            wrap_pending: false,
            saved_cursor: None,
        }
    }

    fn save_cursor(&mut self, rendition: CellAttributes) {
        self.saved_cursor = Some(SavedCursor {
            cursor: self.grid.cursor(),
            rendition,
        });
    }

    fn restore_cursor(&mut self) -> Option<CellAttributes> {
        let saved = self.saved_cursor?;
        let dimensions = self.grid.dimensions();
        self.grid
            .set_cursor_position(
                saved.cursor.row().min(dimensions.rows() - 1),
                saved.cursor.column().min(dimensions.columns() - 1),
            )
            .expect("clamped saved cursor is always in bounds");
        Some(saved.rendition)
    }

    fn reset(&mut self) {
        *self = Self::new(self.grid.dimensions());
    }

    fn resize(&mut self, dimensions: TerminalDimensions) {
        self.grid.resize(dimensions);
        self.vertical_scrolling_margins = VerticalScrollingMargins::full_screen(dimensions.rows());
        self.wrap_pending = false;
    }
}

/// Owns the independent primary and alternate screen-local state.
#[derive(Debug)]
pub(crate) struct ScreenSet {
    primary: ScreenState,
    alternate: ScreenState,
    active: ScreenKind,
}

impl ScreenSet {
    pub(crate) fn new(dimensions: TerminalDimensions) -> Self {
        Self {
            primary: ScreenState::new(dimensions),
            alternate: ScreenState::new(dimensions),
            active: ScreenKind::Primary,
        }
    }

    pub(crate) const fn active_kind(&self) -> ScreenKind {
        self.active
    }

    pub(crate) fn switch_to(&mut self, screen: ScreenKind) {
        self.active = screen;
    }

    pub(crate) fn enter_alternate_screen_1047(&mut self) {
        self.alternate.reset();
        self.active = ScreenKind::Alternate;
    }

    pub(crate) fn leave_alternate_screen_1047(&mut self) {
        self.active = ScreenKind::Primary;
    }

    pub(crate) fn vertical_scrolling_margins(&self) -> VerticalScrollingMargins {
        self.active_state().vertical_scrolling_margins
    }

    pub(crate) fn set_vertical_scrolling_margins(&mut self, margins: VerticalScrollingMargins) {
        self.active_state_mut().vertical_scrolling_margins = margins;
    }

    pub(crate) fn wrap_pending(&self) -> bool {
        self.active_state().wrap_pending
    }

    pub(crate) fn set_wrap_pending(&mut self, pending: bool) {
        self.active_state_mut().wrap_pending = pending;
    }

    pub(crate) fn save_cursor(&mut self, rendition: CellAttributes) {
        self.active_state_mut().save_cursor(rendition);
    }

    pub(crate) fn restore_cursor(&mut self) -> Option<CellAttributes> {
        self.active_state_mut().restore_cursor()
    }

    pub(crate) fn resize_all(&mut self, dimensions: TerminalDimensions) {
        self.primary.resize(dimensions);
        self.alternate.resize(dimensions);
    }

    fn active_state(&self) -> &ScreenState {
        match self.active {
            ScreenKind::Primary => &self.primary,
            ScreenKind::Alternate => &self.alternate,
        }
    }

    fn active_state_mut(&mut self) -> &mut ScreenState {
        match self.active {
            ScreenKind::Primary => &mut self.primary,
            ScreenKind::Alternate => &mut self.alternate,
        }
    }
}

impl Deref for ScreenSet {
    type Target = ScreenGrid;

    fn deref(&self) -> &Self::Target {
        &self.active_state().grid
    }
}

impl DerefMut for ScreenSet {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.active_state_mut().grid
    }
}
