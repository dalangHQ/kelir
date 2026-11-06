// see: cans/01-architecture.md#VirtIO-Device-Complex
// see: cans/11-protocol.md#Control-channel
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub struct BusDispatcher {
    control_transport: Box<dyn kelir_transport::StreamTransport + Send + Sync>,
    stream_mux_transport: Box<dyn kelir_transport::StreamTransport + Send + Sync>,
    active_streams: Arc<RwLock<HashMap<u32, kelir_interface::protocol::StreamKind>>>,
}

impl BusDispatcher {
    pub fn new(
        control: Box<dyn kelir_transport::StreamTransport + Send + Sync>,
        mux: Box<dyn kelir_transport::StreamTransport + Send + Sync>,
    ) -> Self {
        Self {
            control_transport: control,
            stream_mux_transport: mux,
            active_streams: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn poll_ingress(
        &mut self,
    ) -> Result<Option<(u32, u8, Vec<u8>)>, kelir_interface::errors::PluginError> {
        // see: cans/01-architecture.md#VirtIO-Device-Complex
        let _ = (
            &self.control_transport,
            &self.stream_mux_transport,
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
        // see: cans/01-architecture.md#VirtIO-Device-Complex
        let _ = (
            &self.control_transport,
            &self.stream_mux_transport,
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
        // see: cans/11-protocol.md#Control-channel
        let _ = (
            &self.control_transport,
            &self.stream_mux_transport,
            &self.active_streams,
        );
        let _ = (msg_type, payload);
        Ok(())
    }
}
