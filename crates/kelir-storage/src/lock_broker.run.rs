// see: cans/05-storage-plane.md#Host-inode-lock-broker
use async_trait::async_trait;
pub use kelir_engine::planes::storage::LockBrokerWiring;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{HandlerCtx, LockBrokerCoordination};
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
pub(crate) struct HostFdEntry {
    pub host_inode: u64,
    pub fd: i32,
    pub refcount: u32,
}

#[derive(Default)]
// TODO (L3) [storage_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
pub(crate) struct HostFdCache {
    entries: HashMap<LockKey, HostFdEntry>,
}

// TODO (L3) [storage_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
impl HostFdCache {
    // see: cans/05-storage-plane.md#Host-inode-lock-broker
    pub(crate) fn remove_namespace(&mut self, _namespace: [u8; 16]) {
        // TODO (L3) [storage_plane]: drop every cached host fd entry in namespace.; slop: cached host fds never dropped, released locks keep their fds open.
    }
}

// TODO (L3) [storage_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
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
            namespace: Default::default(),
            wiring,
        }
    }

    // see: cans/05-storage-plane.md#Host-inode-lock-broker
    pub fn wiring(&self) -> &LockBrokerWiring {
        &self.wiring
    }

    pub fn with_namespace(self, namespace: [u8; 16]) -> Self {
        Self { namespace, ..self }
    }

    // see: cans/05-storage-plane.md#Host-inode-lock-broker
    pub fn namespace(&self) -> [u8; 16] {
        self.namespace
    }

    pub fn acquire(
        &self,
        _request: &LockRequest,
        _ctx: &HandlerCtx,
    ) -> Result<LockGrant, PluginError> {
        // L3-GAP: [storage_plane]: match request.host_path against wiring.scope_globs before mediation; undeclared paths deny with EPERM; slop: undeclared path mediated, EPERM never denied
        // L3-GAP: [sp_15_host_inode_lock_broker_flock_bridge]: flock LOCK_SH/LOCK_EX/LOCK_NB constants live in the enclave; slop: in-memory lock table, no host flock, contender succeeds
        // L3-GAP: [sp_19_host_inode_flock_contention_ewouldblock]: host fd cache lookup, flock delegation, EWOULDBLOCK on conflict; slop: contention Ok or wrong errno, EWOULDBLOCK never returned
        // TODO (L3) [sp_19_host_inode_flock_contention_ewouldblock]: split SETLK (EWOULDBLOCK on conflict) from SETLKW (bounded waiter pool, EDEADLK on timeout) via acquire_setlk/acquire_setlkw.; slop: SETLKW returns EWOULDBLOCK like SETLK, blocking waiters break cargo, git, sqlite and npm.
        // TODO (L3) [storage_plane]: mediate the lock request through the interposer and the host flock delegation.; slop: lock mediated in memory only, no host flock and a contender lock succeeds.
        Err(PluginError::Halt(
            "TODO (L3) [sp_15_host_inode_lock_broker_flock_bridge]: LockBroker::acquire needs flock delegation in kelir-drivers/kelir-transport enclave; slop: in-memory lock table, no host flock, contender succeeds".into(),
        ))
    }

    // see: cans/05-storage-plane.md#Host-inode-lock-broker
    pub fn acquire_setlk(
        &self,
        _request: &LockRequest,
        _ctx: &HandlerCtx,
    ) -> Result<LockGrant, PluginError> {
        // TODO (L3) [sp_19_host_inode_flock_contention_ewouldblock]: delegate the non-blocking SETLK to host flock and return EWOULDBLOCK on conflict with no waiter parked.; slop: SETLK contention returns Ok, two holders take the same lock.
        Err(PluginError::Halt(
            "TODO (L3) [sp_19_host_inode_flock_contention_ewouldblock]: LockBroker::acquire_setlk needs host flock LOCK_NB delegation; slop: contention Ok or wrong errno, EWOULDBLOCK never returned".into(),
        ))
    }

    // see: cans/05-storage-plane.md#Host-inode-lock-broker
    pub fn acquire_setlkw(
        &self,
        _request: &LockRequest,
        _ctx: &HandlerCtx,
    ) -> Result<LockGrant, PluginError> {
        // TODO (L3) [sp_19_host_inode_flock_contention_ewouldblock]: park the blocking SETLKW on the bounded waiter pool and fail with EDEADLK after self.wiring.blocking_wait_timeout_ms.; slop: SETLKW returns EWOULDBLOCK, blocking waiters break cargo, git, sqlite and npm.
        Err(PluginError::Halt(
            "TODO (L3) [sp_19_host_inode_flock_contention_ewouldblock]: LockBroker::acquire_setlkw needs the bounded waiter pool; slop: waiter blocks the host ring thread, the broker stalls".into(),
        ))
    }

    pub fn release(&self, _key: LockKey) -> Result<(), PluginError> {
        // L3-GAP: [sp_15_host_inode_lock_broker_flock_bridge]: flock LOCK_UN plus host fd cache release in the enclave; slop: in-memory lock table, no host flock, contender succeeds
        // TODO (L3) [storage_plane]: release the lock and its host fd through the flock delegation.; slop: release drops the memory entry only, the host flock stays held and the next acquirer blocks forever.
        Err(PluginError::Halt(
            "TODO (L3) [sp_15_host_inode_lock_broker_flock_bridge]: LockBroker::release needs flock delegation in kelir-drivers/kelir-transport enclave; slop: in-memory lock table, no host flock, contender succeeds".into(),
        ))
    }

    pub fn state(&self, _key: LockKey) -> Result<LockState, PluginError> {
        // TODO (L3) [storage_plane]: read the current lock state for key from the broker state table.; slop: lock state always reads free, two holders take the same lock.
        Err(PluginError::Halt(
            "TODO (L3): LockBroker::state needs the broker state table read".into(),
        ))
    }
}

// see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
#[async_trait]
impl LockBrokerCoordination for LockBroker {
    // see: cans/05-storage-plane.md#Host-inode-lock-broker
    async fn release_transaction_locks(&self) -> Result<(), PluginError> {
        // TODO (L3) [storage_plane]: release every transaction lock held in this broker namespace.; slop: transaction locks never released, aborted transactions hold locks forever.
        Err(PluginError::Halt(
            "TODO (L3): LockBroker::release_transaction_locks needs the namespace lock release"
                .into(),
        ))
    }

    // see: cans/05-storage-plane.md#Host-inode-lock-broker
    async fn release_transaction_locks_for(&self, _namespace: [u8; 16]) -> Result<(), PluginError> {
        // L3-GAP: [sp_15_host_inode_lock_broker_flock_bridge]: close the released host fds through the enclave flock delegation; slop: in-memory lock table, no host flock, contender succeeds
        // TODO (L3) [storage_plane]: release every transaction lock in namespace and close the released host fds.; slop: transaction locks released without closing fds, host fds leak per transaction.
        Err(PluginError::Halt(
            "TODO (L3): LockBroker::release_transaction_locks_for needs the namespace lock release and host fd close".into(),
        ))
    }
}
