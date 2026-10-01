//! Optional atomics only on the instrumented worker path.
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

#[derive(Debug, Default)]
pub(crate) struct Metrics {
    bytes: AtomicU64,
    reads: AtomicU64,
    chunks: AtomicU64,
    pending_bytes: AtomicU64,
    peak_chunks: AtomicU64,
    peak_bytes: AtomicU64,
}

/// Pending output includes queued chunks, the reader's in-flight send and a
/// received chunk until its accounting decrement. The eight-slot channel can
/// therefore report a transient peak of ten. Live fields are independent atomic
/// snapshots, not a transactional channel length; no lock is added to transport.
#[derive(Clone, Copy, Debug, Default)]
pub struct PtyThroughputStats {
    pub bytes: u64,
    pub reads: u64,
    pub pending_chunks: u64,
    pub pending_bytes: u64,
    pub peak_chunks: u64,
    pub peak_bytes: u64,
}

impl Metrics {
    pub(crate) fn clear_pending(&self) {
        self.chunks.store(0, Relaxed);
        self.pending_bytes.store(0, Relaxed);
    }
    pub(crate) fn read(&self, bytes: usize) {
        self.bytes.fetch_add(bytes as u64, Relaxed);
        self.reads.fetch_add(1, Relaxed);
        let chunks = self.chunks.fetch_add(1, Relaxed) + 1;
        let pending = self.pending_bytes.fetch_add(bytes as u64, Relaxed) + bytes as u64;
        self.peak_chunks.fetch_max(chunks, Relaxed);
        self.peak_bytes.fetch_max(pending, Relaxed);
    }

    pub(crate) fn received(&self, bytes: usize) {
        self.chunks.fetch_sub(1, Relaxed);
        self.pending_bytes.fetch_sub(bytes as u64, Relaxed);
    }

    pub(crate) fn snapshot(&self) -> PtyThroughputStats {
        PtyThroughputStats {
            bytes: self.bytes.load(Relaxed),
            reads: self.reads.load(Relaxed),
            pending_chunks: self.chunks.load(Relaxed),
            pending_bytes: self.pending_bytes.load(Relaxed),
            peak_chunks: self.peak_chunks.load(Relaxed),
            peak_bytes: self.peak_bytes.load(Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn received_and_cancelled_sends_balance_pending_output() {
        let metrics = Metrics::default();
        metrics.read(1024);
        metrics.read(7);
        metrics.received(1024);
        metrics.received(7); // Failed send cancels its reservation.
        let stats = metrics.snapshot();
        assert_eq!((stats.bytes, stats.reads), (1031, 2));
        assert_eq!((stats.pending_chunks, stats.pending_bytes), (0, 0));
        assert_eq!((stats.peak_chunks, stats.peak_bytes), (2, 1031));
    }

    #[test]
    fn shutdown_discards_pending_but_preserves_lifetime_totals() {
        let metrics = Metrics::default();
        metrics.read(42);
        metrics.clear_pending();
        let stats = metrics.snapshot();
        assert_eq!((stats.pending_chunks, stats.pending_bytes), (0, 0));
        assert_eq!((stats.bytes, stats.peak_bytes), (42, 42));
    }
}
