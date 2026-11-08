// see: cans/plugins/tool-guard.md#Plugin-tool-guard
// see: cans/plugins/mcp.md#MCP-contract
use kelir_interface::plugin::PluginError;

// see: cans/plugins/mcp.md#MCP-contract
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum McpMethod {
    ToolsList,
    ToolsCall,
    #[default]
    Other,
}

// see: cans/11-protocol.md#Stream-mux-framing
#[derive(Debug, Clone, Default)]
pub struct McpFrame {
    pub method: McpMethod,
    pub id: u64,
    pub tool_name: Option<String>,
    pub arguments: Vec<u8>,
}

impl McpFrame {
    pub fn decode(payload: &[u8]) -> Result<Self, PluginError> {
        let _ = payload;
        // TODO (L3): decode JSON Lines frames and classify tools/call vs tools/list per cans/plugins/mcp.md; FLAG_CONTINUATION reassembly per cans/11-protocol.md#Stream-mux-framing.
        Ok(Self::default())
    }
}
