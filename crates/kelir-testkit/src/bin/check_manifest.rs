use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[allow(dead_code)]
#[derive(serde::Deserialize)]
struct Manifest {
    plan: PlanMeta,
    cases: Vec<ManifestCase>,
}

#[derive(serde::Deserialize)]
struct PlanMeta {
    total_cases: usize,
}

#[allow(dead_code)]
#[derive(serde::Deserialize)]
struct ManifestCase {
    case_id: String,
    file: String,
    domain: Option<String>,
    target_tier: Option<String>,
    target_crate: Option<String>,
    quadrant: Option<String>,
}

#[derive(serde::Deserialize)]
struct CaseFile {
    invariant: Option<Invariant>,
    platform_target: Option<PlatformTarget>,
    target_tier: Option<String>,
    target_crate: Option<String>,
    execution: Option<Execution>,
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

#[derive(serde::Deserialize)]
struct Execution {
    trigger: Option<String>,
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

// see: scripts/codebase_refs.py#slugify
fn is_dash_char(c: char) -> bool {
    matches!(
        c,
        '-' | '_'
            | '\u{2010}'
            | '\u{2011}'
            | '\u{2012}'
            | '\u{2013}'
            | '\u{2014}'
            | '\u{2015}'
            | '\u{2043}'
            | '\u{2212}'
            | '\u{fe58}'
            | '\u{fe63}'
            | '\u{ff0d}'
    )
}

// see: scripts/codebase_refs.py#slugify
fn slugify(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_whitespace() || is_dash_char(c) {
            if !out.is_empty() && !out.ends_with('-') {
                out.push('-');
            }
        } else {
            out.push(c);
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

fn norm_slug(text: &str) -> String {
    slugify(text).to_lowercase()
}

// see: scripts/codebase_refs.py#_strip_pointer_suffix
fn strip_pointer_suffix(text: &str) -> String {
    let Some(see_pos) = text.find("see:") else {
        return text.to_string();
    };
    let after = &text[see_pos + 4..];
    let token = after.split_whitespace().next().unwrap_or("");
    let is_pointer = token.starts_with('#')
        || token.starts_with('[')
        || token.contains(".md")
        || token.contains('#')
        || token.starts_with("artifacts/");
    if !is_pointer {
        return text.to_string();
    }
    text[..see_pos]
        .trim_end_matches([' ', ':', '('])
        .trim()
        .to_string()
}

// see: scripts/codebase_refs.py#_md_texts
fn md_leaves(text: &str) -> Vec<String> {
    let mut leaves = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let title = if let Some(rest) = trimmed.strip_prefix("- ") {
            Some(rest.trim().to_string())
        } else if trimmed.starts_with('#') {
            let hashes = trimmed.chars().take_while(|c| *c == '#').count();
            if (1..=6).contains(&hashes) && trimmed[hashes..].starts_with(' ') {
                let body = trimmed[hashes..].trim();
                let body = body.trim_end_matches('#').trim();
                Some(body.to_string())
            } else {
                None
            }
        } else {
            None
        };
        if let Some(title) = title {
            let title = strip_pointer_suffix(&title);
            if !title.is_empty() {
                leaves.push(title);
            }
        }
    }
    leaves
}

fn load_leaves(
    repo_root: &Path,
    target: &str,
    cache: &mut HashMap<String, Vec<String>>,
) -> Option<Vec<String>> {
    if let Some(leaves) = cache.get(target) {
        return Some(leaves.clone());
    }
    let text = fs::read_to_string(repo_root.join(target)).ok()?;
    let leaves: Vec<String> = md_leaves(&text)
        .into_iter()
        .map(|leaf| slugify(&leaf))
        .filter(|leaf| !leaf.is_empty())
        .collect();
    cache.insert(target.to_string(), leaves.clone());
    Some(leaves)
}

fn law_reference_error(
    reference: &str,
    repo_root: &Path,
    cache: &mut HashMap<String, Vec<String>>,
) -> Option<String> {
    let Some((file, anchor)) = reference.split_once('#') else {
        return Some("malformed law_reference, expected cans/<file>.md#<leaf>".to_string());
    };
    if !file.starts_with("cans/") || !file.ends_with(".md") || anchor.is_empty() {
        return Some("malformed law_reference, expected cans/<file>.md#<leaf>".to_string());
    }
    if !repo_root.join(file).is_file() {
        return Some("missing target file".to_string());
    }
    let Some(leaves) = load_leaves(repo_root, file, cache) else {
        return Some("cannot read target file".to_string());
    };
    let want = norm_slug(anchor);
    if leaves.iter().any(|leaf| norm_slug(leaf) == want) {
        None
    } else {
        Some("unresolved anchor leaf in target file".to_string())
    }
}

// see: cans/artifacts/test-plan/manifest.yaml#plan.crate_tier_convention
fn valid_target_crate(repo_root: &Path, value: &str) -> bool {
    let path = repo_root.join(value);
    if !path.is_dir() {
        return false;
    }
    if path.join("Cargo.toml").is_file()
        || path.join("package.json").is_file()
        || path.join("pyproject.toml").is_file()
    {
        return true;
    }
    let Ok(entries) = fs::read_dir(&path) else {
        return false;
    };
    entries.filter_map(|entry| entry.ok()).any(|entry| {
        entry.path().is_dir()
            && (entry.path().join("Cargo.toml").is_file()
                || entry.path().join("package.json").is_file())
    })
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
    let mut leaf_cache: HashMap<String, Vec<String>> = HashMap::new();

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

    for case in &manifest.cases {
        match &case.target_tier {
            Some(tier) if valid_enum(tier, &["unit", "integration", "e2e"]) => {}
            Some(tier) => {
                eprintln!(
                    "ERROR: manifest {}: invalid target_tier: {}",
                    case.case_id, tier
                );
                errors += 1;
            }
            None => {
                eprintln!("ERROR: manifest {}: missing target_tier", case.case_id);
                errors += 1;
            }
        }
        match &case.target_crate {
            Some(krate) if valid_target_crate(&repo_root, krate) => {}
            Some(krate) => {
                eprintln!(
                    "ERROR: manifest {}: target_crate is not a crate or crate bucket on disk: {}",
                    case.case_id, krate
                );
                errors += 1;
            }
            None => {
                eprintln!("ERROR: manifest {}: missing target_crate", case.case_id);
                errors += 1;
            }
        }
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
                if let Some(reason) = law_reference_error(&reference, &repo_root, &mut leaf_cache) {
                    eprintln!("ERROR: {}: {}: {}", rel, reason, reference);
                    errors += 1;
                }
            }
            None => {
                eprintln!("ERROR: {}: missing invariant.law_reference", rel);
                errors += 1;
            }
        }
        match &case.target_tier {
            Some(tier) if valid_enum(tier, &["unit", "integration", "e2e"]) => {}
            Some(tier) => {
                eprintln!("ERROR: {}: invalid target_tier: {}", rel, tier);
                errors += 1;
            }
            None => {
                eprintln!("ERROR: {}: missing target_tier", rel);
                errors += 1;
            }
        }
        match &case.target_crate {
            Some(krate) if valid_target_crate(&repo_root, krate) => {}
            Some(krate) => {
                eprintln!(
                    "ERROR: {}: target_crate is not a crate or crate bucket on disk: {}",
                    rel, krate
                );
                errors += 1;
            }
            None => {
                eprintln!("ERROR: {}: missing target_crate", rel);
                errors += 1;
            }
        }
        match case.execution.and_then(|exec| exec.trigger) {
            Some(trigger)
                if valid_enum(
                    &trigger,
                    &["rust_api", "vsock_frame", "net_frame", "fuse_op", "sdk"],
                ) => {}
            Some(trigger) => {
                eprintln!("ERROR: {}: invalid execution.trigger: {}", rel, trigger);
                errors += 1;
            }
            None => {
                eprintln!("ERROR: {}: missing execution.trigger", rel);
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

    for case in &manifest.cases {
        let path = repo_root.join("cans/artifacts/test-plan").join(&case.file);
        if !path.is_file() {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(case_file) = serde_yaml::from_str::<CaseFile>(&raw) else {
            continue;
        };
        if case.target_tier.is_some()
            && case_file.target_tier.is_some()
            && case.target_tier != case_file.target_tier
        {
            eprintln!(
                "ERROR: {}: manifest target_tier {:?} disagrees with case file {:?}",
                case.case_id, case.target_tier, case_file.target_tier
            );
            errors += 1;
        }
        if case.target_crate.is_some()
            && case_file.target_crate.is_some()
            && case.target_crate != case_file.target_crate
        {
            eprintln!(
                "ERROR: {}: manifest target_crate {:?} disagrees with case file {:?}",
                case.case_id, case.target_crate, case_file.target_crate
            );
            errors += 1;
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
