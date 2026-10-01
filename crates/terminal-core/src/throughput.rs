//! Opt-in aggregate semantic work, with elapsed timings (not allocation profiling).
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default)]
pub struct CoreThroughputStats {
    pub prints: u64,
    pub line_feeds: u64,
    pub carriage_returns: u64,
    pub transition_moves: u64,
    pub scrolls: u64,
    pub scrolled_rows: u64,
    pub history_pushes: u64,
    pub history_trims: u64,
    pub row_allocations: u64,
    pub history_cells_copied: u64,
    pub grid_cells_moved: u64,
    pub grid_cells_cleared: u64,
    pub line_feed: Duration,
    pub history: Duration,
    pub grid_scroll: Duration,
}

pub(crate) fn enabled() -> bool {
    std::env::var_os("TERMINAL_THROUGHPUT_DIAGNOSTICS").is_some_and(|v| v == "1")
}
