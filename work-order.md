# Misalignments Between `cans/artifacts/examples` and `cans/` Spec — with codebase changes

## 1. Suspend / Resume Signatures (Spec Contradicted by Artifacts)

The spec's SSOT-A1 pin in `cans/08-lifecycle.md` says:

> **Canonical:** `Session.suspend(reason: SuspendReason)` / `Session.resume()`
> **Non-conforming sides:** TS `Session.suspend(path)` / `resume(path)`; Py `Session.suspend()` (no reason) / `resume(path)`

**But every TS/Python example calls `suspend(reason)` and `resume()` (no path):**

| File | Call | Spec says |
|---|---|---|
| `autonomous-swe-agent.py` | `session.suspend(External(...))` / `session.resume()` | Py suspend takes no reason; resume takes path |
| `autonomous-swe-agent.ts` | `session.suspend(SuspendReason.external(...))` / `session.resume()` | TS suspend takes path; resume takes path |
| `serverless-webhook.py` | `session.suspend(External(...))` | Py suspend takes no reason |
| `serverless-webhook.ts` | `session.suspend(SuspendReason.external(...))` | TS suspend takes path |
| `policy-matrix.py` | `session.suspend(ResourcePressure())` / `session.resume()` | Py suspend takes no reason |
| `policy-matrix.ts` | `session.suspend(SuspendReason.resourcePressure)` / `session.resume()` | TS suspend takes path |

The artifacts are actually conforming to the **canonical** reason-based form, which directly contradicts the spec's claim that TS/Py are non-conforming. Either the spec's "non-conforming sides" section is stale, or the artifacts were updated without the spec.

### Changes Needed in Codebase

- **`cans/08-lifecycle.md`**: delete the "Non-conforming sides" block. The artifacts and the shipped SDKs are already canonical:
  - `crates/kelir/src/session_lifecycle.flow.rs` (`Session::suspend(reason)` / `Session::resume()`)
  - `crates/kelir/src/sandbox_runtime.flow.rs` (`Sandbox::suspend(reason)` / `Sandbox::resume()`)
  - `packages/py/kelir/lifecycle_session.py` (`Session.suspend(reason)` derives `session_dir()/suspend.img` internally)
  - `packages/ts/core/src/session.lifecycle.ts` (`Session.suspend(reason)` does the same)
- The only surface that still takes a path is the lower-level `VmHandle` (`NativeVmHandle::suspend(&self, path)`, `PyVmHandle::suspend(&self, path)` in `crates/kelir-bindings/{napi,pyo3}/src/handles.flow.rs`). That is intentional because `VmControl::suspend(reason, path)` in `crates/kelir-interface/src/handles_lifecycle.flow.rs` takes both. Do **not** change `VmHandle` — fix the spec's language to describe `VmHandle::suspend(reason, path)` as the low-level primitive and `Session::suspend(reason)` as the SDK surface.
- Optional: give `NativeSessionHandle::suspend`/`PySessionHandle::suspend` the reason-based form (deriving the path like the SDKs do) so the binding layer matches the SDK layer. Current stubs are `suspend(path)` calling `self.vm.suspend(SDK_SUSPEND_REASON, &path)` with a hard-coded reason — replace that with a real `reason` argument and path derivation.

---

## 2. Capability Key Naming: `repo-policy` vs `policy`

Spec (`cans/03-sdk-architecture.md`, SSOT-A1):
> Capability keys are plugin names. Keys: `ledger`, `db-proxy`, `policy`, `transaction`, `container`.

The plugin in `autonomous-swe-agent.py` is named `repo-policy`:
```python
class RepoPolicyPlugin(BasePlugin):
    name = "repo-policy"
```
And the fixture has `[plugins."repo-policy"]`.

Yet Python accesses it as:
```python
c.cap.policy.violations          # autonomous-swe-agent.py
```
While TypeScript accesses it correctly by plugin name:
```typescript
c["repo-policy"].policy.violations   // autonomous-swe-agent.ts
```

This violates the SSOT-A1 rule ("the capability key is the plugin name") and is inconsistent across languages. The spec's key list including `policy` appears to be either describing a different plugin or codifying the Python bug.

### Changes Needed in Codebase

- **`packages/py/kelir/service_caps.py`**:
  - Rename `class PolicyCap` → `class RepoPolicyCap` (keep `PolicyCap = RepoPolicyCap` as a deprecation alias, or drop).
  - Change `Caps.__init__` from `self.policy = PolicyCap()` to `self.repo_policy = RepoPolicyCap()`.
  - Add `"repo-policy": "repo_policy"` to `_CAP_KEY_ALIASES` so `materialize_cap(cap, "repo-policy", record)` lands on `cap.repo_policy`.
  - If keeping `policy` as a convenience alias, make it a property returning `repo_policy` and document the alias in the class docstring.
- **`packages/py/kelir/__init__.py`**: update the `from kelir.service_caps import (...)` re-export (`PolicyCap` → `RepoPolicyCap`).
- **`cans/03-sdk-architecture.md`** SSOT-A1 key list: replace `policy` with `repo-policy` (or rename the plugin to `policy` — but the class name `RepoPolicyPlugin` and the fixture `[plugins."repo-policy"]` argue against that).
- **`cans/artifacts/examples/autonomous-swe-agent.py`**: change `c.cap.policy.violations` to `c.cap.repo_policy.violations`. (The TS example is already correct.)
- The TypeScript side (`packages/ts/core/src/plugins.registry.ts`) already generates `c["repo-policy"]` from the plugin name and requires no change.

---

## 3. Checkpoint / Lineage API: `create` vs `create_step` vs `vm.checkpoint`

Spec (`cans/03-sdk-architecture.md`):
> Checkpoint: `session.checkpoint.create(label)`, `session.rollback()`

Spec (`cans/09-snapshots.md`):
> Not visible in `session.lineage()`

**What the artifacts actually use:**

| File | Call | Conforms? |
|---|---|---|
| `git-gate.py` / `.ts` | `session.checkpoint.create("git-gate-base")` | ✅ `create(label)` |
| `multi-vm-swarm.py` / `.ts` | `child.checkpoint.create(...)` | ✅ `create(label)` |
| `replay.py` / `.ts` | `session.checkpoint.create(...)` | ✅ `create(label)` |
| `autonomous-swe-agent.py` / `.ts` | `session.vm.checkpoint("clean_base")` | ❌ not `session.checkpoint.create` |
| `policy-matrix.py` / `.ts` / `.rs` | `c.vm.checkpoint("policy-yield")` | ❌ not `session.checkpoint.create` |
| `hook-matrix-full.rs` | `session.checkpoint.create_step(session.lineage(1), …)` | ❌ low-level `create_step` |
| `policy-matrix.rs` | `session.checkpoint.create_step(session.lineage(1), …)` | ❌ low-level `create_step` |
| `git-gate.rs` | `session.checkpoint.create_step(...)` | ❌ low-level `create_step` |
| `replay.rs` | `session.checkpoint.create_step(...)` | ❌ low-level `create_step` |
| `multi-vm-swarm.rs` | `child.checkpoint.create_step(child.lineage(1), …)` | ❌ low-level `create_step` |
| `autonomous-swe-agent.rs` | `session.checkpoint.create_step(session.lineage(1), …)` | ❌ low-level `create_step` |

**Every Rust example** uses `create_step`/`commit_step`/`lineage(1)`, while **every Python/TypeScript example** (except the `vm.checkpoint` ones) uses `create`. The spec's stated SDK surface is `create(label)` and `lineage()`. So:

- Rust examples are systematically non-conforming to the SDK surface.
- `autonomous-swe-agent` and `policy-matrix` (Py/TS) use `session.vm.checkpoint`, which is not the spec's stated surface.
- `session.lineage(1)` appears in every Rust example; the spec says `session.lineage()` (no args).

This is a cross-language API split: Rust exposes `create_step`/`lineage(1)`, Py/TS expose `create`/`vm.checkpoint`. The spec describes only one of these.

### Changes Needed in Codebase

- **`cans/03-sdk-architecture.md` "Session-surfaces" table**: split the single checkpoint row into two, matching the code:
  - **Rust**: `session.lineage(root_pid) -> PidLineage`; `session.checkpoint.create_step(lineage, SnapshotDescriptor) -> u64`; `session.checkpoint.commit_step(lineage) -> u64`; `session.checkpoint.rollback_step(lineage) -> SnapshotDescriptor`; `session.checkpoint.branch_from(node_id, lineage)`; `session.checkpoint.restore(node_id, guest_monotonic_ns) -> SnapshotDescriptor`; `session.checkpoint.head(lineage) -> Option<u64>`; `session.checkpoint.lineage(node_id) -> Vec<u64>`; `session.checkpoint.compact(lineage)`; `session.checkpoint.monotonic_watermark_ns()`.
  - **Py/TS**: `session.checkpoint.create(label) -> SnapshotDescriptor` (one-shot, wraps `VmHandle::checkpoint`); `session.vm.checkpoint(label) -> SnapshotDescriptor` (low-level, same call).
- **`cans/09-snapshots.md`**: change `session.lineage()` to `session.lineage(root_pid)` for Rust; document the Py/TS `Session.rollback(snapshot)`/`VmHandle.rollback(snapshot)` surfaces as the only lineage-visible surface for those SDKs.
- **`crates/kelir/src/session_lifecycle.flow.rs`**: add an explicit `pub fn lineage(&self, pid: u32) -> PidLineage` on `Session<Plugins, Params>` delegating to `self.core.lineage(pid)`. Right now every Rust example relies on `Deref` to `kelir_engine::Session::lineage(pid)` — the spec does not document that this is `Session`-visible, and `Session::rollback(lineage)` depends on it. Add it, or the Deref contract needs to be spelled out.
- **`crates/kelir-bindings/{napi,pyo3}/src/handles.flow.rs`**: add `NativeCheckpointHandle::lineage(pid)` and `create_step`/`commit_step`/`rollback_step`/`head`/`compact` to `NativeCheckpointHandle` and `PyCheckpointHandle` — OR leave them Rust-only and document that the Py/TS SDKs use the one-shot `create`/`vm.checkpoint` surfaces only. Do not leave the examples using `vm.checkpoint` while claiming to use the spec's `session.checkpoint.create` surface.
- **`cans/artifacts/examples/autonomous-swe-agent.py` / `.ts`** and **`policy-matrix.py` / `.ts`**: use `session.checkpoint.create("clean_base")` (or `c.checkpoint.create("policy-yield")`) rather than `session.vm.checkpoint(...)`. Both work through the bindings, but the examples should not teach the undocumented surface.
- `crates/kelir-engine/src/session_lifecycle.flow.rs` already exposes `lineage(pid)` (engine `Session`) and `CheckpointPlane::create_step`/`commit_step`/`lineage(node_id)`/`compact` — those are the intended Rust low-level surfaces; the spec just does not name them.

---

## 4. Fork API: `session.fork()` vs `session.vm.fork()`

Spec (`cans/08-lifecycle.md`):
> `session.vm.fork()` returns full Session

| File | Call | Conforms? |
|---|---|---|
| `autonomous-swe-agent.py` | `await parent.vm.fork()` | ✅ |
| `autonomous-swe-agent.ts` | `await session.vm.fork()` | ✅ |
| `autonomous-swe-agent.rs` | `session.fork().await?` | ❌ `session.fork()` |
| `multi-vm-swarm.py` | `await session.fork()` | ❌ `session.fork()` |
| `multi-vm-swarm.ts` | `await session.vm.fork()` | ✅ |
| `multi-vm-swarm.rs` | `session.fork().await?` | ❌ `session.fork()` |

`multi-vm-swarm.py` and both Rust examples use `session.fork()`, while the spec and TS examples use `session.vm.fork()`.

### Changes Needed in Codebase

- **`cans/08-lifecycle.md`**: change `session.vm.fork()` to `session.fork() -> Session`. The code only has `Session::fork()`:
  - `crates/kelir/src/session_lifecycle.flow.rs`: `Session::fork() -> Session<Plugins, Params>` (rebinds plugins, registration, vetoes, catalog).
  - `crates/kelir-bindings/napi/src/handles.flow.rs`: `NativeSessionHandle::fork() -> NativeSessionHandle` on the session handle.
  - `crates/kelir-bindings/pyo3/src/handles.flow.rs`: `PySessionHandle::fork() -> PySessionHandle`.
  - `crates/kelir-interface/src/handles_lifecycle.flow.rs`: `VmHandle` has **no** `fork()` method — only `checkpoint`/`rollback`/`suspend`/`resume`/`export_snapshot`/`import_snapshot`/`adjust_balloon`/`set_vcpus`/`map_shared_region`/`clock`.
- If the spec genuinely wants `session.vm.fork()`, add a `fork()` to `VmHandle`/`NativeVmHandle`/`PyVmHandle` that returns a full `Session`. That would require plumbing plugin state into the VM handle (currently the fork logic lives on `Session` in `crates/kelir/src/session_lifecycle.flow.rs`, not on `VmHandle`), so the correct fix is the other way: the spec changes to `session.fork()`.
- Update the TS example (`autonomous-swe-agent.ts`, `multi-vm-swarm.ts`) and the Python `autonomous-swe-agent.py` to `session.fork()` — no code change, just the examples (they're not in the live tree, but the spec should match the code that ships).

---

## 5. Rollback API: No-Arg vs `vm.rollback(snapshot)` vs `rollback(lineage)`

Spec (`cans/03-sdk-architecture.md`, `cans/08-lifecycle.md`):
> `session.rollback()` — no socket-preserving flag

| File | Call | Conforms? |
|---|---|---|
| `autonomous-swe-agent.py` | `await session.vm.rollback(base)` | ❌ |
| `autonomous-swe-agent.ts` | `await session.vm.rollback(baseSnap)` | ❌ |
| `autonomous-swe-agent.rs` | `session.rollback(lineage).await?` | ❌ |
| `policy-matrix.py` | `await c.vm.rollback(pre)` | ❌ |
| `policy-matrix.ts` | `await c.vm.rollback(pre)` | ❌ |
| `policy-matrix.rs` | `session.rollback(lineage).await?` | ❌ |
| `replay.py` / `.ts` | `await session.rollback(snapshot)` | ❌ |
| `replay.rs` | `session.rollback(lineage).await?` | ❌ |

Three different surfaces (`session.rollback()`, `session.vm.rollback(snap)`, `session.rollback(lineage)`), none of which match the spec's no-arg `session.rollback()`. The spec is contradicted by all three artifacts.

### Changes Needed in Codebase

- **`cans/03-sdk-architecture.md` / `cans/08-lifecycle.md`**: replace the single `session.rollback()` row with the actual surface set:
  - **Rust**: `session.rollback(lineage: PidLineage)` — rolls back to the head of `lineage` (implemented in `crates/kelir/src/session_lifecycle.flow.rs`, delegates through `self.checkpoint.head(lineage)` + `self.checkpoint.restore(...)` + `self.rollback_to(...)`).
  - **Rust convenience**: `session.rollback_here(pid: u32)` — resolves `pid` to a lineage and calls `session.rollback(lineage)` (same file).
  - **Py/TS**: `session.vm.rollback(snapshot: SnapshotDescriptor)` — `VmHandle::rollback(&SnapshotDescriptor)` in `crates/kelir-interface/src/handles_lifecycle.flow.rs`; bindings in `crates/kelir-bindings/{napi,pyo3}/src/handles.flow.rs` take the snapshot.
  - **Py/TS convenience**: `session.rollback(snapshot: SnapshotDescriptor)` — currently in `packages/py/kelir/lifecycle_session.py` (`Session.rollback` delegates to `self.vm.rollback(snapshot)` when a snapshot is passed; the no-arg branch raises the missing-native error). Same in `packages/ts/core/src/session.lifecycle.ts`.
- If the spec's no-arg `session.rollback()` is intended, implement it. The cleanest implementation:
  - Rust: `Session::rollback_here(pid)` already exists — surface it as `session.rollback()` with `pid` inferred from the current `HandlerCtx` (or add `session.rollback()` as "roll back to head of session root lineage").
  - Py/TS: `Session.rollback()` (no arg) should resolve to the session's root lineage head. The Py/TS bindings currently error on `rollback()` no-arg — either implement head-of-lineage rollback in `NativeSessionHandle::rollback`/`PySessionHandle::rollback`, or remove the no-arg stub and update the spec.

---

## 6. Rollback API Shape: Three Different Signatures, Spec Says None Takes Arguments

**Spec** (`cans/03-sdk-architecture.md`, "Session control surface"):
> `session.rollback()` — no socket-preserving flag

**Artifacts:**

| File | Call | Shape |
|---|---|---|
| `autonomous-swe-agent.py` | `await session.vm.rollback(base)` / `await c.vm.rollback(pre)` | takes `SnapshotDescriptor` |
| `autonomous-swe-agent.ts` | `await session.vm.rollback(baseSnap)` / `await c.vm.rollback(pre)` | takes `SnapshotDescriptor` |
| `autonomous-swe-agent.rs` | `session.rollback(lineage).await?` | takes lineage |

Three different surfaces (`session.rollback()`, `session.vm.rollback(snap)`, `session.rollback(lineage)`), none of which match the spec's no-arg `session.rollback()`. The spec is contradicted by all three artifacts.

### Changes Needed in Codebase

(Consolidated with item 5.)

- **`crates/kelir/src/session_lifecycle.flow.rs`**: `Session::rollback(lineage)` exists; `Session::rollback_here(pid)` exists. Neither takes no args. If the spec's no-arg `session.rollback()` is to survive, add `Session::rollback()` that resolves the head of the session's *root* lineage (`root_pid = 1` or a `session.root_lineage()` helper) and calls `rollback_here`. Otherwise remove the no-arg from the spec.
- **`crates/kelir-bindings/{napi,pyo3}/src/handles.flow.rs`**: `NativeSessionHandle::rollback` and `PySessionHandle::rollback` are `fn rollback(&self)` no-arg stubs that always return an error with the message `"rollback: pass a snapshot; lineage-derived head rollback lands with PID tracking (L3)"`. Either:
  - Implement them to mean "roll back to the head of the session's root lineage", or
  - Remove the no-arg binding methods and require callers to go through `session.vm.rollback(snapshot)`.
- **`crates/kelir/src/sandbox_runtime.flow.rs`**: `Sandbox::rollback(&mut self, lineage: PidLineage)` takes a lineage and delegates to `session.rollback(lineage)`. Fine — keep.
- **`packages/py/kelir/lifecycle_sandbox.py`**: `Sandbox.rollback()` no-arg calls `self._session.rollback()` (no snapshot) and currently errors. Either add a `snapshot: SnapshotDescriptor | None = None` parameter (matching `Session.rollback(snapshot=None)`) or implement no-arg head-of-lineage rollback in the binding.
- **`packages/ts/core/src/sandbox.runtime.ts`**: `Sandbox.rollback()` no-arg calls `this.booted.rollback()` — same mismatch. Add a snapshot parameter or implement the no-arg form.
- **`cans/03-sdk-architecture.md`**: document the actual signatures per language, not the single no-arg form.

---

## 7. `session.spawn(...)` Used Everywhere, but the Only Documented Exec Surface Is `session.exec(cmd, limits?)`

**Spec** (`cans/03-sdk-architecture.md`, "Multiplexed binary RPC for SDK consumers"):
> `session.exec(cmd, limits?)`

The Session-surfaces table lists no `spawn`. But **every** example spawns via `session.spawn(...)`:

| File | Call |
|---|---|
| `container.py` / `.ts` / `.rs` | `await session.spawn("podman", [...])` |
| `hook-matrix.py` / `.ts` / `.rs` | `await session.spawn("cat", [IDENTITY_PATH], cwd="/workspace")` |
| `autonomous-swe-agent.py` / `.ts` | `await session.spawn(...)` |
| `db-backed-runtime.py` | `await session.spawn(...)` |
| `policy-matrix.py` / `.ts` | `await session.spawn(binary, [...])` |
| `multi-vm-swarm.py` / `.ts` | `child.spawn(...)` |

`session.exec(...)` is never called on a Session in any example. `Sandbox::exec(...)` is used (in `profile.rs` / `profile.py` / `profile.ts`), but `Sandbox` is a different facade. The `Session` exec surface is `spawn(...)` in all practice and `exec(...)` in the spec. This is a systematic contradiction across all 13 plugin examples.

### Changes Needed in Codebase

- **`cans/03-sdk-architecture.md`**: list `session.spawn(binary, argv)` as the primary exec surface on the Session-surfaces table. Keep `session.exec(binary, argv, limits)` as a `SpawnBuilder`-returning variant, and document that it implements `Future<Output = Result<SpawnResult, PluginError>>` (it does — see `impl Future for SpawnBuilder` at the bottom of `crates/kelir/src/session_lifecycle.flow.rs`). The spec's `session.exec(cmd, limits?)` currently reads as returning a `SpawnResult`; it doesn't.
- **`crates/kelir/src/session_lifecycle.flow.rs`**: the doc comment on `Session::spawn` already says "Idiomatic-Surfaces-Law"; add a doc comment on `Session::exec` clarifying it returns `SpawnBuilder` and is `.await`-ed. Consider renaming `exec` to `spawn_builder` or `exec_builder` so the surface names align with the examples, or keep `exec` and document the difference.
- **`crates/kelir/src/sandbox_runtime.flow.rs`**: `Sandbox::exec(&mut self, binary, argv)` delegates to `session.spawn(binary, argv)`. Keep as-is; the spec's `Sandbox.exec(cmd, limits?)` should be updated to `Sandbox.exec(binary, argv)`.
- **`crates/kelir-bindings/{napi,pyo3}/src/handles.flow.rs`**: `NativeSessionHandle::spawn` and `PySessionHandle::spawn` are stubs that error with `"spawn: EXEC stream dispatch lands with the shim wire protocol (L3)"`. The spec should not advertise `session.exec` as the shipped surface while the actual `spawn` is a stub — either:
  - Implement `spawn` (preferred), or
  - Document the surface as `session.exec` in all examples.
- Update `cans/artifacts/examples/*` that call `session.spawn(...)` — no code change needed on the Rust side (`Session::spawn` exists and works), but the spec's "no `spawn`" claim needs to be corrected to match every example.

---

## 8. `get_causal_delta_from_parent()` — Called on Session in Some Examples, on Ledger in Others, and Documented Nowhere

**Spec** (`cans/plugins/ledger.md`, SSOT-A1 pin):
> `finalize(FinalizeRequest { include_delta, include_merkle_root })` — the sole documented `finalize`.

The spec never says where `get_causal_delta_from_parent()` lives. Artifacts disagree:

| File | Call site | Conforms? |
|---|---|---|
| `autonomous-swe-agent.py` | `await child.get_causal_delta_from_parent()` | on **Session** |
| `autonomous-swe-agent.ts` | `await child.getCausalDeltaFromParent()` | on **Session** |
| `db-backed-runtime.py` | `await ledger.get_causal_delta_from_parent()` | on **Ledger** |
| `db-backed-runtime.ts` | `await session.getCausalDeltaFromParent()` | on **Session** |
| `db-backed-runtime.rs` | `// TODO (L3): ... session facade exposes no get_causal_delta_from_parent yet` | **does not exist** in Rust |

`db-backed-runtime.py` calls it on `ledger`; every TS example calls it on `session`; and the Rust artifact's TODO explicitly states **the session facade does not expose it**. The spec needs to name one home; the artifacts currently name three (session, ledger, absent).

### Changes Needed in Codebase

- **`cans/plugins/ledger.md`**: add a "Causal delta access" section naming `ledger.get_causal_delta_from_parent() -> CausalDelta` as canonical (aligns with the Rust TODO comment `"causal deltas are produced by plugin-ledger over the delta chain"`).
- **`packages/ts/core/src/context.lifecycle.types.ts`**: add `getCausalDeltaFromParent(): Promise<CausalDelta>` to `LedgerHandle`. Currently `LedgerHandle` has `flush`, `countDenials`, `finalize`, `blake3OfProducingProcess` — no causal-delta accessor. `Session.getCausalDeltaFromParent()` in `session.lifecycle.ts` delegates to `native.getCausalDeltaFromParent()`, which is a stub — the canonical path should be `session.cap.ledger.getCausalDeltaFromParent()`.
- **`packages/py/kelir/service_caps.py`**: `LedgerCap.get_causal_delta_from_parent()` already exists (as a stub with `"kelir native PyO3 bridge (_kelir) is missing: ledger get_causal_delta_from_parent (L3)"`). Keep as the canonical home. `Session.get_causal_delta_from_parent()` in `lifecycle_session.py` should either delegate to `session.cap.ledger.get_causal_delta_from_parent()` or be removed.
- **`crates/kelir-bindings/{napi,pyo3}/src/handles.flow.rs`**: `NativeSessionHandle::get_causal_delta_from_parent` / `PySessionHandle::get_causal_delta_from_parent` currently error with `"get_causal_delta_from_parent: causal deltas come from plugin-ledger (L3)"` — that is correct behavior, but it contradicts the spec (which does not mention it) and the examples (which call it on Session). Update the examples to call `session.cap.ledger.get_causal_delta_from_parent()` (Py/TS) or `session.get_causal_delta_from_parent()` (Rust — once the Rust session facade gains it, or keep the ledger handle as the source).
- **`crates/kelir-plugins/plugin-ledger/src/lib.rs`**: add a `pub async fn get_causal_delta_from_parent(&self) -> Result<CausalDelta, PluginError>` on `LedgerCtx` (currently `finalize`, `flush`, `count_denials`, `blake3_of_producing_process`). This is the Rust side of the canonical home; today the only surface is `compute_ledger::compute_causal_delta(storage, receipt)` — a free function, not a `LedgerCtx` method.
- **`db-backed-runtime.rs`**: its TODO says "session facade exposes no `get_causal_delta_from_parent` yet" — that is still true of `crates/kelir/src/session_lifecycle.flow.rs` (the only `get_causal_delta_from_parent` stubs are on the bindings' session handles). Keep the TODO but point at `plugin-ledger`'s `LedgerCtx` as the intended home rather than the session facade.

---

## 9. `ledger.flush()` Reachability Contradicts Itself Between Python (works) and TypeScript/Rust (dropped)

**Spec** (`cans/plugins/ledger.md`, "Ledger handler API"):
> `c.ledger.flush()` forces WAL flush and OTel export

`db-backed-runtime.py` claims this is reachable from `session.cap.ledger`:
```python
async def _pre_commit(session: Session) -> None:
    await session.cap.ledger.flush()
```

`db-backed-runtime.ts` says the same thing is **not reachable** from the equivalent hook:
```typescript
// TODO (L3): flush the ledger here; onPreCommit delivers a LifecycleContext only,
// and the booted session's ledger handle is not in reach at registration time.
```

`autonomous-swe-agent.py` and `.ts` both say the shutdown-time flush is dropped:
```python
# TODO: SSOT ledger.flush on shutdown dropped; live on_vm_shutdown delivers
# LifecycleCtx only, see cans/08-lifecycle.md#Session-and-context-handles
```

`db-backed-runtime.rs` sidesteps the problem by capturing `ledger_ctx` outside the closure:
```rust
.on_pre_commit({
    let ledger = ledger_ctx.clone();
    move |_ctx| Box::pin(async move { ledger.flush().await })
})
```

So `flush()` exists on the ledger context (Rust proves it), the Python example calls it, and two TS examples say the hook can't reach it. The spec doesn't specify whether the session or the lifecycle context exposes `flush()`, nor when. This is either three different bugs or three different specs.

### Changes Needed in Codebase

- **`cans/03-sdk-architecture.md` / `cans/08-lifecycle.md`**: state the reachability contract for `ledger.flush()`:
  - `LifecycleCtx` / `LifecycleContext` must carry the plugin capability struct (or a `session` reference), so `on_pre_commit`/`on_vm_shutdown` handlers can call `ctx.cap.ledger.flush()`.
  - The Rust example's capture pattern is valid but should be documented as the alternative (for handlers that hold a pre-boot ledger handle).
- **`packages/py/kelir/model_plugin.py`**: add `cap: Caps` to `LifecycleCtx` (currently `_vetoes` and `_watchdog` only). Update `__init__` signature to accept `cap` and `set_watchdog`/`register_veto` unchanged. `handler` in the fused `LifecycleCtx` gets populated by the fused `run` — which currently calls `hook.run(event)` with the raw payload; the runtime must wrap lifecycle payloads in a `LifecycleCtx` that already carries `cap`.
- **`packages/ts/core/src/context.lifecycle.types.ts`**: extend `LifecycleContext` with `readonly cap?: { readonly ledger?: LedgerHandle }` (or a `session` field). Currently `LifecycleContext` is `{ setWatchdog, registerVeto }` only — no path to the ledger handle.
- **`packages/py/kelir/plugin_fusion.py`**: `FusedTrampoline.runs()` currently calls `hook.run({"ring": ring, "ordinal": ordinal})` — for lifecycle hooks the payload must be the `LifecycleCtx` (with `cap` populated), not the ring/ordinal dict. Update the fused runner to distinguish lifecycle hooks and pass the correct ctx.
- **`packages/ts/core/src/fusion.plugin.ts`**: same — `FusedTrampoline.runs()` passes `{ ring, ordinal }`; for lifecycle hooks, pass the `LifecycleContext` (carrying `cap`).
- **`crates/kelir-engine/src/dispatch_plugin/lifecycle_dispatch.flow.rs`**: `dispatch_pre_commit` and `dispatch_lifecycle` call `fused_crossing(TrampolineRing::Lifecycle)` but do not pass `ctx` to the fused callbacks (`FusedTrampolineCallback::run(ring, ordinal)` takes only the ring and ordinal). Either:
  - Change `FusedTrampolineCallback::run` to accept an optional serialized event payload (so lifecycle hooks get `ctx`), or
  - Document the capture-at-registration pattern (as `db-backed-runtime.rs` does) as the canonical answer for lifecycle hooks that need pre-boot handles, and leave the fused path event-payload-free.
- **`db-backed-runtime.py`**: the `_pre_commit(session: Session)` signature is wrong — the fused runner does not pass `Session`. Change to `_pre_commit(ctx)` and capture the ledger handle via the builder (or via a `LifecycleCtx.cap.ledger` once that lands), matching the Rust capture pattern.
- **`db-backed-runtime.ts`**: keep the comment as a TODO but change it to say "capture the ledger handle at builder time (as in `db-backed-runtime.rs`); `onPreCommit` delivers `LifecycleContext` (which carries `cap.ledger` once the fused lifecycle payload lands)". Today it says "`LifecycleContext` only, and the booted session's ledger handle is not in reach at registration time" — that is accurate today but the spec needs to resolve the design, not preserve the limitation.
- **`autonomous-swe-agent.py` / `.ts`**: same `_pre_commit(session)`/`on_vm_shutdown` capture issue — the shutdown flush is currently dropped. Either capture the ledger handle at builder time or wait for `LifecycleCtx.cap.ledger` to land.

---

## 10. Checkpoint API: `session.vm.checkpoint(label)` vs `session.checkpoint.create(label)` vs `session.checkpoint.create_step(lineage, descriptor)` — Three Surfaces, One Spec

**Spec** (`cans/03-sdk-architecture.md`):
> Checkpoint: `session.checkpoint.create(label)`, `session.rollback()`

**Spec** (`cans/09-snapshots.md`):
> `session.lineage()` (no argument) — enumerated as an SDK accessor

**Artifacts:**

| File | Checkpoint API used | Conforms? |
|---|---|---|
| `git-gate.py` / `.ts` | `session.checkpoint.create("git-gate-base")` | ✅ `create(label)` |
| `multi-vm-swarm.py` / `.ts` | `child.checkpoint.create(...)` | ✅ `create(label)` |
| `replay.py` / `.ts` | `session.checkpoint.create(...)` | ✅ `create(label)` |
| `autonomous-swe-agent.py` | `session.vm.checkpoint("clean_base")` | ❌ `session.vm.checkpoint(label)` |
| `autonomous-swe-agent.ts` | `session.vm.checkpoint("clean_base")` | ❌ `session.vm.checkpoint(label)` |
| `policy-matrix.py` | `c.vm.checkpoint("policy-yield")` | ❌ `c.vm.checkpoint(label)` |
| `policy-matrix.ts` | `c.vm.checkpoint("policy-yield")` | ❌ `c.vm.checkpoint(label)` |
| `hook-matrix-full.rs` | `session.checkpoint.create_step(session.lineage(1), SnapshotDescriptor { … })` + `commit_step(lineage(1))` | ❌ `create_step`/`commit_step` pair |
| `hook-matrix.rs` (RS) | same `create_step` + `commit_step` | ❌ |
| `git-gate.rs` | same `create_step` + `commit_step` | ❌ |
| `replay.rs` | same `create_step` + `commit_step` | ❌ |
| `multi-vm-swarm.rs` | same `create_step` + `commit_step` | ❌ |
| `autonomous-swe-agent.rs` | same `create_step` + `commit_step` | ❌ |
| `policy-matrix.rs` | same `create_step` + `commit_step` | ❌ |

Two distinct divergences under one surface:

- **Host vs vm facade:** `session.checkpoint.create(...)` (documented) vs `session.vm.checkpoint(...)` (used by two plugin examples). The spec's surface table lists `session.checkpoint`, never `session.vm.checkpoint`.
- **Two-phase step API vs one-shot:** Every Rust example uses `create_step(lineage, descriptor)` + `commit_step(lineage)` — a two-phase API taking a `SnapshotDescriptor` — while every Py/TS example uses the documented one-shot `create(label)`. The spec documents only `create(label)` and does not mention `create_step`/`commit_step` or `SnapshotDescriptor` as construction inputs.
- **`session.lineage(1)`** is used in every Rust example (with argument `1`); the spec says `session.lineage()` (no argument).

### Changes Needed in Codebase

(Consolidated with item 3.)

- **`cans/03-sdk-architecture.md`** "Session-surfaces" table: split into the two surfaces, as described in item 3.
- **`cans/09-snapshots.md`**: change `session.lineage()` to `session.lineage(root_pid)` for Rust; document `session.checkpoint.create(label)` as the Py/TS one-shot surface.
- **`crates/kelir/src/session_lifecycle.flow.rs`**: add `pub fn lineage(&self, pid: u32) -> PidLineage` on `Session` (delegate to `self.core.lineage(pid)`). The Rust examples already rely on this through `Deref`, but the spec doesn't mention it.
- **`crates/kelir-bindings/{napi,pyo3}/src/handles.flow.rs`**: `NativeCheckpointHandle::create`/`PyCheckpointHandle::create` already exist and delegate to `VmHandle::checkpoint(label)`. For Rust parity on the Py/TS side, either:
  - Add `create_step`/`commit_step`/`lineage(pid)`/`head`/`compact` to the checkpoint handles, or
  - Document that Rust's two-phase step API is Rust-only (it is `CheckpointPlane` on the engine, not a `Session` method exposed through the bindings).
- **`crates/kelir/src/session_lifecycle.flow.rs`**: note that `Session` derefs to `kelir_engine::Session`, so `session.checkpoint.create_step(...)` resolves through `Deref` to `engine::Session::checkpoint: Arc<CheckpointPlane>`. That is the surface the Rust examples use — keep it, but document the Deref contract explicitly (the spec does not today).
- **`cans/artifacts/examples/autonomous-swe-agent.py` / `.ts`** and **`policy-matrix.py` / `.ts`**: replace `session.vm.checkpoint(...)` / `c.vm.checkpoint(...)` with `session.checkpoint.create(...)` / `c.checkpoint.create(...)`. Both resolve through the bindings, but the examples should not teach the undocumented `vm.checkpoint` facade for the one-shot path.
- **`cans/artifacts/examples/*.rs`**: keep `create_step`/`commit_step`/`lineage(1)` but update the spec to name them as the Rust low-level surface (the engine `CheckpointPlane` is the SSOT for the two-phase API; `CheckpointOps` in `crates/kelir-interface/src/checkpoint/ops.flow.rs` is the trait).