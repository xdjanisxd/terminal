use std::{error::Error, fmt};

use crate::TerminalDimensions;

/// A cursor position that can only be changed through its owning screen grid.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Cursor {
    row: usize,
    column: usize,
}

impl Cursor {
    pub const fn row(self) -> usize {
        self.row
    }

    pub const fn column(self) -> usize {
        self.column
    }

    pub(crate) const fn origin() -> Self {
        Self { row: 0, column: 0 }
    }

    pub(crate) fn set_position(
        &mut self,
        row: usize,
        column: usize,
        dimensions: TerminalDimensions,
    ) -> Result<(), CursorError> {
        if row >= dimensions.rows() {
            return Err(CursorError::RowOutOfBounds {
                row,
                rows: dimensions.rows(),
            });
        }
        if column >= dimensions.columns() {
            return Err(CursorError::ColumnOutOfBounds {
                column,
                columns: dimensions.columns(),
            });
        }

        self.row = row;
        self.column = column;
        Ok(())
    }

    pub(crate) fn move_by(
        &mut self,
        row_delta: isize,
        column_delta: isize,
        dimensions: TerminalDimensions,
    ) {
        self.row = offset_clamped(self.row, row_delta, dimensions.rows() - 1);
        self.column = offset_clamped(self.column, column_delta, dimensions.columns() - 1);
    }

    pub(crate) fn clamp_to(&mut self, dimensions: TerminalDimensions) {
        self.row = self.row.min(dimensions.rows() - 1);
        self.column = self.column.min(dimensions.columns() - 1);
    }
}

fn offset_clamped(current: usize, delta: isize, maximum: usize) -> usize {
    if delta < 0 {
        current.saturating_sub(delta.unsigned_abs())
    } else {
        current.saturating_add(delta as usize).min(maximum)
    }
}

/// Failure to place a cursor inside the current screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CursorError {
    RowOutOfBounds { row: usize, rows: usize },
    ColumnOutOfBounds { column: usize, columns: usize },
}

impl fmt::Display for CursorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RowOutOfBounds { row, rows } => {
                write!(formatter, "cursor row {row} is outside {rows} rows")
            }
            Self::ColumnOutOfBounds { column, columns } => {
                write!(
                    formatter,
                    "cursor column {column} is outside {columns} columns"
                )
            }
        }
    }
}

impl Error for CursorError {}
