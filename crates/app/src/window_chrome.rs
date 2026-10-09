//! Project-owned native chrome interactions; terminal input never handles chrome.
use std::time::{Duration, Instant};
use terminal_renderer::{CONTROL_WIDTH, TITLE_BAR_HEIGHT};
use terminal_workspace::PaneRect;
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    window::ResizeDirection,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    Control(usize),
    Drag,
    Resize(ResizeDirection),
}

pub fn height(scale: f64, fullscreen: bool) -> u32 {
    if fullscreen {
        0
    } else {
        (TITLE_BAR_HEIGHT * scale).round() as u32
    }
}

pub fn content_rect(size: PhysicalSize<u32>, height: u32) -> PaneRect {
    let y = height.min(size.height);
    PaneRect {
        x: 0,
        y,
        width: size.width,
        height: size.height.saturating_sub(y),
    }
}

pub fn hit(
    position: PhysicalPosition<f64>,
    size: PhysicalSize<u32>,
    scale: f64,
    maximized: bool,
    fullscreen: bool,
) -> Option<Hit> {
    let (x, y) = (position.x / scale, position.y / scale);
    let (w, h) = (size.width as f64 / scale, size.height as f64 / scale);
    if fullscreen || !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0 || x >= w || y >= h {
        return None;
    }
    if !maximized {
        let left = x < 4.0;
        let right = x >= w - 4.0;
        let top = y < 4.0;
        let bottom = y >= h - 4.0;
        let direction = match (left, right, top, bottom) {
            (true, _, true, _) => Some(ResizeDirection::NorthWest),
            (_, true, true, _) => Some(ResizeDirection::NorthEast),
            (true, _, _, true) => Some(ResizeDirection::SouthWest),
            (_, true, _, true) => Some(ResizeDirection::SouthEast),
            (true, _, _, _) => Some(ResizeDirection::West),
            (_, true, _, _) => Some(ResizeDirection::East),
            (_, _, true, _) => Some(ResizeDirection::North),
            (_, _, _, true) => Some(ResizeDirection::South),
            _ => None,
        };
        if let Some(direction) = direction {
            return Some(Hit::Resize(direction));
        }
    }
    (y < TITLE_BAR_HEIGHT).then(|| {
        if x < CONTROL_WIDTH * 3.0 {
            Hit::Control((x / CONTROL_WIDTH) as usize)
        } else {
            Hit::Drag
        }
    })
}

#[derive(Default)]
pub struct Interaction {
    pub hovered: Option<usize>,
    pub pressed: Option<usize>,
    pub held: bool,
    last_click: Option<(Instant, PhysicalPosition<f64>)>,
}

impl Interaction {
    pub fn double_click(
        &mut self,
        now: Instant,
        position: PhysicalPosition<f64>,
        scale: f64,
    ) -> bool {
        let double = self.last_click.is_some_and(|(then, previous)| {
            now.duration_since(then) <= Duration::from_millis(400)
                && (position.x - previous.x).abs() <= 5.0 * scale
                && (position.y - previous.y).abs() <= 5.0 * scale
        });
        self.last_click = if double { None } else { Some((now, position)) };
        double
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hit_regions_scale_and_fullscreen_hides_chrome() {
        let size = PhysicalSize::new(800, 600);
        assert_eq!(
            hit(PhysicalPosition::new(90.0, 30.0), size, 2.0, false, false),
            Some(Hit::Control(1))
        );
        assert_eq!(
            hit(PhysicalPosition::new(400.0, 30.0), size, 2.0, false, false),
            Some(Hit::Drag)
        );
        assert_eq!(
            hit(PhysicalPosition::new(1.0, 100.0), size, 1.0, false, false),
            Some(Hit::Resize(ResizeDirection::West))
        );
        assert_eq!(
            hit(PhysicalPosition::new(90.0, 30.0), size, 2.0, false, true),
            None
        );
        assert_eq!(
            content_rect(size, height(2.0, false)),
            PaneRect {
                x: 0,
                y: 64,
                width: 800,
                height: 536
            }
        );
    }
    #[test]
    fn double_click_requires_time_and_proximity() {
        let now = Instant::now();
        let mut interaction = Interaction::default();
        let position = PhysicalPosition::new(200.0, 16.0);
        assert!(!interaction.double_click(now, position, 1.0));
        assert!(interaction.double_click(now + Duration::from_millis(100), position, 1.0));
        assert!(!interaction.double_click(now + Duration::from_millis(200), position, 1.0));
        assert!(!interaction.double_click(
            now + Duration::from_millis(250),
            PhysicalPosition::new(240.0, 16.0),
            1.0
        ));
    }
}
