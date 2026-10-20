//! App-owned drag geometry and scheduling; terminal-core owns selection and scrolling.

use super::*;

const SCROLL_INTERVAL: Duration = Duration::from_millis(16);
const MAX_ELAPSED: Duration = Duration::from_millis(50);
const MIN_SPEED: f64 = 12.0;
const MAX_SPEED: f64 = 1_200.0;

#[derive(Clone, Copy)]
pub(super) struct SelectionEdge {
    direction: i32,
    row: usize,
    column: usize,
    speed: f64,
    last_update: Instant,
    pending_rows: f64,
    pub(super) deadline: Instant,
}

impl SelectionEdge {
    pub(super) fn new(direction: i32, row: usize, column: usize, speed: f64, now: Instant) -> Self {
        Self {
            direction,
            row,
            column,
            speed,
            last_update: now,
            pending_rows: 0.0,
            deadline: now + SCROLL_INTERVAL,
        }
    }

    fn advance(&mut self, now: Instant) {
        let elapsed = now
            .saturating_duration_since(self.last_update)
            .min(MAX_ELAPSED);
        self.pending_rows = (self.pending_rows + self.speed * elapsed.as_secs_f64())
            .min(MAX_SPEED * MAX_ELAPSED.as_secs_f64());
        self.last_update = now;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct EdgeRate {
    direction: i32,
    speed: f64,
}

fn edge_speed(distance: f64, zone: f64) -> f64 {
    let penetration = (1.0 - distance / zone).clamp(0.0, 1.0);
    MIN_SPEED + (MAX_SPEED - MIN_SPEED) * penetration * penetration
}

/// Unlike press hit-testing, dragging includes the partial cells at pane edges.
/// Never clamp a pointer from another pane, chrome, or outside the window.
fn drag_cell_at(
    position: PhysicalPosition<f64>,
    metrics: CellMetrics,
    dimensions: TerminalDimensions,
    size: PhysicalSize<u32>,
) -> Option<(usize, usize, Option<EdgeRate>)> {
    if !position.x.is_finite()
        || !position.y.is_finite()
        || position.x < 0.0
        || position.y < 0.0
        || position.x >= f64::from(size.width)
        || position.y >= f64::from(size.height)
    {
        return None;
    }
    let column = ((position.x / f64::from(metrics.width())) as usize).min(dimensions.columns() - 1);
    let row = ((position.y / f64::from(metrics.height())) as usize).min(dimensions.rows() - 1);
    // Two physical cell heights scale with DPI. Reserve a neutral middle third
    // even in very short panes, so moving inward always stops scrolling.
    let text_bottom =
        (dimensions.rows() as f64 * f64::from(metrics.height())).min(f64::from(size.height));
    let zone = (2.0 * f64::from(metrics.height())).min(text_bottom / 3.0);
    let direction = if position.y < zone {
        Some(EdgeRate {
            direction: 1,
            speed: edge_speed(position.y, zone),
        })
    } else if position.y >= text_bottom - zone {
        Some(EdgeRate {
            direction: -1,
            speed: edge_speed(text_bottom - position.y, zone),
        })
    } else {
        None
    };
    Some((column, row, direction))
}

impl Application {
    fn selection_drag_owned(&self) -> bool {
        self.selection_dragging
            && self.terminal.has_selection()
            && !self.panel_open()
            && (self.terminal.input_modes().mouse_tracking() == MouseTracking::Off
                || self.modifiers.shift_key())
    }

    pub(super) fn stop_selection_drag(&mut self) {
        self.selection_dragging = false;
        self.selection_edge = None;
    }

    pub(super) fn refresh_selection_drag(&mut self, now: Instant) {
        let geometry = (|| {
            let position = self.pointer_position?;
            let metrics = self.renderer.as_ref()?.cell_metrics();
            let (_, rect) = self
                .pane_rects()?
                .into_iter()
                .find(|(pane, _)| *pane == self.workspace.active_pane())?;
            Some((
                PhysicalPosition::new(position.x - rect.x as f64, position.y - rect.y as f64),
                metrics,
                PhysicalSize::new(rect.width, rect.height),
            ))
        })();
        if let Some((position, metrics, size)) = geometry {
            self.update_selection_drag(position, metrics, size, now);
        } else {
            self.selection_edge = None;
        }
    }

    fn update_selection_drag(
        &mut self,
        position: PhysicalPosition<f64>,
        metrics: CellMetrics,
        size: PhysicalSize<u32>,
        now: Instant,
    ) {
        if !self.selection_drag_owned() {
            self.selection_edge = None;
            return;
        }
        let Some((column, row, direction)) =
            drag_cell_at(position, metrics, self.terminal.dimensions(), size)
        else {
            self.selection_edge = None;
            return;
        };
        if self.terminal.extend_selection(row, column) {
            self.selection_click = None;
            self.invalidate_frame();
        }
        self.selection_edge = direction
            .filter(|_| self.terminal.active_screen() == ScreenKind::Primary)
            .map(|EdgeRate { direction, speed }| {
                let mut edge = self
                    .selection_edge
                    .filter(|edge| edge.direction == direction)
                    .unwrap_or_else(|| SelectionEdge::new(direction, row, column, speed, now));
                // Account for time at the previous speed before applying motion.
                // Pointer event frequency never multiplies the scroll rate.
                edge.advance(now);
                edge.row = row;
                edge.column = column;
                edge.speed = speed;
                edge
            });
    }

    pub(super) fn tick_selection_edge(&mut self, now: Instant) {
        let Some(mut edge) = self.selection_edge else {
            return;
        };
        if !self.selection_drag_owned() || self.terminal.active_screen() != ScreenKind::Primary {
            self.selection_edge = None;
            return;
        }
        if now < edge.deadline {
            return;
        }
        edge.advance(now);
        let rows = edge.pending_rows.floor() as i32;
        edge.pending_rows -= f64::from(rows);
        edge.deadline = now + SCROLL_INTERVAL;
        if rows != 0 {
            if !self.terminal.scroll_selection_viewport_rows(
                edge.direction * rows,
                edge.row,
                edge.column,
            ) {
                self.selection_edge = None;
                return;
            }
            self.selection_click = None;
            // A batch scroll and selection extension invalidate only once.
            self.invalidate_frame();
        }
        // Reaching a bound drops the deadline immediately, including after a
        // successful clamped batch. No timer or redraw remains to keep alive.
        let offset = self.terminal.viewport_offset();
        self.selection_edge = if (edge.direction > 0 && offset == self.terminal.scrollback_len())
            || (edge.direction < 0 && offset == 0)
        {
            None
        } else {
            Some(edge)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const METRICS: CellMetrics = CellMetrics::from_physical(10, 20, 16.0);
    const SIZE: PhysicalSize<u32> = PhysicalSize::new(85, 125);

    fn app() -> Application {
        let mut app = Application {
            terminal: TerminalState::new(TerminalDimensions::new(8, 6).unwrap()),
            selection_dragging: true,
            ..Application::default()
        };
        TerminalParser::new()
            .advance(
                &mut app.terminal,
                b"one\r\ntwo\r\nthree\r\nfour\r\nfive\r\nsix\r\nseven\r\neight\r\nnine",
            )
            .unwrap();
        assert!(app.terminal.begin_selection(3, 2));
        app
    }

    fn move_to(app: &mut Application, y: f64, now: Instant) {
        app.update_selection_drag(PhysicalPosition::new(15.0, y), METRICS, SIZE, now);
    }

    #[test]
    fn edge_zones_include_partial_cells_and_respect_pane_boundaries() {
        let dims = TerminalDimensions::new(8, 6).unwrap();
        for (x, y, expected) in [
            (15.0, 0.0, Some((1, 0, Some(1)))),
            (15.0, 39.0, Some((1, 1, Some(1)))),
            (15.0, 40.0, Some((1, 2, None))),
            (15.0, 79.0, Some((1, 3, None))),
            (15.0, 85.0, Some((1, 4, Some(-1)))),
            (84.0, 124.0, Some((7, 5, Some(-1)))),
            (-1.0, 0.0, None),
            (85.0, 0.0, None),
            (15.0, -1.0, None),
            (15.0, 125.0, None),
            (f64::NAN, 0.0, None),
            (15.0, f64::INFINITY, None),
        ] {
            assert_eq!(
                drag_cell_at(PhysicalPosition::new(x, y), METRICS, dims, SIZE)
                    .map(|(column, row, edge)| (column, row, edge.map(|edge| edge.direction))),
                expected
            );
        }
        let dims = TerminalDimensions::new(8, 1).unwrap();
        for (y, direction) in [(0.0, Some(1)), (10.0, None), (19.0, Some(-1))] {
            assert_eq!(
                drag_cell_at(
                    PhysicalPosition::new(0.0, y),
                    METRICS,
                    dims,
                    PhysicalSize::new(80, 20)
                )
                .map(|(column, row, edge)| (
                    column,
                    row,
                    edge.map(|edge| edge.direction)
                )),
                Some((0, 0, direction))
            );
        }
        let scaled = CellMetrics::from_physical(20, 40, 32.0);
        assert_eq!(
            drag_cell_at(
                PhysicalPosition::new(30.0, 78.0),
                scaled,
                TerminalDimensions::new(8, 6).unwrap(),
                PhysicalSize::new(170, 250)
            )
            .map(|(column, row, edge)| (column, row, edge.map(|edge| edge.direction))),
            Some((1, 1, Some(1)))
        );
    }

    fn history_app() -> Application {
        let mut app = app();
        app.parser
            .advance(&mut app.terminal, "line\r\n".repeat(6_000).as_bytes())
            .unwrap();
        app.terminal.begin_selection(3, 2);
        app
    }

    #[test]
    fn speed_curve_is_precise_at_zone_entry_and_accelerates_toward_boundary() {
        for (distance, speed) in [
            (40.0, 12.0),
            (30.0, 86.25),
            (20.0, 309.0),
            (10.0, 680.25),
            (0.0, 1200.0),
            (-10.0, 1200.0),
        ] {
            assert_eq!(edge_speed(distance, 40.0), speed);
            assert_eq!(edge_speed(distance * 2.0, 80.0), speed);
        }
        for y in [0.0, 120.0, 124.0] {
            let (_, _, edge) = drag_cell_at(
                PhysicalPosition::new(15.0, y),
                METRICS,
                TerminalDimensions::new(8, 6).unwrap(),
                SIZE,
            )
            .unwrap();
            assert_eq!(edge.unwrap().speed, MAX_SPEED);
        }
        // Even with almost a full partial row, short panes have neutral space.
        assert_eq!(
            drag_cell_at(
                PhysicalPosition::new(15.0, 10.0),
                METRICS,
                TerminalDimensions::new(8, 1).unwrap(),
                PhysicalSize::new(85, 39)
            )
            .unwrap()
            .2,
            None
        );
    }

    #[test]
    fn stationary_drag_batches_extend_selection_in_both_directions_and_stop_at_bounds() {
        let mut app = app();
        let now = Instant::now();
        move_to(&mut app, 0.0, now);
        let before = app.terminal.selected_text();
        app.tick_selection_edge(now + SCROLL_INTERVAL - Duration::from_millis(1));
        assert_eq!(app.terminal.viewport_offset(), 0);
        app.tick_selection_edge(now + SCROLL_INTERVAL);
        assert_eq!(
            app.terminal.viewport_offset(),
            app.terminal.scrollback_len()
        );
        assert_ne!(app.terminal.selected_text(), before);
        assert!(app.selection_edge.is_none());
        app.frame.begin_redraw();
        let requests = app.frame.requests_armed;
        let selected = app.terminal.selected_text();
        app.tick_selection_edge(now + Duration::from_secs(2));
        assert_eq!(app.frame.requests_armed, requests);
        assert!(!app.frame.dirty);
        assert_eq!(app.terminal.selected_text(), selected);

        move_to(&mut app, 124.0, now + Duration::from_secs(3));
        app.tick_selection_edge(now + Duration::from_secs(3) + SCROLL_INTERVAL);
        assert_eq!(app.terminal.viewport_offset(), 0);
        assert!(app.selection_edge.is_none());
        assert_eq!(
            app.terminal.selected_text().as_deref(),
            Some("ven\neight\nni")
        );
    }

    #[test]
    fn elapsed_time_sets_speed_independent_of_wake_and_pointer_frequency() {
        for interval in [16, 20, 40] {
            for pointer_events in [false, true] {
                let mut app = history_app();
                let now = Instant::now();
                move_to(&mut app, 0.0, now);
                for millis in (interval..=2_000).step_by(interval as usize) {
                    let when = now + Duration::from_millis(millis);
                    if pointer_events {
                        move_to(&mut app, 0.0, when - Duration::from_millis(interval / 2));
                        move_to(&mut app, 0.0, when);
                    }
                    app.tick_selection_edge(when);
                }
                assert!(
                    (app.terminal.viewport_offset() as i32 - 2400).abs() <= 1,
                    "interval={interval}, pointer_events={pointer_events}"
                );
            }
        }
    }

    #[test]
    fn acceleration_retains_fractional_rows_and_reversal_resets_credit() {
        let mut app = history_app();
        let now = Instant::now();
        move_to(&mut app, 39.0, now); // 12.7425 rows/s.
        for index in 1..=5 {
            app.tick_selection_edge(now + SCROLL_INTERVAL * index);
        }
        assert_eq!(app.terminal.viewport_offset(), 1);
        let when = now + SCROLL_INTERVAL * 5;
        move_to(&mut app, 0.0, when);
        let deadline = app.selection_edge.unwrap().deadline;
        move_to(&mut app, 0.0, when + Duration::from_millis(1));
        assert_eq!(app.selection_edge.unwrap().deadline, deadline);
        app.tick_selection_edge(deadline);
        assert_eq!(app.terminal.viewport_offset(), 20);
        move_to(&mut app, 124.0, deadline);
        assert_eq!(app.selection_edge.unwrap().direction, -1);
        assert_eq!(app.selection_edge.unwrap().pending_rows, 0.0);
        app.tick_selection_edge(deadline);
        assert_eq!(app.terminal.viewport_offset(), 20);
        app.tick_selection_edge(deadline + SCROLL_INTERVAL);
        assert_eq!(app.terminal.viewport_offset(), 1);
    }

    #[test]
    fn long_stalls_have_bounded_work_and_no_catch_up_debt() {
        let mut app = history_app();
        let now = Instant::now();
        move_to(&mut app, 0.0, now);
        app.tick_selection_edge(now + Duration::from_secs(3));
        assert_eq!(app.terminal.viewport_offset(), 60);
        assert_eq!(app.selection_edge.unwrap().pending_rows, 0.0);
        app.tick_selection_edge(now + Duration::from_secs(3) + SCROLL_INTERVAL);
        assert_eq!(app.terminal.viewport_offset(), 79);
    }

    #[test]
    fn precise_speed_only_invalidates_when_a_row_moves() {
        let mut app = history_app();
        let now = Instant::now();
        move_to(&mut app, 39.0, now);
        app.frame.begin_redraw();
        let requests = app.frame.requests_armed;
        for index in 1..=4 {
            app.tick_selection_edge(now + SCROLL_INTERVAL * index);
        }
        assert_eq!(app.terminal.viewport_offset(), 0);
        assert_eq!(app.frame.requests_armed, requests);
        assert!(!app.frame.dirty);
        app.tick_selection_edge(now + SCROLL_INTERVAL * 5);
        assert_eq!(app.terminal.viewport_offset(), 1);
        assert_eq!(app.frame.requests_armed, requests + 1);
    }

    #[test]
    fn batched_scrolling_matches_single_row_copy_through_soft_wraps_and_reflow() {
        let mut batched = app();
        let mut reference = app();
        let now = Instant::now();
        for app in [&mut batched, &mut reference] {
            app.parser
                .advance(&mut app.terminal, "ab界cdefghij".repeat(100).as_bytes())
                .unwrap();
            app.terminal.begin_selection(3, 2);
            move_to(app, 0.0, now);
        }
        batched.tick_selection_edge(now + SCROLL_INTERVAL);
        for _ in 0..19 {
            assert!(reference.terminal.scroll_selection_viewport_rows(1, 0, 1));
        }
        assert_eq!(
            batched.terminal.selected_text(),
            reference.terminal.selected_text()
        );
        let copied = batched.terminal.selected_text();
        for app in [&mut batched, &mut reference] {
            app.terminal.resize(TerminalDimensions::new(5, 6).unwrap());
            assert_eq!(app.terminal.selected_text(), copied);
            move_to(app, 124.0, now + SCROLL_INTERVAL);
        }
        batched.tick_selection_edge(now + SCROLL_INTERVAL * 2);
        for _ in 0..19 {
            assert!(reference.terminal.scroll_selection_viewport_rows(-1, 5, 1));
        }
        assert_eq!(
            batched.terminal.viewport_offset(),
            reference.terminal.viewport_offset()
        );
        assert_eq!(
            batched.terminal.selected_text(),
            reference.terminal.selected_text()
        );
    }

    #[test]
    fn performance_scrolls_thousands_with_bounded_redraws_and_no_idle_work() {
        let mut app = history_app();
        let now = Instant::now();
        move_to(&mut app, 0.0, now);
        let requests = app.frame.requests_armed;
        let started = Instant::now();
        for index in 1..=250 {
            app.frame.begin_redraw();
            app.tick_selection_edge(now + SCROLL_INTERVAL * index);
        }
        let active_time = started.elapsed();
        assert!((app.terminal.viewport_offset() as i32 - 4800).abs() <= 1);
        assert_eq!(app.frame.requests_armed - requests, 250);
        app.stop_selection_drag();
        app.frame.begin_redraw();
        let requests = app.frame.requests_armed;
        let selected = app.terminal.selected_text();
        let idle_started = Instant::now();
        for _ in 0..1_000_000 {
            std::hint::black_box(&mut app).tick_selection_edge(std::hint::black_box(now));
        }
        let idle_time = idle_started.elapsed();
        assert_eq!(app.frame.requests_armed, requests);
        assert!(!app.frame.dirty);
        assert!(app.selection_edge.is_none());
        assert_eq!(app.terminal.selected_text(), selected);
        eprintln!(
            "autoscroll performance: 4800 rows in 4 simulated seconds, 250 redraw invalidations; active CPU={active_time:?}; 1000000 inactive calls={idle_time:?}, zero invalidations/deadlines"
        );
    }

    #[test]
    fn leaving_zone_or_pane_and_releasing_cancel_the_deadline() {
        for position in [
            PhysicalPosition::new(15.0, 60.0),
            PhysicalPosition::new(85.0, 1.0),
        ] {
            let mut app = app();
            let now = Instant::now();
            move_to(&mut app, 1.0, now);
            app.update_selection_drag(position, METRICS, SIZE, now);
            assert!(app.selection_edge.is_none());
            app.tick_selection_edge(now + Duration::from_secs(1));
            assert_eq!(app.terminal.viewport_offset(), 0);
        }
        let mut app = app();
        let now = Instant::now();
        move_to(&mut app, 1.0, now);
        app.stop_selection_drag();
        let copied = app.terminal.selected_text();
        app.tick_selection_edge(now + Duration::from_secs(1));
        assert!(!app.selection_dragging);
        assert!(app.selection_edge.is_none());
        assert_eq!(app.terminal.viewport_offset(), 0);
        assert_eq!(app.terminal.selected_text(), copied);
    }

    #[test]
    fn selection_end_modes_and_modal_input_cancel_scheduled_scrolls() {
        for change in 0..5 {
            let mut app = app();
            let now = Instant::now();
            move_to(&mut app, 1.0, now);
            match change {
                0 => {
                    app.terminal.clear_selection();
                }
                1 => app.selection_dragging = false,
                2 => app.terminal.switch_to_alternate_screen(),
                3 => {
                    app.parser
                        .advance(&mut app.terminal, b"\x1b[?1000h")
                        .unwrap();
                }
                _ => app.prepare_panel_input(),
            }
            app.tick_selection_edge(now + SCROLL_INTERVAL);
            assert!(app.selection_edge.is_none(), "change {change}");
            assert_eq!(app.terminal.viewport_offset(), 0);
        }
        let mut app = app();
        let now = Instant::now();
        app.parser
            .advance(&mut app.terminal, b"\x1b[?1002h")
            .unwrap();
        move_to(&mut app, 1.0, now);
        assert!(app.selection_edge.is_none());
        app.modifiers = ModifiersState::SHIFT;
        move_to(&mut app, 1.0, now);
        app.tick_selection_edge(now + SCROLL_INTERVAL);
        assert_eq!(
            app.terminal.viewport_offset(),
            app.terminal.scrollback_len()
        );
        app.modifiers = ModifiersState::empty();
        app.tick_selection_edge(now + SCROLL_INTERVAL * 2);
        assert!(app.selection_edge.is_none());
        assert_eq!(
            app.terminal.viewport_offset(),
            app.terminal.scrollback_len()
        );
    }
}
