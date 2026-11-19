// see: cans/11-protocol.md#Guest-paravirtual-micro-init-boundary-(kelir-shim)
// see: cans/11-protocol.md#Trap-fast-path
// see: cans/artifacts/diagrams/trap-flow.txt#Kernel-seccomp-unotify-triggers
use std::fmt;

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
    NotifySend { errno: libc::c_int },
    Winsize { exec_id: u64, errno: libc::c_int },
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
            Self::NotifySend { errno } => {
                write!(f, "seccomp-unotify send failed with errno {errno}")
            }
            Self::Winsize { exec_id, errno } => {
                write!(
                    f,
                    "window geometry update for exec_id {exec_id} failed with errno {errno}"
                )
            }
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

// see: cans/01-architecture.md#Shim-defense-flags-close-in-guest-kernel-pseudo-filesystem-leaks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefenseFlags {
    pub mask_proc_environ: bool,
    pub block_abstract_unix_sockets: bool,
}

impl Default for DefenseFlags {
    fn default() -> Self {
        Self {
            mask_proc_environ: true,
            block_abstract_unix_sockets: true,
        }
    }
}

// see: cans/11-protocol.md#Guest-paravirtual-micro-init-boundary-(kelir-shim)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapVerdict {
    Pass,
    Deny { errno: libc::c_int },
}

// see: cans/11-protocol.md#Blocks-direct-memory-reads-against-/proc/[pid]/environ-and-process-vm-readv
pub fn environ_open(path: &[u8]) -> bool {
    path.starts_with(b"/proc") && path.windows(7).any(|window| window == b"environ")
}

// see: cans/07-wire-plane.md#Abstract-socket-boundary:-wire.block-abstract-unix-sockets-denies-\0-paths
pub fn abstract_unix_addr(addr: &[u8]) -> bool {
    if addr.len() < 3 {
        return false;
    }
    let family = u16::from_ne_bytes([addr[0], addr[1]]);
    family == libc::AF_UNIX as u16 && addr[2] == 0
}

// see: cans/01-architecture.md#Shim-defense-flags-close-in-guest-kernel-pseudo-filesystem-leaks
pub fn defense_verdict(
    syscall_nr: i64,
    defense: &DefenseFlags,
    pathname: Option<&[u8]>,
    sockaddr: Option<&[u8]>,
) -> TrapVerdict {
    if defense.mask_proc_environ && is_openat_nr(syscall_nr) {
        return match pathname {
            Some(path) if environ_open(path) => TrapVerdict::Deny {
                errno: libc::EACCES,
            },
            _ => TrapVerdict::Pass,
        };
    }
    if defense.block_abstract_unix_sockets && is_unix_socket_nr(syscall_nr) {
        return match sockaddr {
            Some(addr) if abstract_unix_addr(addr) => TrapVerdict::Deny {
                errno: libc::EAFNOSUPPORT,
            },
            _ => TrapVerdict::Pass,
        };
    }
    TrapVerdict::Pass
}

// see: cans/11-protocol.md#Blocks-direct-memory-reads-against-/proc/[pid]/environ-and-process-vm-readv
fn is_openat_nr(syscall_nr: i64) -> bool {
    syscall_nr == libc::SYS_openat || syscall_nr == libc::SYS_openat2
}

// see: cans/07-wire-plane.md#Abstract-socket-boundary:-wire.block-abstract-unix-sockets-denies-\0-paths
fn is_unix_socket_nr(syscall_nr: i64) -> bool {
    syscall_nr == libc::SYS_connect || syscall_nr == libc::SYS_bind
}

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
fn trap_filter_program(defense: &DefenseFlags) -> Vec<libc::sock_filter> {
    // L3-GAP: [hd_02_wx_mmu_prot_exec_trap]: gate-1 in-kernel allowlist fast path for harmless tools plus in-filter mprotect(PROT_EXEC)/memfd_create flag masking; slop: allowlist passes mprotect PROT_EXEC, Stage-2 fault never fires
    // see: cans/01-architecture.md#Shim-defense-flags-close-in-guest-kernel-pseudo-filesystem-leaks
    let mut monitored = vec![
        libc::SYS_execve,
        libc::SYS_execveat,
        libc::SYS_mprotect,
        libc::SYS_memfd_create,
        libc::SYS_kill,
        libc::SYS_tgkill,
        libc::SYS_tkill,
        libc::SYS_clone,
        libc::SYS_clone3,
        libc::SYS_connect,
        libc::SYS_bind,
    ];
    #[cfg(not(target_arch = "aarch64"))]
    monitored.extend_from_slice(&[libc::SYS_fork, libc::SYS_vfork]);
    if defense.mask_proc_environ {
        monitored.extend_from_slice(&[libc::SYS_openat, libc::SYS_openat2]);
    }
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
    let deny_tail: u8 = if defense.mask_proc_environ { 2 } else { 0 };
    for (index, nr) in monitored.iter().enumerate() {
        prog.push(bpf_jump(
            libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K,
            *nr as u32,
            (checks - index) as u8 + deny_tail,
            0,
        ));
    }
    if defense.mask_proc_environ {
        // see: cans/11-protocol.md#Blocks-direct-memory-reads-against-/proc/[pid]/environ-and-process-vm-readv
        prog.push(bpf_jump(
            libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K,
            libc::SYS_process_vm_readv as u32,
            0,
            1,
        ));
        prog.push(bpf_stmt(
            libc::BPF_RET | libc::BPF_K,
            libc::SECCOMP_RET_ERRNO | (libc::EPERM as u32 & libc::SECCOMP_RET_DATA),
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

pub struct SeccompFilter {
    fd: libc::c_int,
}

impl SeccompFilter {
    // see: cans/11-protocol.md#Trap-fast-path
    pub fn install(defense: &DefenseFlags) -> Result<Self, ShimError> {
        let mut filter = trap_filter_program(defense);
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

pub struct SeccompNotify {
    fd: libc::c_int,
}

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
        defense: &DefenseFlags,
    ) -> Result<Option<crate::introspect_trap::TrapInspection>, ShimError> {
        if !self.poll_ready()? {
            return Ok(None);
        }
        let trap = self.recv_trap()?;
        let verdict = defense_verdict(
            i64::from(trap.syscall_nr),
            defense,
            self.read_trap_pathname(&trap).as_deref(),
            self.read_trap_sockaddr(&trap).as_deref(),
        );
        match verdict {
            TrapVerdict::Deny { errno } => {
                self.respond_deny(trap.id, errno)?;
                Ok(None)
            }
            TrapVerdict::Pass => Ok(Some(crate::introspect_trap::inspect_trap(&trap, seq)?)),
        }
    }

    // see: cans/01-architecture.md#Shim-defense-flags-close-in-guest-kernel-pseudo-filesystem-leaks
    fn read_trap_bytes(&self, trap: &SeccompTrap, arg_index: usize, max: usize) -> Option<Vec<u8>> {
        let addr = trap.args[arg_index];
        if addr == 0 {
            return None;
        }
        let path = format!("/proc/{}/mem", trap.pid);
        let fd = std::fs::File::open(&path).ok()?;
        let mut buf = vec![0u8; max];
        use std::os::unix::fs::FileExt;
        let _ = fd.read_at(&mut buf, addr);
        Some(buf)
    }

    // see: cans/11-protocol.md#Blocks-direct-memory-reads-against-/proc/[pid]/environ-and-process-vm-readv
    fn read_trap_pathname(&self, trap: &SeccompTrap) -> Option<Vec<u8>> {
        let mut raw = self.read_trap_bytes(trap, 1, 256)?;
        let end = raw.iter().position(|byte| *byte == 0).unwrap_or(raw.len());
        raw.truncate(end);
        Some(raw)
    }

    // see: cans/07-wire-plane.md#Abstract-socket-boundary:-wire.block-abstract-unix-sockets-denies-\0-paths
    fn read_trap_sockaddr(&self, trap: &SeccompTrap) -> Option<Vec<u8>> {
        self.read_trap_bytes(trap, 1, 3)
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
                id: notif.id,
                pid: notif.pid,
                syscall_nr: notif.data.nr,
                args: notif.data.args,
            }
        };
        Ok(trap)
    }

    // see: cans/01-architecture.md#Shim-defense-flags-close-in-guest-kernel-pseudo-filesystem-leaks
    pub fn respond_deny(&self, id: u64, errno: libc::c_int) -> Result<(), ShimError> {
        let resp = libc::seccomp_notif_resp {
            id,
            val: 0,
            error: -errno,
            flags: 0,
        };
        // SAFETY: resp is fully initialized; NOTIF_SEND reads it and writes the verdict for a pending id.
        let rc = unsafe {
            libc::ioctl(
                self.fd,
                libc::SECCOMP_IOCTL_NOTIF_SEND,
                &resp as *const libc::seccomp_notif_resp,
            )
        };
        if rc != 0 {
            return Err(ShimError::NotifySend {
                errno: std::io::Error::last_os_error().raw_os_error().unwrap_or(0),
            });
        }
        Ok(())
    }
}

pub struct SeccompTrap {
    pub id: u64,
    pub pid: u32,
    pub syscall_nr: i32,
    pub args: [u64; 6],
}

impl SeccompTrap {
    pub fn route_channel(&self) -> u16 {
        TRAP_CHANNEL
    }
}

// see: cans/11-protocol.md#Trap-fast-path
pub fn init_seccomp_unotify(defense: &DefenseFlags) -> Result<SeccompNotify, ShimError> {
    let filter = SeccompFilter::install(defense)?;
    SeccompNotify::install(&filter)
}
