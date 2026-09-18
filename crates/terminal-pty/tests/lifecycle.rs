use std::ffi::OsString;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use terminal_pty::{
    PortablePtyBackend, PtyBackend, PtyError, PtyExitStatus, PtyLifecycle, PtyOutputReader,
    PtySession, PtySize, PtySpawnConfig,
};

const HELPER_MARKER: &[u8] = b"TERMINAL_PTY_HELPER_MARKER";
const CONPTY_CURSOR_POSITION_QUERY: &[u8] = b"\x1b[6n";
const DEADLINE: Duration = Duration::from_secs(5);
const MAX_READS: usize = 16;

fn spawn_helper(arguments: &[&str]) -> impl PtySession {
    let configuration = PtySpawnConfig::new(
        PathBuf::from(env!("CARGO_BIN_EXE_pty_test_helper")),
        PtySize::new(24, 80).unwrap(),
    )
    .with_arguments(arguments.iter().map(OsString::from));
    PortablePtyBackend::new().spawn(configuration).unwrap()
}

fn contains_subsequence(bytes: &[u8], expected: &[u8]) -> bool {
    bytes
        .windows(expected.len())
        .any(|window| window == expected)
}

fn drain_until_marker<const CHUNK_SIZE: usize>(
    session: &mut impl PtySession,
    reader: &mut (impl PtyOutputReader + ?Sized),
    max_reads: usize,
) -> Vec<u8> {
    let deadline = Instant::now() + DEADLINE;
    let mut output = Vec::new();
    let mut answered_conpty_query = false;

    for _ in 0..max_reads {
        let mut chunk = [0_u8; CHUNK_SIZE];
        let read = reader.read(&mut chunk).unwrap();
        if read == 0 {
            break;
        }
        output.extend_from_slice(&chunk[..read]);
        if !answered_conpty_query && contains_subsequence(&output, CONPTY_CURSOR_POSITION_QUERY) {
            session.write(b"\x1b[1;1R").unwrap();
            answered_conpty_query = true;
        }
        if contains_subsequence(&output, HELPER_MARKER) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "output deadline elapsed: {output:?}"
        );
    }
    output
}

fn wait_for_exit(session: &impl PtySession, deadline: Instant, output: &[u8]) -> PtyLifecycle {
    loop {
        let lifecycle = session.lifecycle();
        if matches!(lifecycle, PtyLifecycle::Exited(_)) {
            return lifecycle;
        }
        assert!(
            Instant::now() < deadline,
            "child exit observation deadline elapsed: {lifecycle:?}; bytes_read={}; marker_observed={}",
            output.len(),
            contains_subsequence(output, HELPER_MARKER),
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn naturally_exiting_child_preserves_numeric_exit_status() {
    let mut session = spawn_helper(&["exit", "23"]);
    let mut reader = session.take_output_reader().unwrap();
    let output = drain_until_marker::<256>(&mut session, reader.as_mut(), MAX_READS);
    assert!(contains_subsequence(&output, HELPER_MARKER));

    let exited = wait_for_exit(&session, Instant::now() + DEADLINE, &output);
    assert_eq!(exited, PtyLifecycle::Exited(PtyExitStatus::code(23)));
    assert_eq!(session.lifecycle(), exited);
    assert_eq!(session.write(b"after-exit"), Err(PtyError::NotRunning));
    assert_eq!(
        session.resize(PtySize::new(30, 100).unwrap()),
        Err(PtyError::NotRunning)
    );
}

#[test]
fn terminate_rejects_later_operations_and_reader_reaches_eof() {
    let mut session = spawn_helper(&["wait"]);
    let mut reader = session.take_output_reader().unwrap();

    assert_eq!(session.lifecycle(), PtyLifecycle::Running);
    session.terminate().unwrap();
    assert!(matches!(
        session.lifecycle(),
        PtyLifecycle::TerminationRequested | PtyLifecycle::Exited(_)
    ));
    session.terminate().unwrap();
    assert_eq!(session.write(b"after-terminate"), Err(PtyError::NotRunning));
    assert_eq!(
        session.resize(PtySize::new(30, 100).unwrap()),
        Err(PtyError::NotRunning)
    );

    wait_for_exit(&session, Instant::now() + DEADLINE, &[]);
    let mut saw_eof = false;
    for _ in 0..MAX_READS {
        let mut chunk = [0_u8; 256];
        if reader.read(&mut chunk).unwrap() == 0 {
            saw_eof = true;
            break;
        }
    }
    assert!(saw_eof, "reader did not report EOF after termination");
}

#[test]
fn output_eof_follows_buffered_drain_after_child_exit() {
    let mut session = spawn_helper(&["exit", "0"]);
    let mut reader = session.take_output_reader().unwrap();
    let mut startup = [0_u8; 1];
    let read = reader.read(&mut startup).unwrap();
    assert_eq!(read, 1, "reader reached EOF before helper output");
    let mut output = vec![startup[0]];

    if CONPTY_CURSOR_POSITION_QUERY.starts_with(&output) {
        while output.len() < CONPTY_CURSOR_POSITION_QUERY.len() {
            let read = reader.read(&mut startup).unwrap();
            assert_eq!(read, 1, "reader reached EOF during ConPTY startup query");
            output.push(startup[0]);
        }
        assert_eq!(output, CONPTY_CURSOR_POSITION_QUERY);
        session.write(b"\x1b[1;1R").unwrap();
    }

    let exited = wait_for_exit(&session, Instant::now() + DEADLINE, &output);
    assert_eq!(exited, PtyLifecycle::Exited(PtyExitStatus::code(0)));

    let deadline = Instant::now() + DEADLINE;
    let mut read_after_exit = false;
    let mut saw_eof = false;
    for _ in 0..MAX_READS {
        let mut chunk = [0_u8; 256];
        let read = reader.read(&mut chunk).unwrap();
        if read == 0 {
            saw_eof = true;
            break;
        }
        read_after_exit = true;
        output.extend_from_slice(&chunk[..read]);
        assert!(
            Instant::now() < deadline,
            "EOF drain deadline elapsed: {output:?}"
        );
    }

    assert!(
        read_after_exit,
        "reader reached EOF without post-exit bytes: {output:?}"
    );
    assert!(contains_subsequence(&output, HELPER_MARKER));
    assert!(
        saw_eof,
        "reader did not report EOF after child exit: {output:?}"
    );
}

#[test]
fn resize_and_raw_write_succeed_while_running() {
    let mut session = spawn_helper(&["wait"]);
    let _reader = session.take_output_reader().unwrap();

    assert_eq!(session.lifecycle(), PtyLifecycle::Running);
    session.resize(PtySize::new(12, 40).unwrap()).unwrap();
    session.resize(PtySize::new(48, 160).unwrap()).unwrap();
    session.write(&[0x00, 0xff, b'\n']).unwrap();
    session.terminate().unwrap();
}
