//! Opt-in lifetime aggregates and deterministic, self-hosted release workloads.
use crate::{Application, PtyWake};
pub const WORKLOADS: [&str; 6] = ["finite", "short", "long", "ansi", "continuous", "input"];
use std::time::{Duration, Instant};
use terminal_pty::PtyBackend;
use winit::event_loop::ActiveEventLoop;

const ACK: &[u8] = b"THROUGHPUT_ACK";

pub fn enabled() -> bool {
    std::env::var_os("TERMINAL_THROUGHPUT_DIAGNOSTICS").is_some_and(|v| v == "1")
}

impl Application {
    pub(crate) fn start_throughput_workload(&mut self) {
        if self.pty.is_some() {
            return;
        }
        let name = self.throughput.as_ref().unwrap().workload.clone().unwrap();
        let helper = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .join("examples")
            .join(if cfg!(windows) {
                "throughput-workload.exe"
            } else {
                "throughput-workload"
            });
        assert!(
            helper.is_file(),
            "build the throughput-workload console example first"
        );
        let config = terminal_pty::PtySpawnConfig::new(
            helper,
            crate::pty_size_for_terminal(self.terminal.dimensions()),
        )
        .with_arguments(["child".into(), name.into()]);
        let proxy = self.pty_wake_proxy.clone().unwrap();
        let pending = self.pty_wake_pending.clone();
        let stats = self.throughput.as_mut().unwrap();
        *stats = Stats {
            workload: stats.workload.clone(),
            ..Stats::default()
        };
        self.renderer.as_mut().unwrap().reset_throughput_stats();
        let session = terminal_pty::PortablePtyBackend::new()
            .spawn(config)
            .unwrap();
        self.pty = Some(
            terminal_pty::PtyWorker::start_with_notifier(session, move || {
                if !pending.swap(true, std::sync::atomic::Ordering::AcqRel) {
                    let _ = proxy.send_event(PtyWake::OutputAvailable);
                }
            })
            .unwrap(),
        );
    }

    pub(crate) fn finish_throughput_workload(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let Some(stats) = self
            .throughput
            .as_ref()
            .filter(|stats| stats.workload.is_some())
        else {
            return false;
        };
        if !(stats.eof && stats.exit_code.is_some())
            && stats.started.elapsed() >= Duration::from_secs(60)
        {
            self.throughput.as_mut().unwrap().timed_out = true;
            eprintln!("throughput workload timed out; results incomplete");
            event_loop.exit();
            return true;
        }
        if !stats.eof || stats.exit_code.is_none() {
            return false;
        }
        if stats.data_dirty {
            self.redraw_window(event_loop);
        }
        event_loop.exit();
        true
    }

    pub(crate) fn report_throughput(&self) {
        let Some(stats) = &self.throughput else {
            return;
        };
        let elapsed = stats.started.elapsed();
        let mut intervals = stats.frame_intervals.clone();
        intervals.sort_unstable();
        let percentile = |percent: usize| {
            intervals
                .get(intervals.len().saturating_sub(1) * percent / 100)
                .map(|duration| duration.as_secs_f64() * 1000.0)
        };
        eprintln!(
            "throughput mode=visible workload={} complete={} bytes={} elapsed_ms={:.3} mb_s={:.3} batches={} bytes_per_batch={:.1} parse_ms={:.3} redraws={} terminal_frames={} scrollback={} columns={} rows={} max_drain_ms={:.3} frame_p50_ms={:?} frame_p95_ms={:?} frame_max_ms={:?} probe_dispatch_ms={:?} probe_ack_ms={:?}",
            stats.workload.as_deref().unwrap_or("interactive"),
            !stats.timed_out
                && stats.eof
                && stats.exit_code == Some(0)
                && stats.bytes > 0
                && (stats.workload.as_deref() != Some("input") || stats.probe_ack.is_some()),
            stats.bytes,
            elapsed.as_secs_f64() * 1000.0,
            stats.bytes as f64 / elapsed.as_secs_f64() / 1e6,
            stats.batches,
            stats.bytes as f64 / stats.batches.max(1) as f64,
            stats.parse.as_secs_f64() * 1000.0,
            stats.redraws,
            stats.terminal_frames,
            self.terminal.scrollback_len(),
            self.terminal.dimensions().columns(),
            self.terminal.dimensions().rows(),
            stats.max_drain.as_secs_f64() * 1000.0,
            percentile(50),
            percentile(95),
            percentile(100),
            stats.probe_dispatch.map(|v| v.as_secs_f64() * 1000.0),
            stats.probe_ack.map(|v| v.as_secs_f64() * 1000.0)
        );
        for worker in self.pty.iter().chain(
            self.inactive_panes
                .values()
                .filter_map(|runtime| runtime.pty.as_ref()),
        ) {
            if let Some(pty) = worker.throughput_stats() {
                eprintln!("throughput pty={pty:?}");
            }
        }
        if let Some(renderer) = self
            .renderer
            .as_ref()
            .and_then(|renderer| renderer.throughput_stats())
        {
            eprintln!("throughput renderer={renderer:?}");
        }
    }
}

#[derive(Debug)]
pub struct Stats {
    pub started: Instant,
    pub bytes: u64,
    pub batches: u64,
    pub parse: Duration,
    pub redraws: u64,
    pub terminal_frames: u64,
    pub data_dirty: bool,
    pub workload: Option<String>,
    pub eof: bool,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub probe_requested: Option<Instant>,
    pub probe_dispatched: Option<Instant>,
    pub probe_ack: Option<Duration>,
    pub probe_dispatch: Option<Duration>,
    pub frame_intervals: Vec<Duration>,
    pub last_present: Option<Instant>,
    pub max_drain: Duration,
    tail: Vec<u8>,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            bytes: 0,
            batches: 0,
            parse: Duration::ZERO,
            redraws: 0,
            terminal_frames: 0,
            data_dirty: false,
            workload: None,
            eof: false,
            exit_code: None,
            timed_out: false,
            probe_requested: None,
            probe_dispatched: None,
            probe_ack: None,
            probe_dispatch: None,
            frame_intervals: Vec::new(),
            last_present: None,
            max_drain: Duration::ZERO,
            tail: Vec::new(),
        }
    }
}

impl Stats {
    pub fn output(&mut self, bytes: &[u8], parse: Duration) {
        self.bytes += bytes.len() as u64;
        self.batches += 1;
        self.parse += parse;
        self.data_dirty = true;
        if self.workload.as_deref() == Some("input") && self.probe_ack.is_none() {
            self.tail.extend_from_slice(bytes);
            if self.tail.windows(ACK.len()).any(|window| window == ACK) {
                self.probe_ack = self.probe_dispatched.map(|start| start.elapsed());
            }
            let retain = ACK.len() - 1;
            if self.tail.len() > retain {
                self.tail.drain(..self.tail.len() - retain);
            }
        }
    }

    pub fn presented(&mut self) {
        self.terminal_frames += u64::from(self.data_dirty);
        self.data_dirty = false;
        let now = Instant::now();
        // Keep normal opt-in diagnostics bounded, even for a long-lived shell.
        if self.workload.is_some()
            && self.frame_intervals.len() < 100_000
            && let Some(previous) = self.last_present
        {
            self.frame_intervals.push(now.duration_since(previous));
        }
        self.last_present = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acknowledgement_survives_every_chunk_boundary() {
        for split in 0..=ACK.len() {
            let mut stats = Stats {
                workload: Some("input".into()),
                probe_dispatched: Some(Instant::now()),
                ..Stats::default()
            };
            stats.output(&ACK[..split], Duration::ZERO);
            stats.output(&ACK[split..], Duration::ZERO);
            assert!(stats.probe_ack.is_some());
        }
    }

    #[test]
    fn multiple_chunks_count_as_one_terminal_frame() {
        let mut stats = Stats::default();
        stats.output(b"a", Duration::ZERO);
        stats.output(b"b", Duration::ZERO);
        stats.presented();
        stats.presented();
        assert_eq!(
            (stats.bytes, stats.batches, stats.terminal_frames),
            (2, 2, 1)
        );
    }
}
