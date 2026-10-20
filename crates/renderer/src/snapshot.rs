//! Renderer-owned conversion of already-resolved terminal state into draw data.

use terminal_core::{
    CellColor, CellOccupancy, CursorVisibility, InverseVideo, ItalicStyle, ScreenKind,
    TerminalState, TextIntensity, UnderlineStyle,
};

/// Straight-alpha encoded sRGB channels; alpha is linear coverage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba(pub [f32; 4]);

impl Rgba {
    /// Decode only at the GPU boundary when the target encodes sRGB on write.
    pub(crate) fn linearized(self) -> Self {
        let mut color = self.0;
        for channel in &mut color[..3] {
            *channel = if *channel <= 0.04045 {
                *channel / 12.92
            } else {
                ((*channel + 0.055) / 1.055).powf(2.4)
            };
        }
        Self(color)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderTheme {
    pub foreground: Rgba,
    pub background: Rgba,
    pub cursor: Rgba,
    pub selection_foreground: Rgba,
    pub selection_background: Rgba,
    pub ansi: [Rgba; 16],
    pub ui: UiRenderTheme,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiRenderTheme {
    pub background: Rgba,
    pub foreground: Rgba,
    pub selected_background: Rgba,
    pub selected_foreground: Rgba,
    pub muted: Rgba,
    pub accent: Rgba,
    pub border: Rgba,
    pub search_match_foreground: Rgba,
    pub search_match_background: Rgba,
    pub search_active_background: Rgba,
    pub scrollbar_track: Rgba,
    pub scrollbar_track_hover: Rgba,
    pub scrollbar_thumb: Rgba,
    pub scrollbar_thumb_hover: Rgba,
    pub search_marker: Rgba,
    pub search_active_marker: Rgba,
    pub focus_border: Rgba,
}

impl Default for UiRenderTheme {
    fn default() -> Self {
        Self {
            background: DEFAULT_BACKGROUND,
            foreground: DEFAULT_FOREGROUND,
            selected_background: Rgba([0.18, 0.35, 0.65, 1.0]),
            selected_foreground: Rgba([1.0, 1.0, 1.0, 1.0]),
            muted: Rgba([0.7, 0.7, 0.7, 0.9]),
            accent: Rgba([1.0, 1.0, 1.0, 1.0]),
            border: Rgba([0.25, 0.25, 0.25, 0.65]),
            search_match_foreground: Rgba([1.0, 0.85, 0.35, 1.0]),
            search_match_background: Rgba([0.30, 0.22, 0.04, 1.0]),
            search_active_background: Rgba([0.18, 0.35, 0.65, 1.0]),
            scrollbar_track: Rgba([0.25, 0.25, 0.25, 0.65]),
            scrollbar_track_hover: Rgba([0.34, 0.34, 0.34, 0.85]),
            scrollbar_thumb: Rgba([0.7, 0.7, 0.7, 0.9]),
            scrollbar_thumb_hover: Rgba([0.95, 0.95, 0.95, 1.0]),
            search_marker: Rgba([1.0, 0.75, 0.2, 0.95]),
            search_active_marker: Rgba([0.30, 0.75, 1.0, 1.0]),
            focus_border: Rgba([0.35, 0.65, 1.0, 0.9]),
        }
    }
}

impl Default for RenderTheme {
    fn default() -> Self {
        Self {
            foreground: DEFAULT_FOREGROUND,
            background: DEFAULT_BACKGROUND,
            cursor: Rgba([0.8, 0.8, 0.8, 0.45]),
            selection_foreground: Rgba([1.0, 1.0, 1.0, 1.0]),
            selection_background: Rgba([0.18, 0.35, 0.65, 1.0]),
            ansi: std::array::from_fn(|index| indexed_color(index as u8)),
            ui: UiRenderTheme::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderCell {
    pub row: usize,
    pub column: usize,
    pub width: usize,
    pub character: char,
    /// Base scalar plus width-zero marks already attached by `terminal-core`.
    pub text: RenderText,
    pub foreground: Rgba,
    pub background: Rgba,
    pub underline: bool,
    pub bold: bool,
    pub italic: bool,
    pub concealed: bool,
    pub strikethrough: bool,
}

/// Most terminal cells contain one scalar. Keep its UTF-8 bytes in the cell
/// and allocate only when combining marks extend the grapheme.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RenderText {
    Scalar { bytes: [u8; 4], len: u8 },
    Combined(String),
}

impl RenderText {
    fn scalar(character: char) -> Self {
        let mut bytes = [0; 4];
        let len = character.encode_utf8(&mut bytes).len() as u8;
        Self::Scalar { bytes, len }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Scalar { bytes, len } => std::str::from_utf8(&bytes[..usize::from(*len)])
                .expect("char UTF-8 encoding is valid"),
            Self::Combined(text) => text,
        }
    }
}

/// App-supplied text drawn over the terminal projection, without changing terminal state.
#[derive(Clone, Debug, PartialEq)]
pub struct TextOverlay {
    pub lines: Vec<OverlayLine>,
    /// Place the panel at the bottom instead of the top of the viewport.
    pub bottom: bool,
    /// Search match spans in viewport coordinates, with the selected match marked active.
    pub search_matches: Vec<SearchHighlight>,
    pub search_markers: Vec<SearchMarker>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchMarker {
    pub row: usize,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchHighlight {
    pub row: usize,
    pub start_column: usize,
    pub end_column: usize,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OverlayLine {
    pub text: String,
    pub selected: bool,
    /// Optional character column drawn with the UI accent (tab activity).
    pub accent_column: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CursorRenderData {
    pub row: usize,
    pub column: usize,
}

/// Renderer-facing scrollbar inputs; terminal-core retains the actual viewport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScrollbarRenderData {
    pub history_rows: usize,
    pub visible_rows: usize,
    pub viewport_offset: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TerminalRenderData {
    pub rows: usize,
    pub columns: usize,
    pub cells: Vec<RenderCell>,
    pub cursor: Option<CursorRenderData>,
    pub cursor_color: Rgba,
    pub surface_background: Rgba,
    pub scrollbar: Option<ScrollbarRenderData>,
    pub scrollbar_hover: Option<crate::ScrollbarHit>,
    pub search_markers: Vec<SearchMarker>,
}

impl TerminalRenderData {
    pub fn from_terminal(state: &TerminalState) -> Self {
        Self::from_terminal_with_theme(state, &RenderTheme::default())
    }

    pub fn from_terminal_with_theme(state: &TerminalState, theme: &RenderTheme) -> Self {
        Self::from_terminal_with_opacity(state, theme, 1.0)
    }

    pub(crate) fn from_terminal_with_opacity(
        state: &TerminalState,
        theme: &RenderTheme,
        opacity: f32,
    ) -> Self {
        let mut surface_background = theme.background;
        surface_background.0[3] = opacity;
        let dimensions = state.dimensions();
        let mut cells = Vec::with_capacity(dimensions.cell_count() / 4);
        let default_cell = terminal_core::Cell::default();
        for row in 0..dimensions.rows() {
            for column in 0..dimensions.columns() {
                let cell = state.viewport_cell(row, column).unwrap_or(&default_cell);
                if cell.occupancy() == CellOccupancy::WideContinuation {
                    continue;
                }
                let attributes = cell.attributes();
                let selected = state.is_selected(row, column)
                    || (cell.occupancy() == CellOccupancy::WideLead
                        && state.is_selected(row, column + 1));
                // Default-background blanks emit no draw data. Skip before
                // resolving colors/styles while retaining selection/decorations.
                if matches!(cell.character(), ' ' | '\0')
                    && cell.combining_marks().is_empty()
                    && attributes.background() == CellColor::Default
                    && attributes.inverse() == InverseVideo::Disabled
                    && attributes.underline() == UnderlineStyle::Disabled
                    && !attributes.strikethrough()
                    && !selected
                {
                    continue;
                }
                let (mut foreground, mut background) = (
                    color(attributes.foreground(), theme.foreground, theme),
                    color(attributes.background(), theme.background, theme),
                );
                if attributes.inverse() == InverseVideo::Enabled {
                    std::mem::swap(&mut foreground, &mut background);
                }
                if attributes.intensity() == TextIntensity::Faint {
                    for channel in &mut foreground.0[..3] {
                        *channel *= 0.66;
                    }
                }
                background.0[3] = opacity;
                if selected {
                    foreground = theme.selection_foreground;
                    background = theme.selection_background;
                }
                let underline = attributes.underline() == UnderlineStyle::Enabled;
                if matches!(cell.character(), ' ' | '\0')
                    && cell.combining_marks().is_empty()
                    && background == surface_background
                    && !underline
                    && !attributes.strikethrough()
                {
                    continue;
                }
                cells.push(RenderCell {
                    row,
                    column,
                    width: usize::from(cell.occupancy() == CellOccupancy::WideLead) + 1,
                    character: cell.character(),
                    text: if cell.combining_marks().is_empty() {
                        RenderText::scalar(cell.character())
                    } else {
                        RenderText::Combined(
                            std::iter::once(cell.character())
                                .chain(cell.combining_marks().iter().copied())
                                .collect(),
                        )
                    },
                    foreground,
                    background,
                    underline,
                    bold: attributes.intensity() == TextIntensity::Bold,
                    italic: attributes.italic() == ItalicStyle::Italic,
                    concealed: attributes.concealed(),
                    strikethrough: attributes.strikethrough(),
                });
            }
        }
        let cursor = (state.viewport_offset() == 0
            && state.terminal_modes().cursor_visibility() == CursorVisibility::Visible)
            .then(|| {
                let cursor = state.cursor();
                CursorRenderData {
                    row: cursor.row(),
                    column: cursor.column(),
                }
            });
        let scrollbar = (state.active_screen() == ScreenKind::Primary
            && state.scrollback_len() > 0)
            .then(|| ScrollbarRenderData {
                history_rows: state.scrollback_len(),
                visible_rows: dimensions.rows(),
                viewport_offset: state.viewport_offset(),
            });
        Self {
            rows: dimensions.rows(),
            columns: dimensions.columns(),
            cells,
            cursor,
            cursor_color: theme.cursor,
            surface_background,
            scrollbar,
            scrollbar_hover: None,
            search_markers: Vec::new(),
        }
    }

    pub fn apply_text_overlay(&mut self, overlay: &TextOverlay, theme: &RenderTheme) {
        self.search_markers.clone_from(&overlay.search_markers);
        let line_count = overlay.lines.len().min(self.rows);
        if line_count == 0 || self.columns == 0 {
            return;
        }
        for cell in &mut self.cells {
            if let Some(found) = overlay.search_matches.iter().find(|found| {
                cell.row == found.row
                    && cell.column < found.end_column
                    && cell.column + cell.width > found.start_column
            }) {
                if found.active {
                    cell.foreground = theme.ui.selected_foreground;
                    cell.background = theme.ui.search_active_background;
                } else {
                    cell.foreground = theme.ui.search_match_foreground;
                    cell.background = theme.ui.search_match_background;
                }
            }
        }
        let first_row = if overlay.bottom {
            self.rows - line_count
        } else {
            0
        };
        // Drop entire covered rows so wide terminal cells cannot overlap the panel.
        self.cells
            .retain(|cell| cell.row < first_row || cell.row >= first_row + line_count);
        self.cursor = None;
        for (index, line) in overlay.lines.iter().take(line_count).enumerate() {
            let row = first_row + index;
            let (foreground, background) = if line.selected {
                (theme.ui.selected_foreground, theme.ui.selected_background)
            } else {
                (theme.ui.foreground, theme.ui.background)
            };
            let mut characters = line.text.chars();
            for column in 0..self.columns {
                let character = characters.next().unwrap_or(' ');
                self.cells.push(RenderCell {
                    row,
                    column,
                    width: 1,
                    character,
                    text: RenderText::scalar(character),
                    foreground: if line.accent_column == Some(column) {
                        theme.ui.accent
                    } else {
                        foreground
                    },
                    background,
                    underline: false,
                    bold: false,
                    italic: false,
                    concealed: false,
                    strikethrough: false,
                });
            }
        }
    }
}

const DEFAULT_FOREGROUND: Rgba = Rgba([0.9, 0.9, 0.9, 1.0]);
const DEFAULT_BACKGROUND: Rgba = Rgba([0.0, 0.0, 0.0, 1.0]);

fn color(color: CellColor, default: Rgba, theme: &RenderTheme) -> Rgba {
    match color {
        CellColor::Default => default,
        CellColor::Rgb { red, green, blue } => Rgba([
            red as f32 / 255.0,
            green as f32 / 255.0,
            blue as f32 / 255.0,
            1.0,
        ]),
        CellColor::Indexed(index) if index < 16 => theme.ansi[index as usize],
        CellColor::Indexed(index) => indexed_color(index),
    }
}

fn indexed_color(index: u8) -> Rgba {
    const ANSI: [[u8; 3]; 16] = [
        [0, 0, 0],
        [205, 49, 49],
        [13, 188, 121],
        [229, 229, 16],
        [36, 114, 200],
        [188, 63, 188],
        [17, 168, 205],
        [229, 229, 229],
        [102, 102, 102],
        [241, 76, 76],
        [35, 209, 139],
        [245, 245, 67],
        [59, 142, 234],
        [214, 112, 214],
        [41, 184, 219],
        [255, 255, 255],
    ];
    if index < 16 {
        let c = ANSI[index as usize];
        return Rgba([
            c[0] as f32 / 255.0,
            c[1] as f32 / 255.0,
            c[2] as f32 / 255.0,
            1.0,
        ]);
    }
    if index >= 232 {
        let c = 8 + (index - 232) * 10;
        let value = c as f32 / 255.0;
        return Rgba([value, value, value, 1.0]);
    }
    let index = index - 16;
    let levels = [0, 95, 135, 175, 215, 255];
    let c = [
        levels[(index / 36) as usize],
        levels[((index / 6) % 6) as usize],
        levels[(index % 6) as usize],
    ];
    Rgba([
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        1.0,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_core::{
        CellColor, CursorVisibility, InverseVideo, TerminalDimensions, TerminalParser,
        TerminalState, UnderlineStyle,
    };

    #[test]
    fn italic_dim_conceal_and_strike_preserve_colors_opacity_selection_and_text() {
        let mut state = TerminalState::new(TerminalDimensions::new(8, 2).unwrap());
        TerminalParser::new().advance(&mut state,
            "\x1b[38:2::120:150:180;48:2:30:60:90mA\x1b[2;3;4;9m界\u{301}\x1b[7mI\x1b[8mH\x1b[0mR".as_bytes()).unwrap();
        let theme = RenderTheme::default();
        let data = TerminalRenderData::from_terminal_with_opacity(&state, &theme, 0.4);
        let plain = &data.cells[0];
        let dim = &data.cells[1];
        assert_eq!(dim.width, 2);
        assert_eq!(dim.text.as_str(), "界\u{301}");
        assert!(dim.italic && dim.underline && dim.strikethrough);
        assert!(!dim.bold);
        for index in 0..3 {
            assert!((dim.foreground.0[index] - plain.foreground.0[index] * 0.66).abs() < 0.0001);
        }
        assert_eq!(dim.foreground.0[3], plain.foreground.0[3]);
        assert_eq!(dim.background, plain.background);
        let inverse = &data.cells[2];
        assert_eq!(
            inverse.background.0,
            [120.0 / 255.0, 150.0 / 255.0, 180.0 / 255.0, 0.4]
        );
        assert!((inverse.foreground.0[0] - 30.0 / 255.0 * 0.66).abs() < 0.0001);
        assert!(data.cells[3].concealed);
        assert_eq!(data.cells[3].text.as_str(), "H");
        let reset = &data.cells[4];
        assert!(!reset.italic && !reset.concealed && !reset.strikethrough && !reset.underline);
        assert_eq!(reset.foreground, theme.foreground);
        state.begin_selection(0, 1);
        state.extend_selection(0, 4);
        assert_eq!(state.selected_text().as_deref(), Some("界\u{301}IH"));
        let selected = TerminalRenderData::from_terminal_with_opacity(&state, &theme, 0.4);
        assert_eq!(selected.cells[1].foreground, theme.selection_foreground);
        assert_eq!(selected.cells[1].background, theme.selection_background);
        assert!(selected.cells[3].concealed);
    }

    #[test]
    fn strikethrough_spaces_are_projected_and_styles_survive_scrollback() {
        let mut state = TerminalState::new(TerminalDimensions::new(3, 1).unwrap());
        TerminalParser::new()
            .advance(&mut state, b"\x1b[2;3;9mA \r\n")
            .unwrap();
        assert_eq!(state.scrollback_len(), 1);
        state.page_up();
        let data = TerminalRenderData::from_terminal(&state);
        assert_eq!(data.cells.len(), 2);
        assert!(
            data.cells
                .iter()
                .all(|cell| cell.italic && cell.strikethrough)
        );
        assert_eq!(data.cells[1].character, ' ');
    }

    #[test]
    fn blank_grid_projects_no_instances_but_colored_blank_remains() {
        let mut state = TerminalState::new(TerminalDimensions::new(3, 2).unwrap());
        assert!(TerminalRenderData::from_terminal(&state).cells.is_empty());
        state.set_background_color(CellColor::Indexed(1));
        state.print_character(' ').unwrap();
        let data = TerminalRenderData::from_terminal(&state);
        assert_eq!(data.cells.len(), 1);
        assert_eq!((data.cells[0].row, data.cells[0].column), (0, 0));
        assert_ne!(data.cells[0].background, data.surface_background);
    }

    #[test]
    fn selected_blank_remains_visible_in_sparse_projection() {
        let mut state = TerminalState::new(TerminalDimensions::new(3, 1).unwrap());
        assert!(state.begin_selection(0, 0));
        assert!(state.extend_selection(0, 1));
        let data = TerminalRenderData::from_terminal(&state);
        assert_eq!(data.cells.len(), 2);
        assert!(data.cells.iter().all(|cell| cell.character == ' '));
        assert!(
            data.cells
                .iter()
                .all(|cell| cell.background != data.surface_background)
        );
    }

    #[test]
    fn scalar_text_is_inline_for_ascii_and_unicode() {
        assert_eq!(RenderText::scalar('A').as_str(), "A");
        assert_eq!(RenderText::scalar('界').as_str(), "界");
    }

    #[test]
    fn text_overlay_covers_terminal_rows_without_mutating_core() {
        let mut state = TerminalState::new(TerminalDimensions::new(3, 2).unwrap());
        state.print_character('界').unwrap();
        let theme = RenderTheme::default();
        let mut data = TerminalRenderData::from_terminal_with_theme(&state, &theme);
        data.apply_text_overlay(
            &TextOverlay {
                lines: vec![OverlayLine {
                    text: "Go".into(),
                    selected: true,
                    accent_column: None,
                }],
                bottom: false,
                search_matches: Vec::new(),
                search_markers: Vec::new(),
            },
            &theme,
        );
        assert_eq!(
            data.cells
                .iter()
                .filter(|cell| cell.row == 0)
                .map(|cell| cell.character)
                .collect::<String>(),
            "Go "
        );
        assert_eq!(
            data.cells
                .iter()
                .find(|cell| cell.row == 0)
                .unwrap()
                .background,
            theme.selection_background
        );
        assert_eq!(data.cursor, None);
        assert_eq!(state.screen().cell(0, 0).unwrap().character(), '界');
    }

    #[test]
    fn search_overlay_highlights_match_and_uses_opposite_edge() {
        let mut state = TerminalState::new(TerminalDimensions::new(3, 2).unwrap());
        state.print_character('A').unwrap();
        let theme = RenderTheme::default();
        let mut data = TerminalRenderData::from_terminal_with_theme(&state, &theme);
        data.apply_text_overlay(
            &TextOverlay {
                lines: vec![OverlayLine {
                    text: "Find".into(),
                    selected: true,
                    accent_column: None,
                }],
                bottom: true,
                search_matches: vec![SearchHighlight {
                    row: 0,
                    start_column: 0,
                    end_column: 1,
                    active: true,
                }],
                search_markers: vec![SearchMarker {
                    row: 0,
                    active: true,
                }],
            },
            &theme,
        );
        let match_cell = data
            .cells
            .iter()
            .find(|cell| cell.row == 0 && cell.column == 0)
            .unwrap();
        assert_eq!(match_cell.character, 'A');
        assert_eq!(match_cell.background, theme.selection_background);
        assert_eq!(
            data.search_markers,
            vec![SearchMarker {
                row: 0,
                active: true
            }]
        );
        assert!(
            data.cells
                .iter()
                .any(|cell| cell.row == 1 && cell.character == 'F')
        );
    }

    #[test]
    fn search_overlay_highlights_full_spans_with_distinct_active_style() {
        let mut state = TerminalState::new(TerminalDimensions::new(6, 2).unwrap());
        for character in "ABCDEF".chars() {
            state.print_character(character).unwrap();
        }
        let theme = RenderTheme::default();
        let mut data = TerminalRenderData::from_terminal_with_theme(&state, &theme);
        data.apply_text_overlay(
            &TextOverlay {
                lines: vec![OverlayLine {
                    text: "Search".into(),
                    selected: true,
                    accent_column: None,
                }],
                bottom: true,
                search_matches: vec![
                    SearchHighlight {
                        row: 0,
                        start_column: 0,
                        end_column: 3,
                        active: false,
                    },
                    SearchHighlight {
                        row: 0,
                        start_column: 3,
                        end_column: 6,
                        active: true,
                    },
                ],
                search_markers: Vec::new(),
            },
            &theme,
        );

        for column in 0..3 {
            let cell = data
                .cells
                .iter()
                .find(|cell| cell.row == 0 && cell.column == column)
                .unwrap();
            assert_eq!(cell.background, Rgba([0.30, 0.22, 0.04, 1.0]));
        }
        for column in 3..6 {
            let cell = data
                .cells
                .iter()
                .find(|cell| cell.row == 0 && cell.column == column)
                .unwrap();
            assert_eq!(cell.background, theme.selection_background);
        }
        assert_eq!(state.screen().cell(0, 0).unwrap().character(), 'A');
    }
    #[test]
    fn overlay_uses_resolved_ui_colors_for_matches_selection_and_activity() {
        let mut state = TerminalState::new(TerminalDimensions::new(4, 3).unwrap());
        state.print_character('A').unwrap();
        let mut theme = RenderTheme::default();
        theme.ui.background = Rgba([0.1, 0.2, 0.3, 1.0]);
        theme.ui.foreground = Rgba([0.4, 0.5, 0.6, 1.0]);
        theme.ui.selected_background = Rgba([0.2, 0.3, 0.4, 1.0]);
        theme.ui.selected_foreground = Rgba([0.9, 0.8, 0.7, 1.0]);
        theme.ui.accent = Rgba([0.8, 0.2, 0.6, 1.0]);
        theme.ui.search_match_background = Rgba([0.7, 0.1, 0.2, 1.0]);
        let mut data = TerminalRenderData::from_terminal_with_theme(&state, &theme);
        data.apply_text_overlay(
            &TextOverlay {
                lines: vec![
                    OverlayLine {
                        text: "x".into(),
                        selected: false,
                        accent_column: None,
                    },
                    OverlayLine {
                        text: "*x".into(),
                        selected: true,
                        accent_column: Some(0),
                    },
                ],
                bottom: true,
                search_matches: vec![SearchHighlight {
                    row: 0,
                    start_column: 0,
                    end_column: 1,
                    active: false,
                }],
                search_markers: Vec::new(),
            },
            &theme,
        );
        let cell = |row| {
            data.cells
                .iter()
                .find(|cell| cell.row == row && cell.column == 0)
                .unwrap()
        };
        assert_eq!(cell(0).background, theme.ui.search_match_background);
        assert_eq!(cell(1).foreground, theme.ui.foreground);
        assert_eq!(cell(1).background, theme.ui.background);
        assert_eq!(cell(2).foreground, theme.ui.accent);
        assert_eq!(cell(2).background, theme.ui.selected_background);
        assert_eq!(
            data.cells
                .iter()
                .find(|cell| cell.row == 2 && cell.column == 1)
                .unwrap()
                .foreground,
            theme.ui.selected_foreground
        );
    }
    #[test]
    fn selection_highlights_snapshot_without_changing_terminal_cells() {
        let mut state = TerminalState::new(TerminalDimensions::new(2, 1).unwrap());
        state.print_character('x').unwrap();
        assert!(state.begin_selection(0, 0));
        assert!(state.extend_selection(0, 1));
        let snapshot = TerminalRenderData::from_terminal(&state);
        assert_eq!(snapshot.cells[0].background, Rgba([0.18, 0.35, 0.65, 1.0]));
        assert_eq!(state.screen().cell(0, 0).unwrap().character(), 'x');
    }

    #[test]
    fn reflow_projection_uses_mapped_selection_wide_cells_and_cursor() {
        let mut state = TerminalState::new(TerminalDimensions::new(6, 4).unwrap());
        TerminalParser::new()
            .advance(&mut state, "ab界e\u{301}fg".as_bytes())
            .unwrap();
        state.begin_selection(0, 2);
        state.extend_selection(1, 0);
        let selected = state.selected_text();
        for columns in [1, 3, 8] {
            state.resize(TerminalDimensions::new(columns, 8).unwrap());
            let data = TerminalRenderData::from_terminal(&state);
            assert_eq!(data.columns, columns);
            assert_eq!(data.rows, 8);
            let wide = data
                .cells
                .iter()
                .find(|cell| cell.character == '界')
                .unwrap();
            assert_eq!(wide.background, RenderTheme::default().selection_background);
            assert_eq!(state.selected_text(), selected);
            let cursor = data.cursor.unwrap();
            assert_eq!(
                (cursor.row, cursor.column),
                (state.cursor().row(), state.cursor().column())
            );
            assert!(
                data.cells
                    .iter()
                    .any(|cell| cell.text.as_str() == "e\u{301}")
            );
        }
    }

    #[test]
    fn reflowed_compact_history_projects_selection_over_default_padding() {
        let mut state = TerminalState::new(TerminalDimensions::new(4, 3).unwrap());
        TerminalParser::new()
            .advance(&mut state, b"a\r\nb\r\nc\r\nd\r\ne")
            .unwrap();
        state.scroll_viewport_rows(i32::MAX);
        state.begin_selection(0, 0);
        state.extend_selection(1, 0);
        state.resize(TerminalDimensions::new(8, 3).unwrap());
        assert_eq!(state.viewport_row_columns(0), 1);
        assert!(state.is_selected(0, 7));
        let data = TerminalRenderData::from_terminal(&state);
        let padding = data
            .cells
            .iter()
            .find(|cell| cell.row == 0 && cell.column == 7)
            .unwrap();
        assert_eq!(
            padding.background,
            RenderTheme::default().selection_background
        );
        assert_eq!(state.selected_text().as_deref(), Some("a\nb"));
    }

    #[test]
    fn custom_theme_changes_projection_without_mutating_core() {
        let mut state = TerminalState::new(TerminalDimensions::new(2, 1).unwrap());
        state.set_foreground_color(CellColor::Indexed(1));
        state.print_character('x').unwrap();
        let mut theme = RenderTheme::default();
        theme.ansi[1] = Rgba([0.1, 0.2, 0.3, 1.0]);
        theme.background = Rgba([0.4, 0.5, 0.6, 1.0]);
        theme.cursor = Rgba([0.7, 0.8, 0.9, 0.45]);
        let projection = TerminalRenderData::from_terminal_with_theme(&state, &theme);
        assert_eq!(projection.cells[0].foreground, theme.ansi[1]);
        assert_eq!(projection.cells[0].background, theme.background);
        assert_eq!(projection.cursor_color, theme.cursor);
        assert_eq!(projection.surface_background, theme.background);
        assert_eq!(
            state.screen().cell(0, 0).unwrap().attributes().foreground(),
            CellColor::Indexed(1)
        );
    }

    #[test]
    fn ncurses_bold_projection_preserves_colors_and_resets_weight() {
        let mut state = TerminalState::new(TerminalDimensions::new(20, 2).unwrap());
        let mut parser = terminal_core::TerminalParser::new();
        parser.advance(&mut state, b"\x1b(B\x1b[0;1mH\x1b(B\x1b[melp\x1b[1;31mR\x1b[22mN\x1b[1;38;2;10;20;30mT\x1b[0mZ").unwrap();
        let theme = RenderTheme::default();
        let data = TerminalRenderData::from_terminal_with_theme(&state, &theme);
        assert!(data.cells[0].bold);
        assert_eq!(data.cells[0].foreground, theme.foreground);
        assert!(data.cells[1..4].iter().all(|cell| !cell.bold));
        assert!(data.cells[4].bold);
        assert!(!data.cells[5].bold);
        assert_eq!(data.cells[4].foreground, theme.ansi[1]);
        assert_eq!(data.cells[5].foreground, theme.ansi[1]);
        assert!(data.cells[6].bold);
        assert_eq!(
            data.cells[6].foreground,
            Rgba([10.0 / 255.0, 20.0 / 255.0, 30.0 / 255.0, 1.0])
        );
        assert!(!data.cells[7].bold);
        assert_eq!(data.cells[7].foreground, theme.foreground);
    }

    #[test]
    fn converts_resolved_cells_and_cursor_without_semantic_mutation() {
        let mut state = TerminalState::new(TerminalDimensions::new(3, 2).unwrap());
        state.set_foreground_color(CellColor::Rgb {
            red: 1,
            green: 2,
            blue: 3,
        });
        state.set_background_color(CellColor::Indexed(4));
        state.print_character('A').unwrap();
        let data = TerminalRenderData::from_terminal(&state);
        assert_eq!(data.cells[0].character, 'A');
        assert_eq!(
            data.cells[0].foreground,
            Rgba([1.0 / 255.0, 2.0 / 255.0, 3.0 / 255.0, 1.0])
        );
        assert_eq!(data.cursor, Some(CursorRenderData { row: 0, column: 1 }));
    }

    #[test]
    fn omits_wide_continuations_and_hides_cursor_when_core_requests_it() {
        let mut state = TerminalState::new(TerminalDimensions::new(3, 1).unwrap());
        state.print_character('界').unwrap();
        state.set_cursor_visibility(CursorVisibility::Hidden);
        let data = TerminalRenderData::from_terminal(&state);
        assert_eq!(data.cells.len(), 1);
        assert_eq!(data.cells[0].width, 2);
        assert_eq!(data.cursor, None);
    }

    #[test]
    fn projects_core_viewport_rows_and_hides_the_live_cursor_while_scrolled_back() {
        let mut state = TerminalState::new(TerminalDimensions::new(2, 2).unwrap());
        for character in "old".chars() {
            state.print_character(character).unwrap();
        }
        state.set_cursor_position(1, 0).unwrap();
        state.index();
        assert!(state.page_up());

        let data = TerminalRenderData::from_terminal(&state);
        assert_eq!(data.cursor, None);
        assert_eq!(
            data.scrollbar,
            Some(ScrollbarRenderData {
                history_rows: 1,
                visible_rows: 2,
                viewport_offset: 1
            })
        );
        assert_eq!(
            data.cells
                .iter()
                .filter(|cell| cell.row == 0)
                .map(|cell| cell.character)
                .collect::<Vec<_>>(),
            vec!['o', 'l']
        );
    }

    #[test]
    fn scrollbar_follows_primary_history_and_disappears_on_alternate_and_reset() {
        let mut state = TerminalState::new(TerminalDimensions::new(2, 2).unwrap());
        assert_eq!(TerminalRenderData::from_terminal(&state).scrollbar, None);
        state.set_cursor_position(1, 0).unwrap();
        for _ in 0..3 {
            state.index();
        }
        assert_eq!(
            TerminalRenderData::from_terminal(&state)
                .scrollbar
                .unwrap()
                .viewport_offset,
            0
        );
        assert!(state.scroll_viewport_rows(2));
        state.resize(TerminalDimensions::new(2, 3).unwrap());
        assert_eq!(
            TerminalRenderData::from_terminal(&state).scrollbar,
            Some(ScrollbarRenderData {
                history_rows: 2,
                visible_rows: 3,
                viewport_offset: 1
            })
        );
        state.switch_to_alternate_screen();
        assert_eq!(TerminalRenderData::from_terminal(&state).scrollbar, None);
        state.switch_to_primary_screen();
        assert_eq!(
            TerminalRenderData::from_terminal(&state)
                .scrollbar
                .unwrap()
                .viewport_offset,
            1
        );
        state.reset();
        assert_eq!(TerminalRenderData::from_terminal(&state).scrollbar, None);
    }

    #[test]
    fn alternate_region_scroll_projects_current_cells_despite_primary_scrollback() {
        let mut state = TerminalState::new(TerminalDimensions::new(2, 3).unwrap());
        let mut parser = TerminalParser::new();
        state.set_cursor_position(2, 0).unwrap();
        state.index();
        assert!(state.page_up());
        parser.advance(&mut state, b"\x1b[?1049h").unwrap();
        for (row, character) in ['a', 'b', 'c'].into_iter().enumerate() {
            state.set_cursor_position(row, 0).unwrap();
            state.print_character(character).unwrap();
        }
        parser
            .advance(&mut state, b"\x1b[1;2r\x1b[2;1H\r\n\x1b[1;3r\x1b[2;1Hx")
            .unwrap();

        let projection = TerminalRenderData::from_terminal(&state);
        assert_eq!(projection.scrollbar, None);
        assert_eq!(
            projection.cursor,
            Some(CursorRenderData { row: 1, column: 1 })
        );
        assert_eq!(
            projection
                .cells
                .iter()
                .map(|cell| (cell.row, cell.column, cell.character))
                .collect::<Vec<_>>(),
            [(0, 0, 'b'), (1, 0, 'x'), (2, 0, 'c')]
        );

        parser.advance(&mut state, b"\x1b[?1049l").unwrap();
        assert_eq!(
            TerminalRenderData::from_terminal(&state)
                .scrollbar
                .unwrap()
                .viewport_offset,
            1
        );
    }

    #[test]
    fn preserves_resolved_inverse_colors_and_underline_without_reinterpreting_them() {
        let mut state = TerminalState::new(TerminalDimensions::new(1, 1).unwrap());
        state.set_foreground_color(CellColor::Indexed(1));
        state.set_background_color(CellColor::Indexed(4));
        state.set_inverse_video(InverseVideo::Enabled);
        state.set_underline_style(UnderlineStyle::Enabled);
        state.print_character('A').unwrap();

        let cell = &TerminalRenderData::from_terminal(&state).cells[0];
        assert!(cell.underline);
        assert_eq!(cell.foreground, indexed_color(4));
        assert_eq!(cell.background, indexed_color(1));
    }

    #[test]
    fn carries_core_attached_width_zero_marks_as_one_renderer_text_run() {
        let mut state = TerminalState::new(TerminalDimensions::new(2, 1).unwrap());
        state.print_character('e').unwrap();
        state.print_character('\u{301}').unwrap();

        let cell = &TerminalRenderData::from_terminal(&state).cells[0];
        assert_eq!(cell.text.as_str(), "e\u{301}");
    }
}

#[cfg(test)]
mod opacity_tests {
    use super::*;
    use terminal_core::{CellColor, InverseVideo, TerminalDimensions};
    #[test]
    fn painted_and_erased_rgb_backgrounds_project_identically_at_every_opacity() {
        let mut state = TerminalState::new(TerminalDimensions::new(6, 1).unwrap());
        terminal_core::TerminalParser::new()
            .advance(&mut state, b"\x1b[?25l\x1b[48;2;32;32;32m  \x1b[K\x1b[49m")
            .unwrap();
        let theme = RenderTheme::default();
        for opacity in [0.0, 0.85, 1.0] {
            let data = TerminalRenderData::from_terminal_with_opacity(&state, &theme, opacity);
            assert_eq!(data.cells.len(), 6);
            for cell in &data.cells {
                assert_eq!(
                    cell.background,
                    Rgba([32.0 / 255.0, 32.0 / 255.0, 32.0 / 255.0, opacity])
                );
                assert_eq!(cell.foreground, theme.foreground);
                assert!(!cell.bold && !cell.underline);
            }
        }
    }

    #[test]
    fn opacity_changes_backgrounds_after_inverse_resolution() {
        let mut state = TerminalState::new(TerminalDimensions::new(5, 1).unwrap());
        state.set_background_color(CellColor::Indexed(1));
        state.print_character('A').unwrap();
        state.set_inverse_video(InverseVideo::Enabled);
        state.print_character('B').unwrap();
        let theme = RenderTheme::default();
        let original = TerminalRenderData::from_terminal_with_theme(&state, &theme);
        for opacity in [0.0, 0.85, 1.0] {
            let data = TerminalRenderData::from_terminal_with_opacity(&state, &theme, opacity);
            assert_eq!(
                &data.surface_background.0[..3],
                &original.surface_background.0[..3]
            );
            assert_eq!(data.surface_background.0[3], opacity);
            assert_eq!(data.cursor_color, original.cursor_color);
            for (cell, old) in data.cells.iter().zip(&original.cells) {
                assert_eq!(cell.foreground, old.foreground);
                assert_eq!(&cell.background.0[..3], &old.background.0[..3]);
                assert_eq!(cell.background.0[3], opacity);
            }
            if opacity == 1.0 {
                assert_eq!(data, original);
            }
        }
    }
    #[test]
    fn selected_background_and_overlay_are_not_faded() {
        let mut state = TerminalState::new(TerminalDimensions::new(5, 1).unwrap());
        state.print_character('A').unwrap();
        state.print_character('B').unwrap();
        state.print_character('C').unwrap();
        state.begin_selection(0, 0);
        state.extend_selection(0, 1);
        let theme = RenderTheme::default();
        let mut data = TerminalRenderData::from_terminal_with_opacity(&state, &theme, 0.0);
        assert_eq!(data.cells[0].background, theme.selection_background);
        assert_eq!(data.cells[0].foreground, theme.selection_foreground);
        assert_eq!(data.cells[2].background.0[3], 0.0);
        data.apply_text_overlay(
            &TextOverlay {
                lines: vec![OverlayLine {
                    text: "UI".into(),
                    selected: true,
                    accent_column: None,
                }],
                bottom: false,
                search_matches: Vec::new(),
                search_markers: Vec::new(),
            },
            &theme,
        );
        assert!(
            data.cells
                .iter()
                .all(|cell| cell.background == theme.ui.selected_background)
        );
        assert!(
            data.cells
                .iter()
                .all(|cell| cell.foreground == theme.ui.selected_foreground)
        );
    }
}
