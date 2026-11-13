// see: cans/15-operations.md#--unified-builder-coverage
// see: cans/10-attestation.md#Execution-receipts

import type { ExecutionReceipt, ExitKind } from "@kelir/core";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const nodeProcess = (
  globalThis as unknown as { process: { argv: string[]; exitCode: number | undefined } }
).process;

const SCENARIOS_INDEX = "scenarios.yaml";
const CASE_MAP = "case-to-test-map.yaml";

// see: cans/15-operations.md#--unified-builder-coverage
export interface StepDescriptor {
  case_id: string;
  assert_text: string;
}

// see: cans/15-operations.md#--unified-builder-coverage
export interface Scenario {
  scenario_id: string;
  file: string;
  family: string;
  target_tier: string;
  title: string;
  steps: StepDescriptor[];
}

export type CaseStatus = "implemented" | "red" | "gap";

// see: cans/15-operations.md#--unified-builder-coverage
export interface CaseEntry {
  case_id: string;
  case_file: string;
  target_crate: string;
  target_tier: string;
  status: CaseStatus;
  test_file: string | null;
  test_fn: string | null;
}

// see: cans/10-attestation.md#Execution-receipts
export interface ExpectedReceipt {
  exit_code: number;
  exit_kind: string;
  fault_reason: string | null;
}

export type StepOutcome = "pass" | "fail" | "not-executable";
export type ScenarioOutcome = "pass" | "fail" | "not-executable";

// see: cans/15-operations.md#--unified-builder-coverage
export interface StepRecord {
  case_id: string;
  outcome: StepOutcome;
  reason: string;
  owner_test_file: string | null;
  owner_test_fn: string | null;
  expected_exit_code: number | null;
  expected_exit_kind: string | null;
  observed_exit_code: number | null;
  observed_exit_kind: string | null;
  diff: string[];
}

// see: cans/15-operations.md#--unified-builder-coverage
export interface ScenarioRecord {
  scenario_id: string;
  family: string;
  outcome: ScenarioOutcome;
  reason: string;
  steps: StepRecord[];
}

export interface Summary {
  summary: boolean;
  scenarios: number;
  pass: number;
  fail: number;
  not_executable: number;
  steps: number;
  steps_executable: number;
  steps_not_executable: number;
  steps_pass: number;
  steps_fail: number;
}

export interface Report {
  records: ScenarioRecord[];
  summary: Summary;
}

// see: cans/15-operations.md#--unified-builder-coverage
export class ExampleRegistry {
  private readonly owners = new Map<string, string>();

  register(caseId: string, example: string): void {
    this.owners.set(caseId, example);
  }

  findOwner(caseId: string): string | undefined {
    return this.owners.get(caseId);
  }
}

function unquote(value: string): string {
  const trimmed = value.trim();
  if (trimmed.length >= 2 && trimmed.startsWith('"') && trimmed.endsWith('"')) {
    return trimmed.slice(1, -1);
  }
  return trimmed;
}

function optString(value: string): string | null {
  const trimmed = value.trim();
  return trimmed === "null" ? null : unquote(trimmed);
}

function stripPrefix(line: string, prefix: string): string | null {
  return line.startsWith(prefix) ? line.slice(prefix.length) : null;
}

function parseScenarioSteps(text: string): StepDescriptor[] {
  const steps: StepDescriptor[] = [];
  let current: StepDescriptor | null = null;
  let inSteps = false;
  for (const line of text.split("\n")) {
    if (line.startsWith("steps:")) {
      inSteps = true;
      continue;
    }
    if (inSteps && !line.startsWith(" ") && line !== "") {
      inSteps = false;
    }
    if (!inSteps) {
      continue;
    }
    const refValue = stripPrefix(line, "  - ref: ");
    if (refValue !== null) {
      if (current !== null) {
        steps.push(current);
      }
      current = { case_id: unquote(refValue), assert_text: "" };
      continue;
    }
    const assertValue = stripPrefix(line, "    assert: ");
    if (assertValue !== null && current !== null) {
      current.assert_text = unquote(assertValue);
    }
  }
  if (current !== null) {
    steps.push(current);
  }
  return steps;
}

function loadScenarios(planDir: string): Scenario[] {
  const text = readFileSync(join(planDir, SCENARIOS_INDEX), "utf8");
  const scenarios: Scenario[] = [];
  let current: Scenario | null = null;
  for (const line of text.split("\n")) {
    const idValue = stripPrefix(line, "  - scenario_id: ");
    if (idValue !== null) {
      if (current !== null) {
        scenarios.push(current);
      }
      current = {
        scenario_id: unquote(idValue),
        file: "",
        family: "",
        target_tier: "",
        title: "",
        steps: [],
      };
      continue;
    }
    if (current === null) {
      continue;
    }
    const fileValue = stripPrefix(line, "    file: ");
    if (fileValue !== null) {
      current.file = unquote(fileValue);
      continue;
    }
    const familyValue = stripPrefix(line, "    family: ");
    if (familyValue !== null) {
      current.family = unquote(familyValue);
      continue;
    }
    const tierValue = stripPrefix(line, "    target_tier: ");
    if (tierValue !== null) {
      current.target_tier = unquote(tierValue);
      continue;
    }
    const titleValue = stripPrefix(line, "    title: ");
    if (titleValue !== null) {
      current.title = unquote(titleValue);
    }
  }
  if (current !== null) {
    scenarios.push(current);
  }
  for (const scenario of scenarios) {
    scenario.steps = parseScenarioSteps(readFileSync(join(planDir, scenario.file), "utf8"));
  }
  return scenarios;
}

function caseStatus(value: string): CaseStatus {
  switch (value.trim()) {
    case "implemented":
      return "implemented";
    case "red":
      return "red";
    default:
      return "gap";
  }
}

function loadCaseMap(planDir: string): Map<string, CaseEntry> {
  const text = readFileSync(join(planDir, CASE_MAP), "utf8");
  const map = new Map<string, CaseEntry>();
  let current: CaseEntry | null = null;
  for (const line of text.split("\n")) {
    const idValue = stripPrefix(line, "- case_id: ");
    if (idValue !== null) {
      if (current !== null) {
        map.set(current.case_id, current);
      }
      current = {
        case_id: idValue.trim(),
        case_file: "",
        target_crate: "",
        target_tier: "",
        status: "gap",
        test_file: null,
        test_fn: null,
      };
      continue;
    }
    if (current === null) {
      continue;
    }
    const caseFileValue = stripPrefix(line, "  case_file: ");
    if (caseFileValue !== null) {
      current.case_file = unquote(caseFileValue);
      continue;
    }
    const crateValue = stripPrefix(line, "  target_crate: ");
    if (crateValue !== null) {
      current.target_crate = unquote(crateValue);
      continue;
    }
    const tierValue = stripPrefix(line, "  target_tier: ");
    if (tierValue !== null) {
      current.target_tier = unquote(tierValue);
      continue;
    }
    const statusValue = stripPrefix(line, "  status: ");
    if (statusValue !== null) {
      current.status = caseStatus(statusValue);
      continue;
    }
    const testFileValue = stripPrefix(line, "  test_file: ");
    if (testFileValue !== null) {
      current.test_file = optString(testFileValue);
      continue;
    }
    const testFnValue = stripPrefix(line, "  test_fn: ");
    if (testFnValue !== null) {
      current.test_fn = optString(testFnValue);
    }
  }
  if (current !== null) {
    map.set(current.case_id, current);
  }
  return map;
}

function loadExpectedReceipt(planDir: string, entry: CaseEntry): ExpectedReceipt | null {
  const text = readFileSync(join(planDir, entry.case_file), "utf8");
  let inExpected = false;
  let inReceipt = false;
  let exitCode: number | null = null;
  let exitKind: string | null = null;
  let faultReason: string | null = null;
  for (const line of text.split("\n")) {
    if (line.startsWith("expected_observations:")) {
      inExpected = true;
      inReceipt = false;
      continue;
    }
    if (inExpected && !line.startsWith(" ") && line !== "") {
      break;
    }
    if (!inExpected) {
      continue;
    }
    if (line.startsWith("  receipt:")) {
      inReceipt = true;
      continue;
    }
    if (line.startsWith("  ") && !line.startsWith("    ")) {
      inReceipt = false;
    }
    const codeValue = stripPrefix(line, "  exit_code: ");
    if (codeValue !== null) {
      const parsed = Number.parseInt(codeValue.trim(), 10);
      exitCode = Number.isNaN(parsed) ? null : parsed;
      continue;
    }
    if (inReceipt) {
      const kindValue = stripPrefix(line, "    exit_kind: ");
      if (kindValue !== null) {
        exitKind = unquote(kindValue);
        continue;
      }
      const faultValue = stripPrefix(line, "    fault_reason: ");
      if (faultValue !== null) {
        faultReason = optString(faultValue);
      }
    }
  }
  if (exitCode === null || exitKind === null) {
    return null;
  }
  return { exit_code: exitCode, exit_kind: exitKind, fault_reason: faultReason };
}

function notExecutableRecord(
  step: StepDescriptor,
  entry: CaseEntry | null,
  expected: ExpectedReceipt | null,
  reason: string,
): StepRecord {
  return {
    case_id: step.case_id,
    outcome: "not-executable",
    reason,
    owner_test_file: entry === null ? null : entry.test_file,
    owner_test_fn: entry === null ? null : entry.test_fn,
    expected_exit_code: expected === null ? null : expected.exit_code,
    expected_exit_kind: expected === null ? null : expected.exit_kind,
    observed_exit_code: null,
    observed_exit_kind: null,
    diff: [],
  };
}

// TODO (L3): invoke the owning example's run_scenario(step) and return its ExecutionReceipt.
function invokeExample(
  _registry: ExampleRegistry,
  _caseId: string,
  _step: StepDescriptor,
): ExecutionReceipt | null {
  return null;
}

// see: cans/10-attestation.md#Execution-receipts
function exitKindName(kind: ExitKind): string {
  switch (kind.kind) {
    case "normal":
      return "Normal";
    case "signal":
      return "Signal";
    case "kernel-panic":
      return "KernelPanic";
    case "hypervisor-fault":
      return "HypervisorFault";
    case "dirty-shutdown":
      return "DirtyShutdown";
    case "hard-killed":
      return "HardKilled";
  }
}

// see: cans/10-attestation.md#Execution-receipts
function diffReceipt(observed: ExecutionReceipt, expected: ExpectedReceipt): string[] {
  const mismatches: string[] = [];
  if (observed.exitCode !== expected.exit_code) {
    mismatches.push("exit_code");
  }
  if (exitKindName(observed.exitKind) !== expected.exit_kind) {
    mismatches.push("exit_kind");
  }
  return mismatches;
}

function resolveStep(
  step: StepDescriptor,
  cases: Map<string, CaseEntry>,
  registry: ExampleRegistry,
  planDir: string,
): StepRecord {
  const entry = cases.get(step.case_id);
  if (entry === undefined) {
    return notExecutableRecord(step, null, null, "case_id not present in case-to-test-map.yaml");
  }
  const expected = loadExpectedReceipt(planDir, entry);
  switch (entry.status) {
    case "gap":
      return notExecutableRecord(
        step,
        entry,
        expected,
        "case is a verified gap: no live boundary exists",
      );
    case "red":
      return notExecutableRecord(
        step,
        entry,
        expected,
        "case is L3-red: test exists but is ignored-only, no executable path",
      );
    case "implemented": {
      if (registry.findOwner(step.case_id) === undefined) {
        return notExecutableRecord(
          step,
          entry,
          expected,
          "case is implemented but no example exports run_scenario",
        );
      }
      const observed = invokeExample(registry, step.case_id, step);
      if (observed === null) {
        return notExecutableRecord(
          step,
          entry,
          expected,
          "owning example has no run_scenario entry point",
        );
      }
      if (expected === null) {
        return notExecutableRecord(step, entry, null, "case file pins no expected receipt");
      }
      const diff = diffReceipt(observed, expected);
      const outcome: StepOutcome = diff.length === 0 ? "pass" : "fail";
      return {
        case_id: step.case_id,
        outcome,
        reason:
          diff.length === 0
            ? "observed receipt matches the pinned expectation"
            : `observed receipt differs on: ${diff.join(", ")}`,
        owner_test_file: entry.test_file,
        owner_test_fn: entry.test_fn,
        expected_exit_code: expected.exit_code,
        expected_exit_kind: expected.exit_kind,
        observed_exit_code: observed.exitCode,
        observed_exit_kind: exitKindName(observed.exitKind),
        diff,
      };
    }
  }
}

function runScenario(
  scenario: Scenario,
  cases: Map<string, CaseEntry>,
  registry: ExampleRegistry,
  planDir: string,
): ScenarioRecord {
  const steps: StepRecord[] = [];
  let pass = 0;
  let fail = 0;
  let notExecutable = 0;
  for (const step of scenario.steps) {
    const record = resolveStep(step, cases, registry, planDir);
    if (record.outcome === "pass") {
      pass += 1;
    } else if (record.outcome === "fail") {
      fail += 1;
    } else {
      notExecutable += 1;
    }
    steps.push(record);
  }
  const total = steps.length;
  const outcome: ScenarioOutcome =
    fail > 0 ? "fail" : notExecutable > 0 ? "not-executable" : "pass";
  return {
    scenario_id: scenario.scenario_id,
    family: scenario.family,
    outcome,
    reason: `${total - notExecutable}/${total} steps executable: ${pass} pass, ${fail} fail, ${notExecutable} not-executable`,
    steps,
  };
}

export function run(planDir: string): Report {
  const scenarios = loadScenarios(planDir);
  const cases = loadCaseMap(planDir);
  const registry = new ExampleRegistry();
  // TODO (L3): register each example's run_scenario owner here once examples export run_scenario.
  const records: ScenarioRecord[] = [];
  let pass = 0;
  let fail = 0;
  let notExecutable = 0;
  let steps = 0;
  let stepsExecutable = 0;
  let stepsNotExecutable = 0;
  let stepsPass = 0;
  let stepsFail = 0;
  for (const scenario of scenarios) {
    const record = runScenario(scenario, cases, registry, planDir);
    if (record.outcome === "pass") {
      pass += 1;
    } else if (record.outcome === "fail") {
      fail += 1;
    } else {
      notExecutable += 1;
    }
    for (const step of record.steps) {
      steps += 1;
      if (step.outcome === "pass") {
        stepsExecutable += 1;
        stepsPass += 1;
      } else if (step.outcome === "fail") {
        stepsExecutable += 1;
        stepsFail += 1;
      } else {
        stepsNotExecutable += 1;
      }
    }
    records.push(record);
  }
  return {
    records,
    summary: {
      summary: true,
      scenarios: scenarios.length,
      pass,
      fail,
      not_executable: notExecutable,
      steps,
      steps_executable: stepsExecutable,
      steps_not_executable: stepsNotExecutable,
      steps_pass: stepsPass,
      steps_fail: stepsFail,
    },
  };
}

function main(): number {
  const planDir = nodeProcess.argv[2] ?? "cans/artifacts/test-plan";
  try {
    const report = run(planDir);
    for (const record of report.records) {
      console.log(JSON.stringify(record));
    }
    console.log(JSON.stringify(report.summary));
    return 0;
  } catch (error) {
    console.error(`scenario-runner: ${error instanceof Error ? error.message : String(error)}`);
    return 1;
  }
}

nodeProcess.exitCode = main();
