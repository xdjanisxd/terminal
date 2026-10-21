use std::{fs, path::Path, time::Instant};
use terminal_pty::{PortablePtyBackend, PtyBackend, PtySession, PtySize, PtySpawnConfig};

fn snapshot(label: &str) {
    let fds = fs::read_dir("/proc/self/fd").unwrap().count();
    let threads = fs::read_dir("/proc/self/task").unwrap().count();
    let status = fs::read_to_string("/proc/self/status").unwrap();
    let rss = status.lines().find(|s| s.starts_with("VmRSS:")).unwrap();
    println!("{label} fds={fds} threads={threads} {rss}");
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or("direct".into());
    snapshot("start");
    let mut pids = Vec::new();
    let start = Instant::now();
    for cycle in 0..40 {
        let config = PtySpawnConfig::new("python3".into(), PtySize::new(24,80).unwrap())
            .with_arguments(["-c", "import os,signal,time; signal.signal(signal.SIGHUP,signal.SIG_IGN); print(os.getpid(),flush=True); time.sleep(60)"].map(Into::into));
        let mut session = PortablePtyBackend::new().spawn(config).unwrap();
        let mut reader = session.take_output_reader().unwrap();
        let mut bytes = [0;128];
        let count = reader.read(&mut bytes).unwrap();
        let pid: u32 = std::str::from_utf8(&bytes[..count]).unwrap().trim().parse().unwrap();
        println!("child={pid}");
        pids.push(pid);
        // Returning the reader is unavailable; direct sessions retain it across Drop.
        if mode == "explicit" {
            session.terminate().unwrap();
            while !matches!(session.lifecycle(), terminal_pty::PtyLifecycle::Exited(_)) { std::thread::yield_now(); }
        }
        drop(session);
        drop(reader);
        if cycle % 10 == 9 { snapshot(&format!("cycle-{}", cycle+1)); }
    }
    let survivors = pids.iter().filter(|pid| Path::new(&format!("/proc/{pid}")).exists()).count();
    println!("surviving_children={survivors} elapsed_ms={}", start.elapsed().as_millis());
    snapshot("end");
}
