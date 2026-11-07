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
    PtyOpenFailed,
    SlabSaturated,
    SlotEmpty,
    InvalidStreamId,
    UnknownFlags,
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
            Self::PtyOpenFailed => write!(f, "pty open failed"),
            Self::SlabSaturated => write!(f, "exec slot slab saturated"),
            Self::SlotEmpty => write!(f, "no exec slot for stream id"),
            Self::InvalidStreamId => write!(f, "stream id exceeds MAX_STREAM_ID"),
            Self::UnknownFlags => write!(f, "frame flags match no lane"),
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
        Err(ShimError::SeccompInstallPending)
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
        Err(ShimError::SeccompNotifyPending)
    }

    pub fn fd(&self) -> Option<i32> {
        self.fd
    }

    pub fn recv_trap(&self) -> Result<SeccompTrap, ShimError> {
        // TODO (L3): SECCOMP_IOCTL_NOTIF_RECV + SECCOMP_IOCTL_NOTIF_SEND over the notify fd.
        let _ = self.fd;
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
