// see: cans/05-storage-plane.md#Storage-architecture
// see: cans/05-storage-plane.md#Write-gating
// see: cans/05-storage-plane.md#Host-inode-lock-broker
// see: cans/05-storage-plane.md#Synthetic-FUSE-inodes
// see: cans/02-plugin-model.md#Hook-return-semantics
use crate::lock_broker::{LockBroker, LockKind, LockRequest};
use kelir_engine::dispatch_plugin::PipelineEngine;
use kelir_engine::planes::storage::storage_quota::StorageQuota;
use kelir_interface::plugin_model::{FsContext, FsInodeAttr, HandlerCtx, PluginError};
use kelir_interface::protocol_wire::{Errno, FilterAction, PatchPayload, Verdict};
use kelir_interface::storage::{FuseOpcode, FuseOps, FuseReply, FuseRequest, FuseResponse};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

// see: cans/05-storage-plane.md#Interception-pipeline
pub(crate) struct RealFuseOps {
    engine: Arc<PipelineEngine>,
    virtual_inodes: RwLock<HashMap<PathBuf, u64>>,
    next_virtual_inode: AtomicU64,
    quota: Arc<StorageQuota>,
    lock_broker: Arc<LockBroker>,
}

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
    fn virtual_inode(&self, path: &Path) -> Result<u64, PluginError> {
        let mut inodes = self
            .virtual_inodes
            .write()
            .map_err(|_| PluginError::Halt("fuse virtual inode map lock poisoned".into()))?;
        if let Some(ino) = inodes.get(path) {
            return Ok(*ino);
        }
        let ino = self.next_virtual_inode.fetch_add(1, Ordering::SeqCst);
        inodes.insert(path.to_path_buf(), ino);
        Ok(ino)
    }
}

pub(crate) fn build_fuse_ops(engine: Arc<PipelineEngine>) -> Arc<dyn FuseOps> {
    Arc::new(RealFuseOps::new(engine))
}

impl FuseOps for RealFuseOps {
    fn dispatch(
        &self,
        request: &mut FuseRequest,
        ctx: &HandlerCtx,
    ) -> Result<FuseResponse, PluginError> {
        match request.opcode {
            FuseOpcode::Lookup => {
                match self.engine.dispatch_fs_lookup(&request.path, ctx)? {
                    Verdict::Pass => Ok(FuseResponse {
                        guest_inode: request.guest_inode,
                        // L3-GAP: [sp_11_host_fifo_socket_lookup_enxio]: fill lookup payload from broker getattr; slop: lookup accepts FIFO/socket, ENXIO never returned
                        reply: FuseReply::Payload(Vec::new()),
                    }),
                    // see: cans/02-plugin-model.md#Unified-verdict
                    Verdict::Patch(PatchPayload::FsInode {
                        mode,
                        uid,
                        gid,
                        size,
                        content,
                    }) => {
                        // see: cans/05-storage-plane.md#S-IFSOCK-binds-intercepted:-/tmp,-/var/run,-/run-stay-on-guest-tmpfs
                        if (mode & libc::S_IFMT) == libc::S_IFSOCK && !tmpfs_socket_allowed(&request.path)
                        {
                            return Err(PluginError::Halt(
                                "FUSE LOOKUP denied: socket inode outside guest tmpfs mounts (EPERM)".into(),
                            ));
                        }
                        Ok(FuseResponse {
                            guest_inode: self.virtual_inode(&request.path)?,
                            reply: FuseReply::Inode(FsInodeAttr {
                                mode: mode & !0o6000,
                                uid,
                                gid,
                                size,
                                content,
                            }),
                        })
                    }
                    Verdict::Patch(_) => Err(PluginError::Halt(
                        "FUSE LOOKUP Patch payload is not an FsInode".into(),
                    )),
                    Verdict::Drop { errno, reason } => Err(PluginError::Halt(format!(
                        "FUSE LOOKUP denied: {reason} ({errno:?})"
                    ))),
                    Verdict::Suspend { .. } => Err(PluginError::Halt(
                        "FUSE LOOKUP interposer returned Suspend, unsupported for lookup".into(),
                    )),
                    Verdict::Abort { reason } => Err(PluginError::Halt(format!(
                        "FUSE LOOKUP aborted by interposer: {reason}"
                    ))),
                    _ => Err(PluginError::Halt(
                        "FUSE LOOKUP interposer returned an unknown verdict".into(),
                    )),
                }
            }
            FuseOpcode::Read => {
                request.data = self
                    .engine
                    .dispatch_fs_read(&request.path, &request.data, ctx)?;
                Ok(FuseResponse {
                    guest_inode: request.guest_inode,
                    // L3-GAP: [sp_14_in_flight_read_masking_pattern_replace]: fill read payload from broker read at offset; slop: guest reads raw secret, host file mutated instead
                    reply: FuseReply::Payload(Vec::new()),
                })
            }
            FuseOpcode::Getattr => {
                Err(PluginError::Halt(
                    "TODO (L3) [storage_plane]: FUSE GETATTR needs the broker stat across lower, upper, and tombstones; slop: GETATTR without lower/upper/tombstone merge, stat wrong".into(),
                ))
            }
            FuseOpcode::Setattr => {
                // L3-GAP: [vr_10_virtio_fs_fuse_req_channel]: marshal fuse_setattr_in (mode, size) from the virtqueue bytes; slop: FUSE reply never marshalled, virtqueue never consumed
                // TODO (L3) [storage_plane]: parse the fuse_setattr_in mode/size fields from request.data instead of the head bytes; slop: setattr runs with a stub mode/size, chmod/ftruncate policy blind
                let mode = request
                    .data
                    .first_chunk::<4>()
                    .map(|bytes| u32::from_le_bytes(*bytes))
                    .unwrap_or(0);
                let size = request.size;
                match self
                    .engine
                    .dispatch_fs_setattr(&request.path, mode, size, ctx)?
                {
                    Verdict::Pass => Ok(FuseResponse {
                        guest_inode: request.guest_inode,
                        // L3-GAP: [vr_10_virtio_fs_fuse_req_channel]: fill setattr payload from the broker attr commit; slop: FUSE reply never marshalled, virtqueue never consumed
                        reply: FuseReply::Payload(Vec::new()),
                    }),
                    // see: cans/05-storage-plane.md#Write-gating
                    Verdict::Drop { errno, .. } => Ok(FuseResponse {
                        guest_inode: request.guest_inode,
                        reply: FuseReply::Errno(-errno.to_os_errno()),
                    }),
                    Verdict::Patch(_) => Err(PluginError::Halt(
                        "FUSE SETATTR interposer returned Patch, unsupported for setattr".into(),
                    )),
                    Verdict::Suspend { .. } => Err(PluginError::Halt(
                        "FUSE SETATTR interposer returned Suspend, unsupported for setattr".into(),
                    )),
                    Verdict::Abort { reason } => Err(PluginError::Halt(format!(
                        "FUSE SETATTR aborted by interposer: {reason}"
                    ))),
                    _ => Err(PluginError::Halt(
                        "FUSE SETATTR interposer returned an unknown verdict".into(),
                    )),
                }
            }
            FuseOpcode::Readlink => match self.engine.dispatch_fs_readlink(&request.path, ctx)? {
                Verdict::Pass => Ok(FuseResponse {
                    guest_inode: request.guest_inode,
                    // L3-GAP: [vr_10_virtio_fs_fuse_req_channel]: fill readlink payload with the broker-resolved target bytes; slop: FUSE reply never marshalled, virtqueue never consumed
                    reply: FuseReply::Payload(Vec::new()),
                }),
                // see: cans/05-storage-plane.md#Write-gating
                Verdict::Drop { errno, .. } => Ok(FuseResponse {
                    guest_inode: request.guest_inode,
                    reply: FuseReply::Errno(-errno.to_os_errno()),
                }),
                Verdict::Patch(_) => Err(PluginError::Halt(
                    "FUSE READLINK interposer returned Patch, unsupported for readlink".into(),
                )),
                Verdict::Suspend { .. } => Err(PluginError::Halt(
                    "FUSE READLINK interposer returned Suspend, unsupported for readlink".into(),
                )),
                Verdict::Abort { reason } => Err(PluginError::Halt(format!(
                    "FUSE READLINK aborted by interposer: {reason}"
                ))),
                _ => Err(PluginError::Halt(
                    "FUSE READLINK interposer returned an unknown verdict".into(),
                )),
            },
            FuseOpcode::Symlink => {
                // TODO (L3) [storage_plane]: marshal fuse_symlink_in (target, name) from request.data into target/link paths; slop: symlink target/link split is a stub, traversal policy blind
                let target = request.path.clone();
                let link = request.path.clone();
                match self
                    .engine
                    .dispatch_fs_symlink(target.as_path(), link.as_path(), ctx)?
                {
                    Verdict::Pass => Ok(FuseResponse {
                        guest_inode: self.virtual_inode(&request.path)?,
                        // L3-GAP: [vr_10_virtio_fs_fuse_req_channel]: fill symlink payload from the broker link commit; slop: FUSE reply never marshalled, virtqueue never consumed
                        reply: FuseReply::Payload(Vec::new()),
                    }),
                    // see: cans/05-storage-plane.md#Write-gating
                    Verdict::Drop { errno, .. } => Ok(FuseResponse {
                        guest_inode: request.guest_inode,
                        reply: FuseReply::Errno(-errno.to_os_errno()),
                    }),
                    Verdict::Patch(_) => Err(PluginError::Halt(
                        "FUSE SYMLINK interposer returned Patch, unsupported for symlink".into(),
                    )),
                    Verdict::Suspend { .. } => Err(PluginError::Halt(
                        "FUSE SYMLINK interposer returned Suspend, unsupported for symlink".into(),
                    )),
                    Verdict::Abort { reason } => Err(PluginError::Halt(format!(
                        "FUSE SYMLINK aborted by interposer: {reason}"
                    ))),
                    _ => Err(PluginError::Halt(
                        "FUSE SYMLINK interposer returned an unknown verdict".into(),
                    )),
                }
            }
            FuseOpcode::Readdir => {
                // see: cans/05-storage-plane.md#Interception-pipeline
                let mut entries: Vec<kelir_interface::storage::DirEntry> = Vec::new();
                match self
                    .engine
                    .dispatch_fs_readdir(&request.path, &mut entries, ctx)?
                {
                    // TODO (L3) [sp_06_host_cow_broker_dir_merge_cache]: marshal the merged base/upper/tombstone entries into the FUSE READDIR reply payload; slop: readdir re-merges every call, zero cache hits
                    FilterAction::Forward => Ok(FuseResponse {
                        guest_inode: request.guest_inode,
                        reply: FuseReply::Entries(Vec::new()),
                    }),
                    // see: cans/05-storage-plane.md#Write-gating
                    FilterAction::Drop { errno, .. } => {
                        let errno = drop_errno(errno)?;
                        Ok(FuseResponse {
                            guest_inode: request.guest_inode,
                            reply: FuseReply::Errno(-errno),
                        })
                    }
                    FilterAction::Suspend { .. } => Err(PluginError::Halt(
                        "FUSE READDIR interposer returned Suspend, unsupported for readdir".into(),
                    )),
                    FilterAction::Abort { reason } => Err(PluginError::Halt(format!(
                        "FUSE READDIR aborted by interposer: {reason}"
                    ))),
                    _ => Err(PluginError::Halt(
                        "FUSE READDIR interposer returned a filter action outside the known FilterAction variants".into(),
                    )),
                }
            }
            FuseOpcode::Unlink => Err(PluginError::Halt(
                "TODO (L3) [sp_09_reconciler_tombstone_unlink_commit]: FUSE UNLINK needs the broker tombstone write; slop: commit Ok, tombstone or file still on host disk".into(),
            )),
            FuseOpcode::Mkdir => {
                // see: cans/05-storage-plane.md#Storage-quotas
                if self.quota.check_quota(0, 1).is_err() {
                    return Ok(FuseResponse {
                        guest_inode: request.guest_inode,
                        reply: FuseReply::Errno(-libc::ENOSPC),
                    });
                }
                // see: cans/05-storage-plane.md#S-IFSOCK-binds-intercepted:-/tmp,-/var/run,-/run-stay-on-guest-tmpfs
                if !tmpfs_socket_allowed(&request.path) {
                    return Err(PluginError::Halt(
                        "FUSE MKDIR denied: socket inode outside guest tmpfs mounts (EPERM)".into(),
                    ));
                }
                // L3-GAP: [vr_10_virtio_fs_fuse_req_channel]: marshal fuse_mkdir_in mode from the virtqueue bytes; slop: FUSE reply never marshalled, virtqueue never consumed
                // TODO (L3) [storage_plane]: parse the fuse_mkdir_in mode field from request.data instead of the head bytes; slop: mkdir runs with a stub mode, creation policy blind
                let mode = request
                    .data
                    .first_chunk::<4>()
                    .map(|bytes| u32::from_le_bytes(*bytes))
                    .unwrap_or(0);
                match self.engine.dispatch_fs_mkdir(&request.path, mode, ctx)? {
                    Verdict::Pass => Ok(FuseResponse {
                        guest_inode: self.virtual_inode(&request.path)?,
                        // L3-GAP: [sp_09_reconciler_tombstone_unlink_commit]: broker upper-directory create commits at reconcile; slop: commit Ok, tombstone or file still on host disk
                        reply: FuseReply::Payload(Vec::new()),
                    }),
                    // see: cans/05-storage-plane.md#Write-gating
                    Verdict::Drop { errno, .. } => Ok(FuseResponse {
                        guest_inode: request.guest_inode,
                        reply: FuseReply::Errno(-errno.to_os_errno()),
                    }),
                    Verdict::Patch(_) => Err(PluginError::Halt(
                        "FUSE MKDIR interposer returned Patch, unsupported for mkdir".into(),
                    )),
                    Verdict::Suspend { .. } => Err(PluginError::Halt(
                        "FUSE MKDIR interposer returned Suspend, unsupported for mkdir".into(),
                    )),
                    Verdict::Abort { reason } => Err(PluginError::Halt(format!(
                        "FUSE MKDIR aborted by interposer: {reason}"
                    ))),
                    _ => Err(PluginError::Halt(
                        "FUSE MKDIR interposer returned an unknown verdict".into(),
                    )),
                }
            }
            FuseOpcode::Rename => Err(PluginError::Halt(
                "TODO (L3) [sp_08_reconciler_atomic_leaf_commit]: FUSE RENAME needs the broker upper-file move; slop: in-place write, no temp+rename, inode unchanged".into(),
            )),
            // L3-GAP: [sp_16_statfs_zero_crawl_constant_time]: statfs replies remaining broker quota from the plane atomics; slop: statfs crawls host tree, time grows with base
            FuseOpcode::Statfs => Err(PluginError::Halt(
                "TODO (L3) [sp_16_statfs_zero_crawl_constant_time]: FUSE STATFS needs the remaining-quota reply from the plane atomics; slop: statfs crawls host tree, time grows with base"
                    .into(),
            )),
            FuseOpcode::Write => {
                // see: cans/05-storage-plane.md#Storage-quotas
                if self
                    .quota
                    .check_quota(request.data.len() as u64, 0)
                    .is_err()
                {
                    return Ok(FuseResponse {
                        guest_inode: request.guest_inode,
                        reply: FuseReply::Errno(-libc::ENOSPC),
                    });
                }
                match self.engine.dispatch_fs_write(
                    request.path.as_path(),
                    &mut request.data,
                    request.pid,
                    &ctx.clone().with_fs_ctx(FsContext {
                        inode: request.guest_inode,
                        path: request.path.clone(),
                        journal: 0,
                        offset: 0,
                    }),
                )? {
                    // see: cans/05-storage-plane.md#FUSE-opcode-table
                    FilterAction::Forward => Ok(FuseResponse {
                        guest_inode: request.guest_inode,
                        reply: FuseReply::Written(request.size),
                    }),
                    // see: cans/05-storage-plane.md#Write-gating
                    FilterAction::Drop { errno, .. } => {
                        let errno = drop_errno(errno)?;
                        Ok(FuseResponse {
                            guest_inode: request.guest_inode,
                            reply: FuseReply::Errno(-errno),
                        })
                    }
                    FilterAction::Suspend { .. } => Err(PluginError::Halt(
                        "FUSE WRITE interposer returned Suspend, unsupported for write".into(),
                    )),
                    FilterAction::Abort { reason } => Err(PluginError::Halt(format!(
                        "FUSE WRITE aborted by interposer: {reason}"
                    ))),
                    _ => Err(PluginError::Halt(
                        "FUSE WRITE interposer returned a filter action outside the known FilterAction variants".into(),
                    )),
                }
            }
            FuseOpcode::Setlk | FuseOpcode::Setlkw => {
                // see: cans/05-storage-plane.md#Host-inode-lock-broker
                let lock_request = LockRequest {
                    namespace: self.lock_broker.namespace(),
                    guest_inode: request.guest_inode,
                    host_path: request.path.clone(),
                    kind: LockKind::Exclusive,
                    blocking: matches!(request.opcode, FuseOpcode::Setlkw),
                    pid: request.pid,
                };
                self.lock_broker.acquire(&lock_request, ctx)?;
                let flock_cmd = match request.opcode {
                    // L3-GAP: [sp_15_host_inode_lock_broker_flock_bridge]: flock LOCK_EX/LOCK_NB constants live in the enclave; slop: in-memory lock table, no host flock, contender succeeds
                    FuseOpcode::Setlk => 6,
                    FuseOpcode::Setlkw => 2,
                    _ => 2,
                };
                match self
                    .engine
                    .dispatch_fs_lock(&request.path, flock_cmd, 1, request.pid, ctx)?
                {
                    Verdict::Pass => Ok(FuseResponse {
                        guest_inode: request.guest_inode,
                        // L3-GAP: [sp_19_host_inode_flock_contention_ewouldblock]: fill lock payload from host flock grant; conflicts reply EWOULDBLOCK; slop: contention Ok or wrong errno, EWOULDBLOCK never returned
                        reply: FuseReply::Payload(Vec::new()),
                    }),
                    Verdict::Drop { errno, reason } => Err(PluginError::Halt(format!(
                        "FUSE SETLK denied: {reason} ({errno:?})"
                    ))),
                    Verdict::Patch(_) => Err(PluginError::Halt(
                        "FUSE SETLK interposer returned Patch, unsupported for locks".into(),
                    )),
                    Verdict::Suspend { .. } => Err(PluginError::Halt(
                        "FUSE SETLK interposer returned Suspend, unsupported for locks".into(),
                    )),
                    Verdict::Abort { reason } => Err(PluginError::Halt(format!(
                        "FUSE SETLK aborted by interposer: {reason}"
                    ))),
                    _ => Err(PluginError::Halt(
                        "FUSE SETLK interposer returned an unknown verdict".into(),
                    )),
                }
            }
        }
    }
}

// see: cans/05-storage-plane.md#Write-gating
// see: cans/plugins/chaos.md#Error-and-return-semantics-(chaos)
fn drop_errno(errno: Errno) -> Result<i32, PluginError> {
    // see: cans/02-plugin-model.md#Unified-verdict
    Ok(errno.to_os_errno())
}

// see: cans/05-storage-plane.md#S-IFSOCK-binds-intercepted:-/tmp,-/var/run,-/run-stay-on-guest-tmpfs
fn tmpfs_socket_allowed(path: &Path) -> bool {
    path.starts_with("/tmp") || path.starts_with("/var/run") || path.starts_with("/run")
}
