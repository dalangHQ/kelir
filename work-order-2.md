# What to steal from microsandbox's test discipline for kelir

## 1. Diagnosis: what kelir's plan already gets right, and where it structurally plateaus

Kelir's current plan is **physical-invariant-first**: 254 cases across 17 domains, each pinned to a host-level physical observable (a punch-holed sparse extent, an `st_blocks == 0`, a typed `Verdict::Drop`, a BLAKE3 literal). The Anti-Slop Laws (AS-01..AS-08) and Case Discipline Rules (CDR-01..CDR-08) are stronger than anything microsandbox ships — do not dilute them.

What the plan structurally **cannot** express today, and what microsandbox's suite does express naturally:

| Kelir has | Kelir lacks (microsandbox-shaped) |
|---|---|
| Per-subsystem invariants (one plane each) | **Composition**: one user journey asserting 4 planes together |
| Single-session tests | **Concurrency**: N sessions contending for host resources |
| Steady-state tests | **Recovery**: host crash → reaper → catalog state → reattach |
| One-shot executions | **Sustained pressure**: balloon/disk quota over minutes |
| Binding parity via `vectors.json` frames | **Full E2E SDK journeys** across all 3 languages |
| Tool-guard interception unit/red tests | **MCP protocol conformance** (full JSON-RPC round-trip) |
| Red/gap map with 75 gaps | **Smoke tier**: a fast “does the whole stack still boot” gate |

The whole point of the answer below: **add a scenario tier that composes domain invariants** — kelir's equivalent of microsandbox's `tests/integration/python/test_sandbox.py` — without re-asserting a single physical fact that a domain case already owns.

---

## 2. Structure to steal: a `scenarios/` tier that composes, never re-asserts

Microsandbox's biggest structural asset is that its top-level test files are **user journeys**, not subsystems. Steal the shape, not the file names.

### 2.1 New tier, new schema, same anti-slop laws

```
cans/artifacts/test-plan/
  cases/                    ← existing 254 domain cases (unchanged)
  scenarios/                ← NEW: composition tier
    lifecycle/
    agent/
    concurrency/
    recovery/
    pressure/
    network/
    parity/
  scenarios.yaml            ← NEW manifest (mirrors manifest.yaml shape)
  test-plan-structure.md    ← extend with §5 Scenario Discipline
```

Every scenario file uses a **new schema `test-plan/scenario/v1`** whose body is *an ordered chain of `case_id` references with per-step assertions*. Example:

```yaml
$schema: "test-plan/scenario/v1"
scenario_id: "sc_lifecycle_full_roundtrip"
title: "Boot → exec → write file → egress → checkpoint → rollback → resume → stop"
target_tier: e2e
languages: [rust, typescript, python]   # parity matrix, not one language
fixture_ref: fixtures/scenarios/full-roundtrip/   # EROFS + kernel + config
steps:
  - ref: tp_09_exec_stream_sublane_stdio_fidelity
    assert: "stdout carries literal bytes [0x48,0x65,0x6c,0x6c,0x6f]"
  - ref: sp_04_storage_quota_enforcement_enospc
    assert: "upper.img st_blocks stays 0"
  - ref: wp_02_synthetic_dns_nxdomain_enforcement
    assert: "guest observes NXDOMAIN for undeclared host"
  - ref: lc_02_suspend_zero_rss_purge
    assert: "host RSS == 0MB after suspend"
  - ref: lc_09_graceful_shutdown_syncfs_flushed
    assert: "on_vm_shutdown fired; ExitKind::Normal"
anti_slop_assertions:
  - rule: AS-03
    verification: "no .is_ok() — each step matches a typed Verdict/FilterAction"
  - rule: AS-02
    verification: "rollback step asserts st_blocks==0 on upper.img"
```

Key invariants — these are what make it **cohesive and non-redundant**:

- **A scenario may only reference `case_id`s, never invent physical assertions.** If you need a new physical fact, it goes in the domain case first, then the scenario references it. This closes the loop: scenario failures localize to a single domain case.
- **Each scenario step carries one assertion only** (the composition contract), which is the `assert:` string. The domain case owns the *full* proof.
- **`ci_gates.check_manifest` extended**: every `ref:` in `scenarios/` must resolve to a `case_id` in `manifest.yaml`; the scenario tier cannot drift.
- **AS-04A applies unchanged**: no wall-clock sampling in scenarios; use `StepVerdict` / instruction counts.

Microsandbox proves a scenario-level fixture sharing model (`tests/fixtures/` reused across languages). Adopt it explicitly:

```
cans/artifacts/test-plan/fixtures/scenarios/
  full-roundtrip/         EROFS + kernel + kelir.yaml
  mcp-agent-loop/         MCP client stdin + expected stdout fixtures
  network-policy/         CIDR/domain allowlist JSON
  crash-recovery/         pidfile + catalog seed row
```

Fixtures are **read-only from kelir's POV** (a fixture is a test input, not a spec artifact). They live under `cans/artifacts/test-plan/`, not under `cans/`, matching the artifact SSOT split.

---

## 3. Cases to steal, mapped to existing domains

Below are microsandbox-style scenarios grouped by what they add. Each bullet names the existing kelir domain cases it composes. **None of these is a new physical invariant** — they are user-journey compositions, so they cannot duplicate a domain case.

### 3.1 `scenarios/lifecycle/` — full lifecycle round-trips (fills the biggest gap)

Microsandbox's `test_sandbox_lifecycle` runs boot → exec → files → network → suspend → stop in one pass. Kelir has every step isolated but never together.

| Scenario | Composes |
|---|---|
| `full_roundtrip` | `tp_09`, `sp_04`, `wp_02`, `lc_02`, `lc_09` |
| `fork_from_mid_chain` | `lc_01`, `lc_19`, `pe_13`, `ar_01` |
| `rollback_recovers_state` | `sp_01`, `wp_03`, `gv_06`, `lc_08` |
| `suspend_resume_clock_snap` | `lc_02`, `lc_05`, `lc_11` |
| `graceful_vs_abrupt_shutdown` | `lc_09`, `lc_10`, `ta_03` |

### 3.2 `scenarios/agent/` — MCP end-to-end (microsandbox's core strength)

Kelir has `gt_01..gt_12` for tool-guard and `ad_08/ad_09` for MCP framing, but no **end-to-end MCP round-trip** from a real client. Microsandbox runs this as the primary E2E.

| Scenario | Composes |
|---|---|
| `mcp_tool_call_roundtrip` | `gt_01`, `gt_07`, `ad_08`, `ad_09`, `tp_09` |
| `mcp_taint_blocks_mutation` | `gt_03`, `gt_12`, `gt_11` |
| `mcp_schema_drift_halts` | `gt_02`, `gt_08` |
| `mcp_tool_quota_exhaustion` | `gt_04`, `gt_10` |
| `agent_loop_with_egress_denial` | `wp_02`, `gt_03`, `ta_04` |

### 3.3 `scenarios/concurrency/` — the largest true gap

Microsandbox tests multiple sandboxes sharing host resources. Kelir's plan has **zero multi-session cases**.

| Scenario | Composes |
|---|---|
| `n_sessions_isolation` | `sp_12`, `wp_01`, `lc_16` (all N times, asserting no cross-talk) |
| `shared_workspace_lock_contention` | `sp_15`, `sp_19` (two sessions, one flock) |
| `concurrent_catalog_writes` | `sp_21`, `sp_22`, `lc_22` |
| `port_forward_no_collision` | `wp_14`, `wp_15` (parallel binds) |
| `balloon_pressure_under_n_sessions` | `hd_16`, `hd_17` (co-timed via instruction stepping, not sleep) |

### 3.4 `scenarios/recovery/` — crash and reattach (kelir's detached path is under-tested)

Microsandbox tests server restart with running sandboxes. Kelir has `lc_12`, `lc_13`, `lc_14`, `lc_15`, `lc_21` in isolation, never chained.

| Scenario | Composes |
|---|---|
| `host_crash_reattach` | `lc_12`/`lc_13`, `lc_15`, `lc_16` |
| `detached_lease_expiry_sweep` | `lc_14`, `lc_21`, `sp_22` |
| `catalog_corruption_fail_closed` | `lc_22`, `sp_21` |
| `partial_scm_rights_abort` | `lc_21`, `lc_16` |

### 3.5 `scenarios/pressure/` — sustained resource behavior

Microsandbox soaks. Kelir's plan asserts quota denial, never sustained pressure recovery.

| Scenario | Composes |
|---|---|
| `sustained_memory_pressure_converges` | `hd_16`, `hd_17`, `lc_05` |
| `sustained_disk_pressure_quota_holds` | `sp_04`, `sp_05`, `ta_07` |
| `sustained_wal_growth_rotates` | `ta_01`, `ta_06`, `ta_07` |
| `sustained_wire_udp_reaper` | `wp_06`, `wp_14` |

### 3.6 `scenarios/network/` — full policy matrix (microsandbox's network tests are more complete)

Kelir tests each wire mechanism in isolation (`wp_01..wp_17`), but never the **policy matrix** — a single guest making N calls of each class.

| Scenario | Composes |
|---|---|
| `allow_deny_matrix` | `wp_01`, `wp_02`, `wp_07`, `wp_08` |
| `dns_split_horizon_under_policy` | `wp_09`, `wp_10`, `wp_11` |
| `l4_diversion_with_rollback` | `wp_12`, `wp_03`, `wp_16` |
| `park_and_suspend_webhook` | `wp_17`, `lc_02`, `lc_06` |

### 3.7 `scenarios/parity/` — full SDK journeys, not binding vectors

This is the **highest-value steal**. Kelir's `bt_*`/`bp_*` prove binding-level frame parity. Microsandbox proves the *whole journey* is identical across Python/TS/Rust. Extend this idea.

| Scenario (each run in all 3 languages) | Composes |
|---|---|
| `parity_full_roundtrip` | same as `full_roundtrip` above, ×3 languages |
| `parity_error_surface` | `pe_12`, `gt_09`, `gv_02`, `gv_03` (assert exact error strings identical) |
| `parity_session_lifecycle_verbs` | `lc_09`, `lc_10`, `lc_16` (identical timing-independent behavior) |
| `parity_plugin_mount_surface` | `pe_07`, `bt_03`, `bp_03` (identical fused descriptor) |

Microsandbox's error-surface consistency test (`test_error_handling_consistency`) is exactly the shape kelir's AST surface manifest diff should have: **for every error path, the user sees the same typed error across languages**. Add it as `parity_error_surface` and gate it in CI.

### 3.8 Smoke tier — the missing fast gate

Microsandbox has a **smoke suite** (a handful of E2E tests that run in <2 min and catch ~90% of integration breakage). Kelir's plan has no smoke tier — every check is either a domain case or a full manifest CI gate.

Add `scenarios/smoke/` with exactly four scenarios:

| Smoke | Composes | Budget |
|---|---|---|
| `smoke_boot_exec_shutdown` | `tp_09`, `lc_09` | <10s |
| `smoke_mcp_roundtrip` | `gt_01`, `ad_08` | <15s |
| `smoke_rollback_clean` | `sp_01`, `wp_03` | <10s |
| `smoke_catalog_reattach` | `lc_14`, `lc_15` | <10s |

CI runs `scenarios/smoke/` on every PR before the full manifest job. This catches breakage early without spending CI minutes on 254 cases.

---

## 4. Non-redundancy rules (the cohesion contract)

Codify these in `test-plan-structure.md` §5 (new scenario discipline) so nobody slides into duplication:

1. **No scenario may assert a physical fact absent from a domain case.** If a scenario wants `upper.img st_blocks == 0`, that fact is owned by `sp_01`. The scenario's `assert:` is a *reference* to the fact, not a re-definition. `ci_gates.check_manifest` rejects a scenario whose assertion string does not appear (verbatim or as a typed reference) in its referenced domain case.

2. **Every scenario step must be a `ref:` to an existing `case_id`.** Scenarios cannot have free-form setup. (Fixture setup is allowed; step bodies are not.)

3. **Scenarios are tier-`e2e` only.** They cannot satisfy unit or integration coverage.

4. **Scenario count budget:** one scenario per user journey, per language where applicable, per plane-composition. Target **≤ 40 scenarios total** across all domains. This is the non-redundancy ceiling; a 41st scenario must justify itself by composing a pair of planes no other scenario does.

5. **Scenario runtimes do not enter the scorecard's per-domain `pass` counts.** They get their own table:

```
| Scenario Tier | Count | Status |
| lifecycle | 5 | pass |
| agent | 5 | pass |
| concurrency | 5 | pass |
| recovery | 4 | pass |
| pressure | 4 | pass |
| network | 4 | pass |
| parity | 4 | pass |
| smoke | 4 | pass |
```

---

## 5. What *not* to steal

Be explicit so nobody over-copies:

| Microsandbox pattern | Why not |
|---|---|
| Docker-container-based test environment | Kelir's physical boundaries are hardware; mock driver banned by AS-01 |
| `test_cli_*` suite | Kelir ships no runner CLI by design (`15-operations.md#Developer-CLI`) |
| Pool/scale-out tests | Kelir has no pool concept; sessions, not pools |
| `test_sandbox_performance` wall-clock thresholds | AS-04A bans wall-clock; budgets live in `perf/budgets.toml` via AS-04B |
| Per-image `test_oci_*` | `ad_01..ad_05` already own kelir-pack ingestion |
| Python-first test ordering | Kelir is symmetric: `scenarios/parity/` runs all 3 languages in lockstep |

---

## 6. Implementation path (concrete)

1. **Add `scenarios/` directories** with the 8 scenario families above (~35 files).
2. **Extend `test-plan-structure.md`** with §5 “Scenario Discipline” (CDR-09 through CDR-12: the four non-redundancy rules above) and the new `test-plan/scenario/v1` schema.
3. **Extend `manifest.yaml`** with a `scenario_domains:` block mirroring `domains:` but for composition.
4. **Add `ci_gates.check_scenarios`** to `matrix.yaml`:
   - Verifies every scenario `ref:` resolves to a live `case_id`.
   - Verifies every scenario `assert:` string is a substring of the referenced case's `statement` or `expected_observations` — this is the hard non-redundancy gate.
5. **Add `scenarios/smoke/` to the pre-PR CI job** with a 60-second wall budget.
6. **Update `scorecard.md`** with the new “Scenario Tier” table and a “Coverage composition matrix” showing which planes each scenario composes.
7. **Add a `--scenario-coverage` gate** (kelir-only flag) asserting that every pair of adjacent planes (terminal×storage, storage×wire, wire×checkpoint, checkpoint×host) is exercised by at least one scenario.

That last gate is the *cohesive* enforcement: it makes “no two planes are tested together” a CI failure, which is exactly the structural hole microsandbox would fill if kelir were derived from it.
