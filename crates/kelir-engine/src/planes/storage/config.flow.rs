// see: cans/05-storage-plane.md#Workspace-commit
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockBrokerWiring {
    pub scope_globs: Vec<String>,
    pub non_blocking: bool,
    // see: cans/05-storage-plane.md#Workspace-commit
    // see: cans/artifacts/examples/fixtures/kelir.yaml#storage.lock_broker.blocking_wait_timeout_ms
    // Bounded F_SETLKW waiter pool ceiling before EDEADLK: a blocking lock wait parks on
    // the waiter pool up to this many milliseconds, then fails with EDEADLK instead of
    // illegally returning EWOULDBLOCK on a POSIX blocking wait.
    pub blocking_wait_timeout_ms: u64,
}

impl Default for LockBrokerWiring {
    fn default() -> Self {
        Self {
            scope_globs: Vec::new(),
            non_blocking: false,
            blocking_wait_timeout_ms: 5000,
        }
    }
}

// see: cans/05-storage-plane.md#Virtual-ID-remapping
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IdmapConfig {
    pub guest_uid: u32,
    pub host_uid: u32,
}

// see: cans/05-storage-plane.md#Storage-architecture
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WorkspaceEncapsulation {
    #[default]
    Locked,
    Passthrough,
}

// see: cans/05-storage-plane.md#Storage-architecture
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageConfig {
    pub shared_caches: Vec<PathBuf>,
    pub lock_broker: Option<LockBrokerWiring>,
    pub idmap: Option<IdmapConfig>,
    pub encapsulation: WorkspaceEncapsulation,
    // see: cans/05-storage-plane.md#Storage-architecture
    // FUSE default_permissions mount flag: kernel-side permission checks run before
    // FUSE requests, averting the guest clone() deadlock of userspace-only checking.
    // TODO (L3) [storage_plane]: forward default_permissions to the FUSE mount; no mount
    // exists yet; slop: permission checks stay userspace-only, guest clone deadlocks.
    pub default_permissions: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            shared_caches: Vec::new(),
            lock_broker: None,
            idmap: None,
            encapsulation: WorkspaceEncapsulation::Locked,
            default_permissions: true,
        }
    }
}
