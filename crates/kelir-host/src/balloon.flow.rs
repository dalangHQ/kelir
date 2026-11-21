// see: cans/01-architecture.md#Host-memory-pressure-watcher
// see: cans/08-lifecycle.md#Hypervisor-primitives
use std::sync::{Arc, Mutex};

use kelir_interface::driver_interface::BalloonConvergence;
use kelir_interface::handles_lifecycle::VmControl;
use kelir_interface::host::BalloonRequest;
use kelir_interface::plugin_model::PluginError;

// TODO: [hypervisor_driver]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it. (deferred to L3)
#[allow(dead_code)]
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

