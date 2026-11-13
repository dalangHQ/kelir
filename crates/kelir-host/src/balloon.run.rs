// see: cans/01-architecture.md#Host-memory-pressure-watcher
// see: cans/08-lifecycle.md#Hypervisor-primitives
use std::sync::{Arc, Mutex};

use kelir_interface::driver_interface::BalloonConvergence;
use kelir_interface::handles_lifecycle::VmControl;
use kelir_interface::host::{BalloonRequest, ReclaimRequest};
use kelir_interface::plugin_model::PluginError;

pub(crate) async fn inflate_balloon(
    vm_control: &Mutex<Option<Arc<dyn VmControl>>>,
    request: BalloonRequest,
) -> Result<BalloonConvergence, PluginError> {
    let control = vm_control
        .lock()
        .map_err(|_| PluginError::Halt("vm control lock poisoned".into()))?
        .clone();
    let Some(control) = control else {
        return Err(PluginError::Halt(
            "vm control not wired; inflate_balloon needs Session::boot to bind the driver control"
                .into(),
        ));
    };
    control.adjust_balloon(request.target_mb).await
}

pub(crate) fn reclaim(request: ReclaimRequest) -> Result<u64, PluginError> {
    // TODO (L3) [hd_16_virtio_balloon_pressure_reclaim]: reclaim request.bytes of host frames via madvise(MADV_DONTNEED) on Linux, hv_vm_unmap + mach_vm_deallocate on macOS; slop: pressure ignored, no madvise reclaim, balloon unadjusted
    let _ = request;
    Err(PluginError::Halt(
        "TODO (L3) [hd_16_virtio_balloon_pressure_reclaim]: HostPlane::reclaim; slop: pressure ignored, no madvise reclaim, balloon unadjusted".into(),
    ))
}
