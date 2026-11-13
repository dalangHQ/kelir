// see: cans/plugins/mcp.md#Plugin-mcp
#![forbid(unsafe_code)]

// TODO (L3) [governance_tools]: tests/test_mcp_mapping.rs + examples/mcp-stdio.rs per cans/artifacts/config/project-structure.txt; slop: mcp tests/examples missing, console bridging untested, frames unclassified

#[path = "mcp_service.flow.rs"]
pub mod mcp_service;

pub use mcp_service::{McpCtx, McpEndpointService, McpPlugin};

// see: cans/plugins/mcp.md#Crate-dependencies-(mcp)
pub const MCP_SOCKET_PATH: &str = "/run/kelir/mcp.sock";
