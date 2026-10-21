use std::sync::{Arc, Barrier};

fn main() {
    let gate = Arc::new(Barrier::new(7));
    let threads: Vec<_> = (0..7).map(|index| {
        let gate = Arc::clone(&gate);
        std::thread::spawn(move || {
            gate.wait();
            for cycle in 0..4 {
                let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
                let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
                let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
                for _ in 0..64 {
                    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("standalone named buffer"),
                        size: 4096,
                        usage: wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("standalone named encoder") });
                    encoder.clear_buffer(&buffer, 0, None);
                    queue.submit(Some(encoder.finish()));
                }
                device.poll(wgpu::PollType::Wait).unwrap();
                eprintln!("thread={index} cycle={cycle} complete");
            }
        })
    }).collect();
    for thread in threads { thread.join().unwrap(); }
    eprintln!("standalone wgpu probe passed");
}
