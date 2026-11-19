// see: cans/03-sdk-architecture.md#Python-SDK
// see: cans/05-storage-plane.md#Storage-architecture
use std::path::PathBuf;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};

use kelir::planes::storage_plane::SharedCacheConfig;
use kelir_engine::planes::storage::{
    IdmapConfig, LockBrokerWiring, StorageConfig, WorkspaceEncapsulation,
};

// see: cans/05-storage-plane.md#Storage-architecture
#[derive(Default)]
pub struct PyStorageProjection {
    pub config: StorageConfig,
    pub shared_caches: Vec<SharedCacheConfig>,
}

// see: cans/03-sdk-architecture.md#Prerequisite-gating-across-languages
fn econfig(message: impl Into<String>) -> PyErr {
    crate::envelope_error(&kelir_interface::error_envelope::ErrorEnvelope::econfig(
        message,
    ))
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn py_field<'py>(obj: &Bound<'py, PyAny>, name: &str) -> Option<Bound<'py, PyAny>> {
    obj.getattr(name).ok().or_else(|| obj.get_item(name).ok())
}

// see: cans/03-sdk-architecture.md#Python-SDK
fn field_u32(obj: &Bound<'_, PyAny>, name: &str, what: &str) -> PyResult<u32> {
    py_field(obj, name)
        .ok_or_else(|| PyValueError::new_err(format!("storage {what} field `{name}` is missing")))?
        .extract::<u32>()
        .map_err(|_| PyValueError::new_err(format!("storage {what} field `{name}` is not a u32")))
}

// see: cans/05-storage-plane.md#Storage-architecture
fn shared_cache_of(entry: &Bound<'_, PyDict>) -> PyResult<SharedCacheConfig> {
    let host_path = entry
        .get_item("host_path")?
        .ok_or_else(|| PyValueError::new_err("storage shared_cache has no host_path"))?
        .extract::<String>()
        .map_err(|_| PyValueError::new_err("storage shared_cache host_path is not a str"))?;
    let guest_mount = entry
        .get_item("guest_mount")?
        .ok_or_else(|| PyValueError::new_err("storage shared_cache has no guest_mount"))?
        .extract::<String>()
        .map_err(|_| PyValueError::new_err("storage shared_cache guest_mount is not a str"))?;
    if host_path.is_empty() {
        return Err(econfig("storage shared_cache host_path is empty"));
    }
    if guest_mount.is_empty() {
        return Err(econfig("storage shared_cache guest_mount is empty"));
    }
    let read_only = match entry.get_item("read_only")? {
        Some(flag) => flag
            .extract::<bool>()
            .map_err(|_| PyValueError::new_err("storage shared_cache read_only is not a bool"))?,
        None => false,
    };
    Ok(SharedCacheConfig {
        host_path: PathBuf::from(host_path),
        guest_mount: PathBuf::from(guest_mount),
        read_only,
    })
}

// see: cans/05-storage-plane.md#Storage-architecture
pub fn storage_config(cfg: &Bound<'_, PyDict>) -> PyResult<PyStorageProjection> {
    let mut config = StorageConfig::default();
    let mut shared_caches: Vec<SharedCacheConfig> = Vec::new();
    let mut lock_broker_flag: Option<bool> = None;
    for (key, value) in cfg.iter() {
        match key.to_string().as_str() {
            "lock_broker" => {
                lock_broker_flag = Some(
                    value
                        .extract::<bool>()
                        .map_err(|_| PyValueError::new_err("storage lock_broker is not a bool"))?,
                );
            }
            "shared_caches" => {
                let entries = value.downcast::<PyList>().map_err(|_| {
                    PyValueError::new_err("storage shared_caches is not a list of dicts")
                })?;
                for entry in entries.iter() {
                    let entry = entry.downcast::<PyDict>().map_err(|_| {
                        PyValueError::new_err("storage shared_caches entries must be dicts")
                    })?;
                    shared_caches.push(shared_cache_of(entry)?);
                }
            }
            "idmap" => {
                config.idmap = Some(IdmapConfig {
                    guest_uid: field_u32(&value, "guest_uid", "idmap")?,
                    host_uid: field_u32(&value, "host_uid", "idmap")?,
                });
            }
            "encapsulation" => {
                let raw = value
                    .extract::<String>()
                    .map_err(|_| PyValueError::new_err("storage encapsulation is not a str"))?;
                config.encapsulation = match raw.as_str() {
                    "Locked" => WorkspaceEncapsulation::Locked,
                    "Passthrough" => WorkspaceEncapsulation::Passthrough,
                    other => {
                        return Err(econfig(format!(
                            "storage encapsulation `{other}` is not Locked|Passthrough"
                        )));
                    }
                };
            }
            other => {
                return Err(PyValueError::new_err(format!(
                    "set_storage_config: unknown or non-bridged key `{other}`"
                )));
            }
        }
    }
    config.shared_caches = shared_caches
        .iter()
        .map(|cache| cache.host_path.clone())
        .collect();
    if lock_broker_flag == Some(true) {
        config.lock_broker = Some(LockBrokerWiring {
            scope_globs: shared_caches
                .iter()
                .map(|cache| format!("{}/**", cache.guest_mount.display()))
                .collect(),
            non_blocking: false,
        });
    }
    Ok(PyStorageProjection {
        config,
        shared_caches,
    })
}
