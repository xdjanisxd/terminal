use std::hint::black_box;
use std::time::Instant;
use terminal_core::{TerminalDimensions, TerminalParser, TerminalState};
use terminal_renderer::TerminalRenderData;
use terminal_workspace::{PaneRect, SplitAxis, Workspace};

const COLUMNS: usize = 80;
const ROWS: usize = 30;
const ITERATIONS: usize = 500;

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

fn populated(columns: usize, rows: usize) -> TerminalState {
    let mut state = TerminalState::new(TerminalDimensions::new(columns, rows).unwrap());
    let mut parser = TerminalParser::new();
    for row in 0..rows {
        let line = format!(
            "\x1b[{};1H{:04} status: running  task complete",
            row + 1,
            row
        );
        parser.advance(&mut state, line.as_bytes()).unwrap();
    }
    state
}

fn main() {
    let state = populated(COLUMNS, ROWS);
    let projected_cells = TerminalRenderData::from_terminal(&state).cells.len();
    assert!(projected_cells > 0 && projected_cells < COLUMNS * ROWS);
    measure("visible_projection_80x30", ITERATIONS, || {
        let data = TerminalRenderData::from_terminal(black_box(&state));
        black_box(data.cells.len());
    });

    let mut parser = TerminalParser::new();
    let mut state = populated(COLUMNS, ROWS);
    let echo = b"\r\x1b[2Kprompt> a";
    measure("output_echo_plus_projection", ITERATIONS, || {
        parser.advance(&mut state, black_box(echo)).unwrap();
        let data = TerminalRenderData::from_terminal(&state);
        black_box(data.cells.len());
    });

    let mut workspace = Workspace::default();
    workspace.split_active(SplitAxis::Vertical);
    workspace.split_active(SplitAxis::Horizontal);
    let panes = [populated(40, 30), populated(40, 15), populated(40, 15)];
    let viewport = PaneRect {
        x: 0,
        y: 0,
        width: 800,
        height: 600,
    };
    assert_eq!(workspace.active_tab().pane_rects(viewport).len(), 3);
    measure("three_pane_layout_plus_projection", ITERATIONS, || {
        let rects = workspace.active_tab().pane_rects(black_box(viewport));
        for (index, (_, rect)) in rects.iter().enumerate() {
            black_box(rect);
            let data = TerminalRenderData::from_terminal(&panes[index]);
            black_box(data.cells.len());
        }
    });
}
