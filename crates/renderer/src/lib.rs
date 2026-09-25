//! Project-owned GPU surface lifecycle.
//!
//! This crate owns GPU setup and terminal-state rendering, but never terminal
//! protocol semantics.

mod font;
mod gpu;
mod scrollbar;
mod snapshot;

pub use scrollbar::{ScrollbarGeometry, ScrollbarHit};

pub use font::{
    CellMetrics, FontProcessingError, FontRequest, GlyphBitmap, ShapedGlyph, ShapedText,
};
pub use snapshot::{
    CursorRenderData, OverlayLine, RenderCell, RenderText, RenderTheme, Rgba, ScrollbarRenderData,
    SearchHighlight, SearchMarker, TerminalRenderData, TextOverlay,
};

use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use winit::window::Window;

use crate::font::FontSystem;
use crate::gpu::{DrawResources, FrameContext, PaneDraw};

static DIAGNOSTICS_ENABLED: OnceLock<bool> = OnceLock::new();
static DIAGNOSTIC_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static DIAGNOSTIC_FRAME_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Whether native lifecycle diagnostics were explicitly enabled for this process.
pub fn diagnostics_enabled() -> bool {
    *DIAGNOSTICS_ENABLED.get_or_init(|| {
        std::env::var_os("TERMINAL_RENDERER_DIAGNOSTICS").is_some_and(|value| value == "1")
    })
}

/// Emits one ordered, opt-in native lifecycle diagnostic line.
///
/// This is intentionally renderer-owned so app and GPU lifecycle observations
/// share one sequence number without exposing `wgpu` details to the app.
pub fn emit_diagnostic(message: fmt::Arguments<'_>) {
    if diagnostics_enabled() {
        let sequence = DIAGNOSTIC_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        eprintln!("terminal-diagnostic seq={sequence} {message}");
    }
}

/// A validated physical surface size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfaceSize {
    width: u32,
    height: u32,
}

impl SurfaceSize {
    /// Returns a size suitable for surface configuration, or `None` when minimized.
    pub fn new(width: u32, height: u32) -> Option<Self> {
        (width != 0 && height != 0).then_some(Self { width, height })
    }

    pub fn width(self) -> u32 {
        self.width
    }

    pub fn height(self) -> u32 {
        self.height
    }
}

/// The outcome of one redraw attempt without exposing `wgpu` types to the app.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RedrawOutcome {
    Presented,
    Reconfigured,
    Skipped,
    Exit,
}

/// A terminal and its physical viewport, supplied by the workspace owner.
pub struct PaneRenderInput<'a> {
    pub terminal: &'a terminal_core::TerminalState,
    pub rect: [u32; 4],
    pub focused: bool,
    pub scrollbar_hover: Option<ScrollbarHit>,
}

/// Renderer-owned surface state safe to include in native lifecycle diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RendererDiagnosticState {
    desired_size: Option<SurfaceSize>,
    configured_size: Option<SurfaceSize>,
}

impl RendererDiagnosticState {
    pub fn desired_size(self) -> Option<SurfaceSize> {
        self.desired_size
    }

    pub fn configured_size(self) -> Option<SurfaceSize> {
        self.configured_size
    }
}

/// A project-owned initialization failure for the native GPU surface.
#[derive(Debug)]
pub struct RendererInitError {
    message: String,
}

impl RendererInitError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for RendererInitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for RendererInitError {}

/// `wgpu` device, queue, and native surface lifecycle owned by the renderer.
pub struct Renderer {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    configuration: Option<wgpu::SurfaceConfiguration>,
    size: Option<SurfaceSize>,
    #[allow(dead_code)]
    font_system: FontSystem,
    logical_font_size: f32,
    cell_metrics: CellMetrics,
    draw_resources: Option<DrawResources>,
    theme: RenderTheme,
}

impl Renderer {
    /// Creates a surface for `window` and configures it when its size is non-zero.
    pub fn new(window: Arc<Window>) -> Result<Self, RendererInitError> {
        Self::new_with_font_request(window, FontRequest::default())
    }

    /// Creates a renderer with an explicitly selected initial terminal font.
    pub fn new_with_font_request(
        window: Arc<Window>,
        font_request: FontRequest,
    ) -> Result<Self, RendererInitError> {
        Self::new_with_font_settings(window, font_request, font::DEFAULT_LOGICAL_FONT_SIZE)
    }

    /// Creates a renderer with an initial font family and logical pixel size.
    pub fn new_with_font_settings(
        window: Arc<Window>,
        font_request: FontRequest,
        logical_font_size: f32,
    ) -> Result<Self, RendererInitError> {
        let font_system = FontSystem::load_system(font_request).map_err(|error| {
            RendererInitError::new(format!("could not load terminal font: {error}"))
        })?;
        let cell_metrics = font_system
            .cell_metrics(window.scale_factor(), logical_font_size)
            .map_err(|error| {
                RendererInitError::new(format!("could not derive terminal cell metrics: {error}"))
            })?;
        let size = SurfaceSize::new(window.inner_size().width, window.inner_size().height);
        let (surface, adapter, device, queue) = initialize_gpu(&window)?;
        let adapter_info = adapter.get_info();
        emit_diagnostic(format_args!(
            "renderer event=adapter backend={:?} name={:?} driver={:?}",
            adapter_info.backend, adapter_info.name, adapter_info.driver
        ));

        let mut renderer = Self {
            window,
            surface,
            adapter,
            device,
            queue,
            configuration: None,
            size,
            font_system,
            logical_font_size,
            cell_metrics,
            draw_resources: None,
            theme: RenderTheme::default(),
        };
        renderer.reconfigure();
        emit_diagnostic(format_args!(
            "renderer event=created scale_factor={} cell_metrics={:?} state={:?}",
            renderer.window.scale_factor(),
            renderer.cell_metrics,
            renderer.diagnostic_state()
        ));
        Ok(renderer)
    }

    /// Updates the desired surface size and configures only non-zero dimensions.
    pub fn resize(&mut self, width: u32, height: u32) -> bool {
        let size = SurfaceSize::new(width, height);
        if !needs_reconfigure(self.size, self.configuration.is_some(), size) {
            emit_diagnostic(format_args!(
                "renderer event=resize-skipped requested={width}x{height} state={:?}",
                self.diagnostic_state()
            ));
            return false;
        }
        emit_diagnostic(format_args!(
            "renderer event=resize requested={width}x{height} previous={:?}",
            self.diagnostic_state()
        ));
        self.size = size;
        let reconfigured = self.reconfigure();
        emit_diagnostic(format_args!(
            "renderer event=resize-complete reconfigured={reconfigured} state={:?}",
            self.diagnostic_state()
        ));
        reconfigured
    }

    /// Returns stable physical cell metrics for the current DPI scale.
    pub fn cell_metrics(&self) -> CellMetrics {
        self.cell_metrics
    }

    /// Returns only project-owned surface state for opt-in lifecycle diagnostics.
    pub fn diagnostic_state(&self) -> RendererDiagnosticState {
        RendererDiagnosticState {
            desired_size: self.size,
            configured_size: self.configuration.as_ref().and_then(|configuration| {
                SurfaceSize::new(configuration.width, configuration.height)
            }),
        }
    }

    /// Updates DPI-derived cell metrics without coupling font size to window size.
    pub fn set_scale_factor(&mut self, scale_factor: f64) -> Result<bool, FontProcessingError> {
        let metrics = self
            .font_system
            .cell_metrics(scale_factor, self.logical_font_size)?;
        if self.cell_metrics == metrics {
            return Ok(false);
        }
        self.cell_metrics = metrics;
        Ok(true)
    }

    /// Updates logical font size and returns whether cell metrics changed.
    pub fn set_font_size(
        &mut self,
        logical_font_size: f32,
        scale_factor: f64,
    ) -> Result<bool, FontProcessingError> {
        let metrics = self
            .font_system
            .cell_metrics(scale_factor, logical_font_size)?;
        let changed = self.cell_metrics != metrics;
        self.logical_font_size = logical_font_size;
        self.cell_metrics = metrics;
        Ok(changed)
    }

    /// Shapes one terminal text run with the selected initial font face.
    pub fn shape_text(
        &mut self,
        text: &str,
        pixels_per_em: f32,
    ) -> Result<ShapedText, FontProcessingError> {
        self.font_system.shape_text(text, pixels_per_em)
    }

    /// Rasterizes one shaped glyph into a CPU-side alpha bitmap.
    pub fn rasterize_glyph(
        &mut self,
        glyph: &ShapedGlyph,
        pixels_per_em: f32,
    ) -> Result<GlyphBitmap, FontProcessingError> {
        self.font_system.rasterize_glyph(glyph, pixels_per_em)
    }

    /// Acquires and presents an empty frame.
    pub fn redraw(&mut self) -> RedrawOutcome {
        self.redraw_data(None)
    }

    /// Converts terminal-core's resolved state and presents it without owning its semantics.
    pub fn redraw_terminal(&mut self, state: &terminal_core::TerminalState) -> RedrawOutcome {
        self.redraw_terminal_with_overlay(state, None)
    }

    pub fn redraw_terminal_with_overlay(
        &mut self,
        state: &terminal_core::TerminalState,
        overlay: Option<&TextOverlay>,
    ) -> RedrawOutcome {
        let projection_start = Instant::now();
        let mut data = TerminalRenderData::from_terminal_with_theme(state, &self.theme);
        if let Some(overlay) = overlay {
            data.apply_text_overlay(overlay, &self.theme);
        }
        emit_diagnostic(format_args!(
            "renderer event=projection cells={} elapsed_us={}",
            data.cells.len(),
            projection_start.elapsed().as_micros()
        ));
        let size = self.window.inner_size();
        self.redraw_data(Some(&[(data, [0, 0, size.width, size.height], false)]))
    }

    pub fn redraw_panes(
        &mut self,
        panes: &[PaneRenderInput<'_>],
        overlay: Option<&TextOverlay>,
    ) -> RedrawOutcome {
        let projection_start = Instant::now();
        let data = project_panes(panes, overlay, &self.theme);
        emit_diagnostic(format_args!(
            "renderer event=pane-projection panes={} cells={} elapsed_us={}",
            data.len(),
            data.iter()
                .map(|(pane, _, _)| pane.cells.len())
                .sum::<usize>(),
            projection_start.elapsed().as_micros()
        ));
        self.redraw_data(Some(&data))
    }

    pub fn set_theme(&mut self, theme: RenderTheme) {
        self.theme = theme;
    }

    fn redraw_data(
        &mut self,
        data: Option<&[(TerminalRenderData, [u32; 4], bool)]>,
    ) -> RedrawOutcome {
        let frame_id = DIAGNOSTIC_FRAME_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        emit_diagnostic(format_args!(
            "renderer frame={frame_id} event=frame-start state={:?}",
            self.diagnostic_state()
        ));
        if self.configuration.is_none() {
            let size = self.window.inner_size();
            emit_diagnostic(format_args!(
                "renderer frame={frame_id} event=frame-unconfigured window_size={}x{} state={:?}",
                size.width,
                size.height,
                self.diagnostic_state()
            ));
            if self.resize(size.width, size.height) {
                emit_diagnostic(format_args!(
                    "renderer frame={frame_id} event=frame-skipped reason=reconfigured"
                ));
                return RedrawOutcome::Reconfigured;
            }
            emit_diagnostic(format_args!(
                "renderer frame={frame_id} event=frame-skipped reason=unconfigured-no-drawable-size"
            ));
            return RedrawOutcome::Skipped;
        }

        let acquire_started = Instant::now();
        let acquired = self.surface.get_current_texture();
        emit_diagnostic(format_args!(
            "renderer frame={frame_id} event=surface-acquire-complete elapsed_us={}",
            acquire_started.elapsed().as_micros()
        ));
        match acquired {
            Ok(frame) => {
                let frame_start = Instant::now();
                let suboptimal = frame.suboptimal;
                emit_diagnostic(format_args!(
                    "renderer frame={frame_id} event=surface-acquire result=ok suboptimal={suboptimal} state={:?}",
                    self.diagnostic_state()
                ));
                if let (Some(data), Some(size), Some(configuration)) =
                    (data, self.size, self.configuration.as_ref())
                {
                    let draw_resources_created = self.draw_resources.is_none();
                    let resources = self.draw_resources.get_or_insert_with(|| {
                        DrawResources::new(&self.device, configuration.format)
                    });
                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    let mut clear_next = true;
                    for (index, (pane_data, rect, focused)) in data.iter().enumerate() {
                        let rect = [
                            rect[0],
                            rect[1],
                            rect[2].min(size.width().saturating_sub(rect[0])),
                            rect[3].min(size.height().saturating_sub(rect[1])),
                        ];
                        if rect[2] == 0 || rect[3] == 0 {
                            continue;
                        }
                        let work = resources.draw_pane(
                            &self.device,
                            &self.queue,
                            FrameContext {
                                target: &view,
                                surface_size: size,
                                cell_metrics: self.cell_metrics,
                            },
                            pane_data,
                            &mut self.font_system,
                            PaneDraw {
                                rect,
                                clear: clear_next,
                                focused_border: *focused,
                            },
                        );
                        clear_next = false;
                        emit_diagnostic(format_args!(
                            "renderer frame={frame_id} event=pane-rendered pane={index} cells={} instances={:?} draw_resources_created={} queue_submissions={}",
                            pane_data.cells.len(),
                            work.instances,
                            draw_resources_created,
                            work.queue_submissions,
                        ));
                    }
                } else {
                    self.queue.submit(std::iter::empty());
                    emit_diagnostic(format_args!(
                        "renderer frame={frame_id} event=frame-rendered terminal_data=false"
                    ));
                }
                self.window.pre_present_notify();
                frame.present();
                emit_diagnostic(format_args!(
                    "renderer frame={frame_id} event=present-complete elapsed_us={}",
                    frame_start.elapsed().as_micros()
                ));
                if suboptimal && self.reconfigure() {
                    emit_diagnostic(format_args!(
                        "renderer frame={frame_id} event=frame-presented suboptimal=true outcome=reconfigured state={:?}",
                        self.diagnostic_state()
                    ));
                    RedrawOutcome::Reconfigured
                } else {
                    emit_diagnostic(format_args!(
                        "renderer frame={frame_id} event=frame-presented suboptimal={suboptimal} outcome=presented"
                    ));
                    RedrawOutcome::Presented
                }
            }
            Err(
                error @ (wgpu::SurfaceError::Lost
                | wgpu::SurfaceError::Outdated
                | wgpu::SurfaceError::Other),
            ) => {
                emit_diagnostic(format_args!(
                    "renderer frame={frame_id} event=surface-acquire result=error error={error:?} state={:?}",
                    self.diagnostic_state()
                ));
                if self.reconfigure() {
                    emit_diagnostic(format_args!(
                        "renderer frame={frame_id} event=surface-recovery-scheduled"
                    ));
                    RedrawOutcome::Reconfigured
                } else {
                    emit_diagnostic(format_args!(
                        "renderer frame={frame_id} event=frame-skipped reason=surface-unconfigured"
                    ));
                    RedrawOutcome::Skipped
                }
            }
            Err(wgpu::SurfaceError::Timeout) => {
                emit_diagnostic(format_args!(
                    "renderer frame={frame_id} event=surface-acquire result=error error=Timeout outcome=skipped"
                ));
                RedrawOutcome::Skipped
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                emit_diagnostic(format_args!(
                    "renderer frame={frame_id} event=surface-acquire result=error error=OutOfMemory outcome=exit"
                ));
                RedrawOutcome::Exit
            }
        }
    }

    fn reconfigure(&mut self) -> bool {
        let Some(size) = self.size else {
            self.configuration = None;
            emit_diagnostic(format_args!(
                "renderer event=reconfigure result=skipped reason=zero-sized"
            ));
            return false;
        };
        let Some(configuration) =
            self.surface
                .get_default_config(&self.adapter, size.width(), size.height())
        else {
            self.configuration = None;
            emit_diagnostic(format_args!(
                "renderer event=reconfigure result=skipped reason=no-default-config size={}x{}",
                size.width(),
                size.height()
            ));
            return false;
        };
        let draw_resources_reset = needs_draw_resource_rebuild(
            self.configuration.as_ref().map(|current| current.format),
            configuration.format,
        );
        let present_mode = configuration.present_mode;
        let configure_started = Instant::now();
        self.surface.configure(&self.device, &configuration);
        let configure_us = configure_started.elapsed().as_micros();
        if draw_resources_reset {
            self.draw_resources = None;
        }
        self.configuration = Some(configuration);
        emit_diagnostic(format_args!(
            "renderer event=reconfigure result=configured present_mode={present_mode:?} draw_resources_reset={draw_resources_reset} configure_us={configure_us} state={:?}",
            self.diagnostic_state(),
        ));
        true
    }
}

fn gpu_backend_attempts(
    descriptor: wgpu::InstanceDescriptor,
    backend_override: bool,
) -> Vec<wgpu::InstanceDescriptor> {
    if cfg!(target_os = "windows")
        && !backend_override
        && descriptor.backends.contains(wgpu::Backends::DX12)
    {
        let mut dx12 = descriptor.clone();
        dx12.backends = wgpu::Backends::DX12;
        vec![dx12, descriptor]
    } else {
        vec![descriptor]
    }
}

fn initialize_gpu(
    window: &Arc<Window>,
) -> Result<
    (
        wgpu::Surface<'static>,
        wgpu::Adapter,
        wgpu::Device,
        wgpu::Queue,
    ),
    RendererInitError,
> {
    let descriptor = wgpu::InstanceDescriptor::from_env_or_default();
    let backend_override = wgpu::Backends::from_env().is_some();
    let mut last_error = None;
    for (attempt, descriptor) in gpu_backend_attempts(descriptor, backend_override)
        .into_iter()
        .enumerate()
    {
        let result = (|| {
            let instance = wgpu::Instance::new(&descriptor);
            let surface = instance
                .create_surface(Arc::clone(window))
                .map_err(|error| {
                    RendererInitError::new(format!("could not create surface: {error}"))
                })?;
            let adapter =
                pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::default(),
                    force_fallback_adapter: false,
                    compatible_surface: Some(&surface),
                }))
                .map_err(|error| {
                    RendererInitError::new(format!("could not find adapter: {error}"))
                })?;
            let (device, queue) =
                pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                    label: Some("terminal renderer device"),
                    ..Default::default()
                }))
                .map_err(|error| {
                    RendererInitError::new(format!("could not create device: {error}"))
                })?;
            Ok((surface, adapter, device, queue))
        })();
        match result {
            Ok(gpu) => return Ok(gpu),
            Err(error) => {
                emit_diagnostic(format_args!(
                    "renderer event=adapter-attempt-failed attempt={attempt} backends={:?} error={error}",
                    descriptor.backends
                ));
                last_error = Some(error);
            }
        }
    }
    Err(last_error.expect("at least one GPU backend is attempted"))
}

fn project_panes(
    panes: &[PaneRenderInput<'_>],
    overlay: Option<&TextOverlay>,
    theme: &RenderTheme,
) -> Vec<(TerminalRenderData, [u32; 4], bool)> {
    let multiple = panes.len() > 1;
    panes
        .iter()
        .map(|pane| {
            let mut data = TerminalRenderData::from_terminal_with_theme(pane.terminal, theme);
            data.scrollbar_hover = pane.scrollbar_hover;
            if !pane.focused {
                data.cursor = None;
            }
            if pane.focused
                && let Some(overlay) = overlay
            {
                data.apply_text_overlay(overlay, theme);
            }
            (data, pane.rect, multiple && pane.focused)
        })
        .collect()
}

fn needs_reconfigure(
    current_size: Option<SurfaceSize>,
    configured: bool,
    next_size: Option<SurfaceSize>,
) -> bool {
    current_size != next_size || (next_size.is_some() && !configured)
}

fn needs_draw_resource_rebuild(
    current_format: Option<wgpu::TextureFormat>,
    next_format: wgpu::TextureFormat,
) -> bool {
    current_format != Some(next_format)
}

#[cfg(test)]
mod tests {
    use super::{
        PaneRenderInput, SurfaceSize, gpu_backend_attempts, needs_draw_resource_rebuild,
        needs_reconfigure, project_panes,
    };

    #[test]
    fn default_backend_prefers_dx12_on_windows_with_fallback() {
        let descriptor = wgpu::InstanceDescriptor::default();
        let original_backends = descriptor.backends;
        let attempts = gpu_backend_attempts(descriptor, false);
        if cfg!(target_os = "windows") && original_backends.contains(wgpu::Backends::DX12) {
            assert_eq!(attempts.len(), 2);
            assert_eq!(attempts[0].backends, wgpu::Backends::DX12);
            assert_eq!(attempts[1].backends, original_backends);
        } else {
            assert_eq!(attempts.len(), 1);
            assert_eq!(attempts[0].backends, original_backends);
        }
    }

    #[test]
    fn explicit_backend_override_is_respected() {
        let descriptor = wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        };
        let attempts = gpu_backend_attempts(descriptor, true);
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].backends, wgpu::Backends::VULKAN);
    }

    #[test]
    fn pane_projection_keeps_both_outputs_and_only_the_focused_cursor() {
        use terminal_core::{TerminalDimensions, TerminalState};
        let mut left = TerminalState::new(TerminalDimensions::new(3, 2).unwrap());
        let mut right = TerminalState::new(TerminalDimensions::new(3, 2).unwrap());
        left.print_character('L').unwrap();
        right.print_character('R').unwrap();
        let data = project_panes(
            &[
                PaneRenderInput {
                    terminal: &left,
                    rect: [0, 0, 30, 40],
                    focused: false,
                    scrollbar_hover: Some(super::ScrollbarHit::Thumb),
                },
                PaneRenderInput {
                    terminal: &right,
                    rect: [30, 0, 31, 40],
                    focused: true,
                    scrollbar_hover: None,
                },
            ],
            None,
            &Default::default(),
        );
        assert_eq!(data.len(), 2);
        assert_eq!(data[0].1, [0, 0, 30, 40]);
        assert_eq!(data[1].1, [30, 0, 31, 40]);
        assert!(data[0].0.cells.iter().any(|cell| cell.character == 'L'));
        assert!(data[1].0.cells.iter().any(|cell| cell.character == 'R'));
        assert_eq!(data[0].0.cursor, None);
        assert_eq!(data[0].0.scrollbar_hover, Some(super::ScrollbarHit::Thumb));
        assert!(data[1].0.cursor.is_some());
        assert!(!data[0].2 && data[1].2);
    }

    #[test]
    fn surface_size_rejects_minimized_dimensions() {
        assert_eq!(SurfaceSize::new(0, 1), None);
        assert_eq!(SurfaceSize::new(1, 0), None);
        assert_eq!(SurfaceSize::new(0, 0), None);
    }

    #[test]
    fn surface_size_retains_nonzero_dimensions() {
        let size = SurfaceSize::new(800, 600).unwrap();
        assert_eq!((size.width(), size.height()), (800, 600));
    }

    #[test]
    fn unconfigured_nonzero_surface_is_reconfigured_even_at_the_same_size() {
        let size = SurfaceSize::new(800, 600);

        assert!(needs_reconfigure(size, false, size));
        assert!(!needs_reconfigure(size, true, size));
        assert!(needs_reconfigure(None, false, size));
        assert!(!needs_reconfigure(None, false, None));
    }

    #[test]
    fn resizing_with_the_same_surface_format_keeps_draw_resources() {
        assert!(!needs_draw_resource_rebuild(
            Some(wgpu::TextureFormat::Bgra8UnormSrgb),
            wgpu::TextureFormat::Bgra8UnormSrgb
        ));
        assert!(needs_draw_resource_rebuild(
            None,
            wgpu::TextureFormat::Bgra8UnormSrgb
        ));
        assert!(needs_draw_resource_rebuild(
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            wgpu::TextureFormat::Bgra8UnormSrgb
        ));
    }
}
