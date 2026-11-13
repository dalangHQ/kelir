// see: cans/15-operations.md#--unified-builder-coverage
// see: cans/10-attestation.md#Execution-receipts

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use kelir_interface::telemetry::{ExecutionReceipt, ExitKind};
use serde::Serialize;

const SCENARIOS_INDEX: &str = "scenarios.yaml";
const CASE_MAP: &str = "case-to-test-map.yaml";

// see: cans/15-operations.md#--unified-builder-coverage
#[derive(Debug, Clone, Serialize)]
pub struct StepDescriptor {
    pub case_id: String,
    pub assert_text: String,
}

// see: cans/15-operations.md#--unified-builder-coverage
#[derive(Debug, Clone, Serialize)]
pub struct Scenario {
    pub scenario_id: String,
    pub file: String,
    pub family: String,
    pub target_tier: String,
    pub title: String,
    pub steps: Vec<StepDescriptor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseStatus {
    Implemented,
    Red,
    Gap,
}

// see: cans/15-operations.md#--unified-builder-coverage
#[derive(Debug, Clone)]
pub struct CaseEntry {
    pub case_id: String,
    pub case_file: String,
    pub target_crate: String,
    pub target_tier: String,
    pub status: CaseStatus,
    pub test_file: Option<String>,
    pub test_fn: Option<String>,
}

// see: cans/10-attestation.md#Execution-receipts
#[derive(Debug, Clone, Serialize)]
pub struct ExpectedReceipt {
    pub exit_code: i32,
    pub exit_kind: String,
    pub fault_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum StepOutcome {
    Pass,
    Fail,
    NotExecutable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScenarioOutcome {
    Pass,
    Fail,
    NotExecutable,
}

// see: cans/15-operations.md#--unified-builder-coverage
#[derive(Debug, Clone, Serialize)]
pub struct StepRecord {
    pub case_id: String,
    pub outcome: StepOutcome,
    pub reason: String,
    pub owner_test_file: Option<String>,
    pub owner_test_fn: Option<String>,
    pub expected_exit_code: Option<i32>,
    pub expected_exit_kind: Option<String>,
    pub observed_exit_code: Option<i32>,
    pub observed_exit_kind: Option<String>,
    pub diff: Vec<String>,
}

// see: cans/15-operations.md#--unified-builder-coverage
#[derive(Debug, Clone, Serialize)]
pub struct ScenarioRecord {
    pub scenario_id: String,
    pub family: String,
    pub outcome: ScenarioOutcome,
    pub reason: String,
    pub steps: Vec<StepRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub summary: bool,
    pub scenarios: usize,
    pub pass: usize,
    pub fail: usize,
    pub not_executable: usize,
    pub steps: usize,
    pub steps_executable: usize,
    pub steps_not_executable: usize,
    pub steps_pass: usize,
    pub steps_fail: usize,
}

pub struct Report {
    pub records: Vec<ScenarioRecord>,
    pub summary: Summary,
}

// see: cans/15-operations.md#--unified-builder-coverage
pub struct ExampleRegistry {
    owners: BTreeMap<String, String>,
}

impl ExampleRegistry {
    pub fn new() -> Self {
        Self {
            owners: BTreeMap::new(),
        }
    }

    pub fn register(&mut self, case_id: &str, example: &str) {
        self.owners.insert(case_id.to_string(), example.to_string());
    }

    pub fn find_owner(&self, case_id: &str) -> Option<&str> {
        self.owners.get(case_id).map(String::as_str)
    }
}

fn unquote(value: &str) -> &str {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

fn opt_string(value: &str) -> Option<String> {
    let value = value.trim();
    if value == "null" {
        None
    } else {
        Some(unquote(value).to_string())
    }
}

fn parse_scenario_steps(text: &str) -> Vec<StepDescriptor> {
    let mut steps: Vec<StepDescriptor> = Vec::new();
    let mut current: Option<StepDescriptor> = None;
    let mut in_steps = false;
    for line in text.lines() {
        if line.starts_with("steps:") {
            in_steps = true;
            continue;
        }
        if in_steps && !line.starts_with(' ') && !line.is_empty() {
            in_steps = false;
        }
        if !in_steps {
            continue;
        }
        if let Some(value) = line.strip_prefix("  - ref: ") {
            if let Some(done) = current.take() {
                steps.push(done);
            }
            current = Some(StepDescriptor {
                case_id: unquote(value).to_string(),
                assert_text: String::new(),
            });
        } else if let Some(value) = line.strip_prefix("    assert: ") {
            if let Some(step) = current.as_mut() {
                step.assert_text = unquote(value).to_string();
            }
        }
    }
    if let Some(done) = current.take() {
        steps.push(done);
    }
    steps
}

fn load_scenarios(plan_dir: &Path) -> Result<Vec<Scenario>, String> {
    let path = plan_dir.join(SCENARIOS_INDEX);
    let text = fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let mut scenarios: Vec<Scenario> = Vec::new();
    let mut current: Option<Scenario> = None;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("  - scenario_id: ") {
            if let Some(done) = current.take() {
                scenarios.push(done);
            }
            current = Some(Scenario {
                scenario_id: unquote(value).to_string(),
                file: String::new(),
                family: String::new(),
                target_tier: String::new(),
                title: String::new(),
                steps: Vec::new(),
            });
        } else if let Some(value) = line.strip_prefix("    file: ") {
            if let Some(scenario) = current.as_mut() {
                scenario.file = unquote(value).to_string();
            }
        } else if let Some(value) = line.strip_prefix("    family: ") {
            if let Some(scenario) = current.as_mut() {
                scenario.family = unquote(value).to_string();
            }
        } else if let Some(value) = line.strip_prefix("    target_tier: ") {
            if let Some(scenario) = current.as_mut() {
                scenario.target_tier = unquote(value).to_string();
            }
        } else if let Some(value) = line.strip_prefix("    title: ") {
            if let Some(scenario) = current.as_mut() {
                scenario.title = unquote(value).to_string();
            }
        }
    }
    if let Some(done) = current.take() {
        scenarios.push(done);
    }
    for scenario in &mut scenarios {
        let path = plan_dir.join(&scenario.file);
        let file_text =
            fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        scenario.steps = parse_scenario_steps(&file_text);
    }
    Ok(scenarios)
}

fn case_status(value: &str) -> CaseStatus {
    match value.trim() {
        "implemented" => CaseStatus::Implemented,
        "red" => CaseStatus::Red,
        _ => CaseStatus::Gap,
    }
}

fn load_case_map(plan_dir: &Path) -> Result<BTreeMap<String, CaseEntry>, String> {
    let path = plan_dir.join(CASE_MAP);
    let text = fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let mut map: BTreeMap<String, CaseEntry> = BTreeMap::new();
    let mut current: Option<CaseEntry> = None;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("- case_id: ") {
            if let Some(done) = current.take() {
                map.insert(done.case_id.clone(), done);
            }
            current = Some(CaseEntry {
                case_id: value.trim().to_string(),
                case_file: String::new(),
                target_crate: String::new(),
                target_tier: String::new(),
                status: CaseStatus::Gap,
                test_file: None,
                test_fn: None,
            });
        } else if let Some(value) = line.strip_prefix("  case_file: ") {
            if let Some(entry) = current.as_mut() {
                entry.case_file = unquote(value).to_string();
            }
        } else if let Some(value) = line.strip_prefix("  target_crate: ") {
            if let Some(entry) = current.as_mut() {
                entry.target_crate = unquote(value).to_string();
            }
        } else if let Some(value) = line.strip_prefix("  target_tier: ") {
            if let Some(entry) = current.as_mut() {
                entry.target_tier = unquote(value).to_string();
            }
        } else if let Some(value) = line.strip_prefix("  status: ") {
            if let Some(entry) = current.as_mut() {
                entry.status = case_status(value);
            }
        } else if let Some(value) = line.strip_prefix("  test_file: ") {
            if let Some(entry) = current.as_mut() {
                entry.test_file = opt_string(value);
            }
        } else if let Some(value) = line.strip_prefix("  test_fn: ") {
            if let Some(entry) = current.as_mut() {
                entry.test_fn = opt_string(value);
            }
        }
    }
    if let Some(done) = current.take() {
        map.insert(done.case_id.clone(), done);
    }
    Ok(map)
}

fn load_expected_receipt(plan_dir: &Path, entry: &CaseEntry) -> Option<ExpectedReceipt> {
    let text = fs::read_to_string(plan_dir.join(&entry.case_file)).ok()?;
    let mut in_expected = false;
    let mut in_receipt = false;
    let mut exit_code: Option<i32> = None;
    let mut exit_kind: Option<String> = None;
    let mut fault_reason: Option<String> = None;
    for line in text.lines() {
        if line.starts_with("expected_observations:") {
            in_expected = true;
            in_receipt = false;
            continue;
        }
        if in_expected && !line.starts_with(' ') && !line.is_empty() {
            break;
        }
        if !in_expected {
            continue;
        }
        if line.starts_with("  receipt:") {
            in_receipt = true;
            continue;
        }
        if line.starts_with("  ") && !line.starts_with("    ") {
            in_receipt = false;
        }
        if let Some(value) = line.strip_prefix("  exit_code: ") {
            exit_code = value.trim().parse::<i32>().ok();
        } else if in_receipt {
            if let Some(value) = line.strip_prefix("    exit_kind: ") {
                exit_kind = Some(unquote(value).to_string());
            } else if let Some(value) = line.strip_prefix("    fault_reason: ") {
                let value = value.trim();
                fault_reason = if value == "null" {
                    None
                } else {
                    Some(unquote(value).to_string())
                };
            }
        }
    }
    match (exit_code, exit_kind) {
        (Some(code), Some(kind)) => Some(ExpectedReceipt {
            exit_code: code,
            exit_kind: kind,
            fault_reason,
        }),
        _ => None,
    }
}

fn not_executable_record(
    step: &StepDescriptor,
    entry: Option<&CaseEntry>,
    expected: &Option<ExpectedReceipt>,
    reason: &str,
) -> StepRecord {
    let (expected_exit_code, expected_exit_kind) = match expected {
        Some(e) => (Some(e.exit_code), Some(e.exit_kind.clone())),
        None => (None, None),
    };
    let (owner_test_file, owner_test_fn) = match entry {
        Some(e) => (e.test_file.clone(), e.test_fn.clone()),
        None => (None, None),
    };
    StepRecord {
        case_id: step.case_id.clone(),
        outcome: StepOutcome::NotExecutable,
        reason: reason.to_string(),
        owner_test_file,
        owner_test_fn,
        expected_exit_code,
        expected_exit_kind,
        observed_exit_code: None,
        observed_exit_kind: None,
        diff: Vec::new(),
    }
}

// TODO (L3): invoke the owning example's run_scenario(step) and return its ExecutionReceipt.
fn invoke_example(
    _registry: &ExampleRegistry,
    _case_id: &str,
    _step: &StepDescriptor,
) -> Option<ExecutionReceipt> {
    None
}

// see: cans/10-attestation.md#Execution-receipts
fn exit_kind_name(kind: &ExitKind) -> &'static str {
    match kind {
        ExitKind::Normal => "Normal",
        ExitKind::Signal(_) => "Signal",
        ExitKind::KernelPanic(_) => "KernelPanic",
        ExitKind::HypervisorFault(_) => "HypervisorFault",
        ExitKind::DirtyShutdown => "DirtyShutdown",
        ExitKind::HardKilled => "HardKilled",
    }
}

// see: cans/10-attestation.md#Execution-receipts
fn diff_receipt(observed: &ExecutionReceipt, expected: &ExpectedReceipt) -> Vec<String> {
    let mut mismatches: Vec<String> = Vec::new();
    if observed.exit_code != expected.exit_code {
        mismatches.push("exit_code".to_string());
    }
    if exit_kind_name(&observed.exit_kind) != expected.exit_kind {
        mismatches.push("exit_kind".to_string());
    }
    mismatches
}

fn resolve_step(
    step: &StepDescriptor,
    cases: &BTreeMap<String, CaseEntry>,
    registry: &ExampleRegistry,
    plan_dir: &Path,
) -> StepRecord {
    let Some(entry) = cases.get(&step.case_id) else {
        return not_executable_record(
            step,
            None,
            &None,
            "case_id not present in case-to-test-map.yaml",
        );
    };
    let expected = load_expected_receipt(plan_dir, entry);
    match entry.status {
        CaseStatus::Gap => not_executable_record(
            step,
            Some(entry),
            &expected,
            "case is a verified gap: no live boundary exists",
        ),
        CaseStatus::Red => not_executable_record(
            step,
            Some(entry),
            &expected,
            "case is L3-red: test exists but is ignored-only, no executable path",
        ),
        CaseStatus::Implemented => {
            if registry.find_owner(&step.case_id).is_none() {
                return not_executable_record(
                    step,
                    Some(entry),
                    &expected,
                    "case is implemented but no example exports run_scenario",
                );
            }
            match invoke_example(registry, &step.case_id, step) {
                None => not_executable_record(
                    step,
                    Some(entry),
                    &expected,
                    "owning example has no run_scenario entry point",
                ),
                Some(observed) => {
                    let Some(expected) = &expected else {
                        return not_executable_record(
                            step,
                            Some(entry),
                            &None,
                            "case file pins no expected receipt",
                        );
                    };
                    let diff = diff_receipt(&observed, expected);
                    let outcome = if diff.is_empty() {
                        StepOutcome::Pass
                    } else {
                        StepOutcome::Fail
                    };
                    StepRecord {
                        case_id: step.case_id.clone(),
                        outcome,
                        reason: if diff.is_empty() {
                            "observed receipt matches the pinned expectation".to_string()
                        } else {
                            format!("observed receipt differs on: {}", diff.join(", "))
                        },
                        owner_test_file: entry.test_file.clone(),
                        owner_test_fn: entry.test_fn.clone(),
                        expected_exit_code: Some(expected.exit_code),
                        expected_exit_kind: Some(expected.exit_kind.clone()),
                        observed_exit_code: Some(observed.exit_code),
                        observed_exit_kind: Some(exit_kind_name(&observed.exit_kind).to_string()),
                        diff,
                    }
                }
            }
        }
    }
}

fn run_scenario(
    scenario: &Scenario,
    cases: &BTreeMap<String, CaseEntry>,
    registry: &ExampleRegistry,
    plan_dir: &Path,
) -> ScenarioRecord {
    let mut steps: Vec<StepRecord> = Vec::new();
    let mut pass = 0;
    let mut fail = 0;
    let mut not_executable = 0;
    for step in &scenario.steps {
        let record = resolve_step(step, cases, registry, plan_dir);
        match record.outcome {
            StepOutcome::Pass => pass += 1,
            StepOutcome::Fail => fail += 1,
            StepOutcome::NotExecutable => not_executable += 1,
        }
        steps.push(record);
    }
    let total = steps.len();
    let outcome = if fail > 0 {
        ScenarioOutcome::Fail
    } else if not_executable > 0 {
        ScenarioOutcome::NotExecutable
    } else {
        ScenarioOutcome::Pass
    };
    let reason = format!(
        "{}/{} steps executable: {} pass, {} fail, {} not-executable",
        total - not_executable,
        total,
        pass,
        fail,
        not_executable
    );
    ScenarioRecord {
        scenario_id: scenario.scenario_id.clone(),
        family: scenario.family.clone(),
        outcome,
        reason,
        steps,
    }
}

fn run(plan_dir: &Path) -> Result<Report, String> {
    let scenarios = load_scenarios(plan_dir)?;
    let cases = load_case_map(plan_dir)?;
    let registry = ExampleRegistry::new();
    // TODO (L3): register each example's run_scenario owner here once examples export run_scenario.
    let mut records: Vec<ScenarioRecord> = Vec::new();
    let mut pass = 0;
    let mut fail = 0;
    let mut not_executable = 0;
    let mut steps = 0;
    let mut steps_executable = 0;
    let mut steps_not_executable = 0;
    let mut steps_pass = 0;
    let mut steps_fail = 0;
    for scenario in &scenarios {
        let record = run_scenario(scenario, &cases, &registry, plan_dir);
        match record.outcome {
            ScenarioOutcome::Pass => pass += 1,
            ScenarioOutcome::Fail => fail += 1,
            ScenarioOutcome::NotExecutable => not_executable += 1,
        }
        for step in &record.steps {
            steps += 1;
            match step.outcome {
                StepOutcome::Pass => {
                    steps_executable += 1;
                    steps_pass += 1;
                }
                StepOutcome::Fail => {
                    steps_executable += 1;
                    steps_fail += 1;
                }
                StepOutcome::NotExecutable => steps_not_executable += 1,
            }
        }
        records.push(record);
    }
    Ok(Report {
        records,
        summary: Summary {
            summary: true,
            scenarios: scenarios.len(),
            pass,
            fail,
            not_executable,
            steps,
            steps_executable,
            steps_not_executable,
            steps_pass,
            steps_fail,
        },
    })
}

fn main() {
    let plan_dir = env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("cans/artifacts/test-plan"), PathBuf::from);
    match run(&plan_dir) {
        Ok(report) => {
            for record in &report.records {
                match serde_json::to_string(record) {
                    Ok(line) => println!("{line}"),
                    Err(e) => {
                        eprintln!("scenario-runner: serialize record: {e}");
                        process::exit(1);
                    }
                }
            }
            match serde_json::to_string(&report.summary) {
                Ok(line) => println!("{line}"),
                Err(e) => {
                    eprintln!("scenario-runner: serialize summary: {e}");
                    process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("scenario-runner: {e}");
            process::exit(1);
        }
    }
}
