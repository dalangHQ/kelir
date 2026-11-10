// see: cans/14-observability.md#Metrics-pipeline
use std::sync::atomic::{AtomicU32, AtomicU64};

// see: cans/14-observability.md#Metrics-pipeline
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricSnapshot {
    pub exec_count: u64,
    pub syscall_count: u64,
    pub dropped_frames: u64,
    pub backpressure_events: u64,
    pub dirty_pages: u64,
    pub rss_bytes: u64,
    pub active_vcpus: u32,
    pub in_flight_execs: u64,
}

pub struct MetricRegistry {
    pub exec_count: AtomicU64,
    pub syscall_count: AtomicU64,
    pub dropped_frames: AtomicU64,
    pub backpressure_events: AtomicU64,
    pub dirty_pages: AtomicU64,
    pub rss_bytes: AtomicU64,
    pub active_vcpus: AtomicU32,
    pub in_flight_execs: AtomicU64,
}

impl MetricRegistry {
    pub fn new() -> Self {
        Self {
            exec_count: AtomicU64::new(0),
            syscall_count: AtomicU64::new(0),
            dropped_frames: AtomicU64::new(0),
            backpressure_events: AtomicU64::new(0),
            dirty_pages: AtomicU64::new(0),
            rss_bytes: AtomicU64::new(0),
            active_vcpus: AtomicU32::new(0),
            in_flight_execs: AtomicU64::new(0),
        }
    }

    pub fn record_syscall(&self) {
        self.syscall_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn record_exec(&self) {
        self.exec_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn record_drop(&self) {
        self.dropped_frames
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn record_backpressure(&self) {
        self.backpressure_events
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn add_dirty_pages(&self, pages: u64) {
        self.dirty_pages
            .fetch_add(pages, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn set_active_vcpus(&self, vcpus: u32) {
        self.active_vcpus
            .store(vcpus, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn set_rss_bytes(&self, bytes: u64) {
        self.rss_bytes
            .store(bytes, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn exec_started(&self) {
        self.in_flight_execs
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn exec_finished(&self) {
        let _ = self.in_flight_execs.try_update(
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
            |in_flight| Some(in_flight.saturating_sub(1)),
        );
    }

    pub fn snapshot(&self) -> MetricSnapshot {
        let load = std::sync::atomic::Ordering::Relaxed;
        MetricSnapshot {
            exec_count: self.exec_count.load(load),
            syscall_count: self.syscall_count.load(load),
            dropped_frames: self.dropped_frames.load(load),
            backpressure_events: self.backpressure_events.load(load),
            dirty_pages: self.dirty_pages.load(load),
            rss_bytes: self.rss_bytes.load(load),
            active_vcpus: self.active_vcpus.load(load),
            in_flight_execs: self.in_flight_execs.load(load),
        }
    }
}

impl Default for MetricRegistry {
    fn default() -> Self {
        Self::new()
    }
}
