pub const MAX_FRAME_PAYLOAD: usize = 64 * 1024;
pub const MAX_CONTROL_PAYLOAD: usize = 4 * 1024;
pub const MAX_STREAM_ID: u32 = 0x00FF_FFFF;

pub struct MuxFrameHeader {
    pub stream_id: u32,
    pub flags: u8,
    pub len: u32,
}

pub const FLAG_OPEN: u8 = 0x01;
pub const FLAG_DATA: u8 = 0x02;
pub const FLAG_CLOSE: u8 = 0x04;
pub const FLAG_RST: u8 = 0x08;
pub const FLAG_WINDOW_UPDATE: u8 = 0x10;

pub const MSG_TRAP: u16 = 0x0001;
pub const MSG_CHANNEL_PAYLOAD: u16 = 0x0002;
pub const MSG_VM_LIFECYCLE_CONTROL: u16 = 0x0003;
pub const MSG_SYSTEM_TELEMETRY: u16 = 0x0004;

pub const TRANSPORT_CONTROL: u16 = 0x0000;
pub const TRANSPORT_STREAM_MUX: u16 = 0x0001;
pub const KIND_EXEC: u16 = 0x0001;
pub const KIND_PTY_CONTROL: u16 = 0x0002;
pub const KIND_VFS: u16 = 0x0003;
pub const KIND_BULK: u16 = 0x0004;
pub const KIND_L4_FORWARD: u16 = 0x0005;
pub const KIND_TERMINAL_CONSOLE: u16 = 0x0006;
pub const KIND_PROFILE_SYNC: u16 = 0x0010;
pub const KIND_DB_SQLITE: u16 = 0x0011;
pub const KIND_MCP_CONSOLE: u16 = 0x0012;
