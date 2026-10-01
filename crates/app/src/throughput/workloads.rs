mod core;
use std::io::{BufRead, Write};
use std::time::Instant;
use terminal_core::{TerminalDimensions, TerminalParser, TerminalState};
pub const WORKLOADS: [&str; 7] = [
    "finite",
    "short",
    "long",
    "ansi",
    "continuous",
    "input",
    "idle",
];
pub fn block(name: &str) -> Vec<u8> {
    match name {
        "idle" => b"IDLE_READY\r\n".to_vec(),
        "short" => b"x\r\n".repeat(1024),
        "long" => [vec![b'x'; 8192], b"\r\n".to_vec()].concat(),
        "ansi" => b"\x1b[31mred\x1b[0m \x1b[38;2;20;40;60mcolor\x1b[0m\r\n".repeat(128),
        _ => b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ\r\n".repeat(128),
    }
}

pub fn repetitions(name: &str) -> usize {
    if name == "idle" {
        return 1;
    }
    let target = match name {
        "short" => 1024 * 1024,
        "continuous" | "input" => 32 * 1024 * 1024,
        _ => 8 * 1024 * 1024,
    };
    target / block(name).len()
}

pub fn child(name: &str) {
    assert!(WORKLOADS.contains(&name));
    if name == "idle" {
        let mut out = std::io::stdout().lock();
        out.write_all(&block(name)).unwrap();
        out.flush().unwrap();
        // Deliberate quiet interval in the workload, never renderer pacing.
        std::thread::sleep(std::time::Duration::from_secs(3));
        out.write_all(b"IDLE_DONE\r\n").unwrap();
        return;
    }
    // Only the input workload needs an independent child-side stdin reader.
    // Lock stdout per block so acknowledgement output can make progress.
    let input = (name == "input").then(|| {
        std::thread::spawn(|| {
            let mut line = String::new();
            std::io::stdin().lock().read_line(&mut line).unwrap();
            assert_eq!(line.trim(), "probe");
            writeln!(std::io::stdout().lock(), "THROUGHPUT_ACK\r").unwrap();
        })
    });
    let block = block(name);
    for _ in 0..repetitions(name) {
        std::io::stdout().lock().write_all(&block).unwrap();
    }
    if let Some(input) = input {
        input.join().unwrap();
    }
}

pub fn parser_baseline(name: &str) {
    assert!(WORKLOADS.contains(&name));
    let mut parser = TerminalParser::new();
    let rows = std::env::var("TERMINAL_THROUGHPUT_ROWS")
        .ok()
        .map(|v| v.parse().unwrap())
        .unwrap_or(30);
    let columns = std::env::var("TERMINAL_THROUGHPUT_COLUMNS")
        .ok()
        .map(|v| v.parse().unwrap())
        .unwrap_or(80);
    let mut terminal = TerminalState::new(TerminalDimensions::new(columns, rows).unwrap());
    core::configure_history(&mut terminal);
    let block = block(name);
    let start = Instant::now();
    let mut batches = 0;
    for _ in 0..repetitions(name) {
        for chunk in block.chunks(terminal_pty::PTY_READ_CHUNK_SIZE) {
            parser.advance(&mut terminal, chunk).unwrap();
            batches += 1;
        }
    }
    let bytes = block.len() * repetitions(name);
    let elapsed = start.elapsed();
    eprintln!(
        "throughput mode=parser workload={name} bytes={bytes} elapsed_ms={:.3} mb_s={:.3} batches={batches} scrollback={} columns={columns} rows={rows}",
        elapsed.as_secs_f64() * 1000.0,
        bytes as f64 / elapsed.as_secs_f64() / 1e6,
        terminal.scrollback_len()
    );
    core::report_core(&terminal);
}
