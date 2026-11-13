// see: cans/05-storage-plane.md#Storage-architecture
// see: cans/05-storage-plane.md#Write-gating
// see: cans/05-storage-plane.md#Host-inode-lock-broker
// see: cans/05-storage-plane.md#Synthetic-FUSE-inodes
// see: cans/02-plugin-model.md#Hook-return-semantics
use crate::plugin_model::{FsInodeAttr, HandlerCtx, PluginError};
use std::path::PathBuf;

#[path = "uapi_fuse.rs"]
mod uapi_fuse;

// see: cans/05-storage-plane.md#Storage-architecture
pub trait FuseOps: Send + Sync {
    fn dispatch(
        &self,
        req: &mut FuseRequest,
        ctx: &HandlerCtx,
    ) -> Result<FuseResponse, PluginError>;
}

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
