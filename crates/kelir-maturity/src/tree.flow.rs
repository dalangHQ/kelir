// see: cans/01-architecture.md#Codebase-invariants
use crate::ast::{FileCtx, check_file_ast, collect_fn_names, dead_code_shadowing};
use crate::ast::read_crate_metadata;
use crate::structure;
use crate::{CheckReport, ForbidMode, Level, Policy, crate_of, level_of};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const CONCERN_LEAK_MIN_RUN: usize = 3;

pub fn is_tooling_file(path: &Path) -> bool {
    if is_bin_entry_point(path) {
        return true;
    }
    let krate = crate_of(path);
    crate::ast::crate_role_is_tooling(&krate)
}

pub fn is_bin_entry_point(path: &Path) -> bool {
    let parts: Vec<&str> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();
    parts.windows(2).any(|w| w[0] == "src" && w[1] == "bin")
        || parts.windows(2).any(|w| w[0] == "src" && w[1] == "main.rs")
}

pub fn is_import_only_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("use ") || trimmed.starts_with("pub use ")
}

pub fn line_has_guest_io(line: &str) -> bool {
    line.contains(concat!("rusqlite", "::"))
        || line.contains(concat!("tokio", "::fs::"))
}

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
    // (crate dir -> (flow files, run files, run fn names, flow sources))
    let mut by_crate: BTreeMap<PathBuf, CrateSeam> = BTreeMap::new();
    for file in &files {
        report.files += 1;
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
        structure::check_file(file, policy, &src, &mut report);
        let tooling = is_bin_entry_point(file) || crate::ast::crate_role_is_tooling(&krate);
        let meta = crate::ast::read_crate_metadata(&krate.join("Cargo.toml"));
        let exemptions = if is_bin_entry_point(file) {
            Vec::new()
        } else {
            meta.exemptions
        };
        let ctx = FileCtx {
            path: file,
            level,
            tooling,
            exemptions: &exemptions,
        };
        check_file_ast(ctx, &src, &mut report, policy.forbid_mode);
        if is_lib_file(file) {
            let seam = by_crate.entry(krate).or_default();
            match level {
                Level::L2 => seam.flow.push((file.display().to_string(), src.clone())),
                Level::L3 => seam.run_files += 1,
                Level::L1 => {}
            }
            if level == Level::L3 {
                seam.run_fns.extend(collect_fn_names(&src));
            }
        }
    }
    check_c2_seam(root, &by_crate, policy, &mut report);
    report
}

#[derive(Default)]
struct CrateSeam {
    flow: Vec<(String, String)>,
    run_files: usize,
    run_fns: Vec<String>,
}

// see: cans/01-architecture.md#Codebase-invariants
fn check_c2_seam(
    root: &Path,
    by_crate: &BTreeMap<PathBuf, CrateSeam>,
    policy: &Policy,
    report: &mut CheckReport,
) {
    for (krate, seam) in by_crate {
        if seam.flow.is_empty() && seam.run_files == 0 {
            continue;
        }
        let allowed = read_allowed_levels(&krate.join("Cargo.toml"));
        let at = krate
            .strip_prefix(root)
            .unwrap_or(krate)
            .display()
            .to_string();
        if !allowed.contains(&Level::L3) {
            // C2 direction 1: crate without L3 must have zero .run.rs files
            // (and zero markers; the markers are caught per file by S8).
            if seam.run_files > 0 {
                let detail = format!(
                    "no-L3 crate declares {} .run.rs files; allowed_levels has no L3",
                    seam.run_files
                );
                if policy.forbid_mode == ForbidMode::Error {
                    report
                        .fails
                        .push(format!("{at}: [seam:C2] {detail}"));
                } else {
                    report
                        .warns
                        .push(format!("{at}: {detail} (seam:C2, warn-mode rollout)"));
                }
            }
        } else if !seam.flow.is_empty() && seam.run_files == 0 {
            // C2 direction 2: L3 crate with .flow.rs files needs >=1 .run.rs.
            let detail = format!(
                "L3-capable crate has {} .flow.rs files with zero .run.rs counterparts",
                seam.flow.len()
            );
            if policy.forbid_mode == ForbidMode::Error {
                report.fails.push(format!("{at}: [seam:C2] {detail}"));
            } else {
                report
                    .warns
                    .push(format!("{at}: {detail} (seam:C2, warn-mode rollout)"));
            }
        }
        // C2 direction 3: dead_code shadowing between .flow.rs and .run.rs.
        for (file_at, src) in &seam.flow {
            dead_code_shadowing(src, &seam.run_fns, file_at, report, policy.forbid_mode);
        }
        // C1 (kept): >=3 .run.rs with zero .flow.rs seam.
        if seam.flow.is_empty() && seam.run_files >= CONCERN_LEAK_MIN_RUN {
            let detail = format!(
                "concern leak: {} .run.rs files with no .flow.rs seam",
                seam.run_files
            );
            if policy.forbid_mode == ForbidMode::Error {
                report
                    .fails
                    .push(format!("{at}: [concern-leak:C1] {detail}"));
            } else {
                report
                    .warns
                    .push(format!("{at}: {detail} (concern-leak:C1, warn-mode rollout)"));
            }
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
