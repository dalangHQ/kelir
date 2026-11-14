# WO4-A report — Rust crates lane (Agent A, 2026-11-12)

Branch `temp/wo4-a`, base 9c01a66 (= origin/scaffolding). All tasks A1–A5 done. Phase L1/L2 respected: no L3 bodies filled; the single new deferral is the ordered `// TODO (L3)` one-liner on the A3 stub. No `VmHandle`/`NativeVmHandle`/`PyVmHandle` method touched (D1).

## Per-task outcomes

### A1 (work-order item 1) — bindings session `suspend(reason)` / `resume()` — done

`NativeSessionHandle::suspend` / `PySessionHandle::suspend` no longer take a path and no longer hard-code `SDK_SUSPEND_REASON`; they take a real `reason` and derive the suspend image path internally. `resume()` is now no-arg with the same internal derivation. `NativeVmHandle` / `PyVmHandle` keep the low-level `(reason, path)` / `(path)` primitives untouched — the session handle now plays the SDK layer's role on top of them, exactly like `packages/py`/`packages/ts` do.

- **Reason bridge shape (decision):** `{ kind: "external" | "resource-pressure", detail?: ExternalSuspendKind }` — identical field names to the TS `SuspendReason` union (`verdict.protocol.ts`: `{ kind: "external", detail } | { kind: "resource-pressure" }`), so the TS object crosses unchanged; Python sends the same shape as a dict (`ExternalSuspendKind` there is a str-Enum with the kebab values).
  - napi: new `#[napi(object)] NativeSuspendReasonRecord { kind: String, detail: Option<String> }` + `suspend_reason_of(record) -> Result<SuspendReason>` (string-enum match, `missing()` errors — same style as `snapshot_descriptor`/storage_bridge `Locked|Passthrough` matchers). Re-exported from the crate root in `napi/src/lib.rs`.
  - pyo3: `suspend(reason: &Bound<'_, PyDict>)` + `suspend_reason_of(record) -> PyResult<SuspendReason>` using the file's `get_item(...)? .ok_or_else(...)? .extract()?` dict style (same as `snapshot_descriptor`/`guest_clock_of`).
- **Path derivation (decision):** `session_dir()/suspend.img` derived in Rust, per binding, mirroring each SDK:
  - napi `suspend_image_path(&self.session_dir)` — `std::path::Path::new(dir).join("suspend.img")` over the handle's cached `session_dir` string (what the TS `sessionDir` getter returns; TS SDK does `` `${this.sessionDir}/suspend.img` ``).
  - pyo3 `suspend_image_path(&self.session)` — `lock_session(..)?.session_dir()?` then `.join("suspend.img")` (what the Py `session_dir()` getter returns; Py SDK does `str(self.session_dir() / "suspend.img")`).
  - Same derivation the engine uses (`kelir-engine/src/session_lifecycle.flow.rs:806,828`). Binding still calls `self.vm.suspend(reason, &path)` / `self.vm.resume(&path)` — the VmHandle primitive stays the call target per D1.
- **`SDK_SUSPEND_REASON` const:** still used by the untouched `NativeVmHandle::suspend` / `PyVmHandle::suspend`; not dead code.
- **Cross-agent impact (must land with this):**
  - Agent C (Py): `lifecycle_session.py` must call `native.suspend({"kind": "external", "detail": <kebab>} | {"kind": "resource-pressure"})` and `native.resume()` (no path arg, no derivation left in Python).
  - Agent D (TS): native interface decls become `suspend(reason: NativeSuspendReasonRecord): Promise<void>` / `resume(): Promise<void>`; `SuspendReason.external(...)`/`SuspendReason.resourcePressure` are directly assignable to the generated `{ kind: string; detail?: string }`.
  - Verified by grep: nothing else under `crates/` calls the changed session-handle methods, so no in-lane breakage.

### A2 (work-order items 3/10) — `Session::lineage(pid)` explicit on the Rust SDK facade — done

`crates/kelir/src/session_lifecycle.flow.rs:93-96`: `pub fn lineage(&self, pid: u32) -> crate::PidLineage` on `Session<Plugins, Params>`, delegating to `self.core.lineage(pid)` (engine `Session::lineage(root_pid)`, `kelir-engine/src/session_lifecycle.flow.rs:603`; `crate::PidLineage` is the existing lib.rs re-export). Placed next to `rollback_here`, its main consumer. The Rust examples keep working (they now hit the explicit method instead of `Deref`). `// see: cans/09-snapshots.md#Groups-and-lineage` — anchor verified to exist in the node and already referenced by 7 code sites (`kelir-engine/src/planes/checkpoint/*`, `kelir-checkpoint/*`, `kelir-interface/src/checkpoint/graph.flow.rs`); not invented.

### A3 (work-order item 8) — `LedgerCtx::get_causal_delta_from_parent` — stub, not wired (decision)

`crates/kelir-plugins/plugin-ledger/src/lib.rs:178-183`: `pub async fn get_causal_delta_from_parent(&self) -> Result<CausalDelta, PluginError>`. **Wiring was not possible at L2:** `compute_ledger::compute_causal_delta(storage: &StorageDelta, receipt: &ExecutionReceipt)` needs the fork's `StorageDelta` and the parent's `ExecutionReceipt`; `LedgerCtx` holds only `denials` / `otel` / `metrics` — neither input is reachable from the ctx. So it returns the file's typed-error style (`PluginError::Halt("ledger get_causal_delta_from_parent needs the fork storage delta")`, matching `"ledger finalize needs the Merkle WAL sink"` etc.) with a one-line `// TODO (L3) [ar_09_causal_delta_aggregation_merkle]: ...` (tag = real test-plan case `cans/artifacts/test-plan/cases/attestation_receipts/ar_09_causal_delta_aggregation_merkle.yaml`, same tag the bindings' causal-delta stubs carry). Async-ness matches the sibling no-await async methods (`count_denials`, `finalize`).

### A4 — L3 stub-home survey — all homes exist, no new files (expected outcome)

- (a) session spawn EXEC stream dispatch: napi `handles.flow.rs` `spawn`/`exec_in_guest` stubs ("EXEC stream dispatch lands with the shim wire protocol (L3)"), pyo3 same; Rust SDK `SpawnBuilder` poll TODO ("guest exec result" L3) in `crates/kelir/src/session_lifecycle.flow.rs`; L3 relocation TODOs in both `handles.run.rs` files (registered via `#[path = "handles.run.rs"] pub mod handles_run;` in each lib.rs — the registration mechanism for any future `.run.rs`).
- (b) no-arg head-of-lineage rollback: no-arg `rollback()` stubs in both bindings ("pass a snapshot; lineage-derived head rollback lands with PID tracking (L3)"); Py `Session.rollback` no-arg branch raises the missing-bridge EConfig. Kept as-is per D5.
- (c) causal-delta native bridge: session-handle stubs in both bindings ("causal deltas come from plugin-ledger (L3)") stay per D7; canonical home is now the A3 `LedgerCtx` method; Py `LedgerCap.get_causal_delta_from_parent` stub exists (`packages/py/kelir/service_caps.py:63-65`); TS native decl + `Session.getCausalDeltaFromParent()` delegation exist (`packages/ts/core/src/session.lifecycle.ts:112,295`).
- (d) lifecycle ctx.cap delivery: Rust `LifecycleCtx` (`kelir-interface/src/plugin_model.flow.rs:227`) carries its L3 TODO; engine `dispatch_lifecycle`/`dispatch_pre_commit` already hand `&mut LifecycleCtx` to participants. Work-order item 9's chosen Rust-side resolution is *documenting the capture-at-registration pattern* (db-backed-runtime.rs) and leaving the fused path event-payload-free — a spec change (Agent B), not a missing Rust stub. Py/TS `LifecycleContext.cap` additions are agents C/D lanes (`model_plugin.py:84` today has `_vetoes`/`_watchdog` only; `context.lifecycle.types.ts` is `{setWatchdog, registerVeto}` only).

Conclusion: every named L3-deferred surface already has a typed-error stub home; per the task's "ONLY if one is missing" rule, **no stub files were created**.

### A5 — anchors.json — skipped (correctly)

No `cans/` file was touched; no anchor leaf text changed; `crates/kelir-testkit/anchors.json` needs no regeneration from this lane.

## Gates

- Comment law (AGENTS.md §3 greps, scoped to the five touched `.rs` files): every `//` comment is `// see: cans/...#Anchor` or `// TODO (L3) [...] : ...; slop: ...`; zero `///`/`//!`; no trailing comments — **green**.
- Denied lints (workspace `unwrap_used`/`expect_used`/`panic`/`unimplemented`/`todo`/`unreachable`): none introduced; `ok_or_else` used, no unwraps — **green by construction**.
- `cargo check` / `cargo clippy -D warnings` / `cargo fmt --check` / `cargo test`: **no cargo/rustc in this sandbox — deferred to CI.** Edits were kept minimal, pattern-copied from the same files (dict-extraction, string-enum match, napi object + re-export); the only hand-formatted wrap is the `napi/src/lib.rs` re-export list (all lines ≤ 100 cols). One fmt-risk noted for CI: rustfmt may re-pack that import list differently.
- `git diff --stat 9c01a66..HEAD`: 5 files, +106/−11 (below).

## Files changed (9c01a66..HEAD)

```
crates/kelir-bindings/napi/src/handles.flow.rs    | 50 ++++++++-- (A1: reason record + converters + session suspend/resume)
crates/kelir-bindings/napi/src/lib.rs             |  6 +--  (A1: re-export NativeSuspendReasonRecord)
crates/kelir-bindings/pyo3/src/handles.flow.rs    | 49 +++++++-- (A1: dict converter + path helper + session suspend/resume)
crates/kelir-plugins/plugin-ledger/src/lib.rs     |  7 +++  (A3: LedgerCtx::get_causal_delta_from_parent stub)
crates/kelir/src/session_lifecycle.flow.rs        |  5 +++  (A2: Session::lineage(pid))
cans/_collab/wo4-report-a.md                      | (this file)
```

Commits: 3e72dc9 (A1), a83815a (A2), c2fe913 (A3) — each pushed to `temp/wo4-a` immediately after completion; this report commit follows.

## TODO (L3) / see: counts

+1 `TODO (L3)` (the ordered A3 stub marker; total TODO count rises only through this sanctioned new deferral), +7 `// see:` one-liners (3 napi, 3 pyo3, 1 kelir SDK). No prose comments added.

## For the orchestrator / other lanes

1. **Agent C (Py SDK):** switch `Session.suspend`/`resume` in `lifecycle_session.py` to `native.suspend({...reason dict...})` / `native.resume()` — the derivation and the path argument move into the binding (this lane, done). Dict shape: `{"kind": "external", "detail": "human-authorization-hold"|"wire-connection-parking"|"step-evaluation-yield"|"host-power-event-snapshot"}` or `{"kind": "resource-pressure"}`. Keep `suspended_reason` tracking SDK-side; the binding is stateless.
2. **Agent D (TS SDK):** update the native session interface + `Session.suspend/resume` to pass the reason object and drop the path (`await this.native.suspend(reason)` / `await this.native.resume()`). The napi-generated record is `{ kind: string; detail?: string }`; the existing `SuspendReason` union is structurally assignable.
3. **Agent B (spec):** cans/03 + cans/08 should now describe the binding session surface as `suspend(reason)` / `resume()` (path-deriving), with `VmHandle::suspend(reason, path)` as the low-level primitive (D1), and `Session::lineage(pid)` as explicit Rust SDK surface (A2). cans/plugins/ledger.md should name `LedgerCtx::get_causal_delta_from_parent` as the canonical causal-delta home (D7).
4. Maturity policy: `.run.rs` relocation TODOs already cover suspend/resume/spawn bodies at L3 (both bindings' `handles.run.rs`); nothing further to pre-register.
