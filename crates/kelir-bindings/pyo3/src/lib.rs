// see: cans/03-sdk-architecture.md#Python-SDK
// see: cans/01-architecture.md#Foreign-runtime-embedding
// see: cans/02-plugin-model.md#Foreign-plugin-proxy
#![forbid(unsafe_code)]
use std::sync::Arc;
use std::time::Instant;

use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};

pub mod fusion_plugin;
pub mod storage_bridge;

use fusion_plugin::{FusedTrampolineDescriptor, TrampolineTable, attach_runs, convert_descriptor};
use kelir_core::Session;
use kelir_core::planes::storage::storage_reconciler::FsReconcileRequest;
use kelir_interface::driver_interface::{
    ExternalSuspendKind, GuestClock, SnapshotDescriptor, SuspendReason, VmConfig, default_vm_config,
};
use kelir_interface::error_envelope::ErrorEnvelope;
use kelir_interface::handles_lifecycle::VmHandle as CoreVmHandle;
use kelir_interface::hex_codec::decode_hex_fixed;
use kelir_interface::plugin_model::{PluginError, PluginRegistration};
use kelir_interface::runtime_block::block_on;

// see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
fn halt(error: PluginError) -> PyErr {
    envelope_error(&ErrorEnvelope::from(error))
}

// see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
fn envelope_error(envelope: &ErrorEnvelope) -> PyErr {
    PyRuntimeError::new_err(
        serde_json::to_string(envelope).unwrap_or_else(|_| envelope.message.clone()),
    )
}

// see: cans/03-sdk-architecture.md#Session-surfaces
fn missing(reason: &str) -> PyErr {
    PyRuntimeError::new_err(reason.to_string())
}

// see: cans/03-sdk-architecture.md#Session-surfaces
fn u64_of(value: f64, what: &str) -> PyResult<u64> {
    if !(value.is_finite() && value >= 0.0 && value <= u64::MAX as f64) {
        return Err(PyValueError::new_err(what.to_string()));
    }
    Ok(value as u64)
}

// see: cans/08-lifecycle.md#Suspend-and-resume
const SDK_SUSPEND_REASON: SuspendReason =
    SuspendReason::External(ExternalSuspendKind::HumanAuthorizationHold);

// see: cans/08-lifecycle.md#Detached-lifecycle
#[pyfunction]
fn relay_socket_path(session_id: String) -> PyResult<String> {
    let id = decode_hex_fixed::<16>(&session_id).map_err(|_| {
        missing(&format!(
            "attach: session id `{session_id}` is not 32 hex chars"
        ))
    })?;
    Session::relay_socket_path(&id)
        .map_err(halt)
        .map(|path| path.display().to_string())
}

// see: cans/09-snapshots.md#Archives-and-portability
fn snapshot_dict(py: Python<'_>, descriptor: &SnapshotDescriptor) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("id", descriptor.id.clone())?;
    dict.set_item("dirty_bytes", descriptor.dirty_bytes as f64)?;
    dict.set_item("created_at_ns", descriptor.created_at_ns as f64)?;
    dict.set_item("parent_digest", descriptor.parent_digest.clone())?;
    match &descriptor.guest_clock {
        Some(clock) => {
            let guest = PyDict::new(py);
            guest.set_item(
                "absolute_wall_clock_ns",
                clock.absolute_wall_clock_ns as f64,
            )?;
            guest.set_item("monotonic_offset_ns", clock.monotonic_offset_ns as f64)?;
            guest.set_item("timezone", clock.timezone.clone())?;
            guest.set_item("boot_params", clock.boot_params.clone())?;
            dict.set_item("guest_clock", guest)?;
        }
        None => {
            dict.set_item("guest_clock", py.None())?;
        }
    }
    Ok(dict.unbind())
}

// see: cans/09-snapshots.md#Archives-and-portability
fn guest_clock_of(dict: &Bound<'_, PyDict>) -> PyResult<GuestClock> {
    Ok(GuestClock {
        absolute_wall_clock_ns: u64_of(
            dict.get_item("absolute_wall_clock_ns")?
                .ok_or_else(|| missing("guest clock absolute_wall_clock_ns is missing"))?
                .extract()?,
            "guest clock absolute_wall_clock_ns is not a u64",
        )?,
        monotonic_offset_ns: u64_of(
            dict.get_item("monotonic_offset_ns")?
                .ok_or_else(|| missing("guest clock monotonic_offset_ns is missing"))?
                .extract()?,
            "guest clock monotonic_offset_ns is not a u64",
        )?,
        timezone: dict
            .get_item("timezone")?
            .ok_or_else(|| missing("guest clock timezone is missing"))?
            .extract()?,
        boot_params: dict
            .get_item("boot_params")?
            .ok_or_else(|| missing("guest clock boot_params is missing"))?
            .extract()?,
    })
}

// see: cans/09-snapshots.md#Archives-and-portability
fn snapshot_descriptor(record: &Bound<'_, PyDict>) -> PyResult<SnapshotDescriptor> {
    let guest_clock = match record.get_item("guest_clock")? {
        Some(value) if !value.is_none() => Some(guest_clock_of(&value.downcast_into::<PyDict>()?)?),
        _ => None,
    };
    Ok(SnapshotDescriptor {
        id: record
            .get_item("id")?
            .ok_or_else(|| missing("snapshot id is missing"))?
            .extract()?,
        dirty_bytes: u64_of(
            record
                .get_item("dirty_bytes")?
                .ok_or_else(|| missing("snapshot dirty_bytes is missing"))?
                .extract()?,
            "snapshot dirty_bytes is not a u64",
        )?,
        created_at_ns: u64_of(
            record
                .get_item("created_at_ns")?
                .ok_or_else(|| missing("snapshot created_at_ns is missing"))?
                .extract()?,
            "snapshot created_at_ns is not a u64",
        )?,
        parent_digest: match record.get_item("parent_digest")? {
            Some(value) if !value.is_none() => value.extract()?,
            _ => None,
        },
        guest_clock,
    })
}

// see: cans/03-sdk-architecture.md#Session-control-surface
#[pyclass]
pub struct PyVmHandle {
    vm: CoreVmHandle,
}

#[pymethods]
impl PyVmHandle {
    #[pyo3(signature = (name))]
    fn checkpoint(&self, py: Python<'_>, name: String) -> PyResult<Py<PyDict>> {
        let descriptor = block_on(self.vm.checkpoint(name)).map_err(halt)?;
        snapshot_dict(py, &descriptor)
    }

    #[pyo3(signature = (snapshot))]
    fn rollback(&self, snapshot: &Bound<'_, PyDict>) -> PyResult<()> {
        let descriptor = snapshot_descriptor(snapshot)?;
        block_on(self.vm.rollback(&descriptor)).map_err(halt)
    }

    #[pyo3(signature = (path))]
    fn suspend(&self, path: String) -> PyResult<()> {
        block_on(self.vm.suspend(SDK_SUSPEND_REASON, &path)).map_err(halt)
    }

    #[pyo3(signature = (path))]
    fn resume(&self, path: String) -> PyResult<()> {
        block_on(self.vm.resume(&path)).map_err(halt)
    }

    #[pyo3(signature = (seconds))]
    fn clock_skew(&self, seconds: f64) -> PyResult<()> {
        let millis = u64_of(
            seconds * 1000.0,
            "clock skew is not a finite non-negative duration",
        )?;
        block_on(self.vm.clock.skew(std::time::Duration::from_millis(millis))).map_err(halt)
    }

    #[pyo3(signature = (megabytes))]
    fn adjust_balloon(&self, megabytes: u32) -> PyResult<()> {
        block_on(self.vm.adjust_balloon(megabytes)).map_err(halt)?;
        Ok(())
    }

    #[pyo3(signature = (count))]
    fn set_vcpus(&self, count: u32) -> PyResult<()> {
        let count = u8::try_from(count).map_err(|_| missing("vCPU count is not a u8"))?;
        block_on(self.vm.set_vcpus(count)).map_err(halt)
    }

    #[pyo3(signature = (_tag, _size_bytes))]
    fn map_shared_region(&self, _tag: String, _size_bytes: f64) -> PyResult<()> {
        // TODO (L3) [bindings_py]: hand the SharedMemoryHandle across the bridge via SCM_RIGHTS once Session::memory_handoff lands.; slop: handle never crosses, Buffer fabricated or missing
        Err(missing(
            "map_shared_region: the shared-memory handle crosses via SCM_RIGHTS handoff (L3)",
        ))
    }

    fn export_snapshot(&self, py: Python<'_>) -> PyResult<Py<PyBytes>> {
        let bytes = block_on(self.vm.export_snapshot()).map_err(halt)?;
        Ok(PyBytes::new(py, &bytes).unbind())
    }

    #[pyo3(signature = (data))]
    fn import_snapshot(&self, py: Python<'_>, data: Py<PyBytes>) -> PyResult<()> {
        let bytes = data.bind(py).as_bytes().to_vec();
        block_on(self.vm.import_snapshot(&bytes)).map_err(halt)
    }
}

// see: cans/04-terminal-plane.md#Terminal-pipeline
#[pyclass]
pub struct PyTerminalHandle {
    session: Arc<std::sync::Mutex<Session>>,
}

fn lock_session(
    session: &Arc<std::sync::Mutex<Session>>,
) -> PyResult<std::sync::MutexGuard<'_, Session>> {
    session
        .lock()
        .map_err(|_| missing("kelir-core session lock poisoned"))
}

#[pymethods]
impl PyTerminalHandle {
    #[pyo3(signature = (_message))]
    fn notice(&self, _message: String) -> PyResult<()> {
        // TODO (L3) [terminal_plane]: publish the TerminalNotice event once the reactive event bus lands.; slop: notice never published, terminal state unobserved
        Err(missing(
            "terminal notice: TerminalNotice emission lands with the reactive event bus (L3)",
        ))
    }

    #[pyo3(signature = (cols, rows))]
    fn resize(&self, cols: u32, rows: u32) -> PyResult<()> {
        let cols = u16::try_from(cols).map_err(|_| missing("terminal cols is not a u16"))?;
        let rows = u16::try_from(rows).map_err(|_| missing("terminal rows is not a u16"))?;
        lock_session(&self.session)?
            .terminal
            .resize(cols, rows)
            .map_err(halt)
    }

    fn export_screen_matrix(&self, py: Python<'_>) -> PyResult<Py<PyBytes>> {
        let bytes = lock_session(&self.session)?
            .terminal
            .export_screen_matrix()
            .map_err(halt)?;
        Ok(PyBytes::new(py, &bytes).unbind())
    }

    #[pyo3(signature = (port, offset))]
    fn read_from(&self, py: Python<'_>, port: u32, offset: f64) -> PyResult<(Py<PyBytes>, f64)> {
        let cursor = u64_of(offset, "terminal offset is not a u64")?;
        let (bytes, next_offset) = lock_session(&self.session)?
            .terminal
            .read_from(port, cursor)
            .map_err(halt)?;
        Ok((PyBytes::new(py, &bytes).unbind(), next_offset as f64))
    }
}

// see: cans/05-storage-plane.md#Workspace-commit
#[pyclass]
pub struct PyStorageHandle {
    session: Arc<std::sync::Mutex<Session>>,
}

#[pymethods]
impl PyStorageHandle {
    #[pyo3(signature = (cwd, atomic_staging = false, fsync = false))]
    fn reconcile(&self, cwd: String, atomic_staging: bool, fsync: bool) -> PyResult<(f64, f64)> {
        let stats = lock_session(&self.session)?
            .storage
            .reconcile(FsReconcileRequest {
                cwd,
                atomic_staging,
                fsync,
            })
            .map_err(halt)?;
        Ok((stats.files_changed as f64, stats.tombstones_resolved as f64))
    }

    fn discard(&self) -> PyResult<()> {
        lock_session(&self.session)?.storage.discard().map_err(halt)
    }
}

// see: cans/07-wire-plane.md#Port-forwarding
#[pyclass]
pub struct PyWireHandle {
    session: Arc<std::sync::Mutex<Session>>,
}

#[pymethods]
impl PyWireHandle {
    #[pyo3(signature = (guest_port, host_port))]
    fn forward_port(&self, guest_port: u16, host_port: u16) -> PyResult<f64> {
        let allocated = lock_session(&self.session)?
            .wire
            .forward_port(guest_port, Some(host_port))
            .map_err(halt)?;
        Ok(f64::from(allocated))
    }
}

// see: cans/09-snapshots.md#Snapshot-taxonomy
#[pyclass]
pub struct PyCheckpointHandle {
    vm: CoreVmHandle,
}

#[pymethods]
impl PyCheckpointHandle {
    #[pyo3(signature = (label))]
    fn create(&self, py: Python<'_>, label: String) -> PyResult<Py<PyDict>> {
        let descriptor = block_on(self.vm.checkpoint(label)).map_err(halt)?;
        snapshot_dict(py, &descriptor)
    }
}

// see: cans/01-architecture.md#Host-state-root
#[pyclass]
pub struct PyHostHandle {
    session: Arc<std::sync::Mutex<Session>>,
}

#[pymethods]
impl PyHostHandle {
    fn lsm_status(&self) -> PyResult<bool> {
        Ok(lock_session(&self.session)?.host.lsm_status())
    }

    fn pressure_level(&self) -> PyResult<f64> {
        Ok(f64::from(
            lock_session(&self.session)?.host.pressure_level(),
        ))
    }
}

// see: cans/11-protocol.md#Control-channel
#[pyclass]
pub struct PyVsockHandle {}

#[pymethods]
impl PyVsockHandle {
    #[pyo3(signature = (_kind))]
    fn channel(&self, _kind: u32) -> PyResult<()> {
        // TODO (L3) [pm_02_stream_mux_open_data_close_flags]: open the duplex channel once the mux SPSC rings carry stream frames.; slop: channel opened without open/data/close flags
        Err(missing(
            "vsock channel: duplex streams ride the mux SPSC rings (L3)",
        ))
    }
}

// see: cans/03-sdk-architecture.md#Session-data-operations
#[pyclass]
pub struct PyStepHandle {}

#[pymethods]
impl PyStepHandle {
    fn last(&self) -> PyResult<()> {
        // TODO (L3) [lifecycle_checkpoint]: expose the active StepSlot once dispatch constructs HandlerCtx step stacks.; slop: step slot never constructed, hooks run without step
        Err(missing(
            "step last: step slots live in the dispatch HandlerCtx (L3)",
        ))
    }
}

// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
#[pyclass]
pub struct PySessionHandle {
    session: Arc<std::sync::Mutex<Session>>,
    driver_name: String,
    boot_ms: f64,
    vm: CoreVmHandle,
}

impl PySessionHandle {
    // see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
    fn from_session(session: Session, driver_name: String, boot_ms: f64) -> Self {
        let vm = session.vm_handle();
        Self {
            session: Arc::new(std::sync::Mutex::new(session)),
            driver_name,
            boot_ms,
            vm,
        }
    }
}

#[pymethods]
impl PySessionHandle {
    #[getter]
    fn driver_name(&self) -> String {
        self.driver_name.clone()
    }

    #[getter]
    fn boot_ms(&self) -> f64 {
        self.boot_ms
    }

    // see: cans/03-sdk-architecture.md#Sandbox-high-level-facade
    #[pyo3(signature = ())]
    fn session_dir(&self) -> PyResult<String> {
        lock_session(&self.session)?
            .session_dir()
            .map_err(halt)
            .map(|path| path.display().to_string())
    }

    // see: cans/03-sdk-architecture.md#Service-lookup-across-languages
    #[pyo3(signature = (name))]
    fn get_plugin_context(&self, py: Python<'_>, name: String) -> PyResult<Py<PyDict>> {
        let session = lock_session(&self.session)?;
        let Some(record) = session
            .plugin_contexts()
            .into_iter()
            .find(|record| record.name == name)
        else {
            return Err(PyKeyError::new_err(format!(
                "plugin context `{name}` is not registered on this session"
            )));
        };
        let dict = PyDict::new(py);
        dict.set_item("enabled", record.ctx.enabled)?;
        Ok(dict.into())
    }

    #[getter]
    fn vm(&self) -> PyVmHandle {
        PyVmHandle {
            vm: self.vm.clone(),
        }
    }

    #[getter]
    fn terminal(&self) -> PyTerminalHandle {
        PyTerminalHandle {
            session: Arc::clone(&self.session),
        }
    }

    #[getter]
    fn storage(&self) -> PyStorageHandle {
        PyStorageHandle {
            session: Arc::clone(&self.session),
        }
    }

    #[getter]
    fn wire(&self) -> PyWireHandle {
        PyWireHandle {
            session: Arc::clone(&self.session),
        }
    }

    #[getter]
    fn checkpoint(&self) -> PyCheckpointHandle {
        PyCheckpointHandle {
            vm: self.vm.clone(),
        }
    }

    #[getter]
    fn host(&self) -> PyHostHandle {
        PyHostHandle {
            session: Arc::clone(&self.session),
        }
    }

    #[getter]
    fn vsock(&self) -> PyVsockHandle {
        PyVsockHandle {}
    }

    #[getter]
    fn step(&self) -> PyStepHandle {
        PyStepHandle {}
    }

    fn fork(&self) -> PyResult<PySessionHandle> {
        let started = Instant::now();
        let child = {
            let session = lock_session(&self.session)?;
            block_on(session.fork()).map_err(halt)?
        };
        Ok(PySessionHandle::from_session(
            child,
            self.driver_name.clone(),
            started.elapsed().as_secs_f64() * 1000.0,
        ))
    }

    #[pyo3(signature = (_binary, _argv))]
    fn spawn(&self, _binary: String, _argv: Vec<String>) -> PyResult<()> {
        // TODO (L3) [tp_09_exec_stream_sublane_stdio_fidelity]: spawn writes on the EXEC stream kind via the shim wire protocol.; slop: EXEC writes bypass sublane, stdio interleaved
        Err(missing(
            "spawn: EXEC stream dispatch lands with the shim wire protocol (L3)",
        ))
    }

    #[pyo3(signature = (_binary, _argv))]
    fn exec_in_guest(&self, _binary: String, _argv: Vec<String>) -> PyResult<()> {
        // TODO (L3) [tp_09_exec_stream_sublane_stdio_fidelity]: exec_in_guest rides the EXEC stream kind via the shim wire protocol.; slop: exec rides wrong stream kind, output lost
        Err(missing(
            "exec_in_guest: EXEC stream dispatch lands with the shim wire protocol (L3)",
        ))
    }

    #[pyo3(signature = (path))]
    fn suspend(&self, path: String) -> PyResult<()> {
        block_on(self.vm.suspend(SDK_SUSPEND_REASON, &path)).map_err(halt)
    }

    #[pyo3(signature = (path))]
    fn resume(&self, path: String) -> PyResult<()> {
        block_on(self.vm.resume(&path)).map_err(halt)
    }

    fn rollback(&self) -> PyResult<()> {
        // TODO (L3) [lc_03_step_evaluation_taint_rollback]: lineage-derived head rollback needs the dispatch PID lineage.; slop: rollback without lineage, wrong head restored
        Err(missing(
            "rollback: pass a snapshot; lineage-derived head rollback lands with PID tracking (L3)",
        ))
    }

    fn stop(&self) -> PyResult<()> {
        let mut session = lock_session(&self.session)?;
        block_on(session.stop()).map_err(halt)
    }

    fn terminate(&self) -> PyResult<()> {
        lock_session(&self.session)?.kill().map_err(halt)
    }

    fn console(&self) -> PyResult<()> {
        // TODO (L3) [terminal_plane]: attach the PTY console master once the terminal plane owns it.; slop: PTY never attached, console output lost
        Err(missing(
            "console: PTY master attach lands with the terminal plane console (L3)",
        ))
    }

    fn get_causal_delta_from_parent(&self) -> PyResult<()> {
        // TODO (L3) [ar_09_causal_delta_aggregation_merkle]: causal deltas are produced by plugin-ledger over the delta chain.; slop: deltas fabricated, no merkle aggregation
        Err(missing(
            "get_causal_delta_from_parent: causal deltas come from plugin-ledger (L3)",
        ))
    }

    fn handle_ingress(&self) -> PyResult<()> {
        // TODO (L3) [wire_plane]: wire-ingress parked connections surface once the wire plane owns the host listeners.; slop: parked connections never surface, listeners silent
        Err(missing(
            "handle_ingress: inbound connections park in the wire plane listeners (L3)",
        ))
    }
}

#[pyclass]
pub struct PyKelirBuilder {
    config: VmConfig,
    driver_name: String,
    plugins: Vec<PluginRegistration>,
    fused: Option<FusedTrampolineDescriptor>,
    trampolines: TrampolineTable,
    terminal_wiring: Option<kelir_core::planes::terminal::TerminalWiring>,
    wire_wiring: Option<kelir_core::planes::wire::WireWiring>,
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
        let mut wiring = kelir_core::planes::terminal::TerminalWiring::default();
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
        let mut wiring = kelir_core::planes::wire::WireWiring::default();
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
        let plane_wiring = kelir_core::session_lifecycle::PlaneWiring {
            terminal: self.terminal_wiring.clone(),
            wire: self.wire_wiring.clone(),
            storage: self.storage_config.config.clone(),
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

#[pymodule]
fn _kelir(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(relay_socket_path, m)?)?;
    m.add_class::<PyKelirBuilder>()?;
    m.add_class::<PySessionHandle>()?;
    m.add_class::<PyVmHandle>()?;
    m.add_class::<PyTerminalHandle>()?;
    m.add_class::<PyStorageHandle>()?;
    m.add_class::<PyWireHandle>()?;
    m.add_class::<PyCheckpointHandle>()?;
    m.add_class::<PyHostHandle>()?;
    m.add_class::<PyVsockHandle>()?;
    m.add_class::<PyStepHandle>()?;
    Ok(())
}
