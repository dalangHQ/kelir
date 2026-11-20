// see: cans/plugins/mcp.md#Service-exports-(mcp)
use std::path::Path;

use async_trait::async_trait;
use kelir_interface::plugin_model::{
    FsInterposer, HandlerCtx, LifecycleCtx, LifecycleParticipant, PluginError, TrapInterposer,
};
use kelir_interface::protocol_wire::{FilterAction, ProcessExit, TrapEvent, Verdict};

use crate::mcp_service::McpPlugin;

// see: cans/02-plugin-model.md#KelirPlugin-trait
#[async_trait]
impl LifecycleParticipant for McpPlugin {
    async fn on_vm_ready(&self, _ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        // L3-GAP: [governance_tools]: set up the JSON-RPC transport over MCP_CONSOLE and advertise capabilities per cans/plugins/mcp.md; slop: transport unbuilt, capabilities unadvertised, client cannot call tools
        // TODO (L3) [governance_tools]: set up the JSON-RPC transport over MCP_CONSOLE and advertise capabilities.; slop: the JSON-RPC transport never starts, no capabilities are advertised, MCP clients connect to a silent console.
        Ok(())
    }
}

// see: cans/plugins/mcp.md#MCP-hook-wiring
impl FsInterposer for McpPlugin {
    fn on_fs_read(&self, path: &Path, buffer: &[u8]) -> Result<Vec<u8>, PluginError> {
        let _ = path;
        // L3-GAP: [governance_tools]: expose guest reads as the read_file tool per cans/plugins/mcp.md; slop: guest reads unexposed, read_file tool missing, reads bypass policy
        // TODO (L3) [governance_tools]: expose guest reads as the read_file tool.; slop: guest reads are never exposed, the read_file tool is missing, MCP clients cannot read guest files.
        Ok(buffer.to_vec())
    }

    fn on_fs_write(
        &self,
        path: &Path,
        data: &mut [u8],
        pid: u32,
        ctx: &HandlerCtx,
    ) -> Result<FilterAction, PluginError> {
        let _ = (path, data, pid, ctx);
        // L3-GAP: [governance_tools]: expose guest writes as the write_file tool per cans/plugins/mcp.md; slop: guest writes unexposed, write_file tool missing, writes bypass policy
        // TODO (L3) [governance_tools]: expose guest writes as the write_file tool.; slop: guest writes are never exposed, the write_file tool is missing, MCP clients cannot write guest files.
        Ok(FilterAction::Forward)
    }
}

// see: cans/plugins/mcp.md#MCP-hook-wiring
#[async_trait]
impl TrapInterposer for McpPlugin {
    async fn on_trap(&self, event: &TrapEvent, ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        let _ = (event, ctx);
        // L3-GAP: [governance_tools]: correlate execute_command with the trapped execve per cans/plugins/mcp.md; slop: execute_command uncorrelated with trapped execve, command runs unobserved
        // TODO (L3) [governance_tools]: correlate execute_command with the trapped execve.; slop: execute_command is never correlated with the trapped execve, commands run without trap attribution, audit trails miss the execution.
        Ok(Verdict::Pass)
    }

    async fn on_process_exit(
        &self,
        event: &ProcessExit,
        ctx: &HandlerCtx,
    ) -> Result<(), PluginError> {
        let _ = (event, ctx);
        // L3-GAP: [governance_tools]: emit MCP task completion events per cans/plugins/mcp.md; slop: completion events never emitted, client never learns task outcome
        // TODO (L3) [governance_tools]: emit MCP task completion events.; slop: task completion events are never emitted, MCP clients never learn a task finished, orchestrators wait forever.
        Ok(())
    }
}
