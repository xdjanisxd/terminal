use terminal_pty::{PortablePtyBackend, PtyBackend, PtyOutput, PtySize, PtySpawnConfig, PtyWorker, PtyWorkerEvent};
use std::time::Duration;
fn main() {
    let config = PtySpawnConfig::new("python3".into(), PtySize::new(24,80).unwrap())
        .with_arguments(["-c", "import os,time,tty; tty.setraw(0); print(os.getpid(),flush=True); time.sleep(60)"].map(Into::into));
    let mut worker = PtyWorker::start(PortablePtyBackend::new().spawn(config).unwrap()).unwrap();
    let mut output = Vec::new();
    while !output.contains(&b'\n') {
        if let Some(PtyWorkerEvent::Output(PtyOutput::Bytes(bytes))) = worker.recv_timeout(Duration::from_secs(2)).unwrap() { output.extend(bytes); }
    }
    println!("child={}", std::str::from_utf8(&output).unwrap().trim());
    worker.write(vec![b'x'; 8 * 1024 * 1024]).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    println!("shutdown-start");
    worker.shutdown_and_join().unwrap();
    println!("shutdown-done");
}
