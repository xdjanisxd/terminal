//! Platform-independent terminal screen models.

mod cell;
mod cursor;
mod dimensions;
mod grid;
mod modes;
mod parser;
mod reply;
mod scrolling;
mod state;
mod tabs;

pub use cell::{
    Cell, CellAttributes, CellColor, InverseVideo, ItalicStyle, TextIntensity, UnderlineStyle,
};
pub use cursor::{Cursor, CursorError};
pub use dimensions::{DimensionsError, MAX_COLUMNS, MAX_GRID_CELLS, MAX_ROWS, TerminalDimensions};
pub use grid::ScreenGrid;
pub use modes::{
    AutoWrapMode, CharacterInsertionMode, CursorKeyMode, CursorVisibility, InputModes,
    TerminalModes,
};
pub use parser::{TerminalParser, TerminalParserError};
pub use reply::{MAX_PENDING_REPLIES, TerminalReply, TerminalReplyBytes};
pub use scrolling::VerticalScrollingMargins;
pub use state::{CursorMovement, EraseDirection, EraseRegion, PrintError, TerminalState};
