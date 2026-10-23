//! Linux-only measurement harness. Compile against either revision's PTY rlib.
use std::collections::BTreeSet;
use std::fs;
use std::time::{Duration, Instant};
use terminal_pty::{
    PTY_COMMAND_CAPACITY, PortablePtyBackend, PtyBackend, PtyOutput, PtySize, PtySpawnConfig,
    PtyWorker, PtyWorkerEvent,
};

fn tasks() -> BTreeSet<String> {
    fs::read_dir("/proc/self/task")
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect()
}

fn resources() -> (usize, usize) {
    (
        fs::read_dir("/proc/self/fd").unwrap().count(),
        tasks().len(),
    )
}

fn main() {
    let arguments = std::env::args().collect::<Vec<_>>();
    let mode = arguments.get(1).map(String::as_str).unwrap_or("normal");
    let panes = arguments
        .get(2)
        .map(|value| value.parse::<usize>().unwrap())
        .unwrap_or(1);
    let before = resources();
    let mut workers = Vec::new();
    let mut pids = Vec::new();
    for _ in 0..panes {
        let previous = tasks();
        let script = if mode == "ignore-hup" {
            "import os,tty,signal,time; tty.setraw(0); signal.signal(signal.SIGHUP,signal.SIG_IGN); print(os.getpid(),flush=True); time.sleep(60)"
        } else if mode == "graceful" {
            "import os,tty,signal,time; tty.setraw(0); signal.signal(signal.SIGHUP,lambda *_: (open(os.environ['PTY_PROBE_GRACEFUL_MARKER'],'w').write('hup'),exit(0))); print(os.getpid(),flush=True); time.sleep(60)"
        } else {
            "import os,tty,time; tty.setraw(0); print(os.getpid(),flush=True); time.sleep(60)"
        };
        let configuration = PtySpawnConfig::new("python3".into(), PtySize::new(24, 80).unwrap())
            .with_arguments(["-c", script].map(Into::into));
        let worker =
            PtyWorker::start(PortablePtyBackend::new().spawn(configuration).unwrap()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut output = Vec::new();
        while !output.contains(&b'\n') {
            assert!(Instant::now() < deadline, "child readiness timed out");
            if let Some(PtyWorkerEvent::Output(PtyOutput::Bytes(bytes))) =
                worker.recv_timeout(Duration::from_millis(50)).unwrap()
            {
                output.extend(bytes);
            }
        }
        let pid = std::str::from_utf8(&output)
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap();
        println!("child={pid}");
        pids.push(pid);
        if mode == "blocked" || mode == "full" || mode == "ignore-hup" {
            let worker_threads = tasks().difference(&previous).cloned().collect::<Vec<_>>();
            worker.write(vec![b'x'; 8 * 1024 * 1024]).unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let waiting = worker_threads
                    .iter()
                    .filter_map(|tid| {
                        fs::read_to_string(format!("/proc/self/task/{tid}/wchan")).ok()
                    })
                    .collect::<Vec<_>>();
                // Both threads are in kernel I/O waits: read/write on the old
                // adapter, readiness waits on the new adapter. No timing sleep.
                if waiting.len() == 2
                    && waiting
                        .iter()
                        .all(|name| name.contains("wait_woken") || name.contains("poll"))
                {
                    println!("blocked_waits={waiting:?}");
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "write did not block: {waiting:?}"
                );
                std::thread::yield_now();
            }
            if mode == "full" {
                for _ in 0..PTY_COMMAND_CAPACITY {
                    worker.write_input(b"queued").unwrap();
                }
            }
        }
        workers.push(worker);
    }
    let active = resources();
    println!(
        "shutdown-start mode={mode} panes={panes} resources_before={before:?} resources_active={active:?}"
    );
    let started = Instant::now();
    for worker in &mut workers {
        worker.shutdown_and_join().unwrap();
    }
    drop(workers);
    let elapsed = started.elapsed();
    // /proc may briefly retain a kernel task after pthread_join has returned.
    // Observe quiescence separately from the measured shutdown duration.
    let deadline = Instant::now() + Duration::from_secs(1);
    let after = loop {
        let current = resources();
        if current == before || Instant::now() >= deadline {
            break current;
        }
        std::thread::yield_now();
    };
    assert_eq!(after, before, "FD or worker thread leak");
    assert!(
        pids.iter()
            .all(|pid| !std::path::Path::new(&format!("/proc/{pid}")).exists()),
        "child not reaped"
    );
    println!(
        "shutdown-done elapsed_us={} resources_after={after:?} children_gone={}",
        elapsed.as_micros(),
        pids.len()
    );
}
