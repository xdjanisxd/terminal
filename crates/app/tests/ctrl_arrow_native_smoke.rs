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

    fn contains_since(&self, marker: &str, since: usize) -> bool {
        self.visible[since..]
            .windows(marker.len())
            .any(|part| part == marker.as_bytes())
    }

    fn tail(&self) -> String {
        let start = self.visible.len().saturating_sub(1000);
        String::from_utf8_lossy(&self.visible[start..]).into_owned()
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
            "PowerShell output marker {marker:?} missing; normalized output tail {:?}",
            output.tail()
        );
        match receiver.recv_timeout(remaining.min(Duration::from_millis(100))) {
            Ok(chunk) => {
                if output.push(&chunk) {
                    session.write(b"\x1b[1;1R").unwrap();
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                panic!(
                    "PTY output closed while waiting for {marker:?}; normalized output tail {:?}",
                    output.tail()
                )
            }
        }
    }
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
    let prompt = format!("__CTRL_ARROW_PROMPT_{nonce:x}__");
    let ready = format!("__CTRL_ARROW_READY_{nonce:x}__");
    let mut output = PtyOutput::default();

    // PowerShell assembles the markers, so command echo cannot satisfy the assertions.
    let setup = format!(
        "function prompt {{ ('__CTRL_ARROW_PROMPT_' + '{nonce:x}__') }}; \
         function EmitArrowResult {{ param($Direction, $First, $Second) \
         Write-Output ('__CTRL_ARROW_' + $Direction + '_' + $First + '_' + $Second + '_{nonce:x}__') }}; \
         Write-Output ('__CTRL_ARROW_READY_' + '{nonce:x}__')\r"
    );
    let setup_start = output.visible.len();
    session.write(setup.as_bytes()).unwrap();
    wait_for(&ready, setup_start, &mut output, &receiver, &mut session);
    wait_for(&prompt, setup_start, &mut output, &receiver, &mut session);

    let left = format!("__CTRL_ARROW_LEFT_alpha_Xbeta_{nonce:x}__");
    let left_start = output.visible.len();
    session.write(b"EmitArrowResult LEFT alpha beta").unwrap();
    wait_for(
        "EmitArrowResult LEFT alpha beta",
        left_start,
        &mut output,
        &receiver,
        &mut session,
    );
    session.write(b"\x1b[1;5DX\r").unwrap();
    wait_for(&left, left_start, &mut output, &receiver, &mut session);

    let right = format!("__CTRL_ARROW_RIGHT_alpha_Xbeta_{nonce:x}__");
    let right_start = output.visible.len();
    session.write(b"EmitArrowResult RIGHT alpha beta").unwrap();
    wait_for(
        "EmitArrowResult RIGHT alpha beta",
        right_start,
        &mut output,
        &receiver,
        &mut session,
    );
    session.write(b"\x1b[1;5D\x1b[1;5D\x1b[1;5CX\r").unwrap();
    wait_for(&right, right_start, &mut output, &receiver, &mut session);
    session.terminate().unwrap();
}

#[test]
fn result_marker_matching_survives_chunked_repaints() {
    let mut output = PtyOutput::default();
    assert!(!output.push(b"EmitArrowResult LEFT alpha beta\r\x1b[2K\x1b]0;PowerShell\x1b"));
    assert!(!output.push(b"\\\x1b[3"));
    assert!(!output.push(b"1m__CTRL_ARROW_LEFT_\x1b[0m"));
    assert!(!output.push(b"alpha_\x08Xbeta_42__\r\n"));
    assert!(output.contains_since("__CTRL_ARROW_LEFT_alpha_Xbeta_42__", 0));
    assert!(!output.push(b"\x1b[6"));
    assert!(output.push(b"n"));
}
