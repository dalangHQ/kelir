// see: cans/01-architecture.md#Transport-engine-abstraction
// see: cans/11-protocol.md#Control-channel
// see: cans/11-protocol.md#Wire-framing
#[path = "dispatcher.flow.rs"]
mod dispatcher;
#[path = "frame.flow.rs"]
mod frame;

pub use dispatcher::{BusDispatcher, OwnedBusDispatcher};
pub use kelir_interface::codec_protocol::{
    FLAG_DATA, FLAG_OPEN, HEADER_LEN, MAX_FRAME_PAYLOAD, MAX_STREAM_ID, decode_header,
    encode_msg_frame,
};
pub use kelir_interface::plugin_model::StreamFrameSink;

// see: cans/01-architecture.md#Transport-engine-abstraction
pub const CONTROL_RING_CAPACITY: u32 = 128 * 1024;

// see: cans/01-architecture.md#Transport-engine-abstraction
pub const MUX_RING_CAPACITY: u32 = 256 * 1024;
