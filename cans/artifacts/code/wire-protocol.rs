// crates/kelir-interface/src/codec.rs
//
// Frame header for host↔guest transport over vsock:5252.
//
// Owned by: cans/09-protocol.md#Wire-framing

// Wire header layout (24 bytes, MSB-first bit numbering):
//
//  0                   1                   2                   3
//  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
// |          Magic (0x4B45)       |          Version (0x04)       |
// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
// |          Channel ID           |          Message Type         |
// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
// |             Flags             |            Reserved           |
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

// Packed layout, exactly 24 bytes, matching the diagram above:
// 6 x u16 (magic, version, channel, type, flags, reserved) = 12,
// sequence (u64) = 8, payload_len (u32) = 4. No padding bytes.
#[repr(C, packed)]
pub struct FrameHeader {
    pub magic: u16,
    pub version: u16,
    pub channel_id: u16,
    pub message_type: u16,
    pub flags: u16,
    pub reserved: u16,
    pub sequence: u64, // monotonic 64-bit sequence number
    pub payload_len: u32,
}

const _: () = assert!(std::mem::size_of::<FrameHeader>() == HEADER_LEN);
