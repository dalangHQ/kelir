// see: cans/plugins/mcp.md#Plugin-mcp
#![forbid(unsafe_code)]

// TODO (L3): tests/test_mcp_mapping.rs + examples/mcp-stdio.rs per cans/artifacts/config/project-structure.txt.
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{
    ConsoleInterposer, FsInterposer, HandlerCtx, KelirPlugin, LifecycleCtx, LifecycleParticipant,
    MountScope, NoDeps, PipelineBinder, PluginDeps, StreamFrameSink, TrapInterposer, VirtioDevice,
};
use kelir_interface::protocol_wire::{FilterAction, ProcessExit, StreamKind, TrapEvent, Verdict};
use kelir_interface::runtime_block::block_on;

// see: cans/plugins/mcp.md#Crate-dependencies-(mcp)
pub const MCP_SOCKET_PATH: &str = "/run/kelir/mcp.sock";

// see: cans/plugins/mcp.md#MCP-hook-wiring
#[derive(Default)]
struct McpServiceState {
    advertised_tools: Mutex<Vec<String>>,
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
    pub async fn route_request(&self, payload: &[u8]) -> Result<Vec<u8>, PluginError> {
        // TODO (L3): bridge JSON-RPC frames between MCP_CONSOLE and /run/kelir/mcp.sock per cans/plugins/mcp.md.
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
        kind: StreamKind,
        _flags: u8,
        payload: &[u8],
    ) -> Result<(), PluginError> {
        if kind != StreamKind::McpConsole {
            return Ok(());
        }
        block_on(self.service.route_request(payload))?;
        Ok(())
    }
}

// see: cans/plugins/mcp.md#Plugin-mcp
#[derive(Clone)]
pub struct McpPlugin {
    pub service: McpEndpointService,
}

impl McpPlugin {
    pub fn new() -> Self {
        Self {
            service: McpEndpointService::new(),
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
            StreamKind::McpConsole,
            sink,
            MountScope::default().with_stream_kinds([StreamKind::McpConsole]),
        );
    }
}

// see: cans/02-plugin-model.md#KelirPlugin-trait
impl PluginDeps for McpPlugin {
    type Deps = NoDeps;
}

#[async_trait]
impl LifecycleParticipant for McpPlugin {
    async fn on_vm_ready(&self, _ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        // TODO (L3): set up the JSON-RPC transport over MCP_CONSOLE and advertise capabilities per cans/plugins/mcp.md.
        Ok(())
    }
}

// see: cans/plugins/mcp.md#MCP-hook-wiring
#[async_trait]
impl ConsoleInterposer for McpPlugin {
    async fn on_console_input(&self, port: u32, bytes: &mut Vec<u8>) -> Result<(), PluginError> {
        let _ = (port, bytes);
        // TODO (L3): stream JSON-RPC frames from the MCP client into MCP_CONSOLE per cans/plugins/mcp.md.
        Ok(())
    }

    async fn on_console_output(&self, port: u32, chunk: &mut Vec<u8>) -> Result<(), PluginError> {
        let _ = (port, chunk);
        // TODO (L3): stream JSON-RPC frames from MCP_CONSOLE back to the MCP client per cans/plugins/mcp.md.
        Ok(())
    }
}

// see: cans/plugins/mcp.md#MCP-hook-wiring
impl FsInterposer for McpPlugin {
    fn on_fs_read(&self, path: &Path, buffer: &mut [u8]) -> Result<(), PluginError> {
        let _ = (path, buffer);
        // TODO (L3): expose guest reads as the read_file tool per cans/plugins/mcp.md.
        Ok(())
    }

    fn on_fs_write(
        &self,
        path: &Path,
        data: &mut [u8],
        pid: u32,
        ctx: &HandlerCtx,
    ) -> Result<FilterAction, PluginError> {
        let _ = (path, data, pid, ctx);
        // TODO (L3): expose guest writes as the write_file tool per cans/plugins/mcp.md.
        Ok(FilterAction::Forward)
    }
}

// see: cans/plugins/mcp.md#MCP-hook-wiring
#[async_trait]
impl TrapInterposer for McpPlugin {
    async fn on_trap(&self, event: &TrapEvent, ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        let _ = (event, ctx);
        // TODO (L3): correlate execute_command with the trapped execve per cans/plugins/mcp.md.
        Ok(Verdict::Pass)
    }

    async fn on_process_exit(
        &self,
        event: &ProcessExit,
        ctx: &HandlerCtx,
    ) -> Result<(), PluginError> {
        let _ = (event, ctx);
        // TODO (L3): emit MCP task completion events per cans/plugins/mcp.md.
        Ok(())
    }
}
