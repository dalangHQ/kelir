// see: cans/11-protocol.md#Control-channel
// see: cans/11-protocol.md#Binary-exec-channel
// see: cans/11-protocol.md#Guest-paravirtual-micro-init-boundary-(kelir-shim)
// see: cans/08-lifecycle.md#Zombie-reaping

use kelir_interface::protocol_wire::{EXEC_STREAM_CONTROL, EXEC_STREAM_STDIN, MsgType};

use crate::codec_protocol;
use crate::trap_protocol::ShimError;

pub const SLAB_CHUNK: usize = 256;
pub const MAX_EXEC_SLOTS: usize = 4096;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Rlimits {
    pub max_as_bytes: Option<u64>,
    pub max_nofile: Option<u64>,
    pub max_nproc: Option<u64>,
    pub max_cpu_seconds: Option<u64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct ExecSpec {
    pub binary: String,
    pub argv: Vec<String>,
    pub limits: Rlimits,
}

impl ExecSpec {
    pub fn parse(payload: &[u8]) -> Result<Self, ShimError> {
        // TODO (L3): parse EXEC_IO_SPEC binary/argv/limits from the open payload.
        let _ = payload;
        Err(ShimError::ExecSpecParsePending)
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct ExecSlot {
    pub pid: libc::pid_t,
    pub exec_id: u64,
    pub stdin_fd: i32,
    pub stdout_fd: i32,
    pub stderr_fd: i32,
}

#[allow(dead_code)]
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

    pub fn signal(&self, signal: u32) -> Result<(), ShimError> {
        if signal == 0 || signal > 64 {
            return Err(ShimError::InvalidSignal { signal });
        }
        // TODO (L3): deliver via pidfd_open/pidfd_send_signal so a recycled pid cannot receive a stale signal.
        // SAFETY: pid is slab-owned from spawn, never wire-supplied; signal is range-checked 1..=64 above.
        let ret = unsafe { libc::kill(self.pid, signal as libc::c_int) };
        if ret == 0 {
            return Ok(());
        }
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        if errno == libc::ESRCH {
            return Ok(());
        }
        Err(ShimError::SignalDelivery {
            exec_id: self.exec_id,
            errno,
        })
    }
}

#[allow(dead_code)]
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

    pub fn get(&self, idx: usize) -> Option<ExecSlot> {
        let chunk = self.chunks.get(idx / SLAB_CHUNK)?;
        chunk[idx % SLAB_CHUNK]
    }

    pub fn find_by_exec_id(&self, exec_id: u64) -> Option<ExecSlot> {
        self.chunks
            .iter()
            .flat_map(|chunk| chunk.iter())
            .find_map(|slot| {
                let slot = slot.as_ref()?;
                if slot.exec_id == exec_id {
                    Some(*slot)
                } else {
                    None
                }
            })
    }

    // TODO (L3): size the pool by min(guest RLIMIT_NPROC, host max_exec_slots from session config).
    pub fn allocate(&mut self, slot: ExecSlot) -> Result<usize, ShimError> {
        let idx = match self.free.pop() {
            Some(recycled) => recycled,
            None => {
                if self.next_fresh >= MAX_EXEC_SLOTS {
                    return Err(ShimError::SlabSaturated);
                }
                let fresh = self.next_fresh;
                self.next_fresh = fresh.saturating_add(1);
                fresh
            }
        };
        while self.chunks.len() <= idx / SLAB_CHUNK {
            self.chunks.push([None; SLAB_CHUNK]);
        }
        self.chunks[idx / SLAB_CHUNK][idx % SLAB_CHUNK] = Some(slot);
        Ok(idx)
    }

    pub fn release(&mut self, idx: usize) {
        let Some(chunk) = self.chunks.get_mut(idx / SLAB_CHUNK) else {
            return;
        };
        if chunk[idx % SLAB_CHUNK].take().is_none() {
            return;
        }
        self.free.push(idx);
        while self
            .chunks
            .last()
            .is_some_and(|c| c.iter().all(|s| s.is_none()))
        {
            self.chunks.pop();
        }
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

#[allow(dead_code)]
pub struct StreamSlotTable {
    entries: Vec<StreamMapEntry>,
}

impl StreamSlotTable {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn bind(&mut self, stream_id: u32, slot: usize) -> Result<(), ShimError> {
        match self
            .entries
            .binary_search_by_key(&stream_id, |e| e.stream_id)
        {
            Ok(_) => Err(ShimError::StreamIdInUse { stream_id }),
            Err(pos) => {
                self.entries.insert(pos, StreamMapEntry { stream_id, slot });
                Ok(())
            }
        }
    }

    pub fn resolve(&self, stream_id: u32) -> Option<usize> {
        let pos = self
            .entries
            .binary_search_by_key(&stream_id, |e| e.stream_id)
            .ok()?;
        Some(self.entries[pos].slot)
    }

    pub fn unbind(&mut self, stream_id: u32) -> Option<usize> {
        let pos = self
            .entries
            .binary_search_by_key(&stream_id, |e| e.stream_id)
            .ok()?;
        Some(self.entries.remove(pos).slot)
    }
}

#[allow(dead_code)]
pub struct Multiplexer {
    pub slots: ExecSlotSlab,
    table: StreamSlotTable,
    pub next_exec_id: u64,
}

impl Multiplexer {
    pub fn new() -> Self {
        Self {
            slots: ExecSlotSlab::new(),
            table: StreamSlotTable::new(),
            next_exec_id: 1,
        }
    }

    pub fn dispatch_control(&mut self, payload: &[u8]) -> Result<(), ShimError> {
        codec_protocol::check_control_payload(payload)
            .map_err(|()| ShimError::ControlOversize { len: payload.len() })?;
        let (raw_msg_type, body) =
            codec_protocol::decode_msg_type(payload).map_err(|()| ShimError::TruncatedFrame)?;
        let msg_type = MsgType::from_u16(raw_msg_type).ok_or(ShimError::UnknownMsgType {
            msg_type: raw_msg_type,
        })?;
        match msg_type {
            MsgType::Trap | MsgType::VmLifecycleControl => {
                // TODO (L3): route control payloads to the trap dispatcher (MSG_TRAP) and lifecycle controller (MSG_VM_LIFECYCLE_CONTROL); control frames never enter the data lanes.
                Err(ShimError::ControlPending {
                    msg_type: msg_type as u16,
                })
            }
            MsgType::Signal => {
                let (signal, exec_id) =
                    codec_protocol::decode_signal(body).map_err(|()| ShimError::TruncatedFrame)?;
                self.signal_exec(exec_id, signal)
            }
            _ => Err(ShimError::UnknownMsgType {
                msg_type: msg_type as u16,
            }),
        }
    }

    pub fn signal_exec(&mut self, exec_id: u64, signal: u32) -> Result<(), ShimError> {
        let Some(slot) = self.slots.find_by_exec_id(exec_id) else {
            return Ok(());
        };
        slot.signal(signal)
    }

    pub fn dispatch_frame(
        &mut self,
        stream_id: u32,
        flags: u8,
        payload: &[u8],
    ) -> Result<(), ShimError> {
        if stream_id > codec_protocol::MAX_STREAM_ID {
            return Err(ShimError::InvalidStreamId { stream_id });
        }
        let (raw_msg_type, body) =
            codec_protocol::decode_msg_type(payload).map_err(|()| ShimError::TruncatedFrame)?;
        let msg_type = MsgType::from_u16(raw_msg_type).ok_or(ShimError::UnknownMsgType {
            msg_type: raw_msg_type,
        })?;
        match msg_type {
            MsgType::Trap | MsgType::VmLifecycleControl => {
                // TODO (L3): route control payloads to the trap dispatcher (MSG_TRAP) and lifecycle controller (MSG_VM_LIFECYCLE_CONTROL); control frames never enter the data lanes.
                return Err(ShimError::ControlPending {
                    msg_type: msg_type as u16,
                });
            }
            MsgType::ChannelPayload | MsgType::SystemTelemetry => {}
            _ => {
                return Err(ShimError::UnknownMsgType {
                    msg_type: msg_type as u16,
                });
            }
        }
        if (flags & codec_protocol::FLAG_OPEN) != 0 {
            return self.open_stream(stream_id, body);
        }
        if (flags & (codec_protocol::FLAG_CLOSE | codec_protocol::FLAG_RST)) != 0 {
            // TODO (L3): flush stdout/stderr EOF frames before release; exit frames defer until EOF flush.
            self.close_stream(stream_id);
            return Ok(());
        }
        let idx = self
            .table
            .resolve(stream_id)
            .ok_or(ShimError::SlotEmpty { stream_id })?;
        if (flags & codec_protocol::FLAG_WINDOW_UPDATE) != 0 {
            // TODO (L3): apply per-stream credit window update to the resolved slot.
            return Ok(());
        }
        if (flags & codec_protocol::FLAG_DATA) != 0 {
            let slot = self
                .slots
                .get(idx)
                .ok_or(ShimError::SlotEmpty { stream_id })?;
            let (lane, data) = codec_protocol::decode_exec_sub_lane(body)
                .map_err(|()| ShimError::TruncatedFrame)?;
            match lane {
                EXEC_STREAM_STDIN => {
                    // TODO (L3): route payload to slot.stdin_fd (exec sub-lanes 0x00/0x03).
                    let _ = (
                        data,
                        slot.pid,
                        slot.exec_id,
                        slot.stdin_fd,
                        slot.stdout_fd,
                        slot.stderr_fd,
                    );
                    return Ok(());
                }
                EXEC_STREAM_CONTROL => {
                    // TODO (L3): apply exec control frames: exit code, EOF, window resize, rlimit descriptors.
                    return Err(ShimError::ExecControlPending);
                }
                _ => return Err(ShimError::UnknownExecSubLane { lane }),
            }
        }
        Err(ShimError::UnknownFlags { flags })
    }

    fn open_stream(&mut self, stream_id: u32, payload: &[u8]) -> Result<(), ShimError> {
        if self.table.resolve(stream_id).is_some() {
            return Err(ShimError::StreamIdInUse { stream_id });
        }
        let spec = ExecSpec::parse(payload)?;
        let slot = self.spawn_exec(&spec)?;
        let idx = self.slots.allocate(slot)?;
        if let Err(err) = self.table.bind(stream_id, idx) {
            self.slots.release(idx);
            return Err(err);
        }
        Ok(())
    }

    fn close_stream(&mut self, stream_id: u32) {
        if let Some(idx) = self.table.unbind(stream_id) {
            self.slots.release(idx);
        }
    }

    fn spawn_exec(&mut self, spec: &ExecSpec) -> Result<ExecSlot, ShimError> {
        // TODO (L3): openpty via crate::pty_terminal::open_pty_slave, fork, setrlimit from spec.limits between fork and execve, execve spec.binary/spec.argv; the child is reaped by the main.rs waitpid loop.
        let _ = (spec, self.next_exec_id);
        Err(ShimError::SpawnPending)
    }
}

impl Default for Multiplexer {
    fn default() -> Self {
        Self::new()
    }
}
