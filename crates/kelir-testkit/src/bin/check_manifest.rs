//! Manifest Validator: Asserts 100% parity between manifest.yaml and test case files in cases/**/*.yaml.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(serde::Deserialize)]
struct Manifest {
    plan: PlanMeta,
    cases: Vec<CaseEntry>,
}

#[derive(serde::Deserialize)]
struct PlanMeta {
    total_cases: usize,
    domains: usize,
}

#[derive(serde::Deserialize)]
struct CaseEntry {
    case_id: String,
    file: String,
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

    // Check for missing files declared in manifest
    for declared in &manifest_files {
        if !disk_case_files.contains(declared) {
            eprintln!(
                "ERROR: Case file declared in manifest does not exist on disk: {}",
                declared
            );
            errors += 1;
        }
    }

    // Check for orphaned case files on disk not indexed in manifest
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

    if errors == 0 {
        println!("Manifest verification passed: 196/196 cases synchronized with filesystem.");
        Ok(())
    } else {
        eprintln!("Manifest check failed with {} errors.", errors);
        std::process::exit(1)
    }
}
