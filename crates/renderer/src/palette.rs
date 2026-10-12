//! Palette presentation only; filtering, selection, and execution belong to the app.
use crate::{CellMetrics, RenderCell, RenderText, RenderTheme, Rgba, TerminalRenderData};
use unicode_width::UnicodeWidthChar;

#[derive(Clone, Debug)]
pub struct PalettePanel {
    pub query: String,
    pub entries: Vec<String>,
    pub selected: usize,
}

pub(crate) struct PaletteDrawing {
    pub data: TerminalRenderData,
    pub rect: [u32; 4],
    pub metrics: CellMetrics,
    pub text_offset: [f32; 2],
    pub rectangles: Vec<([f32; 4], Rgba)>,
}

/// Quiet the terminal behind the palette without adding a layer of alpha.
pub(crate) fn dim(data: &mut TerminalRenderData, theme: &RenderTheme) {
    fn tint(color: &mut Rgba, toward: Rgba) {
        for channel in 0..3 {
            color.0[channel] = color.0[channel] * 0.86 + toward.0[channel] * 0.14;
        }
    }
    tint(&mut data.surface_background, theme.background);
    for cell in &mut data.cells {
        tint(&mut cell.foreground, theme.background);
        tint(&mut cell.background, theme.background);
    }
    data.cursor = None;
    data.scrollbar = None;
}

pub(crate) fn drawing(
    panel: &PalettePanel,
    size: [u32; 2],
    content_top: u32,
    scale: f64,
    metrics: CellMetrics,
    theme: &RenderTheme,
    opacity: f32,
) -> Option<PaletteDrawing> {
    let s = scale as f32;
    let metrics = metrics.with_vertical_padding((4.0 * s).round() as u32);
    let margin = ((16.0 * s).round() as u32)
        .min(size[0] / 8)
        .min(size[1].saturating_sub(content_top) / 8);
    let width = ((540.0 * s).round() as u32).min(size[0].saturating_sub(margin * 2));
    let padding = ((12.0 * s).round() as u32).min(width / 8);
    let available = size[1].saturating_sub(content_top + margin * 2);
    let rows = available.saturating_sub(padding * 2) / metrics.height();
    let columns = width.saturating_sub(padding * 2) / metrics.width();
    if rows == 0 || columns == 0 {
        return None;
    }
    let header = if rows >= 4 { 2 } else { 1 };
    let footer = usize::from(rows >= 4);
    let visible = (rows as usize - header - footer)
        .min(8)
        .min(panel.entries.len().max(1));
    let rows = visible + header + footer;
    let height = rows as u32 * metrics.height() + padding * 2;
    let rect = [(size[0] - width) / 2, content_top + margin, width, height];
    let first = panel
        .selected
        .saturating_sub(visible / 2)
        .min(panel.entries.len().saturating_sub(visible));

    // This is a local readability surface, not a whole-window opacity change.
    // At opacity 1 the existing opaque rendering path remains opaque.
    let mut background = theme.ui.background;
    background.0[3] = opacity.max(0.94);
    let mut rectangles = vec![([0.0, 0.0, width as f32, height as f32], background)];
    let mut border = theme.ui.border;
    border.0[3] *= 0.45;
    for edge in [
        [0.0, 0.0, width as f32, s],
        [0.0, height as f32 - s, width as f32, s],
        [0.0, 0.0, s, height as f32],
        [width as f32 - s, 0.0, s, height as f32],
    ] {
        rectangles.push((edge, border));
    }
    rectangles.push((
        [
            padding as f32,
            padding as f32 + metrics.height() as f32 * header as f32 - s,
            width.saturating_sub(padding * 2) as f32,
            s,
        ],
        border,
    ));
    let mut cells: Vec<RenderCell> = Vec::new();
    let mut line = |row: usize, text: &str, foreground: Rgba| {
        let mut column = 0;
        let capacity = columns as usize;
        let total: usize = text.chars().map(|c| c.width().unwrap_or(0)).sum();
        let limit = if total > capacity {
            capacity - 1
        } else {
            capacity
        };
        for character in text.chars().filter(|c| !c.is_control()) {
            let width = character.width().unwrap_or(0);
            if column + width > limit {
                break;
            }
            if width == 0 {
                if let Some(cell) = cells.last_mut().filter(|cell| cell.row == row) {
                    let text = format!("{}{character}", cell.text.as_str());
                    cell.text = RenderText::Combined(text);
                }
                continue;
            }
            cells.push(RenderCell {
                row,
                column,
                width,
                character,
                text: RenderText::Combined(character.to_string()),
                foreground,
                background,
                underline: false,
            });
            column += width;
        }
        if total > capacity {
            cells.push(RenderCell {
                row,
                column,
                width: 1,
                character: '…',
                text: RenderText::Combined("…".into()),
                foreground,
                background,
                underline: false,
            });
        }
    };
    if header == 2 {
        line(0, "Command Palette", theme.ui.muted);
    }
    if panel.query.is_empty() {
        line(header - 1, "Search commands…", theme.ui.muted);
    } else {
        // Keep the insertion end visible while typing a long query.
        let tail: String = panel.query.chars().rev().take(columns as usize).collect();
        let query: String = tail.chars().rev().collect();
        line(header - 1, &query, theme.ui.foreground);
    }
    if panel.entries.is_empty() && visible > 0 {
        line(header, "No matching commands", theme.ui.muted);
    } else {
        for (index, entry) in panel.entries.iter().enumerate().skip(first).take(visible) {
            let row = header + index - first;
            let selected = index == panel.selected;
            if selected {
                let mut selection = theme.ui.selected_background;
                selection.0[3] = 0.88;
                rectangles.push((
                    [
                        (padding / 2) as f32,
                        padding as f32 + row as f32 * metrics.height() as f32,
                        width.saturating_sub(padding) as f32,
                        metrics.height() as f32,
                    ],
                    selection,
                ));
            }
            line(
                row,
                entry,
                if selected {
                    theme.ui.selected_foreground
                } else {
                    theme.ui.foreground
                },
            );
        }
    }
    if footer != 0 {
        line(
            rows - 1,
            "↑↓ Navigate   Enter Run   Esc Close",
            theme.ui.muted,
        );
    }
    Some(PaletteDrawing {
        data: TerminalRenderData {
            rows,
            columns: columns as usize,
            cells,
            cursor: None,
            cursor_color: theme.cursor,
            surface_background: background,
            scrollbar: None,
            scrollbar_hover: None,
            search_markers: Vec::new(),
        },
        rect,
        metrics,
        text_offset: [padding as f32; 2],
        rectangles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_core::{TerminalDimensions, TerminalState};

    fn panel(selected: usize) -> PalettePanel {
        PalettePanel {
            query: String::new(),
            entries: (0..30).map(|i| format!("Command {i}")).collect(),
            selected,
        }
    }

    #[test]
    fn palette_selection_stays_visible_and_bounds_follow_surface_and_scale() {
        let theme = RenderTheme::default();
        for scale in [1.0, 1.2, 2.0] {
            for size in [[800, 600], [320, 190], [160, 160]] {
                for selected in [0, 7, 15, 29] {
                    let draw = drawing(
                        &panel(selected),
                        size,
                        32,
                        scale,
                        CellMetrics::from_physical(8, 16, 13.0),
                        &theme,
                        0.5,
                    )
                    .unwrap();
                    assert!(draw.rect[0] + draw.rect[2] <= size[0]);
                    assert!(draw.rect[1] + draw.rect[3] <= size[1]);
                    assert!(draw.rect[1] >= 32);
                    assert!(
                        draw.data
                            .cells
                            .iter()
                            .any(|c| c.foreground == theme.ui.selected_foreground)
                    );
                    for cell in &draw.data.cells {
                        assert!(cell.column + cell.width <= draw.data.columns);
                        assert!(cell.row < draw.data.rows);
                    }
                }
            }
        }
    }

    #[test]
    fn palette_uses_live_semantic_colors_without_fading_text() {
        for light in [false, true] {
            let surface = if light { 0.95 } else { 0.08 };
            let foreground = if light { 0.08 } else { 0.95 };
            let mut theme = RenderTheme::default();
            theme.ui.background = Rgba([surface, surface, surface, 1.0]);
            theme.ui.foreground = Rgba([foreground, foreground, foreground, 1.0]);
            theme.ui.selected_background = Rgba([0.3, 0.4, 0.5, 1.0]);
            theme.ui.selected_foreground = theme.ui.foreground;
            let mut panel = panel(29);
            panel.query = "a very long query with the insertion end here".repeat(5);
            panel.entries[29] = "界e\u{301} long project command".repeat(5);
            for opacity in [0.0, 0.5, 0.85, 1.0] {
                let draw = drawing(
                    &panel,
                    [800, 600],
                    32,
                    1.0,
                    CellMetrics::from_physical(8, 16, 13.0),
                    &theme,
                    opacity,
                )
                .unwrap();
                assert_eq!(
                    draw.data.surface_background.0[..3],
                    theme.ui.background.0[..3]
                );
                assert!(draw.data.surface_background.0[3] >= 0.94);
                assert!(draw.data.cells.iter().all(|c| c.foreground.0[3] >= 0.9));
                assert!(draw.data.cells.iter().any(|c| c.character == '界'));
                assert!(
                    draw.data
                        .cells
                        .iter()
                        .any(|c| c.text.as_str() == "e\u{301}")
                );
                assert!(draw.data.cells.iter().any(|c| c.character == '…'));
            }
        }
    }

    #[test]
    fn palette_dimming_preserves_surface_and_cell_alpha() {
        let mut state = TerminalState::new(TerminalDimensions::new(12, 4).unwrap());
        state.print_character('x').unwrap();
        let theme = RenderTheme::default();
        for opacity in [0.0, 0.5, 0.85, 1.0] {
            let mut data = TerminalRenderData::from_terminal_with_opacity(&state, &theme, opacity);
            let before = data.clone();
            dim(&mut data, &theme);
            assert_eq!(data.surface_background.0[3], opacity);
            for (after, before) in data.cells.iter().zip(before.cells.iter()) {
                assert_eq!(after.background.0[3], before.background.0[3]);
                assert_eq!(after.foreground.0[3], before.foreground.0[3]);
                assert_eq!(after.text, before.text);
            }
            assert!(data.cursor.is_none());
            assert!(data.cells[0].foreground.0[0] < before.cells[0].foreground.0[0]);
        }
    }
}
