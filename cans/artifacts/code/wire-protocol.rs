// crates/kelir-interface/src/codec.rs
//
// Little-Endian Frame header for host↔guest transport over logical
// channel 5252 (Transport backend: SPSC rings primary; AF_VSOCK /
// named-pipe fallback — framing identical, only the byte carrier differs).
//
// Owned by: cans/11-protocol.md#Wire-framing

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

// Channel allocation table (11-protocol). Message types above are
// distinct from channel IDs: 0x0003 here is a message type, while
// CHANNEL_BINARY_EXEC below is the channel that carries exec streams.
// 0x0003: binary-exec (native terminal plane, programmatic exec)
// 0x0005: control-plane VFS (paginated posix-like API, framed CBOR)
// 0x0006: bulk transfer (raw chunked byte streams; no CBOR, no per-chunk
//   FUSE context switches; chunk size bounded by transport MTU, no
//   fixed 64KB ceiling — the 64KB MAX_FRAME_PAYLOAD below is the TLV
//   wire frame, not the bulk chunk size)
pub const CHANNEL_BINARY_EXEC: u16 = 0x0003;
pub const CHANNEL_VFS: u16 = 0x0005;
pub const CHANNEL_BULK: u16 = 0x0006;

// Binary exec frame layout (0x0003): 4-byte channel, 1-byte stream_id,
// 4-byte len, payload. stream_id values live in protocol.rs
// (EXEC_STREAM_*); 0=stdin, 1=stdout, 2=stderr, 3=control.

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
