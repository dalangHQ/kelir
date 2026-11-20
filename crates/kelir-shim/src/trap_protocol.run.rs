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
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO (L3) [protocol_mux]: render the ShimError variant as a human-readable message.; slop: every error renders as a blank message, so a trap failure is indistinguishable from success.
        Err(std::fmt::Error)
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
pub fn environ_open(_path: &[u8]) -> bool {
    // TODO (L3) [protocol_mux]: return true when the path opens /proc/<pid>/environ.; slop: check always returns false, so environ opens are never denied.
    false
}

// see: cans/07-wire-plane.md#Abstract-socket-boundary:-wire.block-abstract-unix-sockets-denies-\0-paths
pub fn abstract_unix_addr(_addr: &[u8]) -> bool {
    // TODO (L3) [protocol_mux]: return true when the sockaddr is an AF_UNIX address with a leading NUL byte.; slop: check always returns false, so abstract socket connects are never denied.
    false
}

// see: cans/01-architecture.md#Shim-defense-flags-close-in-guest-kernel-pseudo-filesystem-leaks
pub fn defense_verdict(
    _syscall_nr: i64,
    _defense: &DefenseFlags,
    _pathname: Option<&[u8]>,
    _sockaddr: Option<&[u8]>,
) -> TrapVerdict {
    // TODO (L3) [protocol_mux]: deny environ opens and abstract unix sockets per the defense flags, else pass.; slop: verdict always passes, so the defense flags silently stop enforcing.
    TrapVerdict::Pass
}

// see: cans/11-protocol.md#Blocks-direct-memory-reads-against-/proc/[pid]/environ-and-process-vm-readv
fn is_openat_nr(_syscall_nr: i64) -> bool {
    // TODO (L3) [protocol_mux]: return true for openat/openat2 syscall numbers.; slop: check always returns false, so openat traps are never recognized.
    false
}

// see: cans/07-wire-plane.md#Abstract-socket-boundary:-wire.block-abstract-unix-sockets-denies-\0-paths
fn is_unix_socket_nr(_syscall_nr: i64) -> bool {
    // TODO (L3) [protocol_mux]: return true for connect/bind syscall numbers.; slop: check always returns false, so connect and bind traps are never recognized.
    false
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
fn trap_filter_program(_defense: &DefenseFlags) -> Vec<libc::sock_filter> {
    // L3-GAP: [hd_02_wx_mmu_prot_exec_trap]: gate-1 in-kernel allowlist fast path for harmless tools plus in-filter mprotect(PROT_EXEC)/memfd_create flag masking; slop: allowlist passes mprotect PROT_EXEC, Stage-2 fault never fires
    // see: cans/01-architecture.md#Shim-defense-flags-close-in-guest-kernel-pseudo-filesystem-leaks
    // TODO (L3) [protocol_mux]: build the seccomp BPF program trapping the monitored syscalls and returning USER_NOTIF.; slop: filter is never built, so no syscall is ever trapped.
    Vec::new()
}

pub struct SeccompFilter {
    fd: libc::c_int,
}

impl SeccompFilter {
    // see: cans/11-protocol.md#Trap-fast-path
    pub fn install(_defense: &DefenseFlags) -> Result<Self, ShimError> {
        // TODO (L3) [protocol_mux]: build the trap filter program and install it via seccomp(2) with a new listener.; slop: filter is never installed, so the guest runs unmonitored while the host believes it is trapped.
        Err(ShimError::SeccompInstall { errno: 0 })
    }

    pub fn fd(&self) -> libc::c_int {
        // TODO (L3) [protocol_mux]: return the seccomp listener fd.; slop: listener fd is invalid, so the first trap notification is lost.
        0
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
        // TODO (L3) [protocol_mux]: return the seccomp notify fd.; slop: notify fd is invalid, so trap polling never sees a pending trap.
        0
    }

    pub fn poll_ready(&self) -> Result<bool, ShimError> {
        // TODO (L3) [protocol_mux]: poll the notify fd with a zero timeout and return whether a notification is pending.; slop: poll always reports none, so pending traps sit unhandled.
        Err(ShimError::NotifyPoll { errno: 0 })
    }

    pub fn dispatch_pending(
        &self,
        _seq: u64,
        _defense: &DefenseFlags,
    ) -> Result<Option<crate::introspect_trap::TrapInspection>, ShimError> {
        // TODO (L3) [protocol_mux]: recv a pending trap, apply the defense verdict, and return the inspection or deny it.; slop: traps are never received, so a denied syscall executes anyway.
        Err(ShimError::NotifyPoll { errno: 0 })
    }

    // see: cans/01-architecture.md#Shim-defense-flags-close-in-guest-kernel-pseudo-filesystem-leaks
    fn read_trap_bytes(
        &self,
        _trap: &SeccompTrap,
        _arg_index: usize,
        _max: usize,
    ) -> Option<Vec<u8>> {
        // TODO (L3) [protocol_mux]: read up to max bytes from the trapped pid memory at the syscall arg address.; slop: memory reads return empty, so syscall args are inspected as blank.
        None
    }

    // see: cans/11-protocol.md#Blocks-direct-memory-reads-against-/proc/[pid]/environ-and-process-vm-readv
    fn read_trap_pathname(&self, _trap: &SeccompTrap) -> Option<Vec<u8>> {
        // TODO (L3) [protocol_mux]: read the NUL-terminated pathname from the trapped pid memory.; slop: pathname reads return empty, so path policy matches against nothing.
        None
    }

    // see: cans/07-wire-plane.md#Abstract-socket-boundary:-wire.block-abstract-unix-sockets-denies-\0-paths
    fn read_trap_sockaddr(&self, _trap: &SeccompTrap) -> Option<Vec<u8>> {
        // TODO (L3) [protocol_mux]: read the sockaddr bytes from the trapped pid memory.; slop: sockaddr reads return empty, so socket policy matches against nothing.
        None
    }

    // see: cans/11-protocol.md#Trap-fast-path
    pub fn recv_trap(&self) -> Result<SeccompTrap, ShimError> {
        // TODO (L3) [protocol_mux]: recv a seccomp notification via SECCOMP_IOCTL_NOTIF_RECV and build the SeccompTrap.; slop: notifications are never received, so the guest blocks forever on a trap that never arrives.
        Err(ShimError::NotifyRecv { errno: 0 })
    }

    // see: cans/01-architecture.md#Shim-defense-flags-close-in-guest-kernel-pseudo-filesystem-leaks
    pub fn respond_deny(&self, _id: u64, _errno: libc::c_int) -> Result<(), ShimError> {
        // TODO (L3) [protocol_mux]: send a deny verdict for the notification id via SECCOMP_IOCTL_NOTIF_SEND.; slop: deny is never sent, so a denied syscall is allowed by default.
        Err(ShimError::NotifySend { errno: 0 })
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
        // TODO (L3) [protocol_mux]: return the trap channel this trap routes to.; slop: routing defaults to the wrong channel, so the trap is delivered to a listener that never reads it.
        0
    }
}

// see: cans/11-protocol.md#Trap-fast-path
pub fn init_seccomp_unotify(_defense: &DefenseFlags) -> Result<SeccompNotify, ShimError> {
    // TODO (L3) [protocol_mux]: install the seccomp filter and return the notify handle for its listener fd.; slop: install is skipped, so the session reports a live trap handle that traps nothing.
    Err(ShimError::SeccompInstall { errno: 0 })
}
