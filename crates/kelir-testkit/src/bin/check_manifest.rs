use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[allow(dead_code)]
#[derive(serde::Deserialize)]
struct Manifest {
    plan: PlanMeta,
    cases: Vec<CaseEntry>,
}

#[derive(serde::Deserialize)]
struct PlanMeta {
    total_cases: usize,
}

#[allow(dead_code)]
#[derive(serde::Deserialize)]
struct CaseEntry {
    case_id: String,
    file: String,
}

#[derive(serde::Deserialize)]
struct CaseFile {
    invariant: Option<Invariant>,
    platform_target: Option<PlatformTarget>,
}

#[derive(serde::Deserialize)]
struct Invariant {
    law_reference: Option<String>,
}

#[derive(serde::Deserialize)]
struct PlatformTarget {
    os: Option<String>,
    arch: Option<String>,
    hypervisor: Option<String>,
}

fn valid_enum(value: &str, allowed: &[&str]) -> bool {
    allowed.contains(&value)
}

fn valid_target_field(field: &Option<String>, allowed: &[&str]) -> bool {
    match field.as_deref() {
        Some(value) => valid_enum(value, allowed),
        None => false,
    }
}

fn valid_law_reference(reference: &str, repo_root: &Path) -> bool {
    let Some((file, anchor)) = reference.split_once('#') else {
        return false;
    };
    if !file.starts_with("cans/") || !file.ends_with(".md") || anchor.is_empty() {
        return false;
    }
    // TODO (L2): resolve law_reference anchor leaf against target file headings.
    repo_root.join(file).is_file()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .ok_or("Failed to locate workspace root")?
        .to_path_buf();

    let manifest_path = repo_root.join("cans/artifacts/test-plan/manifest.yaml");
    let cases_dir = repo_root.join("cans/artifacts/test-plan/cases");

    let manifest_raw = fs::read_to_string(&manifest_path)?;
    let manifest: Manifest = serde_yaml::from_str(&manifest_raw)?;

    let mut disk_case_files = HashSet::new();
    for entry in WalkDir::new(&cases_dir).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.extension().is_some_and(|ext| ext == "yaml") {
            let rel_p = p
                .strip_prefix(repo_root.join("cans/artifacts/test-plan"))?
                .to_string_lossy()
                .to_string();
            disk_case_files.insert(rel_p);
        }
    }

    let mut manifest_files = HashSet::new();
    for case in &manifest.cases {
        manifest_files.insert(case.file.clone());
    }

    let mut errors = 0;

    for declared in &manifest_files {
        if !disk_case_files.contains(declared) {
            eprintln!(
                "ERROR: Case file declared in manifest does not exist on disk: {}",
                declared
            );
            errors += 1;
        }
    }

    for on_disk in &disk_case_files {
        if !manifest_files.contains(on_disk) {
            eprintln!(
                "ERROR: Orphaned test case file on disk not indexed in manifest.yaml: {}",
                on_disk
            );
            errors += 1;
        }
    }

    if manifest.cases.len() != manifest.plan.total_cases {
        eprintln!(
            "ERROR: Total case count mismatch: manifest.plan declares {} but contains {}",
            manifest.plan.total_cases,
            manifest.cases.len()
        );
        errors += 1;
    }

    let mut disk_files: Vec<String> = disk_case_files.iter().cloned().collect();
    disk_files.sort();
    let mut warnings = 0;
    for rel in &disk_files {
        let path = repo_root.join("cans/artifacts/test-plan").join(rel);
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(err) => {
                eprintln!("ERROR: Cannot read case file {}: {}", rel, err);
                errors += 1;
                continue;
            }
        };
        let case: CaseFile = match serde_yaml::from_str(&raw) {
            Ok(case) => case,
            Err(err) => {
                eprintln!("ERROR: Cannot parse case file {}: {}", rel, err);
                errors += 1;
                continue;
            }
        };
        match case.invariant.and_then(|inv| inv.law_reference) {
            Some(reference) => {
                if !valid_law_reference(&reference, &repo_root) {
                    eprintln!(
                        "ERROR: {}: malformed law_reference or missing target file: {}",
                        rel, reference
                    );
                    errors += 1;
                }
            }
            None => {
                eprintln!("ERROR: {}: missing invariant.law_reference", rel);
                errors += 1;
            }
        }
        match case.platform_target {
            Some(target) => {
                if !valid_target_field(&target.os, &["linux", "macos", "windows", "android", "any"])
                {
                    eprintln!("ERROR: {}: invalid or missing platform_target.os", rel);
                    errors += 1;
                }
                if !valid_target_field(&target.arch, &["x86_64", "aarch64", "any"]) {
                    eprintln!("ERROR: {}: invalid or missing platform_target.arch", rel);
                    errors += 1;
                }
                if !valid_target_field(
                    &target.hypervisor,
                    &["kvm", "hvf", "whp", "avf", "emulated", "none"],
                ) {
                    eprintln!(
                        "ERROR: {}: invalid or missing platform_target.hypervisor",
                        rel
                    );
                    errors += 1;
                }
            }
            None => {
                warnings += 1;
            }
        }
    }

    if errors == 0 {
        println!(
            "Manifest verification passed: {}/{} cases synchronized with filesystem.",
            manifest.cases.len(),
            manifest.plan.total_cases
        );
        if warnings > 0 {
            println!(
                "WARNING: {} case files lack platform_target; validated when present until Part 2 lands.",
                warnings
            );
        }
        Ok(())
    } else {
        eprintln!("Manifest check failed with {} errors.", errors);
        std::process::exit(1)
    }
}
