// see: cans/plugins/mcp.md#Plugin-mcp
#![forbid(unsafe_code)]

// L3-GAP: [governance_tools]: tests/test_mcp_mapping.rs + examples/mcp-stdio.rs per cans/artifacts/config/project-structure.txt; slop: mcp tests/examples missing, console bridging untested, frames unclassified

#[path = "ast.flow.rs"]
pub mod ast;

#[path = "mcp_service.flow.rs"]
pub mod mcp_service;
#[path = "mcp_service.run.rs"]
pub mod mcp_service_run;

#[path = "schema.flow.rs"]
pub mod schema;

pub use ast::{JsonRpcId, JsonRpcRequest, JsonRpcResponse, MAX_MCP_FRAME_BYTES, McpFrameAssembler};
pub use mcp_service::{McpConfig, McpCtx, McpEndpointService, McpPlugin};
pub use schema::WithParamsPredicate;

// see: cans/plugins/mcp.md#Crate-dependencies-(mcp)
pub const MCP_SOCKET_PATH: &str = "/run/kelir/mcp.sock";
