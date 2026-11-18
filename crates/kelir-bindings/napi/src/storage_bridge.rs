// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/05-storage-plane.md#Storage-architecture
use std::path::PathBuf;

use napi::{Error, Result};

use kelir::planes::storage_plane::SharedCacheConfig;
use kelir_engine::planes::storage::{
    IdmapConfig, LockBrokerWiring, StorageConfig, WorkspaceEncapsulation,
};

use crate::builder::{NativeSharedCache, NativeStorageConfig};

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
fn shared_cache_of(entry: &NativeSharedCache) -> Result<SharedCacheConfig> {
    if entry.host_path.is_empty() {
        return Err(econfig("storage shared_cache host_path is empty"));
    }
    if entry.guest_mount.is_empty() {
        return Err(econfig("storage shared_cache guest_mount is empty"));
    }
    Ok(SharedCacheConfig {
        host_path: PathBuf::from(&entry.host_path),
        guest_mount: PathBuf::from(&entry.guest_mount),
        read_only: entry.read_only.unwrap_or(false),
    })
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
    let shared_caches: Vec<SharedCacheConfig> = match &cfg.shared_caches {
        Some(entries) => entries
            .iter()
            .map(shared_cache_of)
            .collect::<Result<Vec<SharedCacheConfig>>>()?,
        None => Vec::new(),
    };
    Ok(StorageConfig {
        shared_caches: shared_caches
            .iter()
            .map(|cache| cache.host_path.clone())
            .collect(),
        lock_broker: cfg
            .lock_broker
            .filter(|enabled| *enabled)
            .map(|_| LockBrokerWiring {
                scope_globs: shared_caches
                    .iter()
                    .map(|cache| format!("{}/**", cache.guest_mount.display()))
                    .collect(),
                non_blocking: false,
            }),
        idmap,
        encapsulation,
    })
}
