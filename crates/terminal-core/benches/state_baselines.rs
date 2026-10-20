use std::hint::black_box;
use std::time::Instant;
use terminal_core::{TerminalDimensions, TerminalParser, TerminalState};

const COLUMNS: usize = 80;
const ROWS: usize = 30;
const OUTPUT_ITERATIONS: usize = 2_000;
const NAVIGATION_ITERATIONS: usize = 10_000;

fn terminal() -> TerminalState {
    TerminalState::new(TerminalDimensions::new(COLUMNS, ROWS).unwrap())
}

fn measure(name: &str, operations: usize, mut operation: impl FnMut()) {
    let start = Instant::now();
    for _ in 0..operations {
        operation();
    }
    let elapsed = start.elapsed();
    println!(
        "{name}: operations={operations} elapsed_ns={} ns_per_operation={:.1}",
        elapsed.as_nanos(),
        elapsed.as_nanos() as f64 / operations as f64
    );
}

fn main() {
    let mut extreme = terminal();
    let mut parser = TerminalParser::new();
    for _ in 0..40 {
        parser
            .advance(
                &mut extreme,
                format!("{}界e\u{301}\r\n", "x".repeat(90)).as_bytes(),
            )
            .unwrap();
    }
    measure("resize_one_column_unicode_pair", 20, || {
        extreme.resize(TerminalDimensions::new(1, ROWS).unwrap());
        extreme.resize(TerminalDimensions::new(COLUMNS, ROWS).unwrap());
        black_box(&extreme);
    });
    for history in [0, 2_000, 10_000] {
        let mut state = terminal();
        let mut parser = TerminalParser::new();
        // Hard lines containing a soft wrap, attributes, wide and combining cells.
        let line = format!("\x1b[32m{}界e\u{301}\x1b[0m\r\n", "abcdefghij".repeat(9));
        for _ in 0..history / 2 {
            parser.advance(&mut state, line.as_bytes()).unwrap();
        }
        measure(
            &format!("resize_columns_pair_history_{history}"),
            100,
            || {
                state.resize(TerminalDimensions::new(60, ROWS).unwrap());
                state.resize(TerminalDimensions::new(COLUMNS, ROWS).unwrap());
                black_box(&state);
            },
        );
        measure(
            &format!("resize_height_pair_history_{history}"),
            100,
            || {
                state.resize(TerminalDimensions::new(COLUMNS, ROWS - 1).unwrap());
                state.resize(TerminalDimensions::new(COLUMNS, ROWS).unwrap());
                black_box(&state);
            },
        );
        measure(
            &format!("resize_unchanged_history_{history}"),
            10_000,
            || {
                state.resize(TerminalDimensions::new(COLUMNS, ROWS).unwrap());
                black_box(&state);
            },
        );
    }
    let plain = b"prompt> cargo test\r\nfinished successfully\r\n";
    let escapes =
        b"\x1b[?25l\x1b[1;32muser\x1b[0m$ \x1b[2K\r\x1b[38;2;20;40;60mresult\x1b[0m\r\n\x1b[?25h";
    let mut parser = TerminalParser::new();
    let mut state = terminal();
    measure("parse_plain_state", OUTPUT_ITERATIONS, || {
        parser.advance(&mut state, black_box(plain)).unwrap();
        black_box(state.cursor());
    });

    let mut parser = TerminalParser::new();
    let mut state = terminal();
    measure("parse_escape_state", OUTPUT_ITERATIONS, || {
        parser.advance(&mut state, black_box(escapes)).unwrap();
        black_box(state.cursor());
    });

    let mut state = terminal();
    for _ in 0..2_000 {
        state.index();
    }
    assert!(state.scrollback_len() > ROWS);
    measure("scrollback_page_pair", NAVIGATION_ITERATIONS, || {
        black_box(&mut state);
        black_box(state.page_up());
        black_box(state.page_down());
    });

    let mut state = terminal();
    assert!(state.begin_selection(0, 0));
    let mut column = 0;
    measure("selection_extend", NAVIGATION_ITERATIONS, || {
        black_box(&mut state);
        column = (column + 1) % COLUMNS;
        black_box(state.extend_selection(ROWS - 1, column));
    });
}
