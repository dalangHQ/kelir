use kelir_testkit::spec_anchors::{self, AnchorIndex};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(serde::Deserialize)]
struct Manifest {
    plan: PlanMeta,
    domains: std::collections::BTreeMap<String, DomainMeta>,
    cases: Vec<CaseEntry>,
}

#[derive(serde::Deserialize)]
struct PlanMeta {
    total_cases: usize,
    domains: usize,
}

#[derive(serde::Deserialize)]
struct DomainMeta {
    case_count: usize,
}

#[allow(dead_code)]
#[derive(serde::Deserialize)]
struct CaseEntry {
    case_id: String,
    domain: String,
    file: String,
    target_tier: Option<String>,
    target_crate: Option<String>,
    quadrant: Option<String>,
}

#[derive(serde::Deserialize)]
struct CaseFile {
    case_id: String,
    domain: String,
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

#[derive(serde::Deserialize)]
struct CaseMap {
    cases: Vec<MapEntry>,
    #[serde(default)]
    harness_tests: Vec<HarnessTest>,
}

// see: cans/artifacts/test-plan/case-to-test-map.yaml#harness_tests
#[derive(serde::Deserialize)]
struct HarnessTest {
    file: String,
    reason: String,
}

#[derive(serde::Deserialize)]
struct MapEntry {
    case_id: String,
    status: String,
    test_file: Option<String>,
    test_fn: Option<String>,
    reason: Option<String>,
}

#[derive(serde::Deserialize)]
struct AnchorException {
    reference: String,
    reason: String,
}

// see: cans/15-operations.md#Coverage-gates
enum LawRefStatus<'a> {
    Valid,
    Excepted(&'a str),
    Broken(&'a str),
}

// see: cans/15-operations.md#Coverage-gates
fn check_law_reference<'a>(
    reference: &'a str,
    repo_root: &Path,
    index: &AnchorIndex,
    exceptions: &'a [AnchorException],
) -> LawRefStatus<'a> {
    let Some((file, anchor)) = reference.split_once('#') else {
        return LawRefStatus::Broken("missing #anchor leaf in");
    };
    if !file.starts_with("cans/") || !file.ends_with(".md") || anchor.is_empty() {
        return LawRefStatus::Broken("malformed");
    }
    if !repo_root.join(file).is_file() {
        return LawRefStatus::Broken("target file missing for");
    }
    let want = spec_anchors::norm(anchor);
    let known = index
        .files
        .get(file)
        .is_some_and(|anchors| anchors.iter().any(|a| spec_anchors::norm(a) == want));
    if known {
        return LawRefStatus::Valid;
    }
    match exceptions
        .iter()
        .find(|exception| exception.reference == reference)
    {
        Some(exception) => LawRefStatus::Excepted(&exception.reason),
        None => LawRefStatus::Broken("unresolved anchor leaf in"),
    }
}

fn valid_enum(value: &str, allowed: &[&str]) -> bool {
    allowed.contains(&value)
}

// see: cans/artifacts/test-plan/manifest.yaml#domains
fn domain_of(rel: &str) -> Option<&str> {
    let after_cases = rel.strip_prefix("cases/")?;
    let (domain, _rest) = after_cases.split_once('/')?;
    Some(domain)
}

fn valid_target_field(field: &Option<String>, allowed: &[&str]) -> bool {
    match field.as_deref() {
        Some(value) => valid_enum(value, allowed),
        None => false,
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

    let index_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("anchors.json");
    let committed_index_raw = fs::read_to_string(&index_path).map_err(|err| {
        format!(
            "cannot read {}: {err}; run `cargo run -p kelir-testkit --bin anchors` first",
            index_path.display()
        )
    })?;
    let index: AnchorIndex = spec_anchors::parse_index(&committed_index_raw)
        .map_err(|err| format!("malformed {}: {err}", index_path.display()))?;
    let exceptions_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("anchor-exceptions.yaml");
    let exceptions_raw = fs::read_to_string(&exceptions_path)
        .map_err(|err| format!("cannot read {}: {err}", exceptions_path.display()))?;
    let exceptions: Vec<AnchorException> = serde_yaml::from_str(&exceptions_raw)
        .map_err(|err| format!("malformed {}: {err}", exceptions_path.display()))?;

    let mut errors = 0;
    let live_index = spec_anchors::render_index(&spec_anchors::build_index(&repo_root)?)?;
    if committed_index_raw != live_index {
        eprintln!(
            "ERROR: {} is stale; regenerate with `cargo run -p kelir-testkit --bin anchors`.",
            index_path.display()
        );
        errors += 1;
    }

    let mut disk_case_files = HashSet::new();
    let mut disk_domain_counts: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for entry in WalkDir::new(&cases_dir).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.extension().is_some_and(|ext| ext == "yaml") {
            let rel_p = p
                .strip_prefix(repo_root.join("cans/artifacts/test-plan"))?
                .to_string_lossy()
                .to_string();
            match domain_of(&rel_p) {
                Some(domain) => {
                    *disk_domain_counts.entry(domain.to_string()).or_insert(0) += 1;
                }
                None => {
                    eprintln!("ERROR: Case file outside a domain directory: {}", rel_p);
                    errors += 1;
                }
            }
            disk_case_files.insert(rel_p);
        }
    }

    let mut manifest_files = HashSet::new();
    let mut manifest_by_file: std::collections::HashMap<&str, &CaseEntry> =
        std::collections::HashMap::new();
    for case in &manifest.cases {
        manifest_files.insert(case.file.clone());
        if manifest_by_file.insert(case.file.as_str(), case).is_some() {
            eprintln!(
                "ERROR: Duplicate case file entry in manifest: {}",
                case.file
            );
            errors += 1;
        }
    }

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

    // see: cans/artifacts/test-plan/manifest.yaml#domains
    let disk_total: usize = disk_domain_counts.values().sum();
    if manifest.plan.total_cases != disk_total {
        eprintln!(
            "ERROR: Total case count mismatch: manifest.plan declares {} but cases/ holds {} case files",
            manifest.plan.total_cases, disk_total
        );
        errors += 1;
    }
    if manifest.plan.domains != manifest.domains.len()
        || manifest.plan.domains != disk_domain_counts.len()
    {
        eprintln!(
            "ERROR: Domain count mismatch: manifest.plan declares {}, manifest.domains lists {}, cases/ holds {}",
            manifest.plan.domains,
            manifest.domains.len(),
            disk_domain_counts.len()
        );
        errors += 1;
    }
    for (domain, declared) in &manifest.domains {
        match disk_domain_counts.get(domain) {
            Some(disk_count) if disk_count == &declared.case_count => {}
            Some(disk_count) => {
                eprintln!(
                    "ERROR: Domain {} case_count mismatch: manifest declares {} but cases/ holds {}",
                    domain, declared.case_count, disk_count
                );
                errors += 1;
            }
            None => {
                eprintln!(
                    "ERROR: Domain {} declared in manifest.domains has no case directory",
                    domain
                );
                errors += 1;
            }
        }
    }
    for domain in disk_domain_counts.keys() {
        if !manifest.domains.contains_key(domain) {
            eprintln!(
                "ERROR: Domain {} on disk missing from manifest.domains",
                domain
            );
            errors += 1;
        }
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
    let mut excepted = 0;
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
        // see: cans/artifacts/test-plan/manifest.yaml#cases
        if let Some(entry) = manifest_by_file.get(rel.as_str()) {
            if entry.case_id != case.case_id {
                eprintln!(
                    "ERROR: {}: case_id mismatch: manifest declares {}, case file header declares {}",
                    rel, entry.case_id, case.case_id
                );
                errors += 1;
            }
            if entry.domain != case.domain {
                eprintln!(
                    "ERROR: {}: domain mismatch: manifest declares {}, case file header declares {}",
                    rel, entry.domain, case.domain
                );
                errors += 1;
            }
            if domain_of(rel) != Some(entry.domain.as_str()) {
                eprintln!(
                    "ERROR: {}: domain directory does not match manifest domain {}",
                    rel, entry.domain
                );
                errors += 1;
            }
        }
        match case.invariant.and_then(|inv| inv.law_reference) {
            Some(reference) => {
                match check_law_reference(&reference, &repo_root, &index, &exceptions) {
                    LawRefStatus::Valid => {}
                    LawRefStatus::Excepted(reason) => {
                        excepted += 1;
                        println!(
                            "WARNING: {}: law_reference excepted ({}): {}",
                            rel, reason, reference
                        );
                    }
                    LawRefStatus::Broken(reason) => {
                        eprintln!("ERROR: {}: {} law_reference: {}", rel, reason, reference);
                        errors += 1;
                    }
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

    // see: cans/artifacts/test-plan/case-to-test-map.yaml#cases
    let map_path = repo_root.join("cans/artifacts/test-plan/case-to-test-map.yaml");
    let map_raw = fs::read_to_string(&map_path)
        .map_err(|err| format!("cannot read {}: {err}", map_path.display()))?;
    let map: CaseMap = serde_yaml::from_str(&map_raw)
        .map_err(|err| format!("malformed {}: {err}", map_path.display()))?;
    if map.cases.len() != manifest.plan.total_cases {
        eprintln!(
            "ERROR: case-to-test-map holds {} cases, manifest declares {}",
            map.cases.len(),
            manifest.plan.total_cases
        );
        errors += 1;
    }
    let manifest_ids: HashSet<&str> = manifest.cases.iter().map(|c| c.case_id.as_str()).collect();
    let mut map_ids = HashSet::new();
    let mut listed_files = HashSet::new();
    // see: cans/artifacts/test-plan/case-to-test-map.yaml#harness_tests
    for harness in &map.harness_tests {
        if harness.reason.is_empty() {
            eprintln!("ERROR: harness test {} carries no reason", harness.file);
            errors += 1;
        }
        listed_files.insert(harness.file.clone());
    }
    let mut map_implemented = 0;
    let mut map_red = 0;
    let mut map_gap = 0;
    for entry in &map.cases {
        if !map_ids.insert(entry.case_id.as_str()) {
            eprintln!("ERROR: duplicate case-to-test-map entry: {}", entry.case_id);
            errors += 1;
        }
        if !manifest_ids.contains(entry.case_id.as_str()) {
            eprintln!("ERROR: map entry has no manifest case: {}", entry.case_id);
            errors += 1;
        }
        match entry.status.as_str() {
            "implemented" => map_implemented += 1,
            "red" => {
                map_red += 1;
                if entry.test_file.is_none() || entry.test_fn.is_none() {
                    eprintln!(
                        "ERROR: red entry {} requires test_file and test_fn",
                        entry.case_id
                    );
                    errors += 1;
                }
                if entry.reason.as_deref().is_none_or(|r| r.is_empty()) {
                    eprintln!(
                        "ERROR: red entry {} carries no failure-signature reason",
                        entry.case_id
                    );
                    errors += 1;
                }
            }
            "gap" if entry.reason.as_deref().is_none_or(|r| r.is_empty()) => {
                map_gap += 1;
                eprintln!("ERROR: gap entry {} carries no reason", entry.case_id);
                errors += 1;
            }
            "gap" => map_gap += 1,
            other => {
                eprintln!("ERROR: map {}: invalid status: {}", entry.case_id, other);
                errors += 1;
            }
        }
        if let (Some(file), Some(func)) = (&entry.test_file, &entry.test_fn) {
            listed_files.insert(file.clone());
            let path = repo_root.join(file.as_str());
            if !path.is_file() {
                eprintln!("ERROR: map {}: test file missing: {file}", entry.case_id);
                errors += 1;
            } else {
                let needle = if file.ends_with(".py") {
                    format!("def {func}")
                } else if file.ends_with(".ts") {
                    format!("function {func}")
                } else {
                    format!("fn {func}")
                };
                if !fs::read_to_string(&path)
                    .map(|text| text.contains(&needle))
                    .unwrap_or(false)
                {
                    eprintln!(
                        "ERROR: map {}: test fn {func} not found in {file}",
                        entry.case_id
                    );
                    errors += 1;
                }
            }
        } else if entry.test_file.is_some() || entry.test_fn.is_some() {
            eprintln!(
                "ERROR: map {}: test_file and test_fn must be set together",
                entry.case_id
            );
            errors += 1;
        }
    }
    for entry in WalkDir::new(repo_root.join("crates"))
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if p.extension().is_some_and(|ext| ext == "rs")
            && p.parent()
                .is_some_and(|d| d.file_name().is_some_and(|n| n == "tests"))
        {
            let rel = p
                .strip_prefix(&repo_root)
                .unwrap_or(p)
                .to_string_lossy()
                .to_string();
            if !listed_files.contains(&rel) {
                eprintln!("ERROR: test file {rel} is not listed in case-to-test-map.yaml");
                errors += 1;
            }
        }
    }

    let mut red_by_language = [0usize; 3];
    for entry in &map.cases {
        if entry.status != "red" {
            continue;
        }
        match entry.test_file.as_deref().unwrap_or_default() {
            file if file.ends_with(".rs") => red_by_language[0] += 1,
            file if file.ends_with(".ts") => red_by_language[1] += 1,
            file if file.ends_with(".py") => red_by_language[2] += 1,
            _ => {}
        }
    }

    if errors == 0 {
        println!(
            "Manifest verification passed: {}/{} cases synchronized with filesystem.",
            manifest.cases.len(),
            manifest.plan.total_cases
        );
        println!(
            "Derived from disk: {} cases across {} domains; manifest totals agree.",
            disk_total,
            disk_domain_counts.len()
        );
        println!(
            "Case-to-test map verified: {map_implemented} implemented / {map_red} red / {map_gap} gap."
        );
        println!(
            "Red by language: rust={} typescript={} python={}",
            red_by_language[0], red_by_language[1], red_by_language[2]
        );
        if warnings > 0 {
            println!(
                "WARNING: {} case files lack platform_target; validated when present until Part 2 lands.",
                warnings
            );
        }
        if excepted > 0 {
            println!(
                "WARNING: {} law_reference anchors excepted; see crates/kelir-testkit/anchor-exceptions.yaml.",
                excepted
            );
        }
        Ok(())
    } else {
        eprintln!("Manifest check failed with {} errors.", errors);
        std::process::exit(1)
    }
}
