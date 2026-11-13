// see: cans/plugins/tool-guard.md#Plugin-tool-guard
#![forbid(unsafe_code)]

// TODO (L3) [governance_tools]: tests/test_tool_guard.rs per cans/artifacts/config/project-structure.txt; slop: test_tool_guard.rs missing, guard verdicts untested, Pass stub passes vacuously
#[path = "guard.run.rs"]
pub mod guard;
#[path = "service_tool_guard.flow.rs"]
pub mod service_tool_guard;
#[path = "tool_guard_state.flow.rs"]
pub mod tool_guard_state;

pub use tool_guard_state::{ToolGuardCtx, ToolGuardPlugin};

// see: cans/plugins/tool-guard.md#Call-quota
#[derive(Clone, Debug)]
pub struct ToolGuardConfig {
    pub max_invocations_per_trace: u32,
    pub pin_schemas: bool,
}

// see: cans/plugins/tool-guard.md#Host-extensible-hook-pipeline
#[derive(Clone, Debug, Default)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Vec<u8>,
}
