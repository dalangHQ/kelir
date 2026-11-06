// see: cans/11-protocol.md#Control-channel
// see: cans/11-protocol.md#Binary-exec-channel

use crate::codec;

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
pub struct Multiplexer {
    pub slots: [Option<ExecSlot>; 4096],
}

impl Multiplexer {
    pub fn new() -> Self {
        Self {
            slots: [None; 4096],
        }
    }

    #[allow(dead_code)]
    pub fn dispatch_frame(&mut self, stream_id: u32, flags: u8, payload: &[u8]) -> Result<(), ()> {
        let idx = usize::try_from(stream_id).map_err(|_| ())?;
        if idx >= self.slots.len() {
            return Err(());
        }
        if (flags & codec::FLAG_OPEN) != 0 {
            // see: cans/11-protocol.md#Binary-exec-channel
            self.slots[idx] = Some(ExecSlot {
                pid: -1,
                exec_id: 0,
                stdin_fd: -1,
                stdout_fd: -1,
                stderr_fd: -1,
            });
            return Ok(());
        }
        if (flags & (codec::FLAG_CLOSE | codec::FLAG_RST)) != 0 {
            // see: cans/11-protocol.md#Binary-exec-channel
            self.slots[idx] = None;
            return Ok(());
        }
        if (flags & codec::FLAG_WINDOW_UPDATE) != 0 {
            // see: cans/11-protocol.md#Binary-exec-channel
            return Ok(());
        }
        if (flags & codec::FLAG_DATA) != 0 {
            let slot = match self.slots[idx] {
                Some(slot) => slot,
                None => return Err(()),
            };
            // see: cans/11-protocol.md#Binary-exec-channel
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
        Err(())
    }
}

impl Default for Multiplexer {
    fn default() -> Self {
        Self::new()
    }
}
