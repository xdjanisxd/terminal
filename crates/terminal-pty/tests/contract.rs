use std::ffi::OsString;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use terminal_pty::{
    PortablePtyBackend, PtyBackend, PtyError, PtyExitStatus, PtyLifecycle, PtyOutput, PtySession,
    PtySize, PtySizeError, PtySpawnConfig,
};

const HELPER_OUTPUT: &[u8] = b"TERMINAL_PTY_HELPER_MARKER";
const CONPTY_CURSOR_POSITION_QUERY: &[u8] = b"\x1b[6n";
const TEST_DEADLINE: Duration = Duration::from_secs(5);
const MAX_READS: usize = 16;

fn helper_session() -> impl PtySession {
    let configuration = PtySpawnConfig::new(
        PathBuf::from(env!("CARGO_BIN_EXE_pty_test_helper")),
        PtySize::new(24, 80).unwrap(),
    );
    PortablePtyBackend::new().spawn(configuration).unwrap()
}

fn contains_subsequence(bytes: &[u8], expected: &[u8]) -> bool {
    bytes
        .windows(expected.len())
        .any(|window| window == expected)
}

#[test]
fn size_rejects_zero_dimensions() {
    assert_eq!(PtySize::new(0, 80), Err(PtySizeError::ZeroRows));
    assert_eq!(PtySize::new(24, 0), Err(PtySizeError::ZeroColumns));
    assert_eq!(PtySize::new(24, 80).unwrap().rows(), 24);
    assert_eq!(PtySize::new(24, 80).unwrap().columns(), 80);
}

#[test]
fn spawn_config_owns_platform_arguments_environment_and_working_directory() {
    let program = PathBuf::from("C:/Program Files/example.exe");
    let directory = PathBuf::from("C:/work area");
    let argument = OsString::from("--raw=\u{fffd}");
    let environment = (OsString::from("TERM"), OsString::from("xterm-256color"));
    let config = PtySpawnConfig::new(program.clone(), PtySize::new(24, 80).unwrap())
        .with_arguments([argument.clone()])
        .with_working_directory(directory.clone())
        .with_environment([environment.clone()]);

    assert_eq!(config.program(), program.as_path());
    assert_eq!(config.arguments(), [argument]);
    assert_eq!(config.working_directory(), Some(directory.as_path()));
    assert_eq!(config.environment(), [environment]);
    assert_eq!(config.initial_size(), PtySize::new(24, 80).unwrap());
}

#[test]
fn output_preserves_arbitrary_bytes_and_distinguishes_eof_from_exit() {
    let bytes = vec![0xff, 0x1b, b'[', 0xe2, 0x82];
    assert_eq!(
        PtyOutput::bytes(bytes.clone()),
        Some(PtyOutput::Bytes(bytes))
    );
    assert_eq!(PtyOutput::bytes(Vec::new()), None);
    assert_ne!(PtyOutput::Eof, PtyOutput::Exited(PtyExitStatus::code(7)));
}

#[test]
fn portable_backend_streams_helper_output_without_assuming_read_boundaries() {
    let mut session = helper_session();
    let mut reader = session.take_output_reader().unwrap();
    assert!(matches!(
        session.take_output_reader(),
        Err(PtyError::NotRunning)
    ));
    let deadline = Instant::now() + TEST_DEADLINE;
    let mut output = Vec::new();
    let mut answered_conpty_query = false;
    for _ in 0..MAX_READS {
        let mut chunk = [0_u8; 256];
        let read = reader.read(&mut chunk).unwrap();
        if read == 0 {
            break;
        }
        output.extend_from_slice(&chunk[..read]);
        if !answered_conpty_query && contains_subsequence(&output, CONPTY_CURSOR_POSITION_QUERY) {
            // ConPTY can request cursor position before it permits child output.
            // This protocol response is test-only setup, not shell formatting.
            session.write(b"\x1b[1;1R").unwrap();
            answered_conpty_query = true;
        }
        if contains_subsequence(&output, HELPER_OUTPUT) {
            break;
        }
        assert!(Instant::now() < deadline, "PTY output deadline elapsed");
    }

    assert!(
        contains_subsequence(&output, HELPER_OUTPUT),
        "raw PTY stream did not contain the helper marker: {output:?}"
    );

    while !matches!(session.lifecycle(), PtyLifecycle::Exited(_)) {
        assert!(Instant::now() < deadline, "child exit deadline elapsed");
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        session.lifecycle(),
        PtyLifecycle::Exited(PtyExitStatus::code(0))
    );
    assert_eq!(session.write(b"after-exit"), Err(PtyError::NotRunning));
    assert_eq!(
        session.resize(PtySize::new(30, 100).unwrap()),
        Err(PtyError::NotRunning)
    );
}

#[test]
fn portable_backend_is_project_owned_and_constructible() {
    let _backend = PortablePtyBackend::new();
}

#[test]
fn lifecycle_keeps_exit_and_termination_distinct() {
    assert!(PtyLifecycle::Running.allows_operations());
    assert!(!PtyLifecycle::TerminationRequested.allows_operations());
    assert!(!PtyLifecycle::Exited(PtyExitStatus::unknown()).allows_operations());
    assert!(PtyLifecycle::TerminationRequested.termination_is_idempotent());
}
