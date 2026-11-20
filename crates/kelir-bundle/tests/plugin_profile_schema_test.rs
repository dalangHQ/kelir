// see: cans/02-plugin-model.md#Facade-blindness
use kelir::sandbox_runtime::SandboxProfile;
use kelir_bundle::plugin_profile_schema::{self as schema, PluginsProfile};

fn repo_path(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn fixture_plugins() -> Result<PluginsProfile, Box<dyn std::error::Error>> {
    let yaml = std::fs::read_to_string(repo_path(
        "../../cans/artifacts/examples/fixtures/kelir.yaml",
    ))?;
    let root: serde_yaml::Value = serde_yaml::from_str(&yaml)?;
    let plugins = root
        .get("plugins")
        .cloned()
        .ok_or("plugins section missing from fixture")?;
    Ok(serde_yaml::from_value(plugins)?)
}

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
#[test]
fn plugin_sections_round_trip_from_the_ssot_fixture() -> Result<(), Box<dyn std::error::Error>> {
    let plugins = fixture_plugins()?;
    let wal = plugins
        .ledger
        .as_ref()
        .and_then(|l| l.merkle_wal.as_ref())
        .ok_or("plugins.ledger.merkle_wal missing")?;
    assert!(matches!(
        wal.hashing_standard,
        Some(schema::HashingStandard::Rfc6962DualBlake3Sha256)
    ));
    assert_eq!(wal.record_pre_redaction_hash, Some(true));
    assert_eq!(wal.include_hook_spans_in_root, Some(true));
    assert_eq!(
        wal.witness_cosigning_endpoint.as_deref(),
        Some("https://witness.internal.net/v1/sth")
    );
    let otel = plugins
        .ledger
        .as_ref()
        .and_then(|l| l.otel.as_ref())
        .ok_or("plugins.ledger.otel missing")?;
    assert_eq!(
        otel.endpoint.as_deref(),
        Some("http://localhost:4318/v1/traces")
    );
    let guard = plugins
        .tool_guard
        .as_ref()
        .and_then(|t| t.mcp_interposition.as_ref())
        .ok_or("plugins.tool_guard.mcp_interposition missing")?;
    assert_eq!(guard.max_frame_bytes, Some(16777216));
    assert_eq!(guard.detect_semantic_rug_pull, Some(true));
    assert_eq!(guard.strict_ast_parameter_validation, Some(true));
    let taint = plugins
        .tool_guard
        .as_ref()
        .and_then(|t| t.taint_governance.as_ref())
        .ok_or("plugins.tool_guard.taint_governance missing")?;
    assert_eq!(taint.propagate_byte_range_taints, Some(true));
    let mcp = plugins.mcp.as_ref().ok_or("plugins.mcp missing")?;
    assert_eq!(mcp.max_frame_bytes, Some(16777216));
    schema::validate_plugins(&plugins)?;
    Ok(())
}

// see: cans/02-plugin-model.md#Facade-blindness
#[test]
fn facade_keeps_the_plugins_section_opaque() -> Result<(), Box<dyn std::error::Error>> {
    let yaml = std::fs::read_to_string(repo_path(
        "../../cans/artifacts/examples/fixtures/kelir.yaml",
    ))?;
    let profile: SandboxProfile = serde_yaml::from_str(&yaml)?;
    let plugins = profile
        .plugins
        .as_ref()
        .ok_or("facade dropped the opaque plugins section")?;
    assert!(plugins.get("ledger").is_some());
    assert_eq!(
        schema::plugins_section(&profile).map(|p| p.mcp.is_some()),
        Some(true)
    );
    Ok(())
}

// see: cans/plugins/profile.md#Monotonic-limits
#[test]
fn overlay_dropping_a_plugin_guard_fails_with_e_security_violation()
-> Result<(), Box<dyn std::error::Error>> {
    let base = fixture_plugins()?;
    let mut overlay = base.clone();
    overlay
        .ledger
        .as_mut()
        .ok_or("ledger missing")?
        .merkle_wal
        .as_mut()
        .ok_or("merkle_wal missing")?
        .record_pre_redaction_hash = Some(false);
    let err = match schema::validate_plugins_overlay(&base, &overlay) {
        Ok(()) => panic!("dropped pre-redaction guard passed validation"),
        Err(err) => err,
    };
    assert!(
        err.to_string().contains("ESecurityViolation"),
        "dropping a guard must fail with ESecurityViolation, got {err}"
    );
    Ok(())
}

// see: cans/plugins/ledger.md#Plugin-ledger
#[test]
fn plugin_sections_map_to_typed_plugin_configs() -> Result<(), Box<dyn std::error::Error>> {
    let plugins = fixture_plugins()?;
    let ledger = schema::to_ledger_config(&plugins);
    assert!(ledger.merkle_chain);
    assert!(matches!(
        ledger.hashing_standard,
        plugin_ledger::HashingStandard::Rfc6962DualBlake3Sha256
    ));
    assert!(ledger.record_pre_redaction_hash);
    assert!(ledger.include_hook_spans_in_root);
    let guard = schema::to_tool_guard_config(&plugins);
    assert_eq!(guard.max_frame_bytes, 16777216);
    assert!(guard.detect_semantic_rug_pull);
    assert!(guard.strict_ast_parameter_validation);
    let mcp = schema::to_mcp_config(&plugins);
    assert_eq!(mcp.max_frame_bytes, 16777216);
    Ok(())
}
