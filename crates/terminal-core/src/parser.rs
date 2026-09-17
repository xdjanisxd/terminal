use std::{error::Error, fmt};

use crate::{
    AutoWrapMode, CellColor, CharacterInsertionMode, CursorKeyMode, CursorMovement,
    CursorVisibility, EraseDirection, EraseRegion, InverseVideo, ItalicStyle, PrintError,
    TerminalState, TextIntensity, UnderlineStyle,
};

// Compile vte without its std feature so OSC buffering uses its fixed-capacity
// ArrayVec instead of an unbounded Vec. Unsupported OSC data beyond this limit
// is discarded by vte and never reaches terminal semantics.
const MAX_OSC_BYTES: usize = 1024;
const APPLICATION_CURSOR_KEYS_MODE: u16 = 1;
const ALTERNATE_SCREEN_MODE: u16 = 47;
const ALTERNATE_SCREEN_CLEAR_MODE: u16 = 1047;
const ALTERNATE_SCREEN_SAVE_RESTORE_MODE: u16 = 1049;
const INSERT_REPLACE_MODE: u16 = 4;
const AUTO_WRAP_MODE: u16 = 7;
const CURSOR_VISIBILITY_MODE: u16 = 25;
const CLEAR_CURRENT_TAB_STOP: u16 = 0;
const CLEAR_ALL_TAB_STOPS: u16 = 3;

#[derive(Clone, Copy)]
enum ExtendedColorChannel {
    Foreground,
    Background,
}

/// Incremental raw-byte parser for the supported terminal-core semantics.
///
/// Parser state is retained between calls to [`Self::advance`]. The underlying
/// parser and callback types are implementation details; callers provide bytes
/// and a [`TerminalState`] only.
///
/// This compatibility stage handles printable characters, carriage return,
/// line feed, backspace, horizontal tabs and tab stops, the supported
/// cursor/erase/scrolling-margin CSI subset, and narrow mode dispatch for
/// existing terminal-core mode state. All other parser actions are safely
/// ignored.
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

    fn dispatch_mode(&mut self, params: &vte::Params, private: bool, enabled: bool) {
        for parameter in params {
            let [mode] = parameter else {
                continue;
            };

            match (private, *mode) {
                (true, APPLICATION_CURSOR_KEYS_MODE) => {
                    let cursor_keys = if enabled {
                        CursorKeyMode::Application
                    } else {
                        CursorKeyMode::Normal
                    };
                    self.terminal.set_cursor_key_mode(cursor_keys);
                }
                (true, ALTERNATE_SCREEN_MODE) => {
                    if enabled {
                        self.terminal.switch_to_alternate_screen();
                    } else {
                        self.terminal.switch_to_primary_screen();
                    }
                }
                (true, ALTERNATE_SCREEN_CLEAR_MODE) => {
                    if enabled {
                        self.terminal.enter_alternate_screen_1047();
                    } else {
                        self.terminal.leave_alternate_screen_1047();
                    }
                }
                (true, ALTERNATE_SCREEN_SAVE_RESTORE_MODE) => {
                    if enabled {
                        self.terminal.enter_alternate_screen_1049();
                    } else {
                        self.terminal.leave_alternate_screen_1049();
                    }
                }
                (false, INSERT_REPLACE_MODE) => {
                    let insertion = if enabled {
                        CharacterInsertionMode::Insert
                    } else {
                        CharacterInsertionMode::Replace
                    };
                    self.terminal.set_character_insertion(insertion);
                }
                (true, AUTO_WRAP_MODE) => {
                    let auto_wrap = if enabled {
                        AutoWrapMode::Enabled
                    } else {
                        AutoWrapMode::Disabled
                    };
                    self.terminal.set_auto_wrap(auto_wrap);
                }
                (true, CURSOR_VISIBILITY_MODE) => {
                    let visibility = if enabled {
                        CursorVisibility::Visible
                    } else {
                        CursorVisibility::Hidden
                    };
                    self.terminal.set_cursor_visibility(visibility);
                }
                _ => {}
            }
        }
    }

    fn dispatch_decstbm(&mut self, values: [u16; 2], count: usize) {
        if count > 2 {
            return;
        }

        let screen_rows = self.terminal.dimensions().rows();
        let top = usize::from(values[0].max(1));
        let bottom = if count < 2 || values[1] == 0 {
            screen_rows
        } else {
            usize::from(values[1])
        };

        // DECSTBM requires a multi-row region. A one-row screen is the sole
        // exception so its full-screen region remains representable and CSI r
        // can retain its required reset behavior.
        if top > screen_rows
            || bottom > screen_rows
            || top > bottom
            || (top == bottom && screen_rows > 1)
        {
            return;
        }

        let updated = self
            .terminal
            .set_vertical_scrolling_margins(top - 1, bottom - 1);
        debug_assert!(updated, "validated DECSTBM margins must be accepted");
    }

    fn dispatch_sgr(&mut self, params: &vte::Params) {
        let mut parameters = params.iter();
        while let Some(parameter) = parameters.next() {
            let [attribute] = parameter else {
                continue;
            };

            match *attribute {
                0 => self.terminal.reset_rendition(),
                1 => self.terminal.set_text_intensity(TextIntensity::Bold),
                2 => self.terminal.set_text_intensity(TextIntensity::Faint),
                3 => self.terminal.set_italic_style(ItalicStyle::Italic),
                4 => self.terminal.set_underline_style(UnderlineStyle::Enabled),
                7 => self.terminal.set_inverse_video(InverseVideo::Enabled),
                22 => self.terminal.set_text_intensity(TextIntensity::Normal),
                23 => self.terminal.set_italic_style(ItalicStyle::Upright),
                24 => self.terminal.set_underline_style(UnderlineStyle::Disabled),
                27 => self.terminal.set_inverse_video(InverseVideo::Disabled),
                code @ 30..=37 => self
                    .terminal
                    .set_foreground_color(CellColor::Indexed((code - 30) as u8)),
                38 => {
                    self.dispatch_extended_color(&mut parameters, ExtendedColorChannel::Foreground)
                }
                39 => self.terminal.set_foreground_color(CellColor::Default),
                code @ 40..=47 => self
                    .terminal
                    .set_background_color(CellColor::Indexed((code - 40) as u8)),
                48 => {
                    self.dispatch_extended_color(&mut parameters, ExtendedColorChannel::Background)
                }
                49 => self.terminal.set_background_color(CellColor::Default),
                code @ 90..=97 => self
                    .terminal
                    .set_foreground_color(CellColor::Indexed((code - 90 + 8) as u8)),
                code @ 100..=107 => self
                    .terminal
                    .set_background_color(CellColor::Indexed((code - 100 + 8) as u8)),
                _ => {}
            }
        }
    }

    fn dispatch_extended_color(
        &mut self,
        parameters: &mut vte::ParamsIter<'_>,
        channel: ExtendedColorChannel,
    ) {
        let Some(selector) = parameters.next() else {
            return;
        };
        let [selector] = selector else {
            parameters.for_each(drop);
            return;
        };

        match *selector {
            5 => {
                let Some(index) = parameters.next() else {
                    return;
                };
                let [index] = index else {
                    return;
                };
                let Ok(index) = u8::try_from(*index) else {
                    return;
                };

                match channel {
                    ExtendedColorChannel::Foreground => self
                        .terminal
                        .set_foreground_color(CellColor::Indexed(index)),
                    ExtendedColorChannel::Background => self
                        .terminal
                        .set_background_color(CellColor::Indexed(index)),
                }
            }
            2 => {
                let mut components = [0; 3];
                let mut complete = true;
                for component in &mut components {
                    let Some(parameter) = parameters.next() else {
                        complete = false;
                        continue;
                    };
                    let [value] = parameter else {
                        complete = false;
                        continue;
                    };
                    *component = *value;
                }
                if !complete {
                    return;
                }
                let Ok(red) = u8::try_from(components[0]) else {
                    return;
                };
                let Ok(green) = u8::try_from(components[1]) else {
                    return;
                };
                let Ok(blue) = u8::try_from(components[2]) else {
                    return;
                };
                let color = CellColor::Rgb { red, green, blue };
                match channel {
                    ExtendedColorChannel::Foreground => self.terminal.set_foreground_color(color),
                    ExtendedColorChannel::Background => self.terminal.set_background_color(color),
                }
            }
            // An unknown selector has no defined payload length. Ignore the
            // remainder of this CSI callback so payload cannot leak into SGR.
            _ => parameters.for_each(drop),
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
            0x09 => self.terminal.horizontal_tab(),
            0x0A => self.terminal.line_feed(),
            0x0D => self.terminal.carriage_return(),
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], has_ignored_intermediates: bool, byte: u8) {
        if self.semantic_error.is_some() || has_ignored_intermediates || !intermediates.is_empty() {
            return;
        }

        match byte {
            b'D' => self.terminal.index(),
            b'E' => self.terminal.next_line(),
            b'H' => self.terminal.set_horizontal_tab_stop(),
            b'M' => self.terminal.reverse_index(),
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
        if self.semantic_error.is_some() || has_ignored_intermediates {
            return;
        }

        if matches!(action, 'h' | 'l') {
            let private = match intermediates {
                [] => false,
                [b'?'] => true,
                _ => return,
            };
            self.dispatch_mode(params, private, action == 'h');
            return;
        }

        if action == 'm' {
            if intermediates.is_empty() {
                self.dispatch_sgr(params);
            }
            return;
        }

        let Some((values, count)) = simple_csi_parameters(params) else {
            return;
        };

        if action == 'c' {
            if count == 0 || (count == 1 && values[0] == 0) {
                match intermediates {
                    [] => {
                        let _ = self.terminal.request_primary_device_attributes();
                    }
                    [b'>'] => {
                        let _ = self.terminal.request_secondary_device_attributes();
                    }
                    _ => {}
                }
            }
            return;
        }

        if !intermediates.is_empty() {
            return;
        }

        if action == 'n' {
            if count == 1 {
                match values[0] {
                    5 => {
                        let _ = self.terminal.request_terminal_status();
                    }
                    6 => {
                        let _ = self.terminal.request_cursor_position_report();
                    }
                    _ => {}
                }
            }
            return;
        }

        if action == 'r' {
            self.dispatch_decstbm(values, count);
            return;
        }

        if action == 'g' {
            match (count, values[0]) {
                (0 | 1, CLEAR_CURRENT_TAB_STOP) => {
                    self.terminal.clear_horizontal_tab_stop();
                }
                (1, CLEAR_ALL_TAB_STOPS) => self.terminal.clear_all_horizontal_tab_stops(),
                _ => {}
            }
            return;
        }

        if action == '@' && count <= 1 {
            self.terminal.insert_characters(default_one(values[0]));
            return;
        }

        if action == 'P' && count <= 1 {
            self.terminal.delete_characters(default_one(values[0]));
            return;
        }

        if action == 'X' && count <= 1 {
            self.terminal.erase_characters(default_one(values[0]));
            return;
        }

        if matches!(action, 'L' | 'M') && count <= 1 {
            let rows = default_one(values[0]);
            if action == 'L' {
                self.terminal.insert_lines(rows);
            } else {
                self.terminal.delete_lines(rows);
            }
            return;
        }

        if matches!(action, 'S' | 'T') && count <= 1 {
            let rows = default_one(values[0]);
            if action == 'S' {
                self.terminal.scroll_up(rows);
            } else {
                self.terminal.scroll_down(rows);
            }
            return;
        }

        if matches!(action, 'E' | 'F') && count <= 1 {
            let rows = default_one(values[0]);
            if action == 'E' {
                self.terminal.cursor_next_line(rows);
            } else {
                self.terminal.cursor_previous_line(rows);
            }
            return;
        }

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
