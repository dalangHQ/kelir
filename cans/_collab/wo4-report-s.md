# WO4-S Report — spec alignment (Agent S, temp/s)

Scope: `cans/*.md` + `cans/plugins/*.md` only. Code is SSOT for signatures; every
signature below was verified against the live tree (read-only) this session.

## S1 — suspend/resume pin (cans/08-lifecycle.md)

- Deleted the "Non-conforming sides" block (TS/Py/RS claims were stale — all three
  SDKs already conform to `Session.suspend(reason: SuspendReason)` / `Session.resume()`).
- Canonical block now documents:
  - `Session.suspend(reason: SuspendReason)` / `Session.resume()` — all three SDKs conform.
  - `VmHandle::suspend(reason, path)` / `VmHandle::resume(path)` as the intentional
    low-level primitive.
  - napi/pyo3 binding handles keep `suspend(path)`/`resume(path)` with the fixed
    SDK reason `External(HumanAuthorizationHold)`; fixed stale file refs
    (`napi/src/lib.rs:70` → `napi/src/handles.flow.rs`, same for pyo3).
  - `Sandbox.suspend(reason)` (was wrongly "takes no argument").
- Also fixed the Android LMK line: `session.suspend(ResourcePressure)` (no-arg form gone).
- Follow-up in cans/03-sdk-architecture.md (commit 869fa90): deleted the duplicated
  stale per-SDK spelling sub-block (canonical pin lives in 08) and fixed
  `Sandbox.suspend()` claims there too.
- Verified: engine `Session::suspend(reason)` / `Session::resume()`
  (crates/kelir-engine/src/session_lifecycle.flow.rs:797,822 — facade reaches them
  via Deref); Py `Session.suspend(reason)`/`resume()` (packages/py/kelir/lifecycle_session.py:92-108);
  TS `Session.suspend(reason)`/`resume()` (packages/ts/core/src/session.lifecycle.ts:258-266);
  `VmHandle::suspend(reason, path)` (crates/kelir-interface/src/handles_lifecycle.flow.rs:318-324);
  `Sandbox::suspend(reason)`/`resume()` (crates/kelir/src/sandbox_runtime.flow.rs:364-380).

## S2 — capability key list (cans/03-sdk-architecture.md)

- Keys line: `policy` → `repo-policy`; added clarifier "The policy key is
  \"repo-policy\", not \"policy\"" (parallel to the existing db-proxy clarifier).
- `c.cap.policy` example → `c.cap.repo_policy` (Py spells hyphenated keys with
  underscores, db_proxy precedent; the hyphen→underscore alias is the work order's
  planned code change).
- TBD left: live Py `Caps` still has `self.policy = PolicyCap()`
  (packages/py/kelir/service_caps.py:137) — the rename to `RepoPolicyCap`/`repo_policy`
  is work-order item 2's code change, owned outside this spec pass.

## S3 — items 3+10, checkpoint surfaces (cans/03-sdk-architecture.md, cans/09-snapshots.md)

- 03 "Native Engine Planes" checkpoint line split: Py/TS one-shot
  `session.checkpoint.create(label)` + see: to the Session-surfaces table.
- 03 Session-surfaces table gained two rows:
  - Rust two-phase: `session.lineage(root_pid) -> PidLineage`,
    `session.checkpoint.create_step(lineage, SnapshotDescriptor) -> u64`,
    `commit_step(lineage) -> u64`, `rollback_step(lineage) -> SnapshotDescriptor`,
    `branch_from(node_id, lineage)`, `restore(node_id, guest_monotonic_ns) -> SnapshotDescriptor`,
    `head(lineage) -> Option<u64>`, `lineage(node_id) -> Vec<u64>`, `compact(lineage)`,
    `monotonic_watermark_ns()`.
  - Py/TS one-shot: `session.checkpoint.create(label) -> SnapshotDescriptor` +
    low-level `session.vm.checkpoint(label) -> SnapshotDescriptor`.
  - Deref contract documented: facade `Session` derefs to engine `Session`;
    `session.checkpoint` resolves to `CheckpointPlane` through Deref; two-phase
    step API is Rust-only (bindings expose only the one-shot create).
- 09: `session.lineage()` → `session.lineage(root_pid)` (Rust); Py/TS documented
  as carrying no lineage accessor — their lineage-visible surfaces are the one-shot
  `checkpoint.create(label)` / `rollback(snapshot)`.
- Verified: `CheckpointOps` trait (crates/kelir-interface/src/checkpoint/ops.flow.rs);
  `CheckpointPlane` (crates/kelir-engine/src/planes/checkpoint/plane.flow.rs);
  `pub checkpoint: Arc<CheckpointPlane>` + `lineage(root_pid: u32) -> PidLineage` on
  engine Session (crates/kelir-engine/src/session_lifecycle.flow.rs:56,603);
  `Deref` on facade Session (crates/kelir/src/session_lifecycle.flow.rs:204-216);
  Py `CheckpointHandle.create(label)` / `VmHandle.checkpoint(name)`
  (packages/py/kelir/__init__.py:459,580); TS `CheckpointHandle.create(label)` /
  `NativeVmHandle.checkpoint(name)` (packages/ts/core/src/session.lifecycle.ts:53-55,199-200).
- Note: work order said `Session::suspend/resume` and `Session::rollback(lineage)`
  live in crates/kelir/src/session_lifecycle.flow.rs — actually the engine Session
  (crates/kelir-engine/src/session_lifecycle.flow.rs) owns them; the facade reaches
  them via Deref. Spec documents the Deref contract instead.

## S4 — fork surface (cans/08-lifecycle.md)

- `session.vm.fork() returns full Session` → `session.fork() -> Session returns the
  full forked Session`, with: VmHandle has no fork; Rust `Session::fork()` rebinds
  plugins/registration/vetoes/catalog; Py `session.fork()`; TS
  `session.vm.fork()` (the TS VmHandle projection carries fork).
- Verified: `VmHandle` (crates/kelir-interface/src/handles_lifecycle.flow.rs) has no
  fork; facade `Session::fork() -> Result<Session<Plugins, Params>, PluginError>`
  (crates/kelir/src/session_lifecycle.flow.rs:137); engine `Session::fork()`
  (kelir-engine/src/session_lifecycle.flow.rs:609); Py `Session.fork()`
  (lifecycle_session.py:58) and Py `VmHandle.fork()` (__init__.py:467);
  TS wrapper `vm.fork(): Promise<Session>` (session.lifecycle.ts:203).
- TBD left: TS wrapper Session has no direct `fork()` method today; the napi
  `NativeVmHandle` has no `fork` either (the fork lives on `NativeSessionHandle`,
  napi handles.flow.rs:561) — the TS `session.vm.fork()` path needs the binding-side
  follow-up (code, outside this pass).

## S5 — items 5+6, rollback surfaces (cans/03, cans/08, cans/10)

- 03 table: single `session.rollback()` row replaced with the per-language set:
  - Rust `session.rollback(lineage: PidLineage)` (rolls back to lineage head)
  - Rust convenience `session.rollback_here(pid: u32)`
  - Py/TS low-level `session.vm.rollback(snapshot: SnapshotDescriptor)`
  - Py/TS convenience `session.rollback(snapshot: SnapshotDescriptor)`
  - Explicit: the no-arg `session.rollback()` is NOT a shipped surface; binding stubs
    error; head-of-lineage rollback lands with PID tracking (L3).
- 03 Sandbox facade: `box.rollback()` → `box.rollback(lineage) delegates to
  session.rollback(lineage)`.
- 08 step loop: `StepVerdict::Rollback(reason): calls session.rollback()` →
  `calls session.checkpoint.rollback_step(lineage)` (the step loop discards the
  current delta to the slot base — verified against CheckpointPlane::rollback_step
  and kelir-checkpoint step_slot.run.rs).
- 10-attestation.md follow-up (commit 6bf514d): same step-loop fix for
  "Post-hook failure triggers immediate session.rollback() to slot base".
- Verified: engine `Session::rollback(&mut self, lineage: PidLineage)`
  (head → restore → rollback_to; kelir-engine session_lifecycle.flow.rs:712-724);
  facade `Session::rollback_here(pid)` (crates/kelir/src/session_lifecycle.flow.rs:94-97);
  Py `Session.rollback(snapshot=None)` + `VmHandle.rollback(snapshot)`
  (lifecycle_session.py:62-67, __init__.py:463); TS `Session.rollback(snapshot?)`
  + `vm.rollback(snapshot)` (session.lifecycle.ts:201,268-273); binding no-arg stubs
  error with "rollback: pass a snapshot; lineage-derived head rollback lands with
  PID tracking (L3)" (napi handles.flow.rs:604-607, pyo3 handles.flow.rs:537-540);
  `Sandbox::rollback(lineage)` (sandbox_runtime.flow.rs:357-362).

## S6 — item 7, exec surfaces (cans/03-sdk-architecture.md)

- Session-surfaces table: `session.spawn(binary, argv) | control` added as the
  primary Session exec surface (all three SDKs); Rust variant
  `session.exec(binary, argv, limits) -> SpawnBuilder`, `.await`-ed
  (`impl Future<Output = Result<SpawnResult, PluginError>>`); binding-level spawn
  dispatch lands with the shim wire protocol (L3).
- Exec protocol block: header `session.exec(cmd, limits?)` → `session.spawn(binary, argv)`
  with the SpawnBuilder note retained.
- Sandbox facade: `box.exec(cmd)` → `box.exec(binary, argv) delegates to
  session.spawn(binary, argv)`.
- Verified: `Session::spawn(binary, argv) -> Result<SpawnResult, PluginError>` and
  `Session::exec(binary, argv, limits) -> SpawnBuilder` + `impl Future for SpawnBuilder`
  (crates/kelir/src/session_lifecycle.flow.rs:76-86,331-337); Py `Session.spawn` /
  `exec_in_guest` (lifecycle_session.py:77-90); TS `Session.spawn` / `execInGuest`
  (session.lifecycle.ts:226-251); binding spawn stubs error with "spawn: EXEC stream
  dispatch lands with the shim wire protocol (L3)" (napi:575-578, pyo3:512-515);
  `Sandbox::exec(binary, argv)` (sandbox_runtime.flow.rs:350-355).

## S7 — item 8, causal delta access (cans/plugins/ledger.md)

- New "Causal delta access" node: canonical home
  `ledger.get_causal_delta_from_parent() -> CausalDelta`; Rust reach path
  `session.ctx::<LedgerCtx, _>()`; Py `session.cap.ledger.get_causal_delta_from_parent()`;
  TS `c.ledger.getCausalDeltaFromParent()` on the plugin context; the Session facade
  does not carry it; session-handle stubs error toward plugin-ledger; rationale:
  causal deltas are produced by plugin-ledger over the delta chain.
- DEVIATION from decision S7 (recorded): the decision spelled the TS surface
  `session.cap.ledger.getCausalDeltaFromParent()`, but the TS SDK has no
  `session.cap` — TS plugin capabilities land on the accumulated handler context
  (`c.<pluginKey>`, pinned in 03; ledger key = "ledger",
  packages/ts/core/src/plugins.registry.ts:115). Wrote `c.ledger.getCausalDeltaFromParent()`
  per the tree; flagged rather than inventing a `session.cap` surface for TS.
- TBD left: Rust `LedgerCtx::get_causal_delta_from_parent` and the TS
  `LedgerHandle.getCausalDeltaFromParent` accessor do not exist yet (work-order item 8
  code changes, L3). Py `LedgerCap.get_causal_delta_from_parent()` ships as an L3 stub
  (service_caps.py:63-65); Py/TS Session facade stubs exist and error (lifecycle_session.py:132-134,
  session.lifecycle.ts:295-299; bindings error "causal deltas come from plugin-ledger (L3)").

## S8 — item 9, ledger.flush() reachability (cans/08 home, see: from cans/plugins/ledger.md)

- Canonical home: 08-lifecycle.md#Session-and-context-handles, new "Cap reachability
  in lifecycle hooks (ledger.flush() contract)" node:
  - Capture-at-registration is the canonical pattern today (clone the ledger handle
    into the closure at builder time, as in artifacts/examples/db-backed-runtime.rs).
  - `LifecycleCtx.cap.ledger` (fused lifecycle hooks receiving a ctx that carries cap)
    lands with the fused lifecycle payload: TBD (L3).
- ledger.md "Ledger handler API" carries a see: to that anchor (no duplication).
- Verified: capture pattern (cans/artifacts/examples/db-backed-runtime.rs:121-127);
  Rust `LifecycleCtx<'a>` carries config/session_id/driver/vetoes/watchdog_probe only —
  no cap (crates/kelir-interface/src/plugin_model.flow.rs:227-233); Py `LifecycleCtx`
  has `_vetoes`/`_watchdog` only (packages/py/kelir/model_plugin.py:84-88); TS
  `LifecycleContext` is `{ setWatchdog, registerVeto }` only
  (packages/ts/core/src/context.lifecycle.types.ts:104-107); `LedgerCtx::flush` /
  `LedgerCap.flush` / `LedgerHandle.flush` all exist.

## Checks

- `cans` CLI: ABSENT in this environment — `cans check` (refs-only/full) skipped.
- `bash scripts/check-canon.sh` from worktree root: PASS ("Corpus is 100% Day-1 canonical"),
  run after every spec commit.
- No cargo/pnpm/just available (per work order); nothing installed.

## Other findings (no action, out of item scope)

- work-order item 1's claim that `Session::suspend/resume` sit in
  crates/kelir/src/session_lifecycle.flow.rs is stale (engine Session owns them; Deref).
- 03 still carries the "Non-conforming: db-backed-runtime.py:84 reads c.cap.db"
  item (db key) — untouched; that is work-order item 2's artifact-side cleanup.
- crates/kelir-testkit/anchors.json mirrors the old 08 line
  "StepVerdict::Rollback(reason): calls session.rollback()..." — the anchors file is
  code-side and needs regeneration by the code pass after this spec change.
