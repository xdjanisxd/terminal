use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use terminal_pty::{
    PTY_COMMAND_CAPACITY, PortablePtyBackend, PtyBackend, PtyError, PtyExitStatus, PtyLifecycle,
    PtyOutput, PtyOutputReader, PtySession, PtySize, PtySpawnConfig, PtyWorker, PtyWorkerError,
    PtyWorkerEvent,
};

const HELPER_MARKER: &[u8] = b"TERMINAL_PTY_HELPER_MARKER";
const DEADLINE: Duration = Duration::from_secs(5);

fn contains_subsequence(bytes: &[u8], expected: &[u8]) -> bool {
    bytes
        .windows(expected.len())
        .any(|window| window == expected)
}

fn helper_worker(arguments: &[&str]) -> PtyWorker {
    let configuration = PtySpawnConfig::new(
        PathBuf::from(env!("CARGO_BIN_EXE_pty_test_helper")),
        PtySize::new(24, 80).unwrap(),
    )
    .with_arguments(arguments.iter().map(OsString::from));
    PtyWorker::start(PortablePtyBackend::new().spawn(configuration).unwrap()).unwrap()
}

#[test]
fn worker_forwards_raw_helper_output_and_independent_terminal_events() {
    let mut worker = helper_worker(&["exit", "23"]);
    // ConPTY may require CPR before allowing the helper to complete. This remains raw input.
    worker.write(b"\x1b[1;1R".to_vec()).unwrap();

    let deadline = Instant::now() + DEADLINE;
    let mut output = Vec::new();
    let mut saw_eof = false;
    let mut exited = None;
    while !saw_eof || exited.is_none() {
        assert!(Instant::now() < deadline, "worker event deadline elapsed");
        match worker.recv_timeout(Duration::from_millis(50)).unwrap() {
            Some(PtyWorkerEvent::Output(PtyOutput::Bytes(bytes))) => output.extend(bytes),
            Some(PtyWorkerEvent::Output(PtyOutput::Eof)) => saw_eof = true,
            Some(PtyWorkerEvent::Output(PtyOutput::Exited(status))) => exited = Some(status),
            Some(PtyWorkerEvent::Error(error)) => panic!("worker reported error: {error:?}"),
            None => {}
        }
    }

    assert!(contains_subsequence(&output, HELPER_MARKER));
    assert_eq!(exited, Some(PtyExitStatus::code(23)));
    worker.join().unwrap();
}

struct TestState {
    lifecycle: PtyLifecycle,
    writes: Vec<Vec<u8>>,
    resizes: Vec<PtySize>,
    terminated: usize,
}

impl Default for TestState {
    fn default() -> Self {
        Self {
            lifecycle: PtyLifecycle::Running,
            writes: Vec::new(),
            resizes: Vec::new(),
            terminated: 0,
        }
    }
}

struct ScriptedReader {
    chunks: VecDeque<Vec<u8>>,
}

impl PtyOutputReader for ScriptedReader {
    fn read(&mut self, bytes: &mut [u8]) -> Result<usize, PtyError> {
        let Some(chunk) = self.chunks.pop_front() else {
            return Ok(0);
        };
        assert!(chunk.len() <= bytes.len());
        bytes[..chunk.len()].copy_from_slice(&chunk);
        Ok(chunk.len())
    }
}

struct TestSession {
    state: Arc<Mutex<TestState>>,
    reader: Option<Box<dyn PtyOutputReader + Send>>,
    first_write_started: Option<SyncSender<()>>,
    first_write_gate: Option<Receiver<()>>,
}

impl PtySession for TestSession {
    fn lifecycle(&self) -> PtyLifecycle {
        self.state.lock().unwrap().lifecycle
    }

    fn take_output_reader(&mut self) -> Result<Box<dyn PtyOutputReader + Send>, PtyError> {
        self.reader.take().ok_or(PtyError::NotRunning)
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), PtyError> {
        if let Some(started) = self.first_write_started.take() {
            started.send(()).unwrap();
            self.first_write_gate.take().unwrap().recv().unwrap();
        }
        self.state.lock().unwrap().writes.push(bytes.to_vec());
        Ok(())
    }

    fn resize(&mut self, size: PtySize) -> Result<(), PtyError> {
        self.state.lock().unwrap().resizes.push(size);
        Ok(())
    }

    fn terminate(&mut self) -> Result<(), PtyError> {
        let mut state = self.state.lock().unwrap();
        state.terminated += 1;
        state.lifecycle = PtyLifecycle::Exited(PtyExitStatus::unknown());
        Ok(())
    }
}

fn scripted_worker(
    lifecycle: PtyLifecycle,
    chunks: impl IntoIterator<Item = Vec<u8>>,
) -> (PtyWorker, Arc<Mutex<TestState>>) {
    let state = Arc::new(Mutex::new(TestState {
        lifecycle,
        ..TestState::default()
    }));
    let session = TestSession {
        state: Arc::clone(&state),
        reader: Some(Box::new(ScriptedReader {
            chunks: chunks.into_iter().collect(),
        })),
        first_write_started: None,
        first_write_gate: None,
    };
    (PtyWorker::start(session).unwrap(), state)
}

#[test]
fn bounded_event_queue_backpressures_without_dropping_terminal_events() {
    let chunks = (0_u8..10).map(|byte| vec![byte]).collect::<Vec<_>>();
    let (mut worker, _) = scripted_worker(PtyLifecycle::Exited(PtyExitStatus::code(7)), chunks);
    let deadline = Instant::now() + DEADLINE;
    let mut output = Vec::new();
    let mut eof_count = 0;
    let mut exit_count = 0;

    loop {
        assert!(
            Instant::now() < deadline,
            "bounded event drain deadline elapsed"
        );
        match worker.recv_timeout(Duration::from_millis(50)).unwrap() {
            Some(PtyWorkerEvent::Output(PtyOutput::Bytes(bytes))) => output.extend(bytes),
            Some(PtyWorkerEvent::Output(PtyOutput::Eof)) => eof_count += 1,
            Some(PtyWorkerEvent::Output(PtyOutput::Exited(status))) => {
                assert_eq!(status, PtyExitStatus::code(7));
                exit_count += 1;
            }
            Some(PtyWorkerEvent::Error(error)) => panic!("unexpected worker error: {error:?}"),
            None if eof_count == 1 && exit_count == 1 => break,
            None => {}
        }
        if eof_count == 1 && exit_count == 1 {
            break;
        }
    }

    assert_eq!(output, (0_u8..10).collect::<Vec<_>>());
    assert_eq!(eof_count, 1);
    assert_eq!(exit_count, 1);
    worker.join().unwrap();
}

#[test]
fn worker_notifies_the_controller_after_queueing_each_event() {
    let state = Arc::new(Mutex::new(TestState {
        lifecycle: PtyLifecycle::Exited(PtyExitStatus::code(7)),
        ..TestState::default()
    }));
    let session = TestSession {
        state: Arc::clone(&state),
        reader: Some(Box::new(ScriptedReader {
            chunks: [b"raw".to_vec()].into(),
        })),
        first_write_started: None,
        first_write_gate: None,
    };
    let notifications = Arc::new(AtomicUsize::new(0));
    let notifier = Arc::clone(&notifications);
    let mut worker = PtyWorker::start_with_notifier(session, move || {
        notifier.fetch_add(1, Ordering::Relaxed);
    })
    .unwrap();

    let deadline = Instant::now() + DEADLINE;
    let mut event_count = 0;
    while event_count < 3 {
        assert!(Instant::now() < deadline, "worker event deadline elapsed");
        if worker
            .recv_timeout(Duration::from_millis(50))
            .unwrap()
            .is_some()
        {
            event_count += 1;
        }
    }
    worker.join().unwrap();

    assert_eq!(notifications.load(Ordering::Relaxed), event_count);
}

#[test]
fn full_command_queue_rejects_the_newest_command_without_dropping_queued_commands() {
    let state = Arc::new(Mutex::new(TestState::default()));
    let (started_sender, started_receiver) = mpsc::sync_channel(1);
    let (gate_sender, gate_receiver) = mpsc::sync_channel(0);
    let session = TestSession {
        state: Arc::clone(&state),
        reader: Some(Box::new(ScriptedReader {
            chunks: VecDeque::new(),
        })),
        first_write_started: Some(started_sender),
        first_write_gate: Some(gate_receiver),
    };
    let mut worker = PtyWorker::start(session).unwrap();

    worker.write(vec![0]).unwrap();
    started_receiver.recv_timeout(DEADLINE).unwrap();
    for byte in 1..=PTY_COMMAND_CAPACITY {
        worker.write(vec![byte as u8]).unwrap();
    }
    assert_eq!(
        worker.write(vec![255]),
        Err(PtyWorkerError::CommandQueueFull)
    );

    gate_sender.send(()).unwrap();
    let expected = (0..=PTY_COMMAND_CAPACITY)
        .map(|byte| vec![byte as u8])
        .collect::<Vec<_>>();
    let deadline = Instant::now() + DEADLINE;
    while state.lock().unwrap().writes.len() != expected.len() {
        assert!(
            Instant::now() < deadline,
            "queued worker commands were not processed before the deadline"
        );
        std::thread::yield_now();
    }
    assert_eq!(state.lock().unwrap().writes, expected);
    worker.shutdown_and_join().unwrap();
}

#[test]
fn explicit_termination_emits_each_terminal_event_once_and_join_rejects_later_commands() {
    let (mut worker, state) = scripted_worker(PtyLifecycle::Running, [b"raw".to_vec()]);
    worker.resize(PtySize::new(30, 100).unwrap()).unwrap();
    worker.write(vec![0, 0xff, b'\n']).unwrap();
    worker.terminate().unwrap();

    let deadline = Instant::now() + DEADLINE;
    let mut eof_count = 0;
    let mut exit_count = 0;
    while eof_count == 0 || exit_count == 0 {
        assert!(
            Instant::now() < deadline,
            "termination event deadline elapsed"
        );
        match worker.recv_timeout(Duration::from_millis(50)).unwrap() {
            Some(PtyWorkerEvent::Output(PtyOutput::Eof)) => eof_count += 1,
            Some(PtyWorkerEvent::Output(PtyOutput::Exited(_))) => exit_count += 1,
            Some(PtyWorkerEvent::Output(PtyOutput::Bytes(_))) => {}
            Some(PtyWorkerEvent::Error(error)) => panic!("unexpected worker error: {error:?}"),
            None => {}
        }
    }
    worker.join().unwrap();

    assert_eq!(eof_count, 1);
    assert_eq!(exit_count, 1);
    let state = state.lock().unwrap();
    assert_eq!(state.terminated, 1);
    assert_eq!(state.resizes, [PtySize::new(30, 100).unwrap()]);
    assert_eq!(state.writes, [vec![0, 0xff, b'\n']]);
    drop(state);
    assert_eq!(
        worker.write(vec![1]),
        Err(PtyWorkerError::WorkerUnavailable)
    );
}

#[test]
fn dropping_the_controller_terminates_and_joins_the_session() {
    let (worker, state) = scripted_worker(PtyLifecycle::Running, []);
    drop(worker);
    assert_eq!(state.lock().unwrap().terminated, 1);
}
