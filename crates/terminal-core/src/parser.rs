use std::time::{Duration, Instant};
use std::{error::Error, fmt};

use crate::{
    AutoWrapMode, CellColor, CharacterInsertionMode, CursorKeyMode, CursorMovement,
    CursorVisibility, DefaultColorChannel, EraseDirection, EraseRegion, InverseVideo, ItalicStyle,
    MouseEncoding, PrintError, TerminalState, TextIntensity, UnderlineStyle,
};

// Compile vte without its std feature so OSC buffering uses its fixed-capacity
// ArrayVec instead of an unbounded Vec. Unsupported OSC data beyond this limit
// is discarded by vte. Reject saturated dispatches below rather than applying
// a prefix which may have been truncated by either of vte's OSC limits.
const MAX_OSC_BYTES: usize = 1024;
// vte 0.15's fixed parameter capacity (not exposed by its public API).
const MAX_OSC_PARAMS: usize = 16;
const MAX_INDEXED_COLOR_QUERIES: usize = (MAX_OSC_PARAMS - 1) / 2;
pub const MAX_OSC52_DECODED_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Osc52Policy {
    #[default]
    Deny,
    AllowWrite,
}
const APPLICATION_CURSOR_KEYS_MODE: u16 = 1;
const ALTERNATE_SCREEN_MODE: u16 = 47;
const ALTERNATE_SCREEN_CLEAR_MODE: u16 = 1047;
const ALTERNATE_SCREEN_SAVE_RESTORE_MODE: u16 = 1049;
const INSERT_REPLACE_MODE: u16 = 4;
const ORIGIN_MODE: u16 = 6;
const AUTO_WRAP_MODE: u16 = 7;
const CURSOR_VISIBILITY_MODE: u16 = 25;
const MOUSE_PRESS_MODE: u16 = 1000;
const MOUSE_DRAG_MODE: u16 = 1002;
const MOUSE_ANY_MODE: u16 = 1003;
const ALTERNATE_SCROLL_MODE: u16 = 1007;
const FOCUS_REPORTING_MODE: u16 = 1004;
const SGR_MOUSE_MODE: u16 = 1006;
const BRACKETED_PASTE_MODE: u16 = 2004;
const SYNCHRONIZED_UPDATE_MODE: u16 = 2026;
const SYNCHRONIZED_UPDATE_TIMEOUT: Duration = Duration::from_millis(150);
const MAX_SYNCHRONIZED_UPDATE_BYTES: usize = 2 * 1024 * 1024;
const CLEAR_CURRENT_TAB_STOP: u16 = 0;
const CLEAR_ALL_TAB_STOPS: u16 = 3;

#[derive(Clone, Copy)]
enum ExtendedColorChannel {
    Foreground,
    Background,
    Ignored,
}

enum ColorQuery {
    Default(DefaultColorChannel),
    Indexed([Option<u8>; MAX_INDEXED_COLOR_QUERIES]),
}

impl ColorQuery {
    fn parse(params: &[&[u8]]) -> Option<Self> {
        match params {
            [b"10", b"?"] => Some(Self::Default(DefaultColorChannel::Foreground)),
            [b"11", b"?"] => Some(Self::Default(DefaultColorChannel::Background)),
            [b"4", pairs @ ..] => {
                let (pairs, remainder) = pairs.as_chunks::<2>();
                if !remainder.is_empty() {
                    return None;
                }
                let mut indices = [None; MAX_INDEXED_COLOR_QUERIES];
                for (slot, pair) in indices.iter_mut().zip(pairs) {
                    if let [index, b"?"] = pair
                        && index.iter().all(u8::is_ascii_digit)
                    {
                        *slot = std::str::from_utf8(index)
                            .ok()
                            .and_then(|index| index.parse().ok());
                    }
                }
                Some(Self::Indexed(indices))
            }
            _ => None,
        }
    }

    fn reply(&self, terminal: &mut TerminalState, bell_terminated: bool) {
        match self {
            Self::Default(channel) => terminal.request_default_color(*channel, bell_terminated),
            Self::Indexed(indices) => {
                for &index in indices.iter().flatten() {
                    terminal.request_indexed_color(index, bell_terminated);
                }
            }
        }
    }
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
    osc52_policy: Osc52Policy,
    pending_color_query: Option<ColorQuery>,
    synchronized_update: Option<SynchronizedUpdate>,
    output_version: u64,
}

impl TerminalParser {
    pub fn with_osc52_policy(policy: Osc52Policy) -> Self {
        Self {
            parser: vte::Parser::new_with_size(),
            osc52_policy: policy,
            pending_color_query: None,
            synchronized_update: None,
            output_version: 0,
        }
    }

    /// Creates a parser in its initial ground state.
    pub fn new() -> Self {
        Self::with_osc52_policy(Osc52Policy::Deny)
    }

    /// Changes when output has been dispatched to the terminal model.
    pub fn output_version(&self) -> u64 {
        self.output_version
    }

    /// Returns the current deadline for publishing buffered output without more input.
    pub fn synchronized_update_deadline(&self) -> Option<Instant> {
        self.synchronized_update
            .as_ref()
            .map(|update| update.deadline)
    }

    /// Publishes buffered output if the current synchronized update has expired.
    /// The result reports the first semantic error, if any, after publishing the update.
    pub fn expire_synchronized_update(
        &mut self,
        terminal: &mut TerminalState,
        now: Instant,
    ) -> Option<Result<(), PrintError>> {
        let deadline = self.synchronized_update_deadline()?;
        (now >= deadline).then(|| self.finish_synchronized_update(terminal))
    }

    /// Advances the parser through one input chunk.
    ///
    /// On a terminal semantic error, parsing stops before any later parser
    /// callback can mutate state. The returned byte count is relative to
    /// `bytes` and lets the caller resume with the unconsumed suffix. It can be
    /// zero when a byte invalidates a UTF-8 prefix retained from an earlier
    /// chunk and must itself be reprocessed. This bounds error storage to one
    /// explicit error without silently discarding later errors.
    /// A synchronized update is published as a unit. If its previously accepted
    /// bytes cause semantic errors, the first is reported at the publication
    /// boundary and `bytes_consumed` still refers only to the current chunk.
    pub fn advance(
        &mut self,
        terminal: &mut TerminalState,
        bytes: &[u8],
    ) -> Result<(), TerminalParserError> {
        self.advance_at(terminal, bytes, Instant::now())
    }

    fn advance_at(
        &mut self,
        terminal: &mut TerminalState,
        bytes: &[u8],
        now: Instant,
    ) -> Result<(), TerminalParserError> {
        if let Some(Err(semantic_error)) = self.expire_synchronized_update(terminal, now) {
            return Err(TerminalParserError {
                bytes_consumed: 0,
                semantic_error,
            });
        }
        let mut offset = 0;
        while offset < bytes.len() {
            if let Some(update) = self.synchronized_update.as_mut() {
                let (consumed, finished) = update.advance(&bytes[offset..]);
                offset += consumed;
                if finished && let Err(semantic_error) = self.finish_synchronized_update(terminal) {
                    return Err(TerminalParserError {
                        bytes_consumed: offset,
                        semantic_error,
                    });
                }
            } else {
                let (consumed, begin) = self
                    .dispatch_bytes(terminal, &bytes[offset..], true)
                    .map_err(|mut error| {
                        error.bytes_consumed += offset;
                        error
                    })?;
                offset += consumed;
                if begin {
                    self.synchronized_update = Some(SynchronizedUpdate {
                        probe: vte::Parser::new_with_size(),
                        deadline: now + SYNCHRONIZED_UPDATE_TIMEOUT,
                        bytes: Vec::new(),
                    });
                }
            }
        }
        Ok(())
    }

    fn finish_synchronized_update(
        &mut self,
        terminal: &mut TerminalState,
    ) -> Result<(), PrintError> {
        let update = self.synchronized_update.take().unwrap();
        let mut remaining = update.bytes.as_slice();
        let mut first_error = None;
        while !remaining.is_empty() {
            match self.dispatch_bytes(terminal, remaining, false) {
                Ok(_) => break,
                Err(error) => {
                    first_error.get_or_insert(error.semantic_error);
                    // A zero count cleared an invalid partial UTF-8 sequence.
                    // Parse the same byte again in the new parser state.
                    remaining = &remaining[error.bytes_consumed..];
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    fn dispatch_bytes(
        &mut self,
        terminal: &mut TerminalState,
        bytes: &[u8],
        stop_on_begin: bool,
    ) -> Result<(usize, bool), TerminalParserError> {
        let mut performer =
            SemanticPerformer::new(terminal, self.osc52_policy, &mut self.pending_color_query);

        for (index, byte) in bytes.iter().enumerate() {
            if let Some(query) = performer.pending_color_query.as_ref() {
                if *byte == b'\\' {
                    query.reply(performer.terminal, false);
                    performer.published_output = true;
                }
                *performer.pending_color_query = None;
            }
            performer.dispatch_byte = *byte;
            let consumed = self
                .parser
                .advance_until_terminated(&mut performer, std::slice::from_ref(byte));

            if performer.semantic_error.is_some() || (stop_on_begin && performer.begin_update) {
                self.output_version = self
                    .output_version
                    .wrapping_add(u64::from(performer.published_output));
                return match performer.semantic_error {
                    Some(semantic_error) => Err(TerminalParserError {
                        bytes_consumed: index + consumed,
                        semantic_error,
                    }),
                    None => Ok((index + consumed, true)),
                };
            }

            debug_assert_eq!(consumed, 1);
        }

        self.output_version = self
            .output_version
            .wrapping_add(u64::from(performer.published_output));
        Ok((bytes.len(), false))
    }
}

struct SynchronizedUpdate {
    probe: vte::Parser<MAX_OSC_BYTES>,
    deadline: Instant,
    bytes: Vec<u8>,
}

impl SynchronizedUpdate {
    fn advance(&mut self, input: &[u8]) -> (usize, bool) {
        // Run the same escape syntax parser across chunks so a canceled or
        // embedded sequence cannot end the update. The terminal's semantic
        // parser stays at the begin boundary until these bytes are published.
        let available = MAX_SYNCHRONIZED_UPDATE_BYTES - self.bytes.len();
        let input = &input[..input.len().min(available)];
        let mut end = SynchronizedUpdateEnd::default();
        let consumed = self.probe.advance_until_terminated(&mut end, input);
        let len = self.bytes.len() + consumed;
        if len > self.bytes.capacity() {
            let capacity = (self.bytes.capacity() * 2).clamp(len, MAX_SYNCHRONIZED_UPDATE_BYTES);
            self.bytes.reserve_exact(capacity - self.bytes.len());
        }
        self.bytes.extend_from_slice(&input[..consumed]);
        (
            consumed,
            end.reached || len == MAX_SYNCHRONIZED_UPDATE_BYTES,
        )
    }
}

#[derive(Default)]
struct SynchronizedUpdateEnd {
    reached: bool,
}

impl vte::Perform for SynchronizedUpdateEnd {
    fn esc_dispatch(&mut self, intermediates: &[u8], ignored: bool, action: u8) {
        self.reached = !ignored && intermediates.is_empty() && action == b'c';
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        ignored: bool,
        action: char,
    ) {
        self.reached = !ignored
            && match (intermediates, action) {
                (b"?", 'l') => params
                    .into_iter()
                    .any(|param| param == [SYNCHRONIZED_UPDATE_MODE]),
                (b"!", 'p') => is_soft_reset(params),
                _ => false,
            };
    }

    fn terminated(&self) -> bool {
        self.reached
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
    osc52_policy: Osc52Policy,
    pending_color_query: &'a mut Option<ColorQuery>,
    dispatch_byte: u8,
    begin_update: bool,
    published_output: bool,
}

impl<'a> SemanticPerformer<'a> {
    fn new(
        terminal: &'a mut TerminalState,
        osc52_policy: Osc52Policy,
        pending_color_query: &'a mut Option<ColorQuery>,
    ) -> Self {
        Self {
            terminal,
            semantic_error: None,
            osc52_policy,
            pending_color_query,
            dispatch_byte: 0,
            begin_update: false,
            published_output: false,
        }
    }

    fn dispatch_mode(&mut self, params: &vte::Params, private: bool, enabled: bool) {
        for parameter in params {
            let [mode] = parameter else {
                continue;
            };
            if private && *mode == SYNCHRONIZED_UPDATE_MODE {
                self.begin_update = enabled;
                continue;
            }
            self.published_output = true;

            match (private, *mode) {
                (true, MOUSE_PRESS_MODE) => self.terminal.set_mouse_press_tracking(enabled),
                (true, MOUSE_DRAG_MODE) => self.terminal.set_mouse_drag_tracking(enabled),
                (true, MOUSE_ANY_MODE) => self.terminal.set_mouse_any_tracking(enabled),
                (true, ALTERNATE_SCROLL_MODE) => self.terminal.set_alternate_scroll(enabled),
                (true, FOCUS_REPORTING_MODE) => self.terminal.set_focus_reporting(enabled),
                (true, SGR_MOUSE_MODE) => self.terminal.set_mouse_encoding(if enabled {
                    MouseEncoding::Sgr
                } else {
                    MouseEncoding::Legacy
                }),
                (true, BRACKETED_PASTE_MODE) => self.terminal.set_bracketed_paste(enabled),
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
                (true, ORIGIN_MODE) => self.terminal.set_origin_mode(enabled),
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
                self.dispatch_sgr_subparameters(parameter);
                continue;
            };

            match *attribute {
                0 => self.terminal.reset_rendition(),
                1 => self.terminal.set_text_intensity(TextIntensity::Bold),
                2 => self.terminal.set_text_intensity(TextIntensity::Faint),
                3 => self.terminal.set_italic_style(ItalicStyle::Italic),
                4 => self.terminal.set_underline_style(UnderlineStyle::Enabled),
                7 => self.terminal.set_inverse_video(InverseVideo::Enabled),
                8 => self.terminal.set_concealed(true),
                9 => self.terminal.set_strikethrough(true),
                22 => self.terminal.set_text_intensity(TextIntensity::Normal),
                23 => self.terminal.set_italic_style(ItalicStyle::Upright),
                24 => self.terminal.set_underline_style(UnderlineStyle::Disabled),
                27 => self.terminal.set_inverse_video(InverseVideo::Disabled),
                28 => self.terminal.set_concealed(false),
                29 => self.terminal.set_strikethrough(false),
                code @ 30..=37 => self
                    .terminal
                    .set_foreground_color(CellColor::Indexed((code - 30) as u8)),
                38 => {
                    self.dispatch_extended_color(&mut parameters, ExtendedColorChannel::Foreground)
                }
                58 => self.dispatch_extended_color(&mut parameters, ExtendedColorChannel::Ignored),
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

    fn dispatch_sgr_subparameters(&mut self, values: &[u16]) {
        // A colon group is atomic: invalid payload never becomes another SGR.
        let (channel, payload) = match values {
            [38, payload @ ..] => (ExtendedColorChannel::Foreground, payload),
            [48, payload @ ..] => (ExtendedColorChannel::Background, payload),
            [4, style @ 0..=1] => {
                self.terminal.set_underline_style(if *style == 0 {
                    UnderlineStyle::Disabled
                } else {
                    UnderlineStyle::Enabled
                });
                return;
            }
            _ => return,
        };
        let color = match payload {
            [5, index] => u8::try_from(*index).ok().map(CellColor::Indexed),
            [2, red, green, blue] | [2, 0, red, green, blue] => {
                match (
                    u8::try_from(*red),
                    u8::try_from(*green),
                    u8::try_from(*blue),
                ) {
                    (Ok(red), Ok(green), Ok(blue)) => Some(CellColor::Rgb { red, green, blue }),
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(color) = color {
            match channel {
                ExtendedColorChannel::Foreground => self.terminal.set_foreground_color(color),
                ExtendedColorChannel::Background => self.terminal.set_background_color(color),
                ExtendedColorChannel::Ignored => {}
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
                    ExtendedColorChannel::Ignored => {}
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
                    ExtendedColorChannel::Ignored => {}
                }
            }
            // An unknown selector has no defined payload length. Ignore the
            // remainder of this CSI callback so payload cannot leak into SGR.
            _ => parameters.for_each(drop),
        }
    }
}

fn valid_working_directory_uri(uri: &str) -> bool {
    let Some(prefix) = uri.get(..7) else {
        return false;
    };
    if !prefix.eq_ignore_ascii_case("file://") {
        return false;
    }
    let Some((host, path)) = uri[7..].split_once('/') else {
        return false;
    };
    if !host
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
    {
        return false;
    }
    let mut decoded = Vec::with_capacity(path.len() + 1);
    decoded.push(b'/');
    let mut bytes = path.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let Some(high) = bytes.next().and_then(|digit| (digit as char).to_digit(16)) else {
                return false;
            };
            let Some(low) = bytes.next().and_then(|digit| (digit as char).to_digit(16)) else {
                return false;
            };
            decoded.push((high * 16 + low) as u8);
        } else if byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'.'
                    | b'_'
                    | b'~'
                    | b'!'
                    | b'$'
                    | b'&'
                    | b'\''
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b';'
                    | b'='
                    | b':'
                    | b'@'
                    | b'/'
            )
        {
            decoded.push(byte);
        } else {
            return false;
        }
    }
    std::str::from_utf8(&decoded).is_ok_and(|path| !path.chars().any(char::is_control))
}

impl vte::Perform for SemanticPerformer<'_> {
    fn osc_dispatch(&mut self, params: &[&[u8]], bell_terminated: bool) {
        // vte dispatches OSC on CAN/SUB as well as BEL/ESC. Cancellation must
        // not apply OSC actions.
        // Its callback also lacks an overflow flag: saturation is ambiguous,
        // so reject it conservatively. These checks run only on OSC dispatch.
        if self.semantic_error.is_some()
            || matches!(self.dispatch_byte, 0x18 | 0x1a)
            || params.len() >= MAX_OSC_PARAMS
            || params.iter().map(|param| param.len()).sum::<usize>() >= MAX_OSC_BYTES
        {
            return;
        }
        self.published_output = true;
        if let Some(query) = ColorQuery::parse(params) {
            if bell_terminated {
                query.reply(self.terminal, true);
            } else {
                // vte dispatches at ESC, before the backslash that completes ST.
                *self.pending_color_query = Some(query);
            }
            return;
        }
        if params.len() >= 2 && (params[0] == b"0" || params[0] == b"2") {
            let payload = params[1..].join(&b';');
            if payload.len() <= 512
                && let Ok(title) = String::from_utf8(payload)
                && !title.chars().any(char::is_control)
            {
                self.terminal
                    .set_shell_title((!title.is_empty()).then_some(title));
            }
            return;
        }
        if params.len() >= 2 && params[0] == b"7" {
            let payload = params[1..].join(&b';');
            if let Ok(uri) = String::from_utf8(payload)
                && valid_working_directory_uri(&uri)
            {
                self.terminal.set_working_directory_uri(uri);
            }
            return;
        }
        if params.len() == 2 && params[0] == b"133" && params[1] == b"A" {
            self.terminal.mark_shell_prompt();
            return;
        }
        if params.len() >= 3 && params[0] == b"8" {
            let mut uri = Vec::new();
            for (index, part) in params[2..].iter().enumerate() {
                if index != 0 {
                    uri.push(b';');
                }
                uri.extend_from_slice(part);
            }
            if let Ok(uri) = String::from_utf8(uri) {
                self.terminal
                    .set_hyperlink(if uri.is_empty() { None } else { Some(uri) });
            }
            return;
        }
        use base64::Engine;
        if self.osc52_policy != Osc52Policy::AllowWrite
            || params.len() != 3
            || params[0] != b"52"
            || params[1] != b"c"
        {
            return;
        }
        let encoded = params[2];
        if encoded.is_empty()
            || encoded == b"?"
            || encoded.len() > 4 * MAX_OSC52_DECODED_BYTES / 3 + 4
        {
            return;
        }
        let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(encoded) else {
            return;
        };
        if decoded.len() > MAX_OSC52_DECODED_BYTES || decoded.contains(&0) {
            return;
        }
        let Ok(text) = String::from_utf8(decoded) else {
            return;
        };
        self.terminal.queue_osc52_write(text);
    }

    fn print(&mut self, character: char) {
        if self.semantic_error.is_none() {
            self.published_output = true;
            self.semantic_error = self.terminal.print_character(character).err();
        }
    }

    fn execute(&mut self, byte: u8) {
        if self.semantic_error.is_some() {
            return;
        }
        self.published_output = true;

        match byte {
            0x08 => self.terminal.backspace(),
            0x09 => self.terminal.horizontal_tab(),
            0x0A..=0x0C => self.terminal.line_feed(),
            0x0D => self.terminal.carriage_return(),
            0x0E => self.terminal.invoke_character_set(true),
            0x0F => self.terminal.invoke_character_set(false),
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], has_ignored_intermediates: bool, byte: u8) {
        if self.semantic_error.is_some() || has_ignored_intermediates {
            return;
        }
        self.published_output = true;

        if matches!(intermediates, [b'('] | [b')']) && matches!(byte, b'0' | b'B') {
            self.terminal
                .designate_character_set(intermediates == b")", byte == b'0');
            return;
        }
        if !intermediates.is_empty() {
            return;
        }

        match byte {
            b'7' => self.terminal.save_cursor(),
            b'8' => self.terminal.restore_cursor(),
            b'c' => self.terminal.hard_reset(),
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
        self.published_output = true;

        if action == 'm' {
            if intermediates == b">"
                && let Some((values, count)) = simple_csi_parameters(params)
                && count >= 1
                && values[0] == 4
            {
                self.terminal
                    .modify_other_keys_request(if count > 1 { values[1] } else { 0 });
            }
            if intermediates.is_empty() {
                self.dispatch_sgr(params);
            }
            return;
        }

        if action == 'p' && intermediates == b"!" {
            if is_soft_reset(params) {
                self.terminal.soft_reset();
            }
            return;
        }

        let Some((values, count)) = simple_csi_parameters(params) else {
            return;
        };

        if action == 'p'
            && intermediates == b"?$"
            && count == 1
            && values[0] == ALTERNATE_SCROLL_MODE
        {
            let _ = self.terminal.request_alternate_scroll_status();
            return;
        }

        // DECSCUSR is CSI Ps SP q: the space is an intermediate byte.
        if action == 'q' && intermediates == b" " && count <= 1 {
            if count == 0 || values[0] == 0 {
                self.terminal.reset_cursor_style_from_application();
                return;
            }
            let style = match values[0] {
                1 => Some(crate::CursorStyle {
                    shape: crate::CursorShape::Block,
                    blinking: true,
                }),
                2 => Some(crate::CursorStyle {
                    shape: crate::CursorShape::Block,
                    blinking: false,
                }),
                3 => Some(crate::CursorStyle {
                    shape: crate::CursorShape::Underline,
                    blinking: true,
                }),
                4 => Some(crate::CursorStyle {
                    shape: crate::CursorShape::Underline,
                    blinking: false,
                }),
                5 => Some(crate::CursorStyle {
                    shape: crate::CursorShape::Beam,
                    blinking: true,
                }),
                6 => Some(crate::CursorStyle {
                    shape: crate::CursorShape::Beam,
                    blinking: false,
                }),
                _ => None,
            };
            if let Some(style) = style {
                self.terminal.set_cursor_style_from_application(style);
            }
            return;
        }

        if action == 'u' && intermediates == b"?" && (count == 0 || (count == 1 && values[0] == 0))
        {
            self.terminal.request_keyboard_flags();
            return;
        }

        if action == 'u'
            && intermediates.len() == 1
            && matches!(intermediates[0], b'>' | b'=' | b'<')
            && count <= if intermediates == b"=" { 2 } else { 1 }
        {
            self.terminal.keyboard_reporting_request(
                intermediates[0],
                if count > 0 { values[0] } else { 0 },
                if count > 1 { values[1] } else { 1 },
            );
            return;
        }

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

        if action == 'J' && count == 1 && values[0] == 3 {
            self.terminal.erase_scrollback();
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

fn is_soft_reset(params: &vte::Params) -> bool {
    matches!(simple_csi_parameters(params), Some(([0, _], 0 | 1)))
}

fn default_one(value: u16) -> usize {
    usize::from(value.max(1))
}

#[cfg(test)]
mod synchronized_update_tests {
    use super::*;
    use crate::TerminalDimensions;

    const BEGIN: &[u8] = b"\x1b[?2026h";
    const END: &[u8] = b"\x1b[?2026l";

    fn terminal() -> TerminalState {
        TerminalState::new(TerminalDimensions::new(8, 2).unwrap())
    }

    fn text(terminal: &TerminalState, columns: usize) -> String {
        (0..columns)
            .map(|column| terminal.screen().cell(0, column).unwrap().character())
            .collect()
    }

    #[test]
    fn synchronized_update_publishes_only_after_split_end() {
        let mut parser = TerminalParser::new();
        let mut terminal = terminal();
        let now = Instant::now();
        parser.advance_at(&mut terminal, b"old", now).unwrap();
        let version = parser.output_version();
        let input = [BEGIN, "\r界B\x1b[5n".as_bytes(), END].concat();
        for byte in &input[..input.len() - 1] {
            parser.advance_at(&mut terminal, &[*byte], now).unwrap();
            assert_eq!(parser.output_version(), version);
            assert_eq!(text(&terminal, 3), "old");
            assert_eq!(terminal.take_reply(), None);
        }
        parser.advance_at(&mut terminal, b"l", now).unwrap();
        assert_ne!(parser.output_version(), version);
        assert_eq!(text(&terminal, 3), "界 B");
        assert_eq!(
            terminal.take_reply(),
            Some(crate::TerminalReply::TerminalStatus)
        );
        assert_eq!(parser.synchronized_update_deadline(), None);
    }

    #[test]
    fn adjacent_synchronized_updates_publish_at_their_own_boundaries() {
        let mut parser = TerminalParser::new();
        let mut terminal = terminal();
        let now = Instant::now();
        parser
            .advance_at(&mut terminal, b"A\x1b[?2026hB\x1b[?2026l\x1b[?2026hC", now)
            .unwrap();
        assert_eq!(text(&terminal, 4), "AB  ");
        assert!(parser.synchronized_update_deadline().is_some());
        parser
            .advance_at(&mut terminal, b"\x1b[?2026lD", now)
            .unwrap();
        assert_eq!(text(&terminal, 4), "ABCD");
        assert_eq!(parser.synchronized_update_deadline(), None);
    }

    #[test]
    fn synchronized_update_deadline_is_fixed_and_recovers_without_input() {
        let mut parser = TerminalParser::new();
        let mut terminal = terminal();
        let now = Instant::now();
        let deadline = now + SYNCHRONIZED_UPDATE_TIMEOUT;
        let before_deadline = deadline - Duration::from_nanos(1);
        parser.advance_at(&mut terminal, BEGIN, now).unwrap();
        parser
            .advance_at(&mut terminal, b"A\x1b[?2026hB", before_deadline)
            .unwrap();
        assert_eq!(parser.synchronized_update_deadline(), Some(deadline));
        assert_eq!(
            parser.expire_synchronized_update(&mut terminal, before_deadline),
            None
        );
        assert_eq!(text(&terminal, 3), "   ");
        assert_eq!(
            parser.expire_synchronized_update(&mut terminal, deadline),
            Some(Ok(()))
        );
        assert_eq!(
            parser.expire_synchronized_update(&mut terminal, deadline),
            None
        );
        parser.advance_at(&mut terminal, b"C", deadline).unwrap();
        assert_eq!(text(&terminal, 3), "ABC");
    }

    #[test]
    fn synchronized_update_cap_flushes_and_resumes_partial_escape_syntax() {
        let mut parser = TerminalParser::new();
        let mut terminal = terminal();
        let now = Instant::now();
        parser.advance_at(&mut terminal, BEGIN, now).unwrap();
        // NUL is ignored. An escape prefix ends exactly at the payload cap.
        let padding = vec![0; MAX_SYNCHRONIZED_UPDATE_BYTES - 4];
        parser.advance_at(&mut terminal, b"A", now).unwrap();
        parser.advance_at(&mut terminal, &padding, now).unwrap();
        assert_eq!(text(&terminal, 3), "   ");
        assert!(parser.synchronized_update_deadline().is_some());
        parser.advance_at(&mut terminal, b"\x1b[2GB", now).unwrap();
        assert_eq!(text(&terminal, 3), "AB ");
        assert_eq!(parser.synchronized_update_deadline(), None);
    }

    #[test]
    fn real_reset_ends_the_synchronized_update_but_cancellation_does_not() {
        for (reset, expected) in [(b"\x1bc".as_slice(), "C  "), (b"\x1b[!p", "ABC")] {
            let mut parser = TerminalParser::new();
            let mut terminal = terminal();
            let now = Instant::now();
            parser.advance_at(&mut terminal, b"A", now).unwrap();
            parser.advance_at(&mut terminal, BEGIN, now).unwrap();
            parser
                .advance_at(&mut terminal, b"B\x1b[?2026\x18\x1b[!\x1a", now)
                .unwrap();
            assert_eq!(text(&terminal, 3), "A  ");
            for &byte in reset {
                parser.advance_at(&mut terminal, &[byte], now).unwrap();
            }
            assert_eq!(parser.synchronized_update_deadline(), None);
            parser.advance_at(&mut terminal, b"C", now).unwrap();
            assert_eq!(text(&terminal, 3), expected);
        }
    }

    #[test]
    fn synchronized_update_recovers_replay_error_and_preserves_current_suffix() {
        let mut parser = TerminalParser::new();
        let mut terminal = terminal();
        let now = Instant::now();
        parser.advance_at(&mut terminal, BEGIN, now).unwrap();
        parser.advance_at(&mut terminal, b"\xe2", now).unwrap();
        let input = b"A\x1b[?2026lB";
        let error = parser.advance_at(&mut terminal, input, now).unwrap_err();
        assert_eq!(
            error.semantic_error(),
            PrintError::UnsupportedCharacter('\u{fffd}')
        );
        assert_eq!(error.bytes_consumed(), 1 + END.len());
        assert_eq!(text(&terminal, 2), "A ");
        parser
            .advance_at(&mut terminal, &input[error.bytes_consumed()..], now)
            .unwrap();
        assert_eq!(text(&terminal, 2), "AB");
    }
}
