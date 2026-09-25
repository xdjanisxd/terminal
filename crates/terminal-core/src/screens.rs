use std::collections::HashSet;
use std::ops::{Deref, DerefMut};

use crate::{
    Cell, CellAttributes, Cursor, ScreenGrid, TerminalDimensions, VerticalScrollingMargins,
    scrollback::Scrollback,
};

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
    row_wraps: Vec<bool>,
    saved_cursor: Option<SavedCursor>,
    scrollback: Option<Scrollback>,
    viewport_offset: usize,
    history_origin: usize,
}

impl ScreenState {
    fn new(dimensions: TerminalDimensions, owns_scrollback: bool) -> Self {
        Self {
            grid: ScreenGrid::new(dimensions),
            vertical_scrolling_margins: VerticalScrollingMargins::full_screen(dimensions.rows()),
            wrap_pending: false,
            row_wraps: vec![false; dimensions.rows()],
            saved_cursor: None,
            scrollback: owns_scrollback.then(Scrollback::new),
            viewport_offset: 0,
            history_origin: 0,
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
        let owns_scrollback = self.scrollback.is_some();
        *self = Self::new(self.grid.dimensions(), owns_scrollback);
    }

    fn resize(&mut self, dimensions: TerminalDimensions) {
        self.grid.resize(dimensions);
        self.row_wraps.resize(dimensions.rows(), false);
        self.vertical_scrolling_margins = VerticalScrollingMargins::full_screen(dimensions.rows());
        self.wrap_pending = false;
        self.viewport_offset = self.viewport_offset.min(self.scrollback_len());
    }

    fn scroll_region_up(&mut self, margins: VerticalScrollingMargins, rows: usize) {
        let height = margins.bottom() - margins.top() + 1;
        let rows = rows.min(height);
        if rows == 0 {
            return;
        }

        if margins == VerticalScrollingMargins::full_screen(self.grid.dimensions().rows())
            && let Some(scrollback) = &mut self.scrollback
        {
            for row in margins.top()..margins.top() + rows {
                let displaced = self
                    .grid
                    .row(row)
                    .expect("scrolling margin row is always in bounds");
                let full = scrollback.len() == crate::MAX_SCROLLBACK_ROWS;
                scrollback.push(displaced, self.row_wraps[row]);
                if full {
                    self.history_origin += 1;
                }
            }
            // History is inserted immediately above the live grid. Keeping the
            // same rows in view therefore requires moving the viewport away from
            // bottom by each captured row, including after oldest-row eviction.
            if self.viewport_offset != 0 {
                self.viewport_offset = self
                    .viewport_offset
                    .saturating_add(rows)
                    .min(scrollback.len());
            }
        }

        self.grid.scroll_region_up(margins, rows);
        self.shift_row_wraps(margins, rows, true);
    }

    fn scroll_region_down(&mut self, margins: VerticalScrollingMargins, rows: usize) {
        self.grid.scroll_region_down(margins, rows);
        self.shift_row_wraps(margins, rows, false);
    }

    fn scroll_region_up_without_history(&mut self, margins: VerticalScrollingMargins, rows: usize) {
        self.grid.scroll_region_up(margins, rows);
        self.shift_row_wraps(margins, rows, true);
    }

    fn shift_row_wraps(&mut self, margins: VerticalScrollingMargins, rows: usize, up: bool) {
        if margins.bottom() >= self.row_wraps.len() {
            return;
        }
        let start = margins.top();
        let end = margins.bottom() + 1;
        let count = rows.min(end - start);
        if count == 0 {
            return;
        }
        if count == end - start {
            self.row_wraps[start..end].fill(false);
        } else if up {
            self.row_wraps.copy_within(start + count..end, start);
            self.row_wraps[end - count..end].fill(false);
        } else {
            self.row_wraps
                .copy_within(start..end - count, start + count);
            self.row_wraps[start..start + count].fill(false);
        }
    }

    fn scrollback_len(&self) -> usize {
        self.scrollback.as_ref().map_or(0, Scrollback::len)
    }

    fn scrollback_row(&self, index: usize) -> Option<&[Cell]> {
        self.scrollback.as_ref()?.row(index)
    }

    fn row_wraps_from_previous(&self, index: usize) -> Option<bool> {
        if index < self.scrollback_len() {
            self.scrollback.as_ref()?.wraps_from_previous(index)
        } else {
            self.row_wraps.get(index - self.scrollback_len()).copied()
        }
    }

    fn viewport_offset(&self) -> usize {
        self.viewport_offset
    }

    fn page_up(&mut self) -> bool {
        self.scroll_viewport_rows(self.grid.dimensions().rows() as i32)
    }

    fn page_down(&mut self) -> bool {
        self.scroll_viewport_rows(-(self.grid.dimensions().rows() as i32))
    }

    fn return_to_live_viewport(&mut self) -> bool {
        let changed = self.viewport_offset != 0;
        self.viewport_offset = 0;
        changed
    }

    fn scroll_viewport_rows(&mut self, rows: i32) -> bool {
        let previous = self.viewport_offset;
        if rows >= 0 {
            self.viewport_offset = self
                .viewport_offset
                .saturating_add(rows as usize)
                .min(self.scrollback_len());
        } else {
            self.viewport_offset = self
                .viewport_offset
                .saturating_sub(rows.unsigned_abs() as usize);
        }
        self.viewport_offset != previous
    }

    fn viewport_cell(&self, row: usize, column: usize) -> Option<&Cell> {
        let dimensions = self.grid.dimensions();
        if row >= dimensions.rows() || column >= dimensions.columns() {
            return None;
        }
        let source_row = self.scrollback_len() + row - self.viewport_offset;
        if source_row < self.scrollback_len() {
            return self.scrollback_row(source_row)?.get(column);
        }
        self.grid.cell(source_row - self.scrollback_len(), column)
    }

    fn selection_cell(&self, row: usize, column: usize) -> Option<&Cell> {
        let row = row.checked_sub(self.history_origin)?;
        if row < self.scrollback_len() {
            self.scrollback_row(row)?.get(column)
        } else {
            self.grid.cell(row - self.scrollback_len(), column)
        }
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
    pub(crate) fn target_ids(&self) -> HashSet<u64> {
        let mut ids = HashSet::new();
        for screen in [&self.primary, &self.alternate] {
            for row in 0..screen.grid.dimensions().rows() {
                if let Some(cells) = screen.grid.row(row) {
                    ids.extend(cells.iter().filter_map(|cell| cell.target_id()));
                }
            }
            if let Some(history) = &screen.scrollback {
                for row in 0..history.len() {
                    if let Some(cells) = history.row(row) {
                        ids.extend(cells.iter().filter_map(|cell| cell.target_id()));
                    }
                }
            }
        }
        ids
    }
    pub(crate) fn new(dimensions: TerminalDimensions) -> Self {
        Self {
            primary: ScreenState::new(dimensions, true),
            alternate: ScreenState::new(dimensions, false),
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

    pub(crate) fn scroll_region_up(&mut self, margins: VerticalScrollingMargins, rows: usize) {
        self.active_state_mut().scroll_region_up(margins, rows);
    }

    pub(crate) fn scroll_region_up_without_history(
        &mut self,
        margins: VerticalScrollingMargins,
        rows: usize,
    ) {
        self.active_state_mut()
            .scroll_region_up_without_history(margins, rows);
    }

    pub(crate) fn scroll_region_down(&mut self, margins: VerticalScrollingMargins, rows: usize) {
        self.active_state_mut().scroll_region_down(margins, rows);
    }

    pub(crate) fn set_row_wraps_from_previous(&mut self, row: usize, wraps: bool) {
        if let Some(slot) = self.active_state_mut().row_wraps.get_mut(row) {
            *slot = wraps;
        }
    }

    pub(crate) fn clear_row_wraps(&mut self) {
        self.active_state_mut().row_wraps.fill(false);
    }

    pub(crate) fn primary_row_wraps_from_previous(&self, index: usize) -> Option<bool> {
        self.primary.row_wraps_from_previous(index)
    }

    pub(crate) fn primary_scrollback_len(&self) -> usize {
        self.primary.scrollback_len()
    }

    pub(crate) fn primary_scrollback_row(&self, index: usize) -> Option<&[Cell]> {
        self.primary.scrollback_row(index)
    }

    pub(crate) fn viewport_offset(&self) -> usize {
        self.active_state().viewport_offset()
    }

    pub(crate) fn page_up(&mut self) -> bool {
        self.active_state_mut().page_up()
    }

    pub(crate) fn page_down(&mut self) -> bool {
        self.active_state_mut().page_down()
    }

    pub(crate) fn return_to_live_viewport(&mut self) -> bool {
        self.active_state_mut().return_to_live_viewport()
    }

    pub(crate) fn scroll_viewport_rows(&mut self, rows: i32) -> bool {
        self.active_state_mut().scroll_viewport_rows(rows)
    }

    pub(crate) fn viewport_cell(&self, row: usize, column: usize) -> Option<&Cell> {
        self.active_state().viewport_cell(row, column)
    }

    pub(crate) fn selection_cell(&self, row: usize, column: usize) -> Option<&Cell> {
        self.active_state().selection_cell(row, column)
    }

    pub(crate) fn selection_history_origin(&self) -> usize {
        self.active_state().history_origin
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
