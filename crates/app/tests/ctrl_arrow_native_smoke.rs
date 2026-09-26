#![cfg(windows)]

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use terminal_pty::{
    PortablePtyBackend, PortablePtySession, PtyBackend, PtyError, PtySession, PtySize,
    PtySpawnConfig,
};

enum ReadEvent {
    Bytes(Vec<u8>),
    Eof,
    Error(PtyError),
}

fn executable_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|program| program.is_file())
}

fn power_shell_program() -> PathBuf {
    let pwsh = executable_on_path("pwsh.exe");
    let windows_powershell = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .map(|root| {
            root.join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe")
        })
        .filter(|program| program.is_file())
        .or_else(|| executable_on_path("powershell.exe"));
    eprintln!(
        "PowerShell discovery: pwsh={pwsh:?}, Windows PowerShell={windows_powershell:?}, host architecture={}",
        std::env::consts::ARCH
    );
    let program = pwsh
        .or(windows_powershell)
        .expect("no PowerShell executable found");
    assert!(
        program.is_file(),
        "selected PowerShell is not a file: {program:?}"
    );

    // Verify that the selected executable can run on this host independently of ConPTY.
    let probe = Command::new(&program)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Write-Output ('__CTRL_ARROW_PROCESS_' + 'READY__'); Write-Output ('VERSION=' + $PSVersionTable.PSVersion.ToString()); Write-Output ('ARCH=' + [System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture); Write-Output ('PSREADLINE=' + [bool](Get-Module -ListAvailable PSReadLine))",
        ])
        .output()
        .unwrap_or_else(|error| panic!("could not launch {program:?} directly: {error}"));
    let stdout = String::from_utf8_lossy(&probe.stdout);
    let stderr = String::from_utf8_lossy(&probe.stderr);
    eprintln!(
        "PowerShell direct probe: executable={program:?}, status={}, stdout={stdout:?}, stderr={stderr:?}",
        probe.status
    );
    assert!(
        probe.status.success() && stdout.contains("__CTRL_ARROW_PROCESS_READY__"),
        "selected PowerShell {program:?} did not execute the direct probe"
    );
    program
}

fn spawn_power_shell(setup: String) -> (PortablePtySession, Receiver<ReadEvent>) {
    let program = power_shell_program();
    let config = PtySpawnConfig::new(program.clone(), PtySize::new(24, 80).unwrap())
        .with_arguments([
            OsString::from("-NoLogo"),
            OsString::from("-NoProfile"),
            OsString::from("-NoExit"),
            OsString::from("-Command"),
            OsString::from(setup),
        ]);
    let mut session = PortablePtyBackend::new()
        .spawn(config)
        .unwrap_or_else(|error| panic!("PTY spawn failed for {program:?}: {error}"));
    eprintln!(
        "PowerShell PTY spawn: executable={program:?}, lifecycle={:?}",
        session.lifecycle()
    );
    let mut reader = session.take_output_reader().unwrap();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut chunk = [0; 4096];
        loop {
            let event = match reader.read(&mut chunk) {
                Ok(0) => ReadEvent::Eof,
                Ok(n) => ReadEvent::Bytes(chunk[..n].to_vec()),
                Err(error) => ReadEvent::Error(error),
            };
            let finished = !matches!(event, ReadEvent::Bytes(_));
            if sender.send(event).is_err() || finished {
                break;
            }
        }
    });
    (session, receiver)
}

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
            "{phase}: marker {marker:?} missing; child lifecycle={:?}, normalized output tail {:?}, raw PTY tail {:?}",
            session.lifecycle(),
            output.tail(),
            &output.raw[output.raw.len().saturating_sub(200)..]
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

#[test]
fn powershell_pty_startup_and_psreadline_are_ready() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let pty_marker = format!("__CTRL_ARROW_PTY_{nonce:x}__");
    let module_marker = format!("__CTRL_ARROW_MODULE_{nonce:x}__");
    let prompt_marker = format!("__CTRL_ARROW_PROMPT_{nonce:x}__");
    let interactive_marker = format!("__CTRL_ARROW_INTERACTIVE_{nonce:x}__");
    let setup = format!(
        "Write-Output ('__CTRL_ARROW_PTY_' + '{nonce:x}__'); \
         Import-Module PSReadLine -ErrorAction Stop; \
         Write-Output ('__CTRL_ARROW_MODULE_' + '{nonce:x}__'); \
         function prompt {{ ('__CTRL_ARROW_PROMPT_' + '{nonce:x}__') }}"
    );
    let (mut session, receiver) = spawn_power_shell(setup);
    let mut output = PtyOutput::default();
    wait_for(
        "PowerShell -NoExit -Command did not execute inside the PTY",
        &pty_marker,
        0,
        &mut output,
        &receiver,
        &mut session,
    );
    wait_for(
        "PSReadLine import did not complete",
        &module_marker,
        0,
        &mut output,
        &receiver,
        &mut session,
    );
    wait_for(
        "PowerShell interactive prompt did not appear",
        &prompt_marker,
        0,
        &mut output,
        &receiver,
        &mut session,
    );
    let interactive_start = output.visible.len();
    session
        .write(format!("Write-Output ('__CTRL_ARROW_INTERACTIVE_' + '{nonce:x}__')").as_bytes())
        .unwrap();
    wait_for(
        "PowerShell interactive input did not echo",
        "Write-Output",
        interactive_start,
        &mut output,
        &receiver,
        &mut session,
    );
    session.write(b"\r").unwrap();
    wait_for(
        "PowerShell interactive input did not execute",
        &interactive_marker,
        interactive_start,
        &mut output,
        &receiver,
        &mut session,
    );
    session.terminate().unwrap();
}

#[test]
fn powershell_psreadline_moves_by_words_for_control_arrows() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let prompt = format!("__CTRL_ARROW_PROMPT_{nonce:x}__");
    let ready = format!("__CTRL_ARROW_READY_{nonce:x}__");

    // Setup runs before the interactive reader starts. The markers are assembled by PowerShell,
    // so neither startup source text nor later command echo can satisfy the assertions.
    let setup = format!(
        "Import-Module PSReadLine -ErrorAction Stop; \
         function prompt {{ ('__CTRL_ARROW_PROMPT_' + '{nonce:x}__') }}; \
         function EmitArrowResult {{ param($Direction, $First, $Second) \
         Write-Output ('__CTRL_ARROW_' + $Direction + '_' + $First + '_' + $Second + '_{nonce:x}__') }}; \
         Write-Output ('__CTRL_ARROW_READY_' + '{nonce:x}__')"
    );
    let (mut session, receiver) = spawn_power_shell(setup);

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
        &prompt,
        0,
        &mut output,
        &receiver,
        &mut session,
    );

    let left = format!("__CTRL_ARROW_LEFT_alpha_Xbeta_{nonce:x}__");
    let left_start = output.visible.len();
    session.write(b"EmitArrowResult LEFT alpha beta").unwrap();
    wait_for(
        "Ctrl+Left edit line not visible",
        "EmitArrowResult LEFT alpha beta",
        left_start,
        &mut output,
        &receiver,
        &mut session,
    );
    session.write(b"\x1b[1;5DX\r").unwrap();
    wait_for(
        "Ctrl+Left result missing",
        &left,
        left_start,
        &mut output,
        &receiver,
        &mut session,
    );

    let right = format!("__CTRL_ARROW_RIGHT_alpha_Xbeta_{nonce:x}__");
    let right_start = output.visible.len();
    session.write(b"EmitArrowResult RIGHT alpha beta").unwrap();
    wait_for(
        "Ctrl+Right edit line not visible",
        "EmitArrowResult RIGHT alpha beta",
        right_start,
        &mut output,
        &receiver,
        &mut session,
    );
    session.write(b"\x1b[1;5D\x1b[1;5D\x1b[1;5CX\r").unwrap();
    wait_for(
        "Ctrl+Right result missing",
        &right,
        right_start,
        &mut output,
        &receiver,
        &mut session,
    );
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
