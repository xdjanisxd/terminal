#![cfg(windows)]

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use terminal_pty::{PortablePtyBackend, PtyBackend, PtySession, PtySize, PtySpawnConfig};

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

    let mut output = Vec::new();
    let mut queried = false;
    let mut until = |marker: &[u8], session: &mut dyn PtySession| {
        let deadline = Instant::now() + Duration::from_secs(15);
        let start = output.len();
        loop {
            assert!(
                Instant::now() < deadline,
                "marker {:?} missing; output {:?}",
                marker,
                String::from_utf8_lossy(&output)
            );
            if let Ok(chunk) = receiver.recv_timeout(Duration::from_millis(100)) {
                output.extend(chunk);
                if !queried && output.windows(4).any(|w| w == b"\x1b[6n") {
                    session.write(b"\x1b[1;1R").unwrap();
                    queried = true;
                }
                if output[start..].windows(marker.len()).any(|w| w == marker) {
                    return;
                }
            }
        }
    };

    session.write(b"Write-Output LEFT alpha beta").unwrap();
    until(b"LEFT alpha beta", &mut session);
    session.write(b"\x1b[1;5DX\r").unwrap();
    until(b"LEFT\r\nalpha\r\nXbeta\r\n", &mut session);

    session.write(b"Write-Output RIGHT alpha beta").unwrap();
    until(b"RIGHT alpha beta", &mut session);
    session.write(b"\x1b[1;5D\x1b[1;5D\x1b[1;5CX\r").unwrap();
    until(b"RIGHT\r\nalpha\r\nXbeta\r\n", &mut session);
    session.terminate().unwrap();
}
