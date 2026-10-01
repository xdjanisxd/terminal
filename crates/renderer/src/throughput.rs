//! Aggregate CPU-side measurements; enabled separately from per-event tracing.
use std::time::Duration;

/// These timings cover CPU work and present calls, not GPU completion/display latency.
#[derive(Clone, Copy, Debug, Default)]
pub struct ThroughputRenderStats {
    pub rendered: u64,
    pub presented: u64,
    pub projection: Duration,
    pub generation: Duration,
    pub upload: Duration,
    pub submission: Duration,
    pub render_present: Duration,
    pub buffer_allocations: u64,
    pub buffer_bytes: u64,
}

pub(crate) fn enabled() -> bool {
    std::env::var_os("TERMINAL_THROUGHPUT_DIAGNOSTICS").is_some_and(|v| v == "1")
}
