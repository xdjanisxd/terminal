use crate::{ScrollbarRenderData, SurfaceSize};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScrollbarHit {
    Thumb,
    TrackAbove,
    TrackBelow,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollbarGeometry {
    pub track: [f32; 4],
    pub thumb: [f32; 4],
    history_rows: usize,
    viewport_offset: usize,
}

impl ScrollbarGeometry {
    pub fn new(data: ScrollbarRenderData, surface: SurfaceSize, cell_height: f32) -> Option<Self> {
        if data.history_rows == 0 || data.visible_rows == 0 || cell_height <= 0.0 {
            return None;
        }
        let surface_width = surface.width() as f32;
        let grid_height = (data.visible_rows as f32 * cell_height).min(surface.height() as f32);
        let inset = if grid_height > 4.0 { 2.0 } else { 0.0 };
        let track_height = grid_height - 2.0 * inset;
        if track_height <= 0.0 {
            return None;
        }
        let track_width = surface_width.min(8.0);
        let x = surface_width - track_width;
        let track = [x, inset, track_width, track_height];
        let total_rows = data.history_rows.saturating_add(data.visible_rows);
        let thumb_height = (track_height * data.visible_rows as f32 / total_rows as f32)
            .max(18.0)
            .min(track_height);
        let travel = track_height - thumb_height;
        let position = data.history_rows.saturating_sub(data.viewport_offset) as f32
            / data.history_rows as f32;
        let thumb = [x, inset + travel * position, track_width, thumb_height];
        Some(Self {
            track,
            thumb,
            history_rows: data.history_rows,
            viewport_offset: data.viewport_offset,
        })
    }

    pub fn hit(self, x: f64, y: f64) -> Option<ScrollbarHit> {
        if !x.is_finite()
            || !y.is_finite()
            || x < self.track[0] as f64
            || x >= (self.track[0] + self.track[2]) as f64
            || y < self.track[1] as f64
            || y >= (self.track[1] + self.track[3]) as f64
        {
            return None;
        }
        if y < self.thumb[1] as f64 {
            Some(ScrollbarHit::TrackAbove)
        } else if y >= (self.thumb[1] + self.thumb[3]) as f64 {
            Some(ScrollbarHit::TrackBelow)
        } else {
            Some(ScrollbarHit::Thumb)
        }
    }

    pub fn offset_for_drag(self, y: f64, grab_y: f64) -> usize {
        let travel = (self.track[3] - self.thumb[3]) as f64;
        if travel <= 0.0 {
            return self.viewport_offset;
        }
        let top = (y - grab_y - self.track[1] as f64).clamp(0.0, travel);
        ((1.0 - top / travel) * self.history_rows as f64).round() as usize
    }

    pub fn marker_y(self, row: usize, visible_rows: usize) -> f32 {
        let total = self.history_rows.saturating_add(visible_rows).max(1);
        self.track[1] + self.track[3] * (row.min(total - 1) as f32 + 0.5) / total as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_drag_and_markers_stay_in_bounds() {
        let data = ScrollbarRenderData {
            history_rows: 95,
            visible_rows: 5,
            viewport_offset: 0,
        };
        let geometry =
            ScrollbarGeometry::new(data, SurfaceSize::new(100, 100).unwrap(), 20.0).unwrap();
        assert_eq!(geometry.hit(99.0, 90.0), Some(ScrollbarHit::Thumb));
        assert_eq!(geometry.hit(99.0, 2.0), Some(ScrollbarHit::TrackAbove));
        assert_eq!(geometry.hit(91.0, 90.0), None);
        assert_eq!(geometry.hit(99.0, 98.0), None);
        assert_eq!(geometry.hit(f64::NAN, 90.0), None);
        assert_eq!(geometry.offset_for_drag(-100.0, 4.0), 95);
        assert_eq!(geometry.offset_for_drag(1000.0, 4.0), 0);
        assert!(geometry.offset_for_drag(45.0, 4.0) > geometry.offset_for_drag(55.0, 4.0));
        assert!(geometry.marker_y(0, 5) < geometry.marker_y(99, 5));
        assert!(geometry.marker_y(99, 5) < geometry.track[1] + geometry.track[3]);
    }

    #[test]
    fn full_track_thumb_keeps_its_offset() {
        let geometry = ScrollbarGeometry::new(
            ScrollbarRenderData {
                history_rows: 4,
                visible_rows: 2,
                viewport_offset: 3,
            },
            SurfaceSize::new(8, 12).unwrap(),
            6.0,
        )
        .unwrap();
        assert_eq!(geometry.thumb[3], geometry.track[3]);
        assert_eq!(geometry.offset_for_drag(100.0, 2.0), 3);
    }
}
