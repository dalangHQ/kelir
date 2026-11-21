// see: cans/08-lifecycle.md#Host-crash-reaper
// see: cans/14-observability.md#Metrics-pipeline
use std::sync::Arc;

use kelir_interface::host::HostReaperKind;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::telemetry::MetricRegistry;

// TODO: [hypervisor_driver]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it. (deferred to L3)
#[allow(dead_code)]
pub(crate) fn lsm_status(reaper_kind: &HostReaperKind) -> bool {
    // TODO: [hypervisor_driver]: query the host LSM (Linux LSM / macOS sandbox / Windows integrity) status; slop: LSM status never queried, posture unproven (deferred to L3)
    let _ = reaper_kind;
    false
}

// TODO: [hypervisor_driver]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it. (deferred to L3)
#[allow(dead_code)]
pub(crate) fn refresh_rss(metrics: &Arc<MetricRegistry>) -> Option<u64> {
    crate::reaper_run::refresh_rss(metrics)
}

// TODO: [hypervisor_driver]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it. (deferred to L3)
#[allow(dead_code)]
pub(crate) fn arm_host_reaper(parent_pid: u32) -> Result<(), PluginError> {
    // TODO: [lc_12_host_reaper_pdeathsig_enforcement]: arm the platform reaper (pdeathsig / kqueue NOTE_EXIT / Job Object) on parent_pid; slop: no pdeathsig arm, orphan hypervisor survives parent SIGKILL (deferred to L3)
    let _ = parent_pid;
    Err(PluginError::Halt(
        "deferred to L3 [lc_12_host_reaper_pdeathsig_enforcement]: HostPlane::arm_host_reaper; slop: no pdeathsig arm, orphan hypervisor survives parent SIGKILL".into(),
    ))
}
