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

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
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

pub fn check() -> Result<(), Box<dyn std::error::Error>> {
    for (root, forbidden) in RULES {
        let mut files = Vec::new();
        collect_rs(Path::new(root), &mut files)?;
        for file in files {
            let text = fs::read_to_string(&file)?;
            for (line_no, line) in text.lines().enumerate() {
                for needle in *forbidden {
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
    }
    Ok(())
}
