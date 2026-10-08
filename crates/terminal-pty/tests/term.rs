#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::time::{Duration, Instant};
use terminal_pty::{
    PortablePtyBackend, PtyBackend, PtyOutput, PtySize, PtySpawnConfig, PtyWorker, PtyWorkerEvent,
};

fn bash_output(script: &str, term: Option<&str>) -> Vec<u8> {
    let mut configuration = PtySpawnConfig::new("/bin/bash".into(), PtySize::new(24, 80).unwrap())
        .with_arguments(
            ["--noprofile", "--norc", "-c", script]
                .into_iter()
                .map(Into::into),
        );
    if let Some(term) = term {
        configuration = configuration.with_environment([("TERM".into(), term.into())]);
    }
    let session = PortablePtyBackend::new().spawn(configuration).unwrap();
    let mut worker = PtyWorker::start(session).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut output = Vec::new();
    while !output.windows(8).any(|bytes| bytes == b"TERM_END") {
        assert!(
            Instant::now() < deadline,
            "Bash TERM probe timed out: {output:?}"
        );
        if let Some(event) = worker.recv_timeout(Duration::from_millis(20)).unwrap() {
            match event {
                PtyWorkerEvent::Output(PtyOutput::Bytes(bytes)) => output.extend(bytes),
                PtyWorkerEvent::Error(error) => panic!("Bash PTY error: {error}"),
                _ => {}
            }
        }
    }
    worker.shutdown_and_join().unwrap();
    output
}

#[test]
fn native_bash_receives_default_or_preserved_term_without_parent_changes() {
    let parent = std::env::var_os("TERM");
    let expected = parent
        .clone()
        .filter(|value| !value.is_empty() && value != "dumb")
        .unwrap_or_else(|| "xterm-256color".into());
    let output = bash_output("printf 'TERM_VALUE=%s;TERM_END' \"$TERM\"", None);
    assert!(String::from_utf8_lossy(&output).contains(&format!(
        "TERM_VALUE={};TERM_END",
        expected.to_string_lossy()
    )));
    assert_eq!(std::env::var_os("TERM"), parent);
}

#[test]
fn native_bash_respects_explicit_term_and_later_shell_overrides() {
    for term in ["dumb", "vt100", "xterm-256color"] {
        let output = bash_output("printf 'TERM_VALUE=%s;TERM_END' \"$TERM\"", Some(term));
        assert!(String::from_utf8_lossy(&output).contains(&format!("TERM_VALUE={term};TERM_END")));
    }
    let output = bash_output(
        "export TERM=vt100; printf 'TERM_VALUE=%s;TERM_END' \"$TERM\"",
        Some("xterm-256color"),
    );
    assert!(String::from_utf8_lossy(&output).contains("TERM_VALUE=vt100;TERM_END"));
}

#[test]
fn native_clear_succeeds_with_xterm_256color_and_emits_supported_erase_sequences() {
    let output = bash_output(
        "clear; printf 'CLEAR_STATUS=%s;TERM_END' \"$?\"",
        Some("xterm-256color"),
    );
    assert!(String::from_utf8_lossy(&output).contains("CLEAR_STATUS=0;TERM_END"));
    assert!(output.windows(3).any(|bytes| bytes == b"\x1b[H"));
    assert!(output.windows(4).any(|bytes| bytes == b"\x1b[2J"));
}
