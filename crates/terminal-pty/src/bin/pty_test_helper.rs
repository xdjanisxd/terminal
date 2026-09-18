use std::io::{self, Write};

fn main() -> io::Result<()> {
    let mut output = io::stdout().lock();
    output.write_all(b"TERMINAL_PTY_HELPER_MARKER")?;
    output.flush()
}
