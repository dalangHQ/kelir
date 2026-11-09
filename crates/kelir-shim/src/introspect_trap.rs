// see: cans/11-protocol.md#Trap-fast-path
// see: cans/11-protocol.md#Frozen-inspection-mechanics
// see: cans/10-attestation.md#Execution-receipts
use std::collections::HashMap;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use kelir_interface::protocol_wire::{MsgType, TrapEvent, TrappedSyscall};

use crate::trap_protocol::{SeccompTrap, ShimError};

mod uapi_trap;

pub use self::uapi_trap::{AT_EXECFN, AT_NULL};

const ELF_HEAD_LEN: u64 = 256 * 1024;

// see: cans/11-protocol.md#Frozen-inspection-mechanics
const CGROUP_V2_ROOT: &str = "/sys/fs/cgroup";

// see: cans/11-protocol.md#Frozen-inspection-mechanics
const FREEZE_POLL_LIMIT: u32 = 1000;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TrapInspection {
    pub event: TrapEvent,
    pub interp: Option<String>,
    pub execfn_present: bool,
    pub auxv: Vec<(u64, u64)>,
}

pub fn inspect_trap(trap: &SeccompTrap, seq: u64) -> Result<TrapInspection, ShimError> {
    // TODO (L3): capture argv/envp/auxv via process_vm_readv without thread attachment.
    let _frozen = FreezeGuard::acquire(trap.pid)?;
    let (binary_path, binary_dev, binary_ino) = resolve_exe(trap.pid)?;
    let argv = read_argv(trap.pid)?;
    let envp = read_envp(trap.pid)?;
    let cwd = read_cwd(trap.pid)?;
    let ppid = read_ppid(trap.pid)?;
    let auxv = read_auxv(trap.pid)?;
    let interp = read_interp(&binary_path)?;
    let binary_blake3 = digest_exe(&binary_path)?;
    Ok(TrapInspection {
        event: TrapEvent {
            seq,
            pid: trap.pid,
            ppid,
            task_id: u64::from(trap.pid),
            binary_path,
            binary_dev,
            binary_ino,
            binary_blake3,
            argv,
            envp,
            cwd,
            syscall: Some(TrappedSyscall {
                nr: i64::from(trap.syscall_nr),
                mprotect_prot: None,
                memfd_create_flags: None,
            }),
        },
        interp,
        execfn_present: auxv_value(&auxv, AT_EXECFN).is_some(),
        auxv,
    })
}

pub fn resolve_exe(pid: u32) -> Result<(String, u64, u64), ShimError> {
    let link = format!("/proc/{pid}/exe");
    let path = std::fs::read_link(&link).map_err(|_| ShimError::ExeUnresolved { pid })?;
    let metadata =
        std::fs::metadata(&path).map_err(|_| ShimError::ProcRead { pid, field: "exe" })?;
    Ok((
        path.to_string_lossy().into_owned(),
        metadata.dev(),
        metadata.ino(),
    ))
}

pub fn read_argv(pid: u32) -> Result<Vec<String>, ShimError> {
    let bytes = read_proc_bytes(pid, "cmdline")?;
    Ok(bytes
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect())
}

pub fn read_envp(pid: u32) -> Result<HashMap<String, String>, ShimError> {
    let bytes = read_proc_bytes(pid, "environ")?;
    let mut envp = HashMap::new();
    for part in bytes.split(|byte| *byte == 0) {
        if part.is_empty() {
            continue;
        }
        let text = String::from_utf8_lossy(part);
        if let Some((key, value)) = text.split_once('=') {
            envp.insert(key.to_owned(), value.to_owned());
        }
    }
    Ok(envp)
}

pub fn read_cwd(pid: u32) -> Result<String, ShimError> {
    let path = format!("/proc/{pid}/cwd");
    std::fs::read_link(&path)
        .map(|target| target.to_string_lossy().into_owned())
        .map_err(|_| ShimError::ProcRead { pid, field: "cwd" })
}

pub fn read_ppid(pid: u32) -> Result<u32, ShimError> {
    let bytes = read_proc_bytes(pid, "stat")?;
    let text = String::from_utf8_lossy(&bytes);
    let after_comm = text
        .rfind(')')
        .ok_or(ShimError::ProcRead { pid, field: "stat" })?;
    let fields: Vec<&str> = text[after_comm + 1..].split_whitespace().collect();
    fields
        .get(1)
        .ok_or(ShimError::ProcRead { pid, field: "stat" })?
        .parse::<u32>()
        .map_err(|_| ShimError::ProcRead { pid, field: "stat" })
}

pub fn read_auxv(pid: u32) -> Result<Vec<(u64, u64)>, ShimError> {
    let bytes = read_proc_bytes(pid, "auxv")?;
    let mut pairs = Vec::new();
    for pair in bytes.chunks_exact(16) {
        let key = read_u64_ne(&pair[0..8]).ok_or(ShimError::ProcRead { pid, field: "auxv" })?;
        let value = read_u64_ne(&pair[8..16]).ok_or(ShimError::ProcRead { pid, field: "auxv" })?;
        if key == AT_NULL {
            break;
        }
        pairs.push((key, value));
    }
    Ok(pairs)
}

pub fn auxv_value(auxv: &[(u64, u64)], key: u64) -> Option<u64> {
    auxv.iter()
        .find(|(entry_key, _)| *entry_key == key)
        .map(|(_, value)| *value)
}

pub fn read_interp(exe_path: &str) -> Result<Option<String>, ShimError> {
    let mut file = std::fs::File::open(exe_path).map_err(|_| ShimError::ElfParse {
        path: exe_path.into(),
    })?;
    let mut head = Vec::new();
    file.by_ref()
        .take(ELF_HEAD_LEN)
        .read_to_end(&mut head)
        .map_err(|_| ShimError::ElfParse {
            path: exe_path.into(),
        })?;
    parse_interp(&head).map_err(|_| ShimError::ElfParse {
        path: exe_path.into(),
    })
}

// see: cans/artifacts/code/messages.rs#TrapEvent
pub fn digest_exe(exe_path: &str) -> Result<[u8; 32], ShimError> {
    let mut file = std::fs::File::open(exe_path).map_err(|_| ShimError::ExeDigest {
        path: exe_path.into(),
    })?;
    let mut hasher = blake3::Hasher::new();
    hasher
        .update_reader(&mut file)
        .map_err(|_| ShimError::ExeDigest {
            path: exe_path.into(),
        })?;
    Ok(*hasher.finalize().as_bytes())
}

// see: cans/11-protocol.md#Wire-framing
pub fn encode_trap_frame(event: &TrapEvent) -> Result<Vec<u8>, ShimError> {
    let mut payload = (MsgType::Trap as u16).to_le_bytes().to_vec();
    ciborium::ser::into_writer(event, &mut payload).map_err(|_| ShimError::TrapEncode)?;
    let total = crate::codec_protocol::HEADER_LEN
        .checked_add(payload.len())
        .ok_or(ShimError::TrapEncode)?;
    let mut frame = vec![0u8; total];
    crate::codec_protocol::encode_frame(
        u32::from(crate::trap_protocol::TRAP_CHANNEL),
        crate::codec_protocol::FLAG_DATA,
        &payload,
        &mut frame,
    )
    .map_err(|_| ShimError::TrapEncode)?;
    Ok(frame)
}

// see: cans/11-protocol.md#Frozen-inspection-mechanics
pub struct FreezeGuard {
    freeze_path: PathBuf,
}

impl FreezeGuard {
    // see: cans/11-protocol.md#Frozen-inspection-mechanics
    pub fn acquire(pid: u32) -> Result<Self, ShimError> {
        let relative = cgroup_v2_relative(pid)?;
        if relative == "/" {
            return Err(ShimError::FreezerRoot { pid });
        }
        let dir = Path::new(CGROUP_V2_ROOT).join(relative.trim_start_matches('/'));
        let freeze_path = dir.join("cgroup.freeze");
        std::fs::write(&freeze_path, b"1").map_err(|err| ShimError::FreezerIo {
            path: freeze_path.to_string_lossy().into_owned(),
            errno: err.raw_os_error().unwrap_or(0),
        })?;
        wait_frozen(&dir)?;
        Ok(Self { freeze_path })
    }
}

impl Drop for FreezeGuard {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.freeze_path, b"0");
    }
}

// see: cans/11-protocol.md#Frozen-inspection-mechanics
fn cgroup_v2_relative(pid: u32) -> Result<String, ShimError> {
    let bytes = read_proc_bytes(pid, "cgroup")?;
    let text = String::from_utf8_lossy(&bytes);
    let Some(relative) = text.lines().find_map(|line| line.strip_prefix("0::")) else {
        return Err(ShimError::FreezerUnsupported { pid });
    };
    Ok(relative.trim().to_owned())
}

// see: cans/11-protocol.md#Frozen-inspection-mechanics
fn wait_frozen(dir: &Path) -> Result<(), ShimError> {
    let events_path = dir.join("cgroup.events");
    let nap = libc::timespec {
        tv_sec: 0,
        tv_nsec: 1_000_000,
    };
    for _ in 0..FREEZE_POLL_LIMIT {
        let text = std::fs::read_to_string(&events_path).map_err(|err| ShimError::FreezerIo {
            path: events_path.to_string_lossy().into_owned(),
            errno: err.raw_os_error().unwrap_or(0),
        })?;
        if text.lines().any(|line| line == "frozen 1") {
            return Ok(());
        }
        // SAFETY: nap is a fixed 1ms timespec; a null remainder pointer is valid when the leftover time is discarded.
        unsafe { libc::nanosleep(&nap, std::ptr::null_mut()) };
    }
    Err(ShimError::FreezerTimeout {
        path: events_path.to_string_lossy().into_owned(),
    })
}

fn read_proc_bytes(pid: u32, field: &'static str) -> Result<Vec<u8>, ShimError> {
    let path = format!("/proc/{pid}/{field}");
    std::fs::read(&path).map_err(|_| ShimError::ProcRead { pid, field })
}

fn read_u64_ne(bytes: &[u8]) -> Option<u64> {
    let fixed: [u8; 8] = bytes.try_into().ok()?;
    Some(u64::from_ne_bytes(fixed))
}

fn parse_interp(head: &[u8]) -> Result<Option<String>, ()> {
    if head.len() < uapi_trap::EHDR64_SIZE {
        return Err(());
    }
    if head[uapi_trap::EI_MAG0..uapi_trap::EI_MAG0 + uapi_trap::SELFMAG] != uapi_trap::ELFMAG {
        return Err(());
    }
    if head[uapi_trap::EI_DATA] != uapi_trap::ELFDATA2LSB {
        return Err(());
    }
    match head[uapi_trap::EI_CLASS] {
        uapi_trap::ELFCLASS64 => parse_interp_64(head),
        uapi_trap::ELFCLASS32 => parse_interp_32(head),
        _ => Err(()),
    }
}

fn parse_interp_64(head: &[u8]) -> Result<Option<String>, ()> {
    let phoff = u64_le_at(head, uapi_trap::EHDR64_E_PHOFF).ok_or(())? as usize;
    let phentsize = u16_le_at(head, uapi_trap::EHDR64_E_PHENTSIZE).ok_or(())? as usize;
    let phnum = u16_le_at(head, uapi_trap::EHDR64_E_PHNUM).ok_or(())? as usize;
    if phentsize < uapi_trap::PHDR64_SIZE {
        return Err(());
    }
    for index in 0..phnum {
        let start = phoff
            .checked_add(index.checked_mul(phentsize).ok_or(())?)
            .ok_or(())?;
        let phdr = head
            .get(start..start.checked_add(uapi_trap::PHDR64_SIZE).ok_or(())?)
            .ok_or(())?;
        if u32_le_at(phdr, uapi_trap::PHDR64_P_TYPE).ok_or(())? != uapi_trap::PT_INTERP {
            continue;
        }
        let offset = u64_le_at(phdr, uapi_trap::PHDR64_P_OFFSET).ok_or(())? as usize;
        let filesz = u64_le_at(phdr, uapi_trap::PHDR64_P_FILESZ).ok_or(())? as usize;
        let end = offset.checked_add(filesz).ok_or(())?;
        let raw = head.get(offset..end).ok_or(())?;
        let nul = raw.iter().position(|byte| *byte == 0).ok_or(())?;
        return Ok(Some(String::from_utf8_lossy(&raw[..nul]).into_owned()));
    }
    Ok(None)
}

fn parse_interp_32(head: &[u8]) -> Result<Option<String>, ()> {
    let phoff = u32_le_at(head, uapi_trap::EHDR32_E_PHOFF).ok_or(())? as usize;
    let phentsize = u16_le_at(head, uapi_trap::EHDR32_E_PHENTSIZE).ok_or(())? as usize;
    let phnum = u16_le_at(head, uapi_trap::EHDR32_E_PHNUM).ok_or(())? as usize;
    if phentsize < uapi_trap::PHDR32_SIZE {
        return Err(());
    }
    for index in 0..phnum {
        let start = phoff
            .checked_add(index.checked_mul(phentsize).ok_or(())?)
            .ok_or(())?;
        let phdr = head
            .get(start..start.checked_add(uapi_trap::PHDR32_SIZE).ok_or(())?)
            .ok_or(())?;
        if u32_le_at(phdr, uapi_trap::PHDR32_P_TYPE).ok_or(())? != uapi_trap::PT_INTERP {
            continue;
        }
        let offset = u32_le_at(phdr, uapi_trap::PHDR32_P_OFFSET).ok_or(())? as usize;
        let filesz = u32_le_at(phdr, uapi_trap::PHDR32_P_FILESZ).ok_or(())? as usize;
        let end = offset.checked_add(filesz).ok_or(())?;
        let raw = head.get(offset..end).ok_or(())?;
        let nul = raw.iter().position(|byte| *byte == 0).ok_or(())?;
        return Ok(Some(String::from_utf8_lossy(&raw[..nul]).into_owned()));
    }
    Ok(None)
}

fn u16_le_at(buf: &[u8], offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    let fixed: [u8; 2] = buf.get(offset..end)?.try_into().ok()?;
    Some(u16::from_le_bytes(fixed))
}

fn u32_le_at(buf: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let fixed: [u8; 4] = buf.get(offset..end)?.try_into().ok()?;
    Some(u32::from_le_bytes(fixed))
}

fn u64_le_at(buf: &[u8], offset: usize) -> Option<u64> {
    let end = offset.checked_add(8)?;
    let fixed: [u8; 8] = buf.get(offset..end)?.try_into().ok()?;
    Some(u64::from_le_bytes(fixed))
}
