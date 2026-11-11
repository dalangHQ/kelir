# Kelir Test Plan Structure and Case Discipline Specification

This document defines the normative structure, taxonomy, validation invariants, and Anti-Slop Laws governing all automated tests across the Kelir workspace.

## 1. Physical Authority Law

Kelir is an in-process microVM hypervisor microkernel. Its execution floor is real hardware: physical vCPUs, hardware MMU translation tables (EPT / Stage-2), shared-memory SPSC rings, host file descriptors, and userspace network frames.

Every test case in this test plan asserts host-level physical reality. A test that asserts state inside an unverified guest environment without host-side corroboration is **AI slop** and is rejected by the CI linter.

## 2. Case Discipline Rules (CDR)

Every test case defined in `cases/**/*.yaml` and implemented across `crates/**/tests/` must strictly satisfy these eight rules:

- **CDR-01: Single Invariant Scope**
  - Each test case exercises exactly one physical invariant.
  - Bundling distinct security boundaries into one test case is forbidden.
  - Bundling distinct failure modes into one test case is forbidden.
* **CDR-02: Deterministic Clock and PRNG**: Wall-clock sleeps (`std::thread::sleep`) and unseeded entropy are banned. Deterministic tests must execute under pinned virtual clock parameters (`tsc=reliable clocksource=kvm-clock` or `CNTVOFF_EL2`) and seeded ChaCha20 streams.
* **CDR-03: Hardcoded Expected Literals**: Expected values (BLAKE3 binary hashes, dirty byte totals, POSIX errnos, exit kinds) must be literal constants. Computing expected values by running the subject code through a helper inside the test is banned (Anti-Tautology).
* **CDR-04: Explicit Physical Boundaries**: Tests asserting hardware virtualization must run on real KVM/HVF/WHP hardware. Emulated execution tiers (`driver-bwrap`, `driver-proot`) must fail closed with `E_NOHARDWAREVMM` unless `allow_emulated_fallback: true` is explicitly configured.
* **CDR-05: Mandatory Rollback Probing**: Any test validating a rollback or drop must physically inspect the host storage broker to verify the upper sparse file extents are punched (`FALLOC_FL_PUNCH_HOLE`), tombstones are zeroed, and active host sockets are closed.
* **CDR-06: Strict Typed Verdict Assertions**: Assertions on `session.exec()`, hook dispatches, or filter pipelines must match the typed enum variant (`Verdict::Pass`, `Verdict::Patch`, `Verdict::Drop`, `Verdict::Suspend`, `Verdict::Abort` or `FilterAction`). Calling `.is_ok()` or `.is_err()` is grounds for immediate CI failure.
* **CDR-07: Struct Exhaustiveness**: Test assertions must unpack `ExecutionReceipt` and `CausalDelta` exhaustively. Adding a field to these structs in `kelir-interface` must break compilation across all asserting test fixtures.
* **CDR-08: Zero Host Leakage**: Tests must prove that upon microVM termination or suspend, all host descriptors (`/proc/self/fd`), lockfile leases, and ephemeral scratch paths are completely unlinked.

## 3. Anti-Slop Laws (AS-01 to AS-08)

The workspace enforces eight compile-time and static-analysis linter laws via `kelir-testkit check-tests`:

| Law | Name | Physical Invariant Enforced |
|---|---|---|
| **AS-01** | Banned VMM Mocking | Prohibits mocking `VmmDriver`, Stage-2 MMU, or VirtIO rings in integration/e2e tiers. |
| **AS-02** | Physical Mutation Proof | Prohibits asserting successful drop/revert without asserting host disk bytes == 0 and socket count == 0. |
| **AS-03** | Typed Verdict Law | Prohibits untyped `.is_ok()` / `.is_err()`. Requires explicit enum matching and receipt verification. |
| **AS-04A** | Deterministic Replay | Prohibits host wall-clock sampling, PMU cross-host comparison, and unseeded randomness in unit and deterministic integration runs; clock pinned via pvclock/CNTVOFF_EL2. |
| **AS-04B** | Physical Budget Floor | Requires host wall-clock and vCPU-cycle measurement in hardware benchmark suites; enforces thresholds declared in perf/budgets.toml with enforce = true. |
| **AS-05** | Merkle WAL Chain Integrity | Prohibits claiming committed mutations without proving BLAKE3 RFC 6962 tree updates when ledger is active. |
| **AS-06** | Anti-Tautology Law | Prohibits deriving expected assertion values from subject function outputs. |
| **AS-07** | Hardware Platform Law | Prohibits claiming virtualization coverage while running under user-namespace emulation. |
| **AS-08** | Zero-Dangling Resources | Prohibits tests completing with leaked host FDs, tmpfs mounts, or flock locks. |

## 4. Case Schema (`test-plan/case/v1`)

Every file under `cases/{domain}/{case_id}.yaml` must validate against the following structural layout:

```yaml
$schema: "test-plan/case/v1"
case_id: "<domain_prefix>_<index>_<snake_case_slug>"
title: "<Narrative description of physical invariant>"
domain: "<domain_name>"
target_tier: "unit" | "integration" | "e2e"
target_crate: "<crates/path_to_crate>"
platform_target:
  os: "linux" | "macos" | "windows" | "android" | "any"
  arch: "x86_64" | "aarch64" | "any"
  hypervisor: "kvm" | "hvf" | "whp" | "avf" | "emulated" | "none"

invariant:
  law_reference: "<cans_file.md#anchor>"
  statement: "<Physical statement of truth>"
  blast_radius_if_breached: "<Worst-case security/stability consequence>"

confusion_matrix:
  quadrant: "true_positive" | "true_negative"
  slop_risk: "<How an LLM or lazy tester would fake this test>"
  anti_slop_mitigation: "<Specific AS-xx rules applied to make faking impossible>"

setup:
  env: "dev"
  platform_tier: "tier_1" | "tier_2"
  caller_identity:
    principal: "<id>"
    agent_id: "<id>"
    trust: "reviewed"
  initial_state:
    driver_policy:
      backend: "auto"
      wx_enforcement: "Strict" | "PermissiveAudit"
      allow_emulated_fallback: false
    workspace_files: []
    tables: []
    vault_secrets: []

execution:
  trigger: "rust_api" | "vsock_frame" | "fuse_op" | "net_frame" | "sdk"
  command: "<Action executed against microVM>"
  input_params: {}

expected_observations:
  exit_code: 0 | 1..255
  state_modified: true | false
  diagnostic:
    domain: "<subsystem>"
    culprit: "<cause>"
    remedy: "<action>"
  database:
    row_count_delta: 0
    tables_mutated: []
  storage:
    upper_bytes_dirtied: 0
    tombstones_created: 0
    host_blocks_allocated: 0
  wire:
    frames_outbound: 0
    frames_inbound: 0
    trapped_syscalls: []
    synthetic_responses: []
  audit:
    event_emitted: "<event_name>"
    chain_valid: true | false
    state_modified_field: true | false
  receipt:
    exit_kind: "Normal" | "Signal" | "KernelPanic" | "HypervisorFault"
    fault_reason: "<optional_reason_string>"

anti_slop_assertions:
  - rule: "AS-0x"
    verification: "<Literal code-level proof required>"
```

## 5. Scenario Discipline

Scenarios are the composition tier under `scenarios/`; they compose domain cases into user journeys and never re-assert physical facts owned by domain cases. Four rules extend the Case Discipline Rules of section 2 — CDR-09 through CDR-12 — to this tier.

* **CDR-09: Reference-Only Composition**: A scenario may only reference `case_id`s; every `assert:` is a *reference* to a fact owned by the referenced domain case (a verbatim substring of its `statement` or `expected_observations`), never a new physical assertion. A needed new physical fact goes into the domain case first; the scenario then references it.
* **CDR-10: No Free-Form Steps**: Every step is a `ref:` to an existing `case_id` in `manifest.yaml`; fixture setup is allowed, free-form step bodies are banned.
* **CDR-11: E2E Tier Only**: Scenarios are `target_tier: "e2e"` and cannot satisfy unit or integration coverage.
* **CDR-12: Composition Budget**: At most 40 scenarios total; a 41st must justify itself by composing a plane pair no other scenario covers. Scenario runtimes never enter the per-domain scorecard pass counts; they render in their own Scenario Tier table.

Every file under `scenarios/{family}/{scenario_id}.yaml` must validate against the following structural layout (CDR-10 `ref:`-only steps, CDR-11 tier pin):

```yaml
$schema: "test-plan/scenario/v1"
scenario_id: "<sc_family_snake_case>"
title: "<user journey>"
family: "<lifecycle|agent|concurrency|recovery|pressure|network|parity|smoke>"
target_tier: "e2e"
languages: [rust] | [rust, typescript, python]
budget: "<Ns>"                      # smoke family only
fixture_ref: "fixtures/scenarios/<name>/"   # optional
steps:
  - ref: "<case_id>"
    assert: "<verbatim substring of the referenced case's statement or expected_observations>"
anti_slop_assertions:
  - rule: "AS-0x"
    verification: "<literal proof required>"
```

* `scenarios.yaml` is the scenario index manifest (`$schema: "kelir-test-plan/scenario-manifest/v1"`); it mirrors the `manifest.yaml` shape, is validated by `check-scenarios`, and its family totals obey the CDR-12 composition budget.
* Fixtures are read-only test inputs under `cans/artifacts/test-plan/fixtures/scenarios/`; a fixture is a test input, not a spec artifact, so it lives under `cans/artifacts/` and never among the spec nodes in `cans/` — each directory is declared by a `test-plan/fixture/v1` manifest and forms the CDR-10-permitted setup surface.
* AS-04A applies unchanged: scenario steps use StepVerdict/instruction counts, never wall-clock sampling; smoke `budget:` values are CI job wall budgets, not test-internal assertions.
* The `ci_gates.check_scenarios` gate (see matrix.yaml) rejects a scenario whose `ref:` does not resolve or whose `assert:` is not a substring of the referenced case (CDR-09); `--scenario-coverage` asserts every adjacent plane pair (terminal×storage, storage×wire, wire×checkpoint, checkpoint×host) is composed by at least one scenario.
