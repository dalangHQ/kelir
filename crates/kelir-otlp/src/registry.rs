// see: cans/14-observability.md#Metrics-pipeline
use std::sync::atomic::{AtomicU32, AtomicU64};

pub struct MetricRegistry {
    pub exec_count: AtomicU64,
    pub syscall_count: AtomicU64,
    pub dropped_frames: AtomicU64,
    pub dirty_pages: AtomicU64,
    pub active_vcpus: AtomicU32,
}

impl MetricRegistry {
    pub fn new() -> Self {
        Self {
            exec_count: AtomicU64::new(0),
            syscall_count: AtomicU64::new(0),
            dropped_frames: AtomicU64::new(0),
            dirty_pages: AtomicU64::new(0),
            active_vcpus: AtomicU32::new(0),
        }
    }

    pub fn record_exec(&self) {
        self.exec_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn record_drop(&self) {
        self.dropped_frames
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Default for MetricRegistry {
    fn default() -> Self {
        Self::new()
    }
}
