use std::collections::{HashMap, VecDeque};

use bytemuck::{Pod, Zeroable};

use crate::{FontSystem, TerminalRenderData, diagnostics_enabled, emit_diagnostic};

const ATLAS_WIDTH: u32 = 1024;
const ATLAS_HEIGHT: u32 = 1024;
const ATLAS_SLOT_SIZE: u32 = 64;
const ATLAS_COLUMNS: u32 = ATLAS_WIDTH / ATLAS_SLOT_SIZE;
const ATLAS_CAPACITY: usize = (ATLAS_COLUMNS * (ATLAS_HEIGHT / ATLAS_SLOT_SIZE)) as usize;

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
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct GpuGlyphKey {
    face: String,
    glyph_id: u16,
    pixels_per_em: u32,
}

#[derive(Clone, Copy)]
struct AtlasEntry {
    slot: usize,
    width: u32,
    height: u32,
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
    fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
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
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
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
        if let Some(entry) = self.entries.get(&key).copied() {
            self.touch(&key);
            return Some(entry);
        }
        let slot = self.allocate_slot();
        let x = (slot as u32 % ATLAS_COLUMNS) * ATLAS_SLOT_SIZE;
        let y = (slot as u32 / ATLAS_COLUMNS) * ATLAS_SLOT_SIZE;
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            bitmap.pixels(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bitmap.width()),
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

pub(super) struct DrawResources {
    rect_pipeline: wgpu::RenderPipeline,
    glyph_pipeline: wgpu::RenderPipeline,
    glyph_atlas: GlyphAtlas,
    rectangles: Vec<RectInstance>,
    glyphs: Vec<GlyphInstance>,
    rectangle_buffer: Option<InstanceBuffer<RectInstance>>,
    glyph_buffer: Option<InstanceBuffer<GlyphInstance>>,
    cursor_buffer: Option<InstanceBuffer<RectInstance>>,
}

/// Per-frame draw inputs retained only for concise lifecycle diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RenderInstanceCounts {
    pub(super) backgrounds: usize,
    pub(super) glyphs: usize,
    pub(super) decorations: usize,
    pub(super) cursor: usize,
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
}

struct InstanceBuffer<T> {
    buffer: wgpu::Buffer,
    capacity: usize,
    marker: std::marker::PhantomData<T>,
}

impl<T: Pod> InstanceBuffer<T> {
    fn upload(
        current: &mut Option<Self>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &'static str,
        instances: &[T],
    ) -> bool {
        if instances.is_empty() {
            return false;
        }
        let required = instances.len();
        let allocation_required = instance_buffer_requires_allocation(
            current.as_ref().map(|buffer| buffer.capacity),
            required,
        );
        if allocation_required {
            let capacity = instance_buffer_capacity(required);
            *current = Some(Self {
                buffer: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(label),
                    size: (capacity * std::mem::size_of::<T>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                capacity,
                marker: std::marker::PhantomData,
            });
        }
        let buffer = current
            .as_ref()
            .expect("non-empty instances allocate a buffer");
        queue.write_buffer(&buffer.buffer, 0, bytemuck::cast_slice(instances));
        allocation_required
    }
}

pub(super) struct FrameContext<'a> {
    pub(super) target: &'a wgpu::TextureView,
    pub(super) surface_size: crate::SurfaceSize,
    pub(super) cell_metrics: crate::CellMetrics,
}

impl DrawResources {
    pub(super) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terminal renderer shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("terminal.wgsl").into()),
        });
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
        let rect_pipeline = pipeline(
            device,
            &shader,
            "rect_vertex",
            "rect_fragment",
            format,
            &[],
            rect_layout(),
        );
        let glyph_pipeline = pipeline(
            device,
            &shader,
            "glyph_vertex",
            "glyph_fragment",
            format,
            &[&glyph_bind_group_layout],
            glyph_vertex_layout(),
        );
        let glyph_atlas = GlyphAtlas::new(device, &glyph_bind_group_layout);
        Self {
            rect_pipeline,
            glyph_pipeline,
            glyph_atlas,
            rectangles: Vec::new(),
            glyphs: Vec::new(),
            rectangle_buffer: None,
            glyph_buffer: None,
            cursor_buffer: None,
        }
    }

    pub(super) fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: FrameContext<'_>,
        data: &TerminalRenderData,
        font_system: &mut FontSystem,
    ) -> RenderWork {
        let cell_width = frame.cell_metrics.width() as f32;
        let cell_height = frame.cell_metrics.height() as f32;
        self.rectangles.clear();
        self.glyphs.clear();
        self.rectangles
            .reserve(data.cells.len().saturating_sub(self.rectangles.capacity()));
        let mut previous_geometry: Option<GlyphGeometrySample> = None;
        let mut geometry_pair_logged = false;
        for cell in &data.cells {
            let x = cell.column as f32 * cell_width;
            let y = cell.row as f32 * cell_height;
            let width = cell.width as f32 * cell_width;
            self.rectangles.push(RectInstance {
                rect: to_clip_rect(x, y, width, cell_height, frame.surface_size),
                color: cell.background.0,
            });
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
            let Ok(shaped) = font_system.shape_text(&cell.text, pixels_per_em) else {
                continue;
            };
            for shaped_glyph in shaped.glyphs() {
                let Ok(bitmap) = font_system.rasterize_glyph(shaped_glyph, pixels_per_em) else {
                    continue;
                };
                let key = GpuGlyphKey {
                    face: shaped_glyph.face_cache_identity(),
                    glyph_id: bitmap.glyph_id(),
                    pixels_per_em: pixels_per_em.to_bits(),
                };
                let Some(entry) = self.glyph_atlas.get_or_insert(queue, key, &bitmap) else {
                    continue;
                };
                let slot_x = (entry.slot as u32 % ATLAS_COLUMNS) * ATLAS_SLOT_SIZE;
                let slot_y = (entry.slot as u32 / ATLAS_COLUMNS) * ATLAS_SLOT_SIZE;
                let [glyph_x, glyph_y] = glyph_origin(
                    x,
                    y,
                    frame.cell_metrics.baseline() as f32,
                    bitmap.bearing_x(),
                    bitmap.bearing_y(),
                    shaped_glyph.offset_x(),
                    shaped_glyph.offset_y(),
                );
                let Some((clipped_x, clipped_y, clipped_width, clipped_height)) = clip_rect_to_cell(
                    [glyph_x, glyph_y, entry.width as f32, entry.height as f32],
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
                        (slot_x as f32 + clipped_x - glyph_x) / ATLAS_WIDTH as f32,
                        (slot_y as f32 + clipped_y - glyph_y) / ATLAS_HEIGHT as f32,
                        (slot_x as f32 + clipped_x - glyph_x + clipped_width) / ATLAS_WIDTH as f32,
                        (slot_y as f32 + clipped_y - glyph_y + clipped_height)
                            / ATLAS_HEIGHT as f32,
                    ],
                    color: cell.foreground.0,
                });
                if diagnostics_enabled() && !geometry_pair_logged {
                    let sample = GlyphGeometrySample {
                        row: cell.row as u32,
                        cell_y: y,
                        cell_height,
                        ascent: frame.cell_metrics.ascent(),
                        descent: frame.cell_metrics.descent(),
                        baseline: frame.cell_metrics.baseline() as f32,
                        placement_top: bitmap.bearing_y(),
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
        let cursor = data.cursor.map(|cursor| RectInstance {
            rect: to_clip_rect(
                cursor.column as f32 * cell_width,
                cursor.row as f32 * cell_height,
                cell_width,
                cell_height,
                frame.surface_size,
            ),
            color: [0.8, 0.8, 0.8, 0.45],
        });
        let rectangle_count = self.rectangles.len();
        let glyph_count = self.glyphs.len();
        let rectangle_buffer_allocated = InstanceBuffer::upload(
            &mut self.rectangle_buffer,
            device,
            queue,
            "terminal rectangle instances",
            &self.rectangles,
        );
        let glyph_buffer_allocated = InstanceBuffer::upload(
            &mut self.glyph_buffer,
            device,
            queue,
            "terminal glyph instances",
            &self.glyphs,
        );
        let cursor_buffer_allocated = match cursor {
            Some(cursor) => InstanceBuffer::upload(
                &mut self.cursor_buffer,
                device,
                queue,
                "terminal cursor instance",
                &[cursor],
            ),
            None => false,
        };
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
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
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
                pass.draw(0..6, 0..rectangle_count as u32);
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
            if cursor.is_some() {
                pass.set_pipeline(&self.rect_pipeline);
                pass.set_vertex_buffer(
                    0,
                    self.cursor_buffer
                        .as_ref()
                        .expect("cursor instance has a buffer")
                        .buffer
                        .slice(..),
                );
                pass.draw(0..6, 0..1);
            }
        }
        queue.submit(Some(encoder.finish()));
        RenderWork {
            instances: RenderInstanceCounts {
                backgrounds: data.cells.len(),
                glyphs: glyph_count,
                decorations: rectangle_count - data.cells.len(),
                cursor: usize::from(cursor.is_some()),
            },
            buffer_allocations: usize::from(rectangle_buffer_allocated)
                + usize::from(glyph_buffer_allocated)
                + usize::from(cursor_buffer_allocated),
            buffer_writes: usize::from(rectangle_count != 0)
                + usize::from(glyph_count != 0)
                + usize::from(cursor.is_some()),
            queue_submissions: 1,
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
    const ATTRIBUTES: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4];
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
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
        DrawResources, FrameContext, GpuGlyphKey, clip_rect_to_cell, glyph_origin,
        instance_buffer_capacity, instance_buffer_requires_allocation, to_clip_rect,
    };
    use crate::{FontRequest, FontSystem, SurfaceSize, TerminalRenderData};
    use terminal_core::{CellColor, TerminalDimensions, TerminalState, UnderlineStyle};

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
    #[ignore = "requires a native GPU adapter"]
    fn native_gpu_validates_terminal_pipelines() {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: None,
            ..Default::default()
        }))
        .expect("native GPU adapter");
        let (device, _) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("terminal renderer native smoke device"),
            ..Default::default()
        }))
        .expect("native GPU device");

        let _resources = DrawResources::new(&device, wgpu::TextureFormat::Bgra8UnormSrgb);
    }

    #[test]
    #[ignore = "requires a native GPU adapter and system monospace font"]
    fn native_gpu_renders_backgrounds_glyphs_underlines_and_cursor() {
        const WIDTH: u32 = 768;
        const HEIGHT: u32 = 600;
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: None,
            ..Default::default()
        }))
        .expect("native GPU adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("terminal renderer native frame smoke device"),
            ..Default::default()
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
            format: wgpu::TextureFormat::Rgba8Unorm,
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
        assert_eq!(terminal.screen().cell(0, 2).unwrap().character(), '界');
        let data = TerminalRenderData::from_terminal(&terminal);
        let wide_cell = data
            .cells
            .iter()
            .find(|cell| cell.character == '界')
            .expect("wide lead cell");
        assert_eq!(wide_cell.width, 2);
        assert_eq!(
            wide_cell.foreground.0,
            [230.0 / 255.0, 30.0 / 255.0, 230.0 / 255.0, 1.0]
        );
        let mut resources = DrawResources::new(&device, wgpu::TextureFormat::Rgba8Unorm);
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
                    },
                    &wide_bitmap,
                )
                .is_some()
        );
        resources.draw(
            &device,
            &queue,
            FrameContext {
                target: &view,
                surface_size: SurfaceSize::new(WIDTH, HEIGHT).unwrap(),
                cell_metrics: crate::CellMetrics::from_physical(96, 25, 25.0),
            },
            &data,
            &mut fonts,
        );

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
        drop(bytes);
        readback.unmap();
    }
}
