// see: cans/05-storage-plane.md#Host-inode-lock-broker
use async_trait::async_trait;
use kelir_engine::planes::storage::LockBrokerWiring;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{HandlerCtx, LockBrokerCoordination};
use kelir_interface::protocol_wire::Verdict;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use kelir_engine::dispatch_plugin::PipelineEngine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockKind {
    Shared,
    Exclusive,
}

// see: cans/05-storage-plane.md#Host-inode-lock-broker
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LockKey {
    pub namespace: [u8; 16],
    pub guest_inode: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockRequest {
    pub namespace: [u8; 16],
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
    entries: HashMap<LockKey, HostFdEntry>,
}

impl HostFdCache {
    pub fn insert(&mut self, key: LockKey, entry: HostFdEntry) {
        self.entries.insert(key, entry);
    }

    pub fn lookup(&self, key: LockKey) -> Option<&HostFdEntry> {
        self.entries.get(&key)
    }

    pub fn remove(&mut self, key: LockKey) -> Option<HostFdEntry> {
        self.entries.remove(&key)
    }

    // see: cans/05-storage-plane.md#Host-inode-lock-broker
    pub fn remove_namespace(&mut self, namespace: [u8; 16]) {
        self.entries.retain(|key, _| key.namespace != namespace);
    }
}

pub struct LockBroker {
    engine: Arc<PipelineEngine>,
    cache: RwLock<HostFdCache>,
    states: RwLock<HashMap<LockKey, LockState>>,
    namespace: [u8; 16],
    wiring: LockBrokerWiring,
}

impl LockBroker {
    pub fn new(engine: Arc<PipelineEngine>) -> Self {
        Self::with_wiring(engine, LockBrokerWiring::default())
    }

    // see: cans/05-storage-plane.md#Host-inode-lock-broker
    pub fn with_wiring(engine: Arc<PipelineEngine>, wiring: LockBrokerWiring) -> Self {
        Self {
            engine,
            cache: RwLock::new(HostFdCache::default()),
            states: RwLock::new(HashMap::new()),
            namespace: [0; 16],
            wiring,
        }
    }

    // see: cans/05-storage-plane.md#Host-inode-lock-broker
    pub fn wiring(&self) -> &LockBrokerWiring {
        &self.wiring
    }

    pub fn with_namespace(mut self, namespace: [u8; 16]) -> Self {
        self.namespace = namespace;
        self
    }

    // see: cans/05-storage-plane.md#Host-inode-lock-broker
    pub fn namespace(&self) -> [u8; 16] {
        self.namespace
    }

    pub fn acquire(
        &self,
        request: &LockRequest,
        ctx: &HandlerCtx,
    ) -> Result<LockGrant, PluginError> {
        // TODO (L3) [storage_plane]: match request.host_path against wiring.scope_globs before mediation; undeclared paths deny with EPERM; slop: undeclared path mediated, EPERM never denied
        let blocking = request.blocking && !self.wiring.non_blocking;
        let flock_cmd = match (request.kind, blocking) {
            // TODO (L3) [sp_15_host_inode_lock_broker_flock_bridge]: flock LOCK_SH/LOCK_EX/LOCK_NB constants live in the enclave; slop: in-memory lock table, no host flock, contender succeeds
            (LockKind::Shared, true) => 1,
            (LockKind::Shared, false) => 5,
            (LockKind::Exclusive, true) => 2,
            (LockKind::Exclusive, false) => 6,
        };
        let flock_type: i16 = match request.kind {
            LockKind::Shared => 0,
            LockKind::Exclusive => 1,
        };
        match self.engine.dispatch_fs_lock(
            &request.host_path,
            flock_cmd,
            flock_type,
            request.pid,
            ctx,
        )? {
            Verdict::Pass => {}
            Verdict::Drop { errno, reason } => {
                return Err(PluginError::Halt(format!(
                    "lock denied by interposer: {reason} ({})",
                    errno.map_or("none", |errno| errno.as_str())
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
        // TODO (L3) [sp_19_host_inode_flock_contention_ewouldblock]: host fd cache lookup, flock delegation, EWOULDBLOCK on conflict; slop: contention Ok or wrong errno, EWOULDBLOCK never returned
        let key = LockKey {
            namespace: request.namespace,
            guest_inode: request.guest_inode,
        };
        let _ = (&self.cache, &self.states, key);
        Err(PluginError::Halt(
            "TODO (L3) [sp_15_host_inode_lock_broker_flock_bridge]: LockBroker::acquire needs flock delegation in kelir-drivers/kelir-transport enclave; slop: in-memory lock table, no host flock, contender succeeds".into(),
        ))
    }

    pub fn release(&self, key: LockKey) -> Result<(), PluginError> {
        // TODO (L3) [sp_15_host_inode_lock_broker_flock_bridge]: flock LOCK_UN plus host fd cache release in the enclave; slop: in-memory lock table, no host flock, contender succeeds
        let _ = key;
        Err(PluginError::Halt(
            "TODO (L3) [sp_15_host_inode_lock_broker_flock_bridge]: LockBroker::release needs flock delegation in kelir-drivers/kelir-transport enclave; slop: in-memory lock table, no host flock, contender succeeds".into(),
        ))
    }

    pub fn state(&self, key: LockKey) -> Result<LockState, PluginError> {
        let states = self
            .states
            .read()
            .map_err(|_| PluginError::Halt("lock broker state lock poisoned".into()))?;
        Ok(states.get(&key).copied().unwrap_or(LockState::Free))
    }
}

// see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
#[async_trait]
impl LockBrokerCoordination for LockBroker {
    // see: cans/05-storage-plane.md#Host-inode-lock-broker
    async fn release_transaction_locks(&self) -> Result<(), PluginError> {
        self.release_transaction_locks_for(self.namespace).await
    }

    // see: cans/05-storage-plane.md#Host-inode-lock-broker
    async fn release_transaction_locks_for(&self, namespace: [u8; 16]) -> Result<(), PluginError> {
        // TODO (L3) [sp_15_host_inode_lock_broker_flock_bridge]: close the released host fds through the enclave flock delegation; slop: in-memory lock table, no host flock, contender succeeds
        {
            let mut states = self
                .states
                .write()
                .map_err(|_| PluginError::Halt("lock broker state lock poisoned".into()))?;
            states.retain(|key, _| key.namespace != namespace);
        }
        let mut cache = self
            .cache
            .write()
            .map_err(|_| PluginError::Halt("lock broker fd cache lock poisoned".into()))?;
        cache.remove_namespace(namespace);
        Ok(())
    }
}
