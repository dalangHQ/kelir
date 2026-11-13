# see: cans/15-operations.md#--unified-builder-coverage
# see: cans/10-attestation.md#Execution-receipts

import json
import sys
from dataclasses import asdict, dataclass, field
from enum import Enum
from pathlib import Path

from kelir import ExecutionReceipt

SCENARIOS_INDEX = "scenarios.yaml"
CASE_MAP = "case-to-test-map.yaml"


class CaseStatus(str, Enum):
    IMPLEMENTED = "implemented"
    RED = "red"
    GAP = "gap"


class StepOutcome(str, Enum):
    PASS = "pass"
    FAIL = "fail"
    NOT_EXECUTABLE = "not-executable"


class ScenarioOutcome(str, Enum):
    PASS = "pass"
    FAIL = "fail"
    NOT_EXECUTABLE = "not-executable"


# see: cans/15-operations.md#--unified-builder-coverage
@dataclass
class StepDescriptor:
    case_id: str
    assert_text: str


# see: cans/15-operations.md#--unified-builder-coverage
@dataclass
class Scenario:
    scenario_id: str
    file: str
    family: str
    target_tier: str
    title: str
    steps: list[StepDescriptor] = field(default_factory=list)


# see: cans/15-operations.md#--unified-builder-coverage
@dataclass
class CaseEntry:
    case_id: str
    case_file: str
    target_crate: str
    target_tier: str
    status: CaseStatus
    test_file: str | None = None
    test_fn: str | None = None


# see: cans/10-attestation.md#Execution-receipts
@dataclass
class ExpectedReceipt:
    exit_code: int
    exit_kind: str
    fault_reason: str | None = None


# see: cans/15-operations.md#--unified-builder-coverage
@dataclass
class StepRecord:
    case_id: str
    outcome: StepOutcome
    reason: str
    owner_test_file: str | None
    owner_test_fn: str | None
    expected_exit_code: int | None
    expected_exit_kind: str | None
    observed_exit_code: int | None
    observed_exit_kind: str | None
    diff: list[str] = field(default_factory=list)


# see: cans/15-operations.md#--unified-builder-coverage
@dataclass
class ScenarioRecord:
    scenario_id: str
    family: str
    outcome: ScenarioOutcome
    reason: str
    steps: list[StepRecord] = field(default_factory=list)


@dataclass
class Summary:
    summary: bool = True
    scenarios: int = 0
    pass_count: int = 0
    fail: int = 0
    not_executable: int = 0
    steps: int = 0
    steps_executable: int = 0
    steps_not_executable: int = 0
    steps_pass: int = 0
    steps_fail: int = 0


@dataclass
class Report:
    records: list[ScenarioRecord] = field(default_factory=list)
    summary: Summary = field(default_factory=Summary)


# see: cans/15-operations.md#--unified-builder-coverage
class ExampleRegistry:
    def __init__(self) -> None:
        self._owners: dict[str, str] = {}

    def register(self, case_id: str, example: str) -> None:
        self._owners[case_id] = example

    def find_owner(self, case_id: str) -> str | None:
        return self._owners.get(case_id)


def _unquote(value: str) -> str:
    trimmed = value.strip()
    if len(trimmed) >= 2 and trimmed.startswith('"') and trimmed.endswith('"'):
        return trimmed[1:-1]
    return trimmed


def _opt_string(value: str) -> str | None:
    trimmed = value.strip()
    return None if trimmed == "null" else _unquote(trimmed)


def _strip_prefix(line: str, prefix: str) -> str | None:
    return line[len(prefix):] if line.startswith(prefix) else None


def _parse_scenario_steps(text: str) -> list[StepDescriptor]:
    steps: list[StepDescriptor] = []
    current: StepDescriptor | None = None
    in_steps = False
    for line in text.split("\n"):
        if line.startswith("steps:"):
            in_steps = True
            continue
        if in_steps and not line.startswith(" ") and line != "":
            in_steps = False
        if not in_steps:
            continue
        ref_value = _strip_prefix(line, "  - ref: ")
        if ref_value is not None:
            if current is not None:
                steps.append(current)
            current = StepDescriptor(case_id=_unquote(ref_value), assert_text="")
            continue
        assert_value = _strip_prefix(line, "    assert: ")
        if assert_value is not None and current is not None:
            current.assert_text = _unquote(assert_value)
    if current is not None:
        steps.append(current)
    return steps


def _load_scenarios(plan_dir: Path) -> list[Scenario]:
    text = (plan_dir / SCENARIOS_INDEX).read_text(encoding="utf-8")
    scenarios: list[Scenario] = []
    current: Scenario | None = None
    for line in text.split("\n"):
        id_value = _strip_prefix(line, "  - scenario_id: ")
        if id_value is not None:
            if current is not None:
                scenarios.append(current)
            current = Scenario(
                scenario_id=_unquote(id_value), file="", family="", target_tier="", title=""
            )
            continue
        if current is None:
            continue
        file_value = _strip_prefix(line, "    file: ")
        if file_value is not None:
            current.file = _unquote(file_value)
            continue
        family_value = _strip_prefix(line, "    family: ")
        if family_value is not None:
            current.family = _unquote(family_value)
            continue
        tier_value = _strip_prefix(line, "    target_tier: ")
        if tier_value is not None:
            current.target_tier = _unquote(tier_value)
            continue
        title_value = _strip_prefix(line, "    title: ")
        if title_value is not None:
            current.title = _unquote(title_value)
    if current is not None:
        scenarios.append(current)
    for scenario in scenarios:
        scenario.steps = _parse_scenario_steps(
            (plan_dir / scenario.file).read_text(encoding="utf-8")
        )
    return scenarios


def _case_status(value: str) -> CaseStatus:
    if value.strip() == "implemented":
        return CaseStatus.IMPLEMENTED
    if value.strip() == "red":
        return CaseStatus.RED
    return CaseStatus.GAP


def _load_case_map(plan_dir: Path) -> dict[str, CaseEntry]:
    text = (plan_dir / CASE_MAP).read_text(encoding="utf-8")
    case_map: dict[str, CaseEntry] = {}
    current: CaseEntry | None = None
    for line in text.split("\n"):
        id_value = _strip_prefix(line, "- case_id: ")
        if id_value is not None:
            if current is not None:
                case_map[current.case_id] = current
            current = CaseEntry(
                case_id=id_value.strip(),
                case_file="",
                target_crate="",
                target_tier="",
                status=CaseStatus.GAP,
            )
            continue
        if current is None:
            continue
        case_file_value = _strip_prefix(line, "  case_file: ")
        if case_file_value is not None:
            current.case_file = _unquote(case_file_value)
            continue
        crate_value = _strip_prefix(line, "  target_crate: ")
        if crate_value is not None:
            current.target_crate = _unquote(crate_value)
            continue
        tier_value = _strip_prefix(line, "  target_tier: ")
        if tier_value is not None:
            current.target_tier = _unquote(tier_value)
            continue
        status_value = _strip_prefix(line, "  status: ")
        if status_value is not None:
            current.status = _case_status(status_value)
            continue
        test_file_value = _strip_prefix(line, "  test_file: ")
        if test_file_value is not None:
            current.test_file = _opt_string(test_file_value)
            continue
        test_fn_value = _strip_prefix(line, "  test_fn: ")
        if test_fn_value is not None:
            current.test_fn = _opt_string(test_fn_value)
    if current is not None:
        case_map[current.case_id] = current
    return case_map


def _load_expected_receipt(plan_dir: Path, entry: CaseEntry) -> ExpectedReceipt | None:
    text = (plan_dir / entry.case_file).read_text(encoding="utf-8")
    in_expected = False
    in_receipt = False
    exit_code: int | None = None
    exit_kind: str | None = None
    fault_reason: str | None = None
    for line in text.split("\n"):
        if line.startswith("expected_observations:"):
            in_expected = True
            in_receipt = False
            continue
        if in_expected and not line.startswith(" ") and line != "":
            break
        if not in_expected:
            continue
        if line.startswith("  receipt:"):
            in_receipt = True
            continue
        if line.startswith("  ") and not line.startswith("    "):
            in_receipt = False
        code_value = _strip_prefix(line, "  exit_code: ")
        if code_value is not None:
            try:
                exit_code = int(code_value.strip())
            except ValueError:
                exit_code = None
            continue
        if in_receipt:
            kind_value = _strip_prefix(line, "    exit_kind: ")
            if kind_value is not None:
                exit_kind = _unquote(kind_value)
                continue
            fault_value = _strip_prefix(line, "    fault_reason: ")
            if fault_value is not None:
                fault_reason = _opt_string(fault_value)
    if exit_code is None or exit_kind is None:
        return None
    return ExpectedReceipt(exit_code=exit_code, exit_kind=exit_kind, fault_reason=fault_reason)


def _not_executable_record(
    step: StepDescriptor,
    entry: CaseEntry | None,
    expected: ExpectedReceipt | None,
    reason: str,
) -> StepRecord:
    return StepRecord(
        case_id=step.case_id,
        outcome=StepOutcome.NOT_EXECUTABLE,
        reason=reason,
        owner_test_file=None if entry is None else entry.test_file,
        owner_test_fn=None if entry is None else entry.test_fn,
        expected_exit_code=None if expected is None else expected.exit_code,
        expected_exit_kind=None if expected is None else expected.exit_kind,
        observed_exit_code=None,
        observed_exit_kind=None,
        diff=[],
    )


# TODO (L3): invoke the owning example's run_scenario(step) and return its ExecutionReceipt.
def _invoke_example(
    _registry: ExampleRegistry, _case_id: str, _step: StepDescriptor
) -> ExecutionReceipt | None:
    return None


_EXIT_KIND_NAMES = {
    "normal": "Normal",
    "signal": "Signal",
    "kernel-panic": "KernelPanic",
    "hypervisor-fault": "HypervisorFault",
    "dirty-shutdown": "DirtyShutdown",
    "hard-killed": "HardKilled",
}


# see: cans/10-attestation.md#Execution-receipts
def _exit_kind_name(receipt: ExecutionReceipt) -> str:
    return _EXIT_KIND_NAMES.get(receipt.exit_kind.kind, "Unknown")


# see: cans/10-attestation.md#Execution-receipts
def _diff_receipt(observed: ExecutionReceipt, expected: ExpectedReceipt) -> list[str]:
    mismatches: list[str] = []
    if observed.exit_code != expected.exit_code:
        mismatches.append("exit_code")
    if _exit_kind_name(observed) != expected.exit_kind:
        mismatches.append("exit_kind")
    return mismatches


def _resolve_step(
    step: StepDescriptor,
    cases: dict[str, CaseEntry],
    registry: ExampleRegistry,
    plan_dir: Path,
) -> StepRecord:
    entry = cases.get(step.case_id)
    if entry is None:
        return _not_executable_record(
            step, None, None, "case_id not present in case-to-test-map.yaml"
        )
    expected = _load_expected_receipt(plan_dir, entry)
    if entry.status is CaseStatus.GAP:
        return _not_executable_record(
            step, entry, expected, "case is a verified gap: no live boundary exists"
        )
    if entry.status is CaseStatus.RED:
        return _not_executable_record(
            step,
            entry,
            expected,
            "case is L3-red: test exists but is ignored-only, no executable path",
        )
    if registry.find_owner(step.case_id) is None:
        return _not_executable_record(
            step, entry, expected, "case is implemented but no example exports run_scenario"
        )
    observed = _invoke_example(registry, step.case_id, step)
    if observed is None:
        return _not_executable_record(
            step, entry, expected, "owning example has no run_scenario entry point"
        )
    if expected is None:
        return _not_executable_record(
            step, entry, None, "case file pins no expected receipt"
        )
    diff = _diff_receipt(observed, expected)
    outcome = StepOutcome.PASS if not diff else StepOutcome.FAIL
    return StepRecord(
        case_id=step.case_id,
        outcome=outcome,
        reason="observed receipt matches the pinned expectation"
        if not diff
        else f"observed receipt differs on: {', '.join(diff)}",
        owner_test_file=entry.test_file,
        owner_test_fn=entry.test_fn,
        expected_exit_code=expected.exit_code,
        expected_exit_kind=expected.exit_kind,
        observed_exit_code=observed.exit_code,
        observed_exit_kind=_exit_kind_name(observed),
        diff=diff,
    )


def _run_scenario(
    scenario: Scenario,
    cases: dict[str, CaseEntry],
    registry: ExampleRegistry,
    plan_dir: Path,
) -> ScenarioRecord:
    steps: list[StepRecord] = []
    passed = 0
    failed = 0
    not_executable = 0
    for step in scenario.steps:
        record = _resolve_step(step, cases, registry, plan_dir)
        if record.outcome is StepOutcome.PASS:
            passed += 1
        elif record.outcome is StepOutcome.FAIL:
            failed += 1
        else:
            not_executable += 1
        steps.append(record)
    total = len(steps)
    if failed > 0:
        outcome = ScenarioOutcome.FAIL
    elif not_executable > 0:
        outcome = ScenarioOutcome.NOT_EXECUTABLE
    else:
        outcome = ScenarioOutcome.PASS
    reason = (
        f"{total - not_executable}/{total} steps executable: "
        f"{passed} pass, {failed} fail, {not_executable} not-executable"
    )
    return ScenarioRecord(
        scenario_id=scenario.scenario_id,
        family=scenario.family,
        outcome=outcome,
        reason=reason,
        steps=steps,
    )


def run(plan_dir: Path) -> Report:
    scenarios = _load_scenarios(plan_dir)
    cases = _load_case_map(plan_dir)
    registry = ExampleRegistry()
    # TODO (L3): register each example's run_scenario owner here once examples export run_scenario.
    records: list[ScenarioRecord] = []
    passed = 0
    failed = 0
    not_executable = 0
    steps = 0
    steps_executable = 0
    steps_not_executable = 0
    steps_pass = 0
    steps_fail = 0
    for scenario in scenarios:
        record = _run_scenario(scenario, cases, registry, plan_dir)
        if record.outcome is ScenarioOutcome.PASS:
            passed += 1
        elif record.outcome is ScenarioOutcome.FAIL:
            failed += 1
        else:
            not_executable += 1
        for step in record.steps:
            steps += 1
            if step.outcome is StepOutcome.PASS:
                steps_executable += 1
                steps_pass += 1
            elif step.outcome is StepOutcome.FAIL:
                steps_executable += 1
                steps_fail += 1
            else:
                steps_not_executable += 1
        records.append(record)
    summary = Summary(
        scenarios=len(scenarios),
        pass_count=passed,
        fail=failed,
        not_executable=not_executable,
        steps=steps,
        steps_executable=steps_executable,
        steps_not_executable=steps_not_executable,
        steps_pass=steps_pass,
        steps_fail=steps_fail,
    )
    return Report(records=records, summary=summary)


def main(argv: list[str]) -> int:
    plan_dir = Path(argv[1]) if len(argv) > 1 else Path("cans/artifacts/test-plan")
    try:
        report = run(plan_dir)
    except OSError as exc:
        print(f"scenario-runner: {exc}", file=sys.stderr)
        return 1
    for record in report.records:
        print(json.dumps(asdict(record)))
    summary_dict = asdict(report.summary)
    summary_dict["pass"] = summary_dict.pop("pass_count")
    print(json.dumps(summary_dict))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
