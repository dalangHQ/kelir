// see: cans/15-operations.md#Per-plugin-CI
// see: cans/02-plugin-model.md#Plugin-tiering
use std::fs;
use std::path::Path;

use crate::graph;
use crate::layers;
use crate::rings;

// see: cans/02-plugin-model.md#Plugin-tiering
const PLUGIN_TIERS: &[(&str, u8, &str)] = &[
    ("plugin-mcp", 1, "mcp"),
    ("plugin-mesh", 1, "mesh"),
    ("plugin-tool-guard", 2, "tool-guard"),
    ("plugin-wire-ingress", 2, "wire-ingress"),
    ("plugin-vault", 2, "vault"),
    ("plugin-container", 2, "container"),
    ("plugin-replay", 2, "replay"),
    ("plugin-db-proxy", 2, "db-proxy"),
    ("plugin-supply-chain", 2, "supply-chain"),
    ("plugin-ledger", 3, "ledger"),
    ("plugin-profile", 3, "profile"),
    ("plugin-chaos", 3, "chaos"),
    ("plugin-transaction", 3, "transaction"),
];

// see: cans/02-plugin-model.md#Plugin-ownership
const FORBIDDEN_DEPS: &[&str] = &[
    "kelir",
    "kelir-core",
    "kelir-drivers",
    "kelir-transport",
    "kelir-shim",
    "kelir-catalog",
    "kelir-otlp",
    "kelir-integration",
    "kelir-plugin-cli",
    "kelir-napi",
    "kelir-pyo3",
    "kelir-pack",
    "mcp-server",
    "driver-rustvmm",
    "driver-hvf",
    "driver-openvmm",
    "driver-avf",
    "driver-bwrap",
    "driver-proot",
];

// see: cans/02-plugin-model.md#Plugin-testkit
const TESTKIT_CRATE: &str = "kelir-plugin-testkit";
const INTERFACE_CRATE: &str = "kelir-interface";

struct ManifestInfo {
    package_name: Option<String>,
    deps: Vec<String>,
    dev_deps: Vec<String>,
}

fn strip_quotes(value: &str) -> String {
    value.trim().trim_matches('"').to_string()
}

fn parse_manifest(text: &str) -> ManifestInfo {
    let mut info = ManifestInfo {
        package_name: None,
        deps: Vec::new(),
        dev_deps: Vec::new(),
    };
    let mut section = "";
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            section = &line[1..line.len() - 1];
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, _)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if section == "package" && key == "name" {
            if let Some((_, value)) = line.split_once('=') {
                info.package_name = Some(strip_quotes(value));
            }
        } else if section == "dependencies" {
            info.deps.push(key.to_string());
        } else if let Some(dep) = section.strip_prefix("dependencies.") {
            info.deps.push(dep.to_string());
        } else if section == "dev-dependencies" {
            info.dev_deps.push(key.to_string());
        } else if let Some(dep) = section.strip_prefix("dev-dependencies.") {
            info.dev_deps.push(dep.to_string());
        }
    }
    info
}

fn tier_of(crate_name: &str) -> Option<u8> {
    PLUGIN_TIERS
        .iter()
        .find(|(name, _, _)| *name == crate_name)
        .map(|(_, tier, _)| *tier)
}

fn check_regular_dep(plugin: &str, tier: u8, dep: &str) -> Result<(), Box<dyn std::error::Error>> {
    if dep == INTERFACE_CRATE {
        return Ok(());
    }
    if dep == TESTKIT_CRATE {
        return Err(format!(
            "check -p: plugin `{plugin}` lists {TESTKIT_CRATE} as a regular dependency; the testkit is dev-only"
        )
        .into());
    }
    if FORBIDDEN_DEPS.contains(&dep) {
        return Err(
            format!("check -p: plugin `{plugin}` depends on forbidden crate `{dep}`").into(),
        );
    }
    match tier_of(dep) {
        Some(dep_tier) if dep_tier >= tier => Err(format!(
            "check -p: tier violation: `{plugin}` (tier {tier}) depends on `{dep}` (tier {dep_tier}); dependencies must be downward-only"
        )
        .into()),
        Some(_) | None => Ok(()),
    }
}

fn check_dev_dep(plugin: &str, tier: u8, dep: &str) -> Result<(), Box<dyn std::error::Error>> {
    if dep == TESTKIT_CRATE {
        return Ok(());
    }
    if FORBIDDEN_DEPS.contains(&dep) {
        return Err(
            format!("check -p: plugin `{plugin}` dev-depends on forbidden crate `{dep}`").into(),
        );
    }
    match tier_of(dep) {
        Some(dep_tier) if dep_tier >= tier => Err(format!(
            "check -p: tier violation: `{plugin}` (tier {tier}) dev-depends on `{dep}` (tier {dep_tier}); dependencies must be downward-only"
        )
        .into()),
        Some(_) | None => Ok(()),
    }
}

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let Some(name) = args.first() else {
        return Err("check -p requires a plugin name".into());
    };
    run_plugin(name)
}

pub fn run_plugin(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    // TODO (L3): replace manifest and source scanning with the AST extractor required by cans/02-plugin-model.md#Auto-generated-manifest-contract.
    let manifest = format!("crates/kelir-plugins/{name}/Cargo.toml");
    let Some((_, tier, code_name)) = PLUGIN_TIERS
        .iter()
        .find(|(crate_name, _, _)| *crate_name == name)
    else {
        return Err(format!(
            "check -p: `{name}` is not in the plugin tier roster (cans/02-plugin-model.md#Plugin-tiering)"
        )
        .into());
    };
    let text = fs::read_to_string(&manifest)
        .map_err(|_| format!("check -p: no such plugin `{name}` (missing {manifest})"))?;
    let info = parse_manifest(&text);
    if info.package_name.as_deref() != Some(name) {
        return Err(format!(
            "check -p: package name mismatch for `{name}`: manifest says `{}`",
            info.package_name.as_deref().unwrap_or("<none>")
        )
        .into());
    }
    if !info.deps.iter().any(|dep| dep == INTERFACE_CRATE) {
        return Err(
            format!("check -p: plugin `{name}` does not depend on {INTERFACE_CRATE}").into(),
        );
    }
    for dep in &info.deps {
        check_regular_dep(name, *tier, dep)?;
    }
    if !info.dev_deps.iter().any(|dep| dep == TESTKIT_CRATE) {
        return Err(
            format!("check -p: plugin `{name}` does not dev-depend on {TESTKIT_CRATE}").into(),
        );
    }
    for dep in &info.dev_deps {
        check_dev_dep(name, *tier, dep)?;
    }
    let graph = graph::plugin_graph()?;
    graph::assert_acyclic(&graph)?;
    let src = Path::new("crates/kelir-plugins").join(name).join("src");
    layers::check_dir(&src, &["kelir_core::", "kelir_drivers::", "kelir::"])?;
    let mask = rings::declared_ring_mask(&src)?;
    if !mask.contains("KelirPlugin") {
        return Err(format!(
            "check -p: plugin `{name}` implements no KelirPlugin trait in {}",
            src.display()
        )
        .into());
    }
    let declared_name = rings::declared_plugin_name(&src)?;
    if declared_name.as_deref() != Some(*code_name) {
        return Err(format!(
            "check -p: plugin `{name}` declares name() = `{}` but the tier roster expects `{code_name}`",
            declared_name.as_deref().unwrap_or("<none>")
        )
        .into());
    }
    let ring_mask: Vec<&str> = mask.iter().map(String::as_str).collect();
    println!(
        "check -p {name}: tier {tier}, {} regular dep(s), ring mask [{}]",
        info.deps.len(),
        ring_mask.join(", ")
    );
    Ok(())
}
