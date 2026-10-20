//! Local hint label presentation; URL semantics stay in the app.
use crate::palette::PaletteDrawing;
use crate::{CellMetrics, RenderCell, RenderText, RenderTheme, TerminalRenderData};

#[derive(Clone, Debug)]
pub struct HintLabel {
    pub row: usize,
    pub start: usize,
    pub end: usize,
    pub text: String,
}

pub(crate) fn drawings(
    labels: &[HintLabel],
    pane: [u32; 4],
    cells: CellMetrics,
    metrics: CellMetrics,
    theme: &RenderTheme,
    opacity: f32,
) -> Vec<PaletteDrawing> {
    let padding = (metrics.width() / 4).max(1);
    let mut result: Vec<PaletteDrawing> = Vec::new();
    for label in labels {
        let width = label.text.len() as u32 * metrics.width() + padding * 2;
        let height = metrics.height();
        if width > pane[2] || height == 0 || height > pane[3] {
            continue;
        }
        let after = label.end as u32 * cells.width() + padding;
        let before = (label.start as u32 * cells.width()).saturating_sub(width + padding);
        let x = if after + width <= pane[2] {
            after
        } else {
            before
        };
        // Keep the full UI glyph readable when terminal text is smaller than
        // the hint font, and keep bottom-row labels inside their pane.
        let y = (label.row as u32 * cells.height() + cells.height().saturating_sub(height) / 2)
            .min(pane[3] - height);
        let mut rect = [pane[0] + x, pane[1] + y, width, height];
        // Dense text may force a label beside the start rather than the end.
        // Reserve nonoverlapping rectangles in deterministic source order.
        if result.iter().any(|d| overlaps(d.rect, rect)) {
            // Try the other side, then the URL start, before moving away from
            // the anchor. Keep every feasible label visible in dense content.
            let alternatives = [
                before,
                (label.start as u32 * cells.width()).min(pane[2] - width),
            ];
            if let Some(x) = alternatives.into_iter().find(|x| {
                !result
                    .iter()
                    .any(|d| overlaps(d.rect, [pane[0] + x, pane[1] + y, width, height]))
            }) {
                rect[0] = pane[0] + x;
            } else {
                // The viewport can be too small to fit nonoverlapping labels.
                // Retain the label at its anchor rather than silently hiding it.
                rect[0] = pane[0] + (label.start as u32 * cells.width()).min(pane[2] - width);
            }
        }
        let mut background = theme.ui.selected_background;
        background.0[3] = opacity.max(0.94);
        let data = TerminalRenderData {
            rows: 1,
            columns: label.text.len(),
            cells: label
                .text
                .chars()
                .enumerate()
                .map(|(column, character)| RenderCell {
                    row: 0,
                    column,
                    width: 1,
                    character,
                    text: RenderText::Combined(character.to_string()),
                    foreground: theme.ui.selected_foreground,
                    background,
                    underline: false,
                    bold: false,
                    italic: false,
                    concealed: false,
                    strikethrough: false,
                })
                .collect(),
            cursor: None,
            cursor_color: theme.cursor,
            surface_background: background,
            scrollbar: None,
            scrollbar_hover: None,
            search_markers: Vec::new(),
        };
        result.push(PaletteDrawing {
            data,
            rect,
            metrics,
            text_offset: [padding as f32, 0.0],
            rectangles: vec![
                ([0.0, 0.0, width as f32, height as f32], background),
                ([0.0, 0.0, width as f32, 1.0], theme.ui.accent),
            ],
        });
    }
    result
}

fn overlaps(a: [u32; 4], b: [u32; 4]) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn small_terminal_fonts_do_not_clip_hint_glyphs() {
        let cells = CellMetrics::from_physical(6, 8, 6.0);
        let metrics = CellMetrics::from_physical(7, 14, 11.0);
        let labels = [HintLabel {
            row: 3,
            start: 0,
            end: 4,
            text: "A".into(),
        }];
        let drawing = drawings(
            &labels,
            [0, 32, 100, 32],
            cells,
            metrics,
            &RenderTheme::default(),
            0.85,
        )
        .remove(0);
        assert_eq!(drawing.rect[3], metrics.height());
        assert_eq!(drawing.rect[1] + drawing.rect[3], 64);
    }
    #[test]
    fn labels_follow_pane_scale_theme_and_local_alpha() {
        for scale in [1.0, 1.25, 2.0] {
            let cells = CellMetrics::from_physical(
                (10.0 * scale) as u32,
                (20.0 * scale) as u32,
                16.0 * scale,
            );
            let metrics = CellMetrics::from_physical(
                (7.0 * scale) as u32,
                (14.0 * scale) as u32,
                11.0 * scale,
            );
            let pane = [43, 32, cells.width() * 30, cells.height() * 4];
            let labels = [HintLabel {
                row: 2,
                start: 22,
                end: 30,
                text: "AJ".into(),
            }];
            let mut theme = RenderTheme::default();
            theme.ui.selected_foreground = crate::Rgba([0.1, 0.2, 0.3, 1.0]);
            for opacity in [0.0, 0.85, 1.0] {
                let drawing = drawings(&labels, pane, cells, metrics, &theme, opacity).remove(0);
                assert!(
                    drawing.rect[0] >= pane[0]
                        && drawing.rect[0] + drawing.rect[2] <= pane[0] + pane[2]
                );
                assert!(
                    drawing.rect[1] >= pane[1]
                        && drawing.rect[1] + drawing.rect[3] <= pane[1] + pane[3]
                );
                assert_eq!(
                    drawing.data.cells[0].foreground,
                    theme.ui.selected_foreground
                );
                assert_eq!(drawing.data.surface_background.0[3], opacity.max(0.94));
                assert!(drawing.data.cursor.is_none());
            }
        }
    }
}
