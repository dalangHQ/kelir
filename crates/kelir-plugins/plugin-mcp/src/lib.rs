// see: cans/plugins/mcp.md
#![forbid(unsafe_code)]

// TODO (L3): tests/test_mcp_mapping.rs + examples/mcp-stdio.rs per cans/artifacts/config/project-structure.txt.
use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{
    ConsoleInterposer, HandlerCtx, KelirPlugin, LifecycleParticipant, MountScope, TrapInterposer,
};
use kelir_interface::protocol::{TrapEvent, Verdict};

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
    fn name(&self) -> &'static str {
        "mcp"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
}

impl LifecycleParticipant for McpPlugin {}
impl ConsoleInterposer for McpPlugin {
    fn on_console_output(&self, _port: u32, _buf: &mut Vec<u8>) -> Result<(), PluginError> {
        // TODO: Bridge raw stdio frames to MCP JSON-RPC
        Ok(())
    }
}
impl TrapInterposer for McpPlugin {
    fn on_trap(&self, _event: &TrapEvent, _ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        Ok(Verdict::Pass)
    }
}
