// see: cans/plugins/tool-guard.md#Plugin-tool-guard
#![forbid(unsafe_code)]

// TODO: [governance_tools]: tests/test_tool_guard.rs per cans/artifacts/config/project-structure.txt; slop: test_tool_guard.rs missing, guard verdicts untested, Pass stub passes vacuously (deferred to L3)
#[path = "guard.run.rs"]
pub mod guard;
#[path = "service_tool_guard.flow.rs"]
pub mod service_tool_guard;
#[path = "tool_guard_state.flow.rs"]
pub mod tool_guard_state;
#[path = "tool_guard_state.run.rs"]
pub mod tool_guard_state_run;

pub use service_tool_guard::{
    ApprovalHold, CrossServerDetector, CrossServerEvent, JsonRpcDenyError, ToolCapability,
    classify_tool_capabilities, confusable_tool_names, levenshtein, render_json_rpc_error,
    scan_tool_result,
};
pub use tool_guard_state::{ToolGuardCtx, ToolGuardPlugin};

// see: cans/plugins/tool-guard.md#Call-quota
#[derive(Clone, Debug)]
pub struct ApprovalAuthConfig {
    pub deny_self_approval: bool,
    pub methods: Vec<String>,
    pub max_pending_holds: u32,
    pub hold_ttl_seconds: u64,
}

// see: cans/plugins/tool-guard.md#Call-quota
#[derive(Clone, Debug)]
pub struct ToolGuardConfig {
    pub max_invocations_per_trace: u32,
    pub pin_schemas: bool,
    pub max_frame_bytes: u64,
    pub detect_semantic_rug_pull: bool,
    pub strict_ast_parameter_validation: bool,
    pub propagate_byte_range_taints: bool,
    pub detect_cross_server_exfil: bool,
    pub detect_confusable_names: bool,
    pub scan_tool_results: bool,
    pub approval_auth: ApprovalAuthConfig,
}

// see: cans/plugins/tool-guard.md#Host-extensible-hook-pipeline
#[derive(Clone, Debug, Default)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Vec<u8>,
}
