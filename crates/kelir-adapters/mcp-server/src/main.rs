// see: cans/12-adapters.md#MCP-adapter
#![forbid(unsafe_code)]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO (L3) [ad_08_mcp_server_stdio_mux_bridge]: direct the stdio wire frames to plugin-mcp per cans/12-adapters.md#MCP-adapter.; slop: tool call succeeds via Port 0 PTY, 0x0012 never used
    Ok(())
}
