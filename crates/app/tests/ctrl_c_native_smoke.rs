#![cfg(windows)]

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use terminal_pty::{PtyLifecycle, PtySession};

#[path = "support/powershell.rs"]
mod powershell;
use powershell::{ReadEvent, spawn_power_shell};

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
    phase: &str,
    marker: &str,
    since: usize,
    output: &mut PtyOutput,
    receiver: &Receiver<ReadEvent>,
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
            "{phase}: marker {marker:?} missing; child lifecycle={:?}, normalized output tail {:?}",
            session.lifecycle(),
            output.tail()
        );
        match receiver.recv_timeout(remaining.min(Duration::from_millis(100))) {
            Ok(ReadEvent::Bytes(chunk)) => {
                if output.push(&chunk) {
                    session.write(b"\x1b[1;1R").unwrap();
                }
            }
            Ok(ReadEvent::Eof) => panic!(
                "{phase}: PTY reader reached EOF before {marker:?}; child lifecycle={:?}, normalized output tail {:?}",
                session.lifecycle(),
                output.tail()
            ),
            Ok(ReadEvent::Error(error)) => panic!(
                "{phase}: PTY reader failed ({error}) before {marker:?}; child lifecycle={:?}, normalized output tail {:?}",
                session.lifecycle(),
                output.tail()
            ),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                panic!(
                    "{phase}: PTY reader channel closed before {marker:?}; child lifecycle={:?}, normalized output tail {:?}",
                    session.lifecycle(),
                    output.tail()
                )
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
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let ready = marker("READY", nonce);
    let prompt_ready = marker("PROMPT_READY", nonce);
    let interactive_ready = marker("INTERACTIVE_READY", nonce);
    let started = marker("STARTED", nonce);
    let finished = marker("FINISHED", nonce);
    let interrupted = marker("INTERRUPTED", nonce);
    let prompt_returned = marker("PROMPT_RETURNED", nonce);
    let reused = marker("REUSED", nonce);
    let setup = format!(
        "Import-Module PSReadLine -ErrorAction Stop; \
         $global:CtrlCForegroundStarted = $false; $global:CtrlCForegroundFinished = $false; \
         function prompt {{ if ($global:CtrlCForegroundStarted -and -not $global:CtrlCForegroundFinished) {{ \
         $global:CtrlCForegroundStarted = $false; ({} + [Environment]::NewLine + {}) \
         }} else {{ {} }} }}; Write-Output {}",
        emit_marker("INTERRUPTED", nonce),
        emit_marker("PROMPT_RETURNED", nonce),
        emit_marker("PROMPT_READY", nonce),
        emit_marker("READY", nonce)
    );
    let (mut session, receiver) = spawn_power_shell(setup, true);
    let mut output = PtyOutput::default();
    wait_for(
        "PowerShell startup setup did not execute",
        &ready,
        0,
        &mut output,
        &receiver,
        &mut session,
    );
    wait_for(
        "PowerShell interactive prompt not ready",
        &prompt_ready,
        0,
        &mut output,
        &receiver,
        &mut session,
    );
    let interactive_start = output.visible.len();
    session
        .write(format!("Write-Output {}\r", emit_marker("INTERACTIVE_READY", nonce)).as_bytes())
        .unwrap();
    wait_for(
        "PowerShell did not execute the interactive readiness probe",
        &interactive_ready,
        interactive_start,
        &mut output,
        &receiver,
        &mut session,
    );
    wait_for(
        "PowerShell prompt did not return after the readiness probe",
        &prompt_ready,
        interactive_start,
        &mut output,
        &receiver,
        &mut session,
    );

    let foreground_command = format!(
        "$global:CtrlCForegroundStarted = $true; Write-Output {}; \
         Start-Sleep -Seconds 30; $global:CtrlCForegroundFinished = $true; Write-Output {}\r",
        emit_marker("STARTED", nonce),
        emit_marker("FINISHED", nonce)
    );
    let foreground_start = output.visible.len();
    session.write(foreground_command.as_bytes()).unwrap();
    wait_for(
        "foreground command did not start",
        &started,
        foreground_start,
        &mut output,
        &receiver,
        &mut session,
    );

    let interrupt_start = output.visible.len();
    session.write(&[0x03]).unwrap();
    wait_for(
        "Ctrl+C interruption not observed",
        &interrupted,
        interrupt_start,
        &mut output,
        &receiver,
        &mut session,
    );
    wait_for(
        "PowerShell prompt did not return after Ctrl+C",
        &prompt_returned,
        interrupt_start,
        &mut output,
        &receiver,
        &mut session,
    );
    assert!(
        !output.contains_since(&finished, foreground_start),
        "foreground command completed instead of being interrupted; normalized output tail {:?}",
        output.tail()
    );

    let reuse_command = format!("Write-Output {}\r", emit_marker("REUSED", nonce));
    let reuse_start = output.visible.len();
    session.write(reuse_command.as_bytes()).unwrap();
    wait_for(
        "PowerShell did not execute a follow-up command",
        &reused,
        reuse_start,
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
