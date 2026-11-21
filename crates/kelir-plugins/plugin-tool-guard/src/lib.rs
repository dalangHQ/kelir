// see: cans/plugins/tool-guard.md#Plugin-tool-guard
#![forbid(unsafe_code)]

// L3-GAP: [governance_tools]: tests/test_tool_guard.rs per cans/artifacts/config/project-structure.txt; slop: test_tool_guard.rs missing, guard verdicts untested, Pass stub passes vacuously
#[path = "guard.run.rs"]
pub mod guard;
#[path = "service_tool_guard.flow.rs"]
pub mod service_tool_guard;
#[path = "tool_guard_state.flow.rs"]
pub mod tool_guard_state;
#[path = "tool_guard_state.run.rs"]
pub mod tool_guard_state_run;

pub use tool_guard_state::{ToolGuardCtx, ToolGuardPlugin};

// see: blindess.patch.md#C: role separation / self-approval denial / pending-hold cap + TTL
// for human approval resolution (ApprovalAuth), all model_codegen-safe field types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovalAuthConfig {
    pub deny_self_approval: bool,
    pub methods: Vec<String>,
    pub max_pending_holds: u32,
    pub hold_ttl_seconds: u64,
}

impl Default for ApprovalAuthConfig {
    fn default() -> Self {
        Self {
            deny_self_approval: true,
            // WebAuthn, TOTP, OIDC — mirrors agentsh internal/auth method set.
            methods: vec![
                "webauthn".to_string(),
                "totp".to_string(),
                "oidc".to_string(),
            ],
            max_pending_holds: 10,
            hold_ttl_seconds: 300,
        }
    }
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
    // see: blindess.patch.md#C: semantic cross-server exfiltration (read_then_send across
    // servers regardless of byte identity) that plugin-taint byte-range tracking misses.
    pub detect_cross_server_exfil: bool,
    // see: blindess.patch.md#C: Levenshtein-close / typosquatted tool name detection on
    // tools/list ingestion.
    pub detect_confusable_names: bool,
    // see: blindess.patch.md#C: hidden-instruction / prompt-injection scanning of tool
    // results before they reach the model context.
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
