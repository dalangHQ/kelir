// see: cans/01-architecture.md#Host-memory-pressure-watcher
use std::sync::Mutex;

use kelir_interface::host::{PressureEvent, PressureSubscription};
use kelir_interface::plugin_model::PluginError;

// TODO: [hypervisor_driver]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it. (deferred to L3)
#[allow(dead_code)]
pub(crate) fn subscribe_pressure(
    watcher: &Mutex<Option<PressureSubscription>>,
) -> Result<PressureSubscription, PluginError> {
    // TODO: [hd_16_virtio_balloon_pressure_reclaim]: subscribe to the kernel pressure source (cgroup v2 memory.pressure via epoll/io_uring, DISPATCH_SOURCE_TYPE_MEMORYPRESSURE on macOS) and store the subscription in watcher; slop: pressure ignored, no madvise reclaim, balloon unadjusted (deferred to L3)
    let _ = watcher;
    Err(PluginError::Halt(
        "deferred to L3 [hd_16_virtio_balloon_pressure_reclaim]: HostPlane::subscribe_pressure; slop: pressure ignored, no madvise reclaim, balloon unadjusted".into(),
    ))
}

// TODO: [hypervisor_driver]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it. (deferred to L3)
#[allow(dead_code)]
pub(crate) fn poll_pressure_events(
    watcher: &Mutex<Option<PressureSubscription>>,
) -> Result<Vec<PressureEvent>, PluginError> {
    // TODO: [hd_16_virtio_balloon_pressure_reclaim]: drain kernel pressure events into PressureEvents and update pressure_level; slop: pressure ignored, no madvise reclaim, balloon unadjusted (deferred to L3)
    let _ = watcher;
    Err(PluginError::Halt(
        "deferred to L3 [hd_16_virtio_balloon_pressure_reclaim]: HostPlane::poll_pressure_events; slop: pressure ignored, no madvise reclaim, balloon unadjusted".into(),
    ))
}
