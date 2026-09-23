//! Native application lifecycle and component wiring.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use terminal_core::{
    CellColor, CursorKey, MAX_COLUMNS, MAX_GRID_CELLS, MAX_ROWS,
    MouseButton as TerminalMouseButton, MouseEvent, MouseModifiers, MouseTracking,
    TerminalDimensions, TerminalParser, TerminalState, UnderlineStyle, encode_cursor_key,
    encode_focus, encode_mouse,
};
use terminal_pty::{
    PortablePtyBackend, PtyBackend, PtyOutput, PtySize, PtySpawnConfig, PtyWorker, PtyWorkerEvent,
};
use terminal_renderer::{
    CellMetrics, RedrawOutcome, Renderer, RendererDiagnosticState, diagnostics_enabled,
    emit_diagnostic,
};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};
use winit::window::{Window, WindowId};

struct Application {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    parser: TerminalParser,
    terminal: TerminalState,
    pty: Option<PtyWorker>,
    pty_wake_proxy: Option<EventLoopProxy<PtyWake>>,
    frame: FrameState,
    dpi_size_sync: PhysicalSizeSync,
    pending_resize: PendingResize,
    surface_restore: SurfaceRestore,
    recovery_redraw: RecoveryRedraw,
    fullscreen: FullscreenTransition,
    last_diagnostic_state: Option<AppDiagnosticState>,
    wheel_remainder: f64,
    mouse_wheel_remainder: f64,
    pointer_position: Option<PhysicalPosition<f64>>,
    pressed_mouse_button: Option<TerminalMouseButton>,
    modifiers: ModifiersState,
}

/// App-local notification that PTY worker output is ready to drain.
#[derive(Clone, Copy, Debug)]
enum PtyWake {
    OutputAvailable,
}

/// The minimal key subset needed to drive a line-oriented local shell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BasicKey {
    Enter,
    Backspace,
    Tab,
    Escape,
}

/// App-owned mapping for viewport commands that never enter the PTY input stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ViewportNavigation {
    PageUp,
    PageDown,
}

/// Windows-only shell-selection sources, retained so the policy is testable
/// without reading the host environment.
#[cfg(any(windows, test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WindowsShellSource {
    PowerShellCore,
    WindowsPowerShell,
    ComSpec,
    Cmd,
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

/// Retains the latest authoritative physical size until winit reaches its
/// event-loop idle boundary. This prevents superseded resize events from
/// repeatedly reconfiguring the surface and resizing terminal-owned storage.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PendingResize {
    latest_size: Option<PhysicalSize<u32>>,
    recovery_required: bool,
    resized_events: usize,
}

impl PendingResize {
    fn record(&mut self, size: PhysicalSize<u32>, recovery_required: bool) {
        self.latest_size = Some(size);
        self.recovery_required |= recovery_required;
        self.resized_events += 1;
    }

    fn take(&mut self) -> Option<ResizeBatch> {
        let size = self.latest_size.take()?;
        Some(ResizeBatch {
            size,
            recovery_required: std::mem::take(&mut self.recovery_required),
            resized_events: std::mem::take(&mut self.resized_events),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResizeBatch {
    size: PhysicalSize<u32>,
    recovery_required: bool,
    resized_events: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ResizeWork {
    surface_configures: usize,
    terminal_grid_resizes: usize,
    pty_resize_commands: usize,
    redraw_requests: usize,
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
#[derive(Clone, Copy, Debug, PartialEq)]
struct AppDiagnosticState {
    window_size: Option<(u32, u32)>,
    scale_factor: Option<f64>,
    cell_metrics: Option<CellMetrics>,
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
            parser: TerminalParser::new(),
            terminal,
            pty: None,
            pty_wake_proxy: None,
            frame: FrameState::default(),
            dpi_size_sync: PhysicalSizeSync::default(),
            pending_resize: PendingResize::default(),
            surface_restore: SurfaceRestore::default(),
            recovery_redraw: RecoveryRedraw::default(),
            fullscreen: FullscreenTransition::default(),
            last_diagnostic_state: None,
            wheel_remainder: 0.0,
            mouse_wheel_remainder: 0.0,
            pointer_position: None,
            pressed_mouse_button: None,
            modifiers: ModifiersState::empty(),
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
    fn with_pty_wake_proxy(pty_wake_proxy: EventLoopProxy<PtyWake>) -> Self {
        Self {
            pty_wake_proxy: Some(pty_wake_proxy),
            ..Self::default()
        }
    }

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
            scale_factor: self.window.as_ref().map(|window| window.scale_factor()),
            cell_metrics: self.renderer.as_ref().map(Renderer::cell_metrics),
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
                let grid_changed = self.resize_terminal_to_viewport(window.inner_size());
                if grid_changed {
                    self.resize_pty_to_terminal();
                }
                self.start_local_shell();
                self.invalidate_frame();
                self.diagnose("renderer-created");
            }
            Err(error) => {
                eprintln!("could not initialize terminal renderer: {error}");
                event_loop.exit();
            }
        }
    }

    fn invalidate_frame(&mut self) -> bool {
        if self.frame.invalidate()
            && let Some(window) = self.window.as_ref()
        {
            window.request_redraw();
            self.diagnose("redraw-requested");
            true
        } else {
            self.diagnose("redraw-coalesced");
            false
        }
    }

    /// Requests the one frame needed after a minimized or occluded surface
    /// becomes drawable again. This deliberately replaces a request that the
    /// compositor may have discarded; it is not a continuous retry loop.
    fn invalidate_after_surface_restore(&mut self) -> bool {
        self.frame.rearm();
        self.invalidate_frame()
    }

    /// Applies the latest authoritative physical size after winit has finished
    /// dispatching the current event batch.
    fn apply_pending_resize(&mut self) {
        let Some(batch) = self.pending_resize.take() else {
            return;
        };
        let work = self.resize_renderer(batch.size, batch.recovery_required);
        emit_diagnostic(format_args!(
            "app event=resize-batch resized_events={} latest_size={}x{} surface_configures={} terminal_grid_resizes={} pty_resize_commands={} redraw_requests={}",
            batch.resized_events,
            batch.size.width,
            batch.size.height,
            work.surface_configures,
            work.terminal_grid_resizes,
            work.pty_resize_commands,
            work.redraw_requests,
        ));
    }

    /// Configures the renderer from the latest winit-authoritative physical
    /// resize. Callers must coalesce superseded sizes through [`PendingResize`].
    fn resize_renderer(&mut self, size: PhysicalSize<u32>, force_redraw: bool) -> ResizeWork {
        let mut work = ResizeWork::default();
        self.diagnose("resize-start");
        let surface_changed = self
            .renderer
            .as_mut()
            .is_some_and(|renderer| renderer.resize(size.width, size.height));
        work.surface_configures = usize::from(surface_changed);
        let grid_changed = self.resize_terminal_to_viewport(size);
        work.terminal_grid_resizes = usize::from(grid_changed);
        if grid_changed {
            work.pty_resize_commands = usize::from(self.resize_pty_to_terminal());
        }
        if force_redraw {
            self.recovery_redraw.begin();
            work.redraw_requests = usize::from(self.invalidate_after_surface_restore());
        } else if surface_changed || grid_changed {
            work.redraw_requests = usize::from(self.invalidate_frame());
        }
        self.diagnose("resize-complete");
        work
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
        emit_diagnostic(format_args!(
            "app event=terminal-grid-resized drawable={}x{} scale_factor={} cell_metrics={metrics:?} terminal_grid=columns:{} rows:{} pty=columns:{} rows:{}",
            size.width,
            size.height,
            self.window
                .as_ref()
                .map_or(1.0, |window| window.scale_factor()),
            dimensions.columns(),
            dimensions.rows(),
            dimensions.columns(),
            dimensions.rows(),
        ));
        true
    }

    fn start_local_shell(&mut self) {
        if self.pty.is_some() {
            return;
        }
        let Some(proxy) = self.pty_wake_proxy.clone() else {
            return;
        };
        let size = pty_size_for_terminal(self.terminal.dimensions());
        let backend = PortablePtyBackend::new();
        let session = match backend.spawn(local_shell_spawn_config(size)) {
            Ok(session) => session,
            Err(error) => {
                eprintln!("could not start local shell: {error}");
                return;
            }
        };
        match PtyWorker::start_with_notifier(session, move || {
            let _ = proxy.send_event(PtyWake::OutputAvailable);
        }) {
            Ok(worker) => self.pty = Some(worker),
            Err(error) => eprintln!("could not start local shell worker: {error}"),
        }
    }

    fn resize_pty_to_terminal(&self) -> bool {
        let Some(pty) = self.pty.as_ref() else {
            return false;
        };
        if let Err(error) = pty.resize(pty_size_for_terminal(self.terminal.dimensions())) {
            eprintln!("could not resize local shell: {error}");
            false
        } else {
            let dimensions = self.terminal.dimensions();
            emit_diagnostic(format_args!(
                "app event=pty-resized columns:{} rows:{}",
                dimensions.columns(),
                dimensions.rows(),
            ));
            true
        }
    }

    fn drain_pty_events(&mut self) {
        while let Some(pty) = self.pty.as_ref() {
            let event = match pty.recv_timeout(Duration::ZERO) {
                Ok(Some(event)) => event,
                Ok(None) => break,
                Err(error) => {
                    eprintln!("could not receive local shell output: {error}");
                    break;
                }
            };
            self.handle_pty_event(event);
        }
    }

    fn handle_pty_event(&mut self, event: PtyWorkerEvent) {
        match event {
            PtyWorkerEvent::Output(PtyOutput::Bytes(bytes)) => {
                let replies = parse_terminal_output(&mut self.parser, &mut self.terminal, &bytes);
                for reply in replies {
                    self.write_to_pty(reply);
                }
                self.invalidate_frame();
            }
            PtyWorkerEvent::Output(PtyOutput::Eof) => {
                emit_diagnostic(format_args!("app pty-output=eof"));
            }
            PtyWorkerEvent::Output(PtyOutput::Exited(status)) => {
                emit_diagnostic(format_args!("app pty-output=exited status={status:?}"));
            }
            PtyWorkerEvent::Error(error) => eprintln!("local shell worker error: {error}"),
        }
    }

    fn write_to_pty(&self, bytes: Vec<u8>) -> bool {
        let Some(pty) = self.pty.as_ref() else {
            return false;
        };
        if let Err(error) = pty.write(bytes) {
            eprintln!("could not write local shell input: {error}");
            false
        } else {
            true
        }
    }

    fn send_mouse_event(&self, event: MouseEvent) {
        let Some((column, row)) = self.pointer_position.and_then(|position| {
            let metrics = self.renderer.as_ref()?.cell_metrics();
            terminal_cell_at(position, metrics, self.terminal.dimensions())
        }) else {
            return;
        };
        let modifiers = MouseModifiers {
            shift: self.modifiers.shift_key(),
            alt: self.modifiers.alt_key(),
            control: self.modifiers.control_key(),
        };
        if let Some(bytes) =
            encode_mouse(*self.terminal.input_modes(), event, column, row, modifiers)
        {
            self.write_to_pty(bytes);
        }
    }

    /// Queues the current authoritative window size for the same bounded batch
    /// as ordinary resize events. This also covers expose/occlusion recovery
    /// that has no accompanying `Resized` event.
    fn queue_window_size(&mut self) {
        let size = self.window.as_ref().map(|window| window.inner_size());
        if let Some(size) = size {
            let recovery_required = self.surface_restore.take_if_drawable(size);
            self.pending_resize.record(size, recovery_required);
        }
    }

    fn queue_resize_event(&mut self, size: PhysicalSize<u32>, fullscreen_changed: bool) {
        let recovery_required = self.surface_restore.take_if_drawable(size) || fullscreen_changed;
        self.pending_resize.record(size, recovery_required);
    }
}

impl ApplicationHandler<PtyWake> for Application {
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
        self.pending_resize = PendingResize::default();
        self.surface_restore = SurfaceRestore::default();
        self.recovery_redraw = RecoveryRedraw::default();
        self.fullscreen = FullscreenTransition::default();
        self.diagnose("suspended-complete");
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if self.dpi_size_sync.take() {
            self.queue_window_size();
        }
        if self.surface_restore.pending
            && self.window.as_ref().is_some_and(|window| {
                let size = window.inner_size();
                size.width > 0 && size.height > 0
            })
        {
            self.queue_window_size();
        }
        self.apply_pending_resize();
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
                self.queue_resize_event(size, fullscreen_changed);
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
                if let Some(renderer) = self.renderer.as_mut() {
                    let _ = renderer.set_scale_factor(scale_factor);
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
            WindowEvent::Focused(focused) => {
                if let Some(bytes) = encode_focus(*self.terminal.input_modes(), focused) {
                    self.write_to_pty(bytes.to_vec());
                }
                if !focused {
                    self.pressed_mouse_button = None;
                    self.pointer_position = None;
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::CursorLeft { .. } => self.pointer_position = None,
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer_position = Some(position);
                self.send_mouse_event(MouseEvent::Move(self.pressed_mouse_button));
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(button) = terminal_mouse_button(button) {
                    if state == ElementState::Pressed {
                        self.pressed_mouse_button = Some(button);
                        self.send_mouse_event(MouseEvent::Press(button));
                    } else {
                        self.send_mouse_event(MouseEvent::Release(button));
                        if self.pressed_mouse_button == Some(button) {
                            self.pressed_mouse_button = None;
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(metrics) = self.renderer.as_ref().map(Renderer::cell_metrics) {
                    if self.terminal.input_modes().mouse_tracking() == MouseTracking::Off {
                        self.mouse_wheel_remainder = 0.0;
                        if scroll_terminal_for_wheel(
                            &mut self.terminal,
                            delta,
                            metrics.height(),
                            &mut self.wheel_remainder,
                        ) {
                            self.invalidate_frame();
                        }
                    } else {
                        self.wheel_remainder = 0.0;
                        let rows = wheel_scroll_rows(
                            delta,
                            metrics.height(),
                            &mut self.mouse_wheel_remainder,
                        );
                        let event = if rows > 0 {
                            MouseEvent::WheelUp
                        } else {
                            MouseEvent::WheelDown
                        };
                        for _ in 0..rows.unsigned_abs().min(100) {
                            self.send_mouse_event(event);
                        }
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if let Some(navigation) = viewport_navigation_from_logical_key(&event.logical_key) {
                    let changed = match navigation {
                        ViewportNavigation::PageUp => self.terminal.page_up(),
                        ViewportNavigation::PageDown => self.terminal.page_down(),
                    };
                    if changed {
                        self.invalidate_frame();
                    }
                } else if let Some(cursor_key) = cursor_key_from_logical_key(&event.logical_key) {
                    self.write_to_pty(
                        encode_cursor_key(*self.terminal.input_modes(), cursor_key).to_vec(),
                    );
                } else {
                    let key = basic_key_from_logical_key(&event.logical_key);
                    if let Some(bytes) = basic_key_input(event.text.as_deref(), key) {
                        let is_backspace = key == Some(BasicKey::Backspace)
                            || event.physical_key == PhysicalKey::Code(KeyCode::Backspace);
                        let pty_write_queued = self.write_to_pty(bytes.clone());
                        if is_backspace {
                            emit_diagnostic(format_args!(
                                "app event=backspace-input state={:?} repeat={} logical={:?} physical={:?} committed_text={:?} committed_bytes={:?} selected_bytes={bytes:?} pty_write_requests=1 pty_write_queued={pty_write_queued}",
                                event.state,
                                event.repeat,
                                event.logical_key,
                                event.physical_key,
                                event.text,
                                event.text.as_deref().map(str::as_bytes),
                            ));
                        }
                    }
                }
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

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: PtyWake) {
        match event {
            PtyWake::OutputAvailable => self.drain_pty_events(),
        }
    }
}

fn pty_size_for_terminal(dimensions: TerminalDimensions) -> PtySize {
    PtySize::new(
        u16::try_from(dimensions.rows()).expect("terminal row limit fits PTY size"),
        u16::try_from(dimensions.columns()).expect("terminal column limit fits PTY size"),
    )
    .expect("terminal dimensions are nonzero")
}

fn local_shell_spawn_config(size: PtySize) -> PtySpawnConfig {
    #[cfg(windows)]
    let (program, source) = windows_shell_program();
    #[cfg(windows)]
    emit_diagnostic(format_args!(
        "app event=local-shell-selected source={source:?} program={}",
        program.display()
    ));
    #[cfg(not(windows))]
    let program = std::env::var_os("SHELL")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/bin/sh"));

    PtySpawnConfig::new(program, size)
}

fn parse_terminal_output(
    parser: &mut TerminalParser,
    terminal: &mut TerminalState,
    bytes: &[u8],
) -> Vec<Vec<u8>> {
    let mut remaining = bytes;
    while !remaining.is_empty() {
        match parser.advance(terminal, remaining) {
            Ok(()) => break,
            Err(error) => {
                eprintln!("could not apply local shell output: {error}");
                let consumed = error.bytes_consumed();
                if consumed == 0 {
                    break;
                }
                remaining = &remaining[consumed..];
            }
        }
    }
    let mut replies = Vec::new();
    while let Some(reply) = terminal.take_reply() {
        replies.push(reply.as_bytes().as_slice().to_vec());
    }
    replies
}

fn basic_key_from_logical_key(key: &Key) -> Option<BasicKey> {
    match key {
        Key::Named(NamedKey::Enter) => Some(BasicKey::Enter),
        Key::Named(NamedKey::Backspace) => Some(BasicKey::Backspace),
        Key::Named(NamedKey::Tab) => Some(BasicKey::Tab),
        Key::Named(NamedKey::Escape) => Some(BasicKey::Escape),
        _ => None,
    }
}

fn cursor_key_from_logical_key(key: &Key) -> Option<CursorKey> {
    match key {
        Key::Named(NamedKey::ArrowUp) => Some(CursorKey::Up),
        Key::Named(NamedKey::ArrowDown) => Some(CursorKey::Down),
        Key::Named(NamedKey::ArrowRight) => Some(CursorKey::Right),
        Key::Named(NamedKey::ArrowLeft) => Some(CursorKey::Left),
        _ => None,
    }
}

fn terminal_mouse_button(button: MouseButton) -> Option<TerminalMouseButton> {
    match button {
        MouseButton::Left => Some(TerminalMouseButton::Left),
        MouseButton::Middle => Some(TerminalMouseButton::Middle),
        MouseButton::Right => Some(TerminalMouseButton::Right),
        _ => None,
    }
}

fn terminal_cell_at(
    position: PhysicalPosition<f64>,
    metrics: CellMetrics,
    dimensions: TerminalDimensions,
) -> Option<(usize, usize)> {
    if !position.x.is_finite() || !position.y.is_finite() || position.x < 0.0 || position.y < 0.0 {
        return None;
    }
    let column = (position.x / f64::from(metrics.width())) as usize;
    let row = (position.y / f64::from(metrics.height())) as usize;
    (column < dimensions.columns() && row < dimensions.rows()).then_some((column, row))
}

fn viewport_navigation_from_logical_key(key: &Key) -> Option<ViewportNavigation> {
    match key {
        Key::Named(NamedKey::PageUp) => Some(ViewportNavigation::PageUp),
        Key::Named(NamedKey::PageDown) => Some(ViewportNavigation::PageDown),
        _ => None,
    }
}

/// Routes wheel motion through the primary-screen viewport only.
fn scroll_terminal_for_wheel(
    terminal: &mut TerminalState,
    delta: MouseScrollDelta,
    cell_height: u32,
    remainder: &mut f64,
) -> bool {
    if terminal.active_screen() != terminal_core::ScreenKind::Primary
        || terminal.scrollback_len() == 0
    {
        *remainder = 0.0;
        return false;
    }
    terminal.scroll_viewport_rows(wheel_scroll_rows(delta, cell_height, remainder))
}

/// Converts platform line/pixel wheel deltas to whole terminal rows, retaining
/// sub-row motion so precision wheels and touchpads can eventually move a row.
fn wheel_scroll_rows(delta: MouseScrollDelta, cell_height: u32, remainder: &mut f64) -> i32 {
    let rows = match delta {
        MouseScrollDelta::LineDelta(_, y) => f64::from(y) * 3.0,
        MouseScrollDelta::PixelDelta(position) => position.y / f64::from(cell_height.max(1)),
    };
    if !rows.is_finite() {
        return 0;
    }
    let total = (*remainder + rows).clamp(f64::from(i32::MIN), f64::from(i32::MAX));
    let whole = total.trunc() as i32;
    *remainder = total - f64::from(whole);
    whole
}

fn basic_key_input(text: Option<&str>, key: Option<BasicKey>) -> Option<Vec<u8>> {
    let named_key = match key {
        Some(BasicKey::Enter) => Some(vec![b'\r']),
        Some(BasicKey::Backspace) => Some(vec![basic_backspace_byte_for_platform(cfg!(windows))]),
        Some(BasicKey::Tab) => Some(vec![b'\t']),
        Some(BasicKey::Escape) => Some(vec![0x1b]),
        None => None,
    };
    named_key.or_else(|| {
        text.filter(|text| !text.is_empty())
            .map(|text| text.as_bytes().to_vec())
    })
}

fn basic_backspace_byte_for_platform(_windows: bool) -> u8 {
    0x7f
}

#[cfg(any(windows, test))]
fn select_windows_shell(
    pwsh: Option<PathBuf>,
    windows_powershell: Option<PathBuf>,
    comspec: Option<PathBuf>,
) -> (PathBuf, WindowsShellSource) {
    if let Some(program) = pwsh {
        (program, WindowsShellSource::PowerShellCore)
    } else if let Some(program) = windows_powershell {
        (program, WindowsShellSource::WindowsPowerShell)
    } else if let Some(program) = comspec {
        (program, WindowsShellSource::ComSpec)
    } else {
        (PathBuf::from("cmd.exe"), WindowsShellSource::Cmd)
    }
}

#[cfg(windows)]
fn windows_shell_program() -> (PathBuf, WindowsShellSource) {
    let pwsh = executable_on_path("pwsh.exe");
    let windows_powershell = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .map(|root| {
            root.join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe")
        })
        .filter(|program| program.is_file())
        .or_else(|| executable_on_path("powershell.exe"));
    let comspec = std::env::var_os("COMSPEC")
        .map(PathBuf::from)
        .filter(|program| program.is_file())
        .or_else(|| {
            std::env::var_os("SystemRoot")
                .map(PathBuf::from)
                .map(|root| root.join("System32").join("cmd.exe"))
                .filter(|program| program.is_file())
        });
    select_windows_shell(pwsh, windows_powershell, comspec)
}

#[cfg(windows)]
fn executable_on_path(executable: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(executable))
        .find(|program| program.is_file())
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
    let event_loop = EventLoop::<PtyWake>::with_user_event()
        .build()
        .expect("could not create terminal event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let pty_wake_proxy = event_loop.create_proxy();
    event_loop
        .run_app(&mut Application::with_pty_wake_proxy(pty_wake_proxy))
        .expect("terminal event loop failed");
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        BasicKey, FrameState, PendingResize, PhysicalSizeSync, RecoveryRedraw, SurfaceRestore,
        ViewportNavigation, WindowsShellSource, basic_backspace_byte_for_platform, basic_key_input,
        cursor_key_from_logical_key, parse_terminal_output, pty_size_for_terminal,
        scroll_terminal_for_wheel, select_windows_shell, terminal_cell_at,
        terminal_dimensions_for_viewport, viewport_navigation_from_logical_key, wheel_scroll_rows,
    };
    use terminal_core::{CursorKey, TerminalDimensions, TerminalParser, TerminalState};
    use terminal_renderer::CellMetrics;
    use winit::dpi::{PhysicalPosition, PhysicalSize};
    use winit::event::MouseScrollDelta;
    use winit::keyboard::{Key, NamedKey};

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
    fn pty_output_flows_through_the_parser_into_terminal_state() {
        let mut parser = TerminalParser::new();
        let mut terminal = TerminalState::new(TerminalDimensions::new(8, 2).unwrap());

        let replies = parse_terminal_output(&mut parser, &mut terminal, b"echo ok\r\nok");

        assert!(replies.is_empty());
        assert_eq!(terminal.screen().cell(0, 0).unwrap().character(), 'e');
        assert_eq!(terminal.screen().cell(1, 0).unwrap().character(), 'o');
        assert_eq!(terminal.screen().cell(1, 1).unwrap().character(), 'k');
    }

    #[test]
    fn terminal_replies_are_returned_to_the_pty_transport() {
        let mut parser = TerminalParser::new();
        let mut terminal = TerminalState::new(TerminalDimensions::new(8, 2).unwrap());

        let replies = parse_terminal_output(&mut parser, &mut terminal, b"\x1b[5n");

        assert_eq!(replies, vec![b"\x1b[0n".to_vec()]);
    }

    #[test]
    fn terminal_grid_dimensions_preserve_their_row_column_order_for_the_pty() {
        let dimensions = TerminalDimensions::new(132, 43).unwrap();

        assert_eq!(pty_size_for_terminal(dimensions).rows(), 43);
        assert_eq!(pty_size_for_terminal(dimensions).columns(), 132);
    }

    #[test]
    fn basic_keyboard_text_and_line_editing_keys_encode_for_the_pty() {
        assert_eq!(
            basic_key_input(Some("echo ok"), None),
            Some(b"echo ok".to_vec())
        );
        assert_eq!(
            basic_key_input(None, Some(BasicKey::Enter)),
            Some(vec![b'\r'])
        );
        assert_eq!(
            basic_key_input(None, Some(BasicKey::Backspace)),
            Some(vec![basic_backspace_byte_for_platform(cfg!(windows))])
        );
        assert_eq!(
            basic_key_input(None, Some(BasicKey::Tab)),
            Some(vec![b'\t'])
        );
        assert_eq!(
            basic_key_input(None, Some(BasicKey::Escape)),
            Some(vec![0x1b])
        );
        assert_eq!(basic_key_input(None, None), None);
    }

    #[test]
    fn named_backspace_takes_precedence_over_committed_control_text() {
        assert_eq!(
            basic_key_input(Some("\u{17}"), Some(BasicKey::Backspace)),
            Some(vec![basic_backspace_byte_for_platform(cfg!(windows))])
        );
    }

    #[test]
    fn page_navigation_is_app_local_and_not_basic_pty_input() {
        assert_eq!(
            viewport_navigation_from_logical_key(&Key::Named(NamedKey::PageUp)),
            Some(ViewportNavigation::PageUp)
        );
        assert_eq!(
            viewport_navigation_from_logical_key(&Key::Named(NamedKey::PageDown)),
            Some(ViewportNavigation::PageDown)
        );
        assert_eq!(basic_key_input(None, None), None);
    }

    #[test]
    fn arrow_keys_map_to_core_cursor_keys() {
        assert_eq!(
            cursor_key_from_logical_key(&Key::Named(NamedKey::ArrowUp)),
            Some(CursorKey::Up)
        );
        assert_eq!(
            cursor_key_from_logical_key(&Key::Named(NamedKey::ArrowDown)),
            Some(CursorKey::Down)
        );
        assert_eq!(
            cursor_key_from_logical_key(&Key::Named(NamedKey::ArrowRight)),
            Some(CursorKey::Right)
        );
        assert_eq!(
            cursor_key_from_logical_key(&Key::Named(NamedKey::ArrowLeft)),
            Some(CursorKey::Left)
        );
    }

    #[test]
    fn pointer_position_maps_only_inside_terminal_cells() {
        let dimensions = TerminalDimensions::new(2, 2).unwrap();
        let metrics = CellMetrics::from_physical(10, 20, 12.0);
        assert_eq!(
            terminal_cell_at(PhysicalPosition::new(19.0, 39.0), metrics, dimensions),
            Some((1, 1))
        );
        assert_eq!(
            terminal_cell_at(PhysicalPosition::new(20.0, 10.0), metrics, dimensions),
            None
        );
        assert_eq!(
            terminal_cell_at(PhysicalPosition::new(-1.0, 10.0), metrics, dimensions),
            None
        );
    }

    #[test]
    fn wheel_deltas_accumulate_pixel_motion_and_preserve_direction() {
        let mut remainder = 0.0;
        assert_eq!(
            wheel_scroll_rows(MouseScrollDelta::LineDelta(0.0, 1.0), 20, &mut remainder),
            3
        );
        assert_eq!(
            wheel_scroll_rows(
                MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, 8.0)),
                20,
                &mut remainder
            ),
            0
        );
        assert_eq!(
            wheel_scroll_rows(
                MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, 14.0)),
                20,
                &mut remainder
            ),
            1
        );
        assert!((remainder - 0.1).abs() < 1e-10);
        assert_eq!(
            wheel_scroll_rows(MouseScrollDelta::LineDelta(0.0, -1.0), 20, &mut remainder),
            -2
        );
        assert_eq!(
            wheel_scroll_rows(
                MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, f64::NAN)),
                20,
                &mut remainder
            ),
            0
        );
    }

    #[test]
    fn wheel_scrolls_primary_history_but_not_alternate_screen() {
        let mut terminal = TerminalState::new(TerminalDimensions::new(2, 2).unwrap());
        terminal.set_cursor_position(1, 0).unwrap();
        terminal.index();
        terminal.index();
        let mut remainder = 0.5;

        assert!(scroll_terminal_for_wheel(
            &mut terminal,
            MouseScrollDelta::LineDelta(0.0, 1.0),
            20,
            &mut remainder,
        ));
        assert_eq!(terminal.viewport_offset(), 2);
        assert!(!scroll_terminal_for_wheel(
            &mut terminal,
            MouseScrollDelta::LineDelta(0.0, 1.0),
            20,
            &mut remainder,
        ));

        terminal.switch_to_alternate_screen();
        assert!(!scroll_terminal_for_wheel(
            &mut terminal,
            MouseScrollDelta::LineDelta(0.0, -1.0),
            20,
            &mut remainder,
        ));
        assert_eq!(remainder, 0.0);
        terminal.switch_to_primary_screen();
        assert_eq!(terminal.viewport_offset(), 2);

        assert!(scroll_terminal_for_wheel(
            &mut terminal,
            MouseScrollDelta::LineDelta(0.0, -1.0),
            20,
            &mut remainder,
        ));
        assert_eq!(terminal.viewport_offset(), 0);
    }

    #[test]
    fn basic_backspace_policy_uses_del_for_windows_conpty_and_elsewhere() {
        assert_eq!(basic_backspace_byte_for_platform(true), 0x7f);
        assert_eq!(basic_backspace_byte_for_platform(false), 0x7f);
    }

    #[test]
    fn multiple_resize_events_coalesce_to_the_latest_authoritative_size() {
        let mut pending = PendingResize::default();

        pending.record(PhysicalSize::new(800, 600), false);
        pending.record(PhysicalSize::new(801, 600), false);
        pending.record(PhysicalSize::new(802, 601), false);

        assert_eq!(
            pending.take(),
            Some(super::ResizeBatch {
                size: PhysicalSize::new(802, 601),
                recovery_required: false,
                resized_events: 3,
            })
        );
        assert_eq!(pending.take(), None);
    }

    #[test]
    fn coalesced_resize_preserves_latest_terminal_and_pty_dimensions() {
        let mut pending = PendingResize::default();
        let metrics = CellMetrics::from_physical(10, 20, 16.0);

        pending.record(PhysicalSize::new(800, 600), false);
        pending.record(PhysicalSize::new(1_203, 619), false);

        let batch = pending.take().expect("latest resize is retained");
        let dimensions = terminal_dimensions_for_viewport(batch.size, metrics)
            .expect("nonzero viewport and metrics produce terminal dimensions");

        assert_eq!(batch.resized_events, 2);
        assert_eq!(batch.size, PhysicalSize::new(1_203, 619));
        assert_eq!(dimensions.columns(), 120);
        assert_eq!(dimensions.rows(), 30);
        assert_eq!(pty_size_for_terminal(dimensions).columns(), 120);
        assert_eq!(pty_size_for_terminal(dimensions).rows(), 30);
    }

    #[test]
    fn one_resize_event_is_applied_without_coalescing_away_its_size() {
        let mut pending = PendingResize::default();
        let size = PhysicalSize::new(801, 601);

        pending.record(size, false);

        assert_eq!(
            pending.take(),
            Some(super::ResizeBatch {
                size,
                recovery_required: false,
                resized_events: 1,
            })
        );
    }

    #[test]
    fn restore_recovery_survives_coalescing_to_a_later_nonzero_size() {
        let mut restore = SurfaceRestore::default();
        let mut pending = PendingResize::default();

        restore.defer();
        pending.record(PhysicalSize::new(0, 0), false);
        let restored = PhysicalSize::new(800, 600);
        pending.record(restored, restore.take_if_drawable(restored));
        pending.record(PhysicalSize::new(810, 610), false);

        assert_eq!(
            pending.take(),
            Some(super::ResizeBatch {
                size: PhysicalSize::new(810, 610),
                recovery_required: true,
                resized_events: 3,
            })
        );
    }

    #[test]
    fn windows_shell_selection_prefers_pwsh_then_windows_powershell_then_cmd() {
        let pwsh = PathBuf::from("pwsh.exe");
        let powershell = PathBuf::from("powershell.exe");
        let comspec = PathBuf::from("cmd-from-comspec.exe");

        assert_eq!(
            select_windows_shell(
                Some(pwsh.clone()),
                Some(powershell.clone()),
                Some(comspec.clone())
            ),
            (pwsh, WindowsShellSource::PowerShellCore)
        );
        assert_eq!(
            select_windows_shell(None, Some(powershell.clone()), Some(comspec.clone())),
            (powershell, WindowsShellSource::WindowsPowerShell)
        );
        assert_eq!(
            select_windows_shell(None, None, Some(comspec.clone())),
            (comspec, WindowsShellSource::ComSpec)
        );
        assert_eq!(
            select_windows_shell(None, None, None),
            (PathBuf::from("cmd.exe"), WindowsShellSource::Cmd)
        );
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
