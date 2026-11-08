// see: cans/03-sdk-architecture.md#Python-SDK
// see: cans/01-architecture.md#Foreign-runtime-embedding
// see: cans/02-plugin-model.md#Foreign-plugin-proxy
#![forbid(unsafe_code)]
use std::sync::Mutex;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;

pub mod fusion;

use fusion::{FusedTrampolineDescriptor, TrampolineTable, convert_descriptor};
use kelir_core::Session;
use kelir_interface::driver::{VmConfig, default_vm_config};

pub struct PluginRegistration {
    pub name: String,
    pub enabled: bool,
}

#[pyclass]
pub struct PySessionHandle {
    // TODO (L3): expose the engine plane handles over the PyO3 session surface.
    #[allow(dead_code)]
    session: Option<Mutex<Session>>,
}

#[pyclass]
pub struct PyKelirBuilder {
    config: VmConfig,
    plugins: Vec<PluginRegistration>,
    fused: Option<FusedTrampolineDescriptor>,
    trampolines: TrampolineTable,
}

#[pymethods]
impl PyKelirBuilder {
    #[new]
    #[pyo3(signature = (vcpus=None, memory_mb=None, kernel=None, rootfs_erofs=None, tsc_locked=None, enable_watchdog=None))]
    fn new(
        vcpus: Option<u8>,
        memory_mb: Option<u32>,
        kernel: Option<String>,
        rootfs_erofs: Option<String>,
        tsc_locked: Option<bool>,
        enable_watchdog: Option<bool>,
    ) -> Self {
        Self {
            config: default_vm_config(
                vcpus,
                memory_mb,
                kernel,
                rootfs_erofs,
                tsc_locked,
                enable_watchdog,
            ),
            plugins: Vec::new(),
            fused: None,
            trampolines: TrampolineTable::default(),
        }
    }

    #[pyo3(signature = (name, enabled=None))]
    fn use_plugin(&mut self, name: String, enabled: Option<bool>) -> PyResult<()> {
        if name.trim().is_empty() {
            return Err(PyValueError::new_err(
                "use_plugin: plugin name must not be empty",
            ));
        }
        if self.plugins.iter().any(|plugin| plugin.name == name) {
            return Ok(());
        }
        self.plugins.push(PluginRegistration {
            name,
            enabled: enabled.unwrap_or(true),
        });
        Ok(())
    }

    fn fuse_hooks(&mut self, descriptor: &Bound<'_, PyDict>) -> PyResult<()> {
        if self.fused.is_some() {
            return Err(PyValueError::new_err(
                "fuse_hooks: the trampoline descriptor is frozen at seal",
            ));
        }
        let converted = convert_descriptor(descriptor)?;
        self.trampolines =
            TrampolineTable::from_descriptor(&converted).map_err(PyValueError::new_err)?;
        self.fused = Some(converted);
        Ok(())
    }

    fn trampoline_entry_count(&self) -> u32 {
        self.trampolines.entry_count()
    }

    fn boot(&self) -> PyResult<PySessionHandle> {
        let Some(descriptor) = &self.fused else {
            return Err(PyRuntimeError::new_err(
                "boot: fuse_hooks must run before boot",
            ));
        };
        if descriptor.rings.len() as u32 != self.trampolines.entry_count() {
            return Err(PyRuntimeError::new_err(
                "boot: the trampoline table is not one entry per ring event",
            ));
        }
        for chain in &descriptor.rings {
            for hook in &chain.hooks {
                if let Some(plugin_name) = &hook.plugin_name {
                    match self
                        .plugins
                        .iter()
                        .find(|plugin| &plugin.name == plugin_name)
                    {
                        Some(registration) if registration.enabled => {}
                        Some(_) => {
                            return Err(PyRuntimeError::new_err(format!(
                                "boot: fused ring {} carries hooks of disabled plugin `{plugin_name}`",
                                chain.ring.as_tag()
                            )));
                        }
                        None => {
                            return Err(PyRuntimeError::new_err(format!(
                                "boot: fused ring {} carries hooks of unregistered plugin `{plugin_name}`",
                                chain.ring.as_tag()
                            )));
                        }
                    }
                }
            }
        }
        if let Some(reason) = self.config.missing_boot_path() {
            return Err(PyValueError::new_err(reason));
        }
        // TODO (L3): bind each fused ring chain to its single trampoline entry as one closure and dispatch through the kelir-core pipeline.
        let plugins: Vec<kelir_core::PluginRegistration> = self
            .plugins
            .iter()
            .map(|plugin| kelir_core::PluginRegistration::new(plugin.name.clone(), plugin.enabled))
            .collect();
        let session = Session::boot_with_plugins(self.config.clone(), &plugins)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        Ok(PySessionHandle {
            session: Some(Mutex::new(session)),
        })
    }
}

#[pymodule]
fn _kelir(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyKelirBuilder>()?;
    m.add_class::<PySessionHandle>()?;
    Ok(())
}
