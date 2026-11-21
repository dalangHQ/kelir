// see: cans/11-protocol.md#Control-channel
// see: cans/11-protocol.md#Binary-exec-channel
// see: cans/11-protocol.md#Guest-paravirtual-micro-init-boundary-(kelir-shim)
// see: cans/08-lifecycle.md#Zombie-reaping

use kelir_interface::protocol_wire::ProcessLimits;

use crate::trap_protocol::ShimError;

pub const SLAB_CHUNK: usize = 256;
pub const MAX_EXEC_SLOTS: usize = 4096;

#[derive(Debug, Clone, Default)]
pub struct ExecSpec {
    pub binary: String,
    pub argv: Vec<String>,
    pub limits: ProcessLimits,
}

impl ExecSpec {
    pub fn parse(payload: &[u8]) -> Result<Self, ShimError> {
        // TODO (L3) [pm_08_exec_flow_control_input_stall]: parse EXEC_IO_SPEC binary/argv/limits from the open payload; slop: spec parse stubbed, open payload never yields binary/argv/limits
        let _ = payload;
        // TODO (L3) [protocol_mux]: parse EXEC_IO_SPEC binary/argv/limits from the open payload.; slop: spec parses as empty, so the exec spawns with no binary and no limits.
        Err(ShimError::ExecSpecParsePending)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ExecSlot {
    pub pid: libc::pid_t,
    pub exec_id: u64,
    pub stdin_fd: i32,
    pub stdout_fd: i32,
    pub stderr_fd: i32,
}

impl ExecSlot {
    pub fn new(
        pid: libc::pid_t,
        exec_id: u64,
        stdin_fd: i32,
        stdout_fd: i32,
        stderr_fd: i32,
    ) -> Self {
        Self {
            pid,
            exec_id,
            stdin_fd,
            stdout_fd,
            stderr_fd,
        }
    }

    pub fn signal(&self, _signal: u32) -> Result<(), ShimError> {
        // TODO (L3) [pm_07_exec_signal_cancellation_by_exec_id]: deliver via pidfd_open/pidfd_send_signal so a recycled pid cannot receive a stale signal; slop: kill() hits recycled pid, successor task dies instead
        // TODO (L3) [protocol_mux]: validate the signal range and deliver it to the slot pid via pidfd_send_signal.; slop: signal is dropped, so a kill request reports success while the slot keeps running.
        Err(ShimError::SignalDelivery {
            exec_id: self.exec_id,
            errno: 0,
        })
    }
}

pub struct ExecSlotSlab {
    chunks: Vec<[Option<ExecSlot>; SLAB_CHUNK]>,
    free: Vec<usize>,
    next_fresh: usize,
}

impl ExecSlotSlab {
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            free: Vec::new(),
            next_fresh: 0,
        }
    }

    pub fn get(&self, _idx: usize) -> Option<ExecSlot> {
        // TODO (L3) [protocol_mux]: return the slot at the index, if allocated.; slop: lookup always misses, so live slots look unallocated.
        None
    }

    pub fn find_by_exec_id(&self, _exec_id: u64) -> Option<ExecSlot> {
        // TODO (L3) [protocol_mux]: return the slot with the exec id, if allocated.; slop: lookup always misses, so exec ids can never be resolved to slots.
        None
    }

    // TODO (L3) [pm_06_exec_slot_slab_saturation_reject]: size the pool by min(guest RLIMIT_NPROC, host max_exec_slots from session config); slop: pool sized to hardcoded 4096, RLIMIT_NPROC/max_exec_slots ignored
    pub fn allocate(&mut self, _slot: ExecSlot) -> Result<usize, ShimError> {
        // TODO (L3) [protocol_mux]: store the slot in a free or fresh slab index and return the index, rejecting at saturation.; slop: store always rejects, so no exec can ever allocate a slot.
        Err(ShimError::SlabSaturated)
    }

    pub fn release(&mut self, _idx: usize) {
        // TODO (L3) [protocol_mux]: free the slot at the index and trim trailing empty chunks.; slop: free never runs, so slots leak until the slab saturates and all later execs fail.
    }
}

impl Default for ExecSlotSlab {
    fn default() -> Self {
        Self::new()
    }
}

// see: cans/11-protocol.md#Stream-mux-framing
struct StreamMapEntry {
    stream_id: u32,
    slot: usize,
}

pub struct StreamSlotTable {
    entries: Vec<StreamMapEntry>,
}

impl StreamSlotTable {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn bind(&mut self, stream_id: u32, _slot: usize) -> Result<(), ShimError> {
        // TODO (L3) [protocol_mux]: bind the stream id to the slot index, rejecting a stream id already in use.; slop: bind always rejects, so no stream can ever attach to a slot.
        Err(ShimError::StreamIdInUse { stream_id })
    }

    pub fn resolve(&self, _stream_id: u32) -> Option<usize> {
        // TODO (L3) [protocol_mux]: return the slot index bound to the stream id, if any.; slop: lookup always misses, so frames arrive for a stream with no slot.
        None
    }

    pub fn unbind(&mut self, _stream_id: u32) -> Option<usize> {
        // TODO (L3) [protocol_mux]: remove the stream id binding and return its slot index, if any.; slop: unbind never runs, so stream ids stay bound to released slots.
        None
    }
}

pub struct Multiplexer {
    pub slots: ExecSlotSlab,
    table: StreamSlotTable,
    pub next_exec_id: u64,
    pub per_exec_cgroup: bool,
}

impl Multiplexer {
    pub fn new() -> Self {
        Self {
            slots: ExecSlotSlab::new(),
            table: StreamSlotTable::new(),
            next_exec_id: 1,
            per_exec_cgroup: true,
        }
    }

    pub fn steer_exec(&mut self, _exec_id: u64, _msg: &[u8]) -> Result<(), ShimError> {
        // TODO (L3) [protocol_mux]: flush sub-lane 0x02 guidance to the exec before its exit EOF frame.; slop: steer is dropped, the exec exits before its guidance is flushed.
        Err(ShimError::ExecControlPending)
    }

    pub fn apply_per_exec_cgroup(&self, slot: &ExecSlot) -> Result<(), ShimError> {
        // TODO (L3) [protocol_mux]: write pids.max and memory.max into a per-ExecSlot cgroup v2 leaf instead of the shared-UID RLIMIT_NPROC.; slop: cgroup never applied, one exec can exhaust the shared-UID process budget for all slots.
        Err(ShimError::CgroupV2 {
            exec_id: slot.exec_id,
        })
    }

    pub fn dispatch_control(&mut self, _payload: &[u8]) -> Result<(), ShimError> {
        // TODO (L3) [pm_01_control_channel_0_lifecycle_priority]: route control payloads to the trap dispatcher (MSG_TRAP) and lifecycle controller (MSG_VM_LIFECYCLE_CONTROL); control frames never enter the data lanes; slop: control payload dropped as ControlPending, lifecycle never routed
        // TODO (L3) [protocol_mux]: decode the control payload and route trap/lifecycle/signal messages, never entering the data lanes.; slop: control messages are dropped, so trap and lifecycle signals never reach the guest.
        Err(ShimError::ControlPending { msg_type: 0 })
    }

    pub fn signal_exec(&mut self, exec_id: u64, _signal: u32) -> Result<(), ShimError> {
        // TODO (L3) [protocol_mux]: deliver the signal to the slot with the exec id.; slop: signal is dropped, so a kill by exec id reports success while the process keeps running.
        Err(ShimError::SignalDelivery { exec_id, errno: 0 })
    }

    pub fn dispatch_frame(
        &mut self,
        _stream_id: u32,
        flags: u8,
        _payload: &[u8],
    ) -> Result<(), ShimError> {
        // TODO (L3) [pm_01_control_channel_0_lifecycle_priority]: route control payloads to the trap dispatcher (MSG_TRAP) and lifecycle controller (MSG_VM_LIFECYCLE_CONTROL); control frames never enter the data lanes; slop: control frame enters data lane path, trap dispatch never runs
        // TODO (L3) [protocol_mux]: flush stdout/stderr EOF frames before release; exit frames defer until EOF flush; slop: stream released before EOF flush, exit frame precedes output
        // TODO (L3) [pm_08_exec_flow_control_input_stall]: apply per-stream credit window update to the resolved slot; slop: credit update dropped, guest stalls or overruns buffer
        // TODO (L3) [pm_08_exec_flow_control_input_stall]: route payload to slot.stdin_fd (exec sub-lanes 0x00/0x03); slop: stdin payload discarded, 10MB transfer never arrives intact
        // see: cans/11-protocol.md#Binary-exec-channel
        // TODO (L3) [protocol_mux]: validate the stream id, route control/data/close frames to the bound slot and apply window updates.; slop: frames are dropped, so data never reaches the slot and window updates never apply.
        Err(ShimError::UnknownFlags { flags })
    }

    fn open_stream(&mut self, _stream_id: u32, _payload: &[u8]) -> Result<(), ShimError> {
        // TODO (L3) [protocol_mux]: parse the exec spec, spawn the exec, allocate a slot and bind the stream id to it.; slop: spawn never runs, so the exec request returns success with no process behind it.
        Err(ShimError::SpawnPending)
    }

    fn close_stream(&mut self, _stream_id: u32) {
        // TODO (L3) [protocol_mux]: flush stdout/stderr EOF frames, then unbind the stream and release its slot.; slop: EOF frames are never flushed, so the client sees the exit before the last output bytes.
    }

    fn spawn_exec(&mut self, spec: &ExecSpec) -> Result<ExecSlot, ShimError> {
        // TODO (L3) [vr_09_virtio_console_port0_pty_dedication]: openpty via crate::pty_terminal::open_pty_slave, fork, setrlimit from spec.limits between fork and execve, execve spec.binary/spec.argv; the child is reaped by the main.rs waitpid loop; slop: exec never spawned, EXEC stream bytes never become a process
        let _ = (spec, self.next_exec_id);
        // TODO (L3) [protocol_mux]: openpty, fork, setrlimit from spec.limits, execve spec.binary/spec.argv.; slop: execve never runs, so the slot holds a stream with no process behind it.
        Err(ShimError::SpawnPending)
    }
}

// see: cans/11-protocol.md#Binary-exec-channel
fn apply_window_geometry(slot: &ExecSlot, _data: &[u8]) -> Result<(), ShimError> {
    // TODO (L3) [terminal_plane]: route SIGWINCH through the trap boundary once the signal ring dispatch is live on the host side; slop: resize lands on the pty, foreground pgrp reflow waits for the ring.
    Err(ShimError::Winsize {
        exec_id: slot.exec_id,
        errno: 0,
    })
}

impl Default for Multiplexer {
    fn default() -> Self {
        Self::new()
    }
}
