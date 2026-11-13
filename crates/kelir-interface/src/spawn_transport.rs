// see: cans/11-protocol.md#Binary-exec-channel
// see: cans/03-sdk-architecture.md#Params-typed-views
use crate::plugin_model::PluginError;
use crate::protocol_wire::StreamKind;

// see: cans/11-protocol.md#Binary-exec-channel
pub struct SpawnRequest {
    pub session_id: [u8; 16],
    pub stream_kind: StreamKind,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub limits_bytes: Vec<u8>,
    pub params_bytes: Vec<u8>,
}

// see: cans/11-protocol.md#Binary-exec-channel
// TODO (L2): encode the EXEC control frame onto the bound stream kind and submit it over the session transport.
pub trait SpawnTransport: Send + Sync + 'static {
    fn dispatch_exec(&self, request: &SpawnRequest) -> Result<(), PluginError>;
}
