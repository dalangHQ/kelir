// see: cans/15-operations.md#Coverage-gates
use std::fs;
use std::path::{Path, PathBuf};

const RULES: &[(&str, &[&str])] = &[
    (
        "crates/kelir-core/src",
        &["kelir_plugins::", "kelir_drivers::", "kelir_adapters::"],
    ),
    ("crates/kelir-plugins", &["kelir_core::", "kelir_drivers::"]),
    ("crates/kelir-drivers", &["kelir_plugins::", "kelir_core::"]),
];

pub fn check() -> Result<(), Box<dyn std::error::Error>> {
    // TODO (L3): enforce the per-tier plugin-to-plugin rule per cans/02-plugin-model.md#Plugin-tiering; the scan bans core/driver/adapter prefixes only.
    let root = crate::workspace_root()?;
    for (path, forbidden) in RULES {
        check_dir(&root.join(path), forbidden)?;
    }
    Ok(())
}

pub fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
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

pub fn check_dir(dir: &Path, forbidden: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
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
