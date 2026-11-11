// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
use kelir_testkit::scenarios::{
    self, ADJACENT_PAIRS, CaseFacts, FAMILY_COUNTS, LANGUAGES, SCENARIO_MANIFEST_SCHEMA,
    SCENARIO_SCHEMA, ScenarioFile,
};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

#[derive(serde::Deserialize)]
struct Manifest {
    plan: Option<ManifestPlan>,
    cases: Vec<ManifestCase>,
    scenario_domains: Option<BTreeMap<String, scenarios::FamilyMeta>>,
}

#[derive(serde::Deserialize)]
struct ManifestPlan {
    total_scenarios: Option<usize>,
    scenario_families: Option<usize>,
}

#[derive(serde::Deserialize)]
struct ManifestCase {
    case_id: Option<String>,
}

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
fn budget_matches(budget: &str) -> bool {
    let Some(rest) = budget.strip_prefix('<') else {
        return false;
    };
    let Some(digits) = rest.strip_suffix('s') else {
        return false;
    };
    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
}

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
fn as_rule_matches(rule: &str) -> bool {
    let Some(rest) = rule.strip_prefix("AS-0") else {
        return false;
    };
    match rest.as_bytes().first() {
        Some(byte) if (b'1'..=b'8').contains(byte) => {}
        _ => return false,
    }
    matches!(&rest[1..], "" | "A" | "B")
}

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
fn render_opt(value: Option<usize>) -> String {
    match value {
        Some(number) => number.to_string(),
        None => "None".to_string(),
    }
}

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
fn py_list<'a>(items: impl IntoIterator<Item = &'a str>) -> String {
    let mut quoted = Vec::new();
    for item in items {
        quoted.push(format!("'{item}'"));
    }
    format!("[{}]", quoted.join(", "))
}

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
fn check_fixture(fixture: &Option<String>, rel: &str, tp_root: &Path, errors: &mut Vec<String>) {
    let Some(fixture) = fixture else {
        return;
    };
    let fixture_dir = tp_root.join(fixture.trim_end_matches('/'));
    if !fixture_dir.is_dir() {
        errors.push(format!("{rel}: fixture_ref dir does not exist: {fixture}"));
    } else if !fixture_dir.join("manifest.yaml").is_file() {
        errors.push(format!("{rel}: fixture dir missing manifest.yaml: {fixture}"));
    }
}

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
fn validate_scenario(
    file: &ScenarioFile,
    cases: &BTreeMap<String, CaseFacts>,
    manifest_ids: &HashSet<String>,
    seen_ids: &mut BTreeMap<String, String>,
    errors: &mut Vec<String>,
) {
    let rel = file.rel.as_str();
    let sid = file.scenario.scenario_id.clone();
    let sid = sid.unwrap_or_else(|| "<missing>".to_string());
    if file.scenario.schema.as_deref() != Some(SCENARIO_SCHEMA) {
        errors.push(format!("{rel}: $schema must be '{SCENARIO_SCHEMA}'"));
    }
    let fields: [(&str, bool); 6] = [
        ("scenario_id", file.scenario.scenario_id.is_some()),
        ("title", file.scenario.title.is_some()),
        ("family", file.scenario.family.is_some()),
        ("target_tier", file.scenario.target_tier.is_some()),
        ("steps", file.scenario.steps.is_some()),
        ("anti_slop_assertions", file.scenario.anti_slop_assertions.is_some()),
    ];
    for (field, present) in fields {
        if !present {
            errors.push(format!("{rel}: missing required field '{field}'"));
        }
    }
    if let Some(previous) = seen_ids.insert(sid.clone(), rel.to_string()) {
        errors.push(format!("{rel}: duplicate scenario_id '{sid}' (also in {previous})"));
    }
    let fam = file.scenario.family.as_deref().unwrap_or("None");
    if scenarios::family_expected(fam).is_none() {
        errors.push(format!("{rel}: unknown family '{fam}'"));
    }
    if file.dir_family != fam {
        errors.push(format!("{rel}: file sits in dir '{}' but family is '{fam}'", file.dir_family));
    }
    if !sid.starts_with(&format!("sc_{fam}_")) {
        errors.push(format!("{rel}: scenario_id '{sid}' must start with 'sc_{fam}_'"));
    }
    if file.scenario.target_tier.as_deref() != Some("e2e") {
        errors.push(format!("{rel}: target_tier must be 'e2e' (CDR-11)"));
    }
    let langs = file.scenario.languages.as_deref().unwrap_or(&[]);
    let langs_ok = !langs.is_empty() && langs.iter().all(|lang| LANGUAGES.contains(&lang.as_str()));
    if !langs_ok {
        let allowed = LANGUAGES.iter().map(|lang| format!("'{lang}'")).collect::<Vec<_>>();
        errors.push(format!(
            "{rel}: languages must be a non-empty subset of [{}]",
            allowed.join(", ")
        ));
    }
    let needs_budget = fam == "smoke";
    if needs_budget && !file.scenario.budget.as_deref().is_some_and(budget_matches) {
        errors.push(format!("{rel}: smoke scenario needs budget like '<10s'/'<15s'"));
    }
    if !needs_budget && file.scenario.budget.is_some() {
        errors.push(format!("{rel}: budget only allowed in smoke family"));
    }
    let steps = file.scenario.steps.as_deref().unwrap_or(&[]);
    if steps.is_empty() {
        errors.push(format!("{rel}: steps must be a non-empty list"));
    }
    for (index, step) in steps.iter().enumerate() {
        let number = index + 1;
        let Some(reference) = step.ref_.as_deref().filter(|r| !r.is_empty()) else {
            errors.push(format!("{rel}: step {number} missing ref"));
            continue;
        };
        let Some(facts) = cases.get(reference) else {
            errors.push(format!(
                "{rel}: step {number} ref '{reference}' does not resolve to a case_id"
            ));
            continue;
        };
        if !manifest_ids.contains(reference) {
            errors.push(format!(
                "{rel}: step {number} ref '{reference}' resolves to a case file but is not in manifest.yaml cases"
            ));
        }
        let Some(assertion) = step.assert.as_deref().filter(|a| !a.is_empty()) else {
            errors.push(format!("{rel}: step {number} missing assert"));
            continue;
        };
        if !facts.statement.contains(assertion)
            && !facts.obs_leaves.iter().any(|leaf| leaf.contains(assertion))
        {
            errors.push(format!(
                "{rel}: step {number} assert is NOT a substring of case statement/expected_observations: '{assertion}'"
            ));
        }
    }
    match &file.scenario.anti_slop_assertions {
        Some(entries) if !entries.is_empty() => {
            for entry in entries {
                let rule = entry.rule.as_deref().unwrap_or_default();
                let verification = entry.verification.as_deref().unwrap_or_default();
                if !as_rule_matches(rule) || verification.is_empty() {
                    errors.push(format!(
                        "{rel}: bad anti_slop_assertions entry: {{'rule': '{rule}', 'verification': '{verification}'}}",
                        rule,
                        verification
                    ));
                }
            }
        }
        _ => errors.push(format!("{rel}: anti_slop_assertions must be a non-empty list")),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut print_coverage = false;
    let mut family: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--scenario-coverage" => print_coverage = true,
            "--family" => {
                index += 1;
                let Some(name) = args.get(index) else {
                    return Err("--family requires a family name argument".into());
                };
                family = Some(name.clone());
            }
            other => {
                let message = format!(
                    "unknown argument: {other}; usage: check-scenarios [--scenario-coverage] [--family <family>]"
                );
                return Err(message.into());
            }
        }
        index += 1;
    }
    if let Some(name) = &family {
        if scenarios::family_expected(name).is_none() {
            let message = format!(
                "unknown family '{name}'; expected one of lifecycle, agent, concurrency, recovery, pressure, network, parity, smoke"
            );
            return Err(message.into());
        }
    }
    let full = family.is_none();

    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .ok_or("Failed to locate workspace root")?
        .to_path_buf();
    let tp_root = repo_root.join("cans/artifacts/test-plan");

    let mut errors: Vec<String> = Vec::new();

    let manifest_path = tp_root.join("manifest.yaml");
    let manifest: Option<Manifest> = match std::fs::read_to_string(&manifest_path) {
        Ok(raw) => match serde_yaml::from_str(&raw) {
            Ok(parsed) => Some(parsed),
            Err(err) => {
                errors.push(format!("manifest: cannot parse manifest.yaml: {err}"));
                None
            }
        },
        Err(err) => {
            errors.push(format!("manifest: cannot parse manifest.yaml: {err}"));
            None
        }
    };
    let mut manifest_ids: HashSet<String> = HashSet::new();
    if let Some(parsed) = &manifest {
        for case in &parsed.cases {
            if let Some(case_id) = &case.case_id {
                manifest_ids.insert(case_id.clone());
            }
        }
    }

    let (cases, case_errors) = scenarios::load_case_facts(&tp_root);
    errors.extend(case_errors);
    let (files, scenario_errors) = scenarios::load_scenario_files(&tp_root);
    errors.extend(scenario_errors);

    let mut seen_ids: BTreeMap<String, String> = BTreeMap::new();
    let mut all_family_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut composition: BTreeMap<String, Vec<&'static str>> = BTreeMap::new();
    let mut coverage: Vec<Vec<String>> = ADJACENT_PAIRS.iter().map(|_| Vec::new()).collect();

    for file in &files {
        let planes = scenarios::planes_of(&file.scenario, &cases);
        let sid = file.scenario.scenario_id.clone();
        let sid = sid.unwrap_or_else(|| "<missing>".to_string());
        composition.insert(sid.clone(), planes.clone());
        for (pair_index, (first, second)) in ADJACENT_PAIRS.iter().enumerate() {
            if planes.contains(first) && planes.contains(second) {
                coverage[pair_index].push(sid.clone());
            }
        }
        if let Some(fam) = file.scenario.family.as_deref() {
            if scenarios::family_expected(fam).is_some() {
                *all_family_counts.entry(fam.to_string()).or_insert(0) += 1;
            }
        }
        let in_scope = match &family {
            Some(target) => target == &file.dir_family,
            None => true,
        };
        if !in_scope {
            continue;
        }
        validate_scenario(file, &cases, &manifest_ids, &mut seen_ids, &mut errors);
        if full {
            check_fixture(&file.scenario.fixture_ref, &file.rel, &tp_root, &mut errors);
        }
    }

    match &family {
        Some(target) => {
            let want = scenarios::family_expected(target).unwrap_or_default();
            let got = files.iter().filter(|file| file.dir_family == *target).count();
            if got != want {
                errors.push(format!("family '{target}': expected {want} scenarios, found {got}"));
            }
        }
        None => {
            for &(fam, want) in FAMILY_COUNTS {
                let got = all_family_counts.get(fam).copied().unwrap_or(0);
                if got == 0 {
                    errors.push(format!("family '{fam}': no scenario files found"));
                } else if got != want {
                    errors.push(format!("family '{fam}': expected {want} scenarios, found {got}"));
                }
            }
        }
    }

    if full {
        check_scenarios_manifest(&tp_root, &seen_ids, &all_family_counts, &mut errors);
        if let Some(parsed) = &manifest {
            check_manifest_domains(parsed, &seen_ids, &all_family_counts, &mut errors);
        }
    }

    for (pair_index, (first, second)) in ADJACENT_PAIRS.iter().enumerate() {
        if coverage[pair_index].is_empty() {
            errors.push(format!(
                "adjacent-plane pair '{first}x{second}' covered by NO scenario (--scenario-coverage gate)"
            ));
        }
    }

    if print_coverage {
        for (sid, planes) in &composition {
            let joined = if planes.is_empty() {
                "—".to_string()
            } else {
                planes.join(", ")
            };
            println!("{sid}: {joined}");
        }
    }
    let mut fam_parts: Vec<String> = Vec::new();
    for &(fam, _) in FAMILY_COUNTS {
        let count = all_family_counts.get(fam).copied().unwrap_or(0);
        fam_parts.push(format!("{fam}={count}"));
    }
    let fam_line = fam_parts.join(", ");
    let total = seen_ids.len();
    if let Some(target) = &family {
        println!("scenarios: {total} in family '{target}' (scoped); all families: {fam_line}");
    } else {
        println!("scenarios: {total} across families: {fam_line}");
    }
    for (pair_index, (first, second)) in ADJACENT_PAIRS.iter().enumerate() {
        let mut names = coverage[pair_index].clone();
        names.sort();
        let head: Vec<String> = names.iter().take(3).cloned().collect();
        let count = coverage[pair_index].len();
        println!("pair {first}x{second}: {count} scenarios ({}...)", head.join(", "));
    }

    if errors.is_empty() {
        if let Some(target) = &family {
            println!("OK: family '{target}' scenarios valid; adjacent-plane pairs covered");
        } else {
            println!("OK: scenario tier fully valid");
        }
        Ok(())
    } else {
        for error in &errors {
            eprintln!("ERR: {error}");
        }
        eprintln!("FAILED: {} error(s)", errors.len());
        std::process::exit(1)
    }
}

// see: cans/artifacts/test-plan/scenarios.yaml#plan
fn check_scenarios_manifest(
    tp_root: &Path,
    seen_ids: &BTreeMap<String, String>,
    all_family_counts: &BTreeMap<String, usize>,
    errors: &mut Vec<String>,
) {
    let sman = match scenarios::load_scenario_manifest(tp_root) {
        Ok(sman) => sman,
        Err(err) => {
            errors.push(err);
            return;
        }
    };
    if sman.schema.as_deref() != Some(SCENARIO_MANIFEST_SCHEMA) {
        errors.push(format!(
            "scenarios.yaml: $schema must be '{SCENARIO_MANIFEST_SCHEMA}'"
        ));
    }
    let total = seen_ids.len();
    if sman.plan.total_scenarios != Some(total) {
        errors.push(format!(
            "scenarios.yaml plan.total_scenarios={} != actual {total}",
            render_opt(sman.plan.total_scenarios)
        ));
    }
    if sman.plan.families != Some(FAMILY_COUNTS.len()) {
        errors.push(format!(
            "scenarios.yaml plan.families={} != {}",
            render_opt(sman.plan.families),
            FAMILY_COUNTS.len()
        ));
    }
    for &(fam, _) in FAMILY_COUNTS {
        let got = sman.families.get(fam).and_then(|meta| meta.scenario_count);
        let actual = all_family_counts.get(fam).copied().unwrap_or(0);
        if got != Some(actual) {
            errors.push(format!(
                "scenarios.yaml families.{fam}.scenario_count={} != actual {actual}",
                render_opt(got)
            ));
        }
    }
    let mut idx: BTreeMap<&str, &scenarios::IndexEntry> = BTreeMap::new();
    for entry in &sman.scenarios {
        idx.insert(entry.scenario_id.as_str(), entry);
    }
    let seen: BTreeSet<&str> = seen_ids.keys().map(String::as_str).collect();
    let indexed: BTreeSet<&str> = idx.keys().copied().collect();
    let missing: Vec<&str> = seen.difference(&indexed).copied().collect();
    let extra: Vec<&str> = indexed.difference(&seen).copied().collect();
    if !missing.is_empty() {
        errors.push(format!("scenarios.yaml missing entries: {}", py_list(missing)));
    }
    if !extra.is_empty() {
        errors.push(format!("scenarios.yaml has unknown scenarios: {}", py_list(extra)));
    }
    for (sid, entry) in &idx {
        if !seen_ids.contains_key(*sid) {
            continue;
        }
        let want_file = format!("scenarios/{}/{sid}.yaml", entry.family);
        if entry.file != want_file {
            errors.push(format!("scenarios.yaml[{sid}]: file '{entry.file}' != '{want_file}'"));
        }
        if entry.target_tier != "e2e" {
            errors.push(format!("scenarios.yaml[{sid}]: target_tier must be e2e"));
        }
    }
}

// see: cans/artifacts/test-plan/manifest.yaml#scenario_domains
fn check_manifest_domains(
    manifest: &Manifest,
    seen_ids: &BTreeMap<String, String>,
    all_family_counts: &BTreeMap<String, usize>,
    errors: &mut Vec<String>,
) {
    for &(fam, _) in FAMILY_COUNTS {
        let mut got: Option<usize> = None;
        if let Some(domains) = &manifest.scenario_domains {
            if let Some(meta) = domains.get(fam) {
                got = meta.scenario_count;
            }
        }
        let actual = all_family_counts.get(fam).copied().unwrap_or(0);
        if got != Some(actual) {
            errors.push(format!(
                "manifest.yaml scenario_domains.{fam}.scenario_count={} != actual {actual}",
                render_opt(got)
            ));
        }
    }
    let Some(plan) = &manifest.plan else {
        return;
    };
    let total = seen_ids.len();
    if let Some(value) = plan.total_scenarios {
        if value != total {
            errors.push(format!(
                "manifest.yaml plan.total_scenarios={} != {total}",
                render_opt(plan.total_scenarios)
            ));
        }
    }
    if let Some(value) = plan.scenario_families {
        if value != FAMILY_COUNTS.len() {
            errors.push(format!(
                "manifest.yaml plan.scenario_families={} != {}",
                render_opt(plan.scenario_families),
                FAMILY_COUNTS.len()
            ));
        }
    }
}
