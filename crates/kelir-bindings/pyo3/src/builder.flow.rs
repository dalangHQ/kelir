// see: cans/03-sdk-architecture.md#Python-SDK
use std::sync::Arc;
use std::time::Instant;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use kelir_engine::Session;
use kelir_interface::driver_interface::VmConfig;
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
    wire_dns_to_ip_pinning: Option<bool>,
    storage_config: storage_bridge::PyStorageProjection,
}

#[pymethods]
impl PyKelirBuilder {
    #[new]
    #[pyo3(signature = (vcpus=None, memory_mb=None, kernel=None, rootfs_erofs=None, tsc_locked=None, enable_watchdog=None, driver=None, mask_proc_environ=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        vcpus: Option<u8>,
        memory_mb: Option<u32>,
        kernel: Option<String>,
        rootfs_erofs: Option<String>,
        tsc_locked: Option<bool>,
        enable_watchdog: Option<bool>,
        driver: Option<String>,
        mask_proc_environ: Option<bool>,
    ) -> Self {
        let mut config = VmConfig::default();
        if let Some(vcpus) = vcpus {
            config.vcpus = vcpus;
        }
        if let Some(memory_mb) = memory_mb {
            config.memory_mb = memory_mb;
        }
        if let Some(kernel) = kernel {
            config.kernel_path = kernel.into();
        }
        if let Some(rootfs_erofs) = rootfs_erofs {
            config.rootfs_erofs_path = rootfs_erofs.into();
        }
        if let Some(tsc_locked) = tsc_locked {
            config.tsc_locked = tsc_locked;
        }
        if let Some(enable_watchdog) = enable_watchdog {
            config.enable_watchdog = enable_watchdog;
        }
        if let Some(mask_proc_environ) = mask_proc_environ {
            config.mask_proc_environ = mask_proc_environ;
        }
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
            wire_dns_to_ip_pinning: None,
            storage_config: storage_bridge::PyStorageProjection::default(),
        }
    }

    // see: cans/04-terminal-plane.md#Terminal-pipeline
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
                // TODO (L3) [bindings_py]: forward wireframe_grid flags into TerminalWiring once it carries them; slop: grid tracking flags dropped at the wiring boundary.
                "wireframe_grid" => {
                    let grid = value.downcast::<PyDict>().map_err(|_| {
                        PyValueError::new_err("terminal wireframe_grid is not a dict")
                    })?;
                    for (grid_key, grid_value) in grid.iter() {
                        match grid_key.to_string().as_str() {
                            "track_screen_matrix" | "export_on_trap" => {
                                let _ = grid_value.extract::<bool>().map_err(|_| {
                                    PyValueError::new_err(
                                        "terminal wireframe_grid flags are not bools",
                                    )
                                })?;
                            }
                            other => {
                                return Err(PyValueError::new_err(format!(
                                    "set_terminal_config: unknown wireframe_grid key `{other}`"
                                )));
                            }
                        }
                    }
                }
                "sanitizer" => {
                    let sanitizer = value
                        .downcast::<PyDict>()
                        .map_err(|_| PyValueError::new_err("terminal sanitizer is not a dict"))?;
                    for (sanitizer_key, sanitizer_value) in sanitizer.iter() {
                        match sanitizer_key.to_string().as_str() {
                            "drop_osc52" => {
                                wiring.drop_osc52 =
                                    sanitizer_value.extract::<bool>().map_err(|_| {
                                        PyValueError::new_err(
                                            "terminal sanitizer drop_osc52 is not a bool",
                                        )
                                    })?;
                            }
                            "strip_title" => {
                                wiring.strip_title =
                                    sanitizer_value.extract::<bool>().map_err(|_| {
                                        PyValueError::new_err(
                                            "terminal sanitizer strip_title is not a bool",
                                        )
                                    })?;
                            }
                            other => {
                                return Err(PyValueError::new_err(format!(
                                    "set_terminal_config: unknown sanitizer key `{other}`"
                                )));
                            }
                        }
                    }
                }
                "stream_engine" => {
                    let _ = value.extract::<String>().map_err(|_| {
                        PyValueError::new_err("terminal stream_engine is not a str")
                    })?;
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
        let mut dns_to_ip_pinning: Option<bool> = None;
        for (key, value) in cfg.iter() {
            match key.to_string().as_str() {
                "sinkhole" => {
                    let sinkhole = value
                        .downcast::<PyDict>()
                        .map_err(|_| PyValueError::new_err("wire sinkhole is not a dict"))?;
                    let mut rules: Vec<kelir_interface::wire::net::SinkholeRule> = Vec::new();
                    for (sub_key, sub_value) in sinkhole.iter() {
                        match sub_key.to_string().as_str() {
                            "rules" => {
                                let entries = sub_value.downcast::<PyList>().map_err(|_| {
                                    PyValueError::new_err("sinkhole rules is not a list of dicts")
                                })?;
                                for entry in entries.iter() {
                                    let entry = entry.downcast::<PyDict>().map_err(|_| {
                                        PyValueError::new_err(
                                            "sinkhole rules entries must be dicts",
                                        )
                                    })?;
                                    let host = entry
                                        .get_item("host")?
                                        .ok_or_else(|| {
                                            PyValueError::new_err("sinkhole rule has no host")
                                        })?
                                        .extract::<String>()
                                        .map_err(|_| {
                                            PyValueError::new_err("sinkhole rule host is not a str")
                                        })?;
                                    let allow = match entry.get_item("allow")? {
                                        Some(flag) => flag.extract::<bool>().map_err(|_| {
                                            PyValueError::new_err(
                                                "sinkhole rule allow is not a bool",
                                            )
                                        })?,
                                        None => false,
                                    };
                                    rules.push(kelir_interface::wire::net::SinkholeRule {
                                        host,
                                        allow,
                                    });
                                }
                            }
                            other => {
                                return Err(PyValueError::new_err(format!(
                                    "set_wire_config: unknown sinkhole key `{other}`"
                                )));
                            }
                        }
                    }
                    // see: cans/07-wire-plane.md#Sinkhole-rules
                    wiring.sinkhole = kelir_interface::wire::net::SinkholeConfig { rules };
                }
                "ssh_agent_bridge" => {
                    wiring.ssh_agent_bridge = value
                        .extract::<bool>()
                        .map_err(|_| PyValueError::new_err("wire ssh_agent_bridge is not a bool"))?
                }
                "block_abstract_unix_sockets" => {
                    wiring.block_abstract_unix_sockets = value.extract::<bool>().map_err(|_| {
                        PyValueError::new_err("wire block_abstract_unix_sockets is not a bool")
                    })?
                }
                "tap_filter" => {
                    wiring.allow_udp_proxied = value
                        .extract::<String>()
                        .map_err(|_| PyValueError::new_err("wire tap_filter is not a str"))?
                        == "AllowUdpProxied"
                }
                "dial_timeout_ms" => {
                    wiring.dial_timeout_ms = value
                        .extract::<u64>()
                        .map_err(|_| PyValueError::new_err("wire dial_timeout_ms is not a u64"))?;
                }
                "dns_to_ip_pinning" => {
                    dns_to_ip_pinning = Some(value.extract::<bool>().map_err(|_| {
                        PyValueError::new_err("wire dns_to_ip_pinning is not a bool")
                    })?);
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
                            .and_then(|dest| dest.extract::<String>().ok());
                        let forward_unix_socket = proxy
                            .get_item("forward_unix_socket")
                            .ok()
                            .flatten()
                            .and_then(|socket| socket.extract::<String>().ok());
                        match forward_unix_socket {
                            Some(socket) => {
                                wiring.l4_unix_diversions.push((listen_port, socket.into()));
                            }
                            None => {
                                let destination = destination.ok_or_else(|| {
                                    PyValueError::new_err("l4_proxy destination is not a str")
                                })?;
                                wiring.l4_proxies.push((listen_port, destination));
                            }
                        }
                    }
                }
                "divert_ports" => {
                    let entries = value.downcast::<PyList>().map_err(|_| {
                        PyValueError::new_err("wire divert_ports is not a list of dicts")
                    })?;
                    for entry in entries.iter() {
                        let entry = entry.downcast::<PyDict>().map_err(|_| {
                            PyValueError::new_err("wire divert_ports entries must be dicts")
                        })?;
                        let listen_port = entry
                            .get_item("listen_port")
                            .ok()
                            .flatten()
                            .and_then(|port| port.extract::<u16>().ok())
                            .ok_or_else(|| {
                                PyValueError::new_err("divert_port listen_port is not a u16")
                            })?;
                        let socket = entry
                            .get_item("forward_unix_socket")
                            .ok()
                            .flatten()
                            .and_then(|socket| socket.extract::<String>().ok())
                            .ok_or_else(|| {
                                PyValueError::new_err(
                                    "divert_port forward_unix_socket is not a str",
                                )
                            })?;
                        wiring.l4_unix_diversions.push((listen_port, socket.into()));
                    }
                }
                other => {
                    return Err(PyValueError::new_err(format!(
                        "set_wire_config: unknown or non-bridged key `{other}`"
                    )));
                }
            }
        }
        wiring.dns_to_ip_pinning = dns_to_ip_pinning.unwrap_or(false);
        self.wire_dns_to_ip_pinning = dns_to_ip_pinning;
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
        // L3-GAP: [bp_03_python_fused_trampoline_single_crossing_chained_hooks]: map the callable return value into a Verdict and marshal the ring event payload across the GIL boundary; slop: Python hook runs fire-and-forget, its verdict never gates dispatch
        let plugins: Vec<PluginRegistration> = self.plugins.clone();
        // see: cans/01-architecture.md#Host-runtime
        let metrics = Arc::new(kelir_interface::telemetry::MetricRegistry::new());
        let dns_ops: Arc<dyn kelir_interface::wire::dns::DnsOps> = kelir_wire::dns_ops(
            std::sync::Arc::new(kelir_engine::dispatch_plugin::PipelineEngine::new()),
        );
        // L3-GAP: [composition_root]: bind RealFuseOps once Session shares Arc<PipelineEngine>; slop: FUSE dispatch unreachable from the session, dispatcher built only in tests
        // L3-GAP: [composition_root]: bind the kelir-storage Reconciler once the host broker journal anchors broker_dir; slop: StoragePlane::reconcile fails closed without it, terminate flush never commits
        // TODO (L3) [plugin_engine]: per_exec_cgroup and deny_abi_compat are ShimProfile fields with no native shim-config surface on this builder; slop: seccomp fields accepted by the profile schema, never reach the shim
        let plane_wiring = kelir_engine::session_lifecycle::PlaneWiring {
            terminal: self.terminal_wiring.clone(),
            terminal_ops: Some(kelir_terminal::terminal_ops(metrics.clone())),
            checkpoint_ops: Some(kelir_checkpoint::checkpoint_ops(metrics.clone())),
            checkpoint_config: None,
            host_ops: Some(kelir_host::host_ops(metrics.clone())),
            host_config: None,
            net_ops: Some(kelir_wire::net_ops(
                std::sync::Arc::new(kelir_engine::dispatch_plugin::PipelineEngine::new()),
                Arc::clone(&dns_ops),
            )),
            dns_ops: Some(dns_ops),
            udp_nat_ops: Some(kelir_wire::udp_nat_ops()),
            l4_ops: Some(kelir_wire::l4_ops()),
            storage_io: Some(kelir_storage_io::storage_io()),
            reconciler: None,
            fuse_ops: None,
            wire: self.wire_wiring.clone(),
            storage: self.storage_config.config.clone(),
        };
        let started = Instant::now();
        let session = Session::boot_with_plugins(
            self.config.clone(),
            &plugins,
            Some(descriptor),
            plane_wiring,
        )
        .map_err(halt)?;
        // L3-GAP: [hypervisor_driver]: re-source driver_name from the booted core session once it exposes the resolved driver.; slop: driver name stale, wrong backend reported
        Ok(PySessionHandle::from_session(
            session,
            self.driver_name.clone(),
            started.elapsed().as_secs_f64() * 1000.0,
        ))
    }
}
