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
// |          Magic (0x4B45)       |          Version (0x03)       |
// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
// |          Message Type         |          Flags                |
// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
// |                      Sequence Number (MSB)                    |
// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
// |                      Sequence Number (LSB)                    |
// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
// |                         Payload Length                        |
// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
// |                         Payload Data (CBOR)...                |
// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+

pub const MAGIC: u16 = 0x4B45;     // 'KE'
pub const VERSION: u16 = 0x03;
// Layout: magic(2)+version(2)+type(2)+flags(2)+seq_msb(4)+seq_lsb(4)+payload_len(4)
// The header occupies the first five 32-bit rows of the diagram above;
// the sixth row is payload, not header.
pub const HEADER_LEN: usize = 20;
// Byte order and padding: TBD

#[repr(C)]
pub struct FrameHeader {
    pub magic: u16,
    pub version: u16,
    pub message_type: u16,
    pub flags: u16,
    /// 64-bit sequence number transmitted as two 32-bit words, MSB first.
    /// The codec reassembles u64 from these before payload decode.
    pub seq_msb: u32,
    pub seq_lsb: u32,
    pub payload_len: u32,
}

impl FrameHeader {
    /// Reassemble the on-wire (msb, lsb) pair into a native u64 sequence number.
    pub fn sequence(&self) -> u64 {
        ((self.seq_msb as u64) << 32) | (self.seq_lsb as u64)
    }
}
