// see: cans/01-architecture.md#Host-memory-pressure-watcher
use std::sync::Mutex;

use kelir_interface::host::{PressureEvent, PressureSubscription};
use kelir_interface::plugin_model::PluginError;

pub(crate) fn subscribe_pressure(
    watcher: &Mutex<Option<PressureSubscription>>,
) -> Result<PressureSubscription, PluginError> {
    // TODO (L3) [hd_16_virtio_balloon_pressure_reclaim]: subscribe to the kernel pressure source (cgroup v2 memory.pressure via epoll/io_uring, DISPATCH_SOURCE_TYPE_MEMORYPRESSURE on macOS) and store the subscription in watcher; slop: pressure ignored, no madvise reclaim, balloon unadjusted
    let _ = watcher;
    Err(PluginError::Halt(
        "TODO (L3) [hd_16_virtio_balloon_pressure_reclaim]: HostPlane::subscribe_pressure; slop: pressure ignored, no madvise reclaim, balloon unadjusted".into(),
    ))
}

pub(crate) fn poll_pressure_events(
    watcher: &Mutex<Option<PressureSubscription>>,
) -> Result<Vec<PressureEvent>, PluginError> {
    // TODO (L3) [hd_16_virtio_balloon_pressure_reclaim]: drain kernel pressure events into PressureEvents and update pressure_level; slop: pressure ignored, no madvise reclaim, balloon unadjusted
    let _ = watcher;
    Err(PluginError::Halt(
        "TODO (L3) [hd_16_virtio_balloon_pressure_reclaim]: HostPlane::poll_pressure_events; slop: pressure ignored, no madvise reclaim, balloon unadjusted".into(),
    ))
}
