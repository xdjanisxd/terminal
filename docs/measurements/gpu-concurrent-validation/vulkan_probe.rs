use std::sync::{Arc, Barrier};

fn main() {
    let workers: usize = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "7".into())
        .parse()
        .unwrap();
    assert!(workers > 0);
    let gate = Arc::new(Barrier::new(workers));
    let threads: Vec<_> = (0..workers)
        .map(|index| {
            let gate = Arc::clone(&gate);
            std::thread::spawn(move || {
                gate.wait();
                for cycle in 0..4 {
                    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                        backends: wgpu::Backends::VULKAN,
                        ..Default::default()
                    });
                    let adapter =
                        pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
                    eprintln!(
                        "thread={index} cycle={cycle} adapter={:?}",
                        adapter.get_info()
                    );
                    let (device, queue) = pollster::block_on(
                        adapter.request_device(&wgpu::DeviceDescriptor::default()),
                    )
                    .unwrap();
                    for _ in 0..64 {
                        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("standalone named buffer"),
                            size: 4096,
                            usage: wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        });
                        let mut encoder =
                            device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: Some("standalone named encoder"),
                            });
                        encoder.clear_buffer(&buffer, 0, None);
                        queue.submit(Some(encoder.finish()));
                    }
                    device.poll(wgpu::PollType::Wait).unwrap();
                    eprintln!("thread={index} cycle={cycle} complete");
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    eprintln!("standalone wgpu probe passed");
}
