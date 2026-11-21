// see: cans/02-plugin-model.md#Facade-blindness
#[path = "plugin_profile_schema_extra.flow.rs"]
pub mod plugin_profiles_extra;

pub use plugin_profiles_extra::{
    DbProxyPluginProfile, DeclaredServiceProfile, EgressProxyPluginProfile, L7HttpPluginProfile,
    PackageGuardProfile, PgSchemaRewriteProfile, PgWireRewriteProfile, RedactionPluginProfile,
    SoftDeleteProfile, SqliteProfile, SupplyChainPluginProfile, VaultCredentialProfile,
    VaultMetadataProfile, VaultPluginProfile, WxJitProfile,
};

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use kelir::interface::plugin_model::PluginError;
use plugin_ledger::{LedgerConfig, MerkleWalConfig};
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kms_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_signing_key_bytes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify_key_fingerprint: Option<bool>,
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
    pub strict: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ocsf_mapping: Option<bool>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detect_cross_server_exfil: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detect_confusable_names: Option<bool>,
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

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.tool_guard.sanitization
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SanitizationProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_arguments: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scrub_output_secrets: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_tool_results: Option<bool>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.tool_guard.approval_auth
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalAuthProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_self_approval: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub methods: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_pending_holds: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold_ttl_seconds: Option<u64>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sanitization: Option<SanitizationProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_auth: Option<ApprovalAuthProfile>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_server_sampling: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampling_allowlist: Option<Vec<String>>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub db_proxy: Option<DbProxyPluginProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supply_chain: Option<SupplyChainPluginProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vault: Option<VaultPluginProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub l7_http: Option<L7HttpPluginProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub egress_proxy: Option<EgressProxyPluginProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redaction: Option<RedactionPluginProfile>,
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

fn check_floor(path: &str, base: Option<u64>, overlay: Option<u64>) -> Result<(), PluginError> {
    if let (Some(base), Some(overlay)) = (base, overlay)
        && overlay < base
    {
        return Err(security_violation(format!(
            "overlay lowers floor {path} from {base} to {overlay}"
        )));
    }
    Ok(())
}

fn check_list_kept(
    path: &str,
    base: Option<&Vec<String>>,
    overlay: Option<&Vec<String>>,
) -> Result<(), PluginError> {
    if let (Some(base), Some(overlay)) = (base, overlay) {
        for entry in base {
            if !overlay.contains(entry) {
                return Err(security_violation(format!(
                    "overlay removes {path} entry {entry}"
                )));
            }
        }
    }
    Ok(())
}

fn check_value_kept(
    path: &str,
    base: Option<&str>,
    overlay: Option<&str>,
) -> Result<(), PluginError> {
    if let (Some(base), Some(overlay)) = (base, overlay)
        && base != overlay
    {
        return Err(security_violation(format!(
            "overlay changes {path} from {base} to {overlay}"
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
    if let Some(wal) = plugins.ledger.as_ref().and_then(|l| l.merkle_wal.as_ref()) {
        if wal.kms_provider.is_some() && wal.min_signing_key_bytes.is_some_and(|bytes| bytes < 32) {
            return Err(halt(
                "plugins.ledger.merkle_wal.min_signing_key_bytes must be at least 32".to_string(),
            ));
        }
        if wal.min_signing_key_bytes == Some(0) {
            return Err(halt(
                "plugins.ledger.merkle_wal.min_signing_key_bytes must be greater than zero"
                    .to_string(),
            ));
        }
    }
    if let Some(auth) = plugins
        .tool_guard
        .as_ref()
        .and_then(|t| t.approval_auth.as_ref())
    {
        if auth.max_pending_holds == Some(0) {
            return Err(halt(
                "plugins.tool_guard.approval_auth.max_pending_holds must be greater than zero"
                    .to_string(),
            ));
        }
        if auth.hold_ttl_seconds == Some(0) {
            return Err(halt(
                "plugins.tool_guard.approval_auth.hold_ttl_seconds must be greater than zero"
                    .to_string(),
            ));
        }
        if let Some(methods) = auth.methods.as_ref()
            && methods.iter().any(|method| method.is_empty())
        {
            return Err(halt(
                "plugins.tool_guard.approval_auth.methods must not contain empty entries"
                    .to_string(),
            ));
        }
    }
    if let Some(metadata) = plugins
        .vault
        .as_ref()
        .and_then(|v| v.metadata_service.as_ref())
        && metadata.token_ttl_seconds.is_some_and(|ttl| ttl > 900)
    {
        return Err(halt(
            "plugins.vault.metadata_service.token_ttl_seconds must not exceed 900".to_string(),
        ));
    }
    if let Some(postgres) = plugins.db_proxy.as_ref().and_then(|d| d.postgres.as_ref()) {
        if postgres.listen_port == Some(0) {
            return Err(halt(
                "plugins.db_proxy.postgres.listen_port must be greater than zero".to_string(),
            ));
        }
        if postgres.max_rewrites_per_conn == Some(0) {
            return Err(halt(
                "plugins.db_proxy.postgres.max_rewrites_per_conn must be greater than zero"
                    .to_string(),
            ));
        }
    }
    if let Some(services) = plugins
        .l7_http
        .as_ref()
        .and_then(|l| l.declared_services.as_ref())
    {
        for service in services {
            if service.name.as_deref().is_some_and(|name| name.is_empty()) {
                return Err(halt(
                    "plugins.l7_http.declared_services entries must carry a non-empty name"
                        .to_string(),
                ));
            }
            if service
                .upstream
                .as_deref()
                .is_some_and(|upstream| upstream.is_empty())
            {
                return Err(halt(
                    "plugins.l7_http.declared_services entries must carry a non-empty upstream"
                        .to_string(),
                ));
            }
        }
    }
    if let Some(soft_delete) = plugins
        .redaction
        .as_ref()
        .and_then(|r| r.soft_delete.as_ref())
        && soft_delete.enabled == Some(true)
        && soft_delete
            .trash_dir
            .as_deref()
            .is_some_and(|dir| dir.is_empty())
    {
        return Err(halt(
            "plugins.redaction.soft_delete.trash_dir must be non-empty when soft_delete is enabled"
                .to_string(),
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
    let base_ledger = base.ledger.as_ref();
    let ledger = overlay.ledger.as_ref();
    check_guard_kept(
        "plugins.ledger.strict",
        base_ledger.and_then(|l| l.strict),
        ledger.and_then(|l| l.strict),
    )?;
    check_guard_kept(
        "plugins.ledger.ocsf_mapping",
        base_ledger.and_then(|l| l.ocsf_mapping),
        ledger.and_then(|l| l.ocsf_mapping),
    )?;
    let base_wal = base_ledger.and_then(|l| l.merkle_wal.as_ref());
    let wal = ledger.and_then(|l| l.merkle_wal.as_ref());
    check_guard_kept(
        "plugins.ledger.merkle_wal.verify_key_fingerprint",
        base_wal.and_then(|w| w.verify_key_fingerprint),
        wal.and_then(|w| w.verify_key_fingerprint),
    )?;
    check_floor(
        "plugins.ledger.merkle_wal.min_signing_key_bytes",
        base_wal.and_then(|w| w.min_signing_key_bytes.map(u64::from)),
        wal.and_then(|w| w.min_signing_key_bytes.map(u64::from)),
    )?;
    check_value_kept(
        "plugins.ledger.merkle_wal.kms_provider",
        base_wal.and_then(|w| w.kms_provider.as_deref()),
        wal.and_then(|w| w.kms_provider.as_deref()),
    )?;
    let base_auth = base_guard.and_then(|g| g.approval_auth.as_ref());
    let auth = guard.and_then(|g| g.approval_auth.as_ref());
    check_guard_kept(
        "plugins.tool_guard.approval_auth.deny_self_approval",
        base_auth.and_then(|a| a.deny_self_approval),
        auth.and_then(|a| a.deny_self_approval),
    )?;
    check_ceiling(
        "plugins.tool_guard.approval_auth.max_pending_holds",
        base_auth.and_then(|a| a.max_pending_holds.map(u64::from)),
        auth.and_then(|a| a.max_pending_holds.map(u64::from)),
    )?;
    check_ceiling(
        "plugins.tool_guard.approval_auth.hold_ttl_seconds",
        base_auth.and_then(|a| a.hold_ttl_seconds),
        auth.and_then(|a| a.hold_ttl_seconds),
    )?;
    check_list_kept(
        "plugins.tool_guard.approval_auth.methods",
        base_auth.and_then(|a| a.methods.as_ref()),
        auth.and_then(|a| a.methods.as_ref()),
    )?;
    check_guard_kept(
        "plugins.tool_guard.mcp_interposition.detect_cross_server_exfil",
        base_interposition.and_then(|i| i.detect_cross_server_exfil),
        interposition.and_then(|i| i.detect_cross_server_exfil),
    )?;
    check_guard_kept(
        "plugins.tool_guard.mcp_interposition.detect_confusable_names",
        base_interposition.and_then(|i| i.detect_confusable_names),
        interposition.and_then(|i| i.detect_confusable_names),
    )?;
    let base_sanitization = base_guard.and_then(|g| g.sanitization.as_ref());
    let sanitization = guard.and_then(|g| g.sanitization.as_ref());
    check_guard_kept(
        "plugins.tool_guard.sanitization.scan_tool_results",
        base_sanitization.and_then(|s| s.scan_tool_results),
        sanitization.and_then(|s| s.scan_tool_results),
    )?;
    check_guard_kept(
        "plugins.mcp.deny_server_sampling",
        base.mcp.as_ref().and_then(|m| m.deny_server_sampling),
        overlay.mcp.as_ref().and_then(|m| m.deny_server_sampling),
    )?;
    check_list_kept(
        "plugins.mcp.sampling_allowlist",
        base.mcp
            .as_ref()
            .and_then(|m| m.sampling_allowlist.as_ref()),
        overlay
            .mcp
            .as_ref()
            .and_then(|m| m.sampling_allowlist.as_ref()),
    )?;
    let base_postgres = base.db_proxy.as_ref().and_then(|d| d.postgres.as_ref());
    let postgres = overlay.db_proxy.as_ref().and_then(|d| d.postgres.as_ref());
    check_guard_kept(
        "plugins.db_proxy.postgres.require_where_on_mutations",
        base_postgres.and_then(|p| p.require_where_on_mutations),
        postgres.and_then(|p| p.require_where_on_mutations),
    )?;
    check_guard_kept(
        "plugins.db_proxy.postgres.block_unsafe_io",
        base_postgres.and_then(|p| p.block_unsafe_io),
        postgres.and_then(|p| p.block_unsafe_io),
    )?;
    check_guard_kept(
        "plugins.db_proxy.postgres.catalog_resolution",
        base_postgres.and_then(|p| p.catalog_resolution),
        postgres.and_then(|p| p.catalog_resolution),
    )?;
    check_guard_kept(
        "plugins.db_proxy.postgres.synthetic_cancel_keys",
        base_postgres.and_then(|p| p.synthetic_cancel_keys),
        postgres.and_then(|p| p.synthetic_cancel_keys),
    )?;
    check_value_kept(
        "plugins.db_proxy.postgres.deny_mode_in_tx",
        base_postgres.and_then(|p| p.deny_mode_in_tx.as_deref()),
        postgres.and_then(|p| p.deny_mode_in_tx.as_deref()),
    )?;
    check_value_kept(
        "plugins.db_proxy.postgres.ast_parser",
        base_postgres.and_then(|p| p.ast_parser.as_deref()),
        postgres.and_then(|p| p.ast_parser.as_deref()),
    )?;
    let base_guard_chain = base
        .supply_chain
        .as_ref()
        .and_then(|s| s.package_guard.as_ref());
    let guard_chain = overlay
        .supply_chain
        .as_ref()
        .and_then(|s| s.package_guard.as_ref());
    check_guard_kept(
        "plugins.supply_chain.package_guard.resolve_lockfiles",
        base_guard_chain.and_then(|p| p.resolve_lockfiles),
        guard_chain.and_then(|p| p.resolve_lockfiles),
    )?;
    check_guard_kept(
        "plugins.supply_chain.package_guard.strip_private_scopes",
        base_guard_chain.and_then(|p| p.strip_private_scopes),
        guard_chain.and_then(|p| p.strip_private_scopes),
    )?;
    check_guard_kept(
        "plugins.supply_chain.package_guard.vet_skills",
        base_guard_chain.and_then(|p| p.vet_skills),
        guard_chain.and_then(|p| p.vet_skills),
    )?;
    check_list_kept(
        "plugins.supply_chain.package_guard.advisories",
        base_guard_chain.and_then(|p| p.advisories.as_ref()),
        guard_chain.and_then(|p| p.advisories.as_ref()),
    )?;
    let base_metadata = base
        .vault
        .as_ref()
        .and_then(|v| v.metadata_service.as_ref());
    let metadata = overlay
        .vault
        .as_ref()
        .and_then(|v| v.metadata_service.as_ref());
    check_guard_kept(
        "plugins.vault.metadata_service.credsub_fake_tokens",
        base_metadata.and_then(|m| m.credsub_fake_tokens),
        metadata.and_then(|m| m.credsub_fake_tokens),
    )?;
    check_guard_kept(
        "plugins.vault.metadata_service.scrub_responses",
        base_metadata.and_then(|m| m.scrub_responses),
        metadata.and_then(|m| m.scrub_responses),
    )?;
    check_guard_kept(
        "plugins.vault.metadata_service.fail_closed",
        base_metadata.and_then(|m| m.fail_closed),
        metadata.and_then(|m| m.fail_closed),
    )?;
    check_guard_kept(
        "plugins.vault.metadata_service.zeroize_on_drop",
        base_metadata.and_then(|m| m.zeroize_on_drop),
        metadata.and_then(|m| m.zeroize_on_drop),
    )?;
    check_list_kept(
        "plugins.l7_http.threat_feeds",
        base.l7_http.as_ref().and_then(|l| l.threat_feeds.as_ref()),
        overlay
            .l7_http
            .as_ref()
            .and_then(|l| l.threat_feeds.as_ref()),
    )?;
    check_guard_kept(
        "plugins.l7_http.hash_only_llm_retention",
        base.l7_http
            .as_ref()
            .and_then(|l| l.hash_only_llm_retention),
        overlay
            .l7_http
            .as_ref()
            .and_then(|l| l.hash_only_llm_retention),
    )?;
    for (base_service, service) in base
        .l7_http
        .as_ref()
        .and_then(|l| l.declared_services.as_ref())
        .map(|services| services.iter())
        .into_iter()
        .flatten()
        .filter_map(|base_service| {
            let name = base_service.name.as_deref()?;
            let service = overlay
                .l7_http
                .as_ref()
                .and_then(|l| l.declared_services.as_ref())?
                .iter()
                .find(|candidate| candidate.name.as_deref() == Some(name))?;
            Some((base_service, service))
        })
    {
        check_guard_kept(
            "plugins.l7_http.declared_services.block_direct_egress",
            base_service.block_direct_egress,
            service.block_direct_egress,
        )?;
        check_guard_kept(
            "plugins.l7_http.declared_services.dlp_redact_prompts",
            base_service.dlp_redact_prompts,
            service.dlp_redact_prompts,
        )?;
        check_guard_kept(
            "plugins.l7_http.declared_services.inspect_sse_tool_use",
            base_service.inspect_sse_tool_use,
            service.inspect_sse_tool_use,
        )?;
    }
    check_guard_kept(
        "plugins.egress_proxy.block_tor",
        base.egress_proxy.as_ref().and_then(|e| e.block_tor),
        overlay.egress_proxy.as_ref().and_then(|e| e.block_tor),
    )?;
    check_guard_kept(
        "plugins.egress_proxy.deny_socks5_resolve",
        base.egress_proxy
            .as_ref()
            .and_then(|e| e.deny_socks5_resolve),
        overlay
            .egress_proxy
            .as_ref()
            .and_then(|e| e.deny_socks5_resolve),
    )?;
    check_guard_kept(
        "plugins.redaction.soft_delete.enabled",
        base.redaction
            .as_ref()
            .and_then(|r| r.soft_delete.as_ref())
            .and_then(|s| s.enabled),
        overlay
            .redaction
            .as_ref()
            .and_then(|r| r.soft_delete.as_ref())
            .and_then(|s| s.enabled),
    )?;
    Ok(())
}

// see: cans/plugins/ledger.md#Plugin-ledger
pub fn to_ledger_config(plugins: &PluginsProfile) -> LedgerConfig {
    let mut config = LedgerConfig::default();
    if let Some(ledger) = plugins.ledger.as_ref() {
        if let Some(strict) = ledger.strict {
            config.strict = strict;
        }
        if let Some(ocsf) = ledger.ocsf_mapping {
            config.ocsf_mapping = ocsf;
        }
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

// see: cans/plugins/ledger.md#Merkle-WAL-specification
pub fn to_merkle_wal_config(plugins: &PluginsProfile) -> MerkleWalConfig {
    let mut config = MerkleWalConfig::default();
    if let Some(wal) = plugins.ledger.as_ref().and_then(|l| l.merkle_wal.as_ref()) {
        if let Some(store) = wal.store_path.clone() {
            config.store_path = PathBuf::from(store);
        }
        if let Some(standard) = wal.hashing_standard {
            config.hashing_standard =
                plugin_ledger::HashingStandard::from_name(standard.as_str()).unwrap_or_default();
        }
        if let Some(sign) = wal.sign_leaves {
            config.sign_leaves = sign;
        }
        if let Some(fsync) = wal.fsync_on_reap {
            config.fsync_on_reap = fsync;
        }
        if let Some(record) = wal.record_pre_redaction_hash {
            config.record_pre_redaction_hash = record;
        }
        if let Some(include) = wal.include_hook_spans_in_root {
            config.include_hook_spans_in_root = include;
        }
        config.witness_cosigning_endpoint = wal.witness_cosigning_endpoint.clone();
        config.kms_provider = wal.kms_provider.clone();
        if let Some(min) = wal.min_signing_key_bytes {
            config.min_signing_key_bytes = min;
        }
        if let Some(verify) = wal.verify_key_fingerprint {
            config.verify_key_fingerprint = verify;
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
            if let Some(detect) = interposition.detect_cross_server_exfil {
                config.detect_cross_server_exfil = detect;
            }
            if let Some(detect) = interposition.detect_confusable_names {
                config.detect_confusable_names = detect;
            }
        }
        if let Some(taint) = tool_guard.taint_governance.as_ref()
            && let Some(propagate) = taint.propagate_byte_range_taints
        {
            config.propagate_byte_range_taints = propagate;
        }
        if let Some(sanitization) = tool_guard.sanitization.as_ref()
            && let Some(scan) = sanitization.scan_tool_results
        {
            config.scan_tool_results = scan;
        }
        if let Some(auth) = tool_guard.approval_auth.as_ref() {
            if let Some(deny) = auth.deny_self_approval {
                config.approval_auth.deny_self_approval = deny;
            }
            if let Some(methods) = auth.methods.clone() {
                config.approval_auth.methods = methods;
            }
            if let Some(max) = auth.max_pending_holds {
                config.approval_auth.max_pending_holds = max;
            }
            if let Some(ttl) = auth.hold_ttl_seconds {
                config.approval_auth.hold_ttl_seconds = ttl;
            }
        }
    }
    config
}

// see: cans/plugins/mcp.md#Plugin-mcp
pub fn to_mcp_config(plugins: &PluginsProfile) -> McpConfig {
    let mut config = McpConfig::default();
    if let Some(mcp) = plugins.mcp.as_ref() {
        if let Some(bytes) = mcp.max_frame_bytes {
            config.max_frame_bytes = bytes;
        }
        if let Some(deny) = mcp.deny_server_sampling {
            config.deny_server_sampling = deny;
        }
        if let Some(allowlist) = mcp.sampling_allowlist.clone() {
            config.sampling_allowlist = allowlist;
        }
    }
    config
}
