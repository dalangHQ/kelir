// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_08_mcp_server_stdio_mux_bridge.yaml#case_id
use kelir_interface::plugin_model::KelirPlugin;
use kelir_interface::protocol_wire::StreamKind;
use plugin_mcp::McpPlugin;

// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_08_mcp_server_stdio_mux_bridge.yaml#case_id
const TOOLS_CALL_FRAME: &[u8] =
    b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"fs.read\",\"arguments\":{\"path\":\"/workspace/notes.txt\"}}}\n";

// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_08_mcp_server_stdio_mux_bridge.yaml#case_id
#[tokio::test]
#[ignore]
async fn ad_08_mcp_server_stdio_mux_bridge() -> Result<(), String> {
    if StreamKind::McpConsole != StreamKind::from_u16(0x0012).ok_or("MCP_CONSOLE is unmapped")? {
        return Err("MCP console stream kind is not MCP_CONSOLE 0x0012".to_string());
    }
    let ctx = McpPlugin::new().context();
    match ctx.dispatch_mcp_console(TOOLS_CALL_FRAME).await {
        Ok(_) => Ok(()),
        Err(e) => Err(format!(
            "the tools/call frame never crossed onto MCP_CONSOLE 0x0012: {e}"
        )),
    }
}
