use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use terminal_pty::{
    PortablePtyBackend, PortablePtySession, PtyBackend, PtyError, PtySession, PtySize,
    PtySpawnConfig,
};

pub(crate) enum ReadEvent {
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

    // Separate process startup failures from ConPTY startup failures.
    let probe = Command::new(&program)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Write-Output ('__POWERSHELL_PROCESS_' + 'READY__'); Write-Output ('VERSION=' + $PSVersionTable.PSVersion.ToString()); Write-Output ('ARCH=' + [System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture); Write-Output ('PSREADLINE=' + [bool](Get-Module -ListAvailable PSReadLine))",
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
        probe.status.success() && stdout.contains("__POWERSHELL_PROCESS_READY__"),
        "selected PowerShell {program:?} did not execute the direct probe; status={}, stdout={stdout:?}, stderr={stderr:?}",
        probe.status
    );
    program
}

pub(crate) fn spawn_power_shell(
    setup: String,
    use_profiles: bool,
) -> (PortablePtySession, Receiver<ReadEvent>) {
    let program = power_shell_program();
    let mut arguments = vec![OsString::from("-NoLogo")];
    if !use_profiles {
        arguments.push(OsString::from("-NoProfile"));
    }
    arguments.extend([
        OsString::from("-NoExit"),
        OsString::from("-Command"),
        OsString::from(setup),
    ]);
    let config = PtySpawnConfig::new(program.clone(), PtySize::new(24, 80).unwrap())
        .with_arguments(arguments);
    let mut session = PortablePtyBackend::new()
        .spawn(config)
        .unwrap_or_else(|error| panic!("PTY spawn failed for {program:?}: {error}"));
    eprintln!(
        "PowerShell PTY spawn: executable={program:?}, profiles_enabled={use_profiles}, lifecycle={:?}",
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
