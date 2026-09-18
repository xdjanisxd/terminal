use std::io::{self, Write};
use std::time::Duration;

fn main() -> io::Result<()> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let command = arguments.first().map(String::as_str);
    let exit_code = (command == Some("exit"))
        .then(|| arguments.get(1))
        .flatten()
        .and_then(|code| code.parse::<i32>().ok())
        .unwrap_or(0);

    let mut output = io::stdout().lock();
    output.write_all(b"TERMINAL_PTY_HELPER_MARKER")?;
    if command == Some("payload") {
        for _ in 0..128 {
            output.write_all(b"POST_EXIT_BYTES")?;
        }
    }
    output.flush()?;

    if command == Some("wait") {
        loop {
            std::thread::sleep(Duration::from_secs(60));
        }
    }
    std::process::exit(exit_code);
}
