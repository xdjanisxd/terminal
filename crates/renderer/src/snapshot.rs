//! Renderer-owned conversion of already-resolved terminal state into draw data.

use terminal_core::{
    CellColor, CellOccupancy, CursorVisibility, InverseVideo, TerminalState, UnderlineStyle,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba(pub [f32; 4]);

#[derive(Clone, Debug, PartialEq)]
pub struct RenderCell {
    pub row: usize,
    pub column: usize,
    pub width: usize,
    pub character: char,
    /// Base scalar plus width-zero marks already attached by `terminal-core`.
    pub text: String,
    pub foreground: Rgba,
    pub background: Rgba,
    pub underline: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CursorRenderData {
    pub row: usize,
    pub column: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TerminalRenderData {
    pub rows: usize,
    pub columns: usize,
    pub cells: Vec<RenderCell>,
    pub cursor: Option<CursorRenderData>,
}

impl TerminalRenderData {
    pub fn from_terminal(state: &TerminalState) -> Self {
        let dimensions = state.dimensions();
        let mut cells = Vec::with_capacity(dimensions.cell_count());
        for row in 0..dimensions.rows() {
            for column in 0..dimensions.columns() {
                let cell = state
                    .screen()
                    .cell(row, column)
                    .expect("validated dimensions");
                if cell.occupancy() == CellOccupancy::WideContinuation {
                    continue;
                }
                let attributes = cell.attributes();
                let (mut foreground, mut background) = (
                    color(attributes.foreground(), DEFAULT_FOREGROUND),
                    color(attributes.background(), DEFAULT_BACKGROUND),
                );
                if attributes.inverse() == InverseVideo::Enabled {
                    std::mem::swap(&mut foreground, &mut background);
                }
                cells.push(RenderCell {
                    row,
                    column,
                    width: usize::from(cell.occupancy() == CellOccupancy::WideLead) + 1,
                    character: cell.character(),
                    text: std::iter::once(cell.character())
                        .chain(cell.combining_marks().iter().copied())
                        .collect(),
                    foreground,
                    background,
                    underline: attributes.underline() == UnderlineStyle::Enabled,
                });
            }
        }
        let cursor = (state.terminal_modes().cursor_visibility() == CursorVisibility::Visible)
            .then(|| {
                let cursor = state.cursor();
                CursorRenderData {
                    row: cursor.row(),
                    column: cursor.column(),
                }
            });
        Self {
            rows: dimensions.rows(),
            columns: dimensions.columns(),
            cells,
            cursor,
        }
    }
}

const DEFAULT_FOREGROUND: Rgba = Rgba([0.9, 0.9, 0.9, 1.0]);
const DEFAULT_BACKGROUND: Rgba = Rgba([0.0, 0.0, 0.0, 1.0]);

fn color(color: CellColor, default: Rgba) -> Rgba {
    match color {
        CellColor::Default => default,
        CellColor::Rgb { red, green, blue } => Rgba([
            red as f32 / 255.0,
            green as f32 / 255.0,
            blue as f32 / 255.0,
            1.0,
        ]),
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
        CellColor, CursorVisibility, InverseVideo, TerminalDimensions, TerminalState,
        UnderlineStyle,
    };

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
        assert_eq!(data.cells.len(), 2);
        assert_eq!(data.cells[0].width, 2);
        assert_eq!(data.cursor, None);
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
        assert_eq!(cell.text, "e\u{301}");
    }
}
