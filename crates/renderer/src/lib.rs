//! Project-owned GPU surface lifecycle.
//!
//! This crate owns GPU setup and terminal-state rendering, but never terminal
//! protocol semantics.

mod font;
mod gpu;
mod snapshot;

pub use font::{FontProcessingError, FontRequest, GlyphBitmap, ShapedGlyph, ShapedText};
pub use snapshot::{CursorRenderData, RenderCell, Rgba, TerminalRenderData};

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use winit::window::Window;

use crate::font::FontSystem;
use crate::gpu::DrawResources;

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
    draw_resources: Option<DrawResources>,
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
        let font_system = FontSystem::load_system(font_request).map_err(|error| {
            RendererInitError::new(format!("could not load terminal font: {error}"))
        })?;
        let size = SurfaceSize::new(window.inner_size().width, window.inner_size().height);
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance
            .create_surface(Arc::clone(&window))
            .map_err(|error| {
                RendererInitError::new(format!("could not create surface: {error}"))
            })?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .map_err(|error| RendererInitError::new(format!("could not find adapter: {error}")))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("terminal renderer device"),
            ..Default::default()
        }))
        .map_err(|error| RendererInitError::new(format!("could not create device: {error}")))?;

        let mut renderer = Self {
            window,
            surface,
            adapter,
            device,
            queue,
            configuration: None,
            size,
            font_system,
            draw_resources: None,
        };
        renderer.reconfigure();
        Ok(renderer)
    }

    /// Updates the desired surface size and configures only non-zero dimensions.
    pub fn resize(&mut self, width: u32, height: u32) -> bool {
        self.size = SurfaceSize::new(width, height);
        self.reconfigure()
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
        let data = TerminalRenderData::from_terminal(state);
        self.redraw_data(Some(&data))
    }

    fn redraw_data(&mut self, data: Option<&TerminalRenderData>) -> RedrawOutcome {
        if self.configuration.is_none() {
            return RedrawOutcome::Skipped;
        }

        match self.surface.get_current_texture() {
            Ok(frame) => {
                if let (Some(data), Some(size), Some(configuration)) =
                    (data, self.size, self.configuration.as_ref())
                {
                    let resources = self.draw_resources.get_or_insert_with(|| {
                        DrawResources::new(&self.device, configuration.format)
                    });
                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    resources.draw(
                        &self.device,
                        &self.queue,
                        &view,
                        data,
                        &mut self.font_system,
                        size,
                    );
                } else {
                    self.queue.submit(std::iter::empty());
                }
                self.window.pre_present_notify();
                frame.present();
                RedrawOutcome::Presented
            }
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                if self.reconfigure() {
                    RedrawOutcome::Reconfigured
                } else {
                    RedrawOutcome::Skipped
                }
            }
            Err(wgpu::SurfaceError::Timeout) => RedrawOutcome::Skipped,
            Err(wgpu::SurfaceError::OutOfMemory) => RedrawOutcome::Exit,
            Err(wgpu::SurfaceError::Other) => RedrawOutcome::Skipped,
        }
    }

    fn reconfigure(&mut self) -> bool {
        let Some(size) = self.size else {
            self.configuration = None;
            return false;
        };
        let Some(configuration) =
            self.surface
                .get_default_config(&self.adapter, size.width(), size.height())
        else {
            self.configuration = None;
            return false;
        };
        self.surface.configure(&self.device, &configuration);
        self.draw_resources = None;
        self.configuration = Some(configuration);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::SurfaceSize;

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
}
