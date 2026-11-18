// see: cans/01-architecture.md#Host-state-root
// see: cans/06-storage-catalog.md#Entity-model
use kelir_interface::plugin_model::PluginError;

// see: cans/01-architecture.md#Host-state-root
// L3-GAP: [lifecycle_checkpoint]: the runtime child janitor sweeps orphaned session dirs on startup; slop: orphaned session dirs never swept, disk leaks
pub fn sweep_orphaned_session_dirs() -> Result<(), PluginError> {
    Err(PluginError::Halt(
        "sweep_orphaned_session_dirs: no native implementation (L3)".into(),
    ))
}

// see: cans/06-storage-catalog.md#Entity-model
// L3-GAP: [lc_16_session_incarnation_cas_guard]: swap the minted boot_id and now-based start_time_ns for the OS host_boot_id and proc start_time per cans/08-lifecycle.md#Cross-platform-incarnation-proof; slop: destructive verb skips incarnation CAS, stale row deleted
pub fn register_incarnation_proof_row(
    session_id: [u8; 16],
    incarnation_id: [u8; 16],
    tenant_id: &str,
) -> Result<(), PluginError> {
    let _ = (session_id, incarnation_id, tenant_id);
    Err(PluginError::Halt(
        "register_incarnation_proof_row: no native implementation (L3)".into(),
    ))
}

// see: cans/06-storage-catalog.md#Entity-model
// L3-GAP: [lc_14_detached_session_lease_heartbeat]: detached boots upgrade the row to lease_kind='lease' with runner child pid and expires_at per cans/08-lifecycle.md#Detached-lifecycle; slop: detached row stays Ephemeral, expires_at never advances
pub fn upgrade_detached_lease_row(
    session_id: [u8; 16],
    tenant_id: &str,
    runner_pid: i64,
    expires_at_unix: i64,
) -> Result<(), PluginError> {
    let _ = (session_id, tenant_id, runner_pid, expires_at_unix);
    Err(PluginError::Halt(
        "upgrade_detached_lease_row: no native implementation (L3)".into(),
    ))
}

// see: cans/01-architecture.md#Host-state-root
pub use crate::session_paths_flow::{reap_ephemeral_session_dir, session_dir};
