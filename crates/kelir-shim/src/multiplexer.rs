// see: cans/11-protocol.md#Control-channel
// see: cans/11-protocol.md#Binary-exec-channel
// see: cans/08-lifecycle.md#Zombie-reaping

use crate::codec;
use crate::trap::ShimError;

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
}

#[allow(dead_code)]
pub struct ExecSlotSlab {
    chunks: Vec<[Option<ExecSlot>; SLAB_CHUNK]>,
}

impl ExecSlotSlab {
    pub fn new() -> Self {
        Self { chunks: Vec::new() }
    }

    pub fn get(&self, idx: usize) -> Option<ExecSlot> {
        let chunk = self.chunks.get(idx / SLAB_CHUNK)?;
        chunk[idx % SLAB_CHUNK]
    }

    pub fn insert(&mut self, idx: usize, slot: ExecSlot) -> Result<(), ShimError> {
        if idx >= MAX_EXEC_SLOTS {
            return Err(ShimError::SlabSaturated);
        }
        while self.chunks.len() <= idx / SLAB_CHUNK {
            self.chunks.push([None; SLAB_CHUNK]);
        }
        self.chunks[idx / SLAB_CHUNK][idx % SLAB_CHUNK] = Some(slot);
        Ok(())
    }

    pub fn remove(&mut self, idx: usize) {
        if let Some(chunk) = self.chunks.get_mut(idx / SLAB_CHUNK) {
            chunk[idx % SLAB_CHUNK] = None;
        }
    }
}

impl Default for ExecSlotSlab {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
pub struct Multiplexer {
    pub slots: ExecSlotSlab,
    pub next_exec_id: u64,
}

impl Multiplexer {
    pub fn new() -> Self {
        Self {
            slots: ExecSlotSlab::new(),
            next_exec_id: 1,
        }
    }

    #[allow(dead_code)]
    pub fn dispatch_frame(
        &mut self,
        stream_id: u32,
        flags: u8,
        payload: &[u8],
    ) -> Result<(), ShimError> {
        let idx = usize::try_from(stream_id).map_err(|_| ShimError::InvalidStreamId)?;
        if idx >= MAX_EXEC_SLOTS {
            return Err(ShimError::SlabSaturated);
        }
        if (flags & codec::FLAG_OPEN) != 0 {
            let spec = ExecSpec::parse(payload)?;
            let slot = self.spawn_exec(&spec)?;
            self.slots.insert(idx, slot)?;
            return Ok(());
        }
        if (flags & (codec::FLAG_CLOSE | codec::FLAG_RST)) != 0 {
            // TODO (L3): flush stdout/stderr EOF frames, release slab chunk
            self.slots.remove(idx);
            return Ok(());
        }
        if (flags & codec::FLAG_WINDOW_UPDATE) != 0 {
            // TODO: apply stream credit window update
            return Ok(());
        }
        if (flags & codec::FLAG_DATA) != 0 {
            let slot = self.slots.get(idx).ok_or(ShimError::SlotEmpty)?;
            // TODO: route payload to slot.stdin_fd (exec sub-lanes 0x00/0x03)
            let _ = (
                payload,
                slot.pid,
                slot.exec_id,
                slot.stdin_fd,
                slot.stdout_fd,
                slot.stderr_fd,
            );
            return Ok(());
        }
        Err(ShimError::UnknownFlags)
    }

    fn spawn_exec(&mut self, spec: &ExecSpec) -> Result<ExecSlot, ShimError> {
        // TODO (L3): openpty via crate::pty::open_pty_slave, fork, setrlimit from spec.limits between fork and execve, execve spec.binary/spec.argv; the child is reaped by the main.rs waitpid loop.
        let _ = (spec, self.next_exec_id);
        Err(ShimError::SpawnPending)
    }
}

impl Default for Multiplexer {
    fn default() -> Self {
        Self::new()
    }
}
