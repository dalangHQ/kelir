// see: cans/05-storage-plane.md#Host-inode-lock-broker
use kelir_interface::plugin::FsInterposer;
use kelir_interface::plugin::PluginError;
use kelir_interface::protocol::Verdict;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockKind {
    Shared,
    Exclusive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockRequest {
    pub guest_inode: u64,
    pub host_path: PathBuf,
    pub kind: LockKind,
    pub blocking: bool,
    pub pid: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LockGrant {
    pub guest_inode: u64,
    pub host_inode: u64,
    pub kind: LockKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockState {
    Free,
    Shared { holders: u32 },
    Exclusive { holder_pid: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostFdEntry {
    pub host_inode: u64,
    pub fd: i32,
    pub refcount: u32,
}

#[derive(Default)]
pub struct HostFdCache {
    entries: HashMap<u64, HostFdEntry>,
}

impl HostFdCache {
    pub fn insert(&mut self, guest_inode: u64, entry: HostFdEntry) {
        self.entries.insert(guest_inode, entry);
    }

    pub fn lookup(&self, guest_inode: u64) -> Option<&HostFdEntry> {
        self.entries.get(&guest_inode)
    }

    pub fn remove(&mut self, guest_inode: u64) -> Option<HostFdEntry> {
        self.entries.remove(&guest_inode)
    }
}

pub struct LockBroker {
    interposer: Arc<dyn FsInterposer>,
    cache: RwLock<HostFdCache>,
    states: RwLock<HashMap<u64, LockState>>,
}

impl LockBroker {
    pub fn new(interposer: Arc<dyn FsInterposer>) -> Self {
        Self {
            interposer,
            cache: RwLock::new(HostFdCache::default()),
            states: RwLock::new(HashMap::new()),
        }
    }

    pub fn acquire(&self, request: &LockRequest) -> Result<LockGrant, PluginError> {
        let flock_cmd = match (request.kind, request.blocking) {
            // TODO (L3): flock LOCK_SH/LOCK_EX/LOCK_NB constants live in the enclave.
            (LockKind::Shared, true) => 1,
            (LockKind::Shared, false) => 5,
            (LockKind::Exclusive, true) => 2,
            (LockKind::Exclusive, false) => 6,
        };
        let flock_type: i16 = match request.kind {
            LockKind::Shared => 0,
            LockKind::Exclusive => 1,
        };
        match self
            .interposer
            .on_fs_lock(&request.host_path, flock_cmd, flock_type, request.pid)?
        {
            Verdict::Pass => {}
            Verdict::Drop { errno, reason } => {
                return Err(PluginError::Halt(format!(
                    "lock denied by interposer: {reason} ({errno:?})"
                )));
            }
            Verdict::Patch(_) => {
                return Err(PluginError::Halt(
                    "lock interposer returned Patch, unsupported for locks".into(),
                ));
            }
            Verdict::Suspend { .. } => {
                return Err(PluginError::Halt(
                    "lock interposer returned Suspend, unsupported for locks".into(),
                ));
            }
            Verdict::Abort { reason } => {
                return Err(PluginError::Halt(format!(
                    "lock aborted by interposer: {reason}"
                )));
            }
            _ => {
                return Err(PluginError::Halt(
                    "lock interposer returned an unknown verdict".into(),
                ));
            }
        }
        // TODO (L3): host fd cache lookup, flock delegation, EWOULDBLOCK on conflict.
        let _ = (&self.cache, &self.states, request);
        Err(PluginError::Halt(
            "TODO (L3): LockBroker::acquire needs flock delegation in kelir-drivers/kelir-transport enclave".into(),
        ))
    }

    pub fn release(&self, guest_inode: u64) -> Result<(), PluginError> {
        // TODO (L3): flock LOCK_UN plus host fd cache release in the enclave.
        let _ = guest_inode;
        Err(PluginError::Halt(
            "TODO (L3): LockBroker::release needs flock delegation in kelir-drivers/kelir-transport enclave".into(),
        ))
    }

    pub fn state(&self, guest_inode: u64) -> Result<LockState, PluginError> {
        let states = self
            .states
            .read()
            .map_err(|_| PluginError::Halt("lock broker state lock poisoned".into()))?;
        Ok(states.get(&guest_inode).copied().unwrap_or(LockState::Free))
    }
}
