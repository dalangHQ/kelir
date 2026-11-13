// see: cans/artifacts/test-plan/cases/plugin_engine/pe_01_downward_only_tier_dependency_lint.yaml#case_id
use std::path::Path;
use std::process::Command;

fn write_text(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("write {}: {e}", path.display()))
}

fn make_dir(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|e| format!("mkdir {}: {e}", path.display()))
}

fn plugin_manifest(name: &str, tier_level: u8, tier_name: &str) -> String {
    format!(
        "[package]\nname = \"{name}\"\nversion = \"1.0.0\"\nedition = \"2024\"\n\n[dependencies]\nkelir-interface = {{ path = \"../../kelir-interface\" }}\n\n[package.metadata.kelir.tier]\nlevel = {tier_level}\nname = \"{tier_name}\"\n"
    )
}

// see: cans/artifacts/test-plan/cases/plugin_engine/pe_01_downward_only_tier_dependency_lint.yaml#case_id
#[test]
#[ignore]
fn pe_01_downward_only_tier_dependency_lint() -> Result<(), String> {
    let root = std::env::temp_dir().join(format!("kelir-pe01-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let plugins = root.join("crates/kelir-plugins");
    let mcp_src = plugins.join("plugin-mcp/src");
    make_dir(&mcp_src)?;
    let ledger_src = plugins.join("plugin-ledger/src");
    make_dir(&ledger_src)?;
    make_dir(&root.join("crates/kelir-engine/src"))?;
    make_dir(&root.join("crates/kelir-drivers"))?;
    write_text(
        &plugins.join("plugin-mcp/Cargo.toml"),
        &plugin_manifest("plugin-mcp", 1, "mcp"),
    )?;
    write_text(
        &plugins.join("plugin-ledger/Cargo.toml"),
        &plugin_manifest("plugin-ledger", 3, "ledger"),
    )?;
    write_text(
        &mcp_src.join("lib.rs"),
        "use plugin_ledger::audit;\n\npub fn bridge() -> u8 {\n    audit()\n}\n",
    )?;
    write_text(
        &ledger_src.join("lib.rs"),
        "pub fn audit() -> u8 {\n    3\n}\n",
    )?;
    let output = Command::new(env!("CARGO_BIN_EXE_kelir-plugin-cli"))
        .args(["check", "--layers"])
        .current_dir(&root)
        .output()
        .map_err(|e| format!("failed to spawn kelir-plugin-cli: {e}"))?;
    let _ = std::fs::remove_dir_all(&root);
    if output.status.success() {
        return Err(
            "check --layers accepted Tier 1 plugin-mcp importing Tier 3 plugin-ledger; expected a layer violation"
                .into(),
        );
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("plugin-ledger") {
        return Err(format!(
            "layer violation must name the upward dependency plugin-ledger, got stderr: {stderr}"
        ));
    }
    Ok(())
}
