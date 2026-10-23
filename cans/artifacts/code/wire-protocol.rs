// crates/kelir-core/src/protocol/codec.rs
//
// Frame header for host↔guest transport over vsock:5252.
//
// Owned by: cans/09-protocol.md#Wire-framing

// Wire header layout (20 bytes, MSB-first bit numbering):
//
//  0                   1                   2                   3
//  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
// |          Magic (0x4B45)       |          Version (0x04)       |
// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
// |          Channel ID           |          Message Type         |
// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
// |                     Sequence Number (u64)                     |
// |                                                               |
// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
// |                         Payload Length                        |
// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
// |                         Payload Data (CBOR)...                |
// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+

pub const MAGIC: u16 = 0x4B45;     // 'KE'
pub const VERSION: u16 = 0x04;
pub const HEADER_LEN: usize = 24;

#[repr(C)]
pub struct FrameHeader {
    pub magic: u16,
    pub version: u16,
    pub channel_id: u16,
    pub message_type: u16,
    pub flags: u16,
    pub sequence: u64,
    pub payload_len: u32,
}
