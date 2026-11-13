// see: cans/01-architecture.md#Codebase-invariants
use crate::{CheckReport, ForbidMode, Level, Policy, crate_of, level_of, structure};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const TODO_MARK: &str = concat!("/", "/ ", "TODO");

pub fn read_allowed_levels(cargo_toml: &Path) -> Vec<Level> {
    let text = match fs::read_to_string(cargo_toml) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let value: toml::Value = match toml::from_str(&text) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let array = value
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("kelir"))
        .and_then(|k| k.get("allowed_levels"))
        .and_then(|a| a.as_array());
    match array {
        Some(items) => items
            .iter()
            .filter_map(|v| v.as_str())
            .filter_map(Level::from_name)
            .collect(),
        None => Vec::new(),
    }
}

pub fn rs_files(root: &Path) -> Vec<PathBuf> {
    let base = {
        let crates = root.join("crates");
        if crates.is_dir() {
            crates
        } else {
            root.to_path_buf()
        }
    };
    let mut out = Vec::new();
    let walker = WalkDir::new(base).into_iter().filter_entry(|e| {
        !(e.file_type().is_dir()
            && matches!(e.file_name().to_str(), Some("target" | "tests" | "benches")))
    });
    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path().to_path_buf();
        if path.extension().map(|e| e == "rs").unwrap_or(false) {
            out.push(path);
        }
    }
    out.sort();
    out
}

pub fn check_tree(root: &Path, policy: &Policy) -> CheckReport {
    let mut report = CheckReport::default();
    for file in rs_files(root) {
        report.files += 1;
        structure::check_file(&file, policy, &mut report);
        let level = level_of(&file);
        let krate = crate_of(&file);
        if krate.as_os_str().is_empty() {
            report.hard_fails += 1;
            report
                .fails
                .push(format!("{}: no enclosing crate", file.display()));
            continue;
        }
        let allowed = read_allowed_levels(&krate.join("Cargo.toml"));
        if !allowed.contains(&level) {
            report.hard_fails += 1;
            report.fails.push(format!(
                "{}: {} not in allowed_levels {:?} of {}",
                file.display(),
                level,
                allowed,
                krate.display()
            ));
            continue;
        }
        let src = match fs::read_to_string(&file) {
            Ok(s) => s,
            Err(e) => {
                report.hard_fails += 1;
                report
                    .fails
                    .push(format!("{}: cannot read: {}", file.display(), e));
                continue;
            }
        };
        for r in policy.rules_for(level, true) {
            for (n, line) in src.lines().enumerate() {
                if r.regex.is_match(line) {
                    if policy.forbid_mode == ForbidMode::Error {
                        let text = if let Some(h) = r.hint.as_deref().filter(|h| !h.is_empty()) {
                            format!("{}:{}: [forbid:{}] {}", file.display(), n + 1, r.id, h)
                        } else {
                            format!("{}:{}: [forbid:{}]", file.display(), n + 1, r.id)
                        };
                        report.fails.push(text);
                    } else {
                        report.warns.push(format!(
                            "{}:{}: {} (forbid, warn-mode rollout)",
                            file.display(),
                            n + 1,
                            r.id
                        ));
                    }
                }
            }
        }
        for r in policy.rules_for(level, false) {
            for (n, line) in src.lines().enumerate() {
                if r.regex.is_match(line) {
                    report
                        .warns
                        .push(format!("{}:{}: {} (warn)", file.display(), n + 1, r.id));
                }
            }
        }
        if level == Level::L3 && src.contains(TODO_MARK) {
            report.warns.push(format!(
                "{}: L3 carries a TODO marker (prepared stub, warns until L3 fills it)",
                file.display()
            ));
        }
    }
    report
}
