// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/03-sdk-architecture.md#Python-SDK
use std::fs;
use std::path::{Path, PathBuf};

use crate::sdk_codegen::{Projection, sdk_camel};

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
pub struct BuilderPlugin {
    pub name: String,
    pub module_name: String,
    pub config_name: String,
    pub level: u32,
    pub requires: Vec<String>,
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
fn tier_value(text: &str, key: &str) -> Option<String> {
    let mut in_tier = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_tier = line == "[package.metadata.kelir.tier]";
            continue;
        }
        if !in_tier {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if name.trim() == key {
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
fn parse_tier(dir: &Path, name: &str) -> Result<u32, String> {
    let manifest = dir.join("Cargo.toml");
    let text = fs::read_to_string(&manifest)
        .map_err(|e| format!("cannot read {}: {e}", manifest.display()))?;
    let tier_name = tier_value(&text, "name").ok_or_else(|| {
        format!(
            "{}: [package.metadata.kelir.tier] declares no name",
            dir.display()
        )
    })?;
    if tier_name != name {
        return Err(format!(
            "{}: tier name {tier_name} does not match plugin dir plugin-{name}",
            dir.display()
        ));
    }
    let level = tier_value(&text, "level").ok_or_else(|| {
        format!(
            "{}: [package.metadata.kelir.tier] declares no level",
            dir.display()
        )
    })?;
    let level: u32 = level
        .parse()
        .map_err(|_| format!("{}: tier level {level} is not a u32", dir.display()))?;
    if !(1..=3).contains(&level) {
        return Err(format!(
            "{}: tier level {level} is outside 1..=3",
            dir.display()
        ));
    }
    Ok(level)
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
fn kebab_plugin_type(name: &str) -> Option<String> {
    let stem = name.strip_suffix("Plugin")?;
    if stem.is_empty() {
        return None;
    }
    let mut out = String::new();
    for (index, ch) in stem.chars().enumerate() {
        if ch.is_ascii_uppercase() && index > 0 {
            out.push('-');
        }
        out.push(ch.to_ascii_lowercase());
    }
    Some(out)
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
fn scan_requires(src: &Path) -> Result<Vec<String>, String> {
    let mut requires: Vec<String> = Vec::new();
    let mut entries: Vec<PathBuf> = Vec::new();
    let dir = fs::read_dir(src).map_err(|e| format!("cannot list {}: {e}", src.display()))?;
    for entry in dir {
        let entry = entry.map_err(|e| format!("cannot list {}: {e}", src.display()))?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "rs") {
            entries.push(path);
        }
    }
    entries.sort();
    for path in &entries {
        let text =
            fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let mut in_deps = false;
        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(marker) = trimmed.strip_prefix("// require:") {
                let peer = marker.trim().trim_matches('"').to_string();
                if !peer.is_empty() && !requires.contains(&peer) {
                    requires.push(peer);
                }
                continue;
            }
            if trimmed.starts_with("impl") && trimmed.contains("PluginDeps") {
                in_deps = true;
                continue;
            }
            if in_deps {
                if let Some(rhs) = trimmed.strip_prefix("type Deps =") {
                    let rhs = rhs.trim().trim_end_matches(';');
                    let peers: Vec<String> = rhs
                        .split(|ch: char| !ch.is_ascii_alphanumeric())
                        .filter_map(kebab_plugin_type)
                        .collect();
                    for peer in peers {
                        if !requires.contains(&peer) {
                            requires.push(peer);
                        }
                    }
                }
                if trimmed == "}" {
                    in_deps = false;
                }
            }
        }
    }
    requires.sort();
    Ok(requires)
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
pub fn load_builder_plugins(
    root: &Path,
    projections: &[Projection],
) -> Result<Vec<BuilderPlugin>, String> {
    let plugins_src = root.join("crates/kelir-plugins");
    let mut plugins: Vec<BuilderPlugin> = Vec::new();
    for proj in projections {
        let dir = plugins_src.join(&proj.krate.dir_name);
        let level = parse_tier(&dir, &proj.krate.plugin_name)?;
        let requires = scan_requires(&dir.join("src"))?;
        plugins.push(BuilderPlugin {
            name: proj.krate.plugin_name.clone(),
            module_name: proj.krate.module_name.clone(),
            config_name: proj.root.rust_name.clone(),
            level,
            requires,
        });
    }
    plugins.sort_by(|a, b| (a.level, &a.name).cmp(&(b.level, &b.name)));
    Ok(plugins)
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
pub fn emit_typescript(plugins: &[BuilderPlugin]) -> String {
    let mut lines: Vec<String> = vec![
        String::new(),
        String::new(),
        "// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder ".to_string(),
        "import { createPlugin } from \"./plugin.model.js\";".to_string(),
    ];
    for plugin in plugins {
        let factory = format!("{}Builder", sdk_camel(&plugin.name));
        lines.push(String::new());
        lines.push(String::new());
        lines.push(format!(
            "export function {factory}(config: {}): KelirPlugin {{",
            plugin.config_name
        ));
        let mut chain = format!("createPlugin(\"{}\", {{ config }})", plugin.name);
        for peer in &plugin.requires {
            chain.push_str(&format!(".requires<\"{peer}\">()"));
        }
        chain.push_str(".build()");
        lines.push(format!("  return {chain};"));
        lines.push("}".to_string());
    }
    lines.push(String::new());
    lines.join("\n")
}

// see: cans/03-sdk-architecture.md#Python-SDK
pub fn emit_python_factory(plugin: &BuilderPlugin) -> String {
    let mut lines: Vec<String> = vec![
        String::new(),
        String::new(),
        "# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder ".to_string(),
        "from kelir.plugin_builder import PluginBuilder, _PluginWrapper".to_string(),
        String::new(),
        String::new(),
        format!(
            "def {}_builder(config: {} | None = None) -> _PluginWrapper:",
            plugin.module_name, plugin.config_name
        ),
    ];
    lines.push(format!(
        "    cfg = config if config is not None else {}()",
        plugin.config_name
    ));
    let mut chain = format!("PluginBuilder(\"{}\")", plugin.name);
    for peer in &plugin.requires {
        chain.push_str(&format!(".require(\"{peer}\")"));
    }
    chain.push_str(".context(lambda: cfg).build()");
    lines.push(format!("    return {chain}"));
    lines.push(String::new());
    lines.join("\n")
}
