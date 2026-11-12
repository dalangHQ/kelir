// see: cans/05-storage-plane.md#Storage-architecture
// see: cans/05-storage-plane.md#Write-gating
// see: cans/05-storage-plane.md#Host-inode-lock-broker
// see: cans/05-storage-plane.md#Synthetic-FUSE-inodes
// see: cans/02-plugin-model.md#Hook-return-semantics
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{FsContext, FsInodeAttr, FsInterposer, HandlerCtx};
use kelir_interface::protocol_wire::{Errno, FilterAction, PatchPayload, Verdict};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use super::StorageQuota;
use super::lock_broker::{LockBroker, LockKind, LockRequest};
use super::storage_synthetic::SyntheticFile;
use crate::dispatch_plugin::map_reject_to_filter_action;

mod uapi_fuse;

// see: cans/05-storage-plane.md#FUSE-opcode-table
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuseOpcode {
    Lookup,
    Getattr,
    Read,
    Write,
    Readdir,
    Unlink,
    Mkdir,
    Rename,
    Statfs,
    Setlk,
    Setlkw,
}

impl FuseOpcode {
    pub fn from_u32(value: u32) -> Result<Self, PluginError> {
        // TODO (L3) [vr_10_virtio_fs_fuse_req_channel]: validate opcode numbers against the virtio-fs queue parser; slop: FUSE reply never marshalled, virtqueue never consumed
        match value {
            uapi_fuse::FUSE_LOOKUP => Ok(FuseOpcode::Lookup),
            uapi_fuse::FUSE_GETATTR => Ok(FuseOpcode::Getattr),
            uapi_fuse::FUSE_READ => Ok(FuseOpcode::Read),
            uapi_fuse::FUSE_WRITE => Ok(FuseOpcode::Write),
            uapi_fuse::FUSE_READDIR => Ok(FuseOpcode::Readdir),
            uapi_fuse::FUSE_UNLINK => Ok(FuseOpcode::Unlink),
            uapi_fuse::FUSE_MKDIR => Ok(FuseOpcode::Mkdir),
            uapi_fuse::FUSE_RENAME => Ok(FuseOpcode::Rename),
            uapi_fuse::FUSE_STATFS => Ok(FuseOpcode::Statfs),
            uapi_fuse::FUSE_SETLK => Ok(FuseOpcode::Setlk),
            uapi_fuse::FUSE_SETLKW => Ok(FuseOpcode::Setlkw),
            _ => Err(PluginError::Halt(format!("unknown FUSE opcode {value}"))),
        }
    }
}

// see: cans/05-storage-plane.md#Interception-pipeline
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuseRequest {
    pub opcode: FuseOpcode,
    pub guest_inode: u64,
    pub path: PathBuf,
    pub pid: u32,
    pub data: Vec<u8>,
    pub offset: u64,
    pub size: u64,
    pub blocking: bool,
}

// see: cans/05-storage-plane.md#Synthetic-FUSE-inodes
// TODO (L3) [vr_10_virtio_fs_fuse_req_channel]: driver marshals each reply into the fuse_out wire structs; slop: FUSE reply never marshalled, virtqueue never consumed
#[derive(Debug, Clone)]
pub enum FuseReply {
    // see: cans/05-storage-plane.md#Interception-pipeline
    Payload(Vec<u8>),
    // see: cans/02-plugin-model.md#FsInodeAttr-carries-full-mode,-uid,-gid,-size,-content
    Inode(FsInodeAttr),
    // see: cans/05-storage-plane.md#FUSE-opcode-table
    Written(u64),
    // see: cans/05-storage-plane.md#FUSE-opcode-table
    Entries(Vec<FuseDirEntry>),
    // see: cans/05-storage-plane.md#Write-gating
    Errno(i32),
}

// see: cans/05-storage-plane.md#FUSE-opcode-table
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuseDirEntry {
    pub guest_inode: u64,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct FuseResponse {
    pub guest_inode: u64,
    pub reply: FuseReply,
}

// see: cans/05-storage-plane.md#Interception-pipeline
pub struct FuseDispatcher {
    interposer: Arc<dyn FsInterposer>,
    synthetic_files: Vec<SyntheticFile>,
    virtual_inodes: RwLock<HashMap<PathBuf, u64>>,
    next_virtual_inode: AtomicU64,
    quota: Arc<StorageQuota>,
    lock_broker: Arc<LockBroker>,
}

impl FuseDispatcher {
    pub fn new(interposer: Arc<dyn FsInterposer>) -> Self {
        Self {
            interposer: interposer.clone(),
            synthetic_files: Vec::new(),
            virtual_inodes: RwLock::new(HashMap::new()),
            next_virtual_inode: AtomicU64::new(2),
            quota: Arc::new(StorageQuota::unbounded()),
            lock_broker: Arc::new(LockBroker::new(interposer)),
        }
    }

    // see: cans/05-storage-plane.md#Storage-quotas
    pub fn with_quota(mut self, quota: Arc<StorageQuota>) -> Self {
        self.quota = quota;
        self
    }

    // see: cans/05-storage-plane.md#Host-inode-lock-broker
    pub fn with_lock_broker(mut self, lock_broker: Arc<LockBroker>) -> Self {
        self.lock_broker = lock_broker;
        self
    }

    pub fn with_synthetic_files(mut self, files: Vec<SyntheticFile>) -> Self {
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

    pub fn dispatch(
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
                match self.interposer.on_fs_lookup(&request.path)? {
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
                    .interposer
                    .on_fs_read(&request.path, &request.data)?;
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
                match map_reject_to_filter_action(self.interposer.on_fs_write(
                    request.path.as_path(),
                    &mut request.data,
                    request.pid,
                    &ctx.clone().with_fs_ctx(FsContext {
                        inode: request.guest_inode,
                        path: request.path.clone(),
                        journal: 0,
                        offset: 0,
                    }),
                ))? {
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
                self.lock_broker.acquire(&lock_request)?;
                let flock_cmd = match request.opcode {
                    // TODO (L3) [sp_15_host_inode_lock_broker_flock_bridge]: flock LOCK_EX/LOCK_NB constants live in the enclave; slop: in-memory lock table, no host flock, contender succeeds
                    FuseOpcode::Setlk => 6,
                    FuseOpcode::Setlkw => 2,
                    _ => 2,
                };
                match self
                    .interposer
                    .on_fs_lock(&request.path, flock_cmd, 1, request.pid)?
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

pub fn fuse_channel() -> (Sender<FuseRequest>, Receiver<FuseRequest>) {
    channel()
}

pub struct FuseWorker {
    receiver: Receiver<FuseRequest>,
    dispatcher: FuseDispatcher,
    session_id: [u8; 16],
}

impl FuseWorker {
    pub fn new(
        receiver: Receiver<FuseRequest>,
        interposer: Arc<dyn FsInterposer>,
        lock_broker: Arc<LockBroker>,
        quota: Arc<StorageQuota>,
        session_id: [u8; 16],
    ) -> Self {
        Self {
            receiver,
            dispatcher: FuseDispatcher::new(interposer)
                .with_lock_broker(lock_broker)
                .with_quota(quota),
            session_id,
        }
    }

    pub fn dispatcher(&self) -> &FuseDispatcher {
        &self.dispatcher
    }

    pub fn with_synthetic_files(mut self, files: Vec<SyntheticFile>) -> Self {
        self.dispatcher = self.dispatcher.with_synthetic_files(files);
        self
    }

    pub fn run(&mut self) -> Result<(), PluginError> {
        // TODO (L3) [vr_10_virtio_fs_fuse_req_channel]: virtqueue consumption loop, recv request, dispatch, reply on queue; slop: FUSE reply never marshalled, virtqueue never consumed
        let _ = (&self.receiver, &self.dispatcher, self.session_id);
        Err(PluginError::Halt(
            "TODO (L3) [vr_10_virtio_fs_fuse_req_channel]: FuseWorker::run needs the virtqueue consumption loop in kelir-drivers/kelir-transport enclave; slop: FUSE reply never marshalled, virtqueue never consumed".into(),
        ))
    }
}
