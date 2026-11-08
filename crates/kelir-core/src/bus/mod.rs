// see: cans/01-architecture.md#Transport-engine-abstraction
// see: cans/11-protocol.md#Control-channel
// see: cans/11-protocol.md#Wire-framing
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use kelir_interface::plugin::PluginError;
use kelir_interface::protocol::{MsgType, StreamKind};
use kelir_transport::Transport;
use kelir_transport::spsc_ring::OwnedSpscRing;

// see: cans/11-protocol.md#Wire-framing
pub const FRAME_HEADER_LEN: usize = 9;

// see: cans/11-protocol.md#Wire-framing
pub const MAX_FRAME_PAYLOAD: usize = 64 * 1024;

// see: cans/11-protocol.md#Wire-framing
pub const MAX_STREAM_ID: u32 = 0x00FF_FFFF;

// see: cans/11-protocol.md#Stream-mux-framing
pub const FLAG_OPEN: u8 = 0x01;

// see: cans/11-protocol.md#Stream-mux-framing
pub const FLAG_DATA: u8 = 0x02;

// see: cans/01-architecture.md#Transport-engine-abstraction
pub const CONTROL_RING_CAPACITY: u32 = 128 * 1024;

// see: cans/01-architecture.md#Transport-engine-abstraction
pub const MUX_RING_CAPACITY: u32 = 256 * 1024;

const POLL_SCRATCH_LEN: usize = 4096;

// see: cans/11-protocol.md#Control-channel
pub trait StreamFrameSink: Send + Sync {
    fn on_stream_frame(
        &self,
        stream_id: u32,
        kind: StreamKind,
        flags: u8,
        payload: &[u8],
    ) -> Result<(), PluginError>;
}

pub struct BusDispatcher {
    control_transport: kelir_transport::spsc_ring::SpscRingTransport,
    stream_mux_transport: kelir_transport::spsc_ring::SpscRingTransport,
    active_streams: RwLock<HashMap<u32, kelir_interface::protocol::StreamKind>>,
    stream_sinks: RwLock<HashMap<u16, Arc<dyn StreamFrameSink>>>,
    mux_rx: Vec<u8>,
}

// see: cans/01-architecture.md#Transport-engine-abstraction
pub struct OwnedBusDispatcher {
    control_ring: OwnedSpscRing,
    mux_ring: OwnedSpscRing,
    dispatcher: BusDispatcher,
}

impl OwnedBusDispatcher {
    // see: cans/01-architecture.md#Transport-engine-abstraction
    pub fn new(control_capacity: u32, mux_capacity: u32) -> Self {
        let mut control_ring = OwnedSpscRing::new(control_capacity);
        let mut mux_ring = OwnedSpscRing::new(mux_capacity);
        let dispatcher = BusDispatcher::new(control_ring.transport(), mux_ring.transport());
        Self {
            control_ring,
            mux_ring,
            dispatcher,
        }
    }

    // see: cans/01-architecture.md#Shared-memory-channel
    pub fn from_shared_regions(
        control: &kelir_interface::driver::SharedMemoryHandle,
        mux: &kelir_interface::driver::SharedMemoryHandle,
    ) -> Option<Self> {
        let mut control_ring = OwnedSpscRing::from_shared_region(control)?;
        let mut mux_ring = OwnedSpscRing::from_shared_region(mux)?;
        let dispatcher = BusDispatcher::new(control_ring.transport(), mux_ring.transport());
        Some(Self {
            control_ring,
            mux_ring,
            dispatcher,
        })
    }

    pub fn control_capacity(&self) -> u32 {
        self.control_ring.capacity()
    }

    pub fn mux_capacity(&self) -> u32 {
        self.mux_ring.capacity()
    }

    pub fn dispatcher(&mut self) -> &mut BusDispatcher {
        &mut self.dispatcher
    }
}

// see: cans/11-protocol.md#Wire-framing
fn encode_frame(
    stream_id: u32,
    flags: u8,
    payload: &[u8],
    out: &mut Vec<u8>,
) -> Result<(), PluginError> {
    if stream_id > MAX_STREAM_ID {
        return Err(PluginError::Halt(format!(
            "stream id {stream_id} exceeds 0x00FFFFFF"
        )));
    }
    if payload.len() > MAX_FRAME_PAYLOAD {
        return Err(PluginError::Halt(
            "frame payload exceeds the 64KB budget".into(),
        ));
    }
    out.extend_from_slice(&stream_id.to_le_bytes());
    out.push(flags);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    Ok(())
}

// see: cans/11-protocol.md#Wire-framing
fn take_frame(rx: &mut Vec<u8>) -> Result<Option<(u32, u8, Vec<u8>)>, PluginError> {
    if rx.len() < FRAME_HEADER_LEN {
        return Ok(None);
    }
    let stream_id = u32::from_le_bytes([rx[0], rx[1], rx[2], rx[3]]);
    let flags = rx[4];
    let payload_len = u32::from_le_bytes([rx[5], rx[6], rx[7], rx[8]]);
    let payload_len = usize::try_from(payload_len)
        .map_err(|_| PluginError::Halt("frame length overflows usize".into()))?;
    if stream_id > MAX_STREAM_ID {
        return Err(PluginError::Halt(format!(
            "stream id {stream_id} exceeds 0x00FFFFFF"
        )));
    }
    if payload_len > MAX_FRAME_PAYLOAD {
        return Err(PluginError::Halt(
            "frame payload exceeds the 64KB budget".into(),
        ));
    }
    let total = FRAME_HEADER_LEN
        .checked_add(payload_len)
        .ok_or_else(|| PluginError::Halt("frame length overflow".into()))?;
    if rx.len() < total {
        return Ok(None);
    }
    let payload = rx[FRAME_HEADER_LEN..total].to_vec();
    rx.drain(..total);
    Ok(Some((stream_id, flags, payload)))
}

// see: cans/11-protocol.md#Wire-framing
fn write_all_frame(
    transport: &mut kelir_transport::spsc_ring::SpscRingTransport,
    frame: &[u8],
) -> Result<(), PluginError> {
    let mut written = 0usize;
    while written < frame.len() {
        let count = transport.write(&frame[written..])?;
        written += count;
    }
    Ok(())
}

impl BusDispatcher {
    pub fn new(
        control: kelir_transport::spsc_ring::SpscRingTransport,
        mux: kelir_transport::spsc_ring::SpscRingTransport,
    ) -> Self {
        Self {
            control_transport: control,
            stream_mux_transport: mux,
            active_streams: RwLock::new(HashMap::new()),
            stream_sinks: RwLock::new(HashMap::new()),
            mux_rx: Vec::new(),
        }
    }

    // see: cans/11-protocol.md#Control-channel
    pub fn bind_stream_sink(
        &self,
        kind: StreamKind,
        sink: Arc<dyn StreamFrameSink>,
    ) -> Result<(), PluginError> {
        match self.stream_sinks.write() {
            Ok(mut guard) => {
                guard.insert(kind as u16, sink);
                Ok(())
            }
            Err(_) => Err(PluginError::Halt("stream sink registry poisoned".into())),
        }
    }

    pub fn active_stream_kind(&self, stream_id: u32) -> Option<StreamKind> {
        self.active_streams.read().ok()?.get(&stream_id).copied()
    }

    // see: cans/11-protocol.md#Stream-mux-framing
    fn register_open_stream(&self, stream_id: u32, payload: &[u8]) -> Result<(), PluginError> {
        if payload.len() < 2 {
            return Err(PluginError::Halt(
                "OPEN frame payload carries a u16 kind_id plus tags; got fewer than 2 bytes".into(),
            ));
        }
        let kind_id = u16::from_le_bytes([payload[0], payload[1]]);
        let Some(kind) = StreamKind::from_u16(kind_id) else {
            return Err(PluginError::Halt(format!(
                "EUnknownKind: stream kind 0x{kind_id:04x} is not registered"
            )));
        };
        match self.active_streams.write() {
            Ok(mut guard) => {
                guard.insert(stream_id, kind);
                Ok(())
            }
            Err(_) => Err(PluginError::Halt("active stream table poisoned".into())),
        }
    }

    pub fn poll_ingress(
        &mut self,
    ) -> Result<Option<(u32, u8, Vec<u8>)>, kelir_interface::plugin::PluginError> {
        let mut scratch = [0u8; POLL_SCRATCH_LEN];
        loop {
            let count = self.stream_mux_transport.read(&mut scratch)?;
            if count == 0 {
                return Ok(None);
            }
            self.mux_rx.extend_from_slice(&scratch[..count]);
            if let Some((stream_id, flags, payload)) = take_frame(&mut self.mux_rx)? {
                if flags & FLAG_OPEN != 0 {
                    self.register_open_stream(stream_id, &payload)?;
                }
                return Ok(Some((stream_id, flags, payload)));
            }
        }
    }

    pub fn route_stream_frame(
        &mut self,
        stream_id: u32,
        flags: u8,
        payload: &[u8],
    ) -> Result<(), kelir_interface::plugin::PluginError> {
        let kind = match self.active_streams.read() {
            Ok(guard) => guard.get(&stream_id).copied(),
            Err(_) => return Err(PluginError::Halt("active stream table poisoned".into())),
        };
        let Some(kind) = kind else {
            return Err(PluginError::Halt(format!(
                "EUnknownKind: stream {stream_id} has no registered kind"
            )));
        };
        let sink = match self.stream_sinks.read() {
            Ok(guard) => guard.get(&(kind as u16)).cloned(),
            Err(_) => return Err(PluginError::Halt("stream sink registry poisoned".into())),
        };
        let Some(sink) = sink else {
            return Err(PluginError::Halt(format!(
                "no stream sink bound for kind {kind:?}"
            )));
        };
        sink.on_stream_frame(stream_id, kind, flags, payload)
    }

    pub fn send_control_msg(
        &mut self,
        msg_type: MsgType,
        payload: &[u8],
    ) -> Result<(), kelir_interface::plugin::PluginError> {
        let body_len = payload
            .len()
            .checked_add(2)
            .ok_or_else(|| PluginError::Halt("control payload length overflow".into()))?;
        if body_len > MAX_FRAME_PAYLOAD {
            return Err(PluginError::Halt(
                "control payload exceeds the 64KB frame budget".into(),
            ));
        }
        let mut body = Vec::with_capacity(body_len);
        body.extend_from_slice(&(msg_type as u16).to_le_bytes());
        body.extend_from_slice(payload);
        let mut frame = Vec::with_capacity(FRAME_HEADER_LEN + body.len());
        encode_frame(0, FLAG_DATA, &body, &mut frame)?;
        write_all_frame(&mut self.control_transport, &frame)
    }
}
