// see: cans/plugins/mcp.md#Service-exports-(mcp)
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{
    ConsoleInterposer, FsInterposer, KelirPlugin, MountScope, PipelineBinder, PluginDeps,
    StreamFrameSink, TrapInterposer, VirtioDevice,
};
use kelir_interface::protocol_wire::StreamKind;
use kelir_interface::runtime_block::block_on;

use crate::ast::{JsonRpcId, parse_json_rpc_request};
use crate::schema::WithParamsPredicate;

// see: cans/plugins/mcp.md#MCP-hook-wiring
#[derive(Default)]
struct McpServiceState {
    advertised_tools: Mutex<Vec<String>>,
    last_request_id: Mutex<Option<JsonRpcId>>,
}

// see: cans/plugins/mcp.md#Service-exports-(mcp)
#[derive(Clone, Default)]
pub struct McpEndpointService {
    state: Arc<McpServiceState>,
}

impl McpEndpointService {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock_advertised(&self) -> Result<std::sync::MutexGuard<'_, Vec<String>>, PluginError> {
        self.state
            .advertised_tools
            .lock()
            .map_err(|_| PluginError::Halt("mcp advertisement table poisoned".into()))
    }

    // see: cans/plugins/mcp.md#MCP-hook-wiring
    pub fn advertise_tools(&self, tools: Vec<String>) -> Result<(), PluginError> {
        *self.lock_advertised()? = tools;
        Ok(())
    }

    // see: cans/plugins/mcp.md#MCP-hook-wiring
    pub fn advertised_tools(&self) -> Result<Vec<String>, PluginError> {
        Ok(self.lock_advertised()?.clone())
    }

    // see: cans/plugins/mcp.md#MCP-contract
    pub fn last_request_id(&self) -> Result<Option<JsonRpcId>, PluginError> {
        Ok(self
            .state
            .last_request_id
            .lock()
            .map_err(|_| PluginError::Halt("mcp transaction table poisoned".into()))?
            .clone())
    }

    // see: cans/plugins/mcp.md#Invariants-and-policy-(mcp)
    pub fn with_params(
        &self,
        tool: &str,
        schema: &[u8],
    ) -> Result<WithParamsPredicate, PluginError> {
        WithParamsPredicate::compile(tool, schema)
    }

    // see: cans/plugins/mcp.md#MCP-contract
    pub async fn route_request(&self, payload: &[u8]) -> Result<Vec<u8>, PluginError> {
        let request = parse_json_rpc_request(payload)?;
        if let Ok(mut last) = self.state.last_request_id.lock() {
            *last = request.id.clone();
        }
        if crate::ast::is_sampling_create_message(&request.method) {
            return Err(PluginError::Halt(
                "ESecurityViolation: server-initiated sampling/createMessage is denied by policy"
                    .into(),
            ));
        }
        let _ = request;
        // L3-GAP: [governance_tools]: bridge JSON-RPC frames between MCP_CONSOLE and /run/kelir/mcp.sock per cans/plugins/mcp.md; slop: frames never bridged, MCP client isolated, tool calls never interposed
        Err(PluginError::Halt(format!(
            "MCP_CONSOLE transport routing pending (L3): {} bytes",
            payload.len()
        )))
    }
}

// see: cans/plugins/mcp.md#Service-exports-(mcp)
#[derive(Clone)]
pub struct McpCtx {
    service: McpEndpointService,
}

impl McpCtx {
    pub fn service(&self) -> &McpEndpointService {
        &self.service
    }

    // see: cans/11-protocol.md#Kind-registration
    pub async fn dispatch_mcp_console(&self, payload: &[u8]) -> Result<Vec<u8>, PluginError> {
        self.service.route_request(payload).await
    }
}

// see: cans/11-protocol.md#Kind-registration
// see: cans/plugins/mcp.md#MCP-contract
struct McpCtxStreamSink {
    service: McpEndpointService,
}

// see: cans/11-protocol.md#Kind-registration
impl StreamFrameSink for McpCtxStreamSink {
    fn on_stream_frame(
        &self,
        _stream_id: u32,
        kind: u16,
        _flags: u8,
        payload: &[u8],
    ) -> Result<(), PluginError> {
        if kind != StreamKind::McpConsole as u16 {
            return Ok(());
        }
        block_on(self.service.route_request(payload))?;
        Ok(())
    }
}

// see: cans/plugins/mcp.md#MCP-contract
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpConfig {
    pub max_frame_bytes: u64,
    // see: blindess.patch.md#C: server-initiated sampling lets a rogue MCP server drive the
    // client's LLM, so sampling/createMessage is denied by default.
    pub deny_server_sampling: bool,
    pub sampling_allowlist: Vec<String>,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            max_frame_bytes: 16_777_216,
            deny_server_sampling: true,
            sampling_allowlist: Vec::new(),
        }
    }
}

impl McpConfig {
    pub fn new(max_frame_bytes: u64) -> Self {
        Self {
            max_frame_bytes,
            deny_server_sampling: true,
            sampling_allowlist: Vec::new(),
        }
    }

    // see: cans/plugins/mcp.md#Invariants-and-policy-(mcp)
    // A sampling/createMessage request is only admitted when server-initiated sampling
    // is enabled AND the originating server is explicitly allowlisted.
    pub fn sampling_allowed_for(&self, server: &str) -> bool {
        !self.deny_server_sampling
            && self.sampling_allowlist.iter().any(|entry| entry == server)
    }
}

// see: cans/plugins/mcp.md#Plugin-mcp
#[derive(Clone, PluginDeps)]
pub struct McpPlugin {
    pub service: McpEndpointService,
    pub config: McpConfig,
}

impl McpPlugin {
    pub fn new() -> Self {
        Self {
            service: McpEndpointService::new(),
            config: McpConfig::default(),
        }
    }

    pub fn with_config(config: McpConfig) -> Self {
        Self {
            service: McpEndpointService::new(),
            config,
        }
    }
}

impl Default for McpPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for McpPlugin {
    type Context = McpCtx;
    fn context(&self) -> McpCtx {
        McpCtx {
            service: self.service.clone(),
        }
    }
    fn name(&self) -> &'static str {
        "mcp"
    }
    fn default_scope(&self) -> MountScope {
        // see: cans/11-protocol.md#Control-channel
        MountScope::default().with_devices([
            VirtioDevice::Console,
            VirtioDevice::Fs,
            VirtioDevice::Vsock,
        ])
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        // see: cans/plugins/mcp.md#MCP-hook-wiring
        let console_stage: Arc<dyn ConsoleInterposer> = Arc::new(self.clone());
        pipeline.bind_console_scoped(console_stage, self.default_scope());
        let fs_stage: Arc<dyn FsInterposer> = Arc::new(self.clone());
        pipeline.bind_fs_scoped(fs_stage, self.default_scope());
        let trap_stage: Arc<dyn TrapInterposer> = Arc::new(self.clone());
        pipeline.bind_trap_scoped(trap_stage, self.default_scope());
        // see: cans/11-protocol.md#Kind-registration
        let sink: Arc<dyn StreamFrameSink> = Arc::new(McpCtxStreamSink {
            service: self.service.clone(),
        });
        pipeline.bind_stream_kind_scoped(
            StreamKind::McpConsole as u16,
            sink,
            MountScope::default().with_stream_kinds([StreamKind::McpConsole]),
        );
    }
}

// see: cans/plugins/mcp.md#MCP-hook-wiring
#[async_trait]
impl ConsoleInterposer for McpPlugin {
    async fn on_console_input(&self, port: u32, bytes: &[u8]) -> Result<Vec<u8>, PluginError> {
        let _ = port;
        // L3-GAP: [governance_tools]: stream JSON-RPC frames from the MCP client into MCP_CONSOLE per cans/plugins/mcp.md; slop: client frames never reach MCP_CONSOLE, interposition bypassed
        Ok(bytes.to_vec())
    }

    async fn on_console_output(&self, port: u32, chunk: &[u8]) -> Result<Vec<u8>, PluginError> {
        let _ = port;
        // L3-GAP: [governance_tools]: stream JSON-RPC frames from MCP_CONSOLE back to the MCP client per cans/plugins/mcp.md; slop: console frames never return, client hangs awaiting response
        Ok(chunk.to_vec())
    }
}
