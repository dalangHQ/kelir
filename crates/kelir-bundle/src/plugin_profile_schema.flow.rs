// see: cans/02-plugin-model.md#Facade-blindness
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use kelir::interface::plugin_model::PluginError;
use plugin_ledger::LedgerConfig;
use plugin_mcp::McpConfig;
use plugin_tool_guard::ToolGuardConfig;

// see: cans/02-plugin-model.md#Facade-blindness
// plugin-owned sub-schema of the opaque `plugins:` profile section; the facade
// keeps only plane configs (see: cans/02-plugin-model.md#Facade-blindness)

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.hashing_standard
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HashingStandard {
    #[serde(rename = "RFC_6962_DUAL_BLAKE3_SHA256")]
    Rfc6962DualBlake3Sha256,
}

impl HashingStandard {
    pub fn as_str(&self) -> &'static str {
        match self {
            HashingStandard::Rfc6962DualBlake3Sha256 => "RFC_6962_DUAL_BLAKE3_SHA256",
        }
    }
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MerkleWalProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hashing_standard: Option<HashingStandard>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign_leaves: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fsync_on_reap: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_pre_redaction_hash: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_hook_spans_in_root: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub witness_cosigning_endpoint: Option<String>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.otel
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OtelProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub propagate_w3c_traceparent: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_diagnostic_pmu_gauges: Option<bool>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerPluginProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merkle_wal: Option<MerkleWalProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub otel: Option<OtelProfile>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.tool_guard.mcp_interposition
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpInterpositionProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin_schemas: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub halt_on_schema_drift: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detect_semantic_rug_pull: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict_ast_parameter_validation: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_invocations_per_trace: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_frame_bytes: Option<u64>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.tool_guard.taint_governance
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaintGovernanceProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub untrusted_ingress_taints: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub propagate_byte_range_taints: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mutating_verbs_require_approval: Option<bool>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.tool_guard
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolGuardPluginProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_interposition: Option<McpInterpositionProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taint_governance: Option<TaintGovernanceProfile>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.mcp
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpPluginProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socket_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub framing: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_frame_bytes: Option<u64>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginsProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ledger: Option<LedgerPluginProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_guard: Option<ToolGuardPluginProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp: Option<McpPluginProfile>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// see: cans/02-plugin-model.md#Facade-blindness
fn halt(message: String) -> PluginError {
    PluginError::Halt(message)
}

// see: cans/02-plugin-model.md#Facade-blindness
fn security_violation(message: String) -> PluginError {
    PluginError::Halt(format!("ESecurityViolation: {message}"))
}

fn check_ceiling(path: &str, base: Option<u64>, overlay: Option<u64>) -> Result<(), PluginError> {
    if let (Some(base), Some(overlay)) = (base, overlay)
        && overlay > base
    {
        return Err(security_violation(format!(
            "overlay widens {path} from {base} to {overlay}"
        )));
    }
    Ok(())
}

fn check_guard_kept(
    path: &str,
    base: Option<bool>,
    overlay: Option<bool>,
) -> Result<(), PluginError> {
    if base == Some(true) && overlay == Some(false) {
        return Err(security_violation(format!(
            "overlay drops guard {path} from true to false"
        )));
    }
    Ok(())
}

// see: cans/plugins/profile.md#Monotonic-limits
pub fn plugins_section(profile: &kelir::sandbox_runtime::SandboxProfile) -> Option<PluginsProfile> {
    profile
        .plugins
        .as_ref()
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}

// see: cans/plugins/profile.md#Monotonic-limits
pub fn validate_plugins(plugins: &PluginsProfile) -> Result<(), PluginError> {
    if let Some(interposition) = plugins
        .tool_guard
        .as_ref()
        .and_then(|t| t.mcp_interposition.as_ref())
        && interposition.max_frame_bytes == Some(0)
    {
        return Err(halt(
            "plugins.tool_guard.mcp_interposition.max_frame_bytes must be greater than zero"
                .to_string(),
        ));
    }
    if let Some(mcp) = plugins.mcp.as_ref()
        && mcp.max_frame_bytes == Some(0)
    {
        return Err(halt(
            "plugins.mcp.max_frame_bytes must be greater than zero".to_string(),
        ));
    }
    Ok(())
}

// see: cans/plugins/profile.md#Monotonic-limits
pub fn validate_plugins_overlay(
    base: &PluginsProfile,
    overlay: &PluginsProfile,
) -> Result<(), PluginError> {
    validate_plugins(overlay)?;
    let base_wal = base.ledger.as_ref().and_then(|l| l.merkle_wal.as_ref());
    let wal = overlay.ledger.as_ref().and_then(|l| l.merkle_wal.as_ref());
    check_guard_kept(
        "plugins.ledger.merkle_wal.sign_leaves",
        base_wal.and_then(|w| w.sign_leaves),
        wal.and_then(|w| w.sign_leaves),
    )?;
    check_guard_kept(
        "plugins.ledger.merkle_wal.fsync_on_reap",
        base_wal.and_then(|w| w.fsync_on_reap),
        wal.and_then(|w| w.fsync_on_reap),
    )?;
    check_guard_kept(
        "plugins.ledger.merkle_wal.record_pre_redaction_hash",
        base_wal.and_then(|w| w.record_pre_redaction_hash),
        wal.and_then(|w| w.record_pre_redaction_hash),
    )?;
    check_guard_kept(
        "plugins.ledger.merkle_wal.include_hook_spans_in_root",
        base_wal.and_then(|w| w.include_hook_spans_in_root),
        wal.and_then(|w| w.include_hook_spans_in_root),
    )?;
    let base_guard = base.tool_guard.as_ref();
    let guard = overlay.tool_guard.as_ref();
    let base_interposition = base_guard.and_then(|g| g.mcp_interposition.as_ref());
    let interposition = guard.and_then(|g| g.mcp_interposition.as_ref());
    check_ceiling(
        "plugins.tool_guard.mcp_interposition.max_frame_bytes",
        base_interposition.and_then(|i| i.max_frame_bytes),
        interposition.and_then(|i| i.max_frame_bytes),
    )?;
    check_ceiling(
        "plugins.tool_guard.mcp_interposition.max_invocations_per_trace",
        base_interposition.and_then(|i| i.max_invocations_per_trace.map(u64::from)),
        interposition.and_then(|i| i.max_invocations_per_trace.map(u64::from)),
    )?;
    check_guard_kept(
        "plugins.tool_guard.mcp_interposition.pin_schemas",
        base_interposition.and_then(|i| i.pin_schemas),
        interposition.and_then(|i| i.pin_schemas),
    )?;
    check_guard_kept(
        "plugins.tool_guard.mcp_interposition.halt_on_schema_drift",
        base_interposition.and_then(|i| i.halt_on_schema_drift),
        interposition.and_then(|i| i.halt_on_schema_drift),
    )?;
    check_guard_kept(
        "plugins.tool_guard.mcp_interposition.detect_semantic_rug_pull",
        base_interposition.and_then(|i| i.detect_semantic_rug_pull),
        interposition.and_then(|i| i.detect_semantic_rug_pull),
    )?;
    check_guard_kept(
        "plugins.tool_guard.mcp_interposition.strict_ast_parameter_validation",
        base_interposition.and_then(|i| i.strict_ast_parameter_validation),
        interposition.and_then(|i| i.strict_ast_parameter_validation),
    )?;
    let base_taint = base_guard.and_then(|g| g.taint_governance.as_ref());
    let taint = guard.and_then(|g| g.taint_governance.as_ref());
    check_guard_kept(
        "plugins.tool_guard.taint_governance.untrusted_ingress_taints",
        base_taint.and_then(|t| t.untrusted_ingress_taints),
        taint.and_then(|t| t.untrusted_ingress_taints),
    )?;
    check_guard_kept(
        "plugins.tool_guard.taint_governance.propagate_byte_range_taints",
        base_taint.and_then(|t| t.propagate_byte_range_taints),
        taint.and_then(|t| t.propagate_byte_range_taints),
    )?;
    check_guard_kept(
        "plugins.tool_guard.taint_governance.mutating_verbs_require_approval",
        base_taint.and_then(|t| t.mutating_verbs_require_approval),
        taint.and_then(|t| t.mutating_verbs_require_approval),
    )?;
    check_ceiling(
        "plugins.mcp.max_frame_bytes",
        base.mcp.as_ref().and_then(|m| m.max_frame_bytes),
        overlay.mcp.as_ref().and_then(|m| m.max_frame_bytes),
    )?;
    Ok(())
}

// see: cans/plugins/ledger.md#Plugin-ledger
pub fn to_ledger_config(plugins: &PluginsProfile) -> LedgerConfig {
    let mut config = LedgerConfig::default();
    if let Some(ledger) = plugins.ledger.as_ref() {
        if let Some(wal) = ledger.merkle_wal.as_ref() {
            if let Some(store) = wal.store_path.clone() {
                config.store_path = PathBuf::from(store);
            }
            if let Some(enabled) = wal.enabled {
                config.merkle_chain = enabled;
            }
            if let Some(standard) = wal.hashing_standard {
                config.hashing_standard =
                    plugin_ledger::HashingStandard::from_name(standard.as_str())
                        .unwrap_or_default();
            }
            if let Some(record) = wal.record_pre_redaction_hash {
                config.record_pre_redaction_hash = record;
            }
            if let Some(include) = wal.include_hook_spans_in_root {
                config.include_hook_spans_in_root = include;
            }
            config.witness_cosigning_endpoint = wal.witness_cosigning_endpoint.clone();
        }
        if let Some(otel) = ledger.otel.as_ref() {
            let mut otel_config = plugin_ledger::OtelConfig::default();
            if let Some(endpoint) = otel.endpoint.clone() {
                otel_config.endpoint = endpoint;
            }
            if let Some(service) = otel.service_name.clone() {
                otel_config.service_name = service;
            }
            if let Some(propagate) = otel.propagate_w3c_traceparent {
                otel_config.propagate_w3c_traceparent = propagate;
            }
            if let Some(gauges) = otel.export_diagnostic_pmu_gauges {
                otel_config.export_diagnostic_pmu_gauges = gauges;
            }
            config.otel = Some(otel_config);
        }
    }
    config
}

// see: cans/plugins/tool-guard.md#Plugin-tool-guard
pub fn to_tool_guard_config(plugins: &PluginsProfile) -> ToolGuardConfig {
    let mut config = ToolGuardConfig::default();
    if let Some(tool_guard) = plugins.tool_guard.as_ref() {
        if let Some(interposition) = tool_guard.mcp_interposition.as_ref() {
            if let Some(pin) = interposition.pin_schemas {
                config.pin_schemas = pin;
            }
            if let Some(max) = interposition.max_invocations_per_trace {
                config.max_invocations_per_trace = max;
            }
            if let Some(bytes) = interposition.max_frame_bytes {
                config.max_frame_bytes = bytes;
            }
            if let Some(detect) = interposition.detect_semantic_rug_pull {
                config.detect_semantic_rug_pull = detect;
            }
            if let Some(strict) = interposition.strict_ast_parameter_validation {
                config.strict_ast_parameter_validation = strict;
            }
        }
        if let Some(taint) = tool_guard.taint_governance.as_ref()
            && let Some(propagate) = taint.propagate_byte_range_taints
        {
            config.propagate_byte_range_taints = propagate;
        }
    }
    config
}

// see: cans/plugins/mcp.md#Plugin-mcp
pub fn to_mcp_config(plugins: &PluginsProfile) -> McpConfig {
    let mut config = McpConfig::default();
    if let Some(mcp) = plugins.mcp.as_ref()
        && let Some(bytes) = mcp.max_frame_bytes
    {
        config.max_frame_bytes = bytes;
    }
    config
}
