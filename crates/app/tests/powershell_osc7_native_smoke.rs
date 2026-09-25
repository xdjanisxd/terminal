#![cfg(windows)]

use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use terminal_pty::{
    PortablePtyBackend, PtyBackend, PtyLifecycle, PtySession, PtySize, PtySpawnConfig,
};

const INTEGRATION: &str = include_str!("../src/powershell_osc7.ps1");

// Read the accumulated PTY bytes as a VT stream. PSReadLine can insert repaint
// sequences between bytes of text written to the console.
fn observed_output(raw: &[u8]) -> (String, usize) {
    enum State {
        Text,
        Escape,
        Csi,
        Osc,
        OscEscape,
        ControlString,
        ControlStringEscape,
    }

    let mut state = State::Text;
    let mut text = Vec::new();
    let mut osc = Vec::new();
    let mut ready_count = 0;
    for &byte in raw {
        state = match state {
            State::Text => match byte {
                0x1b => State::Escape,
                b'\n' => {
                    text.push(b'\n');
                    State::Text
                }
                b'\r' => State::Text,
                0x20..=0xff => {
                    text.push(byte);
                    State::Text
                }
                _ => State::Text,
            },
            State::Escape => match byte {
                b'[' => State::Csi,
                b']' => {
                    osc.clear();
                    State::Osc
                }
                b'P' | b'_' | b'^' | b'X' => State::ControlString,
                _ => State::Text,
            },
            State::Csi => {
                if (0x40..=0x7e).contains(&byte) {
                    State::Text
                } else {
                    State::Csi
                }
            }
            State::Osc => match byte {
                0x07 => {
                    ready_count += usize::from(osc == b"133;A");
                    State::Text
                }
                0x1b => State::OscEscape,
                _ => {
                    osc.push(byte);
                    State::Osc
                }
            },
            State::OscEscape => {
                if byte == b'\\' {
                    ready_count += usize::from(osc == b"133;A");
                    State::Text
                } else {
                    osc.push(byte);
                    State::Osc
                }
            }
            State::ControlString => {
                if byte == 0x1b {
                    State::ControlStringEscape
                } else {
                    State::ControlString
                }
            }
            State::ControlStringEscape => {
                if byte == b'\\' {
                    State::Text
                } else {
                    State::ControlString
                }
            }
        };
    }
    (String::from_utf8_lossy(&text).into_owned(), ready_count)
}

fn output_tail(output: &str) -> String {
    output
        .chars()
        .rev()
        .take(1200)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

#[test]
fn vt_observation_ignores_repaint_and_osc_around_split_output_markers() {
    let chunks: [&[u8]; 4] = [
        b"\x1b]0;PowerShell\x07\x1b]133;A\x07Write-Output ('__STARTUP_' + 'EXECUTED__')\r\n",
        b"\x1b[1G__START",
        b"\x1b[32mUP_\x1b[0m\r\x1b[2GEXEC",
        b"UTED__\r\n\x1b]133;A\x07",
    ];
    let mut raw = Vec::new();
    for chunk in chunks {
        raw.extend_from_slice(chunk);
    }
    let (text, ready_count) = observed_output(&raw);
    assert!(text.contains("__STARTUP_EXECUTED__"), "{text:?}");
    assert_eq!(ready_count, 2);
    assert!(!text.contains("PowerShell"), "{text:?}");
}

fn run_script(directory: &Path, body: &str, custom_prompt: bool) -> String {
    let custom_prompt = if custom_prompt {
        "function prompt { 'CUSTOM> ' }"
    } else {
        ""
    };
    let command = format!("{custom_prompt}\n& {{ {INTEGRATION} }}\n{body}");
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", &command])
        .current_dir(directory.parent().unwrap_or(directory))
        .env("TERMINAL_TEST_TARGET", directory)
        .output()
        .expect("Windows PowerShell should be available for the native smoke test");
    assert!(
        output.status.success(),
        "PowerShell failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("PowerShell output should be UTF-8")
}

#[test]
fn prompt_reports_drive_paths_spaces_and_unicode_without_changing_custom_prompt() {
    let root =
        std::env::temp_dir().join(format!("terminal-powershell-osc7-{}", std::process::id()));
    for (name, encoded) in [
        ("normal", "normal"),
        ("with spaces", "with%20spaces"),
        ("目录", "%E7%9B%AE%E5%BD%95"),
    ] {
        let directory = root.join(name);
        fs::create_dir_all(&directory).unwrap();
        let output = run_script(
            &directory,
            "Set-Location -LiteralPath $env:TERMINAL_TEST_TARGET; prompt",
            true,
        );
        assert!(output.contains("CUSTOM> "), "{output:?}");
        assert!(output.contains("\x1b]7;file:///"), "{output:?}");
        assert!(output.contains(encoded), "{output:?}");
        assert_eq!(output.matches("\x1b]7;").count(), 1);
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn default_prompt_still_renders() {
    let output = run_script(&std::env::temp_dir(), "prompt", false);
    assert!(output.contains("\x1b]7;file:///"), "{output:?}");
    assert!(output.contains('>'), "{output:?}");
}

#[test]
fn sourcing_twice_does_not_wrap_recursively_and_unavailable_locations_are_safe() {
    let output = run_script(
        &std::env::temp_dir(),
        &format!(
            "& {{ {INTEGRATION} }}\nprompt\nSet-Location Env:\nprompt\nfunction Get-Location {{ throw 'unavailable' }}\nprompt"
        ),
        true,
    );
    assert_eq!(output.matches("CUSTOM> ").count(), 3, "{output:?}");
    assert_eq!(output.matches("\x1b]7;").count(), 1, "{output:?}");
}

#[test]
fn saved_command_runs_in_fresh_shell_at_cwd_and_shell_survives_exit() {
    let directory = std::env::temp_dir().join(format!("terminal-startup-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let config = PtySpawnConfig::new(
        PathBuf::from("powershell.exe"),
        PtySize::new(24, 80).unwrap(),
    )
    .with_arguments([
        OsString::from("-NoLogo"),
        OsString::from("-NoExit"),
        OsString::from("-Command"),
        OsString::from(INTEGRATION),
    ])
    .with_working_directory(directory.clone());
    let mut session = PortablePtyBackend::new().spawn(config).unwrap();
    let mut reader = session.take_output_reader().unwrap();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        while let Ok(count) = reader.read(&mut chunk) {
            if count == 0 || sender.send(chunk[..count].to_vec()).is_err() {
                break;
            }
        }
    });
    let mut output = Vec::new();
    let mut answered_cursor_query = false;
    fn wait_for(
        marker: &str,
        ready_occurrences: usize,
        output: &mut Vec<u8>,
        receiver: &mpsc::Receiver<Vec<u8>>,
        session: &mut dyn PtySession,
        answered_cursor_query: &mut bool,
    ) {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let (logical_text, ready_count) = observed_output(output);
            if logical_text.contains(marker) && ready_count >= ready_occurrences {
                break;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(
                !remaining.is_zero(),
                "waiting for marker {marker:?} and prompt-ready #{ready_occurrences}; observed {ready_count}; normalized tail {:?}",
                output_tail(&logical_text)
            );
            let chunk = receiver.recv_timeout(remaining).unwrap_or_else(|error| {
                panic!(
                    "waiting for marker {marker:?} and prompt-ready #{ready_occurrences}: {error:?}; observed {ready_count}; normalized tail {:?}; lifecycle {:?}",
                    output_tail(&logical_text),
                    session.lifecycle()
                )
            });
            output.extend(chunk);
            if !*answered_cursor_query && output.windows(4).any(|bytes| bytes == b"\x1b[6n") {
                session.write(b"\x1b[1;1R").unwrap();
                *answered_cursor_query = true;
            }
        }
    }
    wait_for(
        "",
        1,
        &mut output,
        &receiver,
        &mut session,
        &mut answered_cursor_query,
    );
    session.write(b"Write-Output ('__STARTUP_' + 'EXECUTED__'); Write-Output ('__OBSERVED_' + 'CWD__' + (Get-Location).Path); cmd.exe /d /c exit 0; Write-Output ('__CHILD_' + 'EXITED__')\r").unwrap();
    wait_for(
        "__STARTUP_EXECUTED__",
        1,
        &mut output,
        &receiver,
        &mut session,
        &mut answered_cursor_query,
    );
    let expected_cwd = format!("__OBSERVED_CWD__{}", directory.display());
    wait_for(
        &expected_cwd,
        1,
        &mut output,
        &receiver,
        &mut session,
        &mut answered_cursor_query,
    );
    wait_for(
        "__CHILD_EXITED__",
        1,
        &mut output,
        &receiver,
        &mut session,
        &mut answered_cursor_query,
    );
    wait_for(
        "",
        2,
        &mut output,
        &receiver,
        &mut session,
        &mut answered_cursor_query,
    );
    session
        .write(b"Write-Output ('__SHELL_' + 'REUSABLE__')\r")
        .unwrap();
    wait_for(
        "__SHELL_REUSABLE__",
        1,
        &mut output,
        &receiver,
        &mut session,
        &mut answered_cursor_query,
    );
    assert_eq!(session.lifecycle(), PtyLifecycle::Running);
    session.terminate().unwrap();
    drop(session);
    let _ = fs::remove_dir_all(directory);
}
