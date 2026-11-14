// see: cans/01-architecture.md#Codebase-invariants
use crate::{CheckReport, ForbidMode, Level, Policy, crate_of, level_of, structure};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const TODO_MARK: &str = concat!("/", "/ ", "TODO");
// see: cans/01-architecture.md#Codebase-invariants
const CONCERN_LEAK_MIN_RUN: usize = 3;

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
    let files = rs_files(root);
    for file in &files {
        report.files += 1;
        structure::check_file(file, policy, &mut report);
        let level = level_of(file);
        let krate = crate_of(file);
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
        let src = match fs::read_to_string(file) {
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
    check_concern_leak(root, &files, policy, &mut report);
    report
}

// see: cans/01-architecture.md#Codebase-invariants
fn check_concern_leak(root: &Path, files: &[PathBuf], policy: &Policy, report: &mut CheckReport) {
    let mut by_crate: BTreeMap<PathBuf, (usize, usize, Vec<String>)> = BTreeMap::new();
    for file in files {
        if !is_lib_file(file) {
            continue;
        }
        let krate = crate_of(file);
        if krate.as_os_str().is_empty() {
            continue;
        }
        if !read_allowed_levels(&krate.join("Cargo.toml")).contains(&Level::L3) {
            continue;
        }
        let entry = by_crate.entry(krate).or_default();
        match level_of(file) {
            Level::L2 => entry.0 += 1,
            Level::L3 => {
                entry.1 += 1;
                entry.2.push(
                    file.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                );
            }
            Level::L1 => {}
        }
    }
    for (krate, (flow, run, run_files)) in by_crate {
        if flow != 0 || run < CONCERN_LEAK_MIN_RUN {
            continue;
        }
        let at = krate
            .strip_prefix(root)
            .unwrap_or(&krate)
            .display()
            .to_string();
        let detail = format!(
            "concern leak: {run} .run.rs files with no .flow.rs seam ({})",
            run_files.join(", ")
        );
        if policy.forbid_mode == ForbidMode::Error {
            report
                .fails
                .push(format!("{at}: [concern-leak:C1] {detail}"));
        } else {
            report.warns.push(format!(
                "{at}: {detail} (concern-leak:C1, warn-mode rollout)"
            ));
        }
    }
}

// see: cans/01-architecture.md#Codebase-invariants
fn is_lib_file(path: &Path) -> bool {
    let mut under_src = false;
    for part in path.components() {
        match part.as_os_str().to_str() {
            Some("src") => under_src = true,
            Some("bin") if under_src => return false,
            _ => {}
        }
    }
    under_src
}
