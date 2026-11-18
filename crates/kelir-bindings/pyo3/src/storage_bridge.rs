// see: cans/03-sdk-architecture.md#Python-SDK
// see: cans/05-storage-plane.md#Storage-architecture
use std::path::PathBuf;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};

use kelir_engine::planes::storage::{
    IdmapConfig, LockBrokerWiring, StorageConfig, WorkspaceEncapsulation,
};

// see: cans/05-storage-plane.md#Storage-architecture
#[derive(Default)]
pub struct PyStorageProjection {
    pub config: StorageConfig,
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
pub fn storage_config(cfg: &Bound<'_, PyDict>) -> PyResult<PyStorageProjection> {
    let mut config = StorageConfig::default();
    let mut shared_caches: Option<Vec<String>> = None;
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
                shared_caches = Some(value.extract::<Vec<String>>().map_err(|_| {
                    PyValueError::new_err("storage shared_caches is not a list of str")
                })?);
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
        .as_ref()
        .map(|caches| caches.iter().map(PathBuf::from).collect())
        .unwrap_or_default();
    if lock_broker_flag == Some(true) {
        config.lock_broker = Some(LockBrokerWiring {
            scope_globs: shared_caches.unwrap_or_default(),
            non_blocking: false,
        });
    }
    Ok(PyStorageProjection { config })
}
