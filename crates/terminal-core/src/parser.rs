use std::{error::Error, fmt};

use crate::{CursorMovement, EraseDirection, EraseRegion, PrintError, TerminalState};

// Compile vte without its std feature so OSC buffering uses its fixed-capacity
// ArrayVec instead of an unbounded Vec. Unsupported OSC data beyond this limit
// is discarded by vte and never reaches terminal semantics.
const MAX_OSC_BYTES: usize = 1024;

/// Incremental raw-byte parser for the supported terminal-core semantics.
///
/// Parser state is retained between calls to [`Self::advance`]. The underlying
/// parser and callback types are implementation details; callers provide bytes
/// and a [`TerminalState`] only.
///
/// This compatibility stage handles printable characters, carriage return,
/// line feed, backspace, and the supported cursor/erase CSI subset. All other
/// parser actions are safely ignored.
pub struct TerminalParser {
    parser: vte::Parser<MAX_OSC_BYTES>,
}

impl TerminalParser {
    /// Creates a parser in its initial ground state.
    pub fn new() -> Self {
        Self {
            parser: vte::Parser::new_with_size(),
        }
    }

    /// Advances the parser through one input chunk.
    ///
    /// On a terminal semantic error, parsing stops before any later parser
    /// callback can mutate state. The returned byte count is relative to
    /// `bytes` and lets the caller resume with the unconsumed suffix. It can be
    /// zero when a byte invalidates a UTF-8 prefix retained from an earlier
    /// chunk and must itself be reprocessed. This bounds error storage to one
    /// explicit error without silently discarding later errors.
    pub fn advance(
        &mut self,
        terminal: &mut TerminalState,
        bytes: &[u8],
    ) -> Result<(), TerminalParserError> {
        let mut performer = SemanticPerformer::new(terminal);

        for (index, byte) in bytes.iter().enumerate() {
            let consumed = self
                .parser
                .advance_until_terminated(&mut performer, std::slice::from_ref(byte));

            if let Some(semantic_error) = performer.semantic_error.take() {
                return Err(TerminalParserError {
                    bytes_consumed: index + consumed,
                    semantic_error,
                });
            }

            debug_assert_eq!(consumed, 1);
        }

        Ok(())
    }
}

impl Default for TerminalParser {
    fn default() -> Self {
        Self::new()
    }
}

/// A terminal semantic failure encountered while parsing an input chunk.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalParserError {
    bytes_consumed: usize,
    semantic_error: PrintError,
}

impl TerminalParserError {
    /// Returns the number of bytes consumed from the submitted chunk.
    pub const fn bytes_consumed(self) -> usize {
        self.bytes_consumed
    }

    /// Returns the project-owned terminal semantic error.
    pub const fn semantic_error(self) -> PrintError {
        self.semantic_error
    }
}

impl fmt::Display for TerminalParserError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "terminal semantic error after consuming {} input bytes: {}",
            self.bytes_consumed, self.semantic_error
        )
    }
}

impl Error for TerminalParserError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.semantic_error)
    }
}

struct SemanticPerformer<'a> {
    terminal: &'a mut TerminalState,
    semantic_error: Option<PrintError>,
}

impl<'a> SemanticPerformer<'a> {
    fn new(terminal: &'a mut TerminalState) -> Self {
        Self {
            terminal,
            semantic_error: None,
        }
    }
}

impl vte::Perform for SemanticPerformer<'_> {
    fn print(&mut self, character: char) {
        if self.semantic_error.is_none() {
            self.semantic_error = self.terminal.print_character(character).err();
        }
    }

    fn execute(&mut self, byte: u8) {
        if self.semantic_error.is_some() {
            return;
        }

        match byte {
            0x08 => self.terminal.backspace(),
            0x0A => self.terminal.line_feed(),
            0x0D => self.terminal.carriage_return(),
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        has_ignored_intermediates: bool,
        action: char,
    ) {
        if self.semantic_error.is_some() || has_ignored_intermediates || !intermediates.is_empty() {
            return;
        }

        let Some((values, count)) = simple_csi_parameters(params) else {
            return;
        };

        let movement = match action {
            'A' if count <= 1 => Some(CursorMovement::Up(default_one(values[0]))),
            'B' if count <= 1 => Some(CursorMovement::Down(default_one(values[0]))),
            'C' if count <= 1 => Some(CursorMovement::Forward(default_one(values[0]))),
            'D' if count <= 1 => Some(CursorMovement::Backward(default_one(values[0]))),
            'H' | 'f' if count <= 2 => Some(CursorMovement::Position {
                row: default_one(values[0]) - 1,
                column: default_one(values[1]) - 1,
            }),
            'G' if count <= 1 => Some(CursorMovement::HorizontalAbsolute(
                default_one(values[0]) - 1,
            )),
            'd' if count <= 1 => Some(CursorMovement::VerticalAbsolute(default_one(values[0]) - 1)),
            _ => None,
        };

        if let Some(movement) = movement {
            self.terminal.move_cursor(movement);
            return;
        }

        let direction = match (action, count, values[0]) {
            ('J' | 'K', 0 | 1, 0) => Some(EraseDirection::CursorToEnd),
            ('J' | 'K', 1, 1) => Some(EraseDirection::StartToCursor),
            ('J' | 'K', 1, 2) => Some(EraseDirection::EntireRegion),
            _ => None,
        };
        if let Some(direction) = direction {
            let region = if action == 'J' {
                EraseRegion::Display
            } else {
                EraseRegion::Line
            };
            self.terminal.erase(region, direction);
        }
    }

    fn terminated(&self) -> bool {
        self.semantic_error.is_some()
    }
}

fn simple_csi_parameters(params: &vte::Params) -> Option<([u16; 2], usize)> {
    let mut values = [0; 2];
    let mut count = 0;
    for parameter in params {
        if count == values.len() || parameter.len() != 1 {
            return None;
        }
        values[count] = parameter[0];
        count += 1;
    }
    Some((values, count))
}

fn default_one(value: u16) -> usize {
    usize::from(value.max(1))
}
