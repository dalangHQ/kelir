// crates/kelir-interface/src/codec_protocol.rs
//
// Mux frame header for host↔guest transport over port 5252.
// Transport backend: SPSC rings primary; AF_VSOCK / named-pipe fallback —
// framing identical, only the byte carrier differs.
// Exactly two transports: Control 0x0000 and Stream Mux 0x0001.
//
// Owned by: cans/11-protocol.md#Wire-framing

// Wire layout: stream_id, flags, len, payload. The header on the wire
// is 9 bytes (stream_id: u32, flags: u8, len: u32, all Little-Endian),
// with no magic, version, channel, or sequence field. The struct below
// is the logical field layout; it is not a packed 9-byte struct, so
// no size assert is made on it.
// Hostile input rules: all wire-derived numbers (len, stream_id, flags)
// must be validated with checked_* arithmetic. Direct unchecked slicing
// is forbidden; allocations exceeding MAX_FRAME_PAYLOAD fail immediately.

pub const MAX_FRAME_PAYLOAD: usize = 64 * 1024; // 64KB strict boundary
pub const MAX_CONTROL_PAYLOAD: usize = 4 * 1024; // 4KB control ceiling
pub const MAX_STREAM_ID: u32 = 0x00FF_FFFF; // Upper byte reserved for flags

/// Stream mux frame header (11-protocol Stream mux framing).
pub struct MuxFrameHeader {
    pub stream_id: u32,
    pub flags: u8,
    pub len: u32,
}

// Mux frame flags (u8). FLAG_OPEN payload carries kind_id: u16 + tags.
pub const FLAG_OPEN: u8 = 0x01;
pub const FLAG_DATA: u8 = 0x02;
pub const FLAG_CLOSE: u8 = 0x04;
pub const FLAG_RST: u8 = 0x08;
pub const FLAG_WINDOW_UPDATE: u8 = 0x10;

// Per-stream credit windows: a sender parks when its stream's credit
// is exhausted, and resumes on FLAG_WINDOW_UPDATE.

// Canonical message types. These are payload message-type IDs, not
// header fields. No message type names a domain, device, or plugin.
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
//   0x0002: PTY_CONTROL (signal-forward per exec)
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
// MuxFrameHeader (stream_id: u32, flags: u8, len: u32), payload.
// EXEC sub-lane codes live in protocol_wire.rs (EXEC_STREAM_*); 0=stdin,
// 1=stdout, 2=stderr, 3=exec control.
//
// Spawn submit path (11-protocol Binary exec channel, 03-sdk-architecture Params typed views):
// SpawnRequest { session_id: [u8; 16], stream_kind: StreamKind, args: Vec<String>,
//   cwd: Option<String>, limits_bytes: Vec<u8>, params_bytes: Vec<u8> }.
// params_bytes is the CBOR encoding of the SpawnBuilder::params<T: Serialize> value;
// HandlerCtx::params::<T>() decodes the same bytes with ciborium, so encode and
// decode share one codec and one buffer shape.
// SpawnTransport::dispatch_exec(&self, request: &SpawnRequest) -> Result<(), PluginError>
// is the host-side submit; the live transport encodes the EXEC frame on the bound
// stream kind. The EXEC frame encode and the SpawnResult (exit code, stdout,
// stderr) channel are not part of dispatch_exec.
// Per-exec params store: session key (exec_id: u64, stream_id: u8) -> params_bytes.
// The transport stores request.params_bytes under the (exec_id, stream_id) the
// EXEC frame allocates; trap dispatch hydrates HandlerCtx.params_buffer from the
// store per exec before interceptors run, so c.params::<T>() reads the params
// of the exec being trapped and never another exec's.
