// see: cans/11-protocol.md#Trap-fast-path
// see: cans/11-protocol.md#Frozen-inspection-mechanics
// see: cans/10-attestation.md#Execution-receipts
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use kelir_interface::protocol_wire::TrapEvent;

use crate::trap_protocol::{SeccompTrap, ShimError};

#[path = "introspect_trap/uapi_trap.rs"]
mod uapi_trap;

// TODO (L3) [protocol_mux]: remove this allow when the stubbed trap-inspection callers land and use these constants again.; slop: allow masks the unused constants, so the inspection wiring can stay unwired while the build stays green.
#[allow(unused_imports)]
pub use self::uapi_trap::{AT_EXECFN, AT_NULL};

const ELF_HEAD_LEN: u64 = 256 * 1024;

// see: cans/11-protocol.md#Frozen-inspection-mechanics
const CGROUP_V2_ROOT: &str = "/sys/fs/cgroup";

// see: cans/11-protocol.md#Frozen-inspection-mechanics
const FREEZE_POLL_LIMIT: u32 = 1000;

#[derive(Debug, Clone)]
pub struct TrapInspection {
    pub event: TrapEvent,
    pub interp: Option<String>,
    pub execfn_present: bool,
    pub auxv: Vec<(u64, u64)>,
}

pub fn inspect_trap(trap: &SeccompTrap, _seq: u64) -> Result<TrapInspection, ShimError> {
    // TODO (L3) [protocol_mux]: capture argv/envp/auxv via process_vm_readv without thread attachment; slop: argv/envp read via /proc race, frozen thread mutates mid-read
    // TODO (L3) [protocol_mux]: fill resolved_binary from interpreter unwrapping; zero identity until then; slop: interpreter path reported as binary, BLAKE3 binds wrong file
    // TODO (L3) [protocol_mux]: freeze the trapped pid, resolve exe/argv/envp/cwd/ppid/auxv/interp and build the TrapInspection.; slop: TrapInspection is faked from defaults, so a deny decision is made on invented identity.
    Err(ShimError::ExeUnresolved { pid: trap.pid })
}

pub fn resolve_exe(pid: u32) -> Result<(String, u64, u64), ShimError> {
    // TODO (L3) [protocol_mux]: resolve /proc/<pid>/exe and return its path, device and inode.; slop: exe path defaults to empty, so attestation binds the trap to the wrong binary.
    Err(ShimError::ExeUnresolved { pid })
}

pub fn read_argv(pid: u32) -> Result<Vec<String>, ShimError> {
    // TODO (L3) [protocol_mux]: read /proc/<pid>/cmdline and split it into argv entries.; slop: argv defaults to empty, so the policy match runs against no command line.
    Err(ShimError::ProcRead {
        pid,
        field: "cmdline",
    })
}

pub fn read_envp(pid: u32) -> Result<HashMap<String, String>, ShimError> {
    // TODO (L3) [protocol_mux]: read /proc/<pid>/environ and parse it into key/value pairs.; slop: environ defaults to empty, so an env-gated secret leak passes unnoticed.
    Err(ShimError::ProcRead {
        pid,
        field: "environ",
    })
}

pub fn read_cwd(pid: u32) -> Result<String, ShimError> {
    // TODO (L3) [protocol_mux]: resolve /proc/<pid>/cwd and return its target path.; slop: cwd defaults to empty, so a cwd-scoped path check approves the wrong directory.
    Err(ShimError::ProcRead { pid, field: "cwd" })
}

pub fn read_ppid(pid: u32) -> Result<u32, ShimError> {
    // TODO (L3) [protocol_mux]: parse the parent pid from /proc/<pid>/stat.; slop: ppid defaults to zero, so parent-chain policy treats the trap as parentless.
    Err(ShimError::ProcRead { pid, field: "stat" })
}

pub fn read_auxv(pid: u32) -> Result<Vec<(u64, u64)>, ShimError> {
    // TODO (L3) [protocol_mux]: read /proc/<pid>/auxv and parse it into (key, value) pairs up to AT_NULL.; slop: auxv defaults to empty, so interpreter detection silently reports none.
    Err(ShimError::ProcRead { pid, field: "auxv" })
}

pub fn auxv_value(_auxv: &[(u64, u64)], _key: u64) -> Option<u64> {
    // TODO (L3) [protocol_mux]: return the auxv value for the key, if present.; slop: lookup always misses, so AT_EXECFN-dependent identity silently falls back.
    None
}

pub fn read_interp(exe_path: &str) -> Result<Option<String>, ShimError> {
    // TODO (L3) [protocol_mux]: parse the ELF program headers of the exe and return its PT_INTERP path, if any.; slop: interp defaults to none, so a script trap is judged as a native binary.
    Err(ShimError::ElfParse {
        path: exe_path.into(),
    })
}

// see: cans/artifacts/code/messages.rs#TrapEvent
pub fn digest_exe(exe_path: &str) -> Result<[u8; 32], ShimError> {
    // TODO (L3) [protocol_mux]: BLAKE3-digest the exe file and return the 32-byte digest.; slop: digest defaults to zeros, so two different exes share one identity.
    Err(ShimError::ExeDigest {
        path: exe_path.into(),
    })
}

// see: cans/11-protocol.md#Wire-framing
pub fn encode_trap_frame(_event: &TrapEvent) -> Result<Vec<u8>, ShimError> {
    // TODO (L3) [protocol_mux]: CBOR-encode the TrapEvent and frame it as a MSG_TRAP frame on the trap channel.; slop: trap frame is never sent, so the host believes no trap fired.
    Err(ShimError::TrapEncode)
}

// see: cans/11-protocol.md#Frozen-inspection-mechanics
pub struct FreezeGuard {
    freeze_path: PathBuf,
}

impl FreezeGuard {
    // see: cans/11-protocol.md#Frozen-inspection-mechanics
    pub fn acquire(pid: u32) -> Result<Self, ShimError> {
        // TODO (L3) [protocol_mux]: freeze the pid cgroup v2 subtree and wait until it reports frozen.; slop: pid keeps running during inspection, so /proc reads race a live thread and capture torn state.
        Err(ShimError::FreezerUnsupported { pid })
    }
}

impl Drop for FreezeGuard {
    fn drop(&mut self) {
        // TODO (L3) [protocol_mux]: thaw the frozen cgroup by writing 0 to cgroup.freeze.; slop: cgroup stays frozen, so the trapped process never resumes after inspection.
    }
}

// see: cans/11-protocol.md#Frozen-inspection-mechanics
fn cgroup_v2_relative(pid: u32) -> Result<String, ShimError> {
    // TODO (L3) [protocol_mux]: read /proc/<pid>/cgroup and return the cgroup v2 relative path.; slop: cgroup path defaults to empty, so freeze targets the wrong subtree or none.
    Err(ShimError::FreezerUnsupported { pid })
}

// see: cans/11-protocol.md#Frozen-inspection-mechanics
fn wait_frozen(_dir: &Path) -> Result<(), ShimError> {
    // TODO (L3) [protocol_mux]: poll cgroup.events until the directory reports frozen, up to FREEZE_POLL_LIMIT.; slop: poll returns at once, so inspection reads a still-running process as frozen.
    Err(ShimError::FreezerTimeout {
        path: String::new(),
    })
}

fn read_proc_bytes(pid: u32, field: &'static str) -> Result<Vec<u8>, ShimError> {
    // TODO (L3) [protocol_mux]: read the bytes of /proc/<pid>/<field>.; slop: field reads return empty, so every inspection field silently defaults.
    Err(ShimError::ProcRead { pid, field })
}

fn read_u64_ne(_bytes: &[u8]) -> Option<u64> {
    // TODO (L3) [protocol_mux]: decode 8 native-endian bytes as a u64.; slop: decode returns zero, so auxv values silently read as zero.
    None
}

fn parse_interp(_head: &[u8]) -> Result<Option<String>, ()> {
    // TODO (L3) [protocol_mux]: validate the ELF header and dispatch PT_INTERP parsing by ELF class.; slop: dispatch returns none, so PT_INTERP is never found on any ELF class.
    Err(())
}

fn parse_interp_64(_head: &[u8]) -> Result<Option<String>, ()> {
    // TODO (L3) [protocol_mux]: walk the 64-bit ELF program headers and return the PT_INTERP string, if any.; slop: 64-bit interp defaults to none, so 64-bit scripts are misclassified.
    Err(())
}

fn parse_interp_32(_head: &[u8]) -> Result<Option<String>, ()> {
    // TODO (L3) [protocol_mux]: walk the 32-bit ELF program headers and return the PT_INTERP string, if any.; slop: 32-bit interp defaults to none, so 32-bit scripts are misclassified.
    Err(())
}

fn u16_le_at(_buf: &[u8], _offset: usize) -> Option<u16> {
    // TODO (L3) [protocol_mux]: decode 2 little-endian bytes at the offset as a u16.; slop: u16 decode returns zero, so ELF header fields silently read as zero.
    None
}

fn u32_le_at(_buf: &[u8], _offset: usize) -> Option<u32> {
    // TODO (L3) [protocol_mux]: decode 4 little-endian bytes at the offset as a u32.; slop: u32 decode returns zero, so ELF header fields silently read as zero.
    None
}

fn u64_le_at(_buf: &[u8], _offset: usize) -> Option<u64> {
    // TODO (L3) [protocol_mux]: decode 8 little-endian bytes at the offset as a u64.; slop: u64 decode returns zero, so ELF offsets silently read as zero.
    None
}
