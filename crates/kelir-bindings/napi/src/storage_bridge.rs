// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/05-storage-plane.md#Storage-architecture
use napi::{Error, Result};

use kelir_engine::planes::storage::{
    IdmapConfig, LockBrokerWiring, MaskReadConfig, MaskReadRule, ReadMaskEngine, StorageConfig,
    WorkspaceEncapsulation,
};
use kelir_interface::storage::SyntheticFile;

use crate::{NativeMaskRule, NativeStorageConfig, NativeSyntheticFile};

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

// see: cans/03-sdk-architecture.md#Prerequisite-gating-across-languages
fn usize_of(value: f64, what: &str) -> Result<usize> {
    if !(value.is_finite() && value >= 0.0 && value <= usize::MAX as f64) {
        return Err(econfig(format!("storage {what} is not a usize")));
    }
    Ok(value as usize)
}

// see: cans/05-storage-plane.md#Storage-architecture
// TODO (L3) [sp_15_host_inode_lock_broker_flock_bridge]: NAPI lock-broker scope globs ride a dedicated native lock-broker config surface.; slop: globs accepted without flock bridge, lock unenforced
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
    let mask_read = match (&cfg.mask_read, &cfg.scan_engine, &cfg.carryover_bytes) {
        (None, None, None) => None,
        _ => Some(MaskReadConfig {
            engine: match cfg.scan_engine.as_deref() {
                None => ReadMaskEngine::FastRegex,
                Some("FastRegex") => ReadMaskEngine::FastRegex,
                Some(other) => {
                    return Err(econfig(format!(
                        "storage scan_engine `{other}` is not FastRegex"
                    )));
                }
            },
            carryover_bytes: usize_of(cfg.carryover_bytes.unwrap_or(0.0), "carryover_bytes")?,
            rules: cfg
                .mask_read
                .as_deref()
                .map(mask_rules)
                .transpose()?
                .unwrap_or_default(),
        }),
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
        mask_read,
        encapsulation,
    })
}

// see: cans/05-storage-plane.md#In-flight-read-masking
fn mask_rules(rules: &[NativeMaskRule]) -> Result<Vec<MaskReadRule>> {
    rules
        .iter()
        .map(|rule| {
            Ok(MaskReadRule {
                glob: rule.glob.clone(),
                patterns: vec![(rule.pattern.clone(), rule.replace.clone())],
            })
        })
        .collect()
}

// see: cans/05-storage-plane.md#Synthetic-FUSE-inodes
pub fn synthetic_files(cfg: &NativeStorageConfig) -> Result<Vec<SyntheticFile>> {
    cfg.synthetic_files
        .as_ref()
        .map(|files| {
            files
                .iter()
                .map(|file: &NativeSyntheticFile| {
                    let mode = match file.mode {
                        None => 0o400,
                        Some(mode) => u32_of(mode, "synthetic file mode")?,
                    };
                    Ok(SyntheticFile::static_with_mode(
                        &file.path,
                        &file.content,
                        mode,
                    ))
                })
                .collect()
        })
        .transpose()
        .map(|files| files.unwrap_or_default())
}
