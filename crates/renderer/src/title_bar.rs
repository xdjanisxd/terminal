//! Presentation-only title bar geometry and draw data. The app resolves context/actions.
use crate::{CellMetrics, RenderCell, RenderText, RenderTheme, TerminalRenderData};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub const TITLE_BAR_HEIGHT: f64 = 32.0;
pub const CONTROL_WIDTH: f64 = 30.0;

#[derive(Clone, Debug)]
pub struct TitleBar {
    pub path: String,
    pub hovered_control: Option<usize>,
    pub pressed_control: Option<usize>,
    pub maximized: bool,
}

pub(crate) struct TitleBarDrawing {
    pub data: TerminalRenderData,
    pub text_offset: [f32; 2],
    pub rectangles: Vec<([f32; 4], crate::Rgba)>,
}

pub(crate) fn drawing(
    bar: &TitleBar,
    width: u32,
    scale: f64,
    metrics: CellMetrics,
    theme: &RenderTheme,
    opacity: f32,
) -> TitleBarDrawing {
    let height = (TITLE_BAR_HEIGHT * scale).round() as f32;
    let control_width = (CONTROL_WIDTH * scale) as f32;
    let cell_width = metrics.width() as f32;
    // Symmetric exclusion keeps the context centered on the window, even with left controls.
    let available = (width as f32 - 2.0 * (3.0 * control_width + 8.0 * scale as f32)).max(0.0);
    let capacity = (available / cell_width) as usize;
    let clean = |s: &str| s.chars().filter(|c| !c.is_control()).collect::<String>();
    let path = truncate(&clean(&bar.path), capacity);
    let columns = path.width();
    let mut background = theme.background;
    background.0[3] = opacity;
    let mut column = 0;
    let mut cells: Vec<RenderCell> = Vec::new();
    for character in path.chars() {
        let width = character.width().unwrap_or(0);
        if width == 0 {
            if let Some(cell) = cells.last_mut() {
                let mut text = cell.text.as_str().to_owned();
                text.push(character);
                cell.text = RenderText::Combined(text);
            }
            continue;
        }
        cells.push(RenderCell {
            row: 0,
            column,
            width,
            character,
            text: RenderText::Combined(character.to_string()),
            foreground: theme.ui.muted,
            background,
            underline: false,
        });
        column += width;
    }
    let data = TerminalRenderData {
        rows: 1,
        columns,
        cells,
        cursor: None,
        cursor_color: theme.cursor,
        surface_background: background,
        scrollbar: None,
        scrollbar_hover: None,
        search_markers: Vec::new(),
    };
    let mut rectangles = Vec::new();
    let mut line = |x, y, w, h, color| rectangles.push(([x, y, w, h], color));
    let s = scale as f32;
    for control in 0..3 {
        let x = control as f32 * control_width;
        if bar.hovered_control == Some(control) || bar.pressed_control == Some(control) {
            let mut color = theme.ui.selected_background;
            color.0[3] = if bar.pressed_control == Some(control) {
                0.55
            } else {
                0.28
            };
            line(
                x + 2.0 * s,
                3.0 * s,
                control_width - 4.0 * s,
                height - 6.0 * s,
                color,
            );
        }
        let color = if bar.pressed_control == Some(control) {
            theme.ui.selected_foreground
        } else {
            theme.ui.foreground
        };
        let cx = x + control_width / 2.0;
        let cy = height / 2.0;
        match control {
            2 => line(cx - 5.0 * s, cy + 3.0 * s, 10.0 * s, s, color),
            1 => {
                let mut square = |left: f32, top: f32, size: f32| {
                    line(left, top, size, s, color);
                    line(left, top + size - s, size, s, color);
                    line(left, top, s, size, color);
                    line(left + size - s, top, s, size, color);
                };
                if bar.maximized {
                    square(cx - 2.0 * s, cy - 5.0 * s, 7.0 * s);
                    square(cx - 5.0 * s, cy - 2.0 * s, 7.0 * s);
                } else {
                    square(cx - 4.0 * s, cy - 4.0 * s, 9.0 * s);
                }
            }
            _ => {
                for step in 0..9 {
                    let d = (step as f32 - 4.0) * s;
                    line(cx + d, cy + d, s, s, color);
                    line(cx + d, cy - d, s, s, color);
                }
            }
        }
    }
    TitleBarDrawing {
        data,
        text_offset: [
            ((width as f32 - columns as f32 * cell_width) / 2.0).max(0.0),
            ((height - metrics.height() as f32) / 2.0).max(0.0),
        ],
        rectangles,
    }
}

fn truncate(text: &str, capacity: usize) -> String {
    if text.width() <= capacity {
        return text.to_owned();
    }
    if capacity == 0 {
        return String::new();
    }
    let mut output = String::new();
    let mut used = 0;
    for character in text.chars() {
        let width = character.width().unwrap_or(0);
        if used + width > capacity - 1 {
            break;
        }
        used += width;
        output.push(character);
    }
    output.push('…');
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn truncation_respects_wide_characters_and_combining_marks() {
        assert_eq!(truncate("界界界", 5), "界界…");
        assert_eq!(truncate("e\u{301}abc", 3), "e\u{301}a…");
        assert_eq!(truncate("abc", 0), "");
    }
    #[test]
    fn centered_path_uses_theme_and_avoids_controls() {
        let metrics = CellMetrics::from_physical(8, 16, 13.0);
        let theme = RenderTheme::default();
        let bar = TitleBar {
            path: "backend/src".into(),
            hovered_control: None,
            pressed_control: None,
            maximized: false,
        };
        let draw = drawing(&bar, 800, 1.0, metrics, &theme, 0.6);
        assert_eq!(draw.text_offset[0] + draw.data.columns as f32 * 4.0, 400.0);
        assert_eq!(draw.data.surface_background.0[3], 0.6);
        assert_eq!(
            draw.data
                .cells
                .iter()
                .map(|cell| cell.text.as_str())
                .collect::<String>(),
            "backend/src"
        );
        assert_eq!(
            draw.data.cells.last().map(|cell| cell.column + cell.width),
            Some(draw.data.columns)
        );
        assert!(
            draw.data
                .cells
                .iter()
                .all(|cell| cell.foreground == theme.ui.muted)
        );
        // Close occupies the first slot, maximize the second, minimize the third.
        assert_eq!(draw.rectangles.len(), 18 + 4 + 1);
        assert!(draw.rectangles[..18].iter().all(|(rect, _)| rect[0] < 30.0));
        assert!(
            draw.rectangles[18..22]
                .iter()
                .all(|(rect, _)| (30.0..60.0).contains(&rect[0]))
        );
        assert_eq!(draw.rectangles[22].0, [70.0, 19.0, 10.0, 1.0]);
        let narrow = drawing(&bar, 160, 1.0, metrics, &theme, 1.0);
        assert!(narrow.data.cells.is_empty());
    }
}
