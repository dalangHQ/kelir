// see: cans/11-protocol.md#Guest-paravirtual-micro-init-boundary-(kelir-shim)
// see: cans/artifacts/diagrams/trap-flow.txt
use std::fmt;

#[allow(dead_code)]
pub const TRAP_CHANNEL: u16 = 0x0000;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShimError {
    SeccompInstallPending,
    SeccompNotifyPending,
    SpawnPending,
    ExecSpecParsePending,
    SlabSaturated,
    SlotEmpty { stream_id: u32 },
    InvalidStreamId { stream_id: u32 },
    StreamIdInUse { stream_id: u32 },
    UnknownFlags { flags: u8 },
    UnknownMsgType { msg_type: u16 },
    TruncatedFrame,
    ControlPending { msg_type: u16 },
    ControlOversize { len: usize },
    UnknownExecSubLane { lane: u8 },
    ExecControlPending,
    InvalidSignal { signal: u32 },
    SignalDelivery { exec_id: u64, errno: i32 },
    PtyOpen { errno: libc::c_int },
    ProcRead { pid: u32, field: &'static str },
    ExeUnresolved { pid: u32 },
    ElfParse { path: String },
    ExeDigest { path: String },
    NotifyPoll { errno: i32 },
    TrapEncode,
}

impl fmt::Display for ShimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SeccompInstallPending => {
                write!(f, "seccomp filter install pending (L3)")
            }
            Self::SeccompNotifyPending => {
                write!(f, "seccomp-unotify fd pending (L3)")
            }
            Self::SpawnPending => {
                write!(
                    f,
                    "exec spawn via openpty/fork/setrlimit/execve pending (L3)"
                )
            }
            Self::ExecSpecParsePending => {
                write!(f, "EXEC_IO_SPEC parse pending (L3)")
            }
            Self::SlabSaturated => write!(f, "exec slot slab saturated"),
            Self::SlotEmpty { stream_id } => {
                write!(f, "no exec slot mapped for stream id {stream_id}")
            }
            Self::InvalidStreamId { stream_id } => {
                write!(f, "stream id {stream_id} exceeds MAX_STREAM_ID")
            }
            Self::StreamIdInUse { stream_id } => {
                write!(
                    f,
                    "stream id {stream_id} already bound to an active exec slot"
                )
            }
            Self::UnknownFlags { flags } => {
                write!(f, "frame flags {flags:#04x} match no lane")
            }
            Self::UnknownMsgType { msg_type } => {
                write!(f, "frame msg_type {msg_type:#06x} is unknown")
            }
            Self::TruncatedFrame => {
                write!(f, "frame payload shorter than the 2-byte msg_type prefix")
            }
            Self::ControlPending { msg_type } => {
                write!(f, "control msg_type {msg_type:#06x} handling pending (L3)")
            }
            Self::ControlOversize { len } => {
                write!(
                    f,
                    "control payload {len} bytes exceeds the 4KB control ceiling"
                )
            }
            Self::UnknownExecSubLane { lane } => {
                write!(f, "exec sub-lane {lane:#04x} is unknown on the data path")
            }
            Self::ExecControlPending => {
                write!(f, "exec control sub-lane handling pending (L3)")
            }
            Self::InvalidSignal { signal } => {
                write!(f, "signal {signal} is outside the deliverable range 1..=64")
            }
            Self::SignalDelivery { exec_id, errno } => {
                write!(
                    f,
                    "signal delivery to exec_id {exec_id} failed with errno {errno}"
                )
            }
            Self::PtyOpen { errno } => write!(f, "openpty failed with errno {errno}"),
            Self::ProcRead { pid, field } => {
                write!(f, "cannot read /proc/{pid}/{field}")
            }
            Self::ExeUnresolved { pid } => {
                write!(f, "cannot resolve /proc/{pid}/exe")
            }
            Self::ElfParse { path } => {
                write!(f, "cannot parse ELF headers of {path}")
            }
            Self::ExeDigest { path } => {
                write!(f, "cannot digest executable {path}")
            }
            Self::NotifyPoll { errno } => {
                write!(f, "seccomp-unotify poll failed with errno {errno}")
            }
            Self::TrapEncode => write!(f, "cannot encode MSG_TRAP frame"),
        }
    }
}

impl std::error::Error for ShimError {}

#[allow(dead_code)]
pub struct SeccompFilter {
    fd: Option<i32>,
}

#[allow(dead_code)]
impl SeccompFilter {
    pub fn install() -> Result<Self, ShimError> {
        // TODO (L3): seccomp(SECCOMP_SET_MODE_FILTER) with the trap filter program.
        Ok(Self { fd: None })
    }

    pub fn fd(&self) -> Option<i32> {
        self.fd
    }
}

#[allow(dead_code)]
pub struct SeccompNotify {
    fd: Option<i32>,
}

#[allow(dead_code)]
impl SeccompNotify {
    pub fn install(_filter: &SeccompFilter) -> Result<Self, ShimError> {
        // TODO (L3): SECCOMP_IOCTL_NOTIF_RECV fd from the installed filter.
        Ok(Self { fd: None })
    }

    pub fn fd(&self) -> Option<i32> {
        self.fd
    }

    pub fn poll_ready(&self) -> Result<bool, ShimError> {
        let Some(fd) = self.fd else {
            return Ok(false);
        };
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: pfd is a valid single-entry pollfd array; timeout 0 never blocks.
        let rc = unsafe { libc::poll(&mut pfd, 1, 0) };
        if rc < 0 {
            return Err(ShimError::NotifyPoll {
                errno: std::io::Error::last_os_error().raw_os_error().unwrap_or(0),
            });
        }
        Ok(rc > 0 && pfd.revents & libc::POLLIN != 0)
    }

    pub fn dispatch_pending(
        &self,
        seq: u64,
    ) -> Result<Option<crate::introspect::TrapInspection>, ShimError> {
        if !self.poll_ready()? {
            return Ok(None);
        }
        let trap = self.recv_trap()?;
        Ok(Some(crate::introspect::inspect_trap(&trap, seq)?))
    }

    pub fn recv_trap(&self) -> Result<SeccompTrap, ShimError> {
        if self.fd.is_none() {
            return Err(ShimError::SeccompNotifyPending);
        }
        // TODO (L3): SECCOMP_IOCTL_NOTIF_RECV + SECCOMP_IOCTL_NOTIF_SEND over the notify fd.
        Err(ShimError::SeccompNotifyPending)
    }
}

#[allow(dead_code)]
pub struct SeccompTrap {
    pub pid: u32,
    pub syscall_nr: i32,
    pub args: [u64; 6],
}

#[allow(dead_code)]
impl SeccompTrap {
    pub fn route_channel(&self) -> u16 {
        TRAP_CHANNEL
    }
}

pub fn init_seccomp_unotify() -> Result<SeccompNotify, ShimError> {
    let filter = SeccompFilter::install()?;
    SeccompNotify::install(&filter)
}
