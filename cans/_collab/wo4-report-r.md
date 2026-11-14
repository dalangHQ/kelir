# WO4 Report — Agent R (Task ID: WO4-R, branch temp/r)

Scope: `crates/` only. Phase L1/L2. Base: scaffolding @ 9c01a66.

## R1 — Work-order item 1: session-handle suspend takes a reason (bindings)

**Changed** (not already done — live tree had `suspend(path)` with hard-coded `SDK_SUSPEND_REASON` on both session handles):

- `crates/kelir-bindings/napi/src/handles.flow.rs`
  - New `#[napi(object)] NativeSuspendReasonRecord { kind: String, detail: Option<String> }` — mirrors the TS `SuspendReason` union (`{ kind: "external", detail } | { kind: "resource-pressure" }` in `packages/ts/core/src/verdict.protocol.ts`), following the existing `NativeClockSkewOptions`/`NativeSnapshotDescriptorRecord` record pattern.
  - New `suspend_reason(&NativeSuspendReasonRecord) -> Result<SuspendReason>` converter mapping kebab-case kinds/details (`human-authorization-hold`, `wire-connection-parking`, `step-evaluation-yield`, `host-power-event-snapshot`, `resource-pressure`) onto `kelir_interface::driver_interface::SuspendReason`; malformed shapes get a typed `missing(...)` error.
  - `NativeSessionHandle::suspend(reason: NativeSuspendReasonRecord)` now derives `<session_dir>/suspend.img` internally (from the cached `session_dir` field — the getter the prior handoff requested is present at the live tree) and calls `VmHandle::suspend(reason, &path)`. Matches the SDK-layer behavior in `packages/ts/core/src/session.lifecycle.ts` (`${sessionDir}/suspend.img`) and the engine (`session_dir(...)?.join("suspend.img")`).
  - `NativeSessionHandle::resume()` is now no-arg and derives the same path (matches TS `Session.resume()` / engine `Session::resume()`).
  - `NativeVmHandle::suspend(path)` / `resume(path)` left **unchanged** (intentional low-level primitive; `VmControl::suspend(reason, path)` in `kelir-interface/src/handles_lifecycle.flow.rs` stays the reason+path API). `SDK_SUSPEND_REASON` const kept for the VM handle.
- `crates/kelir-bindings/pyo3/src/handles.flow.rs`
  - New `suspend_reason(&Bound<'_, PyDict>) -> PyResult<SuspendReason>` — dict input with `kind`/`detail` keys, following the existing `snapshot_descriptor`/`guest_clock_of` PyDict pattern (Python `External(kind)`/`ResourcePressure()` dataclasses project onto this dict).
  - New `suspend_image(&Arc<Mutex<Session>>) -> PyResult<String>` helper deriving `<session_dir>/suspend.img` via `lock_session` + `session.session_dir()`.
  - `PySessionHandle::suspend(reason: &Bound<'_, PyDict>)` and no-arg `PySessionHandle::resume()` derive the path internally.
  - `PyVmHandle::suspend(path)` / `resume(path)` left **unchanged**.

**Decisions:**
- Reason wire form at the bridge: object record (napi) / PyDict (pyo3) with `kind` + optional `detail`, matching the TS `SuspendReason` shape and both crates' existing record/dict patterns. No stringly-typed invention beyond the kebab-case values the TS/Py SDKs already use.
- Cross-boundary note for the SDK owners (packages/py, packages/ts — outside my scope): `lifecycle_session.py` (`native.suspend(str(session_dir()/"suspend.img"))`) and `session.lifecycle.ts` (`native.suspend(`${sessionDir}/suspend.img`)`) still pass a **path** to the session handle; they must switch to passing the reason record/dict and let the binding derive the path (or keep deriving and call `vm.suspend`-style path form). Until then the SDK layers and the new binding signature are out of sync. Also `cans/08-lifecycle.md#Suspend-and-resume` ("The vm-handle / native-bridge layer keeps suspend(path)... the bridge supplies a fixed reason") now describes only the VmHandle projections; the session-handle form is reason-based — the cans owner should update that block (work-order item 1 already directs deleting the "Non-conforming sides" block).

## R2 — Work-order item 3: facade `Session::lineage(pid)`

**Changed** (was not present on the facade; only reachable through `Deref` to `kelir_engine::Session`):

- `crates/kelir/src/session_lifecycle.flow.rs`: added `pub fn lineage(&self, pid: u32) -> crate::PidLineage { self.core.lineage(pid) }` with `// see: cans/09-snapshots.md#Groups-and-lineage` (real leaf anchor, already used by `kelir-checkpoint/src/manifest_graph.run.rs` et al.). Field name verified as `core`; engine method verified as `lineage(root_pid: u32)` (engine `session_lifecycle.flow.rs:603`); `PidLineage` verified re-exported at `crates/kelir/src/lib.rs:38` (same `crate::PidLineage` style as `sandbox_runtime.flow.rs`). Placed next to `rollback_here`, which consumes the same engine call. Behavior identical for existing `Deref` users (same delegation).

## R3 — Work-order item 8: `LedgerCtx::get_causal_delta_from_parent`

**Changed** (did not exist; only the free function `compute_ledger::compute_causal_delta(storage, receipt)` existed):

- `crates/kelir-plugins/plugin-ledger/src/lib.rs`: added `pub async fn get_causal_delta_from_parent(&self) -> Result<CausalDelta, PluginError>` next to `finalize` on `LedgerCtx`.
- `LedgerCtx` cannot reach the needed inputs at L2: its only fields are `denials: Arc<Mutex<HashMap<u32, u64>>>`, `otel: OtelConfig`, `metrics: Option<Arc<MetricRegistry>>` — no `StorageDelta`, no `ExecutionReceipt`. Per instructions ("Do NOT invent fields") the method returns a typed `PluginError::Halt` following the crate's existing error style, with a one-line `// TODO (L3) [ar_09_causal_delta_aggregation_merkle]:` pointing at the `compute_ledger::compute_causal_delta(storage, receipt)` delegation that lands when the inputs become reachable.

## R4 — Verify-only (work-order items 5, 6, 7): spawn / no-arg rollback stubs

**Verified, no changes needed** (all four stubs exist in the live tree with factually accurate messages):

- `NativeSessionHandle::spawn` — napi `handles.flow.rs:603` (stub), message `"spawn: EXEC stream dispatch lands with the shim wire protocol (L3)"` — accurate: facade spawn dispatches on `StreamKind::Exec` via `SpawnTransport::dispatch_exec` and the result channel is an L3 TODO.
- `PySessionHandle::spawn` — pyo3 `handles.flow.rs:549`, same message — accurate.
- `NativeSessionHandle::rollback` (no-arg) — napi `handles.flow.rs:632`, message `"rollback: pass a snapshot; lineage-derived head rollback lands with PID tracking (L3)"` — accurate: `vm.rollback(snapshot)` exists on both VmHandles as the working path; lineage-derived head rollback needs the dispatch PID lineage (facade `rollback_here(pid)`/`rollback(lineage)` prove the shape).
- `PySessionHandle::rollback` (no-arg) — pyo3 `handles.flow.rs:578`, same message — accurate.

No message fixes required; none referenced a wrong file/surface.

## Hygiene gates (run from /home/z/my-project/wt-r after each item)

- `grep -rn --include='*.rs' -E '^\s*//' crates/ | grep -vE '// (see:|TODO[^:]*:|SAFETY:)'` → empty (exit 1)
- `grep -rn --include='*.rs' -E '^\s*(///|//!)' crates/` → empty (exit 1)
- No `unwrap()/expect()/panic!/todo!()/unimplemented!()/unreachable!()` added; no `#[cfg(test)]` added; all touched files well under 1000 LOC (napi 676, pyo3 615, kelir session_lifecycle ~383, plugin-ledger lib ~293).
- cargo/rustc not installed in this environment (fast mode); correctness relied on pattern-matching to live-tree code and the comment-law greps. CI must run `cargo check/clippy/fmt`.

## Commits on temp/r (scaffolding..temp/r)

- `05de638` WO4-R: R1 session-handle suspend(reason)/resume() derive suspend.img in napi+pyo3 bindings
- `4fd1c91` WO4-R: R2 facade Session::lineage(pid) delegates to engine session
- `9293026` WO4-R: R3 LedgerCtx::get_causal_delta_from_parent typed-Err stub for plugin-ledger
- (this report) WO4-R: R4 verification + report

## Skipped / out of scope

- VmHandle-level `suspend`/`resume` in both bindings: intentionally unchanged (spec-sanctioned low-level path form).
- `cans/08-lifecycle.md` "Non-conforming sides" deletion and `cans/plugins/ledger.md` causal-delta section (work-order items 1/8 cans edits): cans/ outside my exclusive scope.
- packages/py + packages/ts SDK-layer call sites that still pass a path to the session-handle suspend (see R1 decisions) — owned by the packages agents.
- Bindings checkpoint-handle `create_step`/`commit_step`/`lineage` additions (work-order item 3 optional alternative): not in my work items; left as documented Rust-only engine surfaces.
