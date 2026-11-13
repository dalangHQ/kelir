// see: cans/03-sdk-architecture.md#Python-SDK
// see: cans/05-storage-plane.md#Storage-architecture
use std::path::PathBuf;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};

use kelir_engine::planes::storage::{
    IdmapConfig, LockBrokerWiring, MaskReadConfig, MaskReadRule, ReadMaskEngine, StorageConfig,
    WorkspaceEncapsulation,
};
use kelir_interface::storage::SyntheticFile;

// see: cans/05-storage-plane.md#Storage-architecture
#[derive(Default)]
pub struct PyStorageProjection {
    pub config: StorageConfig,
    pub synthetic_files: Vec<SyntheticFile>,
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
fn field_str(obj: &Bound<'_, PyAny>, name: &str, what: &str) -> PyResult<String> {
    py_field(obj, name)
        .ok_or_else(|| PyValueError::new_err(format!("storage {what} field `{name}` is missing")))?
        .extract::<String>()
        .map_err(|_| PyValueError::new_err(format!("storage {what} field `{name}` is not a str")))
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
    let mut synthetic_files = Vec::new();
    let mut shared_caches: Option<Vec<String>> = None;
    let mut lock_broker_flag: Option<bool> = None;
    let mut mask_engine: Option<ReadMaskEngine> = None;
    let mut carryover_bytes: Option<usize> = None;
    let mut mask_rules: Vec<MaskReadRule> = Vec::new();
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
            "mask_read" => {
                let rules = value
                    .downcast::<PyList>()
                    .map_err(|_| PyValueError::new_err("storage mask_read is not a list"))?;
                for rule in rules.iter() {
                    let glob = field_str(&rule, "glob", "mask_read")?;
                    let patterns = py_field(&rule, "patterns")
                        .ok_or_else(|| {
                            PyValueError::new_err("storage mask_read field `patterns` is missing")
                        })?
                        .extract::<Vec<(String, String)>>()
                        .map_err(|_| {
                            PyValueError::new_err(
                                "storage mask_read patterns is not a list of (str, str)",
                            )
                        })?;
                    mask_rules.push(MaskReadRule { glob, patterns });
                }
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
            "scan_engine" => {
                let raw = value
                    .extract::<String>()
                    .map_err(|_| PyValueError::new_err("storage scan_engine is not a str"))?;
                mask_engine = Some(match raw.as_str() {
                    "FastRegex" => ReadMaskEngine::FastRegex,
                    other => {
                        return Err(econfig(format!(
                            "storage scan_engine `{other}` is not FastRegex"
                        )));
                    }
                });
            }
            "carryover_bytes" => {
                carryover_bytes =
                    Some(value.extract::<usize>().map_err(|_| {
                        PyValueError::new_err("storage carryover_bytes is not an int")
                    })?);
            }
            "synthetic_files" => {
                let files = value
                    .downcast::<PyList>()
                    .map_err(|_| PyValueError::new_err("storage synthetic_files is not a list"))?;
                for file in files.iter() {
                    let path = field_str(&file, "path", "synthetic_files")?;
                    let dynamic =
                        py_field(&file, "dynamic_fn").is_some_and(|generator| !generator.is_none());
                    if dynamic {
                        // TODO (L3) [bindings_py]: dynamic synthetic generators stay Python-side until the FFI callback bridge lands.; slop: dynamic synthetic files silently dropped
                        continue;
                    }
                    let content = field_str(&file, "content", "synthetic_files")?;
                    synthetic_files.push(SyntheticFile::static_(&path, &content));
                }
            }
            "snoops" => {
                // TODO (L3) [bindings_py]: snoop on_write callbacks stay Python-side until the FFI callback bridge lands.; slop: snoop callbacks stay Python-side, writes unobserved
                let _ = value;
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
    if mask_engine.is_some() || carryover_bytes.is_some() || !mask_rules.is_empty() {
        config.mask_read = Some(MaskReadConfig {
            engine: mask_engine.unwrap_or_default(),
            carryover_bytes: carryover_bytes.unwrap_or(0),
            rules: mask_rules,
        });
    }
    Ok(PyStorageProjection {
        config,
        synthetic_files,
    })
}
