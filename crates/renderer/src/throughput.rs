//! Opt-in aggregate renderer measurements. No timings describe scanout.
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::{Duration, Instant};

/// CPU wall times include driver calls. Completion latency includes queue backlog
/// and callback delivery delay; it is not isolated GPU execution time.
#[derive(Clone, Copy, Debug, Default)]
pub struct ThroughputRenderStats {
    pub rendered: u64,
    pub presented: u64,
    pub attempted: u64,
    pub skipped: u64,
    pub reconfigured_frames: u64,
    pub suboptimal: u64,
    pub surface_errors: u64,
    pub projection: Duration,
    pub generation: Duration,
    pub upload: Duration,
    /// Encoder creation, render pass recording and encoder.finish().
    pub encoding: Duration,
    /// Only queue.submit(), summed across panes.
    pub submission: Duration,
    pub acquire: Duration,
    pub present: Duration,
    pub reconfiguration: Duration,
    pub reconfigurations: u64,
    /// Renderer entry through outcome, including projection and driver calls.
    pub cpu_frame: Duration,
    pub cpu_frame_max: Duration,
    /// Legacy total: acquire through present, excluding projection/reconfigure.
    pub render_present: Duration,
    pub buffer_allocations: u64,
    pub buffer_bytes: u64,
    pub buffer_writes: u64,
    pub queue_submissions: u64,
    pub glyph_instances: u64,
    pub rectangle_instances: u64,
    pub shape_calls: u64,
    pub shape_misses: u64,
    pub raster_calls: u64,
    pub atlas_lookups: u64,
    pub gpu_completion_samples: u64,
    pub gpu_completion_pending: u64,
    pub gpu_completion_latency: Duration,
    pub gpu_completion_max: Duration,
    pub gpu_sample_busy: u64,
    pub completion_poll: Duration,
    pub completion_poll_errors: u64,
}

pub(crate) fn enabled() -> bool {
    std::env::var_os("TERMINAL_THROUGHPUT_DIAGNOSTICS").is_some_and(|v| v == "1")
}

/// At most one callback is outstanding; sample the first and every 16th frame.
/// Reset replaces the channel so late callbacks cannot contaminate a new run.
pub(crate) struct GpuCompletion {
    sender: SyncSender<Duration>,
    receiver: Receiver<Duration>,
    pending: bool,
    frames: u64,
}

impl Default for GpuCompletion {
    fn default() -> Self {
        let (sender, receiver) = mpsc::sync_channel(1);
        Self {
            sender,
            receiver,
            pending: false,
            frames: 0,
        }
    }
}

impl GpuCompletion {
    pub(crate) fn collect(&mut self, stats: &mut ThroughputRenderStats) {
        if let Ok(latency) = self.receiver.try_recv() {
            self.pending = false;
            stats.gpu_completion_samples += 1;
            stats.gpu_completion_latency += latency;
            stats.gpu_completion_max = stats.gpu_completion_max.max(latency);
        }
        stats.gpu_completion_pending = u64::from(self.pending);
    }

    fn sample(&mut self, stats: &mut ThroughputRenderStats) -> Option<SyncSender<Duration>> {
        let due = self.frames.is_multiple_of(16);
        self.frames += 1;
        if !due {
            return None;
        }
        if self.pending {
            stats.gpu_sample_busy += 1;
            return None;
        }
        self.pending = true;
        stats.gpu_completion_pending = 1;
        Some(self.sender.clone())
    }

    pub(crate) fn submitted(
        &mut self,
        queue: &wgpu::Queue,
        started: Instant,
        stats: &mut ThroughputRenderStats,
    ) {
        if let Some(sender) = self.sample(stats) {
            queue.on_submitted_work_done(move || {
                let _ = sender.try_send(started.elapsed());
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_are_bounded_and_collection_releases_slot() {
        let mut tracker = GpuCompletion::default();
        let mut stats = ThroughputRenderStats::default();
        let sender = tracker.sample(&mut stats).unwrap();
        for _ in 0..32 {
            assert!(tracker.sample(&mut stats).is_none());
        }
        assert_eq!(stats.gpu_sample_busy, 2);
        sender.try_send(Duration::from_millis(3)).unwrap();
        tracker.collect(&mut stats);
        tracker.collect(&mut stats);
        assert_eq!(stats.gpu_completion_samples, 1);
        assert_eq!(stats.gpu_completion_latency, Duration::from_millis(3));
        assert_eq!(stats.gpu_completion_max, Duration::from_millis(3));
        assert_eq!(stats.gpu_completion_pending, 0);
        for _ in 33..48 {
            assert!(tracker.sample(&mut stats).is_none());
        }
        assert!(tracker.sample(&mut stats).is_some());
    }

    #[test]
    fn reset_drops_old_callbacks_without_counting_them() {
        let mut tracker = GpuCompletion::default();
        let mut stats = ThroughputRenderStats::default();
        let sender = tracker.sample(&mut stats).unwrap();
        tracker = GpuCompletion::default();
        stats = ThroughputRenderStats::default();
        assert!(sender.try_send(Duration::from_millis(99)).is_err());
        tracker.collect(&mut stats);
        assert_eq!(stats.gpu_completion_samples, 0);
        assert!(tracker.sample(&mut stats).is_some());
    }
}
