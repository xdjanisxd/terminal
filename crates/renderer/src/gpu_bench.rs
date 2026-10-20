//! Opt-in, fixed-work offscreen measurements. No wall-clock test assertions.
use super::*;
use crate::{FontRequest, RenderTheme, SurfaceSize, UiRenderTheme};
use std::hint::black_box;
use terminal_core::{TerminalDimensions, TerminalParser, TerminalState};

#[test]
#[ignore = "requires a GPU and system monospace/emoji fonts; release measurements only"]
fn renderer_workload_baselines() {
    const FRAMES: usize = 100;
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    eprintln!("adapter={:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&crate::renderer_device_descriptor())).unwrap();
    let mut fonts = FontSystem::load_system(FontRequest::default()).unwrap();
    let metrics = fonts.cell_metrics(1.0, 16.0).unwrap();
    let ui = UiRenderTheme::default();
    for (name, columns, rows, text, opacity) in [
        ("sparse", 80, 30, "prompt> status running", 1.0),
        (
            "dense",
            160,
            50,
            "abcdefghijklmnopqrstuvwxyz0123456789",
            1.0,
        ),
        ("unicode", 80, 30, "Ae\u{301}界🚀👩‍💻", 1.0),
        ("transparent", 80, 30, "\x1b[44mcolored\x1b[0m text", 0.5),
        (
            "blink",
            160,
            50,
            "abcdefghijklmnopqrstuvwxyz0123456789",
            1.0,
        ),
        ("theme", 80, 30, "\x1b[31mstatus\x1b[0m running", 1.0),
        (
            "three_panes",
            80,
            30,
            "status running abcdefghijklmnopqrstuvwxyz",
            1.0,
        ),
    ] {
        let size = SurfaceSize::new(
            columns as u32 * metrics.width(),
            rows as u32 * metrics.height(),
        )
        .unwrap();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("renderer benchmark"),
            size: wgpu::Extent3d {
                width: size.width(),
                height: size.height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let mut resources = DrawResources::new(&device, wgpu::TextureFormat::Rgba8Unorm);
        let mut state = TerminalState::new(TerminalDimensions::new(columns, rows).unwrap());
        let mut parser = TerminalParser::new();
        for row in 0..rows {
            let repeats = if name == "dense" || name == "blink" {
                4
            } else {
                1
            };
            parser
                .advance(
                    &mut state,
                    format!("\x1b[{};1H{}", row + 1, text.repeat(repeats)).as_bytes(),
                )
                .unwrap();
        }
        let theme = RenderTheme::default();
        let alternate = RenderTheme {
            foreground: crate::Rgba([0.2, 0.8, 0.4, 1.0]),
            ..theme.clone()
        };
        let mut data = TerminalRenderData::from_terminal_with_opacity(&state, &theme, opacity);
        let target = if opacity < 1.0 {
            resources.composition_target(&device, size)
        } else {
            view.clone()
        };
        let frame = || FrameContext {
            target: &target,
            surface_size: size,
            cell_metrics: metrics,
            ui: &ui,
        };
        for _ in 0..5 {
            resources.draw(&device, &queue, frame(), &data, &mut fonts);
        }
        device.poll(wgpu::PollType::Wait).unwrap();
        let mut projection = Duration::ZERO;
        let mut generation = Duration::ZERO;
        let mut upload = Duration::ZERO;
        let mut submission = Duration::ZERO;
        let mut bytes = 0;
        let mut rasters = 0;
        let start = Instant::now();
        for index in 0..FRAMES {
            let panes = if name == "three_panes" { 3 } else { 1 };
            for _ in 0..panes {
                let started = Instant::now();
                data = TerminalRenderData::from_terminal_with_opacity(
                    &state,
                    if name == "theme" && index % 2 == 0 {
                        &alternate
                    } else {
                        &theme
                    },
                    opacity,
                );
                if name == "blink" && index % 2 == 0 {
                    data.cursor = None;
                }
                projection += started.elapsed();
                let work = resources.draw(&device, &queue, frame(), &data, &mut fonts);
                generation += work.instance_generation;
                upload += work.buffer_upload;
                submission += work.submission;
                bytes += work.buffer_bytes;
                rasters += work.raster_calls;
                assert_eq!(work.buffer_allocations, 0);
            }
            if opacity < 1.0 {
                resources.compose(&device, &queue, &view);
            }
            // Bound in-flight GPU work. Wall time includes this wait; stage times do not.
            device.poll(wgpu::PollType::Wait).unwrap();
        }
        eprintln!(
            "workload={name} frames={FRAMES} cells={} projection_ns={} generation_ns={} upload_ns={} submission_ns={} wall_ns={} bytes={bytes} rasters={rasters}",
            data.cells.len(),
            projection.as_nanos(),
            generation.as_nanos(),
            upload.as_nanos(),
            submission.as_nanos(),
            start.elapsed().as_nanos()
        );
    }
    let shaped = fonts.shape_text("A", 16.0).unwrap();
    let glyph = &shaped.glyphs()[0];
    let start = Instant::now();
    for _ in 0..100_000 {
        black_box(black_box(glyph).face_cache_identity());
    }
    eprintln!(
        "face_identity operations=100000 elapsed_ns={}",
        start.elapsed().as_nanos()
    );
}
