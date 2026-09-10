//! Platform-independent terminal screen models.

mod cell;
mod cursor;
mod dimensions;
mod grid;
mod modes;
mod parser;
mod state;
mod tabs;

pub use cell::{Cell, CellAttributes, CellColor};
pub use cursor::{Cursor, CursorError};
pub use dimensions::{DimensionsError, MAX_COLUMNS, MAX_GRID_CELLS, MAX_ROWS, TerminalDimensions};
pub use grid::ScreenGrid;
pub use modes::{
    AutoWrapMode, CharacterInsertionMode, CursorKeyMode, CursorVisibility, InputModes,
    TerminalModes,
};
pub use parser::{TerminalParser, TerminalParserError};
pub use state::{CursorMovement, EraseDirection, EraseRegion, PrintError, TerminalState};
