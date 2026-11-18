// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/05-storage-plane.md#Storage-architecture
use napi::{Error, Result};

use kelir_engine::planes::storage::{
    IdmapConfig, LockBrokerWiring, StorageConfig, WorkspaceEncapsulation,
};

use crate::NativeStorageConfig;

// see: cans/03-sdk-architecture.md#Prerequisite-gating-across-languages
fn econfig(message: impl Into<String>) -> Error {
    crate::envelope_error(&kelir_interface::error_envelope::ErrorEnvelope::econfig(
        message,
    ))
}

// see: cans/03-sdk-architecture.md#Prerequisite-gating-across-languages
fn u32_of(value: f64, what: &str) -> Result<u32> {
    if !(value.is_finite() && value >= 0.0 && value <= u32::MAX as f64) {
        return Err(econfig(format!("storage {what} is not a u32")));
    }
    Ok(value as u32)
}

// see: cans/05-storage-plane.md#Storage-architecture
// L3-GAP: [sp_15_host_inode_lock_broker_flock_bridge]: NAPI lock-broker scope globs ride a dedicated native lock-broker config surface.; slop: globs accepted without flock bridge, lock unenforced
pub fn storage_config(cfg: &NativeStorageConfig) -> Result<StorageConfig> {
    let encapsulation = match cfg.encapsulation.as_deref() {
        None => WorkspaceEncapsulation::default(),
        Some("Locked") => WorkspaceEncapsulation::Locked,
        Some("Passthrough") => WorkspaceEncapsulation::Passthrough,
        Some(other) => {
            return Err(econfig(format!(
                "storage encapsulation `{other}` is not Locked|Passthrough"
            )));
        }
    };
    let idmap = match (cfg.idmap_guest_uid, cfg.idmap_host_uid) {
        (None, None) => None,
        (Some(guest), Some(host)) => Some(IdmapConfig {
            guest_uid: u32_of(guest, "idmap guest_uid")?,
            host_uid: u32_of(host, "idmap host_uid")?,
        }),
        _ => return Err(econfig("storage idmap needs both guest_uid and host_uid")),
    };
    Ok(StorageConfig {
        shared_caches: cfg
            .shared_caches
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(std::path::PathBuf::from)
            .collect(),
        lock_broker: cfg
            .lock_broker
            .filter(|enabled| *enabled)
            .map(|_| LockBrokerWiring {
                scope_globs: cfg.shared_caches.clone().unwrap_or_default(),
                non_blocking: false,
            }),
        idmap,
        encapsulation,
    })
}
