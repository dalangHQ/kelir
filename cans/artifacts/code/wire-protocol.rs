// crates/kelir-interface/src/codec.rs
//
// Little-Endian Frame header for host↔guest transport over port 5252
// (retained; Transport backend: SPSC rings primary; AF_VSOCK /
// named-pipe fallback — framing identical, only the byte carrier differs).
// Exactly two transports: Control 0x0000 and Stream Mux 0x0001.
// The 24-byte FrameHeader below is kept as-is.
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

// Transport and stream kind registry (11-protocol). Message types above
// are distinct from transport and kind IDs: 0x0003 above is a message
// type (MSG_VM_LIFECYCLE_CONTROL), while KIND_VFS below is a stream
// kind multiplexed on the Stream Mux transport.
// Transports:
//   0x0000: Control (lifecycle, trap control, signals)
//   0x0001: Stream Mux (all request/response and data flows)
// Stream kinds on 0x0001:
//   0x0001: EXEC (programmatic exec + stdout/stderr/control)
//   0x0002: PTY_CONTROL (window resize, signal-forward per exec)
//   0x0003: VFS (paginated posix-like API, framed CBOR)
//   0x0004: BULK (raw chunked byte streams; no CBOR, no per-chunk
//     FUSE context switches; chunk size bounded by transport MTU, no
//     fixed 64KB ceiling — the 64KB MAX_FRAME_PAYLOAD below is the TLV
//     wire frame, not the bulk chunk size)
//   0x0005: L4_FORWARD (native wire plane stream diversion)
//   0x0006: TERMINAL_CONSOLE (Port 0 human PTY bytes)
//   0x0010: PROFILE_SYNC (plugin-profile)
//   0x0011: DB_SQLITE (plugin-db-proxy)
//   0x0012: MCP_CONSOLE (plugin-mcp)
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

// Binary exec frame layout (mux 0x0001, kind EXEC 0x0001):
// stream_id: u32, flags: u8, len: u32, payload. EXEC sub-lane codes
// live in protocol.rs (EXEC_STREAM_*); 0=stdin, 1=stdout, 2=stderr,
// 3=exec control.

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
