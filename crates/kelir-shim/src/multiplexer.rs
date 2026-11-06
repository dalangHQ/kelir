#![forbid(unsafe_code)]

// see: cans/11-protocol.md#Control-channel
// see: cans/11-protocol.md#Binary-exec-channel

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
        let _ = (stream_id, flags, payload);
        // TODO: route frame to ExecSlot by stream_id (L3).
        Err(())
    }
}

impl Default for Multiplexer {
    fn default() -> Self {
        Self::new()
    }
}
