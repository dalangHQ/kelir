// see: cans/05-storage-plane.md#FUSE-opcode-table
pub const FUSE_LOOKUP: u32 = 1;
pub const FUSE_GETATTR: u32 = 3;
pub const FUSE_MKDIR: u32 = 9;
pub const FUSE_UNLINK: u32 = 10;
pub const FUSE_RENAME: u32 = 12;
pub const FUSE_READ: u32 = 15;
pub const FUSE_WRITE: u32 = 16;
pub const FUSE_STATFS: u32 = 17;
pub const FUSE_READDIR: u32 = 28;
pub const FUSE_SETLK: u32 = 32;
pub const FUSE_SETLKW: u32 = 33;
