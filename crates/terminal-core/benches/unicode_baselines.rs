//! Identical fixed input on baseline/candidate; old output is not a correctness oracle.
use std::{hint::black_box, time::Instant};
use terminal_core::{Cell, TerminalDimensions, TerminalParser, TerminalState};

fn main() {
    if cfg!(debug_assertions) {
        println!("Unicode throughput measurements require cargo bench (release mode)");
        return;
    }
    println!(
        "cell_bytes={} state_bytes={}",
        std::mem::size_of::<Cell>(),
        std::mem::size_of::<TerminalState>()
    );
    for (name, text) in [
        ("ascii", "abcdefghijklmnopqrstuvwxyz0123456789"),
        ("combining", "e\u{301}"),
        ("zwj", "👩‍💻"),
        ("flag", "🇺🇸"),
        ("keycap", "1️⃣"),
    ] {
        let input = format!("\r{text}");
        let mut state = TerminalState::new(TerminalDimensions::new(80, 24).unwrap());
        let mut parser = TerminalParser::new();
        for _ in 0..1000 {
            parser.advance(&mut state, input.as_bytes()).unwrap();
        }
        for sample in 0..5 {
            let started = Instant::now();
            for _ in 0..50_000 {
                parser
                    .advance(&mut state, black_box(input.as_bytes()))
                    .unwrap();
                black_box(state.cursor());
            }
            println!(
                "parse_{name} sample={sample} operations=50000 ns_per_operation={:.1}",
                started.elapsed().as_nanos() as f64 / 50_000.0
            );
        }
    }
}
