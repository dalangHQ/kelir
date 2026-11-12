// see: cans/15-operations.md#Per-plugin-CI
// see: cans/02-plugin-model.md#Plugin-tiering
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use kelir_core::plugin_registry::{PluginManifest, PluginTier, parse_plugin_manifest};

use crate::cli_error::CliError;
use crate::graph_cli;
use crate::layers_cli;
use crate::rings_cli;

// see: cans/02-plugin-model.md#Plugin-testkit
const TESTKIT_CRATE: &str = "kelir-plugin-testkit";
const INTERFACE_CRATE: &str = "kelir-interface";

// see: cans/plugins/ledger.md#OpenTelemetry
const ALLOWED_SUPPORT_CRATES: &[&str] = &["kelir-otlp"];

// see: cans/15-operations.md#Integration-tier
const ALLOWED_DEV_CRATES: &[&str] = &["driver-rustvmm"];

// see: cans/02-plugin-model.md#Plugin-tiering
fn plugin_dir() -> Result<PathBuf, CliError> {
    Ok(crate::workspace_root()?.join("crates/kelir-plugins"))
}

// see: cans/02-plugin-model.md#Plugin-ownership
fn workspace_manifest() -> Result<PathBuf, CliError> {
    Ok(crate::workspace_root()?.join("Cargo.toml"))
}

#[derive(serde::Deserialize)]
struct WorkspaceRootToml {
    workspace: WorkspaceToml,
}

#[derive(serde::Deserialize)]
struct WorkspaceToml {
    members: Vec<String>,
}

#[derive(serde::Deserialize)]
struct MemberToml {
    package: MemberPackageToml,
}

#[derive(serde::Deserialize)]
struct MemberPackageToml {
    name: String,
}

// see: cans/02-plugin-model.md#Plugin-tiering
fn tier_roster() -> Result<Vec<PluginManifest>, CliError> {
    let mut roster: Vec<PluginManifest> = Vec::new();
    for entry in fs::read_dir(plugin_dir()?)? {
        let dir = entry?.path();
        if !dir.is_dir() {
            continue;
        }
        let manifest = dir.join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        let text = fs::read_to_string(&manifest)?;
        let parsed = parse_plugin_manifest(&text)
            .map_err(|e| format!("check -p: {} rejected: {e}", manifest.display()))?;
        roster.push(parsed);
    }
    if roster.is_empty() {
        return Err(format!(
            "check -p: no plugin manifests found under {}",
            plugin_dir()?.display()
        )
        .into());
    }
    roster.sort_by(|a, b| a.tier.crate_name.cmp(&b.tier.crate_name));
    Ok(roster)
}

// see: cans/02-plugin-model.md#Plugin-ownership
fn forbidden_workspace_crates(plugins: &BTreeSet<&str>) -> Result<BTreeSet<String>, CliError> {
    let manifest_path = workspace_manifest()?;
    let root: WorkspaceRootToml =
        toml::from_str(&fs::read_to_string(&manifest_path)?).map_err(|e| {
            format!(
                "check -p: {} is not valid TOML: {e}",
                manifest_path.display()
            )
        })?;
    let mut forbidden: BTreeSet<String> = BTreeSet::new();
    for member in &root.workspace.members {
        let manifest = crate::workspace_root()?.join(member).join("Cargo.toml");
        let text = fs::read_to_string(&manifest).map_err(|e| {
            format!("check -p: workspace member {member} has no readable Cargo.toml: {e}")
        })?;
        let parsed: MemberToml = toml::from_str(&text)
            .map_err(|e| format!("check -p: {} is not valid TOML: {e}", manifest.display()))?;
        let name = parsed.package.name;
        if plugins.contains(name.as_str())
            || name == INTERFACE_CRATE
            || name == TESTKIT_CRATE
            || ALLOWED_SUPPORT_CRATES.contains(&name.as_str())
        {
            continue;
        }
        forbidden.insert(name);
    }
    Ok(forbidden)
}

fn tier_of<'a>(roster: &'a [PluginManifest], crate_name: &str) -> Option<&'a PluginTier> {
    roster
        .iter()
        .find(|plugin| plugin.tier.crate_name == crate_name)
        .map(|plugin| &plugin.tier)
}

fn check_regular_dep(
    plugin: &str,
    tier: u8,
    dep: &str,
    roster: &[PluginManifest],
    forbidden: &BTreeSet<String>,
) -> Result<(), CliError> {
    if dep == INTERFACE_CRATE {
        return Ok(());
    }
    if dep == TESTKIT_CRATE {
        return Err(format!(
            "check -p: plugin `{plugin}` lists {TESTKIT_CRATE} as a regular dependency; the testkit is dev-only"
        )
        .into());
    }
    if forbidden.contains(dep) {
        return Err(
            format!("check -p: plugin `{plugin}` depends on forbidden crate `{dep}`").into(),
        );
    }
    match tier_of(roster, dep) {
        Some(dep_tier) if dep_tier.level >= tier => Err(format!(
            "check -p: tier violation: `{plugin}` (tier {tier}) depends on `{dep}` (tier {}); dependencies must be downward-only",
            dep_tier.level
        )
        .into()),
        Some(_) | None => Ok(()),
    }
}

fn check_dev_dep(
    plugin: &str,
    tier: u8,
    dep: &str,
    roster: &[PluginManifest],
    forbidden: &BTreeSet<String>,
) -> Result<(), CliError> {
    if dep == TESTKIT_CRATE {
        return Ok(());
    }
    if ALLOWED_DEV_CRATES.contains(&dep) {
        return Ok(());
    }
    if forbidden.contains(dep) {
        return Err(
            format!("check -p: plugin `{plugin}` dev-depends on forbidden crate `{dep}`").into(),
        );
    }
    match tier_of(roster, dep) {
        Some(dep_tier) if dep_tier.level >= tier => Err(format!(
            "check -p: tier violation: `{plugin}` (tier {tier}) dev-depends on `{dep}` (tier {}); dependencies must be downward-only",
            dep_tier.level
        )
        .into()),
        Some(_) | None => Ok(()),
    }
}

pub fn run(args: &[String]) -> Result<(), CliError> {
    let Some(name) = args.first() else {
        return Err(CliError::usage("check -p requires a plugin name"));
    };
    run_plugin(name)
}

pub fn run_plugin(name: &str) -> Result<(), CliError> {
    // TODO (L3) [plugin_engine]: replace manifest and source scanning with the AST extractor required by cans/02-plugin-model.md#Auto-generated-manifest-contract.; slop: regex scan passes, AST extractor never built
    let roster = tier_roster()?;
    let Some(plugin) = roster.iter().find(|plugin| plugin.tier.crate_name == name) else {
        return Err(format!(
            "check -p: `{name}` is not in the plugin tier roster (cans/02-plugin-model.md#Plugin-tiering)"
        )
        .into());
    };
    let tier = plugin.tier.level;
    let code_name = plugin.tier.name.as_str();
    let plugin_names: BTreeSet<&str> = roster
        .iter()
        .map(|plugin| plugin.tier.crate_name.as_str())
        .collect();
    let forbidden = forbidden_workspace_crates(&plugin_names)?;
    if !plugin.deps.iter().any(|dep| dep == INTERFACE_CRATE) {
        return Err(
            format!("check -p: plugin `{name}` does not depend on {INTERFACE_CRATE}").into(),
        );
    }
    for dep in &plugin.deps {
        check_regular_dep(name, tier, dep, &roster, &forbidden)?;
    }
    if !plugin.dev_deps.iter().any(|dep| dep == TESTKIT_CRATE) {
        return Err(
            format!("check -p: plugin `{name}` does not dev-depend on {TESTKIT_CRATE}").into(),
        );
    }
    for dep in &plugin.dev_deps {
        check_dev_dep(name, tier, dep, &roster, &forbidden)?;
    }
    let graph = graph_cli::plugin_graph()?;
    graph_cli::assert_acyclic(&graph)?;
    let src = crate::workspace_root()?
        .join("crates/kelir-plugins")
        .join(name)
        .join("src");
    layers_cli::check_dir(&src, &["kelir_core::", "kelir_drivers::", "kelir::"])?;
    let mask = rings_cli::declared_ring_mask(&src)?;
    if !mask.contains("KelirPlugin") {
        return Err(format!(
            "check -p: plugin `{name}` implements no KelirPlugin trait in {}",
            src.display()
        )
        .into());
    }
    let declared_name = rings_cli::declared_plugin_name(&src)?;
    if declared_name.as_deref() != Some(code_name) {
        return Err(format!(
            "check -p: plugin `{name}` declares name() = `{}` but the tier roster expects `{code_name}`",
            declared_name.as_deref().unwrap_or("<none>")
        )
        .into());
    }
    let ring_mask: Vec<&str> = mask.iter().map(String::as_str).collect();
    println!(
        "check -p {name}: tier {tier}, {} regular dep(s), ring mask [{}]",
        plugin.deps.len(),
        ring_mask.join(", ")
    );
    Ok(())
}
