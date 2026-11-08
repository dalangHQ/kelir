# Work Order: Test Plan & CI/CD Architectural Alignment

- **Document:** `work-order.md`
- **Target Subsystem:** `cans/artifacts/test-plan/`, `cans/15-operations.md`, `crates/kelir-testkit/`, `.github/workflows/ci.yml`
- **Objective:** Eliminate architectural drift, wire orphaned test artifacts to normative CANS specs, harmonize CI/CD runner limits with test tiers, and mandate physical multi-language verification.
- **Rule Constraints:** Zero bloat, zero bluff. Bullet lengths in `cans/` strictly bounded to 90 characters. Single-line bullets only.

---

## Phase 1: Workspace & Crate Harmonization

### Task 1.1: Resolve Testkit Crate Collision
- **Problem:** `matrix.yaml` invokes `cargo run -p kelir-testkit --bin check-manifest`, but `project-structure.txt` defines `kelir-plugin-testkit` scoped only to plugins.
- **Action:**
  1. Define `crates/kelir-testkit` as the primary workspace test harness.
  2. Move AST linter, manifest verifier, and scorecard generator binaries into `crates/kelir-testkit/src/bin/`.
  3. Retain `crates/kelir-plugin-testkit` as a lightweight downstream mock fixture library or merge it into `kelir-testkit` with a dev-dependency feature flag.
  4. Update `Cargo.toml` workspace members and `artifacts/config/project-structure.txt`.

### Task 1.2: Add Physical `cases/` Directory Path Convention
- **Problem:** Ambiguity regarding the canonical file location for the YAML test definitions.
- **Action:**
  1. Standardize physical path: `cans/artifacts/test-plan/cases/{domain}/{case_id}.yaml`.
  2. Configure `kelir-testkit check-manifest` to treat this path as the single source of truth.

---

## Phase 2: Inbound Spec Wiring (`cans/15-operations.md`)

### Task 2.1: Bind Test Plan into Canonical Spec
- **Problem:** `artifacts/test-plan/` is an illegal orphan artifact with no inbound `see:` references from `cans/`.
- **Action:**
  Add inbound pointers to `cans/15-operations.md#Coverage-gates` obeying the 90-character limit and single-bullet invariant:

```markdown
- Operations
  - Coverage-gates
    - Invariant test manifest: see: artifacts/test-plan/manifest.yaml
    - Execution matrix and anti-slop rules: see: artifacts/test-plan/matrix.yaml
    - Case discipline and schema: see: artifacts/test-plan/test-plan-structure.md
    - Test manifest invariant
      - Every physical case file references exactly one normative spec anchor
      - Untracked test cases or unanchored assertions fail check-manifest
```

### Task 2.2: Map the 13 Coverage Flags to Test Plan Domains
- **Problem:** The 13 `--*-coverage` flags in `15-operations.md` are completely disconnected from the test plan manifest.
- **Action:**
  Update `cans/15-operations.md#Coverage-gates` to bind each flag directly to manifest domains:
  - `--hook-coverage` & `--action-coverage` -> `domain: plugin_engine`
  - `--lifecycle-coverage` & `--drain-coverage` -> `domain: lifecycle_checkpoint`
  - `--channel-coverage` -> `domain: protocol_mux`
  - `--transport-coverage` -> `domain: virtio_rings`
  - `--snapshot-depth-coverage` -> `domain: lifecycle_checkpoint`
  - `--clock-continuity-coverage` -> `domain: lifecycle_checkpoint` + `plugins/replay`

---

## Phase 3: Schema & Rule Refinement

### Task 3.1: Clean Leaky Schema Variables
- **Problem:** `test-plan-structure.md` contains hardcoded fixture variables: `syscall_42_trapped: false`.
- **Action:**
  Replace with typed schema primitives under `expected_observations.wire`:
  ```yaml
  wire:
    frames_outbound: 0
    frames_inbound: 0
    trapped_syscalls: []
    synthetic_responses: []
  ```

### Task 3.2: Reconcile AS-04 Latency Paradox
- **Problem:** AS-04 bans wall-clock tracking, but the scorecard and cases assert physical `<2µs`, `<8ms`, and `<50ms` bounds.
- **Action:**
  Bifurcate AS-04 in `cans/artifacts/test-plan/matrix.yaml`:
  - **AS-04A (Deterministic Replay Law):** Applies to `unit` and `deterministic` integration runs. Wall-clock sampling, PMU cross-host comparisons, and unseeded randomness are strictly denied. Clock is pinned via pvclock/CNTVOFF_EL2.
  - **AS-04B (Physical Budget Floor Law):** Applies strictly to hardware benchmark suites. Measures host wall-clock and vCPU cycles using host stopwatch/perf hooks. Must enforce thresholds declared in `perf/budgets.toml` with `enforce = true`.

### Task 3.3: Add Target Platform Metadata to Schema
- **Problem:** Hardware tests (KVM, HVF, WHP, AVF) are conflated in `crates/kelir-core` without OS targeting.
- **Action:**
  Update `cans/artifacts/test-plan/test-plan-structure.md` to require `platform_target`:
  ```yaml
  platform_target:
    os: "linux" | "macos" | "windows" | "android" | "any"
    arch: "x86_64" | "aarch64" | "any"
    hypervisor: "kvm" | "hvf" | "whp" | "avf" | "emulated" | "none"
  ```

---

## Phase 4: Manifest & Domain Expansion

### Task 4.1: Add Domain 15 (`adapters_ingestion`)
- **Problem:** `12-adapters.md` is omitted from the test inventory.
- **Action:**
  1. Add domain `adapters_ingestion` (prefix `ad`) to `manifest.yaml`.
  2. Implement 12 missing test cases covering:
     - `ad_01_radix_layer_unpack_whiteout_merge`
     - `ad_02_expansion_ratio_15_to_1_abort` (Zip-bomb defense)
     - `ad_03_single_layer_8gib_ceiling_enforcement`
     - `ad_04_layer_entry_500k_inode_cap`
     - `ad_05_single_pass_erofs_blake3_digest`
     - `ad_06_containerd_shim_v2_mount_lifecycle`
     - `ad_07_containerd_rootless_pod_isolation`
     - `ad_08_mcp_server_stdio_mux_bridge`
     - `ad_09_mcp_console_framing_continuation`
     - `ad_10_openshell_grpc_sandbox_bootstrap`
     - `ad_11_openshell_approval_bridge_suspend`
     - `ad_12_e2b_compat_session_parity`
  3. Total canonical cases updated from 196 to 218 (196 + 12 adapters + 4 concurrency + 6 bindings).

### Task 4.2: Add Missing Concurrency & Fault-Injection Cases
- **Action:**
  Add physical failure test cases to existing domains:
  - `sp_19_host_inode_flock_contention_ewouldblock` (Multi-VM lock race)
  - `sp_20_readdir_cache_invalidation_build_storm` (FUSE write invalidation)
  - `lc_21_detached_scm_rights_parent_sigkill_abort` (Parent dies before handoff ack)
  - `lc_22_sqlite_wal_busy_timeout_recovery` (Concurrent catalog write locks)

---

## Phase 5: Multi-Language Parity Integration

### Task 5.1: Index Language SDK Parity in Manifest
- **Problem:** TS and Python SDK tests run outside the formal test plan.
- **Action:**
  1. Add `bindings_ts` and `bindings_py` case mappings into `manifest.yaml` targeting `packages/ts` and `packages/py`.
  2. Cases must assert:
     - Boot-time `EConfig` on missing dependencies (TypeBox & Pydantic v2).
     - Byte-for-byte execution and receipt equality against `tests/vectors/vectors.json`.
     - Single FFI trampoline execution under chained hooks (AOT hook fusion).

---

## Phase 6: CI/CD Pipeline Alignment

### Task 6.1: Harmonize `justfile` Velocity Targets
- **Problem:** Developer pre-flight cycles cannot run multi-platform hardware tests.
- **Action:**
  Update `justfile` recipes:
  - `just pre-flight`: Runs `check-manifest`, `check-tests` (AST slop linter), `check-spec`, and `test-unit` (pure memory tests, zero hypervisor calls). Must complete in `<60s`.
  - `just test-integration`: Runs local platform-matching hardware tests (Linux KVM or macOS HVF).
  - `just check-scorecard`: Generates scorecard strictly against the platform's executable test subset or reads aggregated CI run outputs.

### Task 6.2: Refactor GitHub Actions Matrix (`.github/workflows/ci.yml`)
- **Problem:** Linux runners attempt to execute macOS/Windows tests or silently fake passes.
- **Action:**
  Structure CI into explicit, partitioned stages:
  1. **Job: Fast Gate (Linux x86):** Lint, AST analysis (`check-tests`), manifest validation (`check-manifest`), documentation and CANS spec check (`cans check --refs-only`).
  2. **Job: Linux KVM Tier (Linux x86):** Executes all cases with `platform_target.hypervisor: kvm` using `cargo nextest`.
  3. **Job: macOS HVF Tier (macOS ARM64):** Executes cases with `platform_target.hypervisor: hvf` (filtered strictly via tag).
  4. **Job: Windows WHP Tier (Self-hosted Windows):** Executes cases with `platform_target.hypervisor: whp`.
  5. **Job: SDK Parity Gate:** Runs `packages/ts` and `packages/py` parity suites against `tests/vectors/vectors.json`.
  6. **Job: Scorecard Aggregator:** Runs only after platform jobs complete, collecting JUnit/JSON test results to verify all 218 invariant cases passed on their respective native targets before writing `scorecard.md`.

---

## Execution Checklist

- [x] `crates/kelir-testkit` created with `check-manifest` and `check-tests` binaries.
- [x] `cans/15-operations.md` updated with 4 inbound pointers and invariant bullets.
- [x] `test-plan-structure.md` cleaned of dummy fields; `platform_target` added to schema.
- [x] `matrix.yaml` updated: AS-04 split into AS-04A and AS-04B; 13 ops coverage gates mapped.
- [x] Domain 15 (`adapters_ingestion`, 12 cases) added to `manifest.yaml` (Total: 218 cases).
- [x] `cases/**/*.yaml` updated to include valid `law_reference` anchors and `platform_target`.
- [x] Multi-language parity test cases added to `manifest.yaml`.
- [x] `.github/workflows/ci.yml` refactored to partition hardware jobs and aggregate the final scorecard.
