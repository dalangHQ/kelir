// see: cans/03-sdk-architecture.md#Python-SDK
use std::sync::Arc;
use std::time::Instant;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use kelir_engine::Session;
use kelir_interface::driver_interface::{VmConfig, default_vm_config};
use kelir_interface::plugin_model::PluginRegistration;

use crate::fusion_plugin::{
    FusedTrampolineDescriptor, TrampolineTable, attach_runs, convert_descriptor,
};
use crate::handles::PySessionHandle;
use crate::handles::halt;
use crate::storage_bridge;

#[pyclass]
pub struct PyKelirBuilder {
    config: VmConfig,
    driver_name: String,
    plugins: Vec<PluginRegistration>,
    fused: Option<FusedTrampolineDescriptor>,
    trampolines: TrampolineTable,
    terminal_wiring: Option<kelir_interface::terminal::TerminalWiring>,
    wire_wiring: Option<kelir_interface::wire::net::WireWiring>,
    storage_config: storage_bridge::PyStorageProjection,
}

#[pymethods]
impl PyKelirBuilder {
    #[new]
    #[pyo3(signature = (vcpus=None, memory_mb=None, kernel=None, rootfs_erofs=None, tsc_locked=None, enable_watchdog=None, driver=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        vcpus: Option<u8>,
        memory_mb: Option<u32>,
        kernel: Option<String>,
        rootfs_erofs: Option<String>,
        tsc_locked: Option<bool>,
        enable_watchdog: Option<bool>,
        driver: Option<String>,
    ) -> Self {
        let mut config = default_vm_config(
            vcpus,
            memory_mb,
            kernel,
            rootfs_erofs,
            tsc_locked,
            enable_watchdog,
        );
        // see: cans/01-architecture.md#Driver-backends
        config.driver_policy.driver = driver.clone();
        Self {
            config,
            driver_name: driver.unwrap_or_else(|| "auto".to_string()),
            plugins: Vec::new(),
            fused: None,
            trampolines: TrampolineTable::default(),
            terminal_wiring: None,
            wire_wiring: None,
            storage_config: storage_bridge::PyStorageProjection::default(),
        }
    }

    // see: cans/04-terminal-plane.md#Terminal-pipeline
    // TODO (L3) [bindings_py]: prompt rules and the token-shield clamp callback stay Python-side until the FFI callback bridge lands.; slop: callbacks stay Python-side, silently dropped
    fn set_terminal_config(&mut self, cfg: &Bound<'_, PyDict>) -> PyResult<()> {
        let mut wiring = kelir_interface::terminal::TerminalWiring::default();
        for (key, value) in cfg.iter() {
            match key.to_string().as_str() {
                "cols" => {
                    wiring.cols = value
                        .extract::<u16>()
                        .map_err(|_| PyValueError::new_err("terminal cols is not a u16"))?
                }
                "rows" => {
                    wiring.rows = value
                        .extract::<u16>()
                        .map_err(|_| PyValueError::new_err("terminal rows is not a u16"))?
                }
                "drop_osc52" => {
                    wiring.drop_osc52 = value
                        .extract::<bool>()
                        .map_err(|_| PyValueError::new_err("terminal drop_osc52 is not a bool"))?
                }
                "strip_title" => {
                    wiring.strip_title = value
                        .extract::<bool>()
                        .map_err(|_| PyValueError::new_err("terminal strip_title is not a bool"))?
                }
                "stream_engine" => {
                    let _ = value.extract::<String>().map_err(|_| {
                        PyValueError::new_err("terminal stream_engine is not a str")
                    })?;
                }
                "token_shield" | "prompts" => {
                    let _ = value;
                }
                other => {
                    return Err(PyValueError::new_err(format!(
                        "set_terminal_config: unknown or non-bridged key `{other}`"
                    )));
                }
            }
        }
        self.terminal_wiring = Some(wiring);
        Ok(())
    }

    // see: cans/05-storage-plane.md#Storage-architecture
    fn set_storage_config(&mut self, cfg: &Bound<'_, PyDict>) -> PyResult<()> {
        self.storage_config = storage_bridge::storage_config(cfg)?;
        Ok(())
    }

    // see: cans/07-wire-plane.md#DNS-policy-filter
    fn set_wire_config(&mut self, cfg: &Bound<'_, PyDict>) -> PyResult<()> {
        let mut wiring = kelir_interface::wire::net::WireWiring::default();
        for (key, value) in cfg.iter() {
            match key.to_string().as_str() {
                "sinkhole" => {
                    let sinkhole = value
                        .downcast::<PyDict>()
                        .map_err(|_| PyValueError::new_err("wire sinkhole is not a dict"))?;
                    for (sub_key, sub_value) in sinkhole.iter() {
                        match sub_key.to_string().as_str() {
                            "allowlist" => {
                                wiring.sinkhole_allowlist =
                                    sub_value.extract::<Vec<String>>().map_err(|_| {
                                        PyValueError::new_err(
                                            "sinkhole allowlist is not a list of str",
                                        )
                                    })?
                            }
                            "drop_all" => {
                                wiring.sinkhole_drop_all =
                                    sub_value.extract::<bool>().map_err(|_| {
                                        PyValueError::new_err("sinkhole drop_all is not a bool")
                                    })?
                            }
                            other => {
                                return Err(PyValueError::new_err(format!(
                                    "set_wire_config: unknown sinkhole key `{other}`"
                                )));
                            }
                        }
                    }
                }
                "ssh_agent_bridge" => {
                    wiring.ssh_agent_bridge = value
                        .extract::<bool>()
                        .map_err(|_| PyValueError::new_err("wire ssh_agent_bridge is not a bool"))?
                }
                "tap_filter" => {
                    wiring.allow_udp_proxied = value
                        .extract::<String>()
                        .map_err(|_| PyValueError::new_err("wire tap_filter is not a str"))?
                        == "AllowUdpProxied"
                }
                "vaults" => {
                    let vaults = value
                        .downcast::<PyList>()
                        .map_err(|_| PyValueError::new_err("wire vaults is not a list of dicts"))?;
                    for vault in vaults.iter() {
                        let vault = vault.downcast::<PyDict>().map_err(|_| {
                            PyValueError::new_err("wire vaults entries must be dicts")
                        })?;
                        if let Some(placeholder) = vault.get_item("placeholder")? {
                            wiring.vault_placeholders.push(
                                placeholder.extract::<String>().map_err(|_| {
                                    PyValueError::new_err("vault placeholder is not a str")
                                })?,
                            );
                        }
                    }
                }
                "l4_proxies" => {
                    let proxies = value.downcast::<PyList>().map_err(|_| {
                        PyValueError::new_err("wire l4_proxies is not a list of dicts")
                    })?;
                    for proxy in proxies.iter() {
                        let proxy = proxy.downcast::<PyDict>().map_err(|_| {
                            PyValueError::new_err("wire l4_proxies entries must be dicts")
                        })?;
                        let listen_port = proxy
                            .get_item("listen_port")
                            .ok()
                            .flatten()
                            .and_then(|port| port.extract::<u16>().ok())
                            .ok_or_else(|| {
                                PyValueError::new_err("l4_proxy listen_port is not a u16")
                            })?;
                        let destination = proxy
                            .get_item("destination")
                            .ok()
                            .flatten()
                            .and_then(|dest| dest.extract::<String>().ok())
                            .ok_or_else(|| {
                                PyValueError::new_err("l4_proxy destination is not a str")
                            })?;
                        wiring.l4_proxies.push((listen_port, destination));
                    }
                }
                other => {
                    return Err(PyValueError::new_err(format!(
                        "set_wire_config: unknown or non-bridged key `{other}`"
                    )));
                }
            }
        }
        self.wire_wiring = Some(wiring);
        Ok(())
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

    #[pyo3(signature = (descriptor, runs=None))]
    fn fuse_hooks(
        &mut self,
        descriptor: &Bound<'_, PyDict>,
        runs: Option<&Bound<'_, PyList>>,
    ) -> PyResult<()> {
        if self.fused.is_some() {
            return Err(PyValueError::new_err(
                "fuse_hooks: the trampoline descriptor is frozen at seal",
            ));
        }
        let mut converted = convert_descriptor(descriptor)?;
        if let Some(runs) = runs {
            attach_runs(&mut converted, runs)?;
        }
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
        // TODO (L3) [bp_03_python_fused_trampoline_single_crossing_chained_hooks]: map the callable return value into a Verdict and marshal the ring event payload across the GIL boundary; slop: Python hook runs fire-and-forget, its verdict never gates dispatch
        let plugins: Vec<PluginRegistration> = self.plugins.clone();
        // see: cans/01-architecture.md#Host-runtime
        let metrics = Arc::new(kelir_otlp::registry_otlp::MetricRegistry::new());
        let dns_ops: Arc<dyn kelir_interface::wire::dns::DnsOps> = kelir_wire::dns_ops();
        // TODO (L3) [composition_root]: bind RealFuseOps once Session shares Arc<PipelineEngine>; slop: FUSE dispatch unreachable from the session, dispatcher built only in tests
        // TODO (L3) [composition_root]: bind the kelir-storage Reconciler once the host broker journal anchors broker_dir; slop: StoragePlane::reconcile fails closed without it, terminate flush never commits
        let plane_wiring = kelir_engine::session_lifecycle::PlaneWiring {
            terminal: self.terminal_wiring.clone(),
            terminal_ops: Some(kelir_terminal::terminal_ops(metrics.clone())),
            checkpoint_ops: Some(kelir_checkpoint::checkpoint_ops(metrics.clone())),
            host_ops: Some(kelir_host::host_ops(metrics.clone())),
            net_ops: Some(kelir_wire::net_ops(Arc::clone(&dns_ops))),
            dns_ops: Some(dns_ops),
            udp_nat_ops: Some(kelir_wire::udp_nat_ops()),
            l4_ops: Some(kelir_wire::l4_ops()),
            storage_io: Some(kelir_storage_io::storage_io()),
            reconciler: None,
            fuse_ops: None,
            wire: self.wire_wiring.clone(),
            storage: self.storage_config.config.clone(),
            snoops: Vec::new(),
        };
        let started = Instant::now();
        let session = Session::boot_with_plugins(
            self.config.clone(),
            &plugins,
            Some(descriptor),
            self.storage_config.synthetic_files.clone(),
            plane_wiring,
        )
        .map_err(halt)?;
        // TODO (L3) [hypervisor_driver]: re-source driver_name from the booted core session once it exposes the resolved driver.; slop: driver name stale, wrong backend reported
        Ok(PySessionHandle::from_session(
            session,
            self.driver_name.clone(),
            started.elapsed().as_secs_f64() * 1000.0,
        ))
    }
}
