// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
use std::sync::Arc;
use std::time::Instant;

use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

use kelir_engine::Session;
use kelir_interface::addon_abi::KELIR_ADDON_ABI_VERSION;
use kelir_interface::driver_interface::{
    ExternalSuspendKind, GuestClock, SnapshotDescriptor, SuspendReason,
};
use kelir_interface::error_envelope::ErrorEnvelope;
use kelir_interface::handles_lifecycle::VmHandle as CoreVmHandle;
use kelir_interface::hex_codec::decode_hex_fixed;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::runtime_block::block_on;
use kelir_interface::storage::FsReconcileRequest;

// see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
pub(crate) fn halt(error: PluginError) -> PyErr {
    envelope_error(&ErrorEnvelope::from(error))
}

// see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
pub(crate) fn envelope_error(envelope: &ErrorEnvelope) -> PyErr {
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

// see: cans/08-lifecycle.md#Suspend-and-resume
fn suspend_reason_of(record: &Bound<'_, PyDict>) -> PyResult<SuspendReason> {
    let kind: String = record
        .get_item("kind")?
        .ok_or_else(|| missing("suspend reason kind is missing"))?
        .extract()?;
    match kind.as_str() {
        "resource-pressure" => Ok(SuspendReason::ResourcePressure),
        "external" => {
            let detail: String = record
                .get_item("detail")?
                .ok_or_else(|| missing("suspend reason external detail is missing"))?
                .extract()?;
            let external = match detail.as_str() {
                "human-authorization-hold" => ExternalSuspendKind::HumanAuthorizationHold,
                "wire-connection-parking" => ExternalSuspendKind::WireConnectionParking,
                "step-evaluation-yield" => ExternalSuspendKind::StepEvaluationYield,
                "host-power-event-snapshot" => ExternalSuspendKind::HostPowerEventSnapshot,
                other => {
                    return Err(missing(&format!(
                        "suspend reason detail `{other}` is not an ExternalSuspendKind"
                    )))
                }
            };
            Ok(SuspendReason::External(external))
        }
        other => Err(missing(&format!(
            "suspend reason kind `{other}` is not external|resource-pressure"
        ))),
    }
}

// see: cans/08-lifecycle.md#Suspend-and-resume
fn suspend_image_path(session: &Arc<std::sync::Mutex<Session>>) -> PyResult<String> {
    let dir = lock_session(session)?.session_dir().map_err(halt)?;
    Ok(dir.join("suspend.img").display().to_string())
}

// see: cans/08-lifecycle.md#Detached-lifecycle
#[pyfunction]
pub(crate) fn relay_socket_path(session_id: String) -> PyResult<String> {
    let id = decode_hex_fixed::<16>(&session_id).map_err(|_| {
        missing(&format!(
            "attach: session id `{session_id}` is not 32 hex chars"
        ))
    })?;
    Session::relay_socket_path(&id)
        .map_err(halt)
        .map(|path| path.display().to_string())
}

// see: cans/03-sdk-architecture.md#Python-SDK
#[pyfunction]
pub(crate) fn abi_version() -> u32 {
    KELIR_ADDON_ABI_VERSION
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
        .map_err(|_| missing("kelir-engine session lock poisoned"))
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
    pub(crate) fn from_session(session: Session, driver_name: String, boot_ms: f64) -> Self {
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

    #[pyo3(signature = (reason))]
    fn suspend(&self, reason: &Bound<'_, PyDict>) -> PyResult<()> {
        let path = suspend_image_path(&self.session)?;
        block_on(self.vm.suspend(suspend_reason_of(reason)?, &path)).map_err(halt)
    }

    fn resume(&self) -> PyResult<()> {
        let path = suspend_image_path(&self.session)?;
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
