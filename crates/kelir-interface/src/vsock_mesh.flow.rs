// see: cans/03-sdk-architecture.md#Runtime-surface
use crate::driver_interface::Stream;
use crate::plugin_model::{HandlerCtx, PluginError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// see: cans/03-sdk-architecture.md#Session-surfaces
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VsockPeer {
    pub cid: u32,
    pub port: u32,
}

// see: cans/11-protocol.md#Control-channel
pub const VSOCK_CONTROL_PORT: u32 = 5252;

// see: cans/03-sdk-architecture.md#Session-surfaces
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VsockServiceSpec {
    pub port: u32,
    pub service: String,
    pub schema: Option<serde_json::Value>,
}

// see: cans/03-sdk-architecture.md#Session-surfaces
#[async_trait]
pub trait VsockServiceHandler: Send + Sync {
    async fn on_connect(
        &self,
        peer: &VsockPeer,
        stream: Box<dyn Stream>,
        ctx: &HandlerCtx,
    ) -> Result<(), PluginError>;
}

// TODO: [wire_plane] fill in the L3 wave; core wires listener ports to the virtio-vsock guest CID and dispatches accepted connections to VsockServiceHandler.; slop: listener bound, guest CID dispatch never proven
