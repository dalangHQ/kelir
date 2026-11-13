// see: cans/08-lifecycle.md#Host-crash-reaper
// see: cans/14-observability.md#Metrics-pipeline
use std::sync::Arc;

use kelir_interface::host::HostReaperKind;
use kelir_interface::plugin_model::PluginError;
use kelir_otlp::registry_otlp::MetricRegistry;

pub(crate) fn lsm_status(reaper_kind: &HostReaperKind) -> bool {
    // TODO (L3) [hypervisor_driver]: query the host LSM (Linux LSM / macOS sandbox / Windows integrity) status; slop: LSM status never queried, posture unproven
    let _ = reaper_kind;
    false
}

pub(crate) fn refresh_rss(metrics: &Arc<MetricRegistry>) -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        for line in status.lines() {
            let Some(rest) = line.strip_prefix("VmRSS:") else {
                continue;
            };
            let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
            let bytes = kb.saturating_mul(1024);
            metrics.set_rss_bytes(bytes);
            return Some(bytes);
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    {
        // TODO (L3) [lc_02_suspend_zero_rss_purge]: report host RSS on non-Linux (task_info on macOS, GetProcessMemoryInfo on Windows); slop: RSS unreported on macOS/Windows, suspend purge unproven
        let _ = metrics;
        None
    }
}

pub(crate) fn arm_host_reaper(parent_pid: u32) -> Result<(), PluginError> {
    // TODO (L3) [lc_12_host_reaper_pdeathsig_enforcement]: arm the platform reaper (pdeathsig / kqueue NOTE_EXIT / Job Object) on parent_pid; slop: no pdeathsig arm, orphan hypervisor survives parent SIGKILL
    let _ = parent_pid;
    Err(PluginError::Halt(
        "TODO (L3) [lc_12_host_reaper_pdeathsig_enforcement]: HostPlane::arm_host_reaper; slop: no pdeathsig arm, orphan hypervisor survives parent SIGKILL".into(),
    ))
}
