// see: cans/11-protocol.md#Trap-fast-path
// see: cans/10-attestation.md#Execution-receipts
use std::collections::HashMap;
use std::io::Read;
use std::os::unix::fs::MetadataExt;

use kelir_interface::protocol::{TrapEvent, TrappedSyscall};

use crate::trap::{SeccompTrap, ShimError};

pub const AT_NULL: u64 = 0;
pub const AT_EXECFN: u64 = 31;

const ELF_HEAD_LEN: u64 = 256 * 1024;
const PT_INTERP: u32 = 3;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TrapInspection {
    pub event: TrapEvent,
    pub interp: Option<String>,
    pub execfn_present: bool,
    pub auxv: Vec<(u64, u64)>,
}

pub fn inspect_trap(trap: &SeccompTrap, seq: u64) -> Result<TrapInspection, ShimError> {
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

pub fn digest_exe(exe_path: &str) -> Result<String, ShimError> {
    let mut file = std::fs::File::open(exe_path).map_err(|_| ShimError::ExeDigest {
        path: exe_path.into(),
    })?;
    let mut hasher = blake3::Hasher::new();
    hasher
        .update_reader(&mut file)
        .map_err(|_| ShimError::ExeDigest {
            path: exe_path.into(),
        })?;
    Ok(hasher.finalize().to_hex().to_string())
}

// see: cans/11-protocol.md#Wire-framing
pub fn encode_trap_frame(event: &TrapEvent) -> Result<Vec<u8>, ShimError> {
    let mut payload = crate::codec::MSG_TRAP.to_le_bytes().to_vec();
    ciborium::ser::into_writer(event, &mut payload).map_err(|_| ShimError::TrapEncode)?;
    let total = crate::codec::HEADER_LEN
        .checked_add(payload.len())
        .ok_or(ShimError::TrapEncode)?;
    let mut frame = vec![0u8; total];
    crate::codec::encode_frame(
        u32::from(crate::trap::TRAP_CHANNEL),
        crate::codec::FLAG_DATA,
        &payload,
        &mut frame,
    )
    .map_err(|_| ShimError::TrapEncode)?;
    Ok(frame)
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
    if head.len() < 64 || &head[0..4] != b"\x7fELF" {
        return Err(());
    }
    if head[5] != 1 {
        return Err(());
    }
    match head[4] {
        2 => parse_interp_64(head),
        1 => parse_interp_32(head),
        _ => Err(()),
    }
}

fn parse_interp_64(head: &[u8]) -> Result<Option<String>, ()> {
    let phoff = u64_le(head.get(0x20..0x28).ok_or(())?).ok_or(())? as usize;
    let phentsize = u16_le(head.get(0x36..0x38).ok_or(())?).ok_or(())? as usize;
    let phnum = u16_le(head.get(0x38..0x3a).ok_or(())?).ok_or(())? as usize;
    if phentsize < 56 {
        return Err(());
    }
    for index in 0..phnum {
        let start = phoff
            .checked_add(index.checked_mul(phentsize).ok_or(())?)
            .ok_or(())?;
        let phdr = head
            .get(start..start.checked_add(56).ok_or(())?)
            .ok_or(())?;
        if u32_le(phdr.get(0..4).ok_or(())?).ok_or(())? != PT_INTERP {
            continue;
        }
        let offset = u64_le(phdr.get(8..16).ok_or(())?).ok_or(())? as usize;
        let filesz = u64_le(phdr.get(32..40).ok_or(())?).ok_or(())? as usize;
        let end = offset.checked_add(filesz).ok_or(())?;
        let raw = head.get(offset..end).ok_or(())?;
        let nul = raw.iter().position(|byte| *byte == 0).ok_or(())?;
        return Ok(Some(String::from_utf8_lossy(&raw[..nul]).into_owned()));
    }
    Ok(None)
}

fn parse_interp_32(head: &[u8]) -> Result<Option<String>, ()> {
    let phoff = u32_le(head.get(0x1c..0x20).ok_or(())?).ok_or(())? as usize;
    let phentsize = u16_le(head.get(0x2a..0x2c).ok_or(())?).ok_or(())? as usize;
    let phnum = u16_le(head.get(0x2c..0x2e).ok_or(())?).ok_or(())? as usize;
    if phentsize < 32 {
        return Err(());
    }
    for index in 0..phnum {
        let start = phoff
            .checked_add(index.checked_mul(phentsize).ok_or(())?)
            .ok_or(())?;
        let phdr = head
            .get(start..start.checked_add(32).ok_or(())?)
            .ok_or(())?;
        if u32_le(phdr.get(0..4).ok_or(())?).ok_or(())? != PT_INTERP {
            continue;
        }
        let offset = u32_le(phdr.get(4..8).ok_or(())?).ok_or(())? as usize;
        let filesz = u32_le(phdr.get(16..20).ok_or(())?).ok_or(())? as usize;
        let end = offset.checked_add(filesz).ok_or(())?;
        let raw = head.get(offset..end).ok_or(())?;
        let nul = raw.iter().position(|byte| *byte == 0).ok_or(())?;
        return Ok(Some(String::from_utf8_lossy(&raw[..nul]).into_owned()));
    }
    Ok(None)
}

fn u16_le(bytes: &[u8]) -> Option<u16> {
    let fixed: [u8; 2] = bytes.try_into().ok()?;
    Some(u16::from_le_bytes(fixed))
}

fn u32_le(bytes: &[u8]) -> Option<u32> {
    let fixed: [u8; 4] = bytes.try_into().ok()?;
    Some(u32::from_le_bytes(fixed))
}

fn u64_le(bytes: &[u8]) -> Option<u64> {
    let fixed: [u8; 8] = bytes.try_into().ok()?;
    Some(u64::from_le_bytes(fixed))
}
