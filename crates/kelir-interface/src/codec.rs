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
