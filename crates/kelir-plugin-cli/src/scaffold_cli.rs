// see: cans/15-operations.md#Subcommands
// see: cans/02-plugin-model.md#Plugin-ownership
// TODO (L3) [plugin_engine]: derive the trait stubs from the KelirPlugin trait AST instead of the static skeleton below; slop: stub drifts from the trait, new trait methods never appear in scaffolds
use std::path::PathBuf;

use crate::cli_error::CliError;

// see: cans/02-plugin-model.md#Plugin-ownership
fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

// see: cans/02-plugin-model.md#Plugin-ownership
fn cargo_toml(name: &str) -> String {
    format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\nlicense = \"MIT OR Apache-2.0\"\npublish = false\n\n[dependencies]\nkelir-interface = \"1\"\n\n[dev-dependencies]\nkelir = \"1\"\nkelir-plugin-testkit = \"1\"\nserde_json = \"1\"\n"
    )
}

// see: cans/02-plugin-model.md#Plugin-ownership
fn lib_rs(name: &str, struct_name: &str) -> String {
    format!(
        "// see: cans/02-plugin-model.md#Plugin-ownership\n#![forbid(unsafe_code)]\n\nuse kelir_interface::plugin_model::{{KelirPlugin, PluginDeps}};\n\n// see: cans/02-plugin-model.md#KelirPlugin-trait\n#[derive(Clone, Default, PluginDeps)]\npub struct {struct_name};\n\n// see: cans/02-plugin-model.md#KelirPlugin-trait\nimpl KelirPlugin for {struct_name} {{\n    type Context = ();\n    fn context(&self) -> Self::Context {{}}\n    fn name(&self) -> &str {{\n        \"{name}\"\n    }}\n}}\n"
    )
}

// see: cans/02-plugin-model.md#Plugin-testkit
fn contract_test_rs(name: &str, struct_name: &str, crate_name: &str) -> String {
    format!(
        "// see: cans/02-plugin-model.md#Plugin-testkit\nuse {crate_name}::{struct_name};\nuse kelir_interface::plugin_model::KelirPlugin;\n\n// see: cans/02-plugin-model.md#Plugin-ownership\n#[test]\nfn plugin_name_matches_manifest() {{\n    assert_eq!({struct_name}.name(), \"{name}\");\n}}\n\n// see: cans/02-plugin-model.md#Plugin-testkit\n#[test]\nfn scaffolded_vectors_parse_empty() -> Result<(), String> {{\n    let path = format!(\"{{}}/vectors/vectors.json\", env!(\"CARGO_MANIFEST_DIR\"));\n    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;\n    let value: serde_json::Value =\n        serde_json::from_str(&text).map_err(|e| e.to_string())?;\n    assert_eq!(value[\"vectors\"].as_array().map(Vec::len), Some(0));\n    Ok(())\n}}\n"
    )
}

// see: cans/02-plugin-model.md#Plugin-ownership
fn example_rs(name: &str, struct_name: &str, crate_name: &str) -> String {
    format!(
        "// see: cans/02-plugin-model.md#Unified-builder\n// see: cans/artifacts/examples/fluent-builder-chaos.rs#build_chaos_host\nuse {crate_name}::{struct_name};\nuse kelir::{{driver_interface::default_vm_config, KelirBuilder}};\n\n// see: cans/02-plugin-model.md#Unified-builder\nfn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {{\n    let _sealed = KelirBuilder::new(default_vm_config(None, None, None, None, None, None))\n        .plugin_instance({struct_name})\n        .seal()?;\n    let _ = \"{name}\";\n    Ok(())\n}}\n"
    )
}

// see: cans/15-operations.md#Subcommands
fn write_file(path: &PathBuf, contents: &str) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)?;
    Ok(())
}

// see: cans/15-operations.md#Subcommands
fn struct_name(crate_name: &str) -> String {
    let mut out = String::with_capacity(crate_name.len());
    let mut upper = true;
    for c in crate_name.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

// see: cans/15-operations.md#Subcommands
pub fn run(args: &[String]) -> Result<(), CliError> {
    if args.len() != 1 {
        return Err(CliError::usage(
            "new: expected exactly one plugin name: kelir-plugin-cli new <plugin-name>",
        ));
    }
    let name = args[0].as_str();
    if !valid_name(name) {
        return Err(CliError::usage(format!(
            "new: invalid plugin name `{name}` (expected lowercase kebab-case, e.g. my-plugin)"
        )));
    }
    let root = std::env::current_dir()?.join(name);
    if root.exists() {
        return Err(CliError::usage(format!(
            "new: `{name}` already exists; scaffold into an empty directory"
        )));
    }
    let crate_name = name.replace('-', "_");
    let struct_prefix = struct_name(&crate_name);
    let struct_type = if struct_prefix.ends_with("Plugin") {
        struct_prefix
    } else {
        format!("{struct_prefix}Plugin")
    };
    write_file(&root.join("Cargo.toml"), &cargo_toml(name))?;
    write_file(&root.join("src/lib.rs"), &lib_rs(name, &struct_type))?;
    write_file(
        &root.join("tests").join(format!("{crate_name}_test.rs")),
        &contract_test_rs(name, &struct_type, &crate_name),
    )?;
    write_file(&root.join("tests/fixtures/.gitkeep"), "")?;
    write_file(&root.join("vectors/vectors.json"), "{\"vectors\": []}")?;
    write_file(
        &root.join("examples").join(format!("{name}-basic.rs")),
        &example_rs(name, &struct_type, &crate_name),
    )?;
    println!("scaffold: wrote {name}/ (Rust crate on kelir-interface only)");
    println!(
        "scaffold: next: kelir-plugin-cli manifest (generates .kelir/manifest.json, never hand-written)"
    );
    println!(
        "scaffold: next: kelir-plugin-cli vectors (regenerates vectors/vectors.json from Rust fixtures)"
    );
    println!(
        "scaffold: next: codegen generates the typed TS/Python wrappers (.node/.whl); no hand-written bindings/ts or bindings/py"
    );
    Ok(())
}
