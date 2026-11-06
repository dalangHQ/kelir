// see: cans/01-architecture.md#Transport-engine-abstraction
// see: cans/11-protocol.md#Control-channel
use std::collections::HashMap;
use std::sync::RwLock;

pub struct BusDispatcher {
    control_transport: kelir_transport::spsc_ring::SpscRingTransport,
    stream_mux_transport: kelir_transport::spsc_ring::SpscRingTransport,
    active_streams: RwLock<HashMap<u32, kelir_interface::protocol::StreamKind>>,
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
        }
    }

    pub fn poll_ingress(
        &mut self,
    ) -> Result<Option<(u32, u8, Vec<u8>)>, kelir_interface::errors::PluginError> {
        // TODO (L3): read one mux frame (stream_id, flags, payload) from the stream mux SPSC ring.
        let _ = (
            &mut self.control_transport,
            &mut self.stream_mux_transport,
            &self.active_streams,
        );
        Ok(None)
    }

    pub fn route_stream_frame(
        &mut self,
        stream_id: u32,
        flags: u8,
        payload: &[u8],
    ) -> Result<(), kelir_interface::errors::PluginError> {
        // TODO (L3): look up stream_id in active_streams and deliver payload to its kind handler.
        let _ = (
            &mut self.control_transport,
            &mut self.stream_mux_transport,
            &self.active_streams,
        );
        let _ = (stream_id, flags, payload);
        Ok(())
    }

    pub fn send_control_msg(
        &mut self,
        msg_type: kelir_interface::protocol::MsgType,
        payload: &[u8],
    ) -> Result<(), kelir_interface::errors::PluginError> {
        // TODO (L3): encode msg_type as the first u16 of the payload frame and write to the control SPSC ring.
        let _ = (
            &mut self.control_transport,
            &mut self.stream_mux_transport,
            &self.active_streams,
        );
        let _ = (msg_type, payload);
        Ok(())
    }
}
