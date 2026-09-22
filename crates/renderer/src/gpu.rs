use std::collections::{HashMap, VecDeque};

use bytemuck::{Pod, Zeroable};

use crate::{FontSystem, TerminalRenderData};

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
        }
    }

    pub(super) fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        data: &TerminalRenderData,
        font_system: &mut FontSystem,
        surface_size: crate::SurfaceSize,
    ) {
        let cell_width = surface_size.width() as f32 / data.columns.max(1) as f32;
        let cell_height = surface_size.height() as f32 / data.rows.max(1) as f32;
        let mut rectangles = Vec::with_capacity(data.cells.len() + data.cells.len() / 4 + 1);
        let mut glyphs = Vec::new();
        for cell in &data.cells {
            let x = cell.column as f32 * cell_width;
            let y = cell.row as f32 * cell_height;
            let width = cell.width as f32 * cell_width;
            rectangles.push(RectInstance {
                rect: to_clip_rect(x, y, width, cell_height, surface_size),
                color: cell.background.0,
            });
            if cell.underline {
                rectangles.push(RectInstance {
                    rect: to_clip_rect(x, y + cell_height - 1.0, width, 1.0, surface_size),
                    color: cell.foreground.0,
                });
            }
            if cell.character == ' ' || cell.character == '\0' {
                continue;
            }
            let pixels_per_em = cell_height.max(1.0);
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
                let glyph_x = x + bitmap.bearing_x() as f32 + shaped_glyph.offset_x();
                let glyph_y = y + cell_height - bitmap.bearing_y() as f32 - shaped_glyph.offset_y();
                glyphs.push(GlyphInstance {
                    rect: to_clip_rect(
                        glyph_x,
                        glyph_y,
                        entry.width as f32,
                        entry.height as f32,
                        surface_size,
                    ),
                    uv: [
                        slot_x as f32 / ATLAS_WIDTH as f32,
                        slot_y as f32 / ATLAS_HEIGHT as f32,
                        (slot_x + entry.width) as f32 / ATLAS_WIDTH as f32,
                        (slot_y + entry.height) as f32 / ATLAS_HEIGHT as f32,
                    ],
                    color: cell.foreground.0,
                });
            }
        }
        let cursor = data.cursor.map(|cursor| RectInstance {
            rect: to_clip_rect(
                cursor.column as f32 * cell_width,
                cursor.row as f32 * cell_height,
                cell_width,
                cell_height,
                surface_size,
            ),
            color: [0.8, 0.8, 0.8, 0.45],
        });
        let rect_buffer = buffer(device, "terminal rectangle instances", &rectangles);
        let glyph_buffer = buffer(device, "terminal glyph instances", &glyphs);
        let cursor_buffer =
            cursor.map(|cursor| buffer(device, "terminal cursor instance", &[cursor]));
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("terminal frame encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("terminal render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
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
            if !rectangles.is_empty() {
                pass.set_pipeline(&self.rect_pipeline);
                pass.set_vertex_buffer(0, rect_buffer.slice(..));
                pass.draw(0..6, 0..rectangles.len() as u32);
            }
            if !glyphs.is_empty() {
                pass.set_pipeline(&self.glyph_pipeline);
                pass.set_bind_group(0, &self.glyph_atlas.bind_group, &[]);
                pass.set_vertex_buffer(0, glyph_buffer.slice(..));
                pass.draw(0..6, 0..glyphs.len() as u32);
            }
            if let Some(cursor_buffer) = &cursor_buffer {
                pass.set_pipeline(&self.rect_pipeline);
                pass.set_vertex_buffer(0, cursor_buffer.slice(..));
                pass.draw(0..6, 0..1);
            }
        }
        queue.submit(Some(encoder.finish()));
    }
}

fn buffer<T: Pod>(device: &wgpu::Device, label: &'static str, data: &[T]) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(data),
        usage: wgpu::BufferUsages::VERTEX,
    })
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

#[cfg(test)]
mod tests {
    use super::{DrawResources, to_clip_rect};
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
        const WIDTH: u32 = 640;
        const HEIGHT: u32 = 160;
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
        let mut terminal = TerminalState::new(TerminalDimensions::new(8, 2).unwrap());
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
        terminal.print_character('界').unwrap();
        terminal.print_character('e').unwrap();
        terminal.print_character('\u{301}').unwrap();
        terminal.line_feed();
        terminal.carriage_return();
        let data = TerminalRenderData::from_terminal(&terminal);
        let mut resources = DrawResources::new(&device, wgpu::TextureFormat::Rgba8Unorm);
        let mut fonts =
            FontSystem::load_system(FontRequest::default()).expect("system monospace font");
        resources.draw(
            &device,
            &queue,
            &view,
            &data,
            &mut fonts,
            SurfaceSize::new(WIDTH, HEIGHT).unwrap(),
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
        drop(bytes);
        readback.unmap();
    }
}
