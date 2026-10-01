use terminal_core::TerminalState;

pub fn configure_history(terminal: &mut TerminalState) {
    if let Ok(capacity) = std::env::var("TERMINAL_THROUGHPUT_HISTORY") {
        let capacity = capacity.parse().unwrap();
        let prefill = std::env::var("TERMINAL_THROUGHPUT_PREFILL")
            .ok()
            .map(|v| v.parse().unwrap())
            .unwrap_or(0);
        terminal.configure_throughput_history(capacity, prefill);
    }
}

pub fn report_core(terminal: &TerminalState) {
    if let Some(stats) = terminal.throughput_stats() {
        eprintln!(
            "throughput core={stats:?} lf_ms={:.3} history_ms={:.3} grid_scroll_ms={:.3}",
            stats.line_feed.as_secs_f64() * 1000.0,
            stats.history.as_secs_f64() * 1000.0,
            stats.grid_scroll.as_secs_f64() * 1000.0
        );
    }
}
