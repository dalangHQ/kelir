// see: cans/01-architecture.md#Transport-engine-abstraction
// see: cans/11-protocol.md#Control-channel
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use kelir_interface::codec_protocol::{FLAG_DATA, FLAG_OPEN};
use kelir_interface::plugin_model::{PluginError, StreamFrameSink};
use kelir_interface::protocol_wire::{MsgType, StreamKind};
use kelir_transport::Transport;
use kelir_transport::spsc_ring::OwnedSpscRing;

use super::frame::{codec_halt, take_frame, write_all_frame};

const POLL_SCRATCH_LEN: usize = 4096;

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
        control: &kelir_interface::driver_interface::SharedMemoryHandle,
        mux: &kelir_interface::driver_interface::SharedMemoryHandle,
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
    ) -> Result<Option<(u32, u8, Vec<u8>)>, kelir_interface::plugin_model::PluginError> {
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
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
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
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        let mut frame = Vec::new();
        encode_msg_frame(0, FLAG_DATA, msg_type, payload, &mut frame).map_err(codec_halt)?;
        write_all_frame(&mut self.control_transport, &frame)
    }
}
