// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
// TODO (L3) [bindings_py]: implement async PySessionHandle bodies across the extension module; slop: handle methods never cross the extension, calls raise instead of dispatching.
// TODO (L3) [bindings_py]: async handle bodies for PySessionHandle (fork, spawn, exec_in_guest, suspend, resume, rollback, stop, terminate) relocate from handles.flow.rs; slop: handle methods return fabricated results without crossing the extension module

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::handles::missing;

    fn map_shared_region(&self, _tag: String, _size_bytes: f64) -> PyResult<()> {
        // TODO (L3) [bindings_py]: hand the SharedMemoryHandle across the bridge via SCM_RIGHTS once Session::memory_handoff lands.; slop: handle never crosses, Buffer fabricated or missing
        Err(missing(
            "map_shared_region: the shared-memory handle crosses via SCM_RIGHTS handoff",
        ))
    }

    fn spawn(&self, _binary: String, _argv: Vec<String>) -> PyResult<()> {
        // TODO (L3) [tp_09_exec_stream_sublane_stdio_fidelity]: spawn writes on the EXEC stream kind via the shim wire protocol.; slop: EXEC writes bypass sublane, stdio interleaved
        Err(missing(
            "spawn: EXEC stream dispatch lands with the shim wire protocol",
        ))
    }

    fn exec_in_guest(&self, _binary: String, _argv: Vec<String>) -> PyResult<()> {
        // TODO (L3) [tp_09_exec_stream_sublane_stdio_fidelity]: exec_in_guest rides the EXEC stream kind via the shim wire protocol.; slop: exec rides wrong stream kind, output lost
        Err(missing(
            "exec_in_guest: EXEC stream dispatch lands with the shim wire protocol",
        ))
    }

    fn steer(&self, exec_id: f64, message: String) -> PyResult<()> {
        // TODO (L3) [terminal_plane]: flush steering guidance onto the EXEC sub-lane 0x02 before the exit EOF frame; slop: steer Ok, guidance never reaches the agent before exit
        Err(missing(&format!(
            "steer: exec {exec_id} guidance `{message}` flushes onto the EXEC sub-lane 0x02 before EOF"
        )))
    }

    fn rollback(&self) -> PyResult<()> {
        // TODO (L3) [lc_03_step_evaluation_taint_rollback]: lineage-derived head rollback needs the dispatch PID lineage.; slop: rollback without lineage, wrong head restored
        Err(missing(
            "rollback: pass a snapshot; lineage-derived head rollback lands with PID tracking",
        ))
    }

    fn console(&self) -> PyResult<()> {
        // TODO (L3) [terminal_plane]: attach the PTY console master once the terminal plane owns it.; slop: PTY never attached, console output lost
        Err(missing(
            "console: PTY master attach lands with the terminal plane console",
        ))
    }

    fn get_causal_delta_from_parent(&self) -> PyResult<()> {
        // TODO (L3) [ar_09_causal_delta_aggregation_merkle]: causal deltas are produced by plugin-ledger over the delta chain.; slop: deltas fabricated, no merkle aggregation
        Err(missing(
            "get_causal_delta_from_parent: causal deltas come from plugin-ledger",
        ))
    }

    fn handle_ingress(&self, _options: &Bound<'_, PyDict>) -> PyResult<()> {
        // TODO (L3) [wire_plane]: wire-ingress parked connections surface once the wire plane owns the host listeners.; slop: parked connections never surface, listeners silent
        Err(missing(
            "handle_ingress: inbound connections park in the wire plane listeners",
        ))
    }

