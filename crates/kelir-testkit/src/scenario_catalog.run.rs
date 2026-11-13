// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
pub const SCENARIO_SCHEMA: &str = "test-plan/scenario/v1";

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
pub const SCENARIO_MANIFEST_SCHEMA: &str = "kelir-test-plan/scenario-manifest/v1";

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
pub const LANGUAGES: &[&str] = &["python", "rust", "typescript"];

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
pub const PLANE_MAP: &[(&str, &str)] = &[
    ("terminal_plane", "terminal"),
    ("storage_plane", "storage"),
    ("wire_plane", "wire"),
    ("lifecycle_checkpoint", "checkpoint"),
    ("hypervisor_driver", "host"),
];

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
pub const PLANE_ORDER: &[&str] = &["terminal", "storage", "wire", "checkpoint", "host"];

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
pub const ADJACENT_PAIRS: &[(&str, &str)] = &[
    ("terminal", "storage"),
    ("storage", "wire"),
    ("wire", "checkpoint"),
    ("checkpoint", "host"),
];

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
pub const FAMILY_COUNTS: &[(&str, usize)] = &[
    ("lifecycle", 5),
    ("agent", 5),
    ("concurrency", 5),
    ("recovery", 4),
    ("pressure", 4),
    ("network", 4),
    ("parity", 4),
    ("smoke", 4),
];

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
pub fn plane_of(domain: &str) -> Option<&'static str> {
    PLANE_MAP
        .iter()
        .find(|(dom, _)| *dom == domain)
        .map(|(_, plane)| *plane)
}

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
pub fn family_expected(family: &str) -> Option<usize> {
    FAMILY_COUNTS
        .iter()
        .find(|(fam, _)| *fam == family)
        .map(|(_, count)| *count)
}

// see: cans/artifacts/test-plan/scenarios.yaml#scenarios
#[derive(serde::Deserialize)]
pub struct Step {
    #[serde(rename = "ref")]
    pub ref_: Option<String>,
    pub assert: Option<String>,
}

// see: cans/artifacts/test-plan/scenarios.yaml#scenarios
#[derive(serde::Deserialize)]
pub struct AntiSlop {
    pub rule: Option<String>,
    pub verification: Option<String>,
}

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
#[derive(serde::Deserialize)]
pub struct Scenario {
    #[serde(rename = "$schema")]
    pub schema: Option<String>,
    pub scenario_id: Option<String>,
    pub title: Option<String>,
    pub family: Option<String>,
    pub target_tier: Option<String>,
    pub languages: Option<Vec<String>>,
    pub budget: Option<String>,
    pub fixture_ref: Option<String>,
    pub steps: Option<Vec<Step>>,
    pub anti_slop_assertions: Option<Vec<AntiSlop>>,
}

// see: cans/artifacts/test-plan/scenarios.yaml#families
#[derive(serde::Deserialize)]
pub struct FamilyMeta {
    pub scenario_count: Option<usize>,
}

// see: cans/artifacts/test-plan/scenarios.yaml#plan
#[derive(serde::Deserialize)]
pub struct ScenarioPlan {
    pub total_scenarios: Option<usize>,
    pub families: Option<usize>,
}

// see: cans/artifacts/test-plan/scenarios.yaml#scenarios
#[derive(serde::Deserialize)]
pub struct IndexEntry {
    pub scenario_id: String,
    pub file: String,
    pub family: String,
    pub target_tier: String,
    pub title: String,
}

// see: cans/artifacts/test-plan/scenarios.yaml#plan
#[derive(serde::Deserialize)]
pub struct ScenarioManifest {
    #[serde(rename = "$schema")]
    pub schema: Option<String>,
    pub plan: ScenarioPlan,
    pub families: BTreeMap<String, FamilyMeta>,
    pub scenarios: Vec<IndexEntry>,
}

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
pub struct ScenarioFile {
    pub rel: String,
    pub dir_family: String,
    pub scenario: Scenario,
}

// see: cans/artifacts/test-plan/manifest.yaml#cases
pub struct CaseFacts {
    pub domain: String,
    pub statement: String,
    pub obs_leaves: Vec<String>,
}

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
pub struct ScenarioTier {
    pub family_counts: Vec<(&'static str, usize)>,
    pub composition: Vec<(String, Vec<&'static str>)>,
}

#[derive(serde::Deserialize)]
struct CaseInvariant {
    statement: Option<String>,
}

#[derive(serde::Deserialize)]
struct CaseRaw {
    case_id: Option<String>,
    domain: Option<String>,
    invariant: Option<CaseInvariant>,
    expected_observations: Option<serde_yaml::Value>,
}

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
pub fn scalar_str(value: &serde_yaml::Value) -> String {
    match value {
        serde_yaml::Value::String(text) => text.clone(),
        serde_yaml::Value::Bool(true) => "true".to_string(),
        serde_yaml::Value::Bool(false) => "false".to_string(),
        serde_yaml::Value::Number(number) => number.to_string(),
        other => format!("{other:?}"),
    }
}

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
pub fn obs_leaves(node: &serde_yaml::Value, out: &mut Vec<String>) {
    match node {
        serde_yaml::Value::Mapping(map) => {
            for (_key, value) in map.iter() {
                obs_leaves(value, out);
            }
        }
        serde_yaml::Value::Sequence(seq) => {
            for value in seq {
                obs_leaves(value, out);
            }
        }
        serde_yaml::Value::Null => {}
        other => out.push(scalar_str(other)),
    }
}

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
pub fn load_case_facts(tp_root: &Path) -> (BTreeMap<String, CaseFacts>, Vec<String>) {
    let cases_dir = tp_root.join("cases");
    let mut rels: Vec<String> = Vec::new();
    for entry in WalkDir::new(&cases_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.depth() != 2 || entry.path().extension().is_none_or(|ext| ext != "yaml") {
            continue;
        }
        match entry.path().strip_prefix(&cases_dir) {
            Ok(rel) => rels.push(format!("cases/{}", rel.to_string_lossy())),
            Err(_) => continue,
        }
    }
    rels.sort();
    let mut cases = BTreeMap::new();
    let mut errors = Vec::new();
    for rel in rels {
        let raw = match fs::read_to_string(tp_root.join(&rel)) {
            Ok(raw) => raw,
            Err(err) => {
                errors.push(format!("case: cannot parse {rel}: {err}"));
                continue;
            }
        };
        let parsed: CaseRaw = match serde_yaml::from_str(&raw) {
            Ok(parsed) => parsed,
            Err(err) => {
                errors.push(format!("case: cannot parse {rel}: {err}"));
                continue;
            }
        };
        let Some(case_id) = parsed.case_id.filter(|id| !id.is_empty()) else {
            continue;
        };
        let mut leaves = Vec::new();
        if let Some(observations) = &parsed.expected_observations {
            obs_leaves(observations, &mut leaves);
        }
        let mut statement = String::new();
        if let Some(invariant) = &parsed.invariant {
            if let Some(text) = &invariant.statement {
                statement = text.clone();
            }
        }
        let facts = CaseFacts {
            domain: parsed.domain.unwrap_or_default(),
            statement,
            obs_leaves: leaves,
        };
        cases.insert(case_id, facts);
    }
    (cases, errors)
}

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
pub fn load_scenario_files(tp_root: &Path) -> (Vec<ScenarioFile>, Vec<String>) {
    let scenarios_dir = tp_root.join("scenarios");
    let mut rels: Vec<String> = Vec::new();
    for entry in WalkDir::new(&scenarios_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.depth() != 2 || entry.path().extension().is_none_or(|ext| ext != "yaml") {
            continue;
        }
        match entry.path().strip_prefix(&scenarios_dir) {
            Ok(rel) => rels.push(format!("scenarios/{}", rel.to_string_lossy())),
            Err(_) => continue,
        }
    }
    rels.sort();
    let mut files = Vec::new();
    let mut errors = Vec::new();
    for rel in rels {
        let raw = match fs::read_to_string(tp_root.join(&rel)) {
            Ok(raw) => raw,
            Err(err) => {
                errors.push(format!("scenario: cannot parse {rel}: {err}"));
                continue;
            }
        };
        let scenario: Scenario = match serde_yaml::from_str(&raw) {
            Ok(scenario) => scenario,
            Err(err) => {
                errors.push(format!("scenario: cannot parse {rel}: {err}"));
                continue;
            }
        };
        let dir_family = rel.split('/').nth(1).unwrap_or_default().to_string();
        files.push(ScenarioFile {
            rel,
            dir_family,
            scenario,
        });
    }
    (files, errors)
}

// see: cans/artifacts/test-plan/scenarios.yaml#plan
pub fn load_scenario_manifest(tp_root: &Path) -> Result<ScenarioManifest, String> {
    let path = tp_root.join("scenarios.yaml");
    let raw = fs::read_to_string(&path)
        .map_err(|err| format!("scenarios.yaml: cannot parse {}: {err}", path.display()))?;
    serde_yaml::from_str(&raw)
        .map_err(|err| format!("scenarios.yaml: cannot parse {}: {err}", path.display()))
}

// see: cans/artifacts/test-plan/matrix.yaml#ci_gates
pub fn planes_of(scenario: &Scenario, cases: &BTreeMap<String, CaseFacts>) -> Vec<&'static str> {
    let mut found: Vec<&'static str> = Vec::new();
    let Some(steps) = &scenario.steps else {
        return found;
    };
    for step in steps {
        let Some(ref_) = step.ref_.as_deref().filter(|r| !r.is_empty()) else {
            continue;
        };
        let Some(facts) = cases.get(ref_) else {
            continue;
        };
        if let Some(plane) = plane_of(&facts.domain) {
            if !found.contains(&plane) {
                found.push(plane);
            }
        }
    }
    PLANE_ORDER
        .iter()
        .filter(|plane| found.contains(plane))
        .copied()
        .collect()
}

// see: cans/artifacts/test-plan/scenarios.yaml#scenarios
pub fn load_scenario_tier(tp_root: &Path) -> Result<ScenarioTier, String> {
    let manifest = load_scenario_manifest(tp_root)?;
    let (files, errors) = load_scenario_files(tp_root);
    if let Some(first) = errors.first() {
        return Err(first.clone());
    }
    let (cases, case_errors) = load_case_facts(tp_root);
    if let Some(first) = case_errors.first() {
        return Err(first.clone());
    }
    let mut family_counts = Vec::new();
    for &(family, _) in FAMILY_COUNTS {
        let mut count = 0;
        for file in &files {
            if file.scenario.family.as_deref() == Some(family) {
                count += 1;
            }
        }
        family_counts.push((family, count));
    }
    let mut by_id: BTreeMap<&str, &ScenarioFile> = BTreeMap::new();
    for file in &files {
        if let Some(sid) = file
            .scenario
            .scenario_id
            .as_deref()
            .filter(|s| !s.is_empty())
        {
            by_id.insert(sid, file);
        }
    }
    let mut composition = Vec::new();
    for entry in &manifest.scenarios {
        let Some(file) = by_id.get(entry.scenario_id.as_str()) else {
            return Err(format!(
                "scenarios.yaml[{}]: no scenario file on disk",
                entry.scenario_id
            ));
        };
        composition.push((entry.scenario_id.clone(), planes_of(&file.scenario, &cases)));
    }
    Ok(ScenarioTier {
        family_counts,
        composition,
    })
}
