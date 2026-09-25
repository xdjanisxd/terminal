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
        OsString::from("-NoProfile"),
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
    fn wait_for(
        needle: &[u8],
        output: &mut Vec<u8>,
        receiver: &mpsc::Receiver<Vec<u8>>,
        session: &mut dyn PtySession,
    ) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while !output.windows(needle.len()).any(|bytes| bytes == needle) {
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(
                !remaining.is_zero(),
                "missing {:?}: {:?}",
                needle,
                String::from_utf8_lossy(output)
            );
            output.extend(receiver.recv_timeout(remaining).expect("PTY output closed"));
            if output.windows(4).any(|bytes| bytes == b"\x1b[6n") {
                session.write(b"\x1b[1;1R").unwrap();
            }
        }
    }
    wait_for(b"\x1b]133;A\x07", &mut output, &receiver, &mut session);
    session.write(b"Write-Output ('__STARTUP_' + 'CWD__' + (Get-Location).Path); Write-Output ('__STARTUP_' + 'DONE__')\r").unwrap();
    wait_for(b"__STARTUP_DONE__", &mut output, &receiver, &mut session);
    let expected_cwd = format!("__STARTUP_CWD__{}", directory.display());
    wait_for(
        expected_cwd.as_bytes(),
        &mut output,
        &receiver,
        &mut session,
    );
    session
        .write(b"Write-Output ('__SHELL_' + 'STILL_HERE__')\r")
        .unwrap();
    wait_for(
        b"__SHELL_STILL_HERE__",
        &mut output,
        &receiver,
        &mut session,
    );
    assert_eq!(session.lifecycle(), PtyLifecycle::Running);
    session.terminate().unwrap();
    drop(session);
    let _ = fs::remove_dir_all(directory);
}
