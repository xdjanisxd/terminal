#![cfg(windows)]

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use terminal_pty::{PortablePtyBackend, PtyBackend, PtySession, PtySize, PtySpawnConfig};

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

    fn find_end_since(&self, marker: &str, since: usize) -> Option<usize> {
        self.visible[since..]
            .windows(marker.len())
            .position(|part| part == marker.as_bytes())
            .map(|offset| since + offset + marker.len())
    }
}

fn wait_for(
    marker: &str,
    since: usize,
    output: &mut PtyOutput,
    receiver: &Receiver<Vec<u8>>,
    session: &mut dyn PtySession,
) -> usize {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(end) = output.find_end_since(marker, since) {
            return end;
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
    format!("__CTRL_ARROW_SMOKE_{kind}_{nonce:x}__")
}

// The complete marker appears only in PowerShell output, never in the echoed command.
fn emit_marker(kind: &str, nonce: u128) -> String {
    format!("('__CTRL_ARROW_SMOKE_' + '{kind}_{nonce:x}__')")
}

#[test]
fn powershell_psreadline_moves_by_words_for_control_arrows() {
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
    let left = marker("LEFT", nonce);
    let right = marker("RIGHT", nonce);
    let mut output = PtyOutput::default();

    // The first default prompt is emitted before any test input, so it cannot be
    // confused with a PSReadLine redraw or an echoed command.
    let prompt_prefix = wait_for("PS ", 0, &mut output, &receiver, &mut session);
    wait_for("> ", prompt_prefix, &mut output, &receiver, &mut session);

    let ready_command = format!(
        "function prompt {{ {} }}; Write-Output {}\r",
        emit_marker("PROMPT", nonce),
        emit_marker("READY", nonce)
    );
    let setup_start = output.visible.len();
    session.write(ready_command.as_bytes()).unwrap();
    let ready_end = wait_for(&ready, setup_start, &mut output, &receiver, &mut session);
    wait_for(&prompt, ready_end, &mut output, &receiver, &mut session);

    // Search from the command's output marker so PSReadLine redraws cannot satisfy the result.
    let left_command = format!("Write-Output {} alpha beta", emit_marker("LEFT", nonce));
    let left_start = output.visible.len();
    session.write(left_command.as_bytes()).unwrap();
    session.write(b"\x1b[1;5DX\r").unwrap();
    let left_output = wait_for(&left, left_start, &mut output, &receiver, &mut session);
    let left_result = wait_for("Xbeta", left_output, &mut output, &receiver, &mut session);
    wait_for(&prompt, left_result, &mut output, &receiver, &mut session);

    let right_command = format!("Write-Output {} alpha beta", emit_marker("RIGHT", nonce));
    let right_start = output.visible.len();
    session.write(right_command.as_bytes()).unwrap();
    session.write(b"\x1b[1;5D\x1b[1;5D\x1b[1;5CX\r").unwrap();
    let right_output = wait_for(&right, right_start, &mut output, &receiver, &mut session);
    let right_result = wait_for("Xbeta", right_output, &mut output, &receiver, &mut session);
    wait_for(&prompt, right_result, &mut output, &receiver, &mut session);
    session.terminate().unwrap();
}

#[test]
fn marker_matching_survives_chunked_vt_sequences() {
    let mut output = PtyOutput::default();
    assert!(!output.push(b"\x1b]0;PowerShell\x1b"));
    assert!(!output.push(b"\\\x1b[3"));
    assert!(!output.push(b"1m__CTRL_ARROW_\x1b[0mSMOKE_LEFT"));
    assert!(!output.push(b"_42__\r\n"));
    assert_eq!(
        output.find_end_since("__CTRL_ARROW_SMOKE_LEFT_42__", 0),
        Some(28)
    );
    assert!(!output.push(b"\x1b[6"));
    assert!(output.push(b"n"));
}
