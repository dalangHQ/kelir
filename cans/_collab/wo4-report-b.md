# WO4-B report — cans/ spec documents alignment (Agent B, 2026-11-14)

Branch `temp/wo4-b`, base 9c01a66 (= origin/scaffolding). All tasks B1–B5 done. Lane: `cans/` spec documents only — no code, artifact, example, or test-plan file touched. Phase L1/L2 respected: every surface written describes the live L1/L2 tree; L3-landing items are marked "(L3)" in the repo style. Every claim below was verified against the live tree before it became normative text (AGENTS.md §0).

## Per-task outcomes

- **B1 cans/08-lifecycle.md — done (D1, D4, D5, D8).**
  - D1: deleted the stale "Non-conforming sides" block (TS/Py/RS rows). Canonical pin unchanged: `Session.suspend(reason: SuspendReason) / Session.resume()` — verified live in all three SDKs: kelir-engine `session_lifecycle.flow.rs:797/822` (derive `session_dir()/suspend.img`), TS `session.lifecycle.ts:258-266`, Py `lifecycle_session.py:92-108`. Replaced the "vm-handle / native-bridge layer keeps suspend(path)" block with: `VmHandle::suspend(reason, path) / VmHandle::resume(path)` is the low-level primitive, `VmControl::suspend(reason, path)` carries both (kelir-interface `handles_lifecycle.flow.rs:21`), the native session-handle bridge keeps the path form with the fixed reason `External(HumanAuthorizationHold)` (`SDK_SUSPEND_REASON`, bindings napi/pyo3 `handles.flow.rs:67/:47`). Stale line refs `napi/src/lib.rs:70` / `pyo3/src/lib.rs:53` corrected to the files that actually carry the constant (lib.rs files are 29/43 lines).
  - D1 spillover (verified stale, fixed): "Sandbox.suspend() takes no argument" → `Sandbox.suspend(reason)` — all three SDKs take the reason (Rust `sandbox_runtime.flow.rs:364`, Py `lifecycle_sandbox.py:161`, TS `sandbox.runtime.ts:138`).
  - D4: `session.vm.fork() returns full Session` → `session.fork() -> Session returns the full child Session` + leaf "VmHandle has no fork(); fork rebinds plugin state, so it lives on Session only". Verified: `Session::fork()` in crates/kelir `session_lifecycle.flow.rs:137`; `NativeSessionHandle::fork` (napi `handles.flow.rs:561`) and `PySessionHandle::fork` (pyo3 `handles.flow.rs:498`); VmHandle surface (kelir-interface `handles_lifecycle.flow.rs:269-358`) has checkpoint/rollback/suspend/resume/export_snapshot/import_snapshot/adjust_balloon/set_vcpus/map_shared_region/clock — no fork.
  - D5: `StepVerdict::Rollback(reason): calls session.rollback()` → `session.rollback(lineage)` (engine `Session::rollback(lineage)`, `session_lifecycle.flow.rs:712`); added one-hop pointer "Rollback surface set per language: see: 03-sdk-architecture.md#Session-control-surface" under Session and context handles.
  - D8: added "Flush handle reachability: see: 03-sdk-architecture.md#Lifecycle-hook-capability-reachability" under Lifecycle observation (one hop; canonical home stays in cans/03).
- **B2 cans/03-sdk-architecture.md — done (D2, D3, D5, D6, D8).**
  - D2: SSOT-A1 key list `policy` → `repo-policy`, plus leaf "The policy key is 'repo-policy', not 'policy'" (mirrors the existing db-proxy leaf). Verified: plugin named `repo-policy` (RepoPolicyPlugin, fixtures `[plugins."repo-policy"]`, TS `c["repo-policy"]` in typescript-accumulation.ts artifact). Also fixed the Python-SDK example leaf `c.cap.policy` → `c.cap.ledger` (stable across the rename).
  - D3/D5: "Native Engine Planes" Checkpoint leaf now points at the surface table; the Session control surface replaces the single no-arg `session.rollback()` row with "Checkpoint and rollback surfaces | control — Split per language (SSOT-A1)": Rust (Deref contract documented: Session derefs to the engine Session, `session.checkpoint` resolves to `checkpoint: Arc<CheckpointPlane>`; `session.lineage(root_pid) -> PidLineage`; create_step/commit_step/rollback_step/branch_from/restore/head/lineage/compact/monotonic_watermark_ns; `session.rollback(lineage: PidLineage)`; `session.rollback_here(pid: u32)`) and Py/TS (`session.checkpoint.create(label)` one-shot wrapping `VmHandle::checkpoint`; `session.vm.checkpoint(label)` low-level same call; `session.vm.rollback(snapshot)`; `session.rollback(snapshot)` convenience). "The step API is Rust-only"; "No-arg rollback lands with PID tracking (L3)" — wording matches the live binding stubs verbatim. Every Rust signature verified against kelir-engine `planes/checkpoint/plane.flow.rs` + kelir-interface `checkpoint/ops.flow.rs`; Py/TS against napi `NativeCheckpointHandle::create` (delegates to `vm.checkpoint(label)`), TS `session.lifecycle.ts:185-223`, Py `lifecycle_session.py:54/62-67`.
  - D1: suspend rows collapsed — "Rust / TS / Python: Session.suspend(reason: SuspendReason) / resume()" (all three verified identical); deleted the TS/Py non-conforming leaves; `Sandbox.suspend(reason) delegates to session suspend`.
  - D6: "Programmatic exec surface" restructured: Primary `session.spawn(binary, argv)` (EXEC stream kind, exec_id cancellation); Variant `session.exec(binary, argv, limits)` returns `SpawnBuilder`, `SpawnBuilder implements Future<Output = Result<SpawnResult, PluginError>>`, `.await` yields the SpawnResult (verified `impl Future for SpawnBuilder`, crates/kelir `session_lifecycle.flow.rs:331-377`); "Py/TS spawn bridge lands with the shim wire protocol (L3)" (matches the Native/PySessionHandle spawn stubs verbatim). Sandbox facade: `box.exec(binary, argv) delegates to session.spawn(binary, argv)` (verified `Sandbox::exec` → `session.spawn`, all 3 SDKs); `box.rollback() delegates to session rollback: see: 08-lifecycle.md#Rollback`; `box.suspend(reason)`.
  - D8: new node "Lifecycle hook capability reachability" under Service lookup (canonical home): lifecycle hooks receive LifecycleCtx which carries no capability caps (verified: Py `model_plugin.py:84-98` and TS `context.lifecycle.types.ts:104-107` carry set_watchdog/register_veto only); canonical L1/L2 pattern is capture-at-registration (the Rust example's closure capture, `artifacts/examples/db-backed-runtime.rs:121-127`); `ctx.cap.ledger` delivery through the fused lifecycle payload lands (L3).
- **B3 cans/09-snapshots.md — done (D3).** `session.lineage()` → `session.lineage(root_pid)` in the base-snapshot leaf; new "SDK lineage surfaces" node under Groups and lineage: Rust `session.lineage(root_pid) -> PidLineage` (step surfaces see: cans/03) and Py/TS `Session.rollback(snapshot) / VmHandle.rollback(snapshot)` as the lineage-visible surfaces.
- **B4 cans/plugins/ledger.md — done (D7 + flush see:).** New "Causal delta access (ledger)" section: `ledger.get_causal_delta_from_parent() -> CausalDelta` canonical (SSOT-A1 pin); capability-key home `c.cap.ledger` (Py), `c.ledger` (TS), `c.ctx::<LedgerCtx>()` (RS); session-level facade access lands on the bindings (L3) — matches the live `Native/PySessionHandle::get_causal_delta_from_parent` stubs ("causal deltas come from plugin-ledger (L3)") and the Py `LedgerCap.get_causal_delta_from_parent` stub. "Causal deltas are produced by plugin-ledger over the delta chain" (bindings TODO wording). Flush reachability: one-hop `see:` from the Ledger handler API leaf to the cans/03 canonical home.
- **B5 decisions.md — done.** 7 records appended (bottom, append-only; existing entries untouched — note the file's historical ordering is newest-first, a re-sort can happen at convergence): suspend/resume pin, fork pin, checkpoint/rollback per-language split, spawn/exec, repo-policy key, causal delta accessor, flush reachability.

## Evidence (live-tree verification per normative claim)

- Engine `Session`: `lineage(root_pid)` (:603), `rollback(lineage)` (:712, head→restore→rollback_to), `suspend(reason)`/`resume()` (:797/:822, path = `session_dir()/suspend.img`), `checkpoint: Arc<CheckpointPlane>` (:56).
- crates/kelir `Session`: `Deref<Target = kelir_engine::Session>` (:204), `rollback_here(pid)` (:94), `fork()` (:137), `spawn(binary, argv)` (:76) = `exec(binary, argv, None).await`, `exec(...) -> SpawnBuilder` (:81), `impl Future for SpawnBuilder` (:331).
- CheckpointPlane (`plane.flow.rs`) / CheckpointOps (`ops.flow.rs`): create_step, active_step, commit_step, rollback_step, branch_from, restore(node_id, guest_monotonic_ns), head -> Option<u64>, lineage(node_id) -> Vec<u64>, monotonic_watermark_ns, compact — all match the spec rows modulo Result wrappers, per the agreed D3 contract text.
- VmHandle (kelir-interface `handles_lifecycle.flow.rs`): `checkpoint(name)`, `rollback(&SnapshotDescriptor)`, `suspend(reason, path)`, `resume(path)`, export/import, adjust_balloon, set_vcpus, map_shared_region, clock — no fork.
- Bindings: `NativeCheckpointHandle::create(label)` → `vm.checkpoint(label)`; `Native/PySessionHandle::fork()`; spawn/rollback/get_causal_delta stubs carry the exact (L3) wording quoted in the spec; `SDK_SUSPEND_REASON = External(HumanAuthorizationHold)`.
- SDKs: TS `Session.suspend(reason)`/`resume()`/`rollback(snapshot?)`/`checkpoint.create`/`vm.checkpoint`/`vm.rollback`; Py identical shape; Sandbox suspend(reason)/resume()/exec(binary, argv) in all three.
- Flush reachability: Py LifecycleCtx and TS LifecycleContext carry no caps; Rust capture pattern at `db-backed-runtime.rs:121-127` (`ledger_ctx.clone()` into the `on_pre_commit` closure).

## Anchor changes (prominent — anchors.json regeneration needed)

No code `// see:` comment, spec node, or artifact references any of the changed/deleted leaf anchors (grepped the whole tree; only `crates/kelir-testkit/anchors.json` lists them). All section-level anchors that code targets are preserved: `#Session-control-surface`, `#Session-surfaces`, `#Native-Engine-Planes-on-Session`, `#Sandbox-high-level-facade`, `#Suspend-and-resume`, `#Rollback`, `#Multiplexed-binary-RPC-for-SDK-consumers...` (parent text untouched), `#Invariants-and-policy-(ledger)`, `#Plugin-ledger`, `#Merkle-WAL-specification`, `#Groups-and-lineage`.

Changed/deleted leaf anchors (old form → outcome):
- cans/03: `Keys: ledger, db-proxy, policy, ...` → repo-policy form; `Checkpoint: session.checkpoint.create(label), session.rollback()` → pointer leaf; `session.rollback() | control` + `no socket-preserving flag...` (deleted, subsumed); TS/Py suspend spelling + 3 non-conforming leaves (deleted); `Sandbox.suspend() takes no argument in all SDKs` + `It delegates to session suspend` → single reason-form leaf; `Programmatic exec surface: session.exec(cmd, limits?)` → `Programmatic exec surface`; `box.exec(cmd)...`, `box.rollback() invokes driver rollback...`, `box.suspend() takes no argument` → new forms; `Capabilities are static members of c.cap (c.cap.policy), never` → `(c.cap.ledger)`.
- cans/08: `session.vm.fork() returns full Session` → session.fork() form (+ new child leaf); `The vm-handle / native-bridge layer keeps suspend(path)` + `And resume(path)...` → VmHandle low-level primitive leaves; two `See: .../lib.rs:NN` leaves → `handles.flow.rs` forms; `Non-conforming sides` + 8 children (deleted); `Sandbox.suspend() takes no argument and delegates...` → reason form; `StepVerdict::Rollback(reason): calls session.rollback(); ...` → lineage form.
- cans/09: `Not visible in session.lineage(); ...` → `session.lineage(root_pid)` form.

New anchors created (all resolve; my new `see:` refs target them): cans/03 `#Lifecycle-hook-capability-reachability`, `#Checkpoint-and-rollback-surfaces-|-control`; cans/09 `#SDK-lineage-surfaces`; ledger.md `#Causal-delta-access-(ledger)`, `#Reachability-from-lifecycle-hooks...`. **Follow-up: `crates/kelir-testkit/anchors.json` needs a regeneration after merge (same class as WO3-A's A1 tool-guard leaf).**

## Gates (run)

- `bash scripts/check-canon.sh` — **OK** ("Corpus is 100% Day-1 canonical"), run after every task and at the end.
- Every `see:` I added or kept resolves (grepped each target leaf in the target file): cans/08 → cans/03 `#Session-control-surface` (:122), `#Lifecycle-hook-capability-reachability` (:41), `#Sandbox-high-level-facade` (:252); cans/03 → in-file `#Session-control-surface`, `08-lifecycle.md#Rollback` (:116) / `#Suspend-and-resume` (:181) / `#Hypervisor-primitives` (:2), `artifacts/examples/db-backed-runtime.rs` (exists); cans/09 → cans/03 `#Session-control-surface`; ledger.md → cans/03 `#Lifecycle-hook-capability-reachability`. No deep hops added (all one hop; the cans/03 canonical homes carry no onward `see:` on these concepts).
- Anchor-safety grep across the tree for every changed/deleted leaf anchor: zero references outside `anchors.json` (see above).
- `cans` CLI is NOT installed in this sandbox — **cans check deferred to CI** (same for `just`).
- `git diff --stat 9c01a66..HEAD` — see Files changed.

## Files changed (9c01a66..HEAD, before this report)

```
 cans/03-sdk-architecture.md | 69 +++++++++++++++++++++++++++--------------
 cans/08-lifecycle.md        | 28 ++++++++----------
 cans/09-snapshots.md        |  5 +++-
 cans/_collab/decisions.md   |  7 +++++
 cans/plugins/ledger.md      |  7 +++++
 5 files changed, 77 insertions(+), 39 deletions(-)
```

## Out-of-scope observations for the orchestrator / other lanes

1. **Examples lane (WO4-D):** `autonomous-swe-agent.ts:210` / `multi-vm-swarm.ts:64` still call `session.vm.fork()`; `autonomous-swe-agent.py:259` still reads `c.cap.policy.violations` — work-order items 2/4 assign those example fixes.
2. **Artifact divergence:** `cans/artifacts/code/python-raii-pydantic.py:30,39` still demonstrates `c.cap.policy`; `cans/artifacts/code/plugin-trait.rs:361` doc-comment still says `session.vm.fork()`. Artifacts are SSOT content outside my lane — reviewer/artifacts pass needed to match the repo-policy and session.fork() pins.
3. **Out-of-file spec stragglers:** `cans/10-attestation.md:162` ("immediate session.rollback() to slot base") still uses the bare no-arg shorthand; not in my four-file lane, one-leaf fix if desired.
4. **Rust lane (WO4-A):** work-order item 3/10 suggests an inherent `Session::lineage(pid)` on crates/kelir `Session` — today it resolves through `Deref`; the spec's Deref-contract leaf is written to stay accurate either way.
5. **anchors.json regeneration** after merge (see Anchor changes).

## Push log

- 653ab08 WO4-B: cans/08 lifecycle (B1) — pushed
- d396309 WO4-B: cans/03 SDK architecture (B2) — pushed
- b5779d6 WO4-B: cans/09 snapshots (B3) — pushed
- fa3023d WO4-B: cans/plugins/ledger (B4) — pushed
- 864e4c7 WO4-B: decisions (B5) — pushed
- (this report commit follows)
