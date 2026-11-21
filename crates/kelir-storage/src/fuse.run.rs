// see: cans/05-storage-plane.md#Storage-architecture
// see: cans/05-storage-plane.md#Write-gating
// see: cans/05-storage-plane.md#Host-inode-lock-broker
// see: cans/05-storage-plane.md#Synthetic-FUSE-inodes
// see: cans/02-plugin-model.md#Hook-return-semantics
use crate::lock_broker::LockBroker;
use kelir_engine::dispatch_plugin::PipelineEngine;
use kelir_engine::planes::storage::storage_quota::StorageQuota;
use kelir_interface::plugin_model::{HandlerCtx, PluginError};
use kelir_interface::protocol_wire::Errno;
use kelir_interface::storage::{FuseOps, FuseRequest, FuseResponse};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::AtomicU64;

// see: cans/05-storage-plane.md#Interception-pipeline
// TODO (L3) [storage_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
pub(crate) struct RealFuseOps {
    engine: Arc<PipelineEngine>,
    virtual_inodes: RwLock<HashMap<PathBuf, u64>>,
    next_virtual_inode: AtomicU64,
    quota: Arc<StorageQuota>,
    lock_broker: Arc<LockBroker>,
}

// TODO (L3) [storage_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
impl RealFuseOps {
    pub(crate) fn new(engine: Arc<PipelineEngine>) -> Self {
        Self {
            engine: Arc::clone(&engine),
            virtual_inodes: RwLock::new(HashMap::new()),
            next_virtual_inode: AtomicU64::new(2),
            quota: Arc::new(StorageQuota::unbounded()),
            lock_broker: Arc::new(LockBroker::new(engine)),
        }
    }

    // see: cans/05-storage-plane.md#Synthetic-FUSE-inodes
    fn virtual_inode(&self, _path: &Path) -> Result<u64, PluginError> {
        // TODO (L3) [storage_plane]: allocate or reuse the synthetic virtual inode for path in the virtual inode map.; slop: virtual inode never allocated, synthetic paths collide on inode zero.
        Err(PluginError::Halt(
            "TODO (L3): RealFuseOps::virtual_inode needs the synthetic virtual inode allocation"
                .into(),
        ))
    }
}

pub(crate) fn build_fuse_ops(engine: Arc<PipelineEngine>) -> Arc<dyn FuseOps> {
    Arc::new(RealFuseOps::new(engine))
}

impl FuseOps for RealFuseOps {
    fn dispatch(
        &self,
        _request: &mut FuseRequest,
        _ctx: &HandlerCtx,
    ) -> Result<FuseResponse, PluginError> {
        // L3-GAP: [sp_11_host_fifo_socket_lookup_enxio]: fill lookup payload from broker getattr; slop: lookup accepts FIFO/socket, ENXIO never returned
        // L3-GAP: [sp_14_in_flight_read_masking_pattern_replace]: fill read payload from broker read at offset; slop: guest reads raw secret, host file mutated instead
        // L3-GAP: [vr_10_virtio_fs_fuse_req_channel]: marshal fuse_setattr_in (mode, size) from the virtqueue bytes; slop: FUSE reply never marshalled, virtqueue never consumed
        // L3-GAP: [vr_10_virtio_fs_fuse_req_channel]: fill setattr payload from the broker attr commit; slop: FUSE reply never marshalled, virtqueue never consumed
        // L3-GAP: [vr_10_virtio_fs_fuse_req_channel]: fill readlink payload with the broker-resolved target bytes; slop: FUSE reply never marshalled, virtqueue never consumed
        // L3-GAP: [vr_10_virtio_fs_fuse_req_channel]: fill symlink payload from the broker link commit; slop: FUSE reply never marshalled, virtqueue never consumed
        // L3-GAP: [vr_10_virtio_fs_fuse_req_channel]: marshal fuse_mkdir_in mode from the virtqueue bytes; slop: FUSE reply never marshalled, virtqueue never consumed
        // L3-GAP: [sp_09_reconciler_tombstone_unlink_commit]: broker upper-directory create commits at reconcile; slop: commit Ok, tombstone or file still on host disk
        // L3-GAP: [sp_16_statfs_zero_crawl_constant_time]: statfs replies remaining broker quota from the plane atomics; slop: statfs crawls host tree, time grows with base
        // L3-GAP: [sp_15_host_inode_lock_broker_flock_bridge]: flock LOCK_EX/LOCK_NB constants live in the enclave; slop: in-memory lock table, no host flock, contender succeeds
        // L3-GAP: [sp_19_host_inode_flock_contention_ewouldblock]: fill lock payload from host flock grant; conflicts reply EWOULDBLOCK; slop: contention Ok or wrong errno, EWOULDBLOCK never returned
        // TODO (L3) [storage_plane]: dispatch the FUSE request through the pipeline engine and marshal the reply per opcode.; slop: FUSE requests never dispatched, every opcode returns the stub reply.
        // TODO (L3) [storage_plane]: mount the FUSE filesystem with the default_permissions flag so the guest kernel resolves permission checks in-kernel instead of round-tripping every lookup into the FUSE dispatch loop, averting the guest clone deadlock.; slop: default_permissions omitted, every access check re-enters the single-threaded FUSE loop and a guest clone blocks forever on it.
        Err(PluginError::Halt(
            "TODO (L3): FuseOps::dispatch needs the per-opcode pipeline dispatch and FUSE reply marshalling".into(),
        ))
    }
}

// see: cans/05-storage-plane.md#Write-gating
// see: cans/plugins/chaos.md#Error-and-return-semantics-(chaos)
// TODO (L3) [storage_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
fn drop_errno(_errno: Errno) -> Result<i32, PluginError> {
    // TODO (L3) [storage_plane]: convert the interposer errno to the host OS errno for the FUSE reply.; slop: errno never converted, FUSE replies carry the wrong error code.
    Err(PluginError::Halt(
        "TODO (L3): drop_errno needs the errno conversion".into(),
    ))
}

// see: cans/05-storage-plane.md#S-IFSOCK-binds-intercepted:-/tmp,-/var/run,-/run-stay-on-guest-tmpfs
// TODO (L3) [storage_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
fn tmpfs_socket_allowed(_path: &Path) -> bool {
    // TODO (L3) [storage_plane]: decide whether path is on a guest tmpfs mount where socket inodes are allowed.; slop: tmpfs check never runs, socket inodes allowed on mounts that forbid them.
    false
}
