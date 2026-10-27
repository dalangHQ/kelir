// crates/kelir-interface/src/codec.rs
//
// Little-Endian Frame header for host↔guest transport over vsock:5252.
//
// Owned by: cans/09-protocol.md#Wire-framing

// Wire header layout (24 bytes, Little-Endian bit numbering):
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
pub const MAX_FRAME_PAYLOAD: usize = 64 * 1024; // 64KB strict boundary

pub const FLAG_CONTINUATION: u16 = 0x0001;

// Canonical message types. The header carries integers only; no
// message type names a domain, device, or plugin.
pub const MSG_TRAP: u16 = 0x0001; // Trap Notification / Decision
pub const MSG_CHANNEL_PAYLOAD: u16 = 0x0002; // Channel Payload (ChannelData)
pub const MSG_VM_LIFECYCLE_CONTROL: u16 = 0x0003; // VM Lifecycle Control
pub const MSG_SYSTEM_TELEMETRY: u16 = 0x0004; // System Telemetry (Heartbeat, Pressure)

// Packed layout, exactly 24 bytes, matching the diagram above:
// All multi-byte integers encoded in Little-Endian byte order.
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
