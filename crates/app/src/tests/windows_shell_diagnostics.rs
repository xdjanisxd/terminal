//! Boundary probe: no launch-policy changes or architecture exceptions.
use super::{SmokeSession, SmokeTerminal, SmokeTransport};
use std::{
    io::Read,
    os::windows::process::CommandExt,
    process::{Command, Stdio},
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};
use terminal_config::ShellConfig;
use terminal_pty::{PtyOutput, PtySize, PtyWorker, PtyWorkerEvent};

const MARKER: &str = "TERMINAL_MINIMAL_MARKER";
const LIMIT: Duration = Duration::from_secs(15);

fn encoded_command(command: &str) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes: Vec<_> = command.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut result = String::new();
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for index in 0..4 {
            result.push(if index > chunk.len() {
                '='
            } else {
                ALPHABET[((value >> (18 - index * 6)) & 63) as usize] as char
            });
        }
    }
    result
}

#[test]
fn encoded_command_uses_utf16le() {
    assert_eq!(encoded_command("exit 0"), "ZQB4AGkAdAAgADAA");
    assert_eq!(encoded_command("A"), "QQA=");
    assert_eq!(encoded_command("AB"), "QQBCAA==");
    assert_eq!(encoded_command(""), "");
}

fn pe_machine(program: &std::path::Path) -> String {
    let inspect = || -> Option<u16> {
        let bytes = std::fs::read(program).ok()?;
        if bytes.get(..2)? != b"MZ" {
            return None;
        }
        let offset = u32::from_le_bytes(bytes.get(60..64)?.try_into().ok()?) as usize;
        if bytes.get(offset..offset.checked_add(4)?)? != b"PE\0\0" {
            return None;
        }
        Some(u16::from_le_bytes(
            bytes.get(offset + 4..offset + 6)?.try_into().ok()?,
        ))
    };
    match inspect() {
        Some(machine) => format!(
            "0x{machine:04x} ({})",
            match machine {
                0x14c => "x86",
                0x8664 => "x64",
                0xaa64 => "ARM64",
                0xa641 => "ARM64EC",
                0xa64e => "ARM64X",
                _ => "unknown",
            }
        ),
        None => "unavailable".into(),
    }
}

fn conpty(config: terminal_pty::PtySpawnConfig, marker_required: bool, prompt: bool) -> bool {
    let started = Instant::now();
    let program = config.program().to_owned();
    let args = config.arguments().to_vec();
    let transport = Arc::new(Mutex::new(SmokeTransport::default()));
    let session =
        match super::super::shell::spawn(&terminal_pty::PortablePtyBackend::new(), config, None) {
            Ok(session) => session,
            Err(error) => {
                eprintln!("ConPTY {program:?} args={args:?} spawn_error={error}");
                return false;
            }
        };
    let mut worker = PtyWorker::start(SmokeSession {
        session,
        transport: transport.clone(),
    })
    .unwrap();
    let spawn_elapsed = started.elapsed();
    let mut smoke = SmokeTerminal::new();
    let mut first_output = None;
    let mut exit = None;
    let mut eof = false;
    let mut errors = Vec::new();
    while started.elapsed() < LIMIT && !(exit.is_some() && eof) {
        match worker.recv_timeout(Duration::from_millis(100)) {
            Ok(Some(PtyWorkerEvent::Output(PtyOutput::Bytes(chunk)))) => {
                first_output.get_or_insert(started.elapsed());
                smoke.feed(&chunk, |reply| worker.write(reply));
            }
            Ok(Some(PtyWorkerEvent::Output(PtyOutput::Exited(status)))) => exit = Some(status),
            Ok(Some(PtyWorkerEvent::Output(PtyOutput::Eof))) => eof = true,
            Ok(Some(PtyWorkerEvent::Error(error))) | Err(error) => {
                errors.push(error.to_string());
                break;
            }
            _ => {}
        }
    }
    let elapsed = started.elapsed();
    let state = transport.lock().unwrap().process_state;
    let shutdown = worker.shutdown_and_join();
    let raw = String::from_utf8_lossy(&smoke.raw);
    let marker = smoke.marker_observed(MARKER);
    let osc7 = raw.contains("\x1b]7;file:");
    let osc133 = raw.contains("\x1b]133;A");
    eprintln!(
        "ConPTY program={program:?} args={args:?} spawned={spawn_elapsed:?} first_output={first_output:?} elapsed={elapsed:?} exit={exit:?} eof={eof} state_before_shutdown={state:?} marker={marker} osc7={osc7} osc133={osc133} queries={:?} generated={:?} queued={:?} queue_errors={:?} transport={:?} errors={errors:?} shutdown={shutdown:?} visible={:?} raw_vt={raw:?}",
        smoke.query_candidates(),
        smoke.replies_generated,
        smoke.replies_queued,
        smoke.reply_queue_failures,
        transport.lock().unwrap(),
        smoke.visible()
    );
    exit.and_then(|status| status.code_value()) == Some(0)
        && errors.is_empty()
        && shutdown.is_ok()
        && smoke.reply_queue_failures.is_empty()
        && transport.lock().unwrap().reply_write_failures.is_empty()
        && (!marker_required || marker)
        && (!prompt || (osc7 && osc133))
}

fn redirected(config: &terminal_pty::PtySpawnConfig, marker_required: bool, prompt: bool) -> bool {
    let started = Instant::now();
    // Detached, redirected CreateProcess control; keep stdin open like the PTY.
    let mut child = match Command::new(config.program())
        .args(config.arguments())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x0800_0000)
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            eprintln!(
                "redirected program={:?} spawn_error={error}",
                config.program()
            );
            return false;
        }
    };
    let spawn_elapsed = started.elapsed();
    let (send, receive) = mpsc::channel();
    let readers: Vec<Box<dyn Read + Send>> = vec![
        Box::new(child.stdout.take().unwrap()),
        Box::new(child.stderr.take().unwrap()),
    ];
    let threads: Vec<_> = readers
        .into_iter()
        .map(|mut reader| {
            let send = send.clone();
            std::thread::spawn(move || {
                let mut bytes = [0; 4096];
                loop {
                    match reader.read(&mut bytes) {
                        Ok(0) => break,
                        Ok(count) => {
                            if send.send(Ok(bytes[..count].to_vec())).is_err() {
                                break;
                            }
                        }
                        Err(error) => {
                            let _ = send.send(Err(error));
                            break;
                        }
                    }
                }
            })
        })
        .collect();
    drop(send);
    let mut raw = Vec::new();
    let mut first_output = None;
    let mut exit = None;
    let mut eof = false;
    let mut errors = Vec::new();
    while started.elapsed() < LIMIT && !(exit.is_some() && eof) {
        match receive.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(bytes)) => {
                first_output.get_or_insert(started.elapsed());
                raw.extend(bytes);
            }
            Ok(Err(error)) => errors.push(error.to_string()),
            Err(mpsc::RecvTimeoutError::Disconnected) => eof = true,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        match child.try_wait() {
            Ok(status) => exit = status,
            Err(error) => {
                errors.push(error.to_string());
                break;
            }
        }
    }
    let elapsed = started.elapsed();
    if exit.is_none() {
        let _ = child.kill();
    }
    let shutdown = child.wait();
    for thread in threads {
        thread.join().unwrap();
    }
    let text = String::from_utf8_lossy(&raw);
    // Redirected output does not echo the command. OSC metadata can precede
    // the unique marker on the same line, so prompt formatting is irrelevant.
    let marker = text.contains(MARKER);
    let osc7 = text.contains("\x1b]7;file:");
    let osc133 = text.contains("\x1b]133;A");
    eprintln!(
        "redirected program={:?} args={:?} spawned={spawn_elapsed:?} first_output={first_output:?} elapsed={elapsed:?} exit={exit:?} marker={marker} osc7={osc7} osc133={osc133} errors={errors:?} shutdown={shutdown:?} raw={text:?}",
        config.program(),
        config.arguments()
    );
    exit.is_some_and(|status| status.success())
        && errors.is_empty()
        && (!marker_required || marker)
        && (!prompt || (osc7 && osc133))
}

#[test]
fn native_powershell_boundary_probe() {
    eprintln!(
        "tester_arch={} PROCESSOR_ARCHITECTURE={:?} PROCESSOR_ARCHITEW6432={:?}",
        std::env::consts::ARCH,
        std::env::var("PROCESSOR_ARCHITECTURE"),
        std::env::var("PROCESSOR_ARCHITEW6432")
    );
    let mut failures = Vec::new();
    let marker_command = format!("Write-Output '{MARKER}'; exit 0");
    for name in ["powershell.exe", "pwsh.exe"] {
        let Some(program) = super::super::executable_on_path(name) else {
            eprintln!("control executable {name} unavailable");
            if name == "powershell.exe" {
                failures.push("required legacy PowerShell executable unavailable".into());
            }
            continue;
        };
        eprintln!(
            "resolved_program={program:?} PE_machine={} (image identity; ConPTY process architecture unavailable through backend API)",
            pe_machine(&program)
        );
        let integration = include_str!("../powershell_osc7.ps1");
        let cases = [
            ("A-exit", "-Command", "exit 0".into(), false, false),
            ("B-marker", "-Command", marker_command.clone(), true, false),
            (
                "C-console",
                "-Command",
                format!("[Console]::WriteLine('{MARKER}'); exit 0"),
                true,
                false,
            ),
            (
                "D-encoded",
                "-EncodedCommand",
                encoded_command(&marker_command),
                true,
                false,
            ),
            (
                "E-integration",
                "-Command",
                format!(
                    "Write-Output 'TERMINAL_COMMAND_BEGIN'; & {{ {integration} }}; if (!$global:__terminalOsc7Installed) {{ exit 1 }}; {marker_command}"
                ),
                true,
                false,
            ),
            (
                "F-prompt-OSC7-OSC133",
                "-Command",
                format!(
                    "Write-Output 'TERMINAL_COMMAND_BEGIN'; & {{ {integration} }}; Write-Output 'TERMINAL_INTEGRATION_INSTALLED'; & prompt | Out-Null; {marker_command}"
                ),
                true,
                true,
            ),
            (
                "G-identity",
                "-Command",
                format!(
                    "Write-Output ('PSVersion=' + $PSVersionTable.PSVersion); Write-Output ('ProcessArchitecture=' + [System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture); Write-Output ('OSArchitecture=' + [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture); Write-Output ('WindowsVersion=' + [Environment]::OSVersion.Version); {marker_command}"
                ),
                true,
                false,
            ),
        ];
        for (case, mode, body, marker, prompt) in cases {
            let config = super::super::shell::configured_spawn_config(
                &ShellConfig {
                    program: program.to_string_lossy().into_owned(),
                    args: vec![
                        "-NoLogo".into(),
                        "-NoProfile".into(),
                        "-NonInteractive".into(),
                        mode.into(),
                        body,
                    ],
                },
                PtySize::new(24, 80).unwrap(),
            );
            eprintln!("boundary case={case} executable={name}");
            // Complete the matrix even after failures to retain the first boundary.
            if !conpty(config.clone(), marker, prompt) {
                failures.push(format!("{name}/{case}/ConPTY"));
            }
            if !redirected(&config, marker, prompt) {
                failures.push(format!("{name}/{case}/redirected"));
            }
        }
    }
    assert!(failures.is_empty(), "boundary failures: {failures:?}");
}
