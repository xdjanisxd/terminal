use crate::{Cell, Cursor, CursorError, TerminalDimensions};

/// A bounded, row-major terminal screen grid.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScreenGrid {
    dimensions: TerminalDimensions,
    cells: Vec<Cell>,
    cursor: Cursor,
}

impl ScreenGrid {
    pub fn new(dimensions: TerminalDimensions) -> Self {
        Self {
            dimensions,
            cells: vec![Cell::default(); dimensions.cell_count()],
            cursor: Cursor::origin(),
        }
    }

    pub const fn dimensions(&self) -> TerminalDimensions {
        self.dimensions
    }

    /// Maps a zero-based `(row, column)` coordinate to its row-major index.
    pub fn index_of(&self, row: usize, column: usize) -> Option<usize> {
        if row >= self.dimensions.rows() || column >= self.dimensions.columns() {
            return None;
        }

        Some(row * self.dimensions.columns() + column)
    }

    /// Returns a cell for a zero-based `(row, column)` coordinate.
    pub fn cell(&self, row: usize, column: usize) -> Option<&Cell> {
        self.index_of(row, column)
            .and_then(|index| self.cells.get(index))
    }

    /// Returns a mutable cell for a zero-based `(row, column)` coordinate.
    pub fn cell_mut(&mut self, row: usize, column: usize) -> Option<&mut Cell> {
        let index = self.index_of(row, column)?;
        self.cells.get_mut(index)
    }

    /// Returns the cursor, which is always inside the current dimensions.
    pub const fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Moves the cursor to an in-bounds zero-based coordinate.
    pub fn set_cursor_position(&mut self, row: usize, column: usize) -> Result<(), CursorError> {
        self.cursor.set_position(row, column, self.dimensions)
    }

    /// Moves the cursor by signed deltas, clamping at screen edges.
    pub fn move_cursor(&mut self, row_delta: isize, column_delta: isize) {
        self.cursor
            .move_by(row_delta, column_delta, self.dimensions);
    }

    /// Replaces every cell with the default blank cell without moving the cursor.
    pub fn clear(&mut self) {
        self.cells.fill(Cell::default());
    }

    /// Resizes while preserving the top-left intersection of the old and new grids.
    ///
    /// Newly exposed cells are default blank cells. Cells below or to the right of
    /// the new dimensions are discarded. The cursor is clamped into the new grid.
    pub fn resize(&mut self, dimensions: TerminalDimensions) {
        if dimensions == self.dimensions {
            return;
        }

        let old_columns = self.dimensions.columns();
        let preserved_rows = self.dimensions.rows().min(dimensions.rows());
        let preserved_columns = old_columns.min(dimensions.columns());
        let mut cells = vec![Cell::default(); dimensions.cell_count()];

        for row in 0..preserved_rows {
            let old_start = row * old_columns;
            let new_start = row * dimensions.columns();
            cells[new_start..new_start + preserved_columns]
                .clone_from_slice(&self.cells[old_start..old_start + preserved_columns]);
        }

        self.dimensions = dimensions;
        self.cells = cells;
        self.cursor.clamp_to(dimensions);
    }
}
