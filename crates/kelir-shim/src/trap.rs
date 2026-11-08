// see: cans/11-protocol.md#Guest-paravirtual-micro-init-boundary-(kelir-shim)
// see: cans/11-protocol.md#Trap-fast-path
// see: cans/artifacts/diagrams/trap-flow.txt#Kernel-seccomp-unotify-triggers
use std::fmt;

#[allow(dead_code)]
pub const TRAP_CHANNEL: u16 = 0x0000;

// see: cans/11-protocol.md#Trap-fast-path
const SECCOMP_DATA_NR_OFFSET: u32 = 0;

// see: cans/11-protocol.md#Trap-fast-path
const SECCOMP_DATA_ARCH_OFFSET: u32 = 4;

// see: cans/11-protocol.md#Trap-fast-path
#[cfg(target_arch = "x86_64")]
const AUDIT_ARCH: u32 = 0xC000_003E;

// see: cans/11-protocol.md#Trap-fast-path
#[cfg(target_arch = "aarch64")]
const AUDIT_ARCH: u32 = 0xC000_00B7;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShimError {
    SeccompInstall { errno: libc::c_int },
    NotifyRecv { errno: libc::c_int },
    NotifyPoll { errno: i32 },
    Poll { errno: libc::c_int },
    ChannelOpen { channel: u16, errno: libc::c_int },
    ChannelConnect { channel: u16, errno: libc::c_int },
    ChannelIo { channel: u16, errno: libc::c_int },
    ChannelEof { channel: u16 },
    FreezerIo { path: String, errno: libc::c_int },
    FreezerTimeout { path: String },
    FreezerRoot { pid: u32 },
    FreezerUnsupported { pid: u32 },
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
    TrapEncode,
}

impl fmt::Display for ShimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SeccompInstall { errno } => {
                write!(f, "seccomp filter install failed with errno {errno}")
            }
            Self::NotifyRecv { errno } => {
                write!(f, "seccomp-unotify recv failed with errno {errno}")
            }
            Self::NotifyPoll { errno } => {
                write!(f, "seccomp-unotify poll failed with errno {errno}")
            }
            Self::Poll { errno } => write!(f, "event poll failed with errno {errno}"),
            Self::ChannelOpen { channel, errno } => {
                write!(
                    f,
                    "channel {channel:#06x} socket open failed with errno {errno}"
                )
            }
            Self::ChannelConnect { channel, errno } => {
                write!(
                    f,
                    "channel {channel:#06x} connect failed with errno {errno}"
                )
            }
            Self::ChannelIo { channel, errno } => {
                write!(f, "channel {channel:#06x} io failed with errno {errno}")
            }
            Self::ChannelEof { channel } => {
                write!(
                    f,
                    "channel {channel:#06x} closed; channel close is terminal"
                )
            }
            Self::FreezerIo { path, errno } => {
                write!(f, "cgroup freezer {path} failed with errno {errno}")
            }
            Self::FreezerTimeout { path } => {
                write!(f, "cgroup {path} never reached the frozen state")
            }
            Self::FreezerRoot { pid } => {
                write!(
                    f,
                    "pid {pid} lives in the root task group; freezing it would deadlock PID 1"
                )
            }
            Self::FreezerUnsupported { pid } => {
                write!(
                    f,
                    "pid {pid} has no cgroup v2 entry; no freezer protection exists"
                )
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
            Self::TrapEncode => write!(f, "cannot encode MSG_TRAP frame"),
        }
    }
}

impl std::error::Error for ShimError {}

// see: cans/11-protocol.md#Trap-fast-path
fn bpf_stmt(code: u32, k: u32) -> libc::sock_filter {
    libc::sock_filter {
        code: code as u16,
        jt: 0,
        jf: 0,
        k,
    }
}

// see: cans/11-protocol.md#Trap-fast-path
fn bpf_jump(code: u32, k: u32, jt: u8, jf: u8) -> libc::sock_filter {
    libc::sock_filter {
        code: code as u16,
        jt,
        jf,
        k,
    }
}

// see: cans/11-protocol.md#Trap-fast-path
fn trap_filter_program() -> Vec<libc::sock_filter> {
    // TODO (L3): gate-1 in-kernel allowlist fast path for harmless tools plus in-filter mprotect(PROT_EXEC)/memfd_create flag masking.
    let monitored = [
        libc::SYS_execve,
        libc::SYS_execveat,
        libc::SYS_mprotect,
        libc::SYS_memfd_create,
    ];
    let mut prog = vec![
        bpf_stmt(
            libc::BPF_LD | libc::BPF_W | libc::BPF_ABS,
            SECCOMP_DATA_ARCH_OFFSET,
        ),
        bpf_jump(
            libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K,
            AUDIT_ARCH,
            1,
            0,
        ),
        bpf_stmt(libc::BPF_RET | libc::BPF_K, libc::SECCOMP_RET_KILL_PROCESS),
        bpf_stmt(
            libc::BPF_LD | libc::BPF_W | libc::BPF_ABS,
            SECCOMP_DATA_NR_OFFSET,
        ),
    ];
    let checks = monitored.len();
    for (index, nr) in monitored.iter().enumerate() {
        prog.push(bpf_jump(
            libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K,
            *nr as u32,
            (checks - index) as u8,
            0,
        ));
    }
    prog.push(bpf_stmt(
        libc::BPF_RET | libc::BPF_K,
        libc::SECCOMP_RET_ALLOW,
    ));
    prog.push(bpf_stmt(
        libc::BPF_RET | libc::BPF_K,
        libc::SECCOMP_RET_USER_NOTIF,
    ));
    prog
}

#[allow(dead_code)]
pub struct SeccompFilter {
    fd: libc::c_int,
}

#[allow(dead_code)]
impl SeccompFilter {
    // see: cans/11-protocol.md#Trap-fast-path
    pub fn install() -> Result<Self, ShimError> {
        let mut filter = trap_filter_program();
        let fprog = libc::sock_fprog {
            len: filter.len() as libc::c_ushort,
            filter: filter.as_mut_ptr(),
        };
        // SAFETY: fprog points at the live filter vector and seccomp(2) copies the program before returning; the returned c_long is the listener fd or -1.
        let rc = unsafe {
            libc::syscall(
                libc::SYS_seccomp,
                libc::SECCOMP_SET_MODE_FILTER as libc::c_long,
                libc::SECCOMP_FILTER_FLAG_NEW_LISTENER as libc::c_long,
                &fprog,
            )
        };
        if rc < 0 {
            return Err(ShimError::SeccompInstall {
                errno: std::io::Error::last_os_error().raw_os_error().unwrap_or(0),
            });
        }
        Ok(Self {
            fd: rc as libc::c_int,
        })
    }

    pub fn fd(&self) -> libc::c_int {
        self.fd
    }
}

#[allow(dead_code)]
pub struct SeccompNotify {
    fd: libc::c_int,
}

#[allow(dead_code)]
impl SeccompNotify {
    // see: cans/11-protocol.md#Trap-fast-path
    pub fn install(filter: &SeccompFilter) -> Result<Self, ShimError> {
        Ok(Self { fd: filter.fd() })
    }

    pub fn fd(&self) -> libc::c_int {
        self.fd
    }

    pub fn poll_ready(&self) -> Result<bool, ShimError> {
        let mut pfd = libc::pollfd {
            fd: self.fd,
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

    // see: cans/11-protocol.md#Trap-fast-path
    pub fn recv_trap(&self) -> Result<SeccompTrap, ShimError> {
        // SAFETY: a zeroed seccomp_notif is all-integer valid; NOTIF_RECV blocks only when no notification is pending, which dispatch_pending rules out via POLLIN.
        let trap = unsafe {
            let mut notif: libc::seccomp_notif = std::mem::zeroed();
            let rc = libc::ioctl(self.fd, libc::SECCOMP_IOCTL_NOTIF_RECV, &mut notif);
            if rc != 0 {
                return Err(ShimError::NotifyRecv {
                    errno: std::io::Error::last_os_error().raw_os_error().unwrap_or(0),
                });
            }
            SeccompTrap {
                pid: notif.pid,
                syscall_nr: notif.data.nr,
                args: notif.data.args,
            }
        };
        Ok(trap)
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

// see: cans/11-protocol.md#Trap-fast-path
pub fn init_seccomp_unotify() -> Result<SeccompNotify, ShimError> {
    let filter = SeccompFilter::install()?;
    SeccompNotify::install(&filter)
}
