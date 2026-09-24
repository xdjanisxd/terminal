#![cfg(windows)]

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use terminal_pty::{
    PortablePtyBackend, PtyBackend, PtyLifecycle, PtySession, PtySize, PtySpawnConfig,
};

#[derive(Default)]
enum EscapeState {
    #[default]
    Text,
    Escape,
    Csi,
    Osc,
    OscEscape,
}

#[derive(Default)]
struct PtyOutput {
    raw: Vec<u8>,
    visible: Vec<u8>,
    escape: EscapeState,
    answered_cursor_query: bool,
}

impl PtyOutput {
    fn push(&mut self, chunk: &[u8]) -> bool {
        self.raw.extend_from_slice(chunk);
        for &byte in chunk {
            self.escape = match self.escape {
                EscapeState::Text if byte == b'\x1b' => EscapeState::Escape,
                EscapeState::Text => {
                    if byte >= b' ' || byte == b'\n' {
                        self.visible.push(byte);
                    }
                    EscapeState::Text
                }
                EscapeState::Escape if byte == b'[' => EscapeState::Csi,
                EscapeState::Escape if byte == b']' => EscapeState::Osc,
                EscapeState::Escape => EscapeState::Text,
                EscapeState::Csi if (0x40..=0x7e).contains(&byte) => EscapeState::Text,
                EscapeState::Csi => EscapeState::Csi,
                EscapeState::Osc if byte == b'\x07' => EscapeState::Text,
                EscapeState::Osc if byte == b'\x1b' => EscapeState::OscEscape,
                EscapeState::Osc => EscapeState::Osc,
                EscapeState::OscEscape if byte == b'\\' => EscapeState::Text,
                EscapeState::OscEscape => EscapeState::Osc,
            };
        }

        if !self.answered_cursor_query && self.raw.windows(4).any(|part| part == b"\x1b[6n") {
            self.answered_cursor_query = true;
            return true;
        }
        false
    }

    fn contains_since(&self, marker: &str, since: usize) -> bool {
        self.visible[since..]
            .windows(marker.len())
            .any(|part| part == marker.as_bytes())
    }
}

fn wait_for(
    marker: &str,
    since: usize,
    output: &mut PtyOutput,
    receiver: &Receiver<Vec<u8>>,
    session: &mut dyn PtySession,
) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if output.contains_since(marker, since) {
            return;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(
            !remaining.is_zero(),
            "marker {marker:?} missing; visible output {:?}",
            String::from_utf8_lossy(&output.visible)
        );
        match receiver.recv_timeout(remaining.min(Duration::from_millis(100))) {
            Ok(chunk) => {
                if output.push(&chunk) {
                    session.write(b"\x1b[1;1R").unwrap();
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                panic!("PTY output closed while waiting for {marker:?}")
            }
        }
    }
}

fn marker(kind: &str, nonce: u128) -> String {
    format!("__CTRL_C_SMOKE_{kind}_{nonce:x}__")
}

// The complete marker is emitted by PowerShell, but never appears in the echoed command.
fn emit_marker(kind: &str, nonce: u128) -> String {
    format!("('__CTRL_C_SMOKE_' + '{kind}_{nonce:x}__')")
}

#[test]
fn foreground_command_interrupt_returns_to_live_shell() {
    let config = PtySpawnConfig::new(
        PathBuf::from("powershell.exe"),
        PtySize::new(24, 80).unwrap(),
    )
    .with_arguments([OsString::from("-NoLogo"), OsString::from("-NoProfile")]);
    let mut session = PortablePtyBackend::new().spawn(config).unwrap();
    let mut reader = session.take_output_reader().unwrap();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut chunk = [0; 4096];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if sender.send(chunk[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let ready = marker("READY", nonce);
    let prompt = marker("PROMPT", nonce);
    let started = marker("STARTED", nonce);
    let finished = marker("FINISHED", nonce);
    let returned = marker("RETURNED", nonce);
    let mut output = PtyOutput::default();

    let ready_command = format!(
        "function prompt {{ {} }}; Write-Output {}\r",
        emit_marker("PROMPT", nonce),
        emit_marker("READY", nonce)
    );
    let since = output.visible.len();
    session.write(ready_command.as_bytes()).unwrap();
    wait_for(&ready, since, &mut output, &receiver, &mut session);
    wait_for(&prompt, since, &mut output, &receiver, &mut session);

    let foreground_command = format!(
        "Write-Output {}; Start-Sleep -Seconds 30; Write-Output {}\r",
        emit_marker("STARTED", nonce),
        emit_marker("FINISHED", nonce)
    );
    let foreground_start = output.visible.len();
    session.write(foreground_command.as_bytes()).unwrap();
    wait_for(
        &started,
        foreground_start,
        &mut output,
        &receiver,
        &mut session,
    );

    let interrupt_start = output.visible.len();
    session.write(&[0x03]).unwrap();
    wait_for(
        &prompt,
        interrupt_start,
        &mut output,
        &receiver,
        &mut session,
    );
    assert!(
        !output.contains_since(&finished, foreground_start),
        "foreground command completed instead of being interrupted"
    );

    let returned_command = format!("Write-Output {}\r", emit_marker("RETURNED", nonce));
    let returned_start = output.visible.len();
    session.write(returned_command.as_bytes()).unwrap();
    wait_for(
        &returned,
        returned_start,
        &mut output,
        &receiver,
        &mut session,
    );
    assert_eq!(session.lifecycle(), PtyLifecycle::Running);
    session.terminate().unwrap();
}

#[test]
fn marker_matching_survives_chunked_vt_sequences() {
    let mut output = PtyOutput::default();
    assert!(!output.push(b"\x1b]0;PowerShell\x1b"));
    assert!(!output.push(b"\\\x1b[3"));
    assert!(!output.push(b"1m__CTRL_C_\x1b[0mSMOKE_READY"));
    assert!(!output.push(b"_42__\r\n"));
    assert!(output.contains_since("__CTRL_C_SMOKE_READY_42__", 0));
    assert!(!output.push(b"\x1b[6"));
    assert!(output.push(b"n"));
}
