#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

//! Native application lifecycle and component wiring.

mod commands;
mod search;

use commands::{Palette, PaletteAction, PaletteEntry};
use search::Search;
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use terminal_config::{Command, Config, FontConfig, Rgb};

use terminal_core::{
    CellColor, CursorKey, InputModes, MAX_COLUMNS, MAX_GRID_CELLS, MAX_ROWS,
    MouseButton as TerminalMouseButton, MouseEvent, MouseModifiers, MouseTracking, Osc52Policy,
    ScreenKind, TerminalDimensions, TerminalParser, TerminalState, UnderlineStyle,
    encode_control_cursor_key, encode_cursor_key, encode_focus, encode_mouse, encode_paste,
};
use terminal_pty::{
    PortablePtyBackend, PtyBackend, PtyOutput, PtySize, PtySpawnConfig, PtyWorker, PtyWorkerEvent,
};
use terminal_renderer::{
    CellMetrics, FontRequest, OverlayLine, PaneRenderInput, RedrawOutcome, RenderTheme, Renderer,
    RendererDiagnosticState, Rgba, ScrollbarGeometry, ScrollbarHit, ScrollbarRenderData,
    SearchHighlight, SearchMarker, SurfaceSize, TextOverlay, diagnostics_enabled, emit_diagnostic,
};
use terminal_workspace::{
    PaneDirection, PaneId, PaneRect, SessionDefinition, SplitAxis, Tab, TabId, Workspace,
    WorkspaceDefinition,
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
    workspace: Workspace,
    active_runtime_pane: PaneId,
    inactive_panes: HashMap<PaneId, PaneRuntime>,
    workspace_loaded: bool,
    pty_wake_proxy: Option<EventLoopProxy<PtyWake>>,
    pty_wake_pending: Arc<AtomicBool>,
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
    selection_dragging: bool,
    scrollbar_drag: Option<f64>,
    scrollbar_click_held: bool,
    scrollbar_hover: Option<ScrollbarHit>,
    selection_click: Option<(Instant, usize, usize, u8)>,
    selection_edge: Option<(i32, usize, usize, Instant)>,
    target_click_held: bool,
    pending_target: Option<String>,
    modifiers: ModifiersState,
    config: Config,
    base_font_size: u16,
    font_size: u16,
    config_path: PathBuf,
    project_root_override: Option<PathBuf>,
    palette: Option<Palette>,
    tab_picker: Option<Palette>,
    unseen_tab_activity: HashSet<TabId>,
    activity_frame: usize,
    activity_deadline: Option<Instant>,
    search: Option<Search>,
    tab_rename: Option<String>,
}

fn selection_click_count(
    previous: Option<(Instant, usize, usize, u8)>,
    now: Instant,
    row: usize,
    column: usize,
) -> u8 {
    match previous {
        Some((when, previous_row, previous_column, count))
            if now.duration_since(when) <= Duration::from_millis(500)
                && row == previous_row
                && column == previous_column =>
        {
            (count % 3) + 1
        }
        _ => 1,
    }
}

fn scrollbar_page_delta(hit: ScrollbarHit, visible_rows: usize) -> i32 {
    let page = visible_rows.min(i32::MAX as usize) as i32;
    match hit {
        ScrollbarHit::TrackAbove => page,
        ScrollbarHit::TrackBelow => -page,
        ScrollbarHit::Thumb => 0,
    }
}

fn selection_edge_direction(row: usize, rows: usize) -> Option<i32> {
    if row == 0 {
        Some(1)
    } else if row + 1 == rows {
        Some(-1)
    } else {
        None
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CliOptions {
    project_root: Option<PathBuf>,
    workspace: Option<PathBuf>,
}

impl CliOptions {
    fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Option<Self>, String> {
        let mut options = Self::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.to_str() {
                Some("--help" | "-h") => return Ok(None),
                Some("--project-root") if options.project_root.is_none() => {
                    let path =
                        PathBuf::from(args.next().ok_or("--project-root needs a directory")?);
                    if !path.is_dir() {
                        return Err(format!(
                            "project root is not a directory: {}",
                            path.display()
                        ));
                    }
                    options.project_root =
                        Some(path.canonicalize().map_err(|error| error.to_string())?);
                }
                Some("--workspace") if options.workspace.is_none() => {
                    let path = PathBuf::from(args.next().ok_or("--workspace needs a TOML file")?);
                    Config::load_workspace_file(&path)
                        .map_err(|error| format!("workspace file: {error}"))?;
                    options.workspace =
                        Some(path.canonicalize().map_err(|error| error.to_string())?);
                }
                _ => {
                    return Err(format!(
                        "unknown or repeated argument: {}",
                        arg.to_string_lossy()
                    ));
                }
            }
        }
        Ok(Some(options))
    }
}

/// App runtime for one workspace-owned pane and its associated session.
struct PaneRuntime {
    parser: TerminalParser,
    terminal: TerminalState,
    pty: Option<PtyWorker>,
}

impl PaneRuntime {
    fn new(dimensions: TerminalDimensions) -> Self {
        Self {
            parser: TerminalParser::with_osc52_policy(Osc52Policy::Deny),
            terminal: TerminalState::new(dimensions),
            pty: None,
        }
    }
}

/// App-local notifications delivered through the winit event loop.
#[derive(Clone, Copy, Debug)]
enum PtyWake {
    OutputAvailable,
    ConfigChanged,
}

/// The minimal key subset needed to drive a line-oriented local shell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BasicKey {
    Enter,
    Backspace,
    Tab,
    Escape,
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
    #[cfg(test)]
    requests_armed: usize,
}

impl FrameState {
    /// Marks the frame dirty and returns whether winit needs one redraw request.
    fn invalidate(&mut self) -> bool {
        self.dirty = true;
        if self.redraw_requested {
            false
        } else {
            self.redraw_requested = true;
            #[cfg(test)]
            {
                self.requests_armed += 1;
            }
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

fn short_cwd(uri: &str) -> Option<String> {
    if !uri.get(..7)?.eq_ignore_ascii_case("file://") {
        return None;
    }
    let (_, path) = uri[7..].split_once('/')?;
    let mut decoded = Vec::with_capacity(path.len());
    let mut bytes = path.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = (bytes.next()? as char).to_digit(16)?;
            let low = (bytes.next()? as char).to_digit(16)?;
            decoded.push((high * 16 + low) as u8);
        } else {
            decoded.push(byte);
        }
    }
    let path = String::from_utf8(decoded).ok()?;
    let segments: Vec<_> = path
        .split(['/', '\\'])
        .filter(|segment| !segment.is_empty() && *segment != "." && *segment != "..")
        .collect();
    (!segments.is_empty()).then(|| segments[segments.len().saturating_sub(2)..].join("/"))
}

fn display_application(name: &str) -> Option<String> {
    let name = name.trim().rsplit(['/', '\\']).next()?.trim();
    let name = name
        .get(..name.len().saturating_sub(4))
        .filter(|_| name.to_ascii_lowercase().ends_with(".exe"))
        .unwrap_or(name);
    if name.is_empty() {
        return None;
    }
    Some(
        if name.eq_ignore_ascii_case("pwsh") || name.eq_ignore_ascii_case("powershell") {
            "PowerShell".into()
        } else {
            name.into()
        },
    )
}

fn tab_picker_label(tab: &Tab, terminal: Option<&TerminalState>, index: usize) -> String {
    if let Some(title) = &tab.custom_title {
        return title.clone();
    }
    let cwd = terminal
        .and_then(TerminalState::working_directory_uri)
        .and_then(short_cwd);
    let startup_program = tab.panes().into_iter().find_map(|pane| {
        (pane.id == tab.active_pane)
            .then_some(pane.startup)
            .and_then(|startup| {
                if let SessionDefinition::Command { program, .. } = startup {
                    program.to_str().map(str::to_owned)
                } else {
                    None
                }
            })
    });
    let app = terminal
        .and_then(TerminalState::shell_title)
        .and_then(display_application)
        .or_else(|| startup_program.as_deref().and_then(display_application));
    match (cwd, app) {
        (Some(cwd), Some(app)) => format!("{cwd}  {app}"),
        (Some(cwd), None) => cwd,
        (None, Some(app)) => app,
        (None, None) => format!("Terminal {}", index + 1),
    }
}

impl Default for Application {
    fn default() -> Self {
        let workspace = Workspace::default();
        let active_runtime_pane = workspace.active_pane();
        let mut terminal =
            TerminalState::new(TerminalDimensions::new(80, 24).expect("valid default"));
        if std::env::var_os("TERMINAL_RENDERER_VISUAL_SMOKE").is_some_and(|value| value == "1") {
            populate_visual_smoke_state(&mut terminal);
        }
        Self {
            window: None,
            renderer: None,
            parser: TerminalParser::with_osc52_policy(Osc52Policy::Deny),
            terminal,
            pty: None,
            workspace,
            active_runtime_pane,
            inactive_panes: HashMap::new(),
            workspace_loaded: false,
            pty_wake_proxy: None,
            pty_wake_pending: Arc::new(AtomicBool::new(false)),
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
            selection_dragging: false,
            scrollbar_drag: None,
            scrollbar_click_held: false,
            scrollbar_hover: None,
            selection_click: None,
            selection_edge: None,
            target_click_held: false,
            pending_target: None,
            modifiers: ModifiersState::empty(),
            config: Config::default(),
            base_font_size: Config::default().font.size,
            font_size: Config::default().font.size,
            config_path: config_path(),
            project_root_override: None,
            palette: None,
            tab_picker: None,
            unseen_tab_activity: HashSet::new(),
            activity_frame: 0,
            activity_deadline: None,
            search: None,
            tab_rename: None,
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

fn queue_pty_write(pty: Option<&PtyWorker>, bytes: Vec<u8>) -> bool {
    let Some(pty) = pty else {
        return false;
    };
    if let Err(error) = pty.write(bytes) {
        eprintln!("could not write local shell input: {error}");
        false
    } else {
        true
    }
}

fn queue_terminal_input(
    terminal: &mut TerminalState,
    bytes: Vec<u8>,
    write: impl FnOnce(Vec<u8>) -> bool,
) -> (bool, bool) {
    if bytes.is_empty() || !write(bytes) {
        return (false, false);
    }
    (true, terminal.return_to_live_viewport())
}

impl Application {
    fn with_pty_wake_proxy(pty_wake_proxy: EventLoopProxy<PtyWake>, cli: CliOptions) -> Self {
        let mut app = Self {
            pty_wake_proxy: Some(pty_wake_proxy.clone()),
            config_path: cli.workspace.unwrap_or_else(config_path),
            project_root_override: cli.project_root,
            ..Self::default()
        };
        app.reload_config();
        let path = app.config_path.clone();
        std::thread::spawn(move || {
            let mut previous = config_fingerprint(&path);
            loop {
                std::thread::sleep(Duration::from_millis(500));
                let current = config_fingerprint(&path);
                if current != previous {
                    previous = current;
                    if pty_wake_proxy.send_event(PtyWake::ConfigChanged).is_err() {
                        break;
                    }
                }
            }
        });
        app
    }

    fn reload_config(&mut self) {
        let next = if self.config_path.exists() {
            Config::load(&self.config_path)
        } else {
            Ok(Config::default())
        };
        match next {
            Ok(mut config) => {
                if let Some(root) = &self.project_root_override {
                    config.workspace.project_root = Some(root.clone());
                }
                if !self.workspace_loaded {
                    self.load_workspace(&config.workspace);
                    self.workspace_loaded = true;
                }
                if config.theme != self.config.theme {
                    if let Some(renderer) = self.renderer.as_mut() {
                        renderer.set_theme(render_theme(&config.theme));
                    }
                    self.invalidate_frame();
                }
                if self.renderer.is_none() {
                    self.base_font_size = config.font.size;
                    self.font_size = config.font.size;
                }
                self.config = config;
            }
            Err(error) => eprintln!("config reload rejected; keeping last valid config: {error}"),
        }
    }

    fn load_workspace(&mut self, definition: &WorkspaceDefinition) {
        let workspace = Workspace::from_definition(definition).expect("validated workspace config");
        let active = workspace.active_pane();
        let dimensions = self.terminal.dimensions();
        self.inactive_panes = workspace
            .panes()
            .into_iter()
            .filter(|pane| pane.id != active)
            .map(|pane| (pane.id, PaneRuntime::new(dimensions)))
            .collect();
        self.workspace = workspace;
        self.active_runtime_pane = active;
        self.update_workspace_title();
    }

    fn update_workspace_title(&self) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let tab = self.workspace.active_tab();
        let panes = tab.panes();
        if self.workspace.tabs().len() == 1
            && panes.len() == 1
            && tab.custom_title.is_none()
            && self.terminal.shell_title().is_none()
        {
            window.set_title("Terminal");
        } else {
            let pane_index = panes
                .iter()
                .position(|pane| pane.id == tab.active_pane)
                .unwrap()
                + 1;
            window.set_title(&format!(
                "Terminal — {} ({}/{}) — Pane {}/{}",
                self.active_tab_title(),
                self.workspace.active_tab_index() + 1,
                self.workspace.tabs().len(),
                pane_index,
                panes.len(),
            ));
        }
    }

    fn active_tab_title(&self) -> &str {
        self.workspace
            .active_tab()
            .display_title_with_shell_title(self.terminal.shell_title())
    }

    fn activate_pane(&mut self, next: PaneId) {
        self.unseen_tab_activity
            .remove(&self.workspace.active_tab().id);
        let current = self.active_runtime_pane;
        if current == next {
            return;
        }
        self.search = None;
        let next_runtime = self
            .inactive_panes
            .remove(&next)
            .expect("workspace pane has runtime");
        let old = PaneRuntime {
            parser: std::mem::replace(&mut self.parser, next_runtime.parser),
            terminal: std::mem::replace(&mut self.terminal, next_runtime.terminal),
            pty: std::mem::replace(&mut self.pty, next_runtime.pty),
        };
        self.inactive_panes.insert(current, old);
        self.active_runtime_pane = next;
        self.selection_dragging = false;
        self.scrollbar_drag = None;
        self.scrollbar_click_held = false;
        self.scrollbar_hover = None;
        self.selection_click = None;
        self.selection_edge = None;
        self.pressed_mouse_button = None;
        self.wheel_remainder = 0.0;
        self.mouse_wheel_remainder = 0.0;
        self.update_workspace_title();
        if self.workspace.active_tab().is_zoomed()
            && let Some(size) = self.window.as_ref().map(|window| window.inner_size())
        {
            self.resize_terminal_to_viewport(size);
        }
        self.invalidate_frame();
    }

    fn create_pane(&mut self, axis: Option<SplitAxis>) {
        self.create_pane_at_root(axis, None);
    }

    fn create_pane_at_root(&mut self, axis: Option<SplitAxis>, root: Option<PathBuf>) {
        emit_diagnostic(format_args!("app event=pane-create-start axis={axis:?}"));
        let new_pane = match axis {
            Some(axis) => self.workspace.split_active_at_root(axis, root),
            None => match root {
                Some(root) => self.workspace.new_tab_at_root(Some(root)),
                None => self.workspace.new_tab(),
            },
        };
        emit_diagnostic(format_args!(
            "app event=pane-workspace-created pane={new_pane:?}"
        ));
        self.inactive_panes
            .insert(new_pane, PaneRuntime::new(self.terminal.dimensions()));
        emit_diagnostic(format_args!(
            "app event=pane-runtime-created pane={new_pane:?}"
        ));
        if let Some(size) = self.window.as_ref().map(|window| window.inner_size()) {
            self.resize_terminal_to_viewport(size);
        }
        self.start_local_shell();
        // Native PTY startup can prevent an earlier queued redraw from being
        // delivered. Request a fresh frame after it finishes.
        self.frame.rearm();
        self.activate_pane(new_pane);
        emit_diagnostic(format_args!("app event=pane-activated pane={new_pane:?}"));
        emit_diagnostic(format_args!(
            "app event=pane-create-complete pane={new_pane:?}"
        ));
    }

    fn close_pane(&mut self) {
        let closing_tab = self.workspace.active_tab().id;
        if let Some(closed) = self.workspace.close_active_pane() {
            if !self
                .workspace
                .tabs()
                .iter()
                .any(|tab| tab.id == closing_tab)
            {
                self.unseen_tab_activity.remove(&closing_tab);
            }
            let next = self.workspace.active_pane();
            self.activate_pane(next);
            self.inactive_panes.remove(&closed);
            if let Some(size) = self.window.as_ref().map(|window| window.inner_size()) {
                self.resize_terminal_to_viewport(size);
            }
            self.invalidate_frame();
        }
    }

    fn dispatch_command(&mut self, command: Command) {
        match command {
            Command::Copy => {
                if let Some(text) = self.terminal.selected_text()
                    && let Err(error) =
                        terminal_platform::write_clipboard(&text, self.window.as_deref())
                {
                    eprintln!("could not copy selected text: {error}");
                }
            }
            Command::Paste => match terminal_platform::read_clipboard(2 * 1024 * 1024) {
                Ok(text) => {
                    self.write_terminal_input(encode_paste(*self.terminal.input_modes(), &text));
                }
                Err(error) => eprintln!("could not paste clipboard text: {error}"),
            },
            Command::PageUp | Command::PageDown => {
                let changed = if command == Command::PageUp {
                    self.terminal.page_up()
                } else {
                    self.terminal.page_down()
                };
                if changed {
                    self.invalidate_frame();
                }
            }
            Command::IncreaseFontSize | Command::DecreaseFontSize | Command::ResetFontSize => {
                if let Some(size) =
                    font_size_for_command(command, self.font_size, self.base_font_size)
                {
                    self.set_font_size(size);
                }
            }
            Command::OpenPalette => {
                self.palette = Some(Palette::with_projects(&self.config.projects));
                self.invalidate_frame();
            }
            Command::OpenTabPicker => {
                let entries = self
                    .workspace
                    .tabs()
                    .iter()
                    .enumerate()
                    .map(|(index, tab)| {
                        let terminal = if tab.active_pane == self.active_runtime_pane {
                            Some(&self.terminal)
                        } else {
                            self.inactive_panes
                                .get(&tab.active_pane)
                                .map(|runtime| &runtime.terminal)
                        };
                        PaletteEntry {
                            action: PaletteAction::SwitchTab(index),
                            name: tab_picker_label(tab, terminal, index),
                        }
                    })
                    .collect();
                self.tab_picker = Some(Palette::from_entries(entries));
                self.invalidate_frame();
            }
            Command::OpenTarget => {
                if let Some(uri) = self.pending_target.take()
                    && commands::allowed_target(&uri)
                    && let Err(error) = terminal_platform::open_url(&uri)
                {
                    eprintln!("could not open terminal target: {error}");
                }
            }
            Command::NewTab => self.create_pane(None),
            Command::SplitHorizontal => self.create_pane(Some(SplitAxis::Horizontal)),
            Command::SplitVertical => self.create_pane(Some(SplitAxis::Vertical)),
            Command::NextTab => {
                let next = self.workspace.focus_tab(1);
                self.activate_pane(next);
            }
            Command::PreviousTab => {
                let next = self.workspace.focus_tab(-1);
                self.activate_pane(next);
            }
            Command::NextPane => {
                let next = self.workspace.focus_pane(1);
                self.activate_pane(next);
            }
            Command::PreviousPane => {
                let next = self.workspace.focus_pane(-1);
                self.activate_pane(next);
            }
            Command::ResizePaneLeft
            | Command::ResizePaneRight
            | Command::ResizePaneUp
            | Command::ResizePaneDown => {
                let Some((size, metrics)) = self
                    .window
                    .as_ref()
                    .zip(self.renderer.as_ref())
                    .map(|(window, renderer)| (window.inner_size(), renderer.cell_metrics()))
                else {
                    return;
                };
                let (direction, step, minimum) = match command {
                    Command::ResizePaneLeft => (
                        PaneDirection::Left,
                        metrics.width() * 2,
                        metrics.width() * 10,
                    ),
                    Command::ResizePaneRight => (
                        PaneDirection::Right,
                        metrics.width() * 2,
                        metrics.width() * 10,
                    ),
                    Command::ResizePaneUp => {
                        (PaneDirection::Up, metrics.height(), metrics.height() * 3)
                    }
                    Command::ResizePaneDown => {
                        (PaneDirection::Down, metrics.height(), metrics.height() * 3)
                    }
                    _ => unreachable!(),
                };
                if self.workspace.resize_focused_pane(
                    direction,
                    PaneRect {
                        x: 0,
                        y: 0,
                        width: size.width,
                        height: size.height,
                    },
                    step,
                    minimum,
                ) {
                    self.resize_terminal_to_viewport(size);
                    self.invalidate_frame();
                }
            }
            Command::FocusPaneLeft
            | Command::FocusPaneRight
            | Command::FocusPaneUp
            | Command::FocusPaneDown => {
                if let Some(size) = self.window.as_ref().map(|window| window.inner_size()) {
                    let direction = match command {
                        Command::FocusPaneLeft => PaneDirection::Left,
                        Command::FocusPaneRight => PaneDirection::Right,
                        Command::FocusPaneUp => PaneDirection::Up,
                        Command::FocusPaneDown => PaneDirection::Down,
                        _ => unreachable!(),
                    };
                    if let Some(next) = self.workspace.focus_pane_direction(
                        direction,
                        PaneRect {
                            x: 0,
                            y: 0,
                            width: size.width,
                            height: size.height,
                        },
                    ) {
                        self.activate_pane(next);
                    }
                }
            }
            Command::TogglePaneZoom => {
                self.workspace.toggle_zoom();
                if let Some(size) = self.window.as_ref().map(|window| window.inner_size()) {
                    self.resize_terminal_to_viewport(size);
                }
                self.invalidate_frame();
            }
            Command::ClosePane => self.close_pane(),
            Command::RenameTab => {
                self.tab_rename = Some(String::new());
                self.invalidate_frame();
            }
        }
    }

    fn set_font_size(&mut self, size: u16) {
        let Some(scale_factor) = self.window.as_ref().map(|window| window.scale_factor()) else {
            return;
        };
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        match renderer.set_font_size(f32::from(size), scale_factor) {
            Ok(metrics_changed) => {
                self.font_size = size;
                if metrics_changed && let Some(window) = self.window.as_ref() {
                    self.resize_terminal_to_viewport(window.inner_size());
                }
                self.invalidate_frame();
            }
            Err(error) => eprintln!("could not change terminal font size: {error}"),
        }
    }

    fn dispatch_palette_action(&mut self, action: PaletteAction) {
        match action {
            PaletteAction::Command(command) => self.dispatch_command(command),
            PaletteAction::ProjectTab(root) => self.create_pane_at_root(None, Some(root)),
            PaletteAction::ProjectSplit(root, axis) => {
                self.create_pane_at_root(Some(axis), Some(root));
            }
            PaletteAction::SwitchTab(index) => {
                let current = self
                    .workspace
                    .tabs()
                    .iter()
                    .position(|tab| tab.id == self.workspace.active_tab().id)
                    .unwrap_or(0);
                let pane = self.workspace.focus_tab(index as isize - current as isize);
                self.activate_pane(pane);
            }
        }
    }

    fn handle_tab_picker_key(&mut self, key: &Key, text: Option<&str>) {
        let Some(mut picker) = self.tab_picker.take() else {
            return;
        };
        let mut chosen = None;
        let mut close = false;
        match key {
            Key::Named(NamedKey::Escape) => close = true,
            Key::Named(NamedKey::Enter) => {
                chosen = picker.chosen();
                close = true;
            }
            Key::Named(NamedKey::Backspace) => picker.backspace(),
            Key::Named(NamedKey::ArrowUp) => picker.move_selection(-1),
            Key::Named(NamedKey::ArrowDown) => picker.move_selection(1),
            Key::Character(c) if c == "j" => picker.move_selection(1),
            Key::Character(c) if c == "k" => picker.move_selection(-1),
            _ if !self.modifiers.control_key()
                && !self.modifiers.alt_key()
                && !self.modifiers.super_key() =>
            {
                if let Some(text) = text {
                    picker.push_text(text);
                }
            }
            _ => {}
        }
        if !close {
            self.tab_picker = Some(picker);
        } else {
            self.activity_deadline = None;
        }
        if let Some(action) = chosen {
            self.dispatch_palette_action(action);
        }
        self.invalidate_frame();
    }

    fn record_background_output(&mut self, pane_id: PaneId) {
        let active_tab = self.workspace.active_tab().id;
        if let Some(tab_id) = self
            .workspace
            .tabs()
            .iter()
            .find(|tab| tab.id != active_tab && tab.panes().iter().any(|pane| pane.id == pane_id))
            .map(|tab| tab.id)
            && self.unseen_tab_activity.insert(tab_id)
            && self.tab_picker.is_some()
        {
            self.invalidate_frame();
        }
    }

    fn tick_tab_activity(&mut self, now: Instant) -> Option<Instant> {
        if self.tab_picker.is_none() || self.unseen_tab_activity.is_empty() {
            self.activity_deadline = None;
            return None;
        }
        let deadline = self
            .activity_deadline
            .get_or_insert(now + Duration::from_millis(300));
        if now >= *deadline {
            self.activity_frame = (self.activity_frame + 1) % 4;
            *deadline = now + Duration::from_millis(300);
            self.invalidate_frame();
        }
        self.activity_deadline
    }

    fn handle_palette_key(&mut self, key: &Key, text: Option<&str>) {
        let Some(mut palette) = self.palette.take() else {
            return;
        };
        let mut command = None;
        let mut close = false;
        match key {
            Key::Named(NamedKey::Escape) => close = true,
            Key::Named(NamedKey::Enter) => {
                command = palette.chosen();
                close = command.is_some();
            }
            Key::Named(NamedKey::Backspace) => palette.backspace(),
            Key::Named(NamedKey::ArrowUp) => palette.move_selection(-1),
            Key::Named(NamedKey::ArrowDown) => palette.move_selection(1),
            _ if !self.modifiers.control_key()
                && !self.modifiers.alt_key()
                && !self.modifiers.super_key() =>
            {
                if let Some(text) = text {
                    palette.push_text(text);
                }
            }
            _ => {}
        }
        if !close {
            self.palette = Some(palette);
        }
        if let Some(command) = command {
            self.dispatch_palette_action(command);
        }
        self.invalidate_frame();
    }

    fn palette_overlay(&self) -> Option<TextOverlay> {
        let palette = self.palette.as_ref()?;
        let mut lines = vec![OverlayLine {
            text: format!(" Command Palette > {}", palette.query()),
            selected: false,
        }];
        let matches = palette.matches();
        if matches.is_empty() {
            lines.push(OverlayLine {
                text: " No matching commands".into(),
                selected: false,
            });
        } else {
            lines.extend(matches.iter().enumerate().map(|(index, info)| OverlayLine {
                text: format!(
                    " {} {}",
                    if index == palette.selected() {
                        '>'
                    } else {
                        ' '
                    },
                    info.name
                ),
                selected: index == palette.selected(),
            }));
        }
        Some(TextOverlay {
            lines,
            bottom: false,
            search_matches: Vec::new(),
            search_markers: Vec::new(),
        })
    }

    fn tab_picker_overlay(&self) -> Option<TextOverlay> {
        let picker = self.tab_picker.as_ref()?;
        let mut lines = vec![OverlayLine {
            text: format!(" Tab Picker > {}", picker.query()),
            selected: false,
        }];
        let matches = picker.matches();
        if matches.is_empty() {
            lines.push(OverlayLine {
                text: " No matching tabs".into(),
                selected: false,
            });
        } else {
            lines.extend(matches.iter().enumerate().map(|(index, entry)| {
                let activity = match &entry.action {
                    PaletteAction::SwitchTab(tab_index)
                        if self
                            .workspace
                            .tabs()
                            .get(*tab_index)
                            .is_some_and(|tab| self.unseen_tab_activity.contains(&tab.id)) =>
                    {
                        ['|', '/', '-', '\\'][self.activity_frame]
                    }
                    _ => ' ',
                };
                OverlayLine {
                    text: format!(
                        " {} {} {}",
                        if index == picker.selected() { '>' } else { ' ' },
                        activity,
                        entry.name
                    ),
                    selected: index == picker.selected(),
                }
            }));
        }
        Some(TextOverlay {
            lines,
            bottom: false,
            search_matches: Vec::new(),
            search_markers: Vec::new(),
        })
    }

    fn handle_search_key(&mut self, key: &Key, text: Option<&str>) {
        let Some(mut search) = self.search.take() else {
            return;
        };
        match key {
            Key::Named(NamedKey::Escape) => {}
            Key::Named(NamedKey::Enter) => {
                search.navigate(
                    &mut self.terminal,
                    if self.modifiers.shift_key() { -1 } else { 1 },
                );
                self.search = Some(search);
            }
            Key::Named(NamedKey::Backspace) => {
                search.backspace(&mut self.terminal);
                self.search = Some(search);
            }
            _ => {
                if !self.modifiers.control_key()
                    && !self.modifiers.alt_key()
                    && !self.modifiers.super_key()
                    && let Some(text) = text
                {
                    search.push_text(text, &mut self.terminal);
                }
                self.search = Some(search);
            }
        }
        self.invalidate_frame();
    }

    fn search_overlay(&self) -> Option<TextOverlay> {
        let search = self.search.as_ref()?;
        let status = match search.position() {
            Some(position) => format!("{position}/{}", search.count()),
            None if search.query().is_empty() => "Enter a query".into(),
            None => "No matches".into(),
        };
        Some(TextOverlay {
            lines: vec![OverlayLine {
                text: format!(
                    " Search: {}  [{status}]  Enter next  Shift+Enter previous  Esc close",
                    search.query()
                ),
                selected: true,
            }],
            bottom: search
                .viewport_match(&self.terminal)
                .is_some_and(|(row, _)| row == 0),
            search_matches: search
                .viewport_matches(&self.terminal)
                .into_iter()
                .map(|(row, start_column, end_column, active)| SearchHighlight {
                    row,
                    start_column,
                    end_column,
                    active,
                })
                .collect(),
            search_markers: search
                .markers()
                .into_iter()
                .map(|(row, active)| SearchMarker { row, active })
                .collect(),
        })
    }

    fn handle_tab_rename_key(&mut self, key: &Key, text: Option<&str>) {
        let Some(mut title) = self.tab_rename.take() else {
            return;
        };
        match key {
            Key::Named(NamedKey::Escape) => {
                self.invalidate_frame();
            }
            Key::Named(NamedKey::Enter) => {
                self.workspace
                    .set_active_tab_custom_title((!title.is_empty()).then_some(title));
                self.update_workspace_title();
                self.invalidate_frame();
            }
            Key::Named(NamedKey::Backspace) => {
                title.pop();
                self.tab_rename = Some(title);
                self.invalidate_frame();
            }
            _ => {
                if !self.modifiers.control_key()
                    && !self.modifiers.alt_key()
                    && !self.modifiers.super_key()
                    && let Some(text) = text
                {
                    title.push_str(text);
                }
                self.tab_rename = Some(title);
                self.invalidate_frame();
            }
        }
    }

    fn tab_rename_overlay(&self) -> Option<TextOverlay> {
        let title = self.tab_rename.as_ref()?;
        Some(TextOverlay {
            lines: vec![OverlayLine {
                text: format!(" Rename Tab: {title}  Enter save  Esc cancel  empty resets"),
                selected: true,
            }],
            bottom: false,
            search_matches: Vec::new(),
            search_markers: Vec::new(),
        })
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
        match Renderer::new_with_font_settings(
            Arc::clone(window),
            font_request(&self.config.font),
            f32::from(self.config.font.size),
        ) {
            Ok(mut renderer) => {
                renderer.set_theme(render_theme(&self.config.theme));
                self.renderer = Some(renderer);
                self.fullscreen.observe(window.fullscreen().is_some());
                if recreating_surface {
                    self.recovery_redraw.begin();
                }
                self.resize_terminal_to_viewport(window.inner_size());
                self.start_local_shell();
                self.update_workspace_title();
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
        let surface_started = std::time::Instant::now();
        let surface_changed = self
            .renderer
            .as_mut()
            .is_some_and(|renderer| renderer.resize(size.width, size.height));
        let surface_us = surface_started.elapsed().as_micros();
        work.surface_configures = usize::from(surface_changed);
        let grid_started = std::time::Instant::now();
        let (grid_resizes, pty_resizes) = self.resize_terminal_to_viewport(size);
        let grid_us = grid_started.elapsed().as_micros();
        work.terminal_grid_resizes = grid_resizes;
        work.pty_resize_commands = pty_resizes;
        if force_redraw {
            self.recovery_redraw.begin();
            work.redraw_requests = usize::from(self.invalidate_after_surface_restore());
        } else if surface_changed || grid_resizes != 0 {
            work.redraw_requests = usize::from(self.invalidate_frame());
        }
        emit_diagnostic(format_args!(
            "app event=resize-stages surface_us={surface_us} grid_and_pty_us={grid_us}"
        ));
        self.diagnose("resize-complete");
        work
    }

    fn resize_terminal_to_viewport(&mut self, size: PhysicalSize<u32>) -> (usize, usize) {
        let Some(metrics) = self.renderer.as_ref().map(Renderer::cell_metrics) else {
            return (0, 0);
        };
        if size.width == 0 || size.height == 0 {
            return (0, 0);
        }
        let mut grid_resizes = 0;
        let mut pty_resizes = 0;
        let mut terminal_resize_time = std::time::Duration::ZERO;
        let mut pty_resize_time = std::time::Duration::ZERO;
        for (pane_id, dimensions) in pane_dimensions(&self.workspace, size, metrics) {
            if pane_id == self.active_runtime_pane {
                if self.terminal.dimensions() != dimensions {
                    let started = std::time::Instant::now();
                    self.terminal.resize(dimensions);
                    terminal_resize_time += started.elapsed();
                    grid_resizes += 1;
                    let started = std::time::Instant::now();
                    pty_resizes += usize::from(self.resize_pty_to_terminal());
                    pty_resize_time += started.elapsed();
                }
            } else if let Some(runtime) = self.inactive_panes.get_mut(&pane_id)
                && runtime.terminal.dimensions() != dimensions
            {
                let started = std::time::Instant::now();
                runtime.terminal.resize(dimensions);
                terminal_resize_time += started.elapsed();
                grid_resizes += 1;
                if let Some(pty) = runtime.pty.as_ref() {
                    let started = std::time::Instant::now();
                    match pty.resize(pty_size_for_terminal(dimensions)) {
                        Ok(()) => pty_resizes += 1,
                        Err(error) => eprintln!("could not resize background shell: {error}"),
                    }
                    pty_resize_time += started.elapsed();
                }
            }
        }
        emit_diagnostic(format_args!(
            "app event=terminal-grid-resized drawable={}x{} scale_factor={} cell_metrics={metrics:?} grids={} pty_resize_commands={} terminal_us={} pty_us={}",
            size.width,
            size.height,
            self.window
                .as_ref()
                .map_or(1.0, |window| window.scale_factor()),
            grid_resizes,
            pty_resizes,
            terminal_resize_time.as_micros(),
            pty_resize_time.as_micros(),
        ));
        (grid_resizes, pty_resizes)
    }

    fn start_local_shell(&mut self) {
        let Some(proxy) = self.pty_wake_proxy.clone() else {
            return;
        };
        if self.pty.is_none()
            && let Some((root, startup)) = self.workspace.pane_launch(self.active_runtime_pane)
        {
            self.pty = spawn_local_shell(
                proxy.clone(),
                Arc::clone(&self.pty_wake_pending),
                self.terminal.dimensions(),
                root,
                startup,
            );
        }
        for (pane_id, runtime) in &mut self.inactive_panes {
            if runtime.pty.is_none()
                && let Some((root, startup)) = self.workspace.pane_launch(*pane_id)
            {
                runtime.pty = spawn_local_shell(
                    proxy.clone(),
                    Arc::clone(&self.pty_wake_pending),
                    runtime.terminal.dimensions(),
                    root,
                    startup,
                );
            }
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

    fn drain_pty_events(&mut self, request_redraw: bool) -> bool {
        let started = std::time::Instant::now();
        let mut events = 0;
        let mut bytes = 0;
        let mut visible_output = false;
        while let Some(pty) = self.pty.as_ref() {
            let event = match pty.recv_timeout(Duration::ZERO) {
                Ok(Some(event)) => event,
                Ok(None) => break,
                Err(error) => {
                    eprintln!("could not receive local shell output: {error}");
                    break;
                }
            };
            events += 1;
            if let PtyWorkerEvent::Output(PtyOutput::Bytes(chunk)) = &event {
                bytes += chunk.len();
            }
            visible_output |= self.handle_pty_event(event);
        }
        let visible: Vec<_> = self
            .workspace
            .active_tab()
            .panes()
            .iter()
            .map(|pane| pane.id)
            .collect();
        let mut background_output = HashSet::new();
        for (pane_id, runtime) in &mut self.inactive_panes {
            while let Some(pty) = runtime.pty.as_ref() {
                let event = match pty.recv_timeout(Duration::ZERO) {
                    Ok(Some(event)) => event,
                    Ok(None) => break,
                    Err(error) => {
                        eprintln!("could not receive background shell output: {error}");
                        break;
                    }
                };
                events += 1;
                if let PtyWorkerEvent::Output(PtyOutput::Bytes(chunk)) = &event {
                    bytes += chunk.len();
                    visible_output |= visible.contains(pane_id);
                    if !chunk.is_empty() && !visible.contains(pane_id) {
                        background_output.insert(*pane_id);
                    }
                }
                handle_background_pty_event(runtime, event);
            }
        }
        for pane_id in background_output {
            self.record_background_output(pane_id);
        }
        let output_us = started.elapsed().as_micros();
        let invalidate_started = std::time::Instant::now();
        if visible_output {
            if self.terminal.active_screen() != terminal_core::ScreenKind::Primary {
                self.search = None;
            } else if let Some(search) = self.search.as_mut() {
                search.refresh(&mut self.terminal);
            }
        }
        if visible_output && request_redraw {
            self.invalidate_frame();
        }
        emit_diagnostic(format_args!(
            "app event=pty-drain events={events} bytes={bytes} elapsed_us={} output_us={output_us} invalidate_us={}",
            started.elapsed().as_micros(),
            invalidate_started.elapsed().as_micros(),
        ));
        visible_output
    }

    fn handle_pty_event(&mut self, event: PtyWorkerEvent) -> bool {
        match event {
            PtyWorkerEvent::Output(PtyOutput::Bytes(bytes)) => {
                let parse_started = std::time::Instant::now();
                self.terminal.clear_selection();
                let previous_title_version = self.terminal.shell_title_version();
                let replies = parse_terminal_output(&mut self.parser, &mut self.terminal, &bytes);
                if self.terminal.shell_title_version() != previous_title_version {
                    self.update_workspace_title();
                }
                emit_diagnostic(format_args!(
                    "app event=parser-feed bytes={} elapsed_us={}",
                    bytes.len(),
                    parse_started.elapsed().as_micros()
                ));
                if let Some(text) = self.terminal.take_osc52_write()
                    && let Err(error) =
                        terminal_platform::write_clipboard(&text, self.window.as_deref())
                {
                    eprintln!("could not write OSC 52 clipboard text: {error}");
                }
                for reply in replies {
                    self.write_to_pty(reply);
                }
                true
            }
            PtyWorkerEvent::Output(PtyOutput::Eof) => {
                emit_diagnostic(format_args!("app pty-output=eof"));
                false
            }
            PtyWorkerEvent::Output(PtyOutput::Exited(status)) => {
                emit_diagnostic(format_args!("app pty-output=exited status={status:?}"));
                false
            }
            PtyWorkerEvent::Error(error) => {
                eprintln!("local shell worker error: {error}");
                false
            }
        }
    }

    fn write_to_pty(&self, bytes: Vec<u8>) -> bool {
        queue_pty_write(self.pty.as_ref(), bytes)
    }

    fn write_terminal_input(&mut self, bytes: Vec<u8>) -> bool {
        let (queued, viewport_changed) = queue_terminal_input(&mut self.terminal, bytes, |bytes| {
            queue_pty_write(self.pty.as_ref(), bytes)
        });
        if viewport_changed {
            self.invalidate_frame();
        }
        queued
    }

    fn send_mouse_event(&mut self, event: MouseEvent) {
        let Some((column, row)) = self.pointer_position.and_then(|position| {
            let metrics = self.renderer.as_ref()?.cell_metrics();
            terminal_cell_at(
                self.local_pointer_position(position)?,
                metrics,
                self.terminal.dimensions(),
            )
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
            self.write_terminal_input(bytes);
        }
    }

    fn pane_rects(&self) -> Option<Vec<(PaneId, PaneRect)>> {
        let size = self.window.as_ref()?.inner_size();
        Some(self.workspace.active_tab().pane_rects(PaneRect {
            x: 0,
            y: 0,
            width: size.width,
            height: size.height,
        }))
    }

    fn local_pointer_position(
        &self,
        position: PhysicalPosition<f64>,
    ) -> Option<PhysicalPosition<f64>> {
        let (_, rect) = self
            .pane_rects()?
            .into_iter()
            .find(|(pane, _)| *pane == self.workspace.active_pane())?;
        if !position.x.is_finite()
            || !position.y.is_finite()
            || position.x < rect.x as f64
            || position.y < rect.y as f64
            || position.x >= (rect.x + rect.width) as f64
            || position.y >= (rect.y + rect.height) as f64
        {
            return None;
        }
        Some(PhysicalPosition::new(
            position.x - rect.x as f64,
            position.y - rect.y as f64,
        ))
    }

    fn scrollbar_geometry(&self) -> Option<ScrollbarGeometry> {
        if self.terminal.active_screen() != ScreenKind::Primary {
            return None;
        }
        let (_, rect) = self
            .pane_rects()?
            .into_iter()
            .find(|(pane, _)| *pane == self.workspace.active_pane())?;
        ScrollbarGeometry::new(
            ScrollbarRenderData {
                history_rows: self.terminal.scrollback_len(),
                visible_rows: self.terminal.dimensions().rows(),
                viewport_offset: self.terminal.viewport_offset(),
            },
            SurfaceSize::new(rect.width, rect.height)?,
            self.renderer.as_ref()?.cell_metrics().height() as f32,
        )
    }

    fn scrollbar_hit(&self, position: PhysicalPosition<f64>) -> Option<ScrollbarHit> {
        let local = self.local_pointer_position(position)?;
        self.scrollbar_geometry()?.hit(local.x, local.y)
    }

    fn scrollbar_input_enabled(&self) -> bool {
        self.terminal.input_modes().mouse_tracking() == MouseTracking::Off
            || self.modifiers.shift_key()
    }

    fn set_scrollbar_hover(&mut self, hover: Option<ScrollbarHit>) {
        if self.scrollbar_hover != hover {
            self.scrollbar_hover = hover;
            self.invalidate_frame();
        }
    }

    fn scroll_to_offset(&mut self, offset: usize) {
        let current = self.terminal.viewport_offset();
        let delta = (offset as i64 - current as i64).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        if delta != 0 && self.terminal.scroll_viewport_rows(delta) {
            self.invalidate_frame();
        }
    }

    fn page_scrollbar(&mut self, hit: ScrollbarHit) {
        let delta = scrollbar_page_delta(hit, self.terminal.dimensions().rows());
        if self.terminal.scroll_viewport_rows(delta) {
            self.invalidate_frame();
        }
    }

    fn focus_pane_at_pointer(&mut self) {
        let Some(position) = self.pointer_position else {
            return;
        };
        if !position.x.is_finite()
            || !position.y.is_finite()
            || position.x < 0.0
            || position.y < 0.0
        {
            return;
        }
        let Some(pane) = self.pane_rects().and_then(|rects| {
            rects
                .into_iter()
                .find(|(_, rect)| rect.contains(position.x as u32, position.y as u32))
                .map(|(pane, _)| pane)
        }) else {
            return;
        };
        if self.workspace.focus_pane_id(pane) {
            self.activate_pane(pane);
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

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
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
        if let Some((direction, row, column, deadline)) = self.selection_edge
            && self.selection_dragging
            && Instant::now() >= deadline
        {
            if self
                .terminal
                .scroll_selection_viewport_rows(direction, row, column)
            {
                self.invalidate_frame();
                self.selection_edge = Some((
                    direction,
                    row,
                    column,
                    Instant::now() + Duration::from_millis(60),
                ));
            } else {
                self.selection_edge = None;
            }
        }
        let activity_deadline = self.tick_tab_activity(Instant::now());
        let deadline = self
            .selection_edge
            .map(|(_, _, _, deadline)| deadline)
            .into_iter()
            .chain(activity_deadline)
            .min();
        event_loop.set_control_flow(deadline.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
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
                    self.selection_dragging = false;
                    self.selection_click = None;
                    self.selection_edge = None;
                    self.scrollbar_drag = None;
                    self.scrollbar_click_held = false;
                    self.set_scrollbar_hover(None);
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
                if !self.scrollbar_click_held {
                    self.set_scrollbar_hover(if self.scrollbar_input_enabled() {
                        self.pointer_position
                            .and_then(|position| self.scrollbar_hit(position))
                    } else {
                        None
                    });
                }
            }
            WindowEvent::CursorLeft { .. } => {
                self.pointer_position = None;
                self.selection_edge = None;
                self.set_scrollbar_hover(None);
            }
            WindowEvent::CursorMoved { position, .. } => {
                if self.selection_dragging {
                    emit_diagnostic(format_args!("app event=selection-move"));
                }
                self.pointer_position = Some(position);
                if let Some(grab_y) = self.scrollbar_drag {
                    if let Some(geometry) = self.scrollbar_geometry()
                        && let Some((_, rect)) = self.pane_rects().and_then(|rects| {
                            rects
                                .into_iter()
                                .find(|(pane, _)| *pane == self.workspace.active_pane())
                        })
                    {
                        self.scroll_to_offset(
                            geometry.offset_for_drag(position.y - rect.y as f64, grab_y),
                        );
                    }
                    self.set_scrollbar_hover(Some(ScrollbarHit::Thumb));
                    return;
                }
                let hover = if self.scrollbar_input_enabled() {
                    self.scrollbar_hit(position)
                } else {
                    None
                };
                self.set_scrollbar_hover(hover);
                if self.scrollbar_click_held {
                    return;
                }
                if self.selection_dragging {
                    if let Some((column, row)) = self.renderer.as_ref().and_then(|renderer| {
                        terminal_cell_at(
                            self.local_pointer_position(position)?,
                            renderer.cell_metrics(),
                            self.terminal.dimensions(),
                        )
                    }) {
                        if self.terminal.extend_selection(row, column) {
                            self.selection_click = None;
                            self.invalidate_frame();
                        }
                        let direction =
                            selection_edge_direction(row, self.terminal.dimensions().rows());
                        self.selection_edge = direction.map(|direction| {
                            let deadline = self
                                .selection_edge
                                .filter(|(previous, _, _, _)| *previous == direction)
                                .map_or(
                                    Instant::now() + Duration::from_millis(60),
                                    |(_, _, _, deadline)| deadline,
                                );
                            (direction, row, column, deadline)
                        });
                    } else {
                        self.selection_edge = None;
                    }
                } else {
                    self.send_mouse_event(MouseEvent::Move(self.pressed_mouse_button));
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if state == ElementState::Pressed {
                    self.focus_pane_at_pointer();
                }
                if button == MouseButton::Left && self.target_click_held {
                    if state == ElementState::Released {
                        self.target_click_held = false;
                    }
                    return;
                }
                if button == MouseButton::Left && self.scrollbar_click_held {
                    if state == ElementState::Released {
                        self.scrollbar_drag = None;
                        self.scrollbar_click_held = false;
                        self.set_scrollbar_hover(if self.scrollbar_input_enabled() {
                            self.pointer_position
                                .and_then(|position| self.scrollbar_hit(position))
                        } else {
                            None
                        });
                    }
                    return;
                }
                if button == MouseButton::Left
                    && state == ElementState::Pressed
                    && !self.selection_dragging
                    && self.scrollbar_input_enabled()
                    && let Some(position) = self.pointer_position
                    && let Some(hit) = self.scrollbar_hit(position)
                {
                    self.scrollbar_click_held = true;
                    match hit {
                        ScrollbarHit::Thumb => {
                            if let Some(geometry) = self.scrollbar_geometry()
                                && let Some(local) = self.local_pointer_position(position)
                            {
                                self.scrollbar_drag = Some(local.y - geometry.thumb[1] as f64);
                            }
                        }
                        ScrollbarHit::TrackAbove | ScrollbarHit::TrackBelow => {
                            self.page_scrollbar(hit);
                        }
                    }
                    self.set_scrollbar_hover(self.scrollbar_hit(position));
                    return;
                }
                if button == MouseButton::Left
                    && state == ElementState::Pressed
                    && !self.selection_dragging
                    && self.modifiers.control_key()
                    && self.modifiers.shift_key()
                {
                    self.target_click_held = true;
                    self.pending_target = self.pointer_position.and_then(|position| {
                        let metrics = self.renderer.as_ref()?.cell_metrics();
                        target_at_pointer(
                            &self.terminal,
                            self.local_pointer_position(position)?,
                            metrics,
                        )
                        .map(str::to_owned)
                    });
                    self.dispatch_command(Command::OpenTarget);
                    return;
                }
                if button == MouseButton::Left
                    && (self.selection_dragging
                        || self.terminal.input_modes().mouse_tracking() == MouseTracking::Off
                        || self.modifiers.shift_key())
                {
                    if state == ElementState::Pressed {
                        if let Some((column, row)) = self.pointer_position.and_then(|position| {
                            let metrics = self.renderer.as_ref()?.cell_metrics();
                            terminal_cell_at(
                                self.local_pointer_position(position)?,
                                metrics,
                                self.terminal.dimensions(),
                            )
                        }) {
                            let now = Instant::now();
                            let count = if self.modifiers.shift_key() {
                                1
                            } else {
                                selection_click_count(self.selection_click, now, row, column)
                            };
                            self.selection_click = Some((now, row, column, count));
                            let previous_selection = self.terminal.has_selection();
                            self.selection_dragging =
                                if self.modifiers.shift_key() && previous_selection {
                                    self.selection_click = None;
                                    self.terminal.extend_selection(row, column);
                                    true
                                } else {
                                    match count {
                                        2 => self.terminal.select_word(row, column),
                                        3 => self.terminal.select_line(row, column),
                                        _ => self.terminal.begin_selection(row, column),
                                    }
                                };
                            emit_diagnostic(format_args!("app event=selection-start"));
                            if previous_selection || count > 1 {
                                self.invalidate_frame();
                            }
                        }
                    } else {
                        self.selection_dragging = false;
                        self.selection_edge = None;
                        emit_diagnostic(format_args!("app event=selection-end"));
                    }
                    return;
                }
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
                self.focus_pane_at_pointer();
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
                emit_diagnostic(format_args!(
                    "app event=keyboard logical={:?} physical={:?} text={:?}",
                    event.logical_key, event.physical_key, event.text
                ));
                if self.tab_rename.is_some() {
                    self.handle_tab_rename_key(&event.logical_key, event.text.as_deref());
                } else if self.search.is_some() {
                    self.handle_search_key(&event.logical_key, event.text.as_deref());
                } else if self.palette.is_some() {
                    self.handle_palette_key(&event.logical_key, event.text.as_deref());
                } else if self.tab_picker.is_some() {
                    self.handle_tab_picker_key(&event.logical_key, event.text.as_deref());
                } else if let Some(command) =
                    configured_command(&self.config, event.physical_key, self.modifiers)
                {
                    self.dispatch_command(command);
                } else if event.physical_key == PhysicalKey::Code(KeyCode::KeyF)
                    && self.modifiers == ModifiersState::CONTROL
                    && self.terminal.active_screen() == terminal_core::ScreenKind::Primary
                {
                    self.search = Some(Search::default());
                    self.invalidate_frame();
                } else if let Some(cursor_key) = cursor_key_from_logical_key(&event.logical_key) {
                    self.write_terminal_input(
                        terminal_cursor_key_input(
                            *self.terminal.input_modes(),
                            cursor_key,
                            self.modifiers,
                        )
                        .to_vec(),
                    );
                } else {
                    let key = basic_key_from_logical_key(&event.logical_key);
                    if let Some(bytes) = terminal_key_input(
                        event.text.as_deref(),
                        key,
                        event.physical_key,
                        self.modifiers,
                    ) {
                        emit_diagnostic(format_args!("app event=key-input bytes={}", bytes.len()));
                        let is_backspace = key == Some(BasicKey::Backspace)
                            || event.physical_key == PhysicalKey::Code(KeyCode::Backspace);
                        let pty_write_queued = self.write_terminal_input(bytes.clone());
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
                // During a live resize, redraws can arrive before about_to_wait.
                // Apply the latest queued size before acquiring a surface frame;
                // otherwise a suboptimal present can reconfigure the old size
                // repeatedly while newer Resized events wait in the queue.
                self.apply_pending_resize();
                // A PTY worker may have queued another chunk after its wake was
                // handled but before this redraw. Include it in this present.
                let output_drained_before_redraw =
                    if self.pty_wake_pending.swap(false, Ordering::AcqRel) {
                        self.drain_pty_events(false)
                    } else {
                        false
                    };
                self.frame.begin_redraw();
                let overlay = self
                    .tab_rename_overlay()
                    .or_else(|| self.search_overlay())
                    .or_else(|| self.palette_overlay())
                    .or_else(|| self.tab_picker_overlay());

                let scrollback_input_enabled = self.scrollbar_input_enabled();

                let Some(renderer) = self.renderer.as_mut() else {
                    return;
                };
                let size = self
                    .window
                    .as_ref()
                    .map(|window| window.inner_size())
                    .unwrap_or(PhysicalSize::new(0, 0));
                let surface = PaneRect {
                    x: 0,
                    y: 0,
                    width: size.width,
                    height: size.height,
                };
                let panes: Vec<_> = self
                    .workspace
                    .active_tab()
                    .pane_rects(surface)
                    .into_iter()
                    .filter_map(|(pane_id, rect)| {
                        let terminal = if pane_id == self.active_runtime_pane {
                            &self.terminal
                        } else {
                            &self.inactive_panes.get(&pane_id)?.terminal
                        };
                        Some(PaneRenderInput {
                            terminal,
                            rect: [rect.x, rect.y, rect.width, rect.height],
                            focused: pane_id == self.workspace.active_pane(),
                            scrollbar_hover: (pane_id == self.workspace.active_pane()
                                && scrollback_input_enabled)
                                .then_some(self.scrollbar_hover)
                                .flatten(),
                        })
                    })
                    .collect();
                let outcome = renderer.redraw_panes(&panes, overlay.as_ref());
                emit_diagnostic(format_args!(
                    "app event=redraw-complete outcome={outcome:?}"
                ));
                match outcome {
                    RedrawOutcome::Reconfigured => {
                        if self.recovery_redraw.reconfigured() || output_drained_before_redraw {
                            self.invalidate_frame();
                        }
                    }
                    RedrawOutcome::Exit => event_loop.exit(),
                    RedrawOutcome::Presented => {
                        if self.recovery_redraw.presented() {
                            self.invalidate_frame();
                        }
                    }
                    RedrawOutcome::Skipped => {
                        self.recovery_redraw.skipped();
                        if output_drained_before_redraw {
                            self.invalidate_frame();
                        }
                    }
                }
                self.diagnose("redraw-handled");
            }
            _ => {}
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: PtyWake) {
        match event {
            PtyWake::OutputAvailable => {
                // Clear before draining: output queued during the drain can arm a
                // successor wake, so no worker event is stranded by a race.
                self.pty_wake_pending.store(false, Ordering::Release);
                emit_diagnostic(format_args!("app event=pty-wake"));
                self.drain_pty_events(true);
            }
            PtyWake::ConfigChanged => self.reload_config(),
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

fn spawn_local_shell(
    proxy: EventLoopProxy<PtyWake>,
    wake_pending: Arc<AtomicBool>,
    dimensions: TerminalDimensions,
    root: Option<PathBuf>,
    startup: SessionDefinition,
) -> Option<PtyWorker> {
    emit_diagnostic(format_args!("app event=pty-backend-start"));
    let backend = PortablePtyBackend::new();
    emit_diagnostic(format_args!("app event=pty-session-spawn-start"));
    let session = match backend.spawn(spawn_config_for_session(
        pty_size_for_terminal(dimensions),
        root,
        startup,
    )) {
        Ok(session) => session,
        Err(error) => {
            eprintln!("could not start local shell: {error}");
            return None;
        }
    };
    emit_diagnostic(format_args!("app event=pty-session-spawn-complete"));
    emit_diagnostic(format_args!("app event=pty-worker-start"));
    match PtyWorker::start_with_notifier(session, move || {
        if !wake_pending.swap(true, Ordering::AcqRel)
            && proxy.send_event(PtyWake::OutputAvailable).is_err()
        {
            wake_pending.store(false, Ordering::Release);
        }
    }) {
        Ok(worker) => {
            emit_diagnostic(format_args!("app event=pty-worker-start-complete"));
            Some(worker)
        }
        Err(error) => {
            eprintln!("could not start local shell worker: {error}");
            None
        }
    }
}

fn handle_background_pty_event(runtime: &mut PaneRuntime, event: PtyWorkerEvent) {
    match event {
        PtyWorkerEvent::Output(PtyOutput::Bytes(bytes)) => {
            runtime.terminal.clear_selection();
            let replies = parse_terminal_output(&mut runtime.parser, &mut runtime.terminal, &bytes);
            // OSC 52 is denied by the parser for every pane.
            runtime.terminal.take_osc52_write();
            if let Some(pty) = runtime.pty.as_ref() {
                for reply in replies {
                    if let Err(error) = pty.write(reply) {
                        eprintln!("could not reply to background shell: {error}");
                    }
                }
            }
        }
        PtyWorkerEvent::Output(PtyOutput::Eof | PtyOutput::Exited(_)) => {}
        PtyWorkerEvent::Error(error) => eprintln!("background shell worker error: {error}"),
    }
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

fn spawn_config_for_session(
    size: PtySize,
    root: Option<PathBuf>,
    startup: SessionDefinition,
) -> PtySpawnConfig {
    let config = match startup {
        SessionDefinition::LocalShell => local_shell_spawn_config(size),
        SessionDefinition::Command { program, args } => {
            PtySpawnConfig::new(program, size).with_arguments(args.into_iter().map(OsString::from))
        }
    };
    match root {
        Some(root) => config.with_working_directory(root),
        None => config,
    }
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

fn target_at_pointer(
    terminal: &TerminalState,
    position: PhysicalPosition<f64>,
    metrics: CellMetrics,
) -> Option<&str> {
    let (column, row) = terminal_cell_at(position, metrics, terminal.dimensions())?;
    terminal.target_at_viewport(row, column)
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

fn terminal_cursor_key_input(
    modes: InputModes,
    key: CursorKey,
    modifiers: ModifiersState,
) -> &'static [u8] {
    if modifiers == ModifiersState::CONTROL
        && let Some(bytes) = encode_control_cursor_key(key)
    {
        return bytes;
    }
    encode_cursor_key(modes, key)
}

fn terminal_key_input(
    text: Option<&str>,
    key: Option<BasicKey>,
    physical_key: PhysicalKey,
    modifiers: ModifiersState,
) -> Option<Vec<u8>> {
    if modifiers == ModifiersState::CONTROL
        && let PhysicalKey::Code(code) = physical_key
        && let Some(byte) = control_letter_byte(code)
    {
        return Some(vec![byte]);
    }
    basic_key_input(text, key)
}

fn control_letter_byte(code: KeyCode) -> Option<u8> {
    Some(match code {
        KeyCode::KeyA => 0x01,
        KeyCode::KeyB => 0x02,
        KeyCode::KeyC => 0x03,
        KeyCode::KeyD => 0x04,
        KeyCode::KeyE => 0x05,
        KeyCode::KeyF => 0x06,
        KeyCode::KeyG => 0x07,
        KeyCode::KeyH => 0x08,
        KeyCode::KeyI => 0x09,
        KeyCode::KeyJ => 0x0a,
        KeyCode::KeyK => 0x0b,
        KeyCode::KeyL => 0x0c,
        KeyCode::KeyM => 0x0d,
        KeyCode::KeyN => 0x0e,
        KeyCode::KeyO => 0x0f,
        KeyCode::KeyP => 0x10,
        KeyCode::KeyQ => 0x11,
        KeyCode::KeyR => 0x12,
        KeyCode::KeyS => 0x13,
        KeyCode::KeyT => 0x14,
        KeyCode::KeyU => 0x15,
        KeyCode::KeyV => 0x16,
        KeyCode::KeyW => 0x17,
        KeyCode::KeyX => 0x18,
        KeyCode::KeyY => 0x19,
        KeyCode::KeyZ => 0x1a,
        _ => return None,
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

fn pane_dimensions(
    workspace: &Workspace,
    size: PhysicalSize<u32>,
    metrics: CellMetrics,
) -> Vec<(PaneId, TerminalDimensions)> {
    let surface = PaneRect {
        x: 0,
        y: 0,
        width: size.width,
        height: size.height,
    };
    workspace
        .tabs()
        .iter()
        .flat_map(|tab| tab.pane_rects(surface))
        .filter_map(|(pane, rect)| {
            terminal_dimensions_for_viewport(PhysicalSize::new(rect.width, rect.height), metrics)
                .map(|dimensions| (pane, dimensions))
        })
        .collect()
}

fn config_path() -> PathBuf {
    if let Some(path) = std::env::var_os("TERMINAL_CONFIG") {
        return PathBuf::from(path);
    }
    let directory = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    };
    directory
        .unwrap_or_else(|| PathBuf::from("."))
        .join("terminal")
        .join("config.toml")
}

fn config_fingerprint(path: &std::path::Path) -> Option<u64> {
    use std::hash::{Hash, Hasher};
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    Some(hasher.finish())
}

fn font_request(font: &FontConfig) -> FontRequest {
    match &font.family {
        Some(family) => FontRequest::Family(family.clone()),
        None => FontRequest::SystemMonospace,
    }
}

fn render_theme(theme: &terminal_config::Theme) -> RenderTheme {
    let rgba = |color: Rgb, alpha: f32| {
        Rgba([
            color.0 as f32 / 255.0,
            color.1 as f32 / 255.0,
            color.2 as f32 / 255.0,
            alpha,
        ])
    };
    RenderTheme {
        foreground: rgba(theme.foreground, 1.0),
        background: rgba(theme.background, 1.0),
        cursor: rgba(theme.cursor, 0.45),
        selection_foreground: rgba(theme.selection_foreground, 1.0),
        selection_background: rgba(theme.selection_background, 1.0),
        ansi: theme.ansi.map(|color| rgba(color, 1.0)),
    }
}

fn font_size_for_command(command: Command, current: u16, base: u16) -> Option<u16> {
    match command {
        Command::IncreaseFontSize => Some(current.saturating_add(1).min(256)),
        Command::DecreaseFontSize => Some(current.saturating_sub(1).max(1)),
        Command::ResetFontSize => Some(base),
        _ => None,
    }
}

fn configured_command(
    config: &Config,
    physical_key: PhysicalKey,
    modifiers: ModifiersState,
) -> Option<Command> {
    let PhysicalKey::Code(code) = physical_key else {
        return None;
    };
    let key = format!("{code:?}");
    config
        .bindings
        .iter()
        .find(|binding| {
            binding.key.key == key
                && binding.key.control == modifiers.control_key()
                && binding.key.shift == modifiers.shift_key()
                && binding.key.alt == modifiers.alt_key()
                && !modifiers.super_key()
        })
        .map(|binding| binding.command)
}

fn main() {
    let cli = match CliOptions::parse(std::env::args_os().skip(1)) {
        Ok(Some(cli)) => cli,
        Ok(None) => {
            println!("Usage: terminal-app [--project-root DIRECTORY] [--workspace CONFIG.toml]");
            return;
        }
        Err(error) => {
            eprintln!(
                "{error}\nUsage: terminal-app [--project-root DIRECTORY] [--workspace CONFIG.toml]"
            );
            std::process::exit(2);
        }
    };
    let event_loop = EventLoop::<PtyWake>::with_user_event()
        .build()
        .expect("could not create terminal event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let pty_wake_proxy = event_loop.create_proxy();
    event_loop
        .run_app(&mut Application::with_pty_wake_proxy(pty_wake_proxy, cli))
        .expect("terminal event loop failed");
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use super::Search;
    use super::{scrollbar_page_delta, selection_click_count, selection_edge_direction};
    use terminal_renderer::ScrollbarHit;

    #[test]
    fn scrollbar_track_click_pages_in_the_expected_direction() {
        assert_eq!(scrollbar_page_delta(ScrollbarHit::TrackAbove, 24), 24);
        assert_eq!(scrollbar_page_delta(ScrollbarHit::TrackBelow, 24), -24);
        assert_eq!(scrollbar_page_delta(ScrollbarHit::Thumb, 24), 0);
    }

    #[test]
    fn scrollbar_hover_invalidates_only_on_change() {
        let mut app = Application::default();
        app.set_scrollbar_hover(Some(ScrollbarHit::TrackAbove));
        assert_eq!(app.scrollbar_hover, Some(ScrollbarHit::TrackAbove));
        assert_eq!(app.frame.requests_armed, 1);
        app.frame.begin_redraw();
        app.set_scrollbar_hover(Some(ScrollbarHit::TrackAbove));
        assert_eq!(app.frame.requests_armed, 1);
        app.set_scrollbar_hover(Some(ScrollbarHit::Thumb));
        assert_eq!(app.frame.requests_armed, 2);
        app.set_scrollbar_hover(None);
        assert_eq!(app.scrollbar_hover, None);
    }

    #[test]
    fn click_count_tracks_same_cell_within_timeout() {
        let now = Instant::now();
        assert_eq!(selection_click_count(None, now, 1, 2), 1);
        assert_eq!(
            selection_click_count(Some((now, 1, 2, 1)), now + Duration::from_millis(100), 1, 2),
            2
        );
        assert_eq!(
            selection_click_count(Some((now, 1, 2, 2)), now + Duration::from_millis(100), 1, 2),
            3
        );
        assert_eq!(
            selection_click_count(Some((now, 1, 2, 3)), now + Duration::from_millis(100), 1, 2),
            1
        );
        assert_eq!(
            selection_click_count(Some((now, 1, 2, 1)), now + Duration::from_millis(501), 1, 2),
            1
        );
        assert_eq!(
            selection_click_count(Some((now, 1, 2, 1)), now + Duration::from_millis(100), 1, 3),
            1
        );
    }

    #[test]
    fn edge_direction_only_applies_at_viewport_borders() {
        assert_eq!(selection_edge_direction(0, 5), Some(1));
        assert_eq!(selection_edge_direction(4, 5), Some(-1));
        assert_eq!(selection_edge_direction(2, 5), None);
    }
    use super::{
        Application, BasicKey, CliOptions, FrameState, PaletteAction, PendingResize,
        PhysicalSizeSync, RecoveryRedraw, SurfaceRestore, WindowsShellSource,
        basic_backspace_byte_for_platform, basic_key_input, configured_command,
        cursor_key_from_logical_key, font_request, pane_dimensions, parse_terminal_output,
        pty_size_for_terminal, queue_terminal_input, scroll_terminal_for_wheel,
        select_windows_shell, target_at_pointer, terminal_cell_at, terminal_cursor_key_input,
        terminal_dimensions_for_viewport, terminal_key_input, wheel_scroll_rows,
    };
    use terminal_config::{Command, Config, Rgb};
    use terminal_core::{CursorKey, TerminalDimensions, TerminalParser, TerminalState};
    use terminal_pty::{PtyOutput, PtyWorkerEvent};
    use terminal_renderer::{
        CellMetrics, FontRequest, ScrollbarGeometry, ScrollbarRenderData, SurfaceSize,
    };
    use terminal_workspace::{LayoutDefinition, PaneDirection, PaneRect, SplitAxis};
    use winit::dpi::{PhysicalPosition, PhysicalSize};
    use winit::event::MouseScrollDelta;
    use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};

    #[test]
    fn font_size_commands_increase_decrease_reset_and_clamp() {
        assert_eq!(
            crate::font_size_for_command(Command::IncreaseFontSize, 16, 20),
            Some(17)
        );
        assert_eq!(
            crate::font_size_for_command(Command::DecreaseFontSize, 16, 20),
            Some(15)
        );
        assert_eq!(
            crate::font_size_for_command(Command::ResetFontSize, 24, 20),
            Some(20)
        );
        assert_eq!(
            crate::font_size_for_command(Command::IncreaseFontSize, 256, 20),
            Some(256)
        );
        assert_eq!(
            crate::font_size_for_command(Command::DecreaseFontSize, 1, 20),
            Some(1)
        );
        assert_eq!(crate::font_size_for_command(Command::Copy, 16, 20), None);
    }

    #[test]
    fn reset_uses_the_startup_configured_font_size() {
        let config = Config::parse("[font]\nsize = 20").unwrap();
        assert_eq!(
            crate::font_size_for_command(Command::ResetFontSize, 28, config.font.size),
            Some(20)
        );
    }

    #[test]
    fn config_font_family_maps_to_renderer_request() {
        assert_eq!(
            font_request(&Config::default().font),
            FontRequest::SystemMonospace
        );
        let configured = Config::parse("[font]\nfamily = 'Cascadia Mono'\nsize = 20").unwrap();
        assert_eq!(
            font_request(&configured.font),
            FontRequest::Family("Cascadia Mono".into())
        );
    }

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
    fn startup_command_builds_a_direct_pty_launch_in_its_project_root() {
        let config = super::spawn_config_for_session(
            terminal_pty::PtySize::new(24, 80).unwrap(),
            Some("project".into()),
            terminal_workspace::SessionDefinition::Command {
                program: "builder".into(),
                args: vec!["--watch".into(), "two words".into()],
            },
        );
        assert_eq!(config.program(), std::path::Path::new("builder"));
        assert_eq!(config.arguments(), &["--watch", "two words"]);
        assert_eq!(
            config.working_directory(),
            Some(std::path::Path::new("project"))
        );
    }

    #[test]
    fn cli_accepts_root_and_workspace_file_and_rejects_bad_input() {
        let root = std::env::temp_dir();
        let file = root.join(format!("terminal-workspace-{}.toml", std::process::id()));
        fs::write(&file, "[workspace]\nactive_tab = 0\n[[workspace.tabs]]\ntitle = 'One'\nactive_pane = 0\n[workspace.tabs.layout]\nkind = 'pane'\nsession = 'local_shell'").unwrap();
        let args = vec![
            "--project-root".into(),
            root.clone().into_os_string(),
            "--workspace".into(),
            file.clone().into_os_string(),
        ];
        let parsed = CliOptions::parse(args).unwrap().unwrap();
        assert_eq!(parsed.project_root, Some(root.canonicalize().unwrap()));
        assert_eq!(parsed.workspace, Some(file.canonicalize().unwrap()));
        assert!(CliOptions::parse(vec!["--workspace".into(), "missing-file".into()]).is_err());
        assert!(CliOptions::parse(vec!["--unknown".into()]).is_err());
        assert_eq!(CliOptions::parse(vec!["--help".into()]).unwrap(), None);
        fs::remove_file(file).unwrap();
    }

    #[test]
    fn configured_project_actions_create_independent_tab_and_split_sessions() {
        let mut app = Application::default();
        let first = app.workspace.active_pane();
        app.dispatch_palette_action(PaletteAction::ProjectTab("core".into()));
        let tab = app.workspace.active_pane();
        assert_ne!(first, tab);
        assert_eq!(
            app.workspace.pane_launch(tab).unwrap().0,
            Some("core".into())
        );
        app.dispatch_palette_action(PaletteAction::ProjectSplit(
            "core".into(),
            SplitAxis::Horizontal,
        ));
        let split = app.workspace.active_pane();
        assert_eq!(
            app.workspace.pane_launch(split).unwrap().0,
            Some("core".into())
        );
        assert_ne!(
            app.workspace.panes()[0].session,
            app.workspace.panes()[1].session
        );
        app.dispatch_command(Command::ClosePane);
        assert_eq!(app.workspace.active_pane(), tab);
    }

    #[test]
    fn workspace_commands_keep_pane_output_and_focus_separate() {
        let mut app = Application::default();
        let first = app.workspace.active_pane();
        app.handle_pty_event(PtyWorkerEvent::Output(PtyOutput::Bytes(b"A".to_vec())));

        app.dispatch_command(Command::SplitVertical);
        let second = app.workspace.active_pane();
        assert_ne!(first, second);
        assert_eq!(app.workspace.panes().len(), 2);
        app.handle_pty_event(PtyWorkerEvent::Output(PtyOutput::Bytes(b"B".to_vec())));
        assert_eq!(app.terminal.screen().cell(0, 0).unwrap().character(), 'B');

        app.dispatch_command(Command::PreviousPane);
        assert_eq!(app.active_runtime_pane, first);
        assert_eq!(app.terminal.screen().cell(0, 0).unwrap().character(), 'A');
        app.dispatch_command(Command::NextPane);
        assert_eq!(app.active_runtime_pane, second);
        app.dispatch_command(Command::ClosePane);
        assert_eq!(app.active_runtime_pane, first);
        assert_eq!(app.workspace.panes().len(), 1);
        app.dispatch_command(Command::ClosePane);
        assert_eq!(app.workspace.panes().len(), 1);
    }

    #[test]
    fn pane_zoom_command_keeps_runtime_and_background_output() {
        let mut app = Application::default();
        let first = app.workspace.active_pane();
        app.handle_pty_event(PtyWorkerEvent::Output(PtyOutput::Bytes(b"A".to_vec())));
        app.dispatch_command(Command::SplitVertical);
        let second = app.workspace.active_pane();
        app.handle_pty_event(PtyWorkerEvent::Output(PtyOutput::Bytes(b"C".to_vec())));
        let layout = app.workspace.active_tab().layout.clone();
        let panes = app.workspace.panes();
        app.dispatch_command(Command::TogglePaneZoom);
        assert!(app.workspace.active_tab().is_zoomed());
        assert_eq!(app.active_runtime_pane, second);
        assert_eq!(app.workspace.active_tab().layout, layout);
        assert_eq!(app.workspace.panes(), panes);
        assert_eq!(app.terminal.screen().cell(0, 0).unwrap().character(), 'C');

        super::handle_background_pty_event(
            app.inactive_panes.get_mut(&first).unwrap(),
            PtyWorkerEvent::Output(PtyOutput::Bytes(b"B".to_vec())),
        );
        app.dispatch_command(Command::PreviousPane);
        assert_eq!(app.active_runtime_pane, first);
        assert_eq!(app.terminal.screen().cell(0, 0).unwrap().character(), 'A');
        assert_eq!(app.terminal.screen().cell(0, 1).unwrap().character(), 'B');
        app.dispatch_command(Command::NextPane);
        assert_eq!(app.active_runtime_pane, second);
        assert_eq!(app.terminal.screen().cell(0, 0).unwrap().character(), 'C');
        app.dispatch_command(Command::TogglePaneZoom);
        assert!(!app.workspace.active_tab().is_zoomed());
        assert_eq!(app.workspace.active_tab().layout, layout);
        assert_eq!(app.workspace.panes(), panes);
        assert_pane_runtimes_match_workspace(&app);
    }

    #[test]
    fn consecutive_pty_chunks_can_share_one_requested_frame() {
        let mut app = Application::default();
        assert!(app.handle_pty_event(PtyWorkerEvent::Output(PtyOutput::Bytes(b"A".to_vec()))));
        assert!(app.handle_pty_event(PtyWorkerEvent::Output(PtyOutput::Bytes(b"B".to_vec()))));
        assert!(!app.frame.redraw_requested);
        app.invalidate_frame();
        assert_eq!(app.frame.requests_armed, 1);
        assert_eq!(app.terminal.screen().cell(0, 0).unwrap().character(), 'A');
        assert_eq!(app.terminal.screen().cell(0, 1).unwrap().character(), 'B');
    }

    #[test]
    fn app_only_command_keeps_scrollback_position() {
        let mut app = Application::default();
        app.terminal.resize(TerminalDimensions::new(3, 2).unwrap());
        app.terminal.set_cursor_position(1, 0).unwrap();
        app.terminal.index();
        app.dispatch_command(Command::PageUp);
        assert_eq!(app.terminal.viewport_offset(), 1);

        app.dispatch_command(Command::Copy);
        assert_eq!(app.terminal.viewport_offset(), 1);
        assert!(!app.write_terminal_input(b"x".to_vec()));
        assert_eq!(app.terminal.viewport_offset(), 1);
    }

    #[test]
    fn page_up_then_terminal_keys_return_to_live_bottom() {
        let mut app = Application::default();
        app.terminal.resize(TerminalDimensions::new(3, 2).unwrap());
        app.terminal.set_cursor_position(1, 0).unwrap();
        app.terminal.index();

        let inputs = [
            terminal_key_input(
                Some("x"),
                None,
                PhysicalKey::Code(KeyCode::KeyX),
                ModifiersState::empty(),
            )
            .unwrap(),
            terminal_key_input(
                None,
                Some(BasicKey::Enter),
                PhysicalKey::Code(KeyCode::Enter),
                ModifiersState::empty(),
            )
            .unwrap(),
            terminal_key_input(
                Some("c"),
                None,
                PhysicalKey::Code(KeyCode::KeyC),
                ModifiersState::CONTROL,
            )
            .unwrap(),
            terminal_key_input(
                None,
                Some(BasicKey::Backspace),
                PhysicalKey::Code(KeyCode::Backspace),
                ModifiersState::empty(),
            )
            .unwrap(),
            terminal_core::encode_cursor_key(*app.terminal.input_modes(), CursorKey::Left).to_vec(),
        ];
        assert_eq!(inputs[2], vec![0x03]);

        for bytes in inputs {
            app.dispatch_command(Command::PageUp);
            assert_eq!(app.terminal.viewport_offset(), 1);
            let expected = bytes.clone();
            let (queued, changed) = queue_terminal_input(&mut app.terminal, bytes, |written| {
                assert_eq!(written, expected);
                true
            });
            assert!(queued);
            assert!(changed);
            assert_eq!(app.terminal.viewport_offset(), 0);
        }
    }

    #[test]
    fn nested_panes_receive_distinct_grid_sizes_and_hidden_tabs_stay_sized() {
        let mut workspace = terminal_workspace::Workspace::default();
        let left = workspace.active_pane();
        let top_right = workspace.split_active(SplitAxis::Vertical);
        let bottom_right = workspace.split_active(SplitAxis::Horizontal);
        let other_tab = workspace.new_tab();
        let sizes = pane_dimensions(
            &workspace,
            PhysicalSize::new(101, 51),
            CellMetrics::from_physical(10, 10, 10.0),
        );
        assert_eq!(
            sizes,
            vec![
                (left, TerminalDimensions::new(5, 5).unwrap()),
                (top_right, TerminalDimensions::new(5, 2).unwrap()),
                (bottom_right, TerminalDimensions::new(5, 2).unwrap()),
                (other_tab, TerminalDimensions::new(10, 5).unwrap()),
            ]
        );
    }

    #[test]
    fn directional_resize_changes_pane_grid_dimensions_without_replacing_sessions() {
        let mut workspace = terminal_workspace::Workspace::default();
        let first = workspace.active_pane();
        let second = workspace.split_active(SplitAxis::Vertical);
        let panes = workspace.panes();
        let size = PhysicalSize::new(400, 200);
        let metrics = CellMetrics::from_physical(10, 10, 10.0);
        assert_eq!(
            pane_dimensions(&workspace, size, metrics),
            vec![
                (first, TerminalDimensions::new(20, 20).unwrap()),
                (second, TerminalDimensions::new(20, 20).unwrap()),
            ]
        );
        assert!(workspace.resize_focused_pane(
            PaneDirection::Left,
            PaneRect {
                x: 0,
                y: 0,
                width: 400,
                height: 200
            },
            20,
            100,
        ));
        assert_eq!(
            pane_dimensions(&workspace, size, metrics),
            vec![
                (first, TerminalDimensions::new(18, 20).unwrap()),
                (second, TerminalDimensions::new(22, 20).unwrap()),
            ]
        );
        assert!(workspace.resize_focused_pane(
            PaneDirection::Right,
            PaneRect {
                x: 0,
                y: 0,
                width: 400,
                height: 200
            },
            20,
            100,
        ));
        assert_eq!(
            pane_dimensions(&workspace, size, metrics),
            vec![
                (first, TerminalDimensions::new(20, 20).unwrap()),
                (second, TerminalDimensions::new(20, 20).unwrap()),
            ]
        );
        assert_eq!(workspace.panes(), panes);
    }

    #[test]
    fn new_tab_keeps_its_own_terminal_session() {
        let mut app = Application::default();
        let first = app.workspace.active_pane();
        app.dispatch_command(Command::NewTab);
        let second = app.workspace.active_pane();
        assert_ne!(first, second);
        assert_eq!(app.workspace.tabs().len(), 2);
        app.dispatch_command(Command::PreviousTab);
        assert_eq!(app.active_runtime_pane, first);
        app.dispatch_command(Command::NextTab);
        assert_eq!(app.active_runtime_pane, second);
        assert_eq!(app.workspace.definition().tabs.len(), 2);
    }

    fn assert_pane_runtimes_match_workspace(app: &Application) {
        let panes = app.workspace.panes();
        assert_eq!(app.active_runtime_pane, app.workspace.active_pane());
        assert_eq!(app.inactive_panes.len() + 1, panes.len());
        let mut sessions = std::collections::HashSet::new();
        for pane in panes {
            assert!(sessions.insert(pane.session));
            assert_eq!(
                pane.id == app.active_runtime_pane,
                !app.inactive_panes.contains_key(&pane.id)
            );
        }
    }

    #[test]
    fn palette_creates_second_tab_after_a_pending_redraw() {
        let mut app = Application::default();
        let first = app.workspace.active_pane();
        app.dispatch_command(Command::OpenPalette);
        app.handle_palette_key(&Key::Character("new tab".into()), Some("new tab"));
        assert!(app.frame.redraw_requested);
        assert_eq!(app.frame.requests_armed, 1);
        app.handle_palette_key(&Key::Named(NamedKey::Enter), None);

        assert!(app.palette.is_none());
        assert_eq!(app.workspace.tabs().len(), 2);
        assert_ne!(app.workspace.active_pane(), first);
        assert!(app.frame.redraw_requested);
        assert_eq!(app.frame.requests_armed, 2);
        assert_pane_runtimes_match_workspace(&app);
    }

    fn assert_split_creation(query: &str, axis: SplitAxis) {
        let mut app = Application::default();
        let first = app.workspace.active_pane();
        app.dispatch_command(Command::OpenPalette);
        app.handle_palette_key(&Key::Character(query.into()), Some(query));
        assert_eq!(app.frame.requests_armed, 1);
        app.handle_palette_key(&Key::Named(NamedKey::Enter), None);
        let second = app.workspace.active_pane();

        assert!(app.palette.is_none());
        assert_eq!(app.frame.requests_armed, 2);
        assert_ne!(second, first);
        assert_eq!(app.workspace.tabs().len(), 1);
        assert!(matches!(
            app.workspace.definition().tabs[0].layout,
            LayoutDefinition::Split { axis: actual, .. } if actual == axis
        ));
        assert_pane_runtimes_match_workspace(&app);
        app.dispatch_command(Command::PreviousPane);
        assert_eq!(app.active_runtime_pane, first);
        app.dispatch_command(Command::NextPane);
        assert_eq!(app.active_runtime_pane, second);
    }

    #[test]
    fn horizontal_split_creates_an_independent_pane() {
        assert_split_creation("split horizontal", SplitAxis::Horizontal);
    }

    #[test]
    fn vertical_split_creates_an_independent_pane() {
        assert_split_creation("split vertical", SplitAxis::Vertical);
    }

    #[test]
    fn sequential_creations_keep_every_runtime_and_session_distinct() {
        let mut app = Application::default();
        for command in [
            Command::NewTab,
            Command::SplitHorizontal,
            Command::SplitVertical,
            Command::NewTab,
            Command::SplitHorizontal,
        ] {
            app.dispatch_command(command);
            assert_pane_runtimes_match_workspace(&app);
        }
        assert_eq!(app.workspace.tabs().len(), 3);
        assert_eq!(app.workspace.panes().len(), 6);
        app.dispatch_command(Command::PreviousTab);
        assert_pane_runtimes_match_workspace(&app);
        app.dispatch_command(Command::NextPane);
        assert_pane_runtimes_match_workspace(&app);
    }

    #[test]
    fn close_after_create_releases_panes_and_keeps_remaining_runtime() {
        let mut app = Application::default();
        let first = app.workspace.active_pane();
        app.handle_pty_event(PtyWorkerEvent::Output(PtyOutput::Bytes(b"A".to_vec())));

        app.dispatch_command(Command::SplitHorizontal);
        let split = app.workspace.active_pane();
        app.dispatch_command(Command::ClosePane);
        assert_eq!(app.workspace.active_pane(), first);
        assert!(!app.inactive_panes.contains_key(&split));
        assert_eq!(app.terminal.screen().cell(0, 0).unwrap().character(), 'A');
        assert_pane_runtimes_match_workspace(&app);

        app.dispatch_command(Command::NewTab);
        let tab = app.workspace.active_pane();
        app.dispatch_command(Command::ClosePane);
        assert_eq!(app.workspace.active_pane(), first);
        assert!(!app.inactive_panes.contains_key(&tab));
        assert_eq!(app.workspace.tabs().len(), 1);
        assert_pane_runtimes_match_workspace(&app);

        app.dispatch_command(Command::SplitVertical);
        assert_pane_runtimes_match_workspace(&app);
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
    fn ctrl_c_encodes_etx_instead_of_committed_text() {
        assert_eq!(
            terminal_key_input(
                Some("c"),
                None,
                PhysicalKey::Code(KeyCode::KeyC),
                ModifiersState::CONTROL,
            ),
            Some(vec![0x03])
        );
        assert_eq!(
            terminal_key_input(
                None,
                None,
                PhysicalKey::Code(KeyCode::KeyC),
                ModifiersState::CONTROL,
            ),
            Some(vec![0x03])
        );
    }

    #[test]
    fn control_letters_encode_ascii_control_bytes_without_committed_text() {
        for (key, text, byte) in [
            (KeyCode::KeyA, Some("a"), 0x01),
            (KeyCode::KeyB, None, 0x02),
            (KeyCode::KeyF, Some("f"), 0x06),
            (KeyCode::KeyR, Some("r"), 0x12),
            (KeyCode::KeyW, Some("w"), 0x17),
            (KeyCode::KeyZ, Some("z"), 0x1a),
        ] {
            assert_eq!(
                terminal_key_input(text, None, PhysicalKey::Code(key), ModifiersState::CONTROL),
                Some(vec![byte]),
                "{key:?}"
            );
        }
    }

    #[test]
    fn every_control_letter_maps_to_its_ascii_byte() {
        let letters = [
            KeyCode::KeyA,
            KeyCode::KeyB,
            KeyCode::KeyC,
            KeyCode::KeyD,
            KeyCode::KeyE,
            KeyCode::KeyF,
            KeyCode::KeyG,
            KeyCode::KeyH,
            KeyCode::KeyI,
            KeyCode::KeyJ,
            KeyCode::KeyK,
            KeyCode::KeyL,
            KeyCode::KeyM,
            KeyCode::KeyN,
            KeyCode::KeyO,
            KeyCode::KeyP,
            KeyCode::KeyQ,
            KeyCode::KeyR,
            KeyCode::KeyS,
            KeyCode::KeyT,
            KeyCode::KeyU,
            KeyCode::KeyV,
            KeyCode::KeyW,
            KeyCode::KeyX,
            KeyCode::KeyY,
            KeyCode::KeyZ,
        ];
        for (index, key) in letters.into_iter().enumerate() {
            assert_eq!(
                terminal_key_input(None, None, PhysicalKey::Code(key), ModifiersState::CONTROL),
                Some(vec![index as u8 + 1]),
                "{key:?}"
            );
        }
    }

    #[test]
    fn plain_letters_and_nonletter_keys_keep_their_input() {
        assert_eq!(
            terminal_key_input(
                Some("c"),
                None,
                PhysicalKey::Code(KeyCode::KeyC),
                ModifiersState::empty(),
            ),
            Some(b"c".to_vec())
        );
        assert_eq!(
            terminal_key_input(
                Some("x"),
                None,
                PhysicalKey::Code(KeyCode::KeyX),
                ModifiersState::CONTROL,
            ),
            Some(vec![0x18])
        );
        assert_eq!(
            terminal_key_input(
                None,
                Some(BasicKey::Enter),
                PhysicalKey::Code(KeyCode::Enter),
                ModifiersState::CONTROL,
            ),
            Some(vec![b'\r'])
        );
    }

    #[test]
    fn copy_shortcut_and_configured_ctrl_c_keep_command_precedence() {
        let copy = ModifiersState::CONTROL | ModifiersState::SHIFT;
        assert_eq!(
            configured_command(&Config::default(), PhysicalKey::Code(KeyCode::KeyC), copy),
            Some(Command::Copy)
        );
        assert_eq!(
            terminal_key_input(Some("C"), None, PhysicalKey::Code(KeyCode::KeyC), copy),
            Some(b"C".to_vec())
        );
        let config = Config::parse("[[bindings]]\nkey = 'Ctrl+C'\naction = 'copy'").unwrap();
        assert_eq!(
            configured_command(
                &config,
                PhysicalKey::Code(KeyCode::KeyC),
                ModifiersState::CONTROL,
            ),
            Some(Command::Copy)
        );
        let config = Config::parse("[[bindings]]\nkey = 'Ctrl+B'\ncommand = 'copy'").unwrap();
        assert_eq!(
            configured_command(
                &config,
                PhysicalKey::Code(KeyCode::KeyB),
                ModifiersState::CONTROL,
            ),
            Some(Command::Copy)
        );
    }

    #[test]
    fn named_backspace_takes_precedence_over_committed_control_text() {
        assert_eq!(
            basic_key_input(Some("\u{17}"), Some(BasicKey::Backspace)),
            Some(vec![basic_backspace_byte_for_platform(cfg!(windows))])
        );
    }

    #[test]
    fn configured_backspace_binding_precedes_unbound_terminal_input() {
        let config =
            Config::parse("[[bindings]]\nkey = 'Ctrl+Shift+Backspace'\ncommand = 'close_pane'")
                .unwrap();
        let backspace = PhysicalKey::Code(KeyCode::Backspace);
        assert_eq!(
            configured_command(
                &config,
                backspace,
                ModifiersState::CONTROL | ModifiersState::SHIFT,
            ),
            Some(Command::ClosePane)
        );
        assert_eq!(
            configured_command(&config, backspace, ModifiersState::empty()),
            None
        );
        assert_eq!(
            terminal_key_input(
                None,
                Some(BasicKey::Backspace),
                backspace,
                ModifiersState::empty(),
            ),
            Some(vec![basic_backspace_byte_for_platform(cfg!(windows))])
        );
    }

    #[test]
    fn page_navigation_is_app_local_and_not_basic_pty_input() {
        assert_eq!(
            configured_command(
                &Config::default(),
                PhysicalKey::Code(KeyCode::PageUp),
                ModifiersState::empty()
            ),
            Some(Command::PageUp)
        );
        assert_eq!(
            configured_command(
                &Config::default(),
                PhysicalKey::Code(KeyCode::PageDown),
                ModifiersState::empty()
            ),
            Some(Command::PageDown)
        );
        assert_eq!(basic_key_input(None, None), None);
    }

    #[test]
    fn configured_bindings_replace_defaults_and_require_exact_modifiers() {
        let config =
            Config::parse("[[bindings]]\nkey = 'Alt+PageUp'\naction = 'page_down'").unwrap();
        assert_eq!(
            configured_command(
                &config,
                PhysicalKey::Code(KeyCode::PageUp),
                ModifiersState::ALT
            ),
            Some(Command::PageDown)
        );
        assert_eq!(
            configured_command(
                &config,
                PhysicalKey::Code(KeyCode::PageUp),
                ModifiersState::empty()
            ),
            None
        );
        assert_eq!(
            configured_command(
                &config,
                PhysicalKey::Code(KeyCode::PageUp),
                ModifiersState::ALT | ModifiersState::SHIFT
            ),
            None
        );
    }

    #[test]
    fn binding_and_palette_share_the_page_command_dispatcher() {
        let mut app = Application {
            terminal: TerminalState::new(TerminalDimensions::new(2, 2).unwrap()),
            ..Application::default()
        };
        app.terminal.set_cursor_position(1, 0).unwrap();
        app.terminal.index();
        let bound = configured_command(
            &app.config,
            PhysicalKey::Code(KeyCode::PageUp),
            ModifiersState::empty(),
        )
        .unwrap();
        app.dispatch_command(bound);
        assert_eq!(app.terminal.viewport_offset(), 1);

        app.dispatch_command(Command::OpenPalette);
        assert!(app.palette.is_some());
        app.handle_palette_key(&Key::Character("page down".into()), Some("page down"));
        assert_eq!(
            app.palette.as_ref().unwrap().chosen(),
            Some(PaletteAction::Command(Command::PageDown))
        );
        app.handle_palette_key(&Key::Named(NamedKey::Enter), None);
        assert!(app.palette.is_none());
        assert_eq!(app.terminal.viewport_offset(), 0);
    }

    #[test]
    fn palette_renames_tabs_and_custom_titles_survive_tab_switches() {
        let mut app = Application::default();
        app.dispatch_command(Command::OpenPalette);
        app.handle_palette_key(&Key::Character("rename tab".into()), Some("rename tab"));
        assert_eq!(
            app.palette.as_ref().unwrap().chosen(),
            Some(PaletteAction::Command(Command::RenameTab))
        );
        app.handle_palette_key(&Key::Named(NamedKey::Enter), None);
        assert!(app.palette.is_none());
        assert_eq!(app.tab_rename.as_deref(), Some(""));

        app.handle_tab_rename_key(&Key::Character("First".into()), Some("First"));
        app.handle_tab_rename_key(&Key::Named(NamedKey::Enter), None);
        assert_eq!(app.workspace.active_tab().display_title(), "First");

        app.dispatch_command(Command::NewTab);
        app.dispatch_command(Command::RenameTab);
        app.handle_tab_rename_key(&Key::Character("Second".into()), Some("Second"));
        app.handle_tab_rename_key(&Key::Named(NamedKey::Enter), None);
        assert_eq!(app.workspace.active_tab().display_title(), "Second");

        app.dispatch_command(Command::PreviousTab);
        assert_eq!(app.workspace.active_tab().display_title(), "First");
        app.dispatch_command(Command::NextTab);
        assert_eq!(app.workspace.active_tab().display_title(), "Second");
    }

    #[test]
    fn shell_metadata_follows_panes_and_custom_tab_titles_take_precedence() {
        let mut app = Application::default();
        let first = app.workspace.active_pane();
        app.handle_pty_event(PtyWorkerEvent::Output(PtyOutput::Bytes(
            b"\x1b]2;First shell\x07\x1b]7;file:///first\x07".to_vec(),
        )));
        assert_eq!(app.active_tab_title(), "First shell");
        assert_eq!(app.terminal.working_directory_uri(), Some("file:///first"));

        app.dispatch_command(Command::SplitVertical);
        let second = app.workspace.active_pane();
        assert_ne!(first, second);
        assert_eq!(app.active_tab_title(), "Terminal 1");
        app.handle_pty_event(PtyWorkerEvent::Output(PtyOutput::Bytes(
            b"\x1b]0;Second shell\x07\x1b]7;file:///second\x07".to_vec(),
        )));
        assert_eq!(app.active_tab_title(), "Second shell");
        assert_eq!(app.terminal.working_directory_uri(), Some("file:///second"));

        app.workspace
            .set_active_tab_custom_title(Some("Renamed".into()));
        assert_eq!(app.active_tab_title(), "Renamed");
        app.handle_pty_event(PtyWorkerEvent::Output(PtyOutput::Bytes(
            b"\x1b]2;Updated shell\x07".to_vec(),
        )));
        assert_eq!(app.active_tab_title(), "Renamed");
        app.dispatch_command(Command::PreviousPane);
        assert_eq!(app.active_tab_title(), "Renamed");
        assert_eq!(app.terminal.working_directory_uri(), Some("file:///first"));
        app.workspace.set_active_tab_custom_title(None);
        assert_eq!(app.active_tab_title(), "First shell");
        app.dispatch_command(Command::NextPane);
        assert_eq!(app.active_tab_title(), "Updated shell");

        super::handle_background_pty_event(
            app.inactive_panes.get_mut(&first).unwrap(),
            PtyWorkerEvent::Output(PtyOutput::Bytes(
                b"\x1b]2;Background shell\x07\x1b]7;file:///background\x07".to_vec(),
            )),
        );
        assert_eq!(app.active_tab_title(), "Updated shell");
        app.dispatch_command(Command::PreviousPane);
        assert_eq!(app.active_tab_title(), "Background shell");
        assert_eq!(
            app.terminal.working_directory_uri(),
            Some("file:///background")
        );
        app.dispatch_command(Command::NextPane);

        app.dispatch_command(Command::NewTab);
        assert_eq!(app.active_tab_title(), "Terminal 2");
        app.dispatch_command(Command::PreviousTab);
        assert_eq!(app.active_tab_title(), "Updated shell");
        assert_eq!(app.terminal.working_directory_uri(), Some("file:///second"));
    }

    #[test]
    fn tab_picker_uses_resolved_titles_and_consumes_filter_navigation_and_cancel() {
        let mut app = Application::default();
        app.handle_pty_event(PtyWorkerEvent::Output(PtyOutput::Bytes(
            b"\x1b]2;Shell title\x07".to_vec(),
        )));
        app.dispatch_command(Command::NewTab);
        app.handle_pty_event(PtyWorkerEvent::Output(PtyOutput::Bytes(
            b"\x1b]2;Other shell\x07".to_vec(),
        )));
        app.workspace
            .set_active_tab_custom_title(Some("User title".into()));
        app.dispatch_command(Command::NewTab);
        app.dispatch_command(Command::OpenTabPicker);
        let names: Vec<_> = app
            .tab_picker
            .as_ref()
            .unwrap()
            .matches()
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert_eq!(names, ["Shell title", "User title", "Terminal 3"]);
        app.handle_tab_picker_key(&Key::Character("j".into()), Some("j"));
        assert_eq!(app.tab_picker.as_ref().unwrap().selected(), 1);
        app.handle_tab_picker_key(&Key::Character("k".into()), Some("k"));
        assert_eq!(app.tab_picker.as_ref().unwrap().selected(), 0);
        app.handle_tab_picker_key(&Key::Named(NamedKey::ArrowDown), None);
        app.handle_tab_picker_key(&Key::Named(NamedKey::ArrowUp), None);
        let before = app.terminal.screen().cell(0, 0).unwrap().character();
        app.handle_tab_picker_key(&Key::Character("User".into()), Some("User"));
        assert_eq!(app.tab_picker.as_ref().unwrap().matches().len(), 1);
        app.handle_tab_picker_key(&Key::Named(NamedKey::ArrowUp), None);
        app.handle_tab_picker_key(&Key::Named(NamedKey::Escape), None);
        assert!(app.tab_picker.is_none());
        assert_eq!(
            app.workspace.tabs()[1].custom_title.as_deref(),
            Some("User title")
        );
        assert_eq!(
            app.terminal.screen().cell(0, 0).unwrap().character(),
            before
        );

        app.dispatch_command(Command::OpenTabPicker);
        app.handle_tab_picker_key(&Key::Character("Shell".into()), Some("Shell"));
        app.handle_tab_picker_key(&Key::Named(NamedKey::Enter), None);
        assert_eq!(app.active_tab_title(), "Shell title");
    }

    #[test]
    fn tab_picker_formats_cwd_application_and_fallback_without_changing_renames() {
        let mut app = Application::default();
        app.handle_pty_event(PtyWorkerEvent::Output(PtyOutput::Bytes(
            b"\x1b]7;file:///home/me/backend/src\x07\x1b]2;C:\\Tools\\nvim.exe\x07".to_vec(),
        )));
        app.dispatch_command(Command::NewTab);
        app.handle_pty_event(PtyWorkerEvent::Output(PtyOutput::Bytes(
            b"\x1b]7;file:///work/frontend/app\x07\x1b]2;C:\\Program%20Files\\pwsh.exe\x07"
                .to_vec(),
        )));
        app.workspace
            .set_active_tab_custom_title(Some("  full-stack-workstation  ".into()));
        app.dispatch_command(Command::NewTab);
        app.handle_pty_event(PtyWorkerEvent::Output(PtyOutput::Bytes(
            b"\x1b]7;file:///work/my%20project/src\x07".to_vec(),
        )));
        app.dispatch_command(Command::NewTab);
        app.handle_pty_event(PtyWorkerEvent::Output(PtyOutput::Bytes(
            b"\x1b]2;/usr/bin/nvim\x07".to_vec(),
        )));
        app.dispatch_command(Command::NewTab);
        app.handle_pty_event(PtyWorkerEvent::Output(PtyOutput::Bytes(
            b"\x1b]7;file:///work/frontend/app\x07\x1b]2;C:\\Tools\\pwsh.exe\x07".to_vec(),
        )));
        app.dispatch_command(Command::NewTab);
        app.dispatch_command(Command::OpenTabPicker);
        let names: Vec<_> = app
            .tab_picker
            .as_ref()
            .unwrap()
            .matches()
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "backend/src  nvim",
                "  full-stack-workstation  ",
                "my project/src",
                "nvim",
                "frontend/app  PowerShell",
                "Terminal 6"
            ]
        );
        assert_eq!(
            app.workspace.tabs()[1].custom_title.as_deref(),
            Some("  full-stack-workstation  ")
        );
        assert_eq!(
            super::display_application("C:\\Tools\\pwsh.exe"),
            Some("PowerShell".into())
        );
    }

    #[test]
    fn background_output_marks_tab_and_focus_clears_activity() {
        let mut app = Application::default();
        let first_pane = app.workspace.active_pane();
        let first_tab = app.workspace.active_tab().id;
        app.dispatch_command(Command::NewTab);
        app.record_background_output(first_pane);
        assert!(app.unseen_tab_activity.contains(&first_tab));
        app.dispatch_command(Command::OpenTabPicker);
        assert!(
            app.tab_picker_overlay().unwrap().lines[1]
                .text
                .contains("| Terminal 1")
        );
        app.dispatch_command(Command::PreviousTab);
        assert!(!app.unseen_tab_activity.contains(&first_tab));
        app.record_background_output(first_pane);
        app.handle_pty_event(PtyWorkerEvent::Output(PtyOutput::Bytes(b"active".to_vec())));
        assert!(app.unseen_tab_activity.is_empty());
    }

    #[test]
    fn activity_animation_runs_only_while_picker_is_open() {
        let mut app = Application::default();
        let first_pane = app.workspace.active_pane();
        app.dispatch_command(Command::NewTab);
        app.frame.begin_redraw();
        let requests_before_output = app.frame.requests_armed;
        app.record_background_output(first_pane);
        assert_eq!(app.frame.requests_armed, requests_before_output);
        let now = Instant::now();
        assert_eq!(app.tick_tab_activity(now), None);
        assert_eq!(app.activity_frame, 0);
        app.dispatch_command(Command::OpenTabPicker);
        let deadline = app.tick_tab_activity(now).unwrap();
        assert_eq!(deadline, now + Duration::from_millis(300));
        assert_eq!(
            app.tick_tab_activity(deadline).unwrap(),
            deadline + Duration::from_millis(300)
        );
        assert_eq!(app.activity_frame, 1);
        assert!(
            app.tab_picker_overlay().unwrap().lines[1]
                .text
                .contains("/ Terminal 1")
        );
        app.handle_tab_picker_key(&Key::Named(NamedKey::Escape), None);
        assert_eq!(
            app.tick_tab_activity(deadline + Duration::from_secs(1)),
            None
        );
        assert_eq!(app.activity_frame, 1);
        assert!(app.activity_deadline.is_none());
    }

    #[test]
    fn palette_consumes_input_and_escape_closes_it() {
        let mut app = Application::default();
        app.dispatch_command(Command::OpenPalette);
        app.handle_palette_key(&Key::Character("paste".into()), Some("paste"));
        assert_eq!(
            app.palette.as_ref().unwrap().chosen(),
            Some(PaletteAction::Command(Command::Paste))
        );
        assert!(
            app.palette_overlay()
                .unwrap()
                .lines
                .iter()
                .any(|line| line.selected)
        );
        app.handle_palette_key(&Key::Named(NamedKey::Escape), None);
        assert!(app.palette.is_none());
    }

    #[test]
    fn search_keys_edit_navigate_and_close_without_terminal_input() {
        let mut app = Application {
            terminal: TerminalState::new(TerminalDimensions::new(12, 2).unwrap()),
            ..Application::default()
        };
        for line in ["find", "other", "find"] {
            for character in line.chars() {
                app.terminal.print_character(character).unwrap();
            }
            app.terminal.carriage_return();
            app.terminal.index();
        }
        app.search = Some(Search::default());
        app.handle_search_key(&Key::Character("find".into()), Some("find"));
        assert_eq!(app.search.as_ref().unwrap().count(), 2);
        let first = app.search.as_ref().unwrap().position();
        app.modifiers = ModifiersState::SHIFT;
        app.handle_search_key(&Key::Named(NamedKey::Enter), None);
        assert_ne!(app.search.as_ref().unwrap().position(), first);
        app.modifiers = ModifiersState::empty();
        app.handle_search_key(&Key::Named(NamedKey::Enter), None);
        assert_eq!(app.search.as_ref().unwrap().position(), first);
        app.handle_search_key(&Key::Named(NamedKey::Backspace), None);
        assert_eq!(app.search.as_ref().unwrap().query(), "fin");
        let overlay = app.search_overlay().unwrap();
        assert!(overlay.lines[0].text.contains("[2/2]"));
        assert!(!overlay.search_matches.is_empty());
        app.handle_search_key(&Key::Named(NamedKey::Escape), None);
        assert!(app.search.is_none());
        assert_eq!(app.terminal.screen().cell(0, 0).unwrap().character(), 'f');
    }

    #[test]
    fn reload_keeps_last_valid_config_and_restores_defaults_when_removed() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "terminal-config-{}-{nonce}.toml",
            std::process::id()
        ));
        let mut app = Application {
            config_path: path.clone(),
            ..Application::default()
        };
        fs::write(&path, "[theme]\nbackground = '#123456'").unwrap();
        app.reload_config();
        assert_eq!(app.config.theme.background, Rgb(0x12, 0x34, 0x56));

        fs::write(&path, "[theme]\nbackground = 'invalid'").unwrap();
        app.reload_config();
        assert_eq!(app.config.theme.background, Rgb(0x12, 0x34, 0x56));

        fs::remove_file(path).unwrap();
        app.reload_config();
        assert_eq!(app.config, Config::default());
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
    fn control_left_and_right_encode_for_pty_after_command_dispatch() {
        let (mut parser, mut terminal) = (
            TerminalParser::new(),
            TerminalState::new(TerminalDimensions::new(80, 24).unwrap()),
        );
        for application_mode in [false, true] {
            if application_mode {
                parser.advance(&mut terminal, b"\x1b[?1h").unwrap();
            }
            let modes = *terminal.input_modes();
            let plain_left = if application_mode {
                b"\x1bOD"
            } else {
                b"\x1b[D"
            };
            let plain_right = if application_mode {
                b"\x1bOC"
            } else {
                b"\x1b[C"
            };
            assert_eq!(
                terminal_cursor_key_input(modes, CursorKey::Left, ModifiersState::empty()),
                plain_left
            );
            assert_eq!(
                terminal_cursor_key_input(modes, CursorKey::Right, ModifiersState::empty()),
                plain_right
            );
            assert_eq!(
                terminal_cursor_key_input(modes, CursorKey::Left, ModifiersState::CONTROL),
                b"\x1b[1;5D"
            );
            assert_eq!(
                terminal_cursor_key_input(modes, CursorKey::Right, ModifiersState::CONTROL),
                b"\x1b[1;5C"
            );
            assert_eq!(
                terminal_cursor_key_input(
                    modes,
                    CursorKey::Left,
                    ModifiersState::CONTROL | ModifiersState::SHIFT
                ),
                plain_left
            );
        }

        let defaults = Config::default();
        assert_eq!(
            configured_command(
                &defaults,
                PhysicalKey::Code(KeyCode::ArrowRight),
                ModifiersState::CONTROL | ModifiersState::SHIFT
            ),
            Some(Command::NextPane)
        );
        assert_eq!(
            configured_command(
                &defaults,
                PhysicalKey::Code(KeyCode::ArrowLeft),
                ModifiersState::CONTROL | ModifiersState::SHIFT
            ),
            Some(Command::PreviousPane)
        );
        for key in [KeyCode::ArrowLeft, KeyCode::ArrowRight] {
            assert_eq!(
                configured_command(&defaults, PhysicalKey::Code(key), ModifiersState::CONTROL),
                None
            );
        }
        for (key, command) in [
            (KeyCode::KeyH, Command::FocusPaneLeft),
            (KeyCode::KeyL, Command::FocusPaneRight),
            (KeyCode::KeyK, Command::FocusPaneUp),
            (KeyCode::KeyJ, Command::FocusPaneDown),
        ] {
            assert_eq!(
                configured_command(
                    &defaults,
                    PhysicalKey::Code(key),
                    ModifiersState::CONTROL | ModifiersState::SHIFT
                ),
                Some(command)
            );
        }
        let custom =
            Config::parse("[[bindings]]\nkey = 'Ctrl+ArrowLeft'\naction = 'copy'").unwrap();
        assert_eq!(
            configured_command(
                &custom,
                PhysicalKey::Code(KeyCode::ArrowLeft),
                ModifiersState::CONTROL
            ),
            Some(Command::Copy)
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
    fn target_hit_testing_uses_the_scrolled_viewport_and_cell_metrics() {
        let mut terminal = TerminalState::new(TerminalDimensions::new(2, 2).unwrap());
        let mut parser = TerminalParser::new();
        parser
            .advance(
                &mut terminal,
                b"\x1b]8;;https://example.org\x07A\x1b]8;;\x07\r\n\r\n",
            )
            .unwrap();
        let metrics = CellMetrics::from_physical(10, 20, 12.0);
        assert_eq!(
            target_at_pointer(&terminal, PhysicalPosition::new(5.0, 5.0), metrics),
            None
        );
        terminal.page_up();
        assert_eq!(
            target_at_pointer(&terminal, PhysicalPosition::new(5.0, 5.0), metrics),
            Some("https://example.org")
        );
        assert_eq!(
            target_at_pointer(&terminal, PhysicalPosition::new(15.0, 5.0), metrics),
            None
        );
        assert_eq!(
            target_at_pointer(&terminal, PhysicalPosition::new(25.0, 5.0), metrics),
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

    fn terminal_with_history_selection() -> TerminalState {
        let mut terminal = TerminalState::new(TerminalDimensions::new(3, 2).unwrap());
        for (index, word) in ["one", "two", "tri", "for"].into_iter().enumerate() {
            terminal.set_cursor_position(1, 0).unwrap();
            if index > 0 {
                terminal.index();
            }
            for character in word.chars() {
                terminal.print_character(character).unwrap();
            }
        }
        assert!(terminal.scroll_viewport_rows(1));
        assert!(terminal.begin_selection(0, 0));
        assert!(terminal.extend_selection(1, 2));
        assert_eq!(terminal.selected_text().as_deref(), Some("two\ntri"));
        terminal
    }

    #[test]
    fn wheel_scroll_preserves_selected_history_coordinates() {
        let mut terminal = terminal_with_history_selection();
        let mut remainder = 0.0;
        assert!(scroll_terminal_for_wheel(
            &mut terminal,
            MouseScrollDelta::LineDelta(0.0, 1.0),
            20,
            &mut remainder,
        ));
        assert_eq!(terminal.viewport_offset(), 3);
        assert_eq!(terminal.selected_text().as_deref(), Some("two\ntri"));
        assert!(scroll_terminal_for_wheel(
            &mut terminal,
            MouseScrollDelta::LineDelta(0.0, -1.0),
            20,
            &mut remainder,
        ));
        assert_eq!(terminal.viewport_offset(), 0);
        assert_eq!(terminal.selected_text().as_deref(), Some("two\ntri"));
        assert!(terminal.is_selected(0, 0));
    }

    #[test]
    fn scrollbar_thumb_drag_preserves_selection() {
        let mut app = Application {
            terminal: terminal_with_history_selection(),
            ..Application::default()
        };
        let geometry = ScrollbarGeometry::new(
            ScrollbarRenderData {
                history_rows: app.terminal.scrollback_len(),
                visible_rows: app.terminal.dimensions().rows(),
                viewport_offset: app.terminal.viewport_offset(),
            },
            SurfaceSize::new(100, 40).unwrap(),
            20.0,
        )
        .unwrap();
        app.scroll_to_offset(geometry.offset_for_drag(-100.0, 4.0));
        assert_eq!(app.terminal.viewport_offset(), 3);
        assert_eq!(app.terminal.selected_text().as_deref(), Some("two\ntri"));
        app.scroll_to_offset(geometry.offset_for_drag(1000.0, 4.0));
        assert_eq!(app.terminal.viewport_offset(), 0);
        assert_eq!(app.terminal.selected_text().as_deref(), Some("two\ntri"));
    }

    #[test]
    fn scrollbar_track_click_preserves_selection() {
        let mut app = Application {
            terminal: terminal_with_history_selection(),
            ..Application::default()
        };
        app.page_scrollbar(ScrollbarHit::TrackAbove);
        assert_eq!(app.terminal.viewport_offset(), 3);
        assert_eq!(app.terminal.selected_text().as_deref(), Some("two\ntri"));
        app.page_scrollbar(ScrollbarHit::TrackBelow);
        assert_eq!(app.terminal.viewport_offset(), 1);
        assert_eq!(app.terminal.selected_text().as_deref(), Some("two\ntri"));
        assert!(app.terminal.is_selected(0, 0));
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
