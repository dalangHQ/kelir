// see: cans/05-storage-plane.md#Storage-architecture
// see: cans/05-storage-plane.md#Write-gating
// see: cans/05-storage-plane.md#Host-inode-lock-broker
// see: cans/05-storage-plane.md#Synthetic-FUSE-inodes
// see: cans/02-plugin-model.md#Hook-return-semantics
use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{FsContext, FsInodeAttr, FsInterposer, HandlerCtx};
use kelir_interface::protocol::{FilterAction, PatchPayload, Verdict};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use super::synthetic::SyntheticFile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuseOpcode {
    Lookup,
    Read,
    Write,
    Setlk,
    Setlkw,
}

impl FuseOpcode {
    pub fn from_u32(value: u32) -> Result<Self, PluginError> {
        // TODO (L3): validate opcode numbers against the virtio-fs queue parser.
        match value {
            1 => Ok(FuseOpcode::Lookup),
            15 => Ok(FuseOpcode::Read),
            16 => Ok(FuseOpcode::Write),
            32 => Ok(FuseOpcode::Setlk),
            33 => Ok(FuseOpcode::Setlkw),
            _ => Err(PluginError::Halt(format!("unknown FUSE opcode {value}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuseRequest {
    pub opcode: FuseOpcode,
    pub guest_inode: u64,
    pub path: PathBuf,
    pub pid: u32,
    pub data: Vec<u8>,
    pub offset: u64,
    pub blocking: bool,
}

// see: cans/05-storage-plane.md#Synthetic-FUSE-inodes
#[derive(Debug, Clone)]
pub enum FuseReply {
    // see: cans/05-storage-plane.md#Interception-pipeline
    Payload(Vec<u8>),
    // see: cans/02-plugin-model.md#FsInodeAttr-carries-full-mode-uid-gid-size-content
    Inode(FsInodeAttr),
    // see: cans/05-storage-plane.md#Write-gating
    Errno(i32),
}

#[derive(Debug, Clone)]
pub struct FuseResponse {
    pub guest_inode: u64,
    pub reply: FuseReply,
}

pub struct FuseDispatcher {
    interposer: Arc<dyn FsInterposer>,
    synthetic_files: Vec<SyntheticFile>,
}

impl FuseDispatcher {
    pub fn new(interposer: Arc<dyn FsInterposer>) -> Self {
        Self {
            interposer,
            synthetic_files: Vec::new(),
        }
    }

    pub fn with_synthetic_files(mut self, files: Vec<SyntheticFile>) -> Self {
        self.synthetic_files = files;
        self
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
                            guest_inode: request.guest_inode,
                            // TODO (L3): driver mints the fuse_entry_out ino for the synthetic inode.
                            reply: FuseReply::Inode(attr),
                        });
                    }
                }
                match self.interposer.on_fs_lookup(&request.path)? {
                    Verdict::Pass => Ok(FuseResponse {
                        guest_inode: request.guest_inode,
                        // TODO (L3): fill lookup payload from broker getattr.
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
                        guest_inode: request.guest_inode,
                        // TODO (L3): driver mints the fuse_entry_out ino for the synthetic inode.
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
                        let start = (request.offset as usize).min(content.len());
                        let end = if request.data.is_empty() {
                            content.len()
                        } else {
                            content.len().min(start.saturating_add(request.data.len()))
                        };
                        return Ok(FuseResponse {
                            guest_inode: request.guest_inode,
                            reply: FuseReply::Payload(content[start..end].to_vec()),
                        });
                    }
                }
                self.interposer
                    .on_fs_read(&request.path, &mut request.data)?;
                Ok(FuseResponse {
                    guest_inode: request.guest_inode,
                    // TODO (L3): fill read payload from broker read at offset.
                    reply: FuseReply::Payload(Vec::new()),
                })
            }
            FuseOpcode::Write => match self.interposer.on_fs_write(
                &mut request.data,
                &ctx.clone().with_fs_ctx(FsContext {
                    inode: request.guest_inode,
                    path: request.path.clone(),
                    journal: 0,
                }),
            )? {
                FilterAction::Forward => Ok(FuseResponse {
                    guest_inode: request.guest_inode,
                    // TODO (L3): fill write payload with bytes committed to upper.
                    reply: FuseReply::Payload(Vec::new()),
                }),
                // see: cans/02-plugin-model.md#Unified-verdict
                FilterAction::Drop { errno: None, .. } => Ok(FuseResponse {
                    guest_inode: request.guest_inode,
                    // TODO (L3): reply the requested chunk size so the blackholed write stays silent.
                    reply: FuseReply::Payload(Vec::new()),
                }),
                // see: cans/05-storage-plane.md#Write-gating
                FilterAction::Drop {
                    errno: Some(errno), ..
                } => {
                    let errno = drop_errno(&errno)?;
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
            },
            FuseOpcode::Setlk | FuseOpcode::Setlkw => {
                let flock_cmd = match request.opcode {
                    // TODO (L3): flock LOCK_EX/LOCK_NB constants live in the enclave.
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
                        // TODO (L3): fill lock payload from host flock grant.
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
// see: cans/plugins/chaos.md#Error-and-return-semantics
fn drop_errno(errno: &str) -> Result<i32, PluginError> {
    match errno {
        "EPERM" => Ok(1),
        "ENXIO" => Ok(6),
        "EWOULDBLOCK" => Ok(11),
        "EACCES" => Ok(13),
        "EFAULT" => Ok(14),
        "EBUSY" => Ok(16),
        "EXDEV" => Ok(18),
        "ENOSPC" => Ok(28),
        "EROFS" => Ok(30),
        "ECONNREFUSED" => Ok(111),
        _ => Err(PluginError::Halt(format!(
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
        session_id: [u8; 16],
    ) -> Self {
        Self {
            receiver,
            dispatcher: FuseDispatcher::new(interposer),
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
        // TODO (L3): virtqueue consumption loop, recv request, dispatch, reply on queue.
        let _ = (&self.receiver, &self.dispatcher, self.session_id);
        Err(PluginError::Halt(
            "TODO (L3): FuseWorker::run needs the virtqueue consumption loop in kelir-drivers/kelir-transport enclave".into(),
        ))
    }
}
