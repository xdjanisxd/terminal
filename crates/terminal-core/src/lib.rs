//! Platform-independent terminal screen models.

mod cell;
mod cursor;
mod dimensions;
mod grid;
mod modes;
mod state;

pub use cell::{Cell, CellAttributes, CellColor};
pub use cursor::{Cursor, CursorError};
pub use dimensions::{DimensionsError, MAX_COLUMNS, MAX_GRID_CELLS, MAX_ROWS, TerminalDimensions};
pub use grid::ScreenGrid;
pub use modes::{
    AutoWrapMode, CharacterInsertionMode, CursorKeyMode, CursorVisibility, InputModes,
    TerminalModes,
};
pub use state::{PrintError, TerminalState};
