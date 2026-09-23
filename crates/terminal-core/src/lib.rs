//! Platform-independent terminal screen models.

mod cell;
mod cursor;
mod dimensions;
mod grid;
mod input;
mod modes;
mod parser;
mod reply;
mod screens;
mod scrollback;
mod scrolling;
mod state;
mod tabs;

pub use cell::{
    Cell, CellAttributes, CellColor, CellOccupancy, InverseVideo, ItalicStyle, MAX_COMBINING_MARKS,
    TextIntensity, UnderlineStyle,
};
pub use cursor::{Cursor, CursorError};
pub use dimensions::{DimensionsError, MAX_COLUMNS, MAX_GRID_CELLS, MAX_ROWS, TerminalDimensions};
pub use grid::ScreenGrid;
pub use input::{
    CursorKey, MouseButton, MouseEvent, MouseModifiers, encode_cursor_key, encode_focus,
    encode_mouse, encode_paste,
};
pub use modes::{
    AutoWrapMode, CharacterInsertionMode, CursorKeyMode, CursorVisibility, InputModes,
    MouseEncoding, MouseTracking, TerminalModes,
};
pub use parser::{TerminalParser, TerminalParserError};
pub use reply::{MAX_PENDING_REPLIES, TerminalReply, TerminalReplyBytes};
pub use screens::ScreenKind;
pub use scrollback::MAX_SCROLLBACK_ROWS;
pub use scrolling::VerticalScrollingMargins;
pub use state::{CursorMovement, EraseDirection, EraseRegion, PrintError, TerminalState};
