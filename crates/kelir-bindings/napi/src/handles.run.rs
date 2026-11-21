// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
// TODO (L3) [bindings_ts]: implement async NativeVmHandle / NativeSessionHandle bodies across the addon; slop: handle methods never cross the addon, calls throw instead of dispatching.
// TODO (L3) [bindings_ts]: async handle bodies for NativeVmHandle / NativeSessionHandle (checkpoint, rollback, suspend, resume, fork, spawn, exec_in_guest, stop, terminate) relocate from handles.flow.rs; slop: handle methods return fabricated records without crossing the addon

use napi::bindgen_prelude::{Buffer, Object};
use napi::Result;

use crate::handles::missing;

    pub async fn map_shared_region(&self, _tag: String, _size_bytes: f64) -> Result<Buffer> {
        // TODO (L3) [bindings_ts]: hand the SharedMemoryHandle across the bridge via SCM_RIGHTS once Session::memory_handoff lands.; slop: handle never crosses, Buffer fabricated or missing
        Err(missing(
            "map_shared_region: the shared-memory handle crosses via SCM_RIGHTS handoff",
        ))
    }

    pub async fn spawn(&self, _binary: String, _argv: Vec<String>) -> Result<()> {
        // TODO (L3) [tp_09_exec_stream_sublane_stdio_fidelity]: spawn writes on the EXEC stream kind via the shim wire protocol.; slop: EXEC writes bypass sublane, stdio interleaved
        Err(missing(
            "spawn: EXEC stream dispatch lands with the shim wire protocol",
        ))
    }

    pub async fn exec_in_guest(&self, _binary: String, _argv: Vec<String>) -> Result<()> {
        // TODO (L3) [tp_09_exec_stream_sublane_stdio_fidelity]: exec_in_guest rides the EXEC stream kind via the shim wire protocol.; slop: exec rides wrong stream kind, output lost
        Err(missing(
            "exec_in_guest: EXEC stream dispatch lands with the shim wire protocol",
        ))
    }

    pub async fn steer(&self, exec_id: f64, message: String) -> Result<()> {
        // TODO (L3) [terminal_plane]: flush steering guidance onto the EXEC sub-lane 0x02 before the exit EOF frame; slop: steer Ok, guidance never reaches the agent before exit
        Err(missing(&format!(
            "steer: exec {exec_id} guidance `{message}` flushes onto the EXEC sub-lane 0x02 before EOF"
        )))
    }

    pub async fn rollback(&self) -> Result<()> {
        // TODO (L3) [lc_03_step_evaluation_taint_rollback]: lineage-derived head rollback needs the dispatch PID lineage.; slop: rollback without lineage, wrong head restored
        Err(missing(
            "rollback: pass a snapshot; lineage-derived head rollback lands with PID tracking",
        ))
    }

    pub async fn console(&self) -> Result<()> {
        // TODO (L3) [terminal_plane]: attach the PTY console master once the terminal plane owns it.; slop: PTY never attached, console output lost
        Err(missing(
            "console: PTY master attach lands with the terminal plane console",
        ))
    }

    pub async fn get_causal_delta_from_parent(&self) -> Result<()> {
        // TODO (L3) [ar_09_causal_delta_aggregation_merkle]: causal deltas are produced by plugin-ledger over the delta chain.; slop: deltas fabricated, no merkle aggregation
        Err(missing(
            "get_causal_delta_from_parent: causal deltas come from plugin-ledger",
        ))
    }

    pub fn handle_ingress(&self, _options: Object) -> Result<()> {
        // TODO (L3) [wire_plane]: wire-ingress parked connections surface once the wire plane owns the host listeners.; slop: parked connections never surface, listeners silent
        Err(missing(
            "handle_ingress: inbound connections park in the wire plane listeners",
        ))
    }

