// see: cans/01-architecture.md#Transport-engine-abstraction
// see: cans/11-protocol.md#Control-channel
// see: cans/11-protocol.md#Wire-framing
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

// see: cans/11-protocol.md#Wire-framing
use kelir_interface::codec_protocol::CodecError;
pub use kelir_interface::codec_protocol::{
    FLAG_DATA, FLAG_OPEN, HEADER_LEN, MAX_FRAME_PAYLOAD, MAX_STREAM_ID, decode_header,
    encode_msg_frame,
};
use kelir_interface::plugin_model::PluginError;
use kelir_interface::protocol_wire::{MsgType, StreamKind};
use kelir_transport::Transport;
use kelir_transport::spsc_ring::OwnedSpscRing;

// see: cans/01-architecture.md#Transport-engine-abstraction
pub const CONTROL_RING_CAPACITY: u32 = 128 * 1024;

// see: cans/01-architecture.md#Transport-engine-abstraction
pub const MUX_RING_CAPACITY: u32 = 256 * 1024;

const POLL_SCRATCH_LEN: usize = 4096;

// see: cans/11-protocol.md#Control-channel
pub use kelir_interface::plugin_model::StreamFrameSink;

pub struct BusDispatcher {
    control_transport: kelir_transport::spsc_ring::SpscRingTransport,
    stream_mux_transport: kelir_transport::spsc_ring::SpscRingTransport,
    active_streams: RwLock<HashMap<u32, kelir_interface::protocol_wire::StreamKind>>,
    stream_sinks: RwLock<HashMap<u16, Arc<dyn StreamFrameSink>>>,
    mux_rx: Vec<u8>,
}

// see: cans/01-architecture.md#Transport-engine-abstraction
pub struct OwnedBusDispatcher {
    control_ring: OwnedSpscRing,
    mux_ring: OwnedSpscRing,
    dispatcher: BusDispatcher,
}
#[path = "dispatcher.flow.rs"]
mod dispatcher;
#[path = "frame.flow.rs"]
mod frame;

use frame::{codec_halt, take_frame, write_all_frame};
