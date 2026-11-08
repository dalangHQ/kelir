// see: cans/plugins/mcp.md
#![forbid(unsafe_code)]

// TODO (L3): tests/test_mcp_mapping.rs + examples/mcp-stdio.rs per cans/artifacts/config/project-structure.txt.
use std::sync::Arc;

use async_trait::async_trait;
use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{
    HandlerCtx, KelirPlugin, LifecycleParticipant, MountScope, PipelineBinder, StreamInterposer,
};
use kelir_interface::protocol::{StreamDirection, StreamKind, Verdict};

// see: cans/plugins/mcp.md
pub const MCP_SOCKET_PATH: &str = "/run/kelir/mcp.sock";

#[derive(Clone)]
pub struct McpEndpointService;

#[derive(Clone)]
pub struct McpPlugin {
    pub service: McpEndpointService,
}

impl McpPlugin {
    pub fn new() -> Self {
        Self {
            service: McpEndpointService,
        }
    }
}

impl Default for McpPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for McpPlugin {
    type Context = ();
    fn context(&self) {}
    fn name(&self) -> &'static str {
        "mcp"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        let stage: Arc<dyn StreamInterposer> = Arc::new(self.clone());
        pipeline.bind_stream(StreamKind::McpConsole, stage);
        pipeline
            .service_registry()
            .register(Arc::new(self.service.clone()));
    }
}

impl LifecycleParticipant for McpPlugin {}

#[async_trait]
impl StreamInterposer for McpPlugin {
    async fn on_stream_open(
        &self,
        stream_id: u32,
        tags: &mut Vec<u8>,
        ctx: &HandlerCtx,
    ) -> Result<Verdict, PluginError> {
        let _ = (stream_id, tags, ctx);
        // TODO (L3): validate MCP_CONSOLE open tags and advertise JSON-RPC capabilities per cans/plugins/mcp.md.
        Ok(Verdict::Pass)
    }

    async fn on_stream_data(
        &self,
        direction: StreamDirection,
        stream_id: u32,
        payload: &mut Vec<u8>,
        ctx: &HandlerCtx,
    ) -> Result<Verdict, PluginError> {
        let _ = (stream_id, ctx);
        match direction {
            StreamDirection::GuestToHost => {
                // TODO (L3): bridge JSON-RPC frames from MCP_CONSOLE to /run/kelir/mcp.sock.
                let _ = payload;
                Ok(Verdict::Pass)
            }
            StreamDirection::HostToGuest => {
                // TODO (L3): bridge JSON-RPC frames from /run/kelir/mcp.sock back to MCP_CONSOLE.
                let _ = payload;
                Ok(Verdict::Pass)
            }
        }
    }

    async fn on_stream_close(&self, stream_id: u32, ctx: &HandlerCtx) -> Result<(), PluginError> {
        let _ = (stream_id, ctx);
        Ok(())
    }
}
