// see: cans/plugins/mesh.md
// see: cans/plugins/vault.md
// see: cans/03-sdk-architecture.md
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::driver::Stream;
use crate::plugin::{HandlerCtx, PluginError};

// see: cans/plugins/mesh.md
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpiffeId {
    pub trust_domain: String,
    pub workload_prefix: String,
}

// see: cans/plugins/mesh.md
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpiffeSvid {
    pub spiffe_id: SpiffeId,
    pub credential: Vec<u8>,
    pub expires_at_ns: u64,
}

// see: cans/plugins/mesh.md
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerSessionHandshake {
    pub svid: SpiffeSvid,
    pub call_hops: u32,
    pub traceparent: Option<String>,
}

// see: cans/03-sdk-architecture.md
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VsockPeer {
    pub cid: u32,
    pub port: u32,
    pub spiffe_id: Option<SpiffeId>,
}

// see: cans/11-protocol.md#Control-channel
pub const VSOCK_CONTROL_PORT: u32 = 5252;

// see: cans/plugins/vault.md
pub const VAULT_METADATA_VSOCK_PORT: u32 = 80;

// see: cans/plugins/mesh.md
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VsockServiceSpec {
    pub port: u32,
    pub service: String,
}

// see: cans/03-sdk-architecture.md
#[async_trait]
pub trait VsockServiceHandler: Send + Sync {
    async fn on_connect(
        &self,
        peer: &VsockPeer,
        stream: Box<dyn Stream>,
        ctx: &HandlerCtx,
    ) -> Result<(), PluginError>;
}

// TODO (L3): core wires listener ports to the virtio-vsock guest CID and dispatches accepted connections to VsockServiceHandler.
