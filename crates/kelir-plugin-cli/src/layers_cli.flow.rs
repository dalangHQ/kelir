// see: cans/02-plugin-model.md#Plugin-tiering
// see: cans/02-plugin-model.md#Facade-blindness
// see: cans/artifacts/test-plan/cases/plugin_engine/pe_01_downward_only_tier_dependency_lint.yaml#case_id
use crate::cli_error::CliError;
use std::fs;
use std::path::{Path, PathBuf};

// see: cans/02-plugin-model.md#Facade-blindness
// Accepted seams, same allowlist as the justfile G2 grep: plugin_model (the
// generic KelirPlugin model lives in kelir-interface per
// cans/02-plugin-model.md#Facade-blindness), the plugin_instance generic
// registration method, and [plugin_engine] audit tags.
const FACADE_ALLOWED_SEAMS: &[&str] = &["plugin_model", "plugin_instance"];
const FACADE_ALLOWED_TAGS: &[&str] = &["plugin_engine"];

const RULES: &[(&str, &[&str])] = &[
    (
        "crates/kelir-engine/src",
        &["kelir_plugins::", "kelir_drivers::", "kelir_adapters::"],
    ),
    (
        "crates/kelir-plugins",
        &["kelir_engine::", "kelir_drivers::"],
    ),
    (
        "crates/kelir-drivers",
        &["kelir_plugins::", "kelir_engine::"],
    ),
];

pub fn check() -> Result<(), CliError> {
    let root = crate::workspace_root()?;
    for (path, forbidden) in RULES {
        check_dir(&root.join(path), forbidden)?;
    }
    // see: cans/02-plugin-model.md#Facade-blindness (G4 facade half)
    check_facade(&root)?;
    // see: cans/02-plugin-model.md#Plugin-tiering
    let roster = crate::audit_cli::tier_roster()?;
    // see: cans/artifacts/test-plan/cases/plugin_engine/pe_01_downward_only_tier_dependency_lint.yaml#case_id
    check_tier_source_imports(&root, &roster)?;
    let mut violations = Vec::new();
    for plugin in &roster {
        for dep in &plugin.deps {
            let Some(dep_tier) = crate::audit_cli::tier_of(&roster, dep) else {
                continue;
            };
            if dep_tier.level >= plugin.tier.level {
                violations.push(format!(
                    "layer violation: `{}` (tier {}) depends on `{}` (tier {}); dependencies must be downward-only",
                    plugin.tier.crate_name, plugin.tier.level,
                    dep_tier.crate_name, dep_tier.level,
                ));
            }
        }
        for dep in &plugin.dev_deps {
            let Some(dep_tier) = crate::audit_cli::tier_of(&roster, dep) else {
                continue;
            };
            if dep_tier.level >= plugin.tier.level {
                violations.push(format!(
                    "layer violation: `{}` (tier {}) dev-depends on `{}` (tier {}); dependencies must be downward-only",
                    plugin.tier.crate_name, plugin.tier.level,
                    dep_tier.crate_name, dep_tier.level,
                ));
            }
        }
    }
    if !violations.is_empty() {
        return Err(violations.join("\n").into());
    }
    Ok(())
}

// see: cans/02-plugin-model.md#Facade-blindness
// G4 facade half: the facade src carries zero plugin_ tokens outside the
// accepted generic seams, and the facade manifest declares zero plugin deps.
pub(crate) fn check_facade(root: &Path) -> Result<(), CliError> {
    let mut files = Vec::new();
    collect_rs(&root.join("crates/kelir/src"), &mut files)?;
    for file in files {
        let text = fs::read_to_string(&file)?;
        for (line_no, line) in text.lines().enumerate() {
            for (idx, _) in line.to_lowercase().match_indices("plugin_") {
                let token = line[idx..]
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect::<String>();
                if FACADE_ALLOWED_TAGS
                    .iter()
                    .any(|tag| line.contains(&format!("[{tag}]")))
                {
                    continue;
                }
                let allowed = FACADE_ALLOWED_SEAMS
                    .iter()
                    .any(|name| token == *name || token.starts_with(&format!("{name}_")));
                if token == "plugin_" {
                    return Err(format!(
                        "layer violation: {}:{} carries a bare `plugin_` token; the facade is core (cans/02-plugin-model.md#Facade-blindness)",
                        file.display(),
                        line_no + 1
                    )
                    .into());
                }
                if !allowed {
                    return Err(format!(
                        "layer violation: {}:{}:{} names plugin token `{}`; the facade is core (cans/02-plugin-model.md#Facade-blindness)",
                        file.display(),
                        line_no + 1,
                        idx + 1,
                        token
                    )
                    .into());
                }
            }
        }
    }
    let manifest = fs::read_to_string(root.join("crates/kelir/Cargo.toml"))?;
    for (line_no, line) in manifest.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("plugin-") || trimmed.starts_with("plugin_") {
            return Err(format!(
                "layer violation: crates/kelir/Cargo.toml:{} declares a plugin dependency `{}`; the facade manifest must carry zero plugin deps (cans/02-plugin-model.md#Facade-blindness)",
                line_no + 1,
                trimmed.split_whitespace().next().unwrap_or(trimmed)
            )
            .into());
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/plugin_engine/pe_01_downward_only_tier_dependency_lint.yaml#case_id
// Per-tier plugin-to-plugin source scan: each plugin src may `use` only
// lower-tier plugin crates from the tier roster; same-tier and upward
// imports are rejected (cans/02-plugin-model.md#Plugin-tiering).
pub(crate) fn check_tier_source_imports(
    root: &Path,
    roster: &[kelir_engine::plugin_registry::PluginManifest],
) -> Result<(), CliError> {
    for plugin in roster {
        let src = root
            .join("crates/kelir-plugins")
            .join(&plugin.tier.crate_name)
            .join("src");
        let mut files = Vec::new();
        collect_rs(&src, &mut files)?;
        for file in files {
            let text = fs::read_to_string(&file)?;
            for (line_no, line) in text.lines().enumerate() {
                for (idx, _) in line.match_indices("plugin_") {
                    let seam = line[idx + "plugin_".len()..]
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect::<String>();
                    if seam.is_empty() {
                        continue;
                    }
                    let crate_name = format!("plugin_{seam}").replace('_', "-");
                    let Some(dep_tier) = crate::audit_cli::tier_of(roster, &crate_name) else {
                        continue;
                    };
                    if dep_tier.level >= plugin.tier.level {
                        return Err(format!(
                            "layer violation: {}:{}:{} references `{}` (tier {}); `{}` is tier {} and imports must be downward-only",
                            file.display(),
                            line_no + 1,
                            idx + 1,
                            crate_name,
                            dep_tier.level,
                            plugin.tier.crate_name,
                            plugin.tier.level
                        )
                        .into());
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), CliError> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_rs(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

pub fn check_dir(dir: &Path, forbidden: &[&str]) -> Result<(), CliError> {
    let mut files = Vec::new();
    collect_rs(dir, &mut files)?;
    for file in files {
        let text = fs::read_to_string(&file)?;
        for (line_no, line) in text.lines().enumerate() {
            for needle in forbidden {
                if line.contains(needle) {
                    return Err(format!(
                        "layer violation: {}:{} references forbidden `{}`",
                        file.display(),
                        line_no + 1,
                        needle
                    )
                    .into());
                }
            }
        }
    }
    Ok(())
}
