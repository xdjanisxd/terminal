//! Native application lifecycle and component wiring.

use std::sync::Arc;

use terminal_core::{
    CellColor, MAX_COLUMNS, MAX_GRID_CELLS, MAX_ROWS, TerminalDimensions, TerminalState,
    UnderlineStyle,
};
use terminal_renderer::{
    CellMetrics, RedrawOutcome, Renderer, RendererDiagnosticState, diagnostics_enabled,
    emit_diagnostic,
};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

struct Application {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    terminal: TerminalState,
    frame: FrameState,
    dpi_size_sync: PhysicalSizeSync,
    surface_restore: SurfaceRestore,
    recovery_redraw: RecoveryRedraw,
    fullscreen: FullscreenTransition,
    last_diagnostic_state: Option<AppDiagnosticState>,
}

/// App-owned redraw coalescing; it has no terminal semantics.
#[derive(Debug, Default)]
struct FrameState {
    dirty: bool,
    redraw_requested: bool,
}

impl FrameState {
    /// Marks the frame dirty and returns whether winit needs one redraw request.
    fn invalidate(&mut self) -> bool {
        self.dirty = true;
        if self.redraw_requested {
            false
        } else {
            self.redraw_requested = true;
            true
        }
    }

    /// Consumes app-owned damage before handling a compositor redraw request.
    ///
    /// A `RedrawRequested` event is itself sufficient reason to present: the
    /// compositor can request one after an expose or surface transition even
    /// when the app has not queued damage.
    fn begin_redraw(&mut self) {
        self.redraw_requested = false;
        self.dirty = false;
    }

    /// Drops a queued request when the window is suspended.
    fn suspend(&mut self) {
        self.redraw_requested = false;
    }

    /// Re-arms a redraw after a surface transition may have discarded winit's
    /// earlier queued request.
    fn rearm(&mut self) {
        self.redraw_requested = false;
        self.dirty = true;
    }
}

/// Coalesces scale-factor events until the window's physical size is stable.
#[derive(Debug, Default)]
struct PhysicalSizeSync {
    pending: bool,
}

impl PhysicalSizeSync {
    fn schedule(&mut self) {
        self.pending = true;
    }

    fn take(&mut self) -> bool {
        std::mem::take(&mut self.pending)
    }
}

/// Defers a recovery frame until the window has a drawable physical size.
///
/// Winit can drop a previously requested redraw while a window is minimized.
/// Keeping this state separate from [`FrameState`] makes the next non-zero
/// surface transition explicitly request one replacement frame.
#[derive(Debug, Default)]
struct SurfaceRestore {
    pending: bool,
}

/// Bounds a compositor recovery to an initial present and one successor present.
///
/// The successor is only requested after the initial recovery frame was actually
/// presented. Surface errors and skipped frames therefore cannot form a retry
/// loop.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum RecoveryRedraw {
    #[default]
    Idle,
    AwaitingInitialPresent {
        reconfigure_redraw_issued: bool,
    },
    AwaitingFollowUpPresent,
}

impl RecoveryRedraw {
    fn begin(&mut self) {
        if *self == Self::Idle {
            *self = Self::AwaitingInitialPresent {
                reconfigure_redraw_issued: false,
            };
        }
    }

    /// Re-arms once after a renderer surface recovery, then waits for an OS
    /// redraw rather than retrying indefinitely if the surface stays invalid.
    fn reconfigured(&mut self) -> bool {
        self.begin();
        match self {
            Self::AwaitingInitialPresent {
                reconfigure_redraw_issued,
            } if !*reconfigure_redraw_issued => {
                *reconfigure_redraw_issued = true;
                true
            }
            Self::Idle | Self::AwaitingInitialPresent { .. } | Self::AwaitingFollowUpPresent => {
                false
            }
        }
    }

    /// Records a successful presentation and returns whether to queue the one
    /// compositor follow-up frame.
    fn presented(&mut self) -> bool {
        match self {
            Self::Idle => false,
            Self::AwaitingInitialPresent { .. } => {
                *self = Self::AwaitingFollowUpPresent;
                true
            }
            Self::AwaitingFollowUpPresent => {
                *self = Self::Idle;
                false
            }
        }
    }

    /// A skipped frame deliberately leaves recovery pending without adding work.
    /// Only an actual presentation is allowed to advance this state machine.
    fn skipped(&mut self) {}
}

/// Detects a fullscreen boundary from the authoritative window state observed
/// with resize events, without treating ordinary resizes as recovery.
#[derive(Debug, Default)]
struct FullscreenTransition {
    previous: Option<bool>,
}

impl FullscreenTransition {
    fn observe(&mut self, fullscreen: bool) -> bool {
        let changed = self.previous.is_some_and(|previous| previous != fullscreen);
        self.previous = Some(fullscreen);
        changed
    }
}

/// A compact app-owned lifecycle snapshot used only to suppress duplicate
/// opt-in diagnostic lines.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AppDiagnosticState {
    window_size: Option<(u32, u32)>,
    terminal_grid: (usize, usize),
    frame_dirty: bool,
    redraw_queued: bool,
    dpi_sync_pending: bool,
    restore_pending: bool,
    recovery_redraw: RecoveryRedraw,
    renderer: Option<RendererDiagnosticState>,
}

impl SurfaceRestore {
    fn defer(&mut self) {
        self.pending = true;
    }

    fn take_if_drawable(&mut self, size: PhysicalSize<u32>) -> bool {
        if size.width == 0 || size.height == 0 {
            return false;
        }
        std::mem::take(&mut self.pending)
    }
}

impl Default for Application {
    fn default() -> Self {
        let mut terminal =
            TerminalState::new(TerminalDimensions::new(80, 24).expect("valid default"));
        if std::env::var_os("TERMINAL_RENDERER_VISUAL_SMOKE").is_some_and(|value| value == "1") {
            populate_visual_smoke_state(&mut terminal);
        }
        Self {
            window: None,
            renderer: None,
            terminal,
            frame: FrameState::default(),
            dpi_size_sync: PhysicalSizeSync::default(),
            surface_restore: SurfaceRestore::default(),
            recovery_redraw: RecoveryRedraw::default(),
            fullscreen: FullscreenTransition::default(),
            last_diagnostic_state: None,
        }
    }
}

/// Creates deterministic content for manual renderer acceptance checks only.
fn populate_visual_smoke_state(terminal: &mut TerminalState) {
    terminal.set_background_color(CellColor::Rgb {
        red: 20,
        green: 55,
        blue: 120,
    });
    terminal.set_foreground_color(CellColor::Rgb {
        red: 245,
        green: 245,
        blue: 245,
    });
    print_smoke_text(terminal, "GLYPHS  background");
    terminal.line_feed();
    terminal.carriage_return();

    terminal.set_background_color(CellColor::Rgb {
        red: 85,
        green: 25,
        blue: 100,
    });
    terminal.set_foreground_color(CellColor::Rgb {
        red: 40,
        green: 235,
        blue: 100,
    });
    terminal.set_underline_style(UnderlineStyle::Enabled);
    print_smoke_text(terminal, "UNDERLINE  decoration");
    terminal.set_underline_style(UnderlineStyle::Disabled);
    terminal.line_feed();
    terminal.carriage_return();

    terminal.set_background_color(CellColor::Default);
    terminal.set_foreground_color(CellColor::Rgb {
        red: 240,
        green: 200,
        blue: 80,
    });
    print_smoke_text(terminal, "wide: 界   combining: e\u{301}");
}

fn print_smoke_text(terminal: &mut TerminalState, text: &str) {
    for character in text.chars() {
        terminal
            .print_character(character)
            .expect("visual smoke text is printable");
    }
}

impl Application {
    fn diagnose(&mut self, event: &str) {
        if !diagnostics_enabled() {
            return;
        }
        let dimensions = self.terminal.dimensions();
        let state = AppDiagnosticState {
            window_size: self.window.as_ref().map(|window| {
                let size = window.inner_size();
                (size.width, size.height)
            }),
            terminal_grid: (dimensions.columns(), dimensions.rows()),
            frame_dirty: self.frame.dirty,
            redraw_queued: self.frame.redraw_requested,
            dpi_sync_pending: self.dpi_size_sync.pending,
            restore_pending: self.surface_restore.pending,
            recovery_redraw: self.recovery_redraw,
            renderer: self.renderer.as_ref().map(Renderer::diagnostic_state),
        };
        if self.last_diagnostic_state == Some(state) {
            return;
        }
        self.last_diagnostic_state = Some(state);
        emit_diagnostic(format_args!(
            "app state-change trigger={event} state={state:?}"
        ));
    }

    fn create_window_and_renderer(&mut self, event_loop: &ActiveEventLoop) {
        let recreating_surface = self.window.is_some();
        if self.window.is_none() {
            let attributes = Window::default_attributes().with_title("Terminal");
            match event_loop.create_window(attributes) {
                Ok(window) => self.window = Some(Arc::new(window)),
                Err(error) => {
                    eprintln!("could not create terminal window: {error}");
                    event_loop.exit();
                    return;
                }
            }
        }

        let window = self.window.as_ref().unwrap();
        match Renderer::new(Arc::clone(window)) {
            Ok(renderer) => {
                self.renderer = Some(renderer);
                self.fullscreen.observe(window.fullscreen().is_some());
                if recreating_surface {
                    self.recovery_redraw.begin();
                }
                self.resize_terminal_to_viewport(window.inner_size());
                self.invalidate_frame();
                self.diagnose("renderer-created");
            }
            Err(error) => {
                eprintln!("could not initialize terminal renderer: {error}");
                event_loop.exit();
            }
        }
    }

    fn invalidate_frame(&mut self) {
        if self.frame.invalidate()
            && let Some(window) = self.window.as_ref()
        {
            window.request_redraw();
            self.diagnose("redraw-requested");
        } else {
            self.diagnose("redraw-coalesced");
        }
    }

    /// Requests the one frame needed after a minimized or occluded surface
    /// becomes drawable again. This deliberately replaces a request that the
    /// compositor may have discarded; it is not a continuous retry loop.
    fn invalidate_after_surface_restore(&mut self) {
        self.frame.rearm();
        self.invalidate_frame();
    }

    /// Configures the renderer from winit's authoritative physical resize event.
    fn resize_renderer(&mut self, size: PhysicalSize<u32>, force_redraw: bool) {
        self.diagnose("resize-start");
        let surface_changed = self
            .renderer
            .as_mut()
            .is_some_and(|renderer| renderer.resize(size.width, size.height));
        let grid_changed = self.resize_terminal_to_viewport(size);
        if force_redraw {
            self.recovery_redraw.begin();
            self.invalidate_after_surface_restore();
        } else if surface_changed || grid_changed {
            self.invalidate_frame();
        }
        self.diagnose("resize-complete");
    }

    fn resize_terminal_to_viewport(&mut self, size: PhysicalSize<u32>) -> bool {
        let Some(metrics) = self.renderer.as_ref().map(Renderer::cell_metrics) else {
            return false;
        };
        let Some(dimensions) = terminal_dimensions_for_viewport(size, metrics) else {
            return false;
        };
        if self.terminal.dimensions() == dimensions {
            return false;
        }
        self.terminal.resize(dimensions);
        true
    }

    /// Handles the rare scale-factor path that does not deliver its resulting
    /// physical size through `Resized` in the same event sequence.
    fn sync_renderer_to_window_size(&mut self) {
        let size = self.window.as_ref().map(|window| window.inner_size());
        if let Some(size) = size {
            let force_redraw = self.surface_restore.take_if_drawable(size);
            self.resize_renderer(size, force_redraw);
        }
    }

    fn restore_surface_if_drawable(&mut self) {
        let size = self.window.as_ref().map(|window| window.inner_size());
        if let Some(size) = size
            && self.surface_restore.take_if_drawable(size)
        {
            self.resize_renderer(size, true);
        }
    }
}

impl ApplicationHandler for Application {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        emit_diagnostic(format_args!("app event=resumed"));
        self.diagnose("resumed");
        if self.renderer.is_none() {
            self.create_window_and_renderer(event_loop);
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        emit_diagnostic(format_args!("app event=suspended"));
        self.diagnose("suspended-start");
        self.renderer = None;
        self.frame.suspend();
        self.dpi_size_sync = PhysicalSizeSync::default();
        self.surface_restore = SurfaceRestore::default();
        self.recovery_redraw = RecoveryRedraw::default();
        self.fullscreen = FullscreenTransition::default();
        self.diagnose("suspended-complete");
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if self.dpi_size_sync.take() {
            self.sync_renderer_to_window_size();
        }
        self.restore_surface_if_drawable();
        self.diagnose("about-to-wait");
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(active_window_id) = self.window.as_ref().map(|window| window.id()) else {
            return;
        };
        if active_window_id != window_id {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                emit_diagnostic(format_args!("app event=close-requested"));
                self.diagnose("close-requested");
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                emit_diagnostic(format_args!(
                    "app event=resized physical_size={}x{}",
                    size.width, size.height
                ));
                self.diagnose("resized-received");
                if size.width == 0 || size.height == 0 {
                    self.surface_restore.defer();
                }
                let fullscreen = self
                    .window
                    .as_ref()
                    .is_some_and(|window| window.fullscreen().is_some());
                let fullscreen_changed = self.fullscreen.observe(fullscreen);
                let force_redraw =
                    self.surface_restore.take_if_drawable(size) || fullscreen_changed;
                self.resize_renderer(size, force_redraw);
                self.diagnose("resized-handled");
            }
            // Winit applies the OS-suggested physical size by default and emits a
            // `Resized` event for it. Repaint here so an unchanged physical size
            // still recovers after an expose/DPI transition without querying a
            // potentially stale window size.
            WindowEvent::ScaleFactorChanged { .. } => {
                let scale_factor = self
                    .window
                    .as_ref()
                    .map(|window| window.scale_factor())
                    .unwrap_or(1.0);
                emit_diagnostic(format_args!(
                    "app event=scale-factor-changed scale_factor={}",
                    scale_factor
                ));
                self.diagnose("scale-factor-changed-received");
                if let Some(renderer) = self.renderer.as_mut()
                    && renderer.set_scale_factor(scale_factor).unwrap_or(false)
                {
                    let size = self.window.as_ref().map(|window| window.inner_size());
                    if let Some(size) = size {
                        self.resize_terminal_to_viewport(size);
                    }
                }
                self.dpi_size_sync.schedule();
                self.invalidate_frame();
                self.diagnose("scale-factor-changed-handled");
            }
            WindowEvent::Occluded(false) => {
                self.diagnose("occluded-false-received");
                self.surface_restore.defer();
                self.diagnose("occluded-false-handled");
            }
            WindowEvent::Occluded(true) => {
                self.diagnose("occluded-true-received");
                self.surface_restore.defer();
                self.diagnose("occluded-true-handled");
            }
            WindowEvent::RedrawRequested => {
                emit_diagnostic(format_args!("app event=redraw-requested"));
                self.diagnose("redraw-requested-received");
                self.frame.begin_redraw();
                let Some(renderer) = self.renderer.as_mut() else {
                    return;
                };
                let outcome = renderer.redraw_terminal(&self.terminal);
                emit_diagnostic(format_args!(
                    "app event=redraw-complete outcome={outcome:?}"
                ));
                match outcome {
                    RedrawOutcome::Reconfigured => {
                        if self.recovery_redraw.reconfigured() {
                            self.invalidate_frame();
                        }
                    }
                    RedrawOutcome::Exit => event_loop.exit(),
                    RedrawOutcome::Presented => {
                        if self.recovery_redraw.presented() {
                            self.invalidate_frame();
                        }
                    }
                    RedrawOutcome::Skipped => self.recovery_redraw.skipped(),
                }
                self.diagnose("redraw-handled");
            }
            _ => {}
        }
    }
}

fn terminal_dimensions_for_viewport(
    size: PhysicalSize<u32>,
    metrics: CellMetrics,
) -> Option<TerminalDimensions> {
    if size.width == 0 || size.height == 0 {
        return None;
    }
    let columns = (size.width as usize / metrics.width() as usize).clamp(1, MAX_COLUMNS);
    let rows = (size.height as usize / metrics.height() as usize)
        .clamp(1, MAX_ROWS)
        .min(MAX_GRID_CELLS / columns);
    TerminalDimensions::new(columns, rows).ok()
}

fn main() {
    let event_loop = EventLoop::new().expect("could not create terminal event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop
        .run_app(&mut Application::default())
        .expect("terminal event loop failed");
}

#[cfg(test)]
mod tests {
    use super::{
        FrameState, PhysicalSizeSync, RecoveryRedraw, SurfaceRestore,
        terminal_dimensions_for_viewport,
    };
    use terminal_renderer::CellMetrics;
    use winit::dpi::PhysicalSize;

    #[test]
    fn invalidation_coalesces_requests_until_redraw_consumes_the_damage() {
        let mut frame = FrameState::default();

        assert!(frame.invalidate());
        assert!(!frame.invalidate());
        frame.begin_redraw();
        assert!(!frame.dirty);
        assert!(!frame.redraw_requested);
        assert!(frame.invalidate());
    }

    #[test]
    fn compositor_redraw_clears_damage_even_without_an_app_queued_request() {
        let mut frame = FrameState::default();

        frame.begin_redraw();
        assert!(!frame.dirty);
        assert!(!frame.redraw_requested);
        assert!(frame.invalidate());
    }

    #[test]
    fn scale_factor_transitions_coalesce_one_physical_size_sync() {
        let mut sync = PhysicalSizeSync::default();

        sync.schedule();
        sync.schedule();
        assert!(sync.take());
        assert!(!sync.take());
    }

    #[test]
    fn restore_rearms_one_redraw_after_a_zero_sized_surface() {
        let mut frame = FrameState::default();
        let mut restore = SurfaceRestore::default();
        let mut recovery = RecoveryRedraw::default();

        assert!(frame.invalidate());
        restore.defer();
        assert!(!restore.take_if_drawable(PhysicalSize::new(0, 0)));
        assert!(restore.take_if_drawable(PhysicalSize::new(800, 600)));

        recovery.begin();
        frame.rearm();
        assert!(frame.invalidate());
        assert!(!frame.invalidate());
        assert!(!restore.take_if_drawable(PhysicalSize::new(800, 600)));

        frame.begin_redraw();
        assert!(recovery.presented());
        assert!(frame.invalidate());
        frame.begin_redraw();
        assert!(!recovery.presented());
        assert_eq!(recovery, RecoveryRedraw::Idle);
    }

    #[test]
    fn restore_waits_for_a_nonzero_size_when_an_expose_arrives_early() {
        let mut restore = SurfaceRestore::default();

        restore.defer();
        assert!(!restore.take_if_drawable(PhysicalSize::new(0, 400)));
        assert!(restore.take_if_drawable(PhysicalSize::new(640, 400)));
    }

    #[test]
    fn ordinary_resize_changes_grid_not_logical_font_metrics() {
        let metrics = CellMetrics::from_physical(10, 20, 16.0);
        let initial =
            terminal_dimensions_for_viewport(PhysicalSize::new(800, 600), metrics).unwrap();
        let wider =
            terminal_dimensions_for_viewport(PhysicalSize::new(1200, 600), metrics).unwrap();

        assert_eq!((initial.columns(), initial.rows()), (80, 30));
        assert_eq!((wider.columns(), wider.rows()), (120, 30));
        assert_eq!(metrics.pixels_per_em(), 16.0);
    }

    #[test]
    fn dpi_metrics_recalculate_the_grid_and_zero_size_defers_it() {
        let one_x = CellMetrics::from_physical(10, 20, 16.0);
        let two_x = CellMetrics::from_physical(20, 40, 32.0);

        assert_eq!(
            terminal_dimensions_for_viewport(PhysicalSize::new(800, 600), one_x)
                .map(|dimensions| (dimensions.columns(), dimensions.rows())),
            Some((80, 30))
        );
        assert_eq!(
            terminal_dimensions_for_viewport(PhysicalSize::new(800, 600), two_x)
                .map(|dimensions| (dimensions.columns(), dimensions.rows())),
            Some((40, 15))
        );
        assert_eq!(
            terminal_dimensions_for_viewport(PhysicalSize::new(0, 600), one_x),
            None
        );
    }

    #[test]
    fn suspension_preserves_damage_but_allows_resume_to_request_a_new_redraw() {
        let mut frame = FrameState::default();

        assert!(frame.invalidate());
        frame.suspend();
        assert!(frame.invalidate());
        frame.begin_redraw();
        assert!(!frame.dirty);
    }

    #[test]
    fn ordinary_dirty_frame_does_not_request_a_recovery_follow_up() {
        let mut frame = FrameState::default();
        let mut recovery = RecoveryRedraw::default();

        assert!(frame.invalidate());
        frame.begin_redraw();
        assert!(!recovery.presented());
        assert!(!frame.redraw_requested);
        assert_eq!(recovery, RecoveryRedraw::Idle);
    }

    #[test]
    fn repeated_idle_iterations_do_not_queue_redraws() {
        let frame = FrameState::default();
        let mut sync = PhysicalSizeSync::default();
        let mut restore = SurfaceRestore::default();
        let recovery = RecoveryRedraw::default();

        for _ in 0..2 {
            assert!(!sync.take());
            assert!(!restore.take_if_drawable(PhysicalSize::new(800, 600)));
        }
        assert!(!frame.dirty);
        assert!(!frame.redraw_requested);
        assert_eq!(recovery, RecoveryRedraw::Idle);
    }

    #[test]
    fn multiple_resize_events_during_recovery_coalesce_to_one_follow_up() {
        let mut frame = FrameState::default();
        let mut recovery = RecoveryRedraw::default();

        recovery.begin();
        frame.rearm();
        assert!(frame.invalidate());
        // Subsequent non-recovery resizes may invalidate changed content, but
        // the already queued redraw stays singular.
        assert!(!frame.invalidate());
        recovery.begin();

        frame.begin_redraw();
        assert!(recovery.presented());
        assert!(frame.invalidate());

        frame.begin_redraw();
        recovery.begin();
        assert!(!recovery.presented());
        assert_eq!(recovery, RecoveryRedraw::Idle);
    }

    #[test]
    fn surface_lost_or_outdated_recovery_rearms_once_then_a_pair_of_presents_completes_recovery() {
        let mut frame = FrameState::default();
        let mut recovery = RecoveryRedraw::default();

        assert!(recovery.reconfigured());
        assert!(frame.invalidate());
        frame.begin_redraw();
        assert!(!recovery.reconfigured());
        assert!(!frame.redraw_requested);

        assert!(recovery.presented());
        assert!(frame.invalidate());
        frame.begin_redraw();
        assert!(!recovery.presented());
        assert_eq!(recovery, RecoveryRedraw::Idle);
    }

    #[test]
    fn skipped_recovery_frames_neither_consume_nor_multiply_the_follow_up() {
        let mut frame = FrameState::default();
        let mut recovery = RecoveryRedraw::default();

        recovery.begin();
        recovery.skipped();
        assert_eq!(
            recovery,
            RecoveryRedraw::AwaitingInitialPresent {
                reconfigure_redraw_issued: false,
            }
        );
        assert!(!frame.redraw_requested);

        assert!(recovery.presented());
        assert!(frame.invalidate());
        frame.begin_redraw();
        recovery.skipped();
        assert_eq!(recovery, RecoveryRedraw::AwaitingFollowUpPresent);
        assert!(!frame.redraw_requested);
        assert!(!recovery.presented());
        assert_eq!(recovery, RecoveryRedraw::Idle);
    }
}
