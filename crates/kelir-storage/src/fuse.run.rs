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
use kelir_interface::storage::synthetic::SyntheticFile;
use kelir_interface::storage::{FuseOpcode, FuseOps, FuseReply, FuseRequest, FuseResponse};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

// see: cans/05-storage-plane.md#Interception-pipeline
pub(crate) struct RealFuseOps {
    engine: Arc<PipelineEngine>,
    synthetic_files: Vec<SyntheticFile>,
    virtual_inodes: RwLock<HashMap<PathBuf, u64>>,
    next_virtual_inode: AtomicU64,
    quota: Arc<StorageQuota>,
    lock_broker: Arc<LockBroker>,
}

impl RealFuseOps {
    pub(crate) fn new(engine: Arc<PipelineEngine>) -> Self {
        Self {
            engine: Arc::clone(&engine),
            synthetic_files: Vec::new(),
            virtual_inodes: RwLock::new(HashMap::new()),
            next_virtual_inode: AtomicU64::new(2),
            quota: Arc::new(StorageQuota::unbounded()),
            lock_broker: Arc::new(LockBroker::new(engine)),
        }
    }

    pub(crate) fn with_synthetic_files(mut self, files: Vec<SyntheticFile>) -> Self {
        self.synthetic_files = files;
        self
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

pub(crate) fn build_fuse_ops_with_files(
    engine: Arc<PipelineEngine>,
    files: Vec<SyntheticFile>,
) -> Arc<dyn FuseOps> {
    Arc::new(RealFuseOps::new(engine).with_synthetic_files(files))
}

impl FuseOps for RealFuseOps {
    fn dispatch(
        &self,
        request: &mut FuseRequest,
        ctx: &HandlerCtx,
    ) -> Result<FuseResponse, PluginError> {
        match request.opcode {
            FuseOpcode::Lookup => {
                for file in &self.synthetic_files {
                    if let Some(attr) = file.lookup_attr(&request.path)? {
                        return Ok(FuseResponse {
                            guest_inode: self.virtual_inode(&request.path)?,
                            reply: FuseReply::Inode(attr),
                        });
                    }
                }
                match self.engine.dispatch_fs_lookup(&request.path, ctx)? {
                    Verdict::Pass => Ok(FuseResponse {
                        guest_inode: request.guest_inode,
                        // TODO (L3) [sp_11_host_fifo_socket_lookup_enxio]: fill lookup payload from broker getattr; slop: lookup accepts FIFO/socket, ENXIO never returned
                        reply: FuseReply::Payload(Vec::new()),
                    }),
                    // see: cans/02-plugin-model.md#Unified-verdict
                    Verdict::Patch(PatchPayload::FsInode {
                        mode,
                        uid,
                        gid,
                        size,
                        content,
                    }) => Ok(FuseResponse {
                        guest_inode: self.virtual_inode(&request.path)?,
                        reply: FuseReply::Inode(FsInodeAttr {
                            mode: mode & !0o6000,
                            uid,
                            gid,
                            size,
                            content,
                        }),
                    }),
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
                for file in &self.synthetic_files {
                    if let Some(attr) = file.lookup_attr(&request.path)? {
                        let content = attr.content;
                        let len = content.len() as u64;
                        let start = request.offset.min(len) as usize;
                        let end = request.offset.saturating_add(request.size).min(len) as usize;
                        return Ok(FuseResponse {
                            guest_inode: self.virtual_inode(&request.path)?,
                            reply: FuseReply::Payload(content[start..end].to_vec()),
                        });
                    }
                }
                request.data = self
                    .engine
                    .dispatch_fs_read(&request.path, &request.data, ctx)?;
                Ok(FuseResponse {
                    guest_inode: request.guest_inode,
                    // TODO (L3) [sp_14_in_flight_read_masking_pattern_replace]: fill read payload from broker read at offset; slop: guest reads raw secret, host file mutated instead
                    reply: FuseReply::Payload(Vec::new()),
                })
            }
            FuseOpcode::Getattr => {
                for file in &self.synthetic_files {
                    if let Some(attr) = file.lookup_attr(&request.path)? {
                        return Ok(FuseResponse {
                            guest_inode: self.virtual_inode(&request.path)?,
                            reply: FuseReply::Inode(attr),
                        });
                    }
                }
                Err(PluginError::Halt(
                    "TODO (L3) [storage_plane]: FUSE GETATTR needs the broker stat across lower, upper, and tombstones; slop: GETATTR without lower/upper/tombstone merge, stat wrong".into(),
                ))
            }
            FuseOpcode::Readdir => Err(PluginError::Halt(
                "TODO (L3) [sp_06_host_cow_broker_dir_merge_cache]: FUSE READDIR needs the host-side dir merge of base, upper, and tombstones; slop: readdir re-merges every call, zero cache hits".into(),
            )),
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
                Err(PluginError::Halt(
                    "TODO (L3) [sp_09_reconciler_tombstone_unlink_commit]: FUSE MKDIR needs the broker upper-directory create; slop: commit Ok, tombstone or file still on host disk".into(),
                ))
            }
            FuseOpcode::Rename => Err(PluginError::Halt(
                "TODO (L3) [sp_08_reconciler_atomic_leaf_commit]: FUSE RENAME needs the broker upper-file move; slop: in-place write, no temp+rename, inode unchanged".into(),
            )),
            // TODO (L3) [sp_16_statfs_zero_crawl_constant_time]: statfs replies remaining broker quota from the plane atomics; slop: statfs crawls host tree, time grows with base
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
                    // see: cans/plugins/chaos.md#Error-and-return-semantics-(chaos)
                    FilterAction::Drop { errno: None, .. } => Ok(FuseResponse {
                        guest_inode: request.guest_inode,
                        reply: FuseReply::Written(request.size),
                    }),
                    // see: cans/05-storage-plane.md#Write-gating
                    FilterAction::Drop {
                        errno: Some(errno), ..
                    } => {
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
                    // TODO (L3) [sp_15_host_inode_lock_broker_flock_bridge]: flock LOCK_EX/LOCK_NB constants live in the enclave; slop: in-memory lock table, no host flock, contender succeeds
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
                        // TODO (L3) [sp_19_host_inode_flock_contention_ewouldblock]: fill lock payload from host flock grant; conflicts reply EWOULDBLOCK; slop: contention Ok or wrong errno, EWOULDBLOCK never returned
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
    match errno {
        Errno::Eperm => Ok(libc::EPERM),
        Errno::Enxio => Ok(libc::ENXIO),
        Errno::Ewouldblock => Ok(libc::EWOULDBLOCK),
        Errno::Eacces => Ok(libc::EACCES),
        Errno::Efault => Ok(libc::EFAULT),
        Errno::Ebusy => Ok(libc::EBUSY),
        Errno::Exdev => Ok(libc::EXDEV),
        Errno::Enospc => Ok(libc::ENOSPC),
        Errno::Erofs => Ok(libc::EROFS),
        Errno::Econnrefused => Ok(libc::ECONNREFUSED),
        Errno::Enoent => Ok(libc::ENOENT),
        Errno::Einval => Ok(libc::EINVAL),
        Errno::Enodata => Ok(libc::ENODATA),
        Errno::Epipe => Ok(libc::EPIPE),
        Errno::Eproto => Ok(libc::EPROTO),
        Errno::Econfig
        | Errno::Epolicy
        | Errno::Etoolquota
        | Errno::Ewxviolation
        | Errno::Eclockregress
        | Errno::Esecurityviolation
        | Errno::Eunknownkind => Err(PluginError::Halt(format!(
            "FUSE errno {errno} has no POSIX mapping on the storage plane"
        ))),
    }
}
