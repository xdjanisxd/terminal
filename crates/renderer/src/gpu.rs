use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use bytemuck::{Pod, Zeroable};

use crate::{
    FontSystem, ScrollbarGeometry, ScrollbarHit, ScrollbarRenderData, TerminalRenderData,
    diagnostics_enabled, emit_diagnostic,
};

const ATLAS_WIDTH: u32 = 1024;
const ATLAS_HEIGHT: u32 = 1024;
const ATLAS_SLOT_SIZE: u32 = 64;
const ATLAS_COLUMNS: u32 = ATLAS_WIDTH / ATLAS_SLOT_SIZE;
const ATLAS_CAPACITY: usize = (ATLAS_COLUMNS * (ATLAS_HEIGHT / ATLAS_SLOT_SIZE)) as usize;

#[cfg(test)]
#[path = "gpu_bench.rs"]
mod benchmarks;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RectInstance {
    rect: [f32; 4],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GlyphInstance {
    rect: [f32; 4],
    uv: [f32; 4],
    color: [f32; 4],
    // 0: foreground mask, 1: encoded color, 2: linearize color for sRGB target.
    content: u32,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct GpuGlyphKey {
    face: crate::font::FaceCacheIdentity,
    glyph_id: u16,
    pixels_per_em: u32,
    bold: bool,
    italic: bool,
}

#[derive(Clone, Copy)]
struct AtlasEntry {
    slot: usize,
    width: u32,
    height: u32,
    bearing_x: i32,
    bearing_y: i32,
    is_color: bool,
}

#[derive(Clone, Copy)]
struct GlyphGeometrySample {
    row: u32,
    cell_y: f32,
    cell_height: f32,
    ascent: f32,
    descent: f32,
    baseline: f32,
    placement_top: i32,
    bitmap_height: u32,
    quad_y_start: f32,
    quad_y_end: f32,
}

struct GlyphAtlas {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    entries: HashMap<GpuGlyphKey, AtlasEntry>,
    lru: VecDeque<GpuGlyphKey>,
    slots: Vec<Option<GpuGlyphKey>>,
}

impl GlyphAtlas {
    fn get(&mut self, key: &GpuGlyphKey) -> Option<AtlasEntry> {
        let entry = self.entries.get(key).copied()?;
        self.touch(key);
        Some(entry)
    }

    fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let texture_stage = crate::startup_stage("atlas-texture-create");
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("terminal glyph atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_WIDTH,
                height: ATLAS_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Keep encoded color here; the fragment shader decodes when the
            // render target blends in linear light. Masks retain byte coverage.
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        drop(texture_stage);
        let bindings_stage = crate::startup_stage("atlas-view-sampler-bind-group");
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("terminal glyph atlas sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terminal glyph atlas bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        drop(bindings_stage);
        Self {
            texture,
            bind_group,
            entries: HashMap::new(),
            lru: VecDeque::new(),
            slots: vec![None; ATLAS_CAPACITY],
        }
    }

    fn get_or_insert(
        &mut self,
        queue: &wgpu::Queue,
        key: GpuGlyphKey,
        bitmap: &crate::GlyphBitmap,
    ) -> Option<AtlasEntry> {
        if bitmap.width() == 0
            || bitmap.height() == 0
            || bitmap.width() > ATLAS_SLOT_SIZE
            || bitmap.height() > ATLAS_SLOT_SIZE
        {
            return None;
        }
        if let Some(entry) = self.get(&key) {
            return Some(entry);
        }
        let slot = self.allocate_slot();
        let x = (slot as u32 % ATLAS_COLUMNS) * ATLAS_SLOT_SIZE;
        let y = (slot as u32 / ATLAS_COLUMNS) * ATLAS_SLOT_SIZE;
        let mask_rgba;
        let pixels = if bitmap.is_color() {
            bitmap.pixels()
        } else {
            mask_rgba = bitmap
                .pixels()
                .iter()
                .flat_map(|&a| [a; 4])
                .collect::<Vec<_>>();
            &mask_rgba
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bitmap.width() * 4),
                rows_per_image: Some(bitmap.height()),
            },
            wgpu::Extent3d {
                width: bitmap.width(),
                height: bitmap.height(),
                depth_or_array_layers: 1,
            },
        );
        let entry = AtlasEntry {
            slot,
            width: bitmap.width(),
            height: bitmap.height(),
            bearing_x: bitmap.bearing_x(),
            bearing_y: bitmap.bearing_y(),
            is_color: bitmap.is_color(),
        };
        self.entries.insert(key.clone(), entry);
        self.slots[slot] = Some(key.clone());
        self.lru.push_back(key);
        Some(entry)
    }

    fn allocate_slot(&mut self) -> usize {
        if let Some(slot) = self.slots.iter().position(Option::is_none) {
            return slot;
        }
        let oldest = self.lru.pop_front().expect("non-empty full glyph atlas");
        let entry = self
            .entries
            .remove(&oldest)
            .expect("atlas LRU matches entries");
        entry.slot
    }

    fn touch(&mut self, key: &GpuGlyphKey) {
        if let Some(index) = self.lru.iter().position(|candidate| candidate == key) {
            self.lru.remove(index);
        }
        self.lru.push_back(key.clone());
    }
}

struct CompositionResources {
    size: crate::SurfaceSize,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

struct CompositionPipeline {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
}

pub(super) struct DrawResources {
    composition_pipeline: Option<CompositionPipeline>,
    composition: Option<CompositionResources>,
    background_pipeline: Option<wgpu::RenderPipeline>,
    format: wgpu::TextureFormat,
    rect_pipeline: wgpu::RenderPipeline,
    glyph_pipeline: wgpu::RenderPipeline,
    glyph_atlas: GlyphAtlas,
    rectangles: Vec<RectInstance>,
    glyphs: Vec<GlyphInstance>,
    rectangle_buffer: Option<InstanceBuffer<RectInstance>>,
    glyph_buffer: Option<InstanceBuffer<GlyphInstance>>,
    overlay_buffer: Option<InstanceBuffer<RectInstance>>,
}

/// Per-frame draw inputs retained only for concise lifecycle diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RenderInstanceCounts {
    pub(super) backgrounds: usize,
    pub(super) glyphs: usize,
    pub(super) decorations: usize,
    pub(super) cursor: usize,
    pub(super) scrollbar: usize,
}

/// GPU work performed for one submitted terminal frame.
///
/// These counts deliberately describe resource churn rather than terminal
/// contents so a resize gesture can be diagnosed without retaining frames.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RenderWork {
    pub(super) instances: RenderInstanceCounts,
    pub(super) buffer_allocations: usize,
    pub(super) buffer_writes: usize,
    pub(super) queue_submissions: usize,
    pub(super) buffer_bytes: usize,
    pub(super) shape_calls: usize,
    pub(super) shape_misses: usize,
    pub(super) raster_calls: usize,
    pub(super) atlas_lookups: usize,
    pub(super) instance_generation: Duration,
    pub(super) buffer_upload: Duration,
    pub(super) submission: Duration,
}

struct InstanceBuffer<T> {
    buffer: wgpu::Buffer,
    capacity: usize,
    previous: Vec<T>,
    marker: std::marker::PhantomData<T>,
}

#[derive(Default)]
struct UploadWork {
    allocated: bool,
    writes: usize,
    bytes: usize,
}

impl<T: Pod> InstanceBuffer<T> {
    fn upload(
        current: &mut Option<Self>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &'static str,
        instances: &[T],
    ) -> UploadWork {
        if instances.is_empty() {
            return UploadWork::default();
        }
        let required = instances.len();
        let allocation_required = instance_buffer_requires_allocation(
            current.as_ref().map(|buffer| buffer.capacity),
            required,
        );
        if allocation_required {
            let _allocation_stage = crate::startup_stage("instance-buffer-allocation");
            let capacity = instance_buffer_capacity(required);
            *current = Some(Self {
                buffer: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(label),
                    size: (capacity * std::mem::size_of::<T>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                capacity,
                previous: Vec::new(),
                marker: std::marker::PhantomData,
            });
        }
        let buffer = current
            .as_mut()
            .expect("non-empty instances allocate a buffer");
        let ranges = changed_instance_ranges(&buffer.previous, instances);
        let mut work = UploadWork {
            allocated: allocation_required,
            ..UploadWork::default()
        };
        let write_stage = crate::startup_stage("instance-buffer-writes");
        for range in ranges {
            let bytes = bytemuck::cast_slice(&instances[range.clone()]);
            queue.write_buffer(
                &buffer.buffer,
                (range.start * std::mem::size_of::<T>()) as u64,
                bytes,
            );
            work.writes += 1;
            work.bytes += bytes.len();
        }
        drop(write_stage);
        buffer.previous.clear();
        buffer.previous.extend_from_slice(instances);
        work
    }
}

fn changed_instance_ranges<T: Pod>(previous: &[T], current: &[T]) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = None;
    for (index, instance) in current.iter().enumerate() {
        let changed = previous
            .get(index)
            .is_none_or(|old| bytemuck::bytes_of(old) != bytemuck::bytes_of(instance));
        match (start, changed) {
            (None, true) => start = Some(index),
            (Some(begin), false) => {
                ranges.push(begin..index);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(begin) = start {
        ranges.push(begin..current.len());
    }
    if ranges.len() > 8 {
        let merged = ranges[0].start..ranges.last().expect("non-empty ranges").end;
        ranges.clear();
        ranges.push(merged);
    }
    ranges
}

pub(super) struct FrameContext<'a> {
    pub(super) target: &'a wgpu::TextureView,
    pub(super) surface_size: crate::SurfaceSize,
    pub(super) cell_metrics: crate::CellMetrics,
    pub(super) ui: &'a crate::UiRenderTheme,
}

pub(super) struct PaneDraw<'a> {
    pub text_offset: [f32; 2],
    pub rectangles: &'a [([f32; 4], crate::Rgba)],
    pub rect: [u32; 4],
    pub clear: bool,
    pub focused_border: bool,
}

impl DrawResources {
    pub(super) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader_stage = crate::startup_stage("shader-and-layout");
        let module_stage = crate::startup_stage("shader-module-create");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terminal renderer shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("terminal.wgsl").into()),
        });
        drop(module_stage);
        let layout_stage = crate::startup_stage("glyph-bind-group-layout");
        let glyph_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("terminal glyph atlas layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        drop(layout_stage);
        drop(shader_stage);
        let rect_stage = crate::startup_stage("rectangle-pipeline");
        let rect_pipeline = pipeline(
            device,
            &shader,
            "rect_vertex",
            "rect_fragment",
            format,
            &[],
            rect_layout(),
        );
        drop(rect_stage);
        let glyph_stage = crate::startup_stage("glyph-pipeline");
        let glyph_pipeline = pipeline(
            device,
            &shader,
            "glyph_vertex",
            "glyph_fragment",
            format,
            &[&glyph_bind_group_layout],
            glyph_vertex_layout(),
        );
        drop(glyph_stage);
        let atlas_stage = crate::startup_stage("glyph-atlas");
        let glyph_atlas = GlyphAtlas::new(device, &glyph_bind_group_layout);
        drop(atlas_stage);
        Self {
            composition_pipeline: None,
            composition: None,
            background_pipeline: None,
            format,
            rect_pipeline,
            glyph_pipeline,
            glyph_atlas,
            rectangles: Vec::new(),
            glyphs: Vec::new(),
            rectangle_buffer: None,
            glyph_buffer: None,
            overlay_buffer: None,
        }
    }

    /// Allocate only for translucent frames, retaining the opaque rendering path.
    pub(super) fn composition_target(
        &mut self,
        device: &wgpu::Device,
        size: crate::SurfaceSize,
    ) -> wgpu::TextureView {
        if self.composition_pipeline.is_none() {
            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("terminal composition layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
            });
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("terminal composition shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("composition.wgsl").into()),
            });
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("terminal composition pipeline layout"),
                bind_group_layouts: &[&layout],
                push_constant_ranges: &[],
            });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("terminal stored premultiplication"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vertex"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(if self.format.is_srgb() {
                        "srgb_fragment"
                    } else {
                        "linear_fragment"
                    }),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: self.format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview: None,
                cache: None,
            });
            self.composition_pipeline = Some(CompositionPipeline { layout, pipeline });
        }
        if self
            .composition
            .as_ref()
            .is_none_or(|current| current.size != size)
        {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("terminal linear composition target"),
                size: wgpu::Extent3d {
                    width: size.width(),
                    height: size.height(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("terminal composition input"),
                layout: &self.composition_pipeline.as_ref().unwrap().layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                }],
            });
            self.composition = Some(CompositionResources {
                size,
                view,
                bind_group,
            });
        }
        self.composition.as_ref().unwrap().view.clone()
    }

    pub(super) fn compose(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
    ) {
        let composition = self.composition.as_ref().unwrap();
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("terminal native alpha composition"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.composition_pipeline.as_ref().unwrap().pipeline);
            pass.set_bind_group(0, &composition.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit(Some(encoder.finish()));
    }

    #[cfg(test)]
    pub(super) fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: FrameContext<'_>,
        data: &TerminalRenderData,
        font_system: &mut FontSystem,
    ) -> RenderWork {
        let pane = [
            0,
            0,
            frame.surface_size.width(),
            frame.surface_size.height(),
        ];
        self.draw_pane(
            device,
            queue,
            frame,
            data,
            font_system,
            PaneDraw {
                text_offset: [0.0; 2],
                rectangles: &[],
                rect: pane,
                clear: true,
                focused_border: false,
            },
        )
    }

    pub(super) fn draw_pane(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: FrameContext<'_>,
        data: &TerminalRenderData,
        font_system: &mut FontSystem,
        pane_draw: PaneDraw<'_>,
    ) -> RenderWork {
        let pane = pane_draw.rect;
        let transparent = data.surface_background.0[3] < 1.0;
        if transparent && self.background_pipeline.is_none() {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("terminal background replacement shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("terminal.wgsl").into()),
            });
            self.background_pipeline = Some(pipeline(
                device,
                &shader,
                "rect_vertex",
                "background_fragment",
                self.format,
                &[],
                rect_layout(),
            ));
        }
        let generation_stage = crate::startup_stage("instance-generation");
        let generation_start = Instant::now();
        let mut shape_calls = 0;
        let mut shape_misses = 0;
        let mut raster_calls = 0;
        let mut atlas_lookups = 0;
        let mut background_count = 0;
        let cell_width = frame.cell_metrics.width() as f32;
        let cell_height = frame.cell_metrics.height() as f32;
        self.rectangles.clear();
        self.glyphs.clear();
        self.rectangles
            .reserve(data.cells.len().saturating_sub(self.rectangles.capacity()));
        let mut previous_geometry: Option<GlyphGeometrySample> = None;
        let mut geometry_pair_logged = false;
        if transparent {
            for cell in &data.cells {
                if cell.background != data.surface_background {
                    self.rectangles.push(RectInstance {
                        rect: to_clip_rect(
                            pane[0] as f32 + cell.column as f32 * cell_width,
                            pane[1] as f32 + cell.row as f32 * cell_height,
                            cell.width as f32 * cell_width,
                            cell_height,
                            frame.surface_size,
                        ),
                        color: cell.background.0,
                    });
                    background_count += 1;
                }
            }
        }
        let replacement_count = background_count;
        for cell in &data.cells {
            let x = pane[0] as f32 + pane_draw.text_offset[0] + cell.column as f32 * cell_width;
            let y = pane[1] as f32 + pane_draw.text_offset[1] + cell.row as f32 * cell_height;
            let width = cell.width as f32 * cell_width;
            if !transparent && cell.background != data.surface_background {
                self.rectangles.push(RectInstance {
                    rect: to_clip_rect(x, y, width, cell_height, frame.surface_size),
                    color: cell.background.0,
                });
                background_count += 1;
            }
            if cell.concealed {
                continue;
            }
            if cell.strikethrough {
                self.rectangles.push(RectInstance {
                    rect: to_clip_rect(
                        x,
                        y + (frame.cell_metrics.baseline() as f32 * 0.6).round(),
                        width,
                        1.0,
                        frame.surface_size,
                    ),
                    color: cell.foreground.0,
                });
            }
            if cell.underline {
                self.rectangles.push(RectInstance {
                    rect: to_clip_rect(x, y + cell_height - 1.0, width, 1.0, frame.surface_size),
                    color: cell.foreground.0,
                });
            }
            if cell.character == ' ' || cell.character == '\0' {
                continue;
            }
            let pixels_per_em = frame.cell_metrics.pixels_per_em();
            shape_calls += 1;
            let Ok((shaped, cache_hit)) = font_system.shape_text_cached_with_bold(
                cell.text.as_str(),
                pixels_per_em,
                cell.bold,
            ) else {
                continue;
            };
            shape_misses += usize::from(!cache_hit);
            for shaped_glyph in shaped.glyphs() {
                let key = GpuGlyphKey {
                    face: shaped_glyph.face_cache_identity(),
                    glyph_id: shaped_glyph.glyph_id(),
                    pixels_per_em: pixels_per_em.to_bits(),
                    bold: cell.bold,
                    italic: cell.italic,
                };
                atlas_lookups += 1;
                let entry = if let Some(entry) = self.glyph_atlas.get(&key) {
                    Some(entry)
                } else {
                    raster_calls += 1;
                    font_system
                        .rasterize_glyph_with_style(
                            shaped_glyph,
                            pixels_per_em,
                            cell.bold,
                            cell.italic,
                        )
                        .ok()
                        .and_then(|bitmap| self.glyph_atlas.get_or_insert(queue, key, &bitmap))
                };
                let Some(entry) = entry else {
                    continue;
                };
                let slot_x = (entry.slot as u32 % ATLAS_COLUMNS) * ATLAS_SLOT_SIZE;
                let slot_y = (entry.slot as u32 / ATLAS_COLUMNS) * ATLAS_SLOT_SIZE;
                let [mut glyph_x, mut glyph_y] = glyph_origin(
                    x,
                    y,
                    frame.cell_metrics.baseline() as f32,
                    entry.bearing_x,
                    entry.bearing_y,
                    shaped_glyph.offset_x(),
                    shaped_glyph.offset_y(),
                );
                // Color fonts can have bitmap-strike metrics much larger than
                // the primary face. Fit ink to the terminal-owned cell span.
                let scale = if entry.is_color {
                    (width / entry.width as f32)
                        .min(cell_height / entry.height as f32)
                        .min(1.0)
                } else {
                    1.0
                };
                if entry.is_color {
                    glyph_x = x + (width - entry.width as f32 * scale) * 0.5;
                    glyph_y = y + (cell_height - entry.height as f32 * scale) * 0.5;
                }
                let Some((clipped_x, clipped_y, clipped_width, clipped_height)) = clip_rect_to_cell(
                    [
                        glyph_x,
                        glyph_y,
                        entry.width as f32 * scale,
                        entry.height as f32 * scale,
                    ],
                    [x, y, width, cell_height],
                ) else {
                    continue;
                };
                self.glyphs.push(GlyphInstance {
                    rect: to_clip_rect(
                        clipped_x,
                        clipped_y,
                        clipped_width,
                        clipped_height,
                        frame.surface_size,
                    ),
                    uv: [
                        (slot_x as f32 + (clipped_x - glyph_x) / scale) / ATLAS_WIDTH as f32,
                        (slot_y as f32 + (clipped_y - glyph_y) / scale) / ATLAS_HEIGHT as f32,
                        (slot_x as f32 + (clipped_x - glyph_x + clipped_width) / scale)
                            / ATLAS_WIDTH as f32,
                        (slot_y as f32 + (clipped_y - glyph_y + clipped_height) / scale)
                            / ATLAS_HEIGHT as f32,
                    ],
                    color: cell.foreground.0,
                    content: if entry.is_color {
                        1 + u32::from(self.format.is_srgb())
                    } else {
                        0
                    },
                });
                if diagnostics_enabled() && !geometry_pair_logged {
                    let sample = GlyphGeometrySample {
                        row: cell.row as u32,
                        cell_y: y,
                        cell_height,
                        ascent: frame.cell_metrics.ascent(),
                        descent: frame.cell_metrics.descent(),
                        baseline: frame.cell_metrics.baseline() as f32,
                        placement_top: entry.bearing_y,
                        bitmap_height: entry.height,
                        quad_y_start: clipped_y,
                        quad_y_end: clipped_y + clipped_height,
                    };
                    if let Some(previous) = previous_geometry
                        && sample.row == previous.row + 1
                    {
                        emit_glyph_geometry_sample(previous);
                        emit_glyph_geometry_sample(sample);
                        geometry_pair_logged = true;
                    } else if previous_geometry.is_none_or(|previous| previous.row != sample.row) {
                        previous_geometry = Some(sample);
                    }
                }
            }
        }
        for (rect, color) in pane_draw.rectangles {
            self.rectangles.push(RectInstance {
                rect: to_clip_rect(
                    pane[0] as f32 + rect[0],
                    pane[1] as f32 + rect[1],
                    rect[2],
                    rect[3],
                    frame.surface_size,
                ),
                color: color.0,
            });
        }
        let mut overlays = Vec::with_capacity(3);
        if let Some(cursor) = data.cursor {
            let (x, y, width, height) = match cursor.shape {
                terminal_core::CursorShape::Block => (0.0, 0.0, cell_width, cell_height),
                terminal_core::CursorShape::Beam => {
                    (0.0, 0.0, (cell_width * 0.08).max(1.0), cell_height)
                }
                terminal_core::CursorShape::Underline => (
                    0.0,
                    cell_height * 0.9,
                    cell_width,
                    (cell_height * 0.1).max(1.0),
                ),
            };
            overlays.push(RectInstance {
                rect: to_clip_rect(
                    pane[0] as f32 + cursor.column as f32 * cell_width + x,
                    pane[1] as f32 + cursor.row as f32 * cell_height + y,
                    width,
                    height,
                    frame.surface_size,
                ),
                color: data.cursor_color.0,
            });
        }
        if let Some(scrollbar) = data.scrollbar
            && let Some(geometry) = scrollbar_geometry(
                scrollbar,
                crate::SurfaceSize::new(pane[2], pane[3]).unwrap(),
                cell_height,
            )
        {
            overlays.push(RectInstance {
                rect: to_clip_rect(
                    pane[0] as f32 + geometry.track[0],
                    pane[1] as f32 + geometry.track[1],
                    geometry.track[2],
                    geometry.track[3],
                    frame.surface_size,
                ),
                color: if data.scrollbar_hover.is_some() {
                    frame.ui.scrollbar_track_hover.0
                } else {
                    frame.ui.scrollbar_track.0
                },
            });
            overlays.push(RectInstance {
                rect: to_clip_rect(
                    pane[0] as f32 + geometry.thumb[0],
                    pane[1] as f32 + geometry.thumb[1],
                    geometry.thumb[2],
                    geometry.thumb[3],
                    frame.surface_size,
                ),
                color: if data.scrollbar_hover == Some(ScrollbarHit::Thumb) {
                    frame.ui.scrollbar_thumb_hover.0
                } else {
                    frame.ui.scrollbar_thumb.0
                },
            });
            for marker in data
                .search_markers
                .iter()
                .filter(|marker| !marker.active)
                .chain(data.search_markers.iter().filter(|marker| marker.active))
            {
                let y = geometry.marker_y(marker.row, scrollbar.visible_rows);
                overlays.push(RectInstance {
                    rect: to_clip_rect(
                        pane[0] as f32 + geometry.track[0],
                        pane[1] as f32 + y - 1.0,
                        geometry.track[2],
                        2.0,
                        frame.surface_size,
                    ),
                    color: if marker.active {
                        frame.ui.search_active_marker.0
                    } else {
                        frame.ui.search_marker.0
                    },
                });
            }
        }
        if pane_draw.focused_border && pane[2] > 1 && pane[3] > 1 {
            let color = frame.ui.focus_border.0;
            for rect in [
                [pane[0] as f32, pane[1] as f32, pane[2] as f32, 2.0],
                [
                    pane[0] as f32,
                    (pane[1] + pane[3] - 2) as f32,
                    pane[2] as f32,
                    2.0,
                ],
                [pane[0] as f32, pane[1] as f32, 2.0, pane[3] as f32],
                [
                    (pane[0] + pane[2] - 2) as f32,
                    pane[1] as f32,
                    2.0,
                    pane[3] as f32,
                ],
            ] {
                overlays.push(RectInstance {
                    rect: to_clip_rect_from_pixels(rect, frame.surface_size),
                    color,
                });
            }
        }
        // Projection/theme channels are encoded sRGB. sRGB attachments encode
        // shader output on write and blend in linear light, including the
        // translucent intermediate target. Leave non-sRGB targets unchanged.
        if self.format.is_srgb() {
            for instance in &mut self.rectangles {
                instance.color = crate::Rgba(instance.color).linearized().0;
            }
            for instance in &mut self.glyphs {
                instance.color = crate::Rgba(instance.color).linearized().0;
            }
            for instance in &mut overlays {
                instance.color = crate::Rgba(instance.color).linearized().0;
            }
        }
        let clear_color = if self.format.is_srgb() {
            data.surface_background.linearized().0
        } else {
            data.surface_background.0
        };
        let rectangle_count = self.rectangles.len();
        let glyph_count = self.glyphs.len();
        let instance_generation = generation_start.elapsed();
        drop(generation_stage);
        let buffer_stage = crate::startup_stage("buffer-create-upload");
        let upload_start = Instant::now();
        let rectangle_upload = InstanceBuffer::upload(
            &mut self.rectangle_buffer,
            device,
            queue,
            "terminal rectangle instances",
            &self.rectangles,
        );
        let glyph_upload = InstanceBuffer::upload(
            &mut self.glyph_buffer,
            device,
            queue,
            "terminal glyph instances",
            &self.glyphs,
        );
        let overlay_upload = if overlays.is_empty() {
            UploadWork::default()
        } else {
            InstanceBuffer::upload(
                &mut self.overlay_buffer,
                device,
                queue,
                "terminal overlay instances",
                &overlays,
            )
        };
        let buffer_upload = upload_start.elapsed();
        drop(buffer_stage);
        let encoding_stage = crate::startup_stage("render-encoding");
        let submission_start = Instant::now();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("terminal frame encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("terminal render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: frame.target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if pane_draw.clear {
                            wgpu::LoadOp::Clear(wgpu::Color {
                                r: (clear_color[0] * clear_color[3]) as f64,
                                g: (clear_color[1] * clear_color[3]) as f64,
                                b: (clear_color[2] * clear_color[3]) as f64,
                                a: data.surface_background.0[3] as f64,
                            })
                        } else {
                            wgpu::LoadOp::Load
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_scissor_rect(pane[0], pane[1], pane[2], pane[3]);
            if rectangle_count != 0 {
                pass.set_pipeline(&self.rect_pipeline);
                pass.set_vertex_buffer(
                    0,
                    self.rectangle_buffer
                        .as_ref()
                        .expect("rectangle instances have a buffer")
                        .buffer
                        .slice(..),
                );
                if replacement_count != 0 {
                    pass.set_pipeline(self.background_pipeline.as_ref().unwrap());
                    pass.draw(0..6, 0..replacement_count as u32);
                    pass.set_pipeline(&self.rect_pipeline);
                }
                pass.draw(0..6, replacement_count as u32..rectangle_count as u32);
            }
            if glyph_count != 0 {
                pass.set_pipeline(&self.glyph_pipeline);
                pass.set_bind_group(0, &self.glyph_atlas.bind_group, &[]);
                pass.set_vertex_buffer(
                    0,
                    self.glyph_buffer
                        .as_ref()
                        .expect("glyph instances have a buffer")
                        .buffer
                        .slice(..),
                );
                pass.draw(0..6, 0..glyph_count as u32);
            }
            if !overlays.is_empty() {
                pass.set_pipeline(&self.rect_pipeline);
                pass.set_vertex_buffer(
                    0,
                    self.overlay_buffer
                        .as_ref()
                        .expect("overlay instances have a buffer")
                        .buffer
                        .slice(..),
                );
                pass.draw(0..6, 0..overlays.len() as u32);
            }
        }
        let commands = encoder.finish();
        drop(encoding_stage);
        let submit_stage = crate::startup_stage("queue-submit");
        queue.submit(Some(commands));
        drop(submit_stage);
        let submission = submission_start.elapsed();
        RenderWork {
            instances: RenderInstanceCounts {
                backgrounds: background_count,
                glyphs: glyph_count,
                decorations: rectangle_count - background_count,
                cursor: usize::from(data.cursor.is_some()),
                scrollbar: usize::from(data.scrollbar.is_some()) * 2,
            },
            buffer_allocations: usize::from(rectangle_upload.allocated)
                + usize::from(glyph_upload.allocated)
                + usize::from(overlay_upload.allocated),
            buffer_writes: rectangle_upload.writes + glyph_upload.writes + overlay_upload.writes,
            queue_submissions: 1,
            buffer_bytes: rectangle_upload.bytes + glyph_upload.bytes + overlay_upload.bytes,
            shape_calls,
            shape_misses,
            raster_calls,
            atlas_lookups,
            instance_generation,
            buffer_upload,
            submission,
        }
    }
}

fn emit_glyph_geometry_sample(sample: GlyphGeometrySample) {
    emit_diagnostic(format_args!(
        "renderer event=glyph-geometry row={} cell_y={} cell_height={} ascent={} descent={} baseline={} placement_top={} bitmap_height={} quad_y_start={} quad_y_end={}",
        sample.row,
        sample.cell_y,
        sample.cell_height,
        sample.ascent,
        sample.descent,
        sample.baseline,
        sample.placement_top,
        sample.bitmap_height,
        sample.quad_y_start,
        sample.quad_y_end,
    ));
}

fn instance_buffer_capacity(required: usize) -> usize {
    required
        .max(1)
        .checked_next_power_of_two()
        .unwrap_or(required)
}

fn instance_buffer_requires_allocation(current_capacity: Option<usize>, required: usize) -> bool {
    required != 0 && current_capacity.is_none_or(|capacity| capacity < required)
}

fn rect_layout() -> wgpu::VertexBufferLayout<'static> {
    const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4];
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<RectInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &ATTRIBUTES,
    }
}

fn glyph_vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    const ATTRIBUTES: [wgpu::VertexAttribute; 4] =
        wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Uint32];
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GlyphInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &ATTRIBUTES,
    }
}

fn pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    vertex: &str,
    fragment: &str,
    format: wgpu::TextureFormat,
    layouts: &[&wgpu::BindGroupLayout],
    vertex_layout: wgpu::VertexBufferLayout<'static>,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("terminal pipeline layout"),
        bind_group_layouts: layouts,
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("terminal pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(vertex),
            buffers: &[vertex_layout],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: (fragment != "background_fragment")
                    .then_some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: Default::default(),
        multiview: None,
        cache: None,
    })
}

fn to_clip_rect(x: f32, y: f32, width: f32, height: f32, surface: crate::SurfaceSize) -> [f32; 4] {
    let scale_x = 2.0 / surface.width() as f32;
    let scale_y = 2.0 / surface.height() as f32;
    [
        x * scale_x - 1.0,
        1.0 - y * scale_y,
        width * scale_x,
        height * scale_y,
    ]
}

fn scrollbar_geometry(
    data: ScrollbarRenderData,
    surface: crate::SurfaceSize,
    cell_height: f32,
) -> Option<ScrollbarGeometry> {
    ScrollbarGeometry::new(data, surface, cell_height)
}

fn to_clip_rect_from_pixels(rect: [f32; 4], surface: crate::SurfaceSize) -> [f32; 4] {
    to_clip_rect(rect[0], rect[1], rect[2], rect[3], surface)
}

fn glyph_origin(
    cell_x: f32,
    cell_y: f32,
    baseline: f32,
    placement_left: i32,
    placement_top: i32,
    offset_x: f32,
    offset_y: f32,
) -> [f32; 2] {
    [
        cell_x + placement_left as f32 + offset_x,
        cell_y + baseline - placement_top as f32 - offset_y,
    ]
}

fn clip_rect_to_cell(rect: [f32; 4], cell: [f32; 4]) -> Option<(f32, f32, f32, f32)> {
    let [x, y, width, height] = rect;
    let [cell_x, cell_y, cell_width, cell_height] = cell;
    let left = x.max(cell_x);
    let top = y.max(cell_y);
    let right = (x + width).min(cell_x + cell_width);
    let bottom = (y + height).min(cell_y + cell_height);
    (left < right && top < bottom).then_some((left, top, right - left, bottom - top))
}

#[cfg(test)]
mod tests {
    use super::{
        DrawResources, FrameContext, GpuGlyphKey, changed_instance_ranges, clip_rect_to_cell,
        glyph_origin, instance_buffer_capacity, instance_buffer_requires_allocation,
        scrollbar_geometry, to_clip_rect,
    };
    use crate::{FontRequest, FontSystem, ScrollbarRenderData, SurfaceSize, TerminalRenderData};
    use terminal_core::{CellColor, TerminalDimensions, TerminalState, UnderlineStyle};

    #[test]
    fn scrollbar_geometry_tracks_history_position_growth_and_resize() {
        let surface = SurfaceSize::new(100, 100).unwrap();
        let mut data = ScrollbarRenderData {
            history_rows: 5,
            visible_rows: 5,
            viewport_offset: 0,
        };
        let bottom = scrollbar_geometry(data, surface, 20.0).unwrap();
        assert_eq!(bottom.track, [92.0, 2.0, 8.0, 96.0]);
        assert_eq!(bottom.thumb, [92.0, 50.0, 8.0, 48.0]);
        data.viewport_offset = 5;
        let top = scrollbar_geometry(data, surface, 20.0).unwrap();
        assert_eq!(top.thumb[1], 2.0);
        data.viewport_offset = 2;
        let middle = scrollbar_geometry(data, surface, 20.0).unwrap();
        assert!(top.thumb[1] < middle.thumb[1] && middle.thumb[1] < bottom.thumb[1]);

        data.history_rows = 15;
        let grown = scrollbar_geometry(data, surface, 20.0).unwrap();
        assert!(grown.thumb[3] < middle.thumb[3]);
        data.visible_rows = 3;
        let resized = scrollbar_geometry(data, SurfaceSize::new(100, 60).unwrap(), 20.0).unwrap();
        assert!(resized.thumb[1] + resized.thumb[3] <= 60.0);
        assert!(resized.thumb[3] >= 18.0);
        data.history_rows = 0;
        assert_eq!(scrollbar_geometry(data, surface, 20.0), None);
    }

    #[test]
    fn clips_cell_rectangles_to_surface_coordinates() {
        assert_eq!(
            to_clip_rect(0.0, 0.0, 50.0, 25.0, SurfaceSize::new(100, 50).unwrap()),
            [-1.0, 1.0, 1.0, 1.0]
        );
    }

    #[test]
    fn glyph_rectangles_are_clipped_to_their_own_cell_bounds() {
        assert_eq!(
            clip_rect_to_cell([8.0, -2.0, 10.0, 20.0], [10.0, 0.0, 8.0, 12.0]),
            Some((10.0, 0.0, 8.0, 12.0))
        );
        assert_eq!(
            clip_rect_to_cell([20.0, 0.0, 3.0, 3.0], [10.0, 0.0, 8.0, 12.0]),
            None
        );
    }

    #[test]
    fn glyph_origin_uses_the_font_baseline_instead_of_the_cell_bottom() {
        assert_eq!(glyph_origin(4.0, 20.0, 15.0, 2, 11, 0.5, 0.0), [6.5, 24.0]);
    }

    #[test]
    fn glyph_origins_keep_terminal_rows_one_cell_height_apart() {
        let first = glyph_origin(0.0, 0.0, 15.0, 0, 11, 0.0, 0.0);
        let second = glyph_origin(0.0, 20.0, 15.0, 0, 11, 0.0, 0.0);
        assert_eq!(second[1] - first[1], 20.0);
    }

    #[test]
    fn adjacent_row_glyph_quads_remain_in_their_respective_cell_bounds() {
        let first = clip_rect_to_cell([0.0, -3.0, 8.0, 24.0], [0.0, 0.0, 8.0, 20.0]).unwrap();
        let second = clip_rect_to_cell([0.0, 17.0, 8.0, 24.0], [0.0, 20.0, 8.0, 20.0]).unwrap();

        assert_eq!(first.1 + first.3, 20.0);
        assert_eq!(second.1, 20.0);
        assert!(first.1 + first.3 <= second.1);
    }

    #[test]
    fn reusable_instance_buffers_grow_geometrically_instead_of_per_frame() {
        assert_eq!(instance_buffer_capacity(1), 1);
        assert_eq!(instance_buffer_capacity(1_920), 2_048);
        assert_eq!(instance_buffer_capacity(2_048), 2_048);
        assert_eq!(instance_buffer_capacity(2_049), 4_096);

        assert!(!instance_buffer_requires_allocation(None, 0));
        assert!(instance_buffer_requires_allocation(None, 1_920));
        assert!(!instance_buffer_requires_allocation(Some(2_048), 1_920));
        assert!(!instance_buffer_requires_allocation(Some(2_048), 2_048));
        assert!(instance_buffer_requires_allocation(Some(2_048), 2_049));
    }

    #[test]
    fn instance_upload_ranges_cover_only_changed_spans_and_new_tail() {
        let previous = [1_u32, 2, 3, 4, 5];
        assert!(changed_instance_ranges(&previous, &previous).is_empty());
        assert_eq!(
            changed_instance_ranges(&previous, &[1, 9, 8, 4, 5]),
            vec![1..3]
        );
        assert_eq!(
            changed_instance_ranges(&previous, &[1, 2, 3, 4, 5, 6]),
            vec![5..6]
        );
        assert!(changed_instance_ranges(&previous, &[1, 2, 3]).is_empty());
    }

    #[test]
    #[ignore = "requires a GPU, system monospace font, and Noto Color Emoji"]
    fn native_gpu_color_emoji_dpi_and_warm_cache() {
        const WIDTH: u32 = 256;
        const HEIGHT: u32 = 128;
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        eprintln!("emoji GPU: {:?}", adapter.get_info());
        let (device, queue) =
            pollster::block_on(adapter.request_device(&crate::renderer_device_descriptor()))
                .unwrap();
        for format in [
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ] {
            for scale in [1.0, 1.25, 2.0] {
                for opacity in [1.0, 0.5] {
                    let texture = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("emoji regression target"),
                        size: wgpu::Extent3d {
                            width: WIDTH,
                            height: HEIGHT,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::COPY_SRC,
                        view_formats: &[],
                    });
                    let view = texture.create_view(&Default::default());
                    let mut resources = DrawResources::new(&device, format);
                    let mut fonts = FontSystem::load_system(FontRequest::default()).unwrap();
                    let metrics = fonts.cell_metrics(scale, 16.0).unwrap();
                    let mut state = TerminalState::new(TerminalDimensions::new(8, 2).unwrap());
                    terminal_core::TerminalParser::new()
                        .advance(&mut state, "A🚀B".as_bytes())
                        .unwrap();
                    let mut data = TerminalRenderData::from_terminal_with_opacity(
                        &state,
                        &Default::default(),
                        opacity,
                    );
                    data.cursor = None;
                    let target = if opacity < 1.0 {
                        resources
                            .composition_target(&device, SurfaceSize::new(WIDTH, HEIGHT).unwrap())
                    } else {
                        view.clone()
                    };
                    let ui = crate::UiRenderTheme::default();
                    let frame = || FrameContext {
                        target: &target,
                        surface_size: SurfaceSize::new(WIDTH, HEIGHT).unwrap(),
                        cell_metrics: metrics,
                        ui: &ui,
                    };
                    let cold = resources.draw(&device, &queue, frame(), &data, &mut fonts);
                    assert_eq!(cold.raster_calls, 3);
                    let color = resources
                        .glyphs
                        .iter()
                        .find(|g| g.content != 0)
                        .expect("visible color emoji quad");
                    let x = (color.rect[0] + 1.0) * WIDTH as f32 * 0.5;
                    let right = x + color.rect[2] * WIDTH as f32 * 0.5;
                    assert!(x >= metrics.width() as f32 - 0.01);
                    assert!(right <= 3.0 * metrics.width() as f32 + 0.01);
                    let start = std::time::Instant::now();
                    for _ in 0..100 {
                        let warm = resources.draw(&device, &queue, frame(), &data, &mut fonts);
                        assert_eq!(warm.raster_calls, 0);
                        assert_eq!(warm.shape_misses, 0);
                        assert_eq!(warm.buffer_allocations, 0);
                        assert_eq!(warm.buffer_writes, 0);
                    }
                    eprintln!(
                        "emoji warm 100 frames: {:?}, {format:?}, scale={scale}, opacity={opacity}",
                        start.elapsed()
                    );
                    if opacity < 1.0 {
                        resources.compose(&device, &queue, &view);
                    }
                    let pixels = read_rgba(&device, &queue, &texture);
                    let mut colored = 0;
                    for y in 0..metrics.height() {
                        for x in metrics.width()..3 * metrics.width() {
                            let p = &pixels[((y * WIDTH + x) * 4) as usize..][..4];
                            if p[0].abs_diff(p[1]) > 30 || p[1].abs_diff(p[2]) > 30 {
                                colored += 1;
                            }
                        }
                    }
                    assert!(
                        colored > 10,
                        "color pixels={colored}, scale={scale}, format={format:?}"
                    );
                    // Foreground changes must not tint cached intrinsic color.
                    for cell in &mut data.cells {
                        cell.foreground = crate::Rgba([1.0, 0.0, 0.0, 1.0]);
                    }
                    let tinted = resources.draw(&device, &queue, frame(), &data, &mut fonts);
                    assert_eq!(tinted.raster_calls, 0);
                    if opacity < 1.0 {
                        resources.compose(&device, &queue, &view);
                    }
                    let tinted = read_rgba(&device, &queue, &texture);
                    for y in 0..metrics.height() {
                        let start = ((y * WIDTH + metrics.width()) * 4) as usize;
                        let end = start + (metrics.width() * 2 * 4) as usize;
                        assert_eq!(&pixels[start..end], &tinted[start..end]);
                    }
                    // Selection still owns the full two-column cell span.
                    state.begin_selection(0, 1);
                    state.extend_selection(0, 3);
                    assert_eq!(state.selected_text().as_deref(), Some("🚀B"));
                    let mut selected = TerminalRenderData::from_terminal_with_opacity(
                        &state,
                        &Default::default(),
                        opacity,
                    );
                    selected.cursor = None;
                    resources.draw(&device, &queue, frame(), &selected, &mut fonts);
                    assert!(resources.glyphs.iter().any(|g| g.content != 0));
                }
            }
        }
    }

    fn read_rgba(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
        // Callers use 256-wide RGBA targets, satisfying copy-row alignment.
        let width = texture.width();
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("emoji readback"),
            size: u64::from(width * texture.height() * 4),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(texture.height()),
                },
            },
            texture.size(),
        );
        queue.submit(Some(encoder.finish()));
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                sender.send(result).unwrap()
            });
        device.poll(wgpu::PollType::wait()).unwrap();
        receiver.recv().unwrap().unwrap();
        let pixels = buffer.slice(..).get_mapped_range().to_vec();
        buffer.unmap();
        pixels
    }

    #[test]
    #[ignore = "requires a native GPU adapter"]
    fn native_gpu_validates_terminal_pipelines() {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: if cfg!(target_os = "windows") {
                wgpu::Backends::DX12
            } else {
                wgpu::Backends::all()
            },
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: None,
            ..Default::default()
        }))
        .expect("native GPU adapter");
        let (device, _) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("terminal renderer native smoke device"),
            ..crate::renderer_device_descriptor()
        }))
        .expect("native GPU device");

        let _resources = DrawResources::new(&device, wgpu::TextureFormat::Bgra8UnormSrgb);
    }

    #[test]
    #[ignore = "requires a GPU and JetBrains Mono variable font, or FONTCONFIG_FILE pointing to fixtures"]
    fn native_gpu_atlas_separates_variable_weight_instances() {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&crate::renderer_device_descriptor()))
                .unwrap();
        let mut resources = DrawResources::new(&device, wgpu::TextureFormat::Bgra8UnormSrgb);
        let mut fonts = FontSystem::load_system(FontRequest::Family("JetBrains Mono".into()))
            .unwrap()
            .with_bold_intensity(0.75);
        let light = fonts
            .shape_text_cached_with_bold("A", 20.0, true)
            .unwrap()
            .0;
        let light_bitmap = fonts
            .rasterize_glyph_with_bold(&light.glyphs()[0], 20.0, true)
            .unwrap();
        fonts = fonts.with_bold_intensity(1.25);
        let heavy = fonts
            .shape_text_cached_with_bold("A", 20.0, true)
            .unwrap()
            .0;
        let heavy_bitmap = fonts
            .rasterize_glyph_with_bold(&heavy.glyphs()[0], 20.0, true)
            .unwrap();
        assert_eq!(light.glyphs()[0].glyph_id(), heavy.glyphs()[0].glyph_id());
        assert_ne!(light_bitmap, heavy_bitmap);
        let key = |glyph: &crate::font::ShapedGlyph| super::GpuGlyphKey {
            face: glyph.face_cache_identity(),
            glyph_id: glyph.glyph_id(),
            pixels_per_em: 20.0f32.to_bits(),
            bold: true,
            italic: false,
        };
        let light_key = key(&light.glyphs()[0]);
        let heavy_key = key(&heavy.glyphs()[0]);
        assert_ne!(light_key, heavy_key);
        let a = resources
            .glyph_atlas
            .get_or_insert(&queue, light_key.clone(), &light_bitmap)
            .unwrap();
        let b = resources
            .glyph_atlas
            .get_or_insert(&queue, heavy_key.clone(), &heavy_bitmap)
            .unwrap();
        assert_ne!(a.slot, b.slot);
        assert_eq!(resources.glyph_atlas.get(&light_key).unwrap().slot, a.slot);
        assert_eq!(resources.glyph_atlas.get(&heavy_key).unwrap().slot, b.slot);
        let glyph = &light.glyphs()[0];
        let mut italic_key = light_key.clone();
        italic_key.italic = true;
        let italic_bitmap = fonts
            .rasterize_glyph_with_style(glyph, 20.0, true, true)
            .unwrap();
        let italic = resources
            .glyph_atlas
            .get_or_insert(&queue, italic_key.clone(), &italic_bitmap)
            .unwrap();
        assert_ne!(italic.slot, a.slot);
        assert_ne!(italic.slot, b.slot);
        assert_eq!(resources.glyph_atlas.get(&light_key).unwrap().slot, a.slot);
        assert_eq!(
            resources.glyph_atlas.get(&italic_key).unwrap().slot,
            italic.slot
        );
    }

    #[test]
    #[ignore = "requires a native GPU adapter and system monospace font"]
    fn native_gpu_renders_backgrounds_glyphs_underlines_and_cursor() {
        render_native_frame(1.0, wgpu::TextureFormat::Rgba8Unorm);
    }

    #[test]
    #[ignore = "requires a native GPU adapter"]
    fn native_gpu_background_opacity_and_srgb_premultiplication() {
        for format in [
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ] {
            for opacity in [0.0, 0.5, 0.85, 1.0] {
                render_native_frame(opacity, format);
            }
        }
    }

    fn render_native_frame(opacity: f32, format: wgpu::TextureFormat) {
        render_native_frame_with_palette(opacity, format, false);
    }

    #[test]
    #[ignore = "requires a native GPU adapter and system monospace font"]
    fn native_gpu_palette_preserves_surrounding_alpha() {
        for format in [
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ] {
            for opacity in [0.0, 0.5, 0.85, 1.0] {
                render_native_frame_with_palette(opacity, format, true);
            }
        }
    }

    #[test]
    #[ignore = "requires a native GPU adapter and system monospace font"]
    fn native_gpu_url_hints_preserve_surrounding_alpha() {
        for format in [
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ] {
            for opacity in [0.0, 0.85, 1.0] {
                render_native_frame_with_overlay(opacity, format, false, true);
            }
        }
    }

    fn render_native_frame_with_palette(opacity: f32, format: wgpu::TextureFormat, palette: bool) {
        render_native_frame_with_overlay(opacity, format, palette, false);
    }

    fn render_native_frame_with_overlay(
        opacity: f32,
        format: wgpu::TextureFormat,
        palette: bool,
        hints: bool,
    ) {
        const WIDTH: u32 = 768;
        const HEIGHT: u32 = 600;
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: if cfg!(target_os = "windows") {
                wgpu::Backends::DX12
            } else {
                wgpu::Backends::all()
            },
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: None,
            ..Default::default()
        }))
        .expect("native GPU adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("terminal renderer native frame smoke device"),
            ..crate::renderer_device_descriptor()
        }))
        .expect("native GPU device");
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("terminal renderer native frame smoke texture"),
            size: wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut terminal = TerminalState::new(TerminalDimensions::new(8, 24).unwrap());
        terminal.set_background_color(CellColor::Rgb {
            red: 15,
            green: 45,
            blue: 120,
        });
        terminal.set_foreground_color(CellColor::Rgb {
            red: 230,
            green: 20,
            blue: 20,
        });
        terminal.print_character('A').unwrap();
        terminal.set_foreground_color(CellColor::Rgb {
            red: 20,
            green: 230,
            blue: 20,
        });
        terminal.set_underline_style(UnderlineStyle::Enabled);
        terminal.print_character('B').unwrap();
        terminal.set_underline_style(UnderlineStyle::Disabled);
        terminal.set_foreground_color(CellColor::Rgb {
            red: 230,
            green: 30,
            blue: 230,
        });
        terminal.print_character('界').unwrap();
        terminal.set_foreground_color(CellColor::Rgb {
            red: 230,
            green: 200,
            blue: 40,
        });
        terminal.print_character('e').unwrap();
        terminal.print_character('\u{301}').unwrap();
        terminal.line_feed();
        terminal.carriage_return();
        terminal.print_character('A').unwrap();
        terminal.set_text_intensity(terminal_core::TextIntensity::Bold);
        terminal.print_character('A').unwrap();
        terminal.set_text_intensity(terminal_core::TextIntensity::Normal);
        terminal.print_character('A').unwrap();

        // Normal / italic / normal exercises atlas identity after a reset.
        // Further columns cover dim, conceal, and strike-through in the GPU path.
        terminal.set_cursor_position(3, 0).unwrap();
        terminal.reset_rendition();
        terminal_core::TerminalParser::new()
            .advance(
                &mut terminal,
                b"A\x1b[3mA\x1b[23mA\x1b[2mA\x1b[22;8mA\x1b[28;9mA",
            )
            .unwrap();
        terminal.reset_rendition();
        assert_eq!(terminal.screen().cell(0, 2).unwrap().character(), '界');
        let swatches = [(32, 32, 32), (57, 57, 57), (68, 68, 68), (120, 169, 255)];
        for (row, &(red, green, blue)) in swatches.iter().enumerate() {
            terminal.set_cursor_position(row + 16, 0).unwrap();
            terminal.set_background_color(CellColor::Rgb { red, green, blue });
            terminal.print_character(' ').unwrap();
        }
        let theme = crate::RenderTheme {
            background: crate::Rgba([0.2, 0.3, 0.4, 1.0]),
            ..Default::default()
        };
        let mut data = TerminalRenderData::from_terminal_with_opacity(&terminal, &theme, opacity);
        if palette {
            crate::palette::dim(&mut data, &theme);
        }
        let wide_cell = data
            .cells
            .iter()
            .find(|cell| cell.character == '界')
            .expect("wide lead cell");
        assert_eq!(wide_cell.width, 2);
        if !palette {
            assert_eq!(
                wide_cell.foreground.0,
                [230.0 / 255.0, 30.0 / 255.0, 230.0 / 255.0, 1.0]
            );
        }
        let mut resources = DrawResources::new(&device, format);
        let mut fonts =
            FontSystem::load_system(FontRequest::default()).expect("system monospace font");
        let pixels_per_em = HEIGHT as f32 / data.rows as f32;
        let wide_shaped = fonts.shape_text("界", pixels_per_em).unwrap();
        let wide_glyph = wide_shaped.glyphs().first().expect("wide shaped glyph");
        let wide_bitmap = fonts.rasterize_glyph(wide_glyph, pixels_per_em).unwrap();
        assert!(wide_bitmap.width() <= 64 && wide_bitmap.height() <= 64);
        assert!(
            resources
                .glyph_atlas
                .get_or_insert(
                    &queue,
                    GpuGlyphKey {
                        face: wide_glyph.face_cache_identity(),
                        glyph_id: wide_bitmap.glyph_id(),
                        pixels_per_em: pixels_per_em.to_bits(),
                        bold: false,
                        italic: false,
                    },
                    &wide_bitmap,
                )
                .is_some()
        );
        let target = if opacity < 1.0 {
            resources.composition_target(&device, SurfaceSize::new(WIDTH, HEIGHT).unwrap())
        } else {
            view.clone()
        };
        resources.draw(
            &device,
            &queue,
            FrameContext {
                target: &target,
                surface_size: SurfaceSize::new(WIDTH, HEIGHT).unwrap(),
                cell_metrics: crate::CellMetrics::from_physical(96, 25, 25.0),
                ui: &crate::UiRenderTheme::default(),
            },
            &data,
            &mut fonts,
        );

        if hints {
            let labels = [crate::HintLabel {
                row: 10,
                start: 0,
                end: 1,
                text: "AJ".into(),
            }];
            let drawing = crate::hints::drawings(
                &labels,
                [0, 0, WIDTH, HEIGHT],
                crate::CellMetrics::from_physical(96, 25, 25.0),
                fonts.cell_metrics(1.0, 11.0).unwrap(),
                &theme,
                opacity,
            )
            .remove(0);
            resources.draw_pane(
                &device,
                &queue,
                FrameContext {
                    target: &target,
                    surface_size: SurfaceSize::new(WIDTH, HEIGHT).unwrap(),
                    cell_metrics: drawing.metrics,
                    ui: &theme.ui,
                },
                &drawing.data,
                &mut fonts,
                super::PaneDraw {
                    rect: drawing.rect,
                    clear: false,
                    focused_border: false,
                    text_offset: drawing.text_offset,
                    rectangles: &drawing.rectangles,
                },
            );
        }
        if palette {
            let panel = crate::PalettePanel {
                title: "Command Palette",
                placeholder: "Search commands…",
                empty_message: "No matching commands",
                footer: "↑↓ Navigate   Enter Run   Esc Close",
                query: "terminal".into(),
                entries: vec!["New Tab".into(), "Split Vertical".into()],
                selected: 1,
                ..crate::PalettePanel::default()
            };
            let drawing = crate::palette::drawing(
                &panel,
                [WIDTH, HEIGHT],
                32,
                1.0,
                fonts.cell_metrics(1.0, 13.0).unwrap(),
                &theme,
                opacity,
            )
            .unwrap();
            resources.draw_pane(
                &device,
                &queue,
                FrameContext {
                    target: &target,
                    surface_size: SurfaceSize::new(WIDTH, HEIGHT).unwrap(),
                    cell_metrics: drawing.metrics,
                    ui: &theme.ui,
                },
                &drawing.data,
                &mut fonts,
                super::PaneDraw {
                    rect: drawing.rect,
                    clear: false,
                    focused_border: false,
                    text_offset: drawing.text_offset,
                    rectangles: &drawing.rectangles,
                },
            );
        }
        if opacity < 1.0 {
            resources.compose(&device, &queue, &view);
        }
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terminal renderer native frame smoke readback"),
            size: (WIDTH * HEIGHT * 4) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("terminal renderer native frame smoke copy"),
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(WIDTH * 4),
                    rows_per_image: Some(HEIGHT),
                },
            },
            texture.size(),
        );
        queue.submit(Some(encoder.finish()));
        let slice = readback.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap()
        });
        device.poll(wgpu::PollType::wait()).expect("device poll");
        receiver
            .recv()
            .expect("map callback")
            .expect("readback map");
        let bytes = slice.get_mapped_range();
        let (pixels, remainder) = bytes.as_chunks::<4>();
        assert!(remainder.is_empty(), "RGBA readback must have whole pixels");
        // Theme and terminal channels are encoded sRGB, so stored flat pixels
        // must retain their byte values on both linear and sRGB targets.
        let expected = [
            0.2 * opacity * 255.0,
            0.3 * opacity * 255.0,
            0.4 * opacity * 255.0,
            opacity * 255.0,
        ];
        let background = pixels[(HEIGHT as usize - 1) * WIDTH as usize + WIDTH as usize - 1];
        for (actual, expected) in background.into_iter().zip(expected) {
            assert!(
                (f32::from(actual) - expected).abs() <= 2.0,
                "format={format:?} opacity={opacity} background={background:?} expected={expected}"
            );
        }
        if !palette && !hints {
            for (row, &(red, green, blue)) in swatches.iter().enumerate() {
                let pixel = pixels[(row + 16) * 25 * WIDTH as usize];
                for (actual, channel) in pixel[..3].iter().zip([red, green, blue]) {
                    assert!(
                        (f32::from(*actual) - f32::from(channel) * opacity).abs() <= 2.0,
                        "RGB swatch {red},{green},{blue} format={format:?} opacity={opacity}: {pixel:?}"
                    );
                }
            }
            // Underline coverage is solid: foreground must retain RGB and alpha
            // independently of the terminal background opacity.
            let underline = pixels[24 * WIDTH as usize + 100];
            for (actual, expected) in underline.into_iter().zip([20, 230, 20, 255]) {
                assert!(
                    (i32::from(actual) - expected).abs() <= 2,
                    "underline {underline:?}"
                );
            }
        }
        // Colored cell background replaces the clear; alpha must not accumulate.
        let colored = pixels[0];
        assert!((f32::from(colored[3]) - opacity * 255.0).abs() <= 1.0);
        if hints {
            let label_background = pixels[260 * WIDTH as usize + 98];
            let expected_alpha = (opacity.max(0.94) + opacity * (1.0 - opacity.max(0.94))) * 255.0;
            assert!(
                (f32::from(label_background[3]) - expected_alpha).abs() <= 2.0,
                "hint alpha {label_background:?}, opacity={opacity}"
            );
        }
        if palette {
            let panel_background = pixels[50 * WIDTH as usize + 120];
            let expected_alpha = (opacity.max(0.94) + opacity * (1.0 - opacity.max(0.94))) * 255.0;
            assert!(
                (f32::from(panel_background[3]) - expected_alpha).abs() <= 2.0,
                "palette background {panel_background:?}, opacity {opacity}"
            );
            assert!(
                pixels[80 * WIDTH as usize + 114..180 * WIDTH as usize + 650]
                    .iter()
                    .any(|p| p[3] > panel_background[3])
                    || opacity == 1.0
            );
        }
        assert!(
            pixels
                .iter()
                .any(|pixel| pixel[3] > (opacity * 255.0) as u8)
                || opacity == 1.0,
            "foreground coverage must remain independent of background opacity"
        );
        if opacity == 1.0 && !palette {
            // Same glyph and color, normal / bold / normal. This exercises both
            // atlas weights and proves the reset cannot reuse the bold bitmap.
            let coverage = |column: usize| {
                (25..50)
                    .flat_map(|row| {
                        (column * 96..(column + 1) * 96)
                            .map(move |x| pixels[row * WIDTH as usize + x])
                    })
                    .map(|p| u64::from(p[0]) + u64::from(p[1]))
                    .sum::<u64>()
            };

            let cell_pixels = |column: usize| {
                (75..100)
                    .flat_map(|row| {
                        (column * 96..(column + 1) * 96)
                            .map(move |x| pixels[row * WIDTH as usize + x])
                    })
                    .collect::<Vec<_>>()
            };
            let normal = cell_pixels(0);
            assert_ne!(normal, cell_pixels(1), "italic must change drawn pixels");
            assert_eq!(
                normal,
                cell_pixels(2),
                "italic reset must reuse upright pixels"
            );
            let brightness = |cell: Vec<[u8; 4]>| cell.iter().map(|p| u64::from(p[0])).sum::<u64>();
            assert!(
                brightness(cell_pixels(3)) < brightness(normal.clone()),
                "faint must reduce brightness"
            );
            assert!(
                cell_pixels(4)
                    .iter()
                    .all(|p| *p == pixels[75 * WIDTH as usize + 4 * 96]),
                "conceal must suppress glyphs"
            );
            assert!(
                brightness(cell_pixels(5)) > brightness(normal),
                "strike-through must add pixels"
            );
            assert!(coverage(1) > coverage(0));
            assert_eq!(coverage(0), coverage(2));
            assert!(pixels.iter().any(|pixel| pixel[2] > 80));
            assert!(
                pixels
                    .iter()
                    .any(|pixel| i16::from(pixel[0]) > i16::from(pixel[1]) * 2)
            );
            assert!(
                pixels
                    .iter()
                    .any(|pixel| i16::from(pixel[1]) > i16::from(pixel[0]) * 2)
            );
            assert!(pixels.iter().any(|pixel| {
                pixel[0] > 70
                    && pixel[1] > 70
                    && pixel[2] > 70
                    && (pixel[0] as i16 - pixel[1] as i16).abs() < 20
                    && (pixel[1] as i16 - pixel[2] as i16).abs() < 20
            }));
            assert!(
                pixels
                    .iter()
                    .any(|pixel| pixel[0] > 150 && pixel[1] < 100 && pixel[2] > 150)
            );
        }
        drop(bytes);
        readback.unmap();
        if !palette && !hints {
            // The no-pane/empty-frame clear must agree with normal drawing,
            // both directly to a surface and through translucent composition.
            for intermediate in [false, true] {
                let target = if intermediate {
                    resources.composition_target(&device, SurfaceSize::new(WIDTH, HEIGHT).unwrap())
                } else {
                    view.clone()
                };
                crate::clear_background(
                    &device,
                    &queue,
                    &target,
                    [0.2, 0.3, 0.4, opacity],
                    format,
                    !intermediate,
                );
                if intermediate {
                    resources.compose(&device, &queue, &view);
                }
                let mut encoder = device.create_command_encoder(&Default::default());
                encoder.copy_texture_to_buffer(
                    texture.as_image_copy(),
                    wgpu::TexelCopyBufferInfo {
                        buffer: &readback,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(WIDTH * 4),
                            rows_per_image: Some(HEIGHT),
                        },
                    },
                    texture.size(),
                );
                queue.submit(Some(encoder.finish()));
                let slice = readback.slice(..);
                let (sender, receiver) = std::sync::mpsc::channel();
                slice.map_async(wgpu::MapMode::Read, move |result| {
                    sender.send(result).unwrap()
                });
                device.poll(wgpu::PollType::wait()).unwrap();
                receiver.recv().unwrap().unwrap();
                let bytes = slice.get_mapped_range();
                for (actual, expected) in bytes[..4].iter().zip(expected) {
                    assert!(
                        (f32::from(*actual) - expected).abs() <= 2.0,
                        "empty clear format={format:?} opacity={opacity} intermediate={intermediate}: {:?}",
                        &bytes[..4]
                    );
                }
                drop(bytes);
                readback.unmap();
            }
        }
    }
}
