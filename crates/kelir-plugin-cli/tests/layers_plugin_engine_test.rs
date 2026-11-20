// see: cans/artifacts/test-plan/cases/plugin_engine/pe_01_downward_only_tier_dependency_lint.yaml#case_id
// see: cans/artifacts/test-plan/cases/plugin_engine/pe_22_facade_zero_plugin_knowledge.yaml#case_id
use std::path::{Path, PathBuf};
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
fn mock_tree(tag: &str) -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!("kelir-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let plugins = root.join("crates/kelir-plugins");
    make_dir(&plugins.join("plugin-mcp/src"))?;
    make_dir(&plugins.join("plugin-ledger/src"))?;
    make_dir(&root.join("crates/kelir-engine/src"))?;
    make_dir(&root.join("crates/kelir-drivers"))?;
    make_dir(&root.join("crates/kelir/src"))?;
    write_text(
        &plugins.join("plugin-mcp/Cargo.toml"),
        &plugin_manifest("plugin-mcp", 1, "mcp"),
    )?;
    write_text(
        &plugins.join("plugin-ledger/Cargo.toml"),
        &plugin_manifest("plugin-ledger", 3, "ledger"),
    )?;
    write_text(
        &root.join("crates/kelir/Cargo.toml"),
        "[package]\nname = \"kelir\"\nversion = \"1.0.0\"\nedition = \"2024\"\n\n[dependencies]\nkelir-interface = { path = \"../kelir-interface\" }\n",
    )?;
    write_text(
        &root.join("crates/kelir/src/lib.rs"),
        "// see: cans/01-architecture.md#Host-runtime\npub use kelir_interface as interface;\n",
    )?;
    Ok(root)
}

fn run_layers(root: &Path) -> Result<std::process::Output, String> {
    Command::new(env!("CARGO_BIN_EXE_kelir-plugin-cli"))
        .args(["check", "--layers"])
        .env("KELIR_PLUGIN_CLI_ROOT", root)
        .output()
        .map_err(|e| format!("failed to spawn kelir-plugin-cli: {e}"))
}

// see: cans/artifacts/test-plan/cases/plugin_engine/pe_01_downward_only_tier_dependency_lint.yaml#case_id
#[test]
fn pe_01_downward_only_tier_dependency_lint() -> Result<(), String> {
    let root = mock_tree("pe01")?;
    let mcp_src = root.join("crates/kelir-plugins/plugin-mcp/src");
    write_text(
        &mcp_src.join("lib.rs"),
        "use plugin_ledger::audit;\n\npub fn bridge() -> u8 {\n    audit()\n}\n",
    )?;
    write_text(
        &root.join("crates/kelir-plugins/plugin-ledger/src/lib.rs"),
        "pub fn audit() -> u8 {\n    3\n}\n",
    )?;
    let output = run_layers(&root)?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let _ = std::fs::remove_dir_all(&root);
    if output.status.success() {
        return Err(
            "check --layers accepted Tier 1 plugin-mcp importing Tier 3 plugin-ledger; expected a layer violation"
                .into(),
        );
    }
    if !stderr.contains("plugin-ledger") {
        return Err(format!(
            "layer violation must name the upward dependency plugin-ledger, got stderr: {stderr}"
        ));
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/plugin_engine/pe_22_facade_zero_plugin_knowledge.yaml#case_id
#[test]
fn pe_22_facade_zero_plugin_knowledge() -> Result<(), String> {
    let root = mock_tree("pe22")?;
    let facade_src = root.join("crates/kelir/src");
    write_text(
        &root.join("crates/kelir/Cargo.toml"),
        "[package]\nname = \"kelir\"\nversion = \"1.0.0\"\nedition = \"2024\"\n\n[dependencies]\nkelir-interface = { path = \"../kelir-interface\" }\nplugin-mcp = { path = \"../kelir-plugins/plugin-mcp\" }\n",
    )?;
    let manifest_output = run_layers(&root)?;
    let manifest_stderr = String::from_utf8_lossy(&manifest_output.stderr);
    if manifest_output.status.success() {
        return Err(
            "check --layers accepted a plugin-mcp dependency in the facade manifest; expected a layer violation"
                .into(),
        );
    }
    if !manifest_stderr.contains("plugin-mcp") {
        return Err(format!(
            "facade manifest violation must name plugin-mcp, got stderr: {manifest_stderr}"
        ));
    }
    write_text(
        &root.join("crates/kelir/Cargo.toml"),
        "[package]\nname = \"kelir\"\nversion = \"1.0.0\"\nedition = \"2024\"\n\n[dependencies]\nkelir-interface = { path = \"../kelir-interface\" }\n",
    )?;
    write_text(
        &facade_src.join("lib.rs"),
        "use plugin_mcp::bridge;\n\npub fn call() -> u8 {\n    bridge()\n}\n",
    )?;
    let token_output = run_layers(&root)?;
    let token_stderr = String::from_utf8_lossy(&token_output.stderr);
    let _ = std::fs::remove_dir_all(&root);
    if token_output.status.success() {
        return Err(
            "check --layers accepted a plugin_mcp token in facade src; expected a layer violation"
                .into(),
        );
    }
    if !token_stderr.contains("plugin_mcp") {
        return Err(format!(
            "facade token violation must name plugin_mcp, got stderr: {token_stderr}"
        ));
    }
    Ok(())
}
