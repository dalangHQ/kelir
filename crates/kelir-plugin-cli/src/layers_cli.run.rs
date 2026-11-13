// see: cans/15-operations.md#Coverage-gates
// see: cans/02-plugin-model.md#Plugin-tiering
use crate::cli_error::CliError;
use std::fs;
use std::path::{Path, PathBuf};

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
    // see: cans/02-plugin-model.md#Plugin-tiering
    let roster = crate::audit_cli::tier_roster()?;
    let mut violations = Vec::new();
    for plugin in &roster {
        for dep in &plugin.deps {
            if let Some(dep_tier) = crate::audit_cli::tier_of(&roster, dep) {
                if dep_tier.level >= plugin.tier.level {
                    violations.push(format!(
                        "layer violation: `{}` (tier {}) depends on `{}` (tier {}); dependencies must be downward-only",
                        plugin.tier.crate_name, plugin.tier.level,
                        dep_tier.crate_name, dep_tier.level,
                    ));
                }
            }
        }
        for dep in &plugin.dev_deps {
            if let Some(dep_tier) = crate::audit_cli::tier_of(&roster, dep) {
                if dep_tier.level >= plugin.tier.level {
                    violations.push(format!(
                        "layer violation: `{}` (tier {}) dev-depends on `{}` (tier {}); dependencies must be downward-only",
                        plugin.tier.crate_name, plugin.tier.level,
                        dep_tier.crate_name, dep_tier.level,
                    ));
                }
            }
        }
    }
    if !violations.is_empty() {
        return Err(violations.join("\n").into());
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
