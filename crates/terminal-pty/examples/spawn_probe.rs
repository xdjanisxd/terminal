//! Manual diagnostic harness: same backend without the app, renderer, or event loop.
use std::error::Error;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use terminal_pty::{
    PortablePtyBackend, PtyOutput, PtySize, PtySpawnConfig, PtyWorker, PtyWorkerEvent,
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let program = arguments.next().ok_or("usage: spawn_probe EXE [ARG ...]")?;
    let start = Instant::now();
    let config = PtySpawnConfig::new(PathBuf::from(program), PtySize::new(24, 80)?)
        .with_arguments(arguments)
        .with_environment([("TERM".into(), "xterm-256color".into())]);
    let stages = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&stages);
    let session = PortablePtyBackend::new().spawn_observed(
        config,
        Some(Arc::new(move |stage| {
            recorded
                .lock()
                .unwrap()
                .push((stage, start.elapsed().as_micros()));
        })),
    )?;
    let mut worker = PtyWorker::start(session)?;
    stages
        .lock()
        .unwrap()
        .push(("pty-worker-ready", start.elapsed().as_micros()));
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut first_output = false;
    while Instant::now() < deadline {
        match worker.recv_timeout(Duration::from_millis(100))? {
            Some(PtyWorkerEvent::Output(PtyOutput::Bytes(_))) => {
                first_output = true;
                break;
            }
            Some(PtyWorkerEvent::Output(PtyOutput::Eof)) => break,
            Some(PtyWorkerEvent::Error(error)) => return Err(error.into()),
            _ => {}
        }
    }
    worker.shutdown_and_join()?;
    for (stage, elapsed) in stages.lock().unwrap().iter() {
        println!("probe elapsed_us={elapsed} stage={stage}");
    }
    if !first_output {
        return Err("no first child output within observation deadline".into());
    }
    Ok(())
}
