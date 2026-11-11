use std::fs;
use std::path::{Path, PathBuf};

use kelir_testkit::perf_budgets::{Budgets, ScorecardFloor, validate_scorecard_budgets};
use kelir_testkit::scenario_catalog::ScenarioTier;

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
    prefix: String,
    case_count: usize,
    description: String,
}

#[allow(dead_code)]
#[derive(serde::Deserialize)]
struct CaseEntry {
    case_id: String,
    domain: String,
    target_tier: String,
    quadrant: String,
    title: String,
    file: String,
}

#[derive(serde::Deserialize)]
struct CaseFile {
    platform_target: Option<PlatformTarget>,
}

#[derive(serde::Deserialize)]
struct CoverageEntry {
    status: String,
}

#[derive(serde::Deserialize)]
struct CoverageMap {
    cases: Vec<CoverageEntry>,
}

#[derive(serde::Deserialize)]
struct PlatformTarget {
    os: String,
    arch: String,
    hypervisor: String,
}

impl Default for PlatformTarget {
    fn default() -> Self {
        Self {
            os: "any".to_string(),
            arch: "any".to_string(),
            hypervisor: "none".to_string(),
        }
    }
}

struct PlatformSubset {
    host_os: String,
    host_arch: String,
    host_hypervisors: Vec<String>,
    executable: usize,
    total: usize,
    by_hypervisor: Vec<(String, usize, usize)>,
}

struct MapCounts {
    implemented: usize,
    red: usize,
    gap: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mode = match args.get(1) {
        Some(arg) => arg.as_str(),
        None => "generate",
    };
    let in_place = args.iter().skip(2).any(|arg| arg == "--in-place");
    if in_place && mode != "generate" {
        return Err("--in-place applies to `generate` only.".into());
    }

    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .ok_or("Failed to locate workspace root")?
        .to_path_buf();

    let manifest_path = repo_root.join("cans/artifacts/test-plan/manifest.yaml");
    let scorecard_path = repo_root.join("cans/artifacts/scorecard.md");

    let manifest_raw = fs::read_to_string(&manifest_path)?;
    let manifest: Manifest = serde_yaml::from_str(&manifest_raw)?;

    let budgets_raw = fs::read_to_string(repo_root.join("perf/budgets.toml"))?;
    let budgets: Budgets = toml::from_str(&budgets_raw)?;
    if !budgets.enforce {
        return Err(
            "perf/budgets.toml has enforce = false; AS-04B requires enforced budgets.".into(),
        );
    }
    validate_scorecard_budgets(&budgets)?;

    let subset = platform_subset(&repo_root, &manifest)?;

    let map_raw =
        fs::read_to_string(repo_root.join("cans/artifacts/test-plan/case-to-test-map.yaml"))?;
    let map: CoverageMap = serde_yaml::from_str(&map_raw)?;
    let counts = MapCounts {
        implemented: map
            .cases
            .iter()
            .filter(|c| c.status == "implemented")
            .count(),
        red: map.cases.iter().filter(|c| c.status == "red").count(),
        gap: map.cases.iter().filter(|c| c.status == "gap").count(),
    };

    let tier = kelir_testkit::scenario_catalog::load_scenario_tier(
        &repo_root.join("cans/artifacts/test-plan"),
    )?;

    let generated_md = render_scorecard(&manifest, &budgets, &subset, &counts, &tier);

    match mode {
        "generate" => {
            let out_path = if in_place {
                scorecard_path
            } else {
                repo_root.join("target/scorecard.md")
            };
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&out_path, &generated_md)?;
            if in_place {
                println!(
                    "Scorecard artifact updated in place: {} (gate with `scorecard -- check`).",
                    out_path.display()
                );
            } else {
                println!(
                    "Scorecard rendered to {} (cans/artifacts/scorecard.md stays frozen; `scorecard -- check` gates the committed artifact; pass --in-place to update it).",
                    out_path.display()
                );
            }
            Ok(())
        }
        "check" => {
            let existing_md = fs::read_to_string(&scorecard_path).unwrap_or_default();
            if existing_md.trim() == generated_md.trim() {
                println!("Scorecard verification passed: physical reality matches scorecard.md.");
                Ok(())
            } else {
                eprintln!(
                    "ERROR: Scorecard drift detected. cans/artifacts/scorecard.md does not match manifest truth."
                );
                eprintln!("Run 'just scorecard' to synchronize evidence.");
                std::process::exit(1)
            }
        }
        unknown => {
            eprintln!(
                "Unknown mode: {}. Use 'generate [--in-place]' or 'check'.",
                unknown
            );
            std::process::exit(1)
        }
    }
}

// see: cans/15-operations.md#Coverage-gates
fn floor_metric(
    floor: &ScorecardFloor,
    latencies: &std::collections::BTreeMap<String, u64>,
) -> String {
    if let Some(target) = &floor.target {
        return target.clone();
    }
    let value = floor.value.or_else(|| {
        floor
            .budget
            .as_ref()
            .and_then(|key| latencies.get(key).copied())
    });
    match value {
        Some(value) => format!(
            "≤ {value} {} p95",
            floor.unit.as_deref().unwrap_or_default()
        ),
        None => String::new(),
    }
}

fn render_scorecard(
    manifest: &Manifest,
    budgets: &Budgets,
    subset: &PlatformSubset,
    counts: &MapCounts,
    tier: &ScenarioTier,
) -> String {
    let mut out = String::new();
    out.push_str("# Kelir Platform Scorecard\n\n");
    out.push_str(
        "Compiled by `kelir-testkit scorecard` from `cans/artifacts/test-plan/manifest.yaml` + `scenarios.yaml`.\n",
    );
    out.push_str("Do not edit by hand: CI runs `just check-scorecard`.\n\n");

    out.push_str("## Invariant Test Inventory\n\n");
    out.push_str("| Total Invariants | Domains | Integration Tier | Unit Tier | E2E Tier |\n");
    out.push_str("|---:|---:|---:|---:|---:|\n");

    let integration_count = manifest
        .cases
        .iter()
        .filter(|c| c.target_tier == "integration")
        .count();
    let unit_count = manifest
        .cases
        .iter()
        .filter(|c| c.target_tier == "unit")
        .count();
    let e2e_count = manifest
        .cases
        .iter()
        .filter(|c| c.target_tier == "e2e")
        .count();

    out.push_str(&format!(
        "| {} | {} | {} | {} | {} |\n\n",
        manifest.plan.total_cases, manifest.plan.domains, integration_count, unit_count, e2e_count
    ));

    out.push_str("## Domain Coverage Matrix\n\n");
    out.push_str("| Domain | Prefix | Cases | Status | Scope Description |\n");
    out.push_str("|---|---|---:|---|---|\n");

    for (name, domain) in &manifest.domains {
        let domain_cases = manifest.cases.iter().filter(|c| &c.domain == name).count();
        let status = if domain_cases == domain.case_count {
            "pass"
        } else {
            "drift"
        };
        out.push_str(&format!(
            "| `{}` | `{}` | {} | {} | {} |\n",
            name, domain.prefix, domain_cases, status, domain.description
        ));
    }

    out.push_str("\n## Scenario Tier\n\n");
    out.push_str(
        "Composition scenarios from `cans/artifacts/test-plan/scenarios.yaml`;\neach step references a domain case_id (non-redundancy contract, CDR-09..CDR-12).\n\n",
    );
    out.push_str("| Scenario Tier | Count | Status |\n");
    out.push_str("|---|---:|---|\n");
    for ((family, actual), &(_, expected)) in tier
        .family_counts
        .iter()
        .zip(kelir_testkit::scenario_catalog::FAMILY_COUNTS)
    {
        let status = if *actual == expected { "pass" } else { "drift" };
        out.push_str(&format!("| {family} | {actual} | {status} |\n"));
    }

    out.push_str("\n## Scenario Coverage Composition\n\n");
    out.push_str(
        "Per-scenario plane composition (domain → plane map; adjacent pairs enforced by check-scenarios).\n\n",
    );
    out.push_str("| Scenario | Planes Composed |\n");
    out.push_str("|---|---|\n");
    for (sid, planes) in &tier.composition {
        let joined = if planes.is_empty() {
            "—".to_string()
        } else {
            planes.join(", ")
        };
        out.push_str(&format!("| {sid} | {joined} |\n"));
    }

    out.push_str("\n## Test Plan Coverage\n\n");
    out.push_str(
        "Status counts from `cans/artifacts/test-plan/case-to-test-map.yaml`;\nstate machine is gap → red → implemented, one direction only.\n\n",
    );
    out.push_str("| Implemented | Red | Gap |\n");
    out.push_str("|---:|---:|---:|\n");
    out.push_str(&format!(
        "| {} | {} | {} |\n",
        counts.implemented, counts.red, counts.gap
    ));

    out.push_str("\n## Hypervisor Performance Floor (Budgets)\n\n");
    out.push_str("Thresholds below come from perf/budgets.toml (enforce = true) where a key is declared; rows without a declared key keep their spec-fixed floor.\n\n");
    out.push_str("| Scenario | Target Metric | Enforcement | Status |\n");
    out.push_str("|---|---|---|---|\n");
    for floor in &budgets.scorecard.floors {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            floor.scenario,
            floor_metric(floor, &budgets.latencies),
            floor.enforcement,
            floor.status
        ));
    }

    out.push_str("\n## Platform Executable Subset\n\n");
    out.push_str(&format!(
        "Host: {}/{}; hypervisors present: {}. Executable on this host: {} / {} cases.\n\n",
        subset.host_os,
        subset.host_arch,
        if subset.host_hypervisors.is_empty() {
            "none".to_string()
        } else {
            subset.host_hypervisors.join(", ")
        },
        subset.executable,
        subset.total
    ));
    out.push_str("| Hypervisor | Cases | Executable Here |\n");
    out.push_str("|---|---:|---:|\n");
    for (hypervisor, total, executable) in &subset.by_hypervisor {
        out.push_str(&format!(
            "| `{}` | {} | {} |\n",
            hypervisor, total, executable
        ));
    }

    out.push_str("\n## Anti-Slop Enforcement\n\n");
    out.push_str("| Law | Rule Enforced | Enforcement Mechanism | Status |\n");
    out.push_str("|---|---|---|---|\n");
    for rule in &budgets.scorecard.rules {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            rule.id, rule.rule, rule.mechanism, rule.status
        ));
    }

    out
}

// see: cans/01-architecture.md#Driver-backends
fn host_hypervisors() -> Vec<String> {
    driver_rustvmm::host_probe::available_hypervisors(std::env::consts::OS)
        .into_iter()
        .map(String::from)
        .collect()
}

fn case_is_executable(
    target: &PlatformTarget,
    host_os: &str,
    host_arch: &str,
    host_hypervisors: &[String],
) -> bool {
    (target.os == "any" || target.os == host_os)
        && (target.arch == "any" || target.arch == host_arch)
        && (target.hypervisor == "none"
            || target.hypervisor == "emulated"
            || host_hypervisors.iter().any(|h| h == &target.hypervisor))
}

fn platform_subset(
    repo_root: &Path,
    manifest: &Manifest,
) -> Result<PlatformSubset, Box<dyn std::error::Error>> {
    let host_os = std::env::consts::OS.to_string();
    let host_arch = std::env::consts::ARCH.to_string();
    let host_hypervisors = host_hypervisors();
    let mut executable = 0;
    let mut grouped: std::collections::BTreeMap<String, (usize, usize)> =
        std::collections::BTreeMap::new();
    for case in &manifest.cases {
        let case_path = repo_root.join("cans/artifacts/test-plan").join(&case.file);
        let case_raw = fs::read_to_string(&case_path)?;
        let case_file: CaseFile = serde_yaml::from_str(&case_raw)?;
        let target = case_file.platform_target.unwrap_or_default();
        let runnable = case_is_executable(&target, &host_os, &host_arch, &host_hypervisors);
        if runnable {
            executable += 1;
        }
        let entry = grouped.entry(target.hypervisor).or_insert((0, 0));
        entry.0 += 1;
        if runnable {
            entry.1 += 1;
        }
    }
    Ok(PlatformSubset {
        host_os,
        host_arch,
        host_hypervisors,
        executable,
        total: manifest.cases.len(),
        by_hypervisor: grouped
            .into_iter()
            .map(|(hypervisor, (total, runnable))| (hypervisor, total, runnable))
            .collect(),
    })
}
