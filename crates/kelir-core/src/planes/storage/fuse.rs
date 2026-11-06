// see: cans/05-storage-plane.md#Storage-architecture
// see: cans/05-storage-plane.md#Write-gating
// see: cans/05-storage-plane.md#Host-inode-lock-broker
use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{FsInterposer, HandlerCtx};
use kelir_interface::protocol::{FilterAction, Verdict};
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
pub struct FuseResponse {
    pub guest_inode: u64,
    pub payload: Vec<u8>,
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
                Verdict::Pass => Ok(FuseResponse {
                    guest_inode: request.guest_inode,
                    // TODO (L3): fill lookup payload from broker getattr.
                    payload: Vec::new(),
                }),
                Verdict::Drop { errno, reason } => Err(PluginError::Halt(format!(
                    "FUSE LOOKUP denied: {reason} ({errno:?})"
                ))),
                Verdict::Patch(_) => Err(PluginError::Halt(
                    "FUSE LOOKUP interposer returned Patch, unsupported for lookup".into(),
                )),
                Verdict::Suspend { .. } => Err(PluginError::Halt(
                    "FUSE LOOKUP interposer returned Suspend, unsupported for lookup".into(),
                )),
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
                Ok(FuseResponse {
                    guest_inode: request.guest_inode,
                    // TODO (L3): fill read payload from broker read at offset.
                    payload: Vec::new(),
                })
            }
            FuseOpcode::Write => match self.interposer.on_fs_write(
                &request.path,
                &mut request.data,
                request.pid,
                ctx,
            )? {
                FilterAction::Forward => Ok(FuseResponse {
                    guest_inode: request.guest_inode,
                    // TODO (L3): fill write payload with bytes committed to upper.
                    payload: Vec::new(),
                }),
                FilterAction::Drop { errno, reason } => Err(PluginError::Halt(format!(
                    "FUSE WRITE denied: {reason} ({errno:?})"
                ))),
                FilterAction::Suspend { .. } => Err(PluginError::Halt(
                    "FUSE WRITE interposer returned Suspend, unsupported for write".into(),
                )),
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
                    Verdict::Pass => Ok(FuseResponse {
                        guest_inode: request.guest_inode,
                        // TODO (L3): fill lock payload from host flock grant.
                        payload: Vec::new(),
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
