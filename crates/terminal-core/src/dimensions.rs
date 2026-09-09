use std::{error::Error, fmt, num::NonZeroUsize};

/// Maximum supported column count for a terminal screen.
pub const MAX_COLUMNS: usize = 4_096;
/// Maximum supported row count for a terminal screen.
pub const MAX_ROWS: usize = 4_096;
/// Maximum number of cells that one screen grid may allocate.
pub const MAX_GRID_CELLS: usize = 1_048_576;

/// Non-zero terminal dimensions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalDimensions {
    columns: NonZeroUsize,
    rows: NonZeroUsize,
}

impl TerminalDimensions {
    /// Validates terminal dimensions against non-zero and resource bounds.
    pub fn new(columns: usize, rows: usize) -> Result<Self, DimensionsError> {
        let columns = NonZeroUsize::new(columns).ok_or(DimensionsError::ZeroColumns)?;
        let rows = NonZeroUsize::new(rows).ok_or(DimensionsError::ZeroRows)?;

        if columns.get() > MAX_COLUMNS {
            return Err(DimensionsError::TooManyColumns {
                requested: columns.get(),
                maximum: MAX_COLUMNS,
            });
        }
        if rows.get() > MAX_ROWS {
            return Err(DimensionsError::TooManyRows {
                requested: rows.get(),
                maximum: MAX_ROWS,
            });
        }
        if columns.get() * rows.get() > MAX_GRID_CELLS {
            return Err(DimensionsError::TooManyCells {
                columns: columns.get(),
                rows: rows.get(),
                maximum: MAX_GRID_CELLS,
            });
        }

        Ok(Self { columns, rows })
    }

    pub const fn columns(self) -> usize {
        self.columns.get()
    }

    pub const fn rows(self) -> usize {
        self.rows.get()
    }

    pub const fn cell_count(self) -> usize {
        self.columns.get() * self.rows.get()
    }
}

/// Reason that terminal dimensions were rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DimensionsError {
    ZeroColumns,
    ZeroRows,
    TooManyColumns {
        requested: usize,
        maximum: usize,
    },
    TooManyRows {
        requested: usize,
        maximum: usize,
    },
    TooManyCells {
        columns: usize,
        rows: usize,
        maximum: usize,
    },
}

impl fmt::Display for DimensionsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroColumns => formatter.write_str("terminal columns must be non-zero"),
            Self::ZeroRows => formatter.write_str("terminal rows must be non-zero"),
            Self::TooManyColumns { requested, maximum } => write!(
                formatter,
                "terminal column count {requested} exceeds maximum {maximum}"
            ),
            Self::TooManyRows { requested, maximum } => write!(
                formatter,
                "terminal row count {requested} exceeds maximum {maximum}"
            ),
            Self::TooManyCells {
                columns,
                rows,
                maximum,
            } => write!(
                formatter,
                "terminal dimensions {columns}x{rows} exceed maximum cell count {maximum}"
            ),
        }
    }
}

impl Error for DimensionsError {}
