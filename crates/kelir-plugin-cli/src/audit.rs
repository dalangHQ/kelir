// see: cans/15-operations.md#Per-plugin-CI
// see: cans/02-plugin-model.md#Plugin-tiering
use std::fs;

const RING_EVIDENCE: &[&str] = &[
    "impl TrapInterposer for",
    "impl ConsoleInterposer for",
    "impl FsInterposer for",
    "impl NetInterposer for",
    "impl HostInterposer for",
    "impl LifecycleParticipant for",
    "register_interceptors",
    "VirtioDevice::Vsock",
    "StreamKind::DbSqlite",
    "PeerListener",
];

const FORBIDDEN_RUNTIME_DEPS: &[&str] = &[
    "kelir",
    "kelir-core",
    "kelir-plugin-cli",
    "kelir-plugin-testkit",
    "kelir-catalog",
    "kelir-otlp",
    "kelir-transport",
    "kelir-shim",
];

fn tier_of(plugin: &str) -> Option<u8> {
    match plugin {
        "mcp" | "mesh" => Some(1),
        "tool-guard" | "wire-ingress" | "vault" | "container" | "replay" | "db-proxy"
        | "supply-chain" => Some(2),
        "ledger" | "profile" | "chaos" | "transaction" => Some(3),
        _ => None,
    }
}

fn fail(message: String) -> Result<(), Box<dyn std::error::Error>> {
    Err(message.into())
}

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let Some(name) = args.first() else {
        return Err("check -p requires a plugin name".into());
    };
    run_plugin(name)
}

pub fn run_plugin(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let short = match name.strip_prefix("plugin-") {
        Some(short) => short,
        None => {
            return fail(format!(
                "check -p: plugin name `{name}` must start with `plugin-`"
            ));
        }
    };
    let own_tier = match tier_of(short) {
        Some(tier) => tier,
        None => {
            return fail(format!(
                "check -p: unknown community plugin `{name}` (not in the cans/02 tier table)"
            ));
        }
    };
    let manifest = format!("crates/kelir-plugins/{name}/Cargo.toml");
    let text = fs::read_to_string(&manifest)
        .map_err(|_| format!("check -p: no such plugin `{name}` (missing {manifest})"))?;
    let mut found_name = false;
    let mut edition = String::new();
    let mut license = String::new();
    let mut runtime_deps: Vec<String> = Vec::new();
    let mut dev_deps: Vec<String> = Vec::new();
    let mut section = "";
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            section = line;
            continue;
        }
        if section == "[package]" {
            if let Some(value) = line.strip_prefix("name = ") {
                let found = value.trim().trim_matches('"');
                if found != name {
                    return fail(format!(
                        "check -p: package name mismatch for `{name}`: manifest says `{found}`"
                    ));
                }
                found_name = true;
            }
            if let Some(value) = line.strip_prefix("edition = ") {
                edition = value.trim().trim_matches('"').to_string();
            }
            if let Some(value) = line.strip_prefix("license = ") {
                license = value.trim().trim_matches('"').to_string();
            }
        }
        if section == "[dependencies]" || section == "[dev-dependencies]" {
            if let Some(dep) = line.split('=').next().map(str::trim) {
                if !dep.is_empty() && !dep.starts_with('#') {
                    if section == "[dependencies]" {
                        runtime_deps.push(dep.to_string());
                    } else {
                        dev_deps.push(dep.to_string());
                    }
                }
            }
        }
    }
    if !found_name {
        return fail(format!("check -p: no package name in {manifest}"));
    }
    if edition != "2024" {
        return fail(format!(
            "check -p: `{name}` edition is `{edition}`, expected 2024"
        ));
    }
    if license.is_empty() {
        return fail(format!("check -p: `{name}` declares no license"));
    }
    if !runtime_deps.iter().any(|dep| dep == "kelir-interface") {
        return fail(format!(
            "check -p: `{name}` has no kelir-interface runtime dependency"
        ));
    }
    for dep in &runtime_deps {
        if FORBIDDEN_RUNTIME_DEPS.contains(&dep.as_str()) {
            return fail(format!(
                "check -p: `{name}` must not depend on `{dep}` at runtime"
            ));
        }
        if let Some(peer) = dep.strip_prefix("plugin-") {
            let peer_tier = match tier_of(peer) {
                Some(tier) => tier,
                None => {
                    return fail(format!(
                        "check -p: `{name}` depends on unknown plugin crate `{dep}`"
                    ));
                }
            };
            if peer_tier >= own_tier {
                return fail(format!(
                    "check -p: `{name}` (tier {own_tier}) must not depend on `{dep}` (tier {peer_tier}): downward-only dependencies"
                ));
            }
        }
    }
    if dev_deps.iter().any(|dep| dep == "kelir") {
        return fail(format!(
            "check -p: `{name}` must never depend on the kelir facade, even as a dev-dependency"
        ));
    }
    let lib = format!("crates/kelir-plugins/{name}/src/lib.rs");
    let lib_text =
        fs::read_to_string(&lib).map_err(|_| format!("check -p: no such plugin source `{lib}`"))?;
    // TODO (L3): extract the ring mask from the real AST instead of substring evidence; a comment naming a trait would pass this check.
    if !RING_EVIDENCE.iter().any(|mark| lib_text.contains(mark)) {
        return fail(format!(
            "check -p: `{name}` declares no ring attachment (no ring-trait impl, register_interceptors, or vsock service) in src/lib.rs"
        ));
    }
    println!("check -p: {name} ok (tier {own_tier})");
    Ok(())
}
