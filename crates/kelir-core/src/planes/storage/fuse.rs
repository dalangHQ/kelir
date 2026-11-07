// see: cans/05-storage-plane.md#Storage-architecture
// see: cans/05-storage-plane.md#Write-gating
// see: cans/05-storage-plane.md#Host-inode-lock-broker
// see: cans/02-plugin-model.md#Hook-return-semantics
// see: cans/artifacts/code/messages.rs
use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{FsInodeAttr, FsInterposer, HandlerCtx};
use kelir_interface::protocol::{FilterAction, PatchPayload, Verdict};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuseError {
    pub errno: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct FuseResponse {
    pub guest_inode: u64,
    pub payload: Vec<u8>,
    pub attr: Option<FsInodeAttr>,
    pub error: Option<FuseError>,
}

impl FuseResponse {
    pub fn ok(guest_inode: u64) -> Self {
        Self {
            guest_inode,
            payload: Vec::new(),
            attr: None,
            error: None,
        }
    }

    pub fn served_inode(guest_inode: u64, attr: FsInodeAttr) -> Self {
        Self {
            guest_inode,
            payload: attr.content.clone(),
            attr: Some(attr),
            error: None,
        }
    }

    pub fn denied(guest_inode: u64, errno: Option<String>, reason: String) -> Self {
        Self {
            guest_inode,
            payload: Vec::new(),
            attr: None,
            error: Some(FuseError { errno, reason }),
        }
    }
}

pub struct FuseDispatcher {
    interposer: Arc<dyn FsInterposer>,
}

impl FuseDispatcher {
    pub fn new(interposer: Arc<dyn FsInterposer>) -> Self {
        Self { interposer }
    }

    pub fn dispatch(
        &self,
        request: &mut FuseRequest,
        ctx: &HandlerCtx,
    ) -> Result<FuseResponse, PluginError> {
        match request.opcode {
            FuseOpcode::Lookup => match self.interposer.on_fs_lookup(&request.path)? {
                Verdict::Pass => {
                    // TODO (L3): fill lookup payload from broker getattr.
                    Ok(FuseResponse::ok(request.guest_inode))
                }
                Verdict::Patch(PatchPayload::FsInode {
                    mode,
                    uid,
                    gid,
                    size,
                    content,
                }) => {
                    // TODO (L3): materialize a guest inode number and encode attr into the FUSE LOOKUP reply.
                    Ok(FuseResponse::served_inode(
                        request.guest_inode,
                        FsInodeAttr {
                            mode: mode & !0o6000,
                            uid,
                            gid,
                            size,
                            content,
                        },
                    ))
                }
                Verdict::Patch(_) => Err(PluginError::Halt(
                    "FUSE LOOKUP interposer returned a non-inode Patch".into(),
                )),
                Verdict::Drop { errno, reason } => {
                    Ok(FuseResponse::denied(request.guest_inode, errno, reason))
                }
                Verdict::Suspend { .. } => {
                    // TODO (L3): park Suspended requests in a hold queue once the virtqueue loop lands.
                    Err(PluginError::Halt(
                        "FUSE LOOKUP interposer returned Suspend, unsupported for lookup".into(),
                    ))
                }
                Verdict::Abort { reason } => Err(PluginError::Halt(format!(
                    "FUSE LOOKUP aborted by interposer: {reason}"
                ))),
                _ => Err(PluginError::Halt(
                    "FUSE LOOKUP interposer returned an unknown verdict".into(),
                )),
            },
            FuseOpcode::Read => {
                self.interposer
                    .on_fs_read(&request.path, &mut request.data)?;
                // TODO (L3): fill read payload from broker read at offset.
                Ok(FuseResponse::ok(request.guest_inode))
            }
            FuseOpcode::Write => match self.interposer.on_fs_write(
                &request.path,
                &mut request.data,
                request.pid,
                ctx,
            )? {
                FilterAction::Forward => {
                    // TODO (L3): fill write payload with bytes committed to upper.
                    Ok(FuseResponse::ok(request.guest_inode))
                }
                FilterAction::Drop { errno, reason } => {
                    // TODO (L3): encode errno into the FUSE reply error field for the guest kernel.
                    Ok(FuseResponse::denied(request.guest_inode, errno, reason))
                }
                FilterAction::Suspend { .. } => {
                    // TODO (L3): park Suspended requests in a hold queue once the virtqueue loop lands.
                    Err(PluginError::Halt(
                        "FUSE WRITE interposer returned Suspend, unsupported for write".into(),
                    ))
                }
                _ => Err(PluginError::Halt(
                    "FUSE WRITE interposer returned an unknown filter action".into(),
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
                    Verdict::Pass => {
                        // TODO (L3): fill lock payload from host flock grant.
                        Ok(FuseResponse::ok(request.guest_inode))
                    }
                    Verdict::Drop { errno, reason } => {
                        Ok(FuseResponse::denied(request.guest_inode, errno, reason))
                    }
                    Verdict::Patch(_) => Err(PluginError::Halt(
                        "FUSE SETLK interposer returned Patch, unsupported for locks".into(),
                    )),
                    Verdict::Suspend { .. } => {
                        // TODO (L3): park Suspended requests in a hold queue once the virtqueue loop lands.
                        Err(PluginError::Halt(
                            "FUSE SETLK interposer returned Suspend, unsupported for locks".into(),
                        ))
                    }
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

    pub fn run(&mut self) -> Result<(), PluginError> {
        // TODO (L3): virtqueue consumption loop, recv request, dispatch, reply on queue.
        let _ = (&self.receiver, &self.dispatcher, self.session_id);
        Err(PluginError::Halt(
            "TODO (L3): FuseWorker::run needs the virtqueue consumption loop in kelir-drivers/kelir-transport enclave".into(),
        ))
    }
}
