# Coverage audit: `cans/artifacts/examples`

Short answer: **no — plugin coverage is satisfied, but hook, action, and scenario coverage are not.**

The coverage invariant in `02-plugin-model.md` is per‑gate, not "one example per plugin". Each gate has its own enumeration requirement. Let me go through them against what actually exists.

---

## What is already covered

### `--plugin-coverage` — **satisfied**

Every community plugin in `cans/plugins/` resolves to at least one example:

| Plugin | Demonstrating example |
|---|---|
| chaos | `autonomous-swe-agent.{rs,ts,py}` |
| container | `container.py` |
| db-proxy | `db-backed-runtime.py` |
| ledger | `autonomous-swe-agent.{rs,ts,py}` |
| mcp | `serverless-webhook.ts` |
| mesh | `multi-vm-swarm.rs` |
| profile | `profile.rs` |
| replay | `replay.rs` |
| supply-chain | `multi-vm-swarm.rs` |
| tool-guard | `serverless-webhook.ts` |
| transaction | `db-backed-runtime.py` |
| vault | `autonomous-swe-agent.{rs,ts,py}` (wire-plane `vaults` only) |
| wire-ingress | `serverless-webhook.ts` |

The commercial set (`hyper-density`, `cas-snapshot`, `oci-translator`, `openshell-coordinator`) is proprietary and exempt by construction.

### `--veto-coverage` — **satisfied**

`VetoEvent::{Fork, Suspend, ClockSkew}` are all registered:
- `Fork` — `multi-vm-swarm.rs` (`builder.lifecycle_mut().register_veto(VetoEvent::Fork, …)`)
- `Suspend`, `ClockSkew` — `policy-matrix.{rs,ts,py}` in `on_vm_init`

### `--watchdog-coverage` — **satisfied**

`WatchdogVerdict::{ExecuteDefault, Suppress, Mitigate}` all appear in `policy-matrix.{rs,ts,py}` (Mitigate in `PolicyMatrixPlugin`, Suppress in `WatchdogSilencerPlugin`, ExecuteDefault in the fall-through).

### Verdict coverage (most of it) — **partial**

Present: `Pass`, `Patch(ExecveMutation)` (git-gate, policy-matrix, autonomous-swe-agent), `Patch(FsInode)` (hook-matrix), `Patch(Custom)` (policy-matrix), `Drop`, `Suspend(Deadline)`, `Suspend(Approval)`, `Abort`.

---

## What is NOT covered

### 1. `--hook-coverage` — **materially incomplete**

The trait list in `02-plugin-model.md#Segregated-ring-traits` declares this surface. Unimplemented hooks in any example:

| Hook | Home trait | Status |
|---|---|---|
| `on_fs_lock` | `FsInterposer` | missing |
| `on_net_inbound` | `NetInterposer` | missing (`hook-matrix` only covers outbound) |
| `on_exec_stdout` | `ConsoleInterposer` | missing |
| `on_exec_stderr` | `ConsoleInterposer` | missing |
| `on_signal` | `TrapInterposer` | missing |
| `on_port` | `TrapInterposer` | missing |
| `on_window_resize` | `ConsoleInterposer` | missing |
| `on_disclosure` | `ConsoleInterposer` | missing |
| `on_memory_pressure` | `SystemTelemetryParticipant` | missing |
| `on_host_preempt` | `LifecycleParticipant` | missing |
| `on_vm_shutdown` | `LifecycleParticipant` | TODO in `autonomous-swe-agent.*` — never actually wired |

### 2. `--action-coverage` — **one gap**

`FilterAction::Abort { reason }` appears in `02-plugin-model.md#Unified-verdict` but is not exercised anywhere in `examples/`. `Verdict::Abort` is used (policy-matrix), but the filter-ring variant is not.

### 3. `--unified-builder-coverage` — **partial**

`fluent-builder-chaos.{rs,ts,py}` only exercises `require`, `scope`, `fs`, `net`. The unified builder pins **30 verbs + 8 scopes + 8 merge cases + 4 chain cases** (see `02-plugin-model.md#Unified-builder`). Uncovered in examples at minimum:

- Verbs: `context`, `fork_with`, `vsock_service`, `on_trap` (scope form `trap_scope`), `on_host_preempt`, `on_memory_pressure`, `on_pre_commit`, `on_post_commit`, `on_vm_shutdown`
- Scopes: `trap`, `console`, `exec`, `host`, `lifecycle`, `telemetry`
- Merge cases: host descriptor ordering, plugin scope propagation, nested scope = flat chain equivalence

The live count per `15-operations.md#Unified-builder-parity-vectors` is 58 vectors; 50 builder cases. Examples currently demonstrate maybe a dozen of those.

### 4. Scenario coverage — **absent**

`scorecard.md` lists 36 scenarios (`sc_lifecycle_*`, `sc_agent_*`, `sc_concurrency_*`, `sc_recovery_*`, `sc_pressure_*`, `sc_network_*`, `sc_parity_*`, `sc_smoke_*`) but `cans/artifacts/examples/` contains **no scenario files** and the referenced `cans/artifacts/test-plan/{manifest,scenarios,matrix,case-to-test-map}.yaml` are not in the tree. The scorecard is currently computed from files that do not exist.

### 5. Tri-language parity on plugin examples

The spec says "Same first-class plugins across Rust, TypeScript, Python (1:1)" and parity is gated on `vectors.json`. The examples are not 1:1:

- Python-only: `container.py`, `db-backed-runtime.py`
- Rust-only: `git-gate.rs`, `multi-vm-swarm.rs`, `profile.rs`, `replay.rs`
- TS-only: `serverless-webhook.ts`
- All three: `autonomous-swe-agent.*`, `hook-matrix.*`, `policy-matrix.*`, `fluent-builder-chaos.*`

That's not strictly a coverage-gate failure (the gate only names `vectors.json` and the AST manifest), but it is a parity gap in the examples corpus and worth closing if you're using examples to generate `vectors.json` cases.

---

## What I would add

Below is the minimum file set to make each gate provably green, plus scenarios. I've separated "gate-closing" from "parity-closing" from "scenario harness".

### A. Close `--hook-coverage` (gate blocker)

Two options. I'd do option 2 because it keeps `hook-matrix` as the single canonical surface example.

**Option 1 — extend `hook-matrix.{rs,ts,py}`** with the missing hooks:

- `on_fs_lock` — exercise `SETLK` vs `SETLKW`, and `EWOULDBLOCK`
- `on_net_inbound` — return `FilterAction::Drop { errno: None }` for a marker packet (chaos-adjacent)
- `on_exec_stdout` / `on_exec_stderr` — tail `EXEC` stream chunks and mask tokens
- `on_signal` — observe a `SIGTERM` forwarded through Channel 0x0000
- `on_port` — observe console port open/close
- `on_window_resize` — observe a `PTY_CONTROL` `MsgType 0x03`
- `on_disclosure` — annotate a host mutation
- `on_memory_pressure` — record `pressure_level_typed`
- `on_host_preempt` — record a `SIGTERM` to the host and re-raise
- `on_vm_shutdown` — replace the TODOs in `autonomous-swe-agent.*` with a real handler, or move shutdown handling into `hook-matrix` and drop the TODOs

**Option 2 — new `hook-matrix-full.{rs,ts,py}`** — a superset of `hook-matrix` that contains every hook in the Segregated-ring-traits list. `hook-matrix` stays the narrow demonstration; `hook-matrix-full` becomes the `--hook-coverage` oracle. This avoids inflating the already-dense `hook-matrix` and keeps the file under the 1,000-LOC ceiling (`01-architecture.md#Codebase-invariants`).

Files to add under option 2:
```
cans/artifacts/examples/hook-matrix-full.rs
cans/artifacts/examples/hook-matrix-full.ts
cans/artifacts/examples/hook-matrix-full.py
```

### B. Close `--action-coverage` (gate blocker)

One addition anywhere that already owns a filter ring. Cleanest: add to `hook-matrix.{rs,ts,py}`'s `on_net_inbound` a `FilterAction::Abort { reason }` branch, or drop it into `fluent-builder-chaos.*`'s `on_net_outbound` path.

No new file needed; one-line branch per language.

### C. Close `--unified-builder-coverage` (gate blocker for the 58-vector target)

The examples that exercise the builder surface end-to-end are `fluent-builder-chaos.*`, but they're pinned to the *legacy* DX shape ("No `build()`: the plugin-mode builder is mounted as-is"). The unified builder contract requires host-mode *and* plugin-mode for every verb.

Add one builder-surface example per language that walks the full verb set:

```
cans/artifacts/examples/unified-builder-surface.rs
cans/artifacts/examples/unified-builder-surface.ts
cans/artifacts/examples/unified-builder-surface.py
```

Each walks:
- structural: `context`, `require(s)`, `scope`, `fork_with`, `vsock_service`
- 24 ring verbs (one handler each, delegating to a trivial `Forward`/`Pass`)
- 8 scope closures (`trap`, `fs`, `net`, `console`, `exec`, `host`, `lifecycle`, `telemetry`)
- the four chain cases: `seal → trap filter → trap_scope → lock_broker`
- the four mount cases: host-only verbs on a host builder, host-only verbs on a plugin builder (expecting `EConfig` in TS/Py, compile error in Rust), `host.use(builder)` merge ordering, nested-scope = flat-chain equivalence

That's the file `--unified-builder-coverage` looks for. It also becomes the generator source for `vectors.json`'s 50-case `plugin_builder` family, replacing the historical 38-case family flagged in `15-operations.md#Fluent-builder-parity-vectors`.

### D. Close scenario coverage (the bulk of the work)

The 36 scenarios in `scorecard.md` are *composition* tests — the scorecard states "each step references a domain case_id (non-redundancy contract, CDR-09..CDR-12)". They are YAML-defined, not example programs. The two missing pieces:

**D1 — the test-plan artifacts** (referenced but absent from the tree):
```
cans/artifacts/test-plan/manifest.yaml
cans/artifacts/test-plan/scenarios.yaml
cans/artifacts/test-plan/matrix.yaml
cans/artifacts/test-plan/case-to-test-map.yaml
cans/artifacts/test-plan/test-plan-structure.md
```
`scorecard.md` is currently computed from files that do not exist; until these land, `just check-scorecard` cannot pass.

**D2 — a scenario runner per language** so the YAML steps execute against real examples. One runner per language, not one file per scenario:

```
cans/artifacts/examples/scenario-runner.rs
cans/artifacts/examples/scenario-runner.ts
cans/artifacts/examples/scenario-runner.py
```

The runner:
- reads `scenarios.yaml`
- for each scenario, resolves each `step` to the example that owns that case_id (`case-to-test-map.yaml`)
- invokes the example's exported `run_scenario(step)` with a typed step descriptor
- diffs the observed `ExecutionReceipt` against the pinned expectation
- emits a per-scenario `pass|fail|not-executable` record that `scorecard.md` consumes

If you genuinely want one file per scenario, that's 36 × 3 = 108 files — I'd argue against it. The runner plus `scenarios.yaml` matches the "0-hop reference" discipline (`artifacts/README.md`) and avoids duplicating composition logic in 108 places. The 36 scenarios decompose into 8 tier families; one runner per language plus the YAML gives the same coverage with a tenth of the files.

If you insist on per-scenario files, at minimum add them for the families that currently have **zero** example coverage:
```
sc_lifecycle_full_roundtrip.rs / .ts / .py
sc_lifecycle_fork_from_mid_chain.rs / .ts / .py
sc_agent_mcp_tool_call_roundtrip.ts        # mcp path exists; rust/py missing
sc_concurrency_n_sessions_isolation.rs / .ts / .py
sc_recovery_host_crash_reattach.rs / .ts / .py
sc_pressure_sustained_memory_pressure_converges.rs / .ts / .py
sc_network_park_and_suspend_webhook.ts     # others missing
sc_parity_full_roundtrip.rs / .ts / .py
sc_smoke_boot_exec_shutdown.rs / .ts / .py
```
That's already ~25 new files before you get to the rest of the matrix; the runner approach is materially cheaper.

### E. Close tri-language parity for the examples (optional, recommended)

If you want `vectors.json` generation to be exercised by example programs rather than only by `plugin-builder-vectors.rs` fixtures, port the missing plugin demos:

```
cans/artifacts/examples/container.rs
cans/artifacts/examples/container.ts
cans/artifacts/examples/db-backed-runtime.rs
cans/artifacts/examples/db-backed-runtime.ts
cans/artifacts/examples/multi-vm-swarm.ts
cans/artifacts/examples/multi-vm-swarm.py
cans/artifacts/examples/profile.ts
cans/artifacts/examples/profile.py
cans/artifacts/examples/replay.ts
cans/artifacts/examples/replay.py
cans/artifacts/examples/serverless-webhook.rs
cans/artifacts/examples/serverless-webhook.py
cans/artifacts/examples/git-gate.ts
cans/artifacts/examples/git-gate.py
```

This is 14 new files. Note that some are currently marked partial in the spec — e.g. `db-backed-runtime.py` is explicitly called out as non-conforming on the capability key (`c.cap.db` vs the canonical `db-proxy` key, per `plugins/db-proxy.md#Key-pin-(SSOT-A1)`). Porting them is the moment to fix those.

---

## Summary table

| Gate | Currently green? | Files to add |
|---|---|---|
| `--plugin-coverage` | yes | — |
| `--veto-coverage` | yes | — |
| `--watchdog-coverage` | yes | — |
| `--hook-coverage` | **no** | `hook-matrix-full.{rs,ts,py}` or extend `hook-matrix.*` |
| `--action-coverage` | **no** (`FilterAction::Abort`) | one branch in an existing filter ring |
| `--unified-builder-coverage` | **no** | `unified-builder-surface.{rs,ts,py}` |
| Scenario coverage | **no** (files absent) | `test-plan/{manifest,scenarios,matrix,case-to-test-map}.yaml` + `scenario-runner.{rs,ts,py}` |
| Tri-language parity (examples) | partial | the 14 port files above |

**Minimum gate-closing set:** 1–2 new example files (`hook-matrix-full` or extended `hook-matrix`, plus `unified-builder-surface`), one branch for `FilterAction::Abort`, and the test-plan YAML + runner. That's roughly **7–8 new files**, not 108.

**Recommended full set:** add the 14 parity ports on top, giving ~21 new files and 5–10 edits to existing ones.
