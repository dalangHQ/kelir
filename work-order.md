# Misalignments: cans/artifacts/examples/ vs cans/ spec

I cross-checked every example against the normative spec. Below are the concrete mismatches, ordered by severity.

---

1. session.terminate() vs session.stop() — TS examples are non-conforming

The spec only defines session.stop(), session.stop_with_timeout(d), session.kill(), session.drain(ms) (08-lifecycle.md#Pre-teardown-flush, 08-lifecycle.md#Hypervisor-primitives). Rust and Python examples correctly use stop() / async with:

· container.rs, hook-matrix.rs, hook-matrix-full.rs, policy-matrix.rs, git-gate.rs, replay.rs → session.stop()
· hook-matrix.ts, hook-matrix-full.ts, container.ts, db-backed-runtime.ts, policy-matrix.ts, serverless-webhook.ts → session.terminate() (undefined on Session)

git-gate.ts and replay.ts use stop() correctly — so TS is internally inconsistent as well as out of step with the spec.

**Codebase changes needed:**
- `crates/kelir-engine/src/session_lifecycle.flow.rs`: add `pub fn terminate(&mut self)` delegating to `kill()` so the native engine exposes the same verb Python/TS already call.
- `crates/kelir/src/session_lifecycle.flow.rs`: re-export `terminate` on the SDK `Session`.
- `packages/py/kelir/lifecycle_session.py`: already calls `native.terminate()`; keep. Align `stop()` (graceful) vs `terminate()` (abrupt = kill) in one place.
- `packages/ts/core/src/session.lifecycle.ts`: `Session.terminate()` already calls `native.terminate()`; keep.
- `cans/08-lifecycle.md#Pre-teardown-flush`: document `terminate()` explicitly as the abrupt variant (alias for `kill()`, no flush), alongside `stop()`, `stop_with_timeout()`, `drain()`, `kill()`.

---

2. Undocumented APIs used in the autonomous-swe-agent family

These appear in RS/TS/Py but appear nowhere in the spec's HandlerCtx / Session surface:

API File(s) Note
c.step.last() autonomous-swe-agent.{rs,ts,py} HandlerCtx is pinned to c.trap, c.vm, c.receipt(), c.params::<T>() in 02-plugin-model.md#Core-context-keys; no c.step
c.terminal.notice(...) autonomous-swe-agent.{rs,ts,py} Spec route is "publish TerminalNotice to EventBus" (04-terminal-plane.md#Disclosure); no .notice() API
c.cap.ledger.blake3_of_producing_process(path) autonomous-swe-agent.py no such method in plugins/ledger.md
session.cap.ledger.count_denials(pid) autonomous-swe-agent.py no such method in plugins/ledger.md
session.handleIngress(...) serverless-webhook.ts RS/Py examples explicitly comment "exposes no handle_ingress"; TS calls it
MeshCtx::await_winner(...) multi-vm-swarm.rs TS/Py examples explicitly comment "MeshHandle exposes peers only, with no awaitWinner"; RS calls it

**Codebase changes needed (document, don't remove):**
- `cans/02-plugin-model.md#Core-context-keys`: add `c.step` (StepHandle, `last() -> SnapshotDescriptor`) to the pinned HandlerCtx key list.
- `cans/04-terminal-plane.md#Terminal-pipeline` (or #Disclosure): document `c.terminal.notice(msg)` as the canonical TerminalNotice publisher alongside the EventBus route; `crates/kelir-interface/src/handles_lifecycle.flow.rs` `TerminalHandle::notice` already exists — spec must reference it.
- `cans/plugins/ledger.md#Ledger-handler-API`: add `blake3_of_producing_process(path) -> str` and `count_denials(ppid) -> int` — both already implemented in `crates/kelir-plugins/plugin-ledger/src/lib.rs` `LedgerCtx`. Spec has the anchor but omits these methods.
- `cans/plugins/wire-ingress.md` + `cans/03-sdk-architecture.md#Session-surfaces`: document `session.handleIngress({cwd?, handler}) -> AsyncGenerator`; wire `PySessionHandle`/`NativeSessionHandle::handle_ingress` from the current `Err(Halt)` stub once `handleIngress` is spec'd.
- `cans/plugins/mesh.md#CoW-swarm-mesh-federation`: document `MeshCtx::await_winner(timeout) -> u32`; the method exists in `crates/kelir-plugins/plugin-mesh/src/mesh_state.flow.rs` (currently `Err(Halt)` TODO), TS/Py need to add it once spec'd.

---

3. RepoPolicyPlugin prerequisite parity break

The spec insists plugin prerequisites are 1:1 across languages (03-sdk-architecture.md#Cross-SDK-guarantees).

· autonomous-swe-agent.py: class RepoPolicyPlugin: requires = ("ledger",)
· autonomous-swe-agent.ts: createPlugin("repo-policy", ...).requires("ledger")
· autonomous-swe-agent.rs: impl PluginDeps for RepoPolicyPlugin { type Deps = NoDeps; }

Rust declares no dependency where Py/TS declare ledger. This is exactly the kind of divergence #Prerequisite-gating-across-languages forbids.

**Codebase changes needed:**
- `crates/kelir-plugins/` has no repo-policy crate — add `plugin-repo-policy` (or wire `RepoPolicyPlugin` in the example crate) with `type Deps = HCons<plugin_ledger::LedgerCtx, HNil>;` so the Rust side matches Py/TS.
- `crates/kelir/src/lib.rs`: re-export the new plugin under the `all-plugins` feature.
- `autonomous-swe-agent.rs` example: change `type Deps = NoDeps;` → `type Deps = HCons<LedgerCtx, HNil>;` and `.require::<LedgerPlugin>()` (or `.plugin(ledger_builder(...))`) at build time.
- `crates/kelir-interface/src/plugin_model.flow.rs`: `Satisfies<Deps>` already supports this; no engine change.

---

4. Spec text contradicts current db-backed-runtime.py

plugins/db-proxy.md (SSOT-A1 pin) states:

The Py example reading c.cap.db is non-conforming
See: cans/artifacts/examples/db-backed-runtime.py:84

Line 84 of the current file reads c.cap.db_proxy.rewrite_target (conforming to 03-sdk-architecture.md#Service-lookup-across-languages, which says the capability key is db-proxy, Py-spelled db_proxy). The spec is stale — it describes a non-conforming usage that no longer exists in the artifact.

**Codebase changes needed:**
- `cans/plugins/db-proxy.md`: update the stale note to reference the conforming spelling `c.cap.db_proxy.rewrite_target` (Py) / `c.cap.dbProxy.rewriteTarget` (TS) / `c.cap.db_proxy.rewrite_target` (RS), and remove the claim that the example reads `c.cap.db`.
- `cans/03-sdk-architecture.md#Service-lookup-across-languages`: reaffirm that `db-proxy` maps to `db_proxy` (Py) / `dbProxy` (TS) / `db_proxy` (RS) in the `cap` bag; the existing table covers this — the db-proxy.md note is the only contradiction.

---

5. Non-POSIX errno in db-backed-runtime.py

```python
return Verdict.drop("EdbProxySchema", ...)
```

Verdict::Drop carries Option<Errno> per 02-plugin-model.md#Unified-verdict; Errno is the fixed POSIX-ish enum (Errno::Eperm, Errno::Enospc, Errno::Epolicy, …). EdbProxySchema is not a defined variant — no other language uses a free-form string errno.

**Codebase changes needed:**
- `cans/artifacts/examples/db-backed-runtime.py`: replace `Verdict.drop("EdbProxySchema", ...)` with `Verdict.drop("Epolicy", ...)` (or `Econfig` if it is a config-shaped failure). The frame-schema mismatch on the wire is an `Epolicy`/`Eproto` case per `crates/kelir-interface/src/errno_types.flow.rs`.
- `crates/kelir-interface/src/errno_types.flow.rs`: do **not** add `EdbProxySchema` — the enum is deliberately POSIX-shaped. If a non-POSIX code is truly needed, the SSOT is `error_envelope.flow.rs` (`EConfig`, `EWXVIOLATION`, `EFFIQUEUESATURATED`, `EAMBIGUOUSMUTATION`, `EHALT`) — use one of those codes in the envelope, not in `Verdict::Drop`'s `errno`.

---

6. Ledger finalize asymmetry (same call, three shapes)

plugins/ledger.md pins: finalize(FinalizeRequest { include_delta: CausalDelta, include_merkle_root }) and describes ledger.get_causal_delta_from_parent() as the canonical accessor.

· db-backed-runtime.py: include_delta=await ledger.get_causal_delta_from_parent() ✅
· db-backed-runtime.ts: includeDelta: await ledgerHandle.getCausalDeltaFromParent() ✅
· db-backed-runtime.rs: include_delta: CausalDelta::new(0, vec![], vec![], 0, [0; 32], [0; 32]) with a // TODO (L3) that says the session facade has no get_causal_delta_from_parent "yet"

The three examples demonstrate three different answers to the same spec pin. If Py/TS can call it, RS should either call it or all three should carry the same TODO.

**Codebase changes needed:**
- `crates/kelir-plugins/plugin-ledger/src/lib.rs` `LedgerCtx::get_causal_delta_from_parent`: currently returns `Err(Halt)`. Implement it — assemble the delta from the fork `StorageDelta` plus the parent `ExecutionReceipt` via `compute_ledger::compute_causal_delta`. Then `db-backed-runtime.rs` can call `ledger_handle.get_causal_delta_from_parent().await?`.
- `crates/kelir/src/session_lifecycle.flow.rs` `Session`: add `pub async fn get_causal_delta_from_parent(&self)` delegating to the ledger context (parity with Py `Session.get_causal_delta_from_parent` and TS `NativeSessionHandle.getCausalDeltaFromParent`).
- `crates/kelir-plugin-cli` / examples: drop the zeroed `CausalDelta::new(...)` stand-in; replace with the real accessor.

---

7. Broken spec anchor in autonomous-swe-agent.rs

```rust
// see: cans/10-attestation.md#ExecutionReceipt-records-final-binary-blake3
```

10-attestation.md has no #ExecutionReceipt-records-final-binary-blake3 anchor. Existing anchors are #Execution-receipts and #binary_blake3-identity. This will fail cans check --refs-only.

The file also references artifacts/code/execution-receipt.rs (and many examples reference artifacts/code/*.rs), but the directory tree listed in the repo dump contains no artifacts/code/ directory at all — those targets are missing.

**Codebase changes needed:**
- `cans/10-attestation.md`: either add the anchor `#ExecutionReceipt-records-final-binary-blake3` to the existing `#Execution-receipts` node (rename/split), or fix the example to point at the existing anchor `#binary_blake3-identity` / `#Execution-receipts`.
- `cans/artifacts/code/`: **create the missing directory** and add the referenced artifacts (`execution-receipt.rs`, `vmm-driver-trait.rs`, `plugin-trait.rs`, `wire-protocol.rs`, `messages.rs`, `transaction-service.rs`, `container-service.rs`, `causal-delta.rs`) — every `// see: cans/artifacts/code/...` ref in the tree currently resolves to a non-existent path; the artifacts are the SSOT and must exist. Keep them byte-identical to the spec.
- `cans/_rules.yaml`: add an exclusion or fix so `cans check --refs-only` resolves the anchor once added.

---

8. Console-output masking drops cross-chunk carryover (spec violation)

04-terminal-plane.md#Terminal-pipeline (step 4):

Interposer dispatch: fires ConsoleInterposer::on_console_output for in-place masking
Maintains MAX_PLACEHOLDER_LEN - 1 carryover window across chunk boundaries

Yet every hook-matrix variant masks on_console_output per chunk, with no carryover:

· hook-matrix-full.rs → let mut out = chunk.to_vec(); mask_tokens(&mut out);
· hook-matrix-full.ts → maskTokens(chunk) (no consoleTail)
· hook-matrix-full.py → mask_tokens(chunk)

Their on_fs_read implementations do implement carryover (hookState.tail, state.tail, self._state.tail) — so the omission is specific to console output. hook-matrix.ts carries a TODO acknowledging it ("live matrix.hook.test.ts masks console output cross-chunk … SSOT keeps single-chunk masking here"), which contradicts the spec text.

**Codebase changes needed:**
- `cans/artifacts/examples/hook-matrix-full.rs`: add a `console_tail: String` (or `Vec<u8>`) on the plugin state; prepend it to `chunk`, run `mask_tokens`, retain the trailing `MAX_PLACEHOLDER_LEN - 1` bytes for the next chunk.
- `cans/artifacts/examples/hook-matrix-full.ts`: add `consoleTail`; prepend, mask, trim to `MAX_PLACEHOLDER_LEN - 1`.
- `cans/artifacts/examples/hook-matrix-full.py`: add `self._console_tail`; same pattern.
- `cans/artifacts/examples/hook-matrix.rs`/`.ts`/`.py`: remove the contradictory TODO; if the short variant is intentionally single-chunk, the spec text must say so (or drop the short variant).
- `cans/04-terminal-plane.md#Terminal-pipeline`: reaffirm `MAX_PLACEHOLDER_LEN - 1` carryover applies to *both* `on_console_output` and `on_fs_read`; the existing text says it does — examples are the bug.

---

9. session.vm vs session.vm()

Spec is internally split:

· 03-sdk-architecture.md#Session-surfaces: session.vm.export_snapshot(), session.vm.adjust_balloon(...), session.vm.set_vcpus(n) — property-style
· 09-snapshots.md#Archives-and-portability: session.vm().export_snapshot() — method

Examples follow both:

· db-backed-runtime.rs: session.vm().import_snapshot(...) / session.vm().export_snapshot() (method)
· autonomous-swe-agent.rs, policy-matrix.rs: session.vm.checkpoint(...), session.vm.rollback(...) (property) — via c.vm in handlers

This is a Session surface ambiguity that at least one example will violate no matter what the real API is.

**Codebase changes needed:**
- `cans/03-sdk-architecture.md#Session-surfaces`: pin `session.vm` as a **property** (matching TS `readonly vm` and Py attribute). Change the one method-style call in 09-snapshots.md (`session.vm().export_snapshot()`) to `session.vm.export_snapshot()`.
- `crates/kelir/src/session_lifecycle.flow.rs` `Session`: currently exposes `pub fn vm(&self)` — add a `pub vm: VmHandle` field (or a `Deref` shim) so `session.vm.export_snapshot()` compiles; keep `vm()` as a deprecated alias if needed. Same treatment for `terminal`, `storage`, `wire`, `checkpoint`, `host`, `vsock`, `step`.
- `crates/kelir-engine/src/session_lifecycle.flow.rs`: same property surface.
- `cans/artifacts/examples/db-backed-runtime.rs`: change `session.vm().export_snapshot()` → `session.vm.export_snapshot()`.

---

10. trap_async / post_exec string-filter form (minor)

autonomous-swe-agent.rs:

```rust
.trap_async("*", { ... })
.post_exec("*", { ... })
```

Passes a raw "*" where every other call site (RS/TS/Py) uses TrapFilter::binary(...) or TrapFilter.binary(...). Same pattern in serverless-webhook.{rs,ts,py} (trap("dispatch-webhook", ...)). If trap/trap_async accept impl Into<TrapFilter>, fine — but the spec's #Chainable-verbs / #Verbs-in-both-modes section doesn't pin this coercion, and the RS comment ("Rust trap filters only match binary/pattern, so the string filter stands in for the action") admits it's a stand-in, not the intended API.

**Codebase changes needed:**
- `cans/02-plugin-model.md#Chainable-verbs` (and `#Verbs-in-both-modes`): document that `trap`/`trap_async`/`post_exec` accept `impl Into<TrapFilter>`, where `&str` is coerced (`"*"` = match-all, other strings = `TrapFilter::pattern(str)`).
- `crates/kelir/src/builder_sdk.flow.rs`: the `impl From<&str> for TrapFilter` already exists — ensure it treats `"*"` as wildcard (`binary: None, pattern: None, args: []`) and any other string as `binary: Some(s)`. Current code sets `pattern: None` for both; document the `"*"` behavior explicitly.
- `crates/kelir-engine/src/dispatch_plugin/engine.flow.rs` / `trap_gate.flow.rs`: confirm `"*"`-shaped `TrapFilter` falls through the gate rather than short-circuiting; no code change if the coercion is correct.

---

11. snoops config field is undocumented and Rust-only-missing

.storage() is documented in 05-storage-plane.md with: idmap, shared_caches, lock_broker, synthetic_files, mask_read, encapsulation, scan_engine, carryover_bytes, quotas, cow_broker, reconciler, ephemeral_scratch. There is no snoops field.

Yet:

```python
# autonomous-swe-agent.py
.storage(
    idmap=IdmapConfig(...),
    shared_caches=[...],
    lock_broker=True,
    synthetic_files=[...],
    mask_read=[...],
    snoops=[
        SnoopRule(
            glob="~/.claude/sessions/*.json",
            sync=True,
            on_write=make_snoop(oracle),
        ),
    ],
)
```

```ts
// autonomous-swe-agent.ts
.storage({
  idmap: {...},
  sharedCaches: [...],
  lockBroker: true,
  syntheticFiles: [...],
  maskRead: [...],
  snoops: [
    { path: "~/.claude/sessions/*.json", sync: true,
      onWrite: async (path, raw, c) => {...} },
  ],
})
```

But autonomous-swe-agent.rs has no snoops field at all:

```rust
.storage(StoragePlaneConfig {
    idmap: IdmapConfig {...},
    shared_caches: vec![...],
    lock_broker: true,
    synthetic_files: vec![...],
    mask_read: vec![...],
    encapsulation: WorkspaceEncapsulation::Passthrough,
    scan_engine: ReadMaskEngine::FastRegex,
    carryover_bytes: 1024,
})
```

Three-way parity break: Py and TS demonstrate a config feature (SnoopRule) that is (a) absent from the spec and (b) absent from the Rust SDK. This is exactly what the parity gate should catch — either the spec needs a snoops section and RS needs the field, or Py/TS need to drop the feature.

**Codebase changes needed (document + implement in Rust):**
- `cans/05-storage-plane.md#Storage-architecture`: add a `snoops` subsection — `SnoopRule { path: glob, sync: bool, on_write: Fn(path, raw, ctx) }` — firing on guest writes to matching paths.
- `crates/kelir-interface/src/storage/mod.rs`: add `SnoopRule` (path glob, sync flag, callback trait `SnoopCallback`) alongside `SyntheticFile`.
- `crates/kelir/src/planes/storage_plane.rs`: add `pub snoops: Vec<SnoopRule>` to `StoragePlaneConfig`.
- `crates/kelir-engine/src/planes/storage/plane.flow.rs`: consume `snoops` — register them as `FsInterposer` hooks on the write path (per `02-plugin-model.md#Unified-interposition-pipeline`) so `on_write` fires with `(path, raw, ctx)`.
- `crates/kelir-bindings/napi/src/storage_bridge.rs` and `crates/kelir-bindings/pyo3/src/storage_bridge.rs`: the Py/TS paths already accept the field; wire it through to the new Rust `SnoopRule`.
- `autonomous-swe-agent.rs` example: add the `snoops: vec![...]` field matching Py/TS.

---

12. SuspendReason has three different shapes

08-lifecycle.md#Suspend-and-resume pins the canonical form:

Canonical signature (SSOT-A1 pin)
Session.suspend(reason: SuspendReason) / Session.resume()

And #Hypervisor-primitives pins the enum:

SuspendReason: External(ExternalSuspendKind) | ResourcePressure

Yet the examples construct it three different ways:

· Rust (serverless-webhook.rs, autonomous-swe-agent.rs):
  ```rust
  session.suspend(SuspendReason::External(
      ExternalSuspendKind::HumanAuthorizationHold,
  )).await?;
  session.suspend(SuspendReason::ResourcePressure).await?;
  ```
  Proper enum syntax.
· TypeScript (serverless-webhook.ts, policy-matrix.ts):
  ```ts
  await session.suspend(SuspendReason.external("human-authorization-hold"));
  await session.suspend(SuspendReason.resourcePressure);
  ```
  A factory class — SuspendReason is a static namespace, not a value.
· Python (policy-matrix.py, serverless-webhook.py):
  ```python
  await session.suspend(ResourcePressure())
  await session.suspend(External(ExternalSuspendKind.HUMAN_AUTHORIZATION_HOLD))
  ```
  Two peers: ResourcePressure looks like a class instantiation, External(...) looks like a function call. Within Python itself the two forms don't look alike — one takes (), the other takes a payload.

"Same concept, per-SDK spelling" doesn't cover this. These are three different value shapes for the same parameter, and Py is internally inconsistent (ResourcePressure() vs External(...)).

**Codebase changes needed:**
- `cans/08-lifecycle.md#Suspend-and-resume`: pin the *shape* per language:
  - RS: `SuspendReason::External(ExternalSuspendKind::HumanAuthorizationHold)` / `SuspendReason::ResourcePressure` (unit variant).
  - TS: `SuspendReason.external("human-authorization-hold")` / `SuspendReason.resourcePressure` (factory class — match `packages/ts/core/src/verdict.protocol.ts`).
  - Py: `External(ExternalSuspendKind.HUMAN_AUTHORIZATION_HOLD)` / `ResourcePressure()` — **fix the inconsistency** by making `ResourcePressure` an instance used as `ResourcePressure` (no parens) or `External(...)` a class instantiation; pick one and pin it.
- `packages/py/kelir/__init__.py`: reconcile `class ResourcePressure: pass` (bare class) vs `@dataclass class External: kind` — make both consistently instantiated (`ResourcePressure()` and `External(kind)`) and update the docstring/`__all__` to reflect the pinned shape.
- `packages/ts/core/src/verdict.protocol.ts` already has the factory form; no change.

---

13. FilterAction constructors diverge — and diverge within the same SDK

Spec 02-plugin-model.md#Unified-verdict:

FilterAction = { Forward, Drop{errno,reason}, Suspend{until}, Abort{reason} }

SuspendUntil is pinned in 11-protocol.md#Verdict-method-mapping:

SuspendUntil: Deadline(ms) | Approval | Handshake

Examples:

hook-matrix-full.{rs,ts,py} (the "canonical" hook surface):

· RS: FilterAction::Suspend { until: SuspendUntil::Deadline(0) }
· TS: FilterAction.Suspend(SuspendUntil.deadline(0))
· Py: FilterAction.suspend(until=Deadline(0))

hook-matrix.{ts,py} (the short variant):

· TS: FilterAction.Abort("inbound abort marker") — single-argument positional
· Py: FilterAction.abort(reason="inbound abort marker") — kwarg

hook-matrix-full.ts (short variant of the same op):

· FilterAction.Suspend(SuspendUntil.deadline(0)) — positional argument

So within TS: FilterAction.Suspend(x) takes positional, but FilterAction.Abort(x) — the interface pinned in spec has fields {until} and {reason}, not positional. The TS example erases both field names. Same for Py — one call uses until=, another uses reason=.

The spec pins field names. The examples erase them inconsistently.

**Codebase changes needed:**
- `cans/02-plugin-model.md#Unified-verdict`: pin `FilterAction.Suspend({ until })` and `FilterAction.Abort({ reason })` as object-shaped constructors in TS/Py (field names visible at call site).
- `packages/ts/core/src/verdict.protocol.ts`: change `Suspend(until: SuspendUntil)` → `Suspend({ until }: { until: SuspendUntil })` and `Abort(reason?: string)` → `Abort({ reason }: { reason?: string })`. Same for `Verdict.Suspend`/`Verdict.Abort`. Update `hook-matrix-full.ts` and `hook-matrix.ts` call sites.
- `packages/py/kelir/__init__.py` `Verdict`/`FilterAction`: `suspend(cls, until=None)` and `abort(cls, reason="")` — change to require/accept object-shaped payloads consistently (`FilterAction.suspend(until=...)` vs `FilterAction.abort(reason=...)` is fine as kwargs; the bug is TS dropping names).
- `hook-matrix.{ts,py}`: make the short and full variants use the same shape.

---

14. on_fs_write — mutable vs immutable buffers breaks the "in-place mutation" contract

02-plugin-model.md#Read/console-pin says:

Write/net hooks mutate the passed buffer and return FilterAction
RS mutates in place and returns () here; it is the outlier

But the buffer types diverge:

· Rust (autonomous-swe-agent.rs, hook-matrix-full.rs, hook-matrix.rs, git-gate.rs):
  ```rust
  fn on_fs_write(&self, path: &Path, data: &mut [u8], _pid: u32, _ctx: &HandlerCtx) -> Result<FilterAction, PluginError>
  ```
  &mut [u8] — mutable slice, mutation is possible.
· Python (hook-matrix-full.py):
  ```python
  def on_fs_write(self, path: Path, data: bytes, pid: int, ctx: HandlerContext) -> FilterAction:
  ```
  bytes — immutable. Nothing can be mutated.
· TypeScript (hook-matrix-full.ts, hook-matrix.ts, unified-builder-surface.ts, fluent-builder-chaos.ts):
  ```ts
  .onFsWrite((_path, buffer, _pid, _ctx) => FilterAction.Forward)
  ```
  buffer: Buffer — technically mutable, but no example mutates it.

So the "mutate the passed buffer" model works only in Rust. In Py, the type system precludes it. The spec's claim that Py/TS mutate in place is not implementable given bytes/Buffer semantics.

If bytes is meant to be bytearray for mutation, examples are wrong. If Buffer is meant to be mutated, no example demonstrates it. Either way, spec and examples disagree on whether "in-place mutation" is a capability.

**Codebase changes needed:**
- `cans/02-plugin-model.md#Read/console-pin`: pin the buffer type per language — RS `&mut [u8]`, TS `Buffer` (mutable; document that the bridge copies back on return), Py **`bytearray`** (mutable; `bytes` is read-only).
- `cans/artifacts/examples/hook-matrix-full.py`: change `data: bytes` → `data: bytearray` and add a `data[:] = ...` mutation in the example.
- `cans/artifacts/examples/hook-matrix.ts`/`autonomous-swe-agent.ts`/`git-gate.ts`: add at least one example that mutates `buffer` in place to demonstrate the capability.
- `crates/kelir-bindings/pyo3/src/handles.flow.rs`: the Py↔Rust bridge already exposes `&mut [u8]` on the Rust side; document that Py receives `bytearray` (writable) and returns a `bytes` snapshot per call.
- `crates/kelir-bindings/napi/src/handles.flow.rs`: `Buffer` is already `Uint8Array`-mutable; document copy-back semantics.

---

15. set_watchdog arguments differ three ways — and no cross-language shape is pinned

Spec 08-lifecycle.md#Watchdog:

Armed via LifecycleCtx.set_watchdog … WatchdogAction: Reboot | Terminate

Signature is not pinned beyond the verb name. Examples build three different shapes:

· Rust (autonomous-swe-agent.rs):
  ```rust
  ctx.set_watchdog(Duration::from_secs(5 * 60), kelir::WatchdogAction::Reboot)
  ```
  (Duration, WatchdogAction) positional.
· Python (autonomous-swe-agent.py):
  ```python
  ctx.set_watchdog(timeout=timedelta(minutes=5), action="reboot")
  ```
  (timeout=timedelta, action=str) kwargs.
· TypeScript (autonomous-swe-agent.ts):
  ```ts
  driver.setWatchdog({ timeoutMs: 5 * 60_000, action: "reboot" })
  ```
  Single object with timeoutMs: number.

Three different times: Duration vs timedelta vs number. Three different action types: enum variant vs string vs string. Three different parameter shapes: positional vs kwargs vs object.

The spec has one name and zero argument shape. This is the kind of thing #Multi-language-parity-gate should catch via the AST surface manifest, but the spec must first say what the shape is.

**Codebase changes needed:**
- `cans/08-lifecycle.md#Watchdog`: pin the per-language shape:
  - RS: `set_watchdog(timeout: Duration, action: WatchdogAction)` (positional, `Duration` native).
  - TS: `setWatchdog({ timeoutMs: number, action: WatchdogAction })` (object; `timeoutMs` is the wire unit; `action` is `"reboot" | "terminate"`).
  - Py: `set_watchdog(timeout: timedelta, action: WatchdogAction)` (kwargs; `timedelta` native).
- `crates/kelir-interface/src/plugin_model.flow.rs` `LifecycleCtx::set_watchdog`: already `(Duration, WatchdogAction)` — matches RS.
- `packages/ts/core/src/context.lifecycle.types.ts`: already object-shaped `{ timeoutMs, action }` — matches TS.
- `packages/py/kelir/model_plugin.py` `LifecycleCtx.set_watchdog`: currently accepts `(timeout: int | timedelta, action: str)` — accept `timedelta` only, `action: WatchdogAction` (add the enum), and pin the shape in the docstring.
- `packages/py/kelir/__init__.py`: export `WatchdogAction` (Reboot / Terminate) so the type matches RS/TS.

---

16. on_trap_deny returns Result in Rust, void in TS/Py

03-sdk-architecture.md#Unified-builder-surface says .on_trap_deny is host-only, but does not pin return type.

· Rust (autonomous-swe-agent.rs):
  ```rust
  .on_trap_deny(|_event, _session| async move { Ok::<(), PluginError>(()) })
  ```
  Returns Result<(), PluginError> — the hook can fail.
· Python (autonomous-swe-agent.py):
  ```python
  async def _deny(event: TrapEvent, session: Session) -> None:
      ...
  ```
  Returns None — cannot fail.
· TypeScript (autonomous-swe-agent.ts):
  ```ts
  .onTrapDeny(async (e, s) => { ... })
  ```
  Returns Promise<void> — cannot fail.

The Rust form can propagate PluginError, e.g. if await s.vm.suspend(...) inside the callback fails. The TS/Py forms silently swallow errors, or would need throw/raise. If the callback's whole purpose is to await s.console() after a quarantine suspend (as in autonomous-swe-agent.{py,ts,rs}), a failure inside s.console() has different consequences per language.

Parity gate needs the spec to pin the return type. Currently the three examples demonstrate three different failure contracts.

**Codebase changes needed:**
- `cans/03-sdk-architecture.md#Unified-builder-surface`: pin `on_trap_deny` return type — `Result<(), PluginError>` in RS, `Promise<void>` in TS (throws on failure), `Awaitable[None]` in Py (raises on failure). Failures propagate as session errors, not silent no-ops.
- `packages/ts/core/src/builder.lifecycle.ts` `onTrapDeny`: callback type already `Promise<void> | void`; add a wrapper that rethrows on error (`EConfig.fromNative` for envelope-shaped rejects).
- `packages/py/kelir/kelir_builder.py` `on_trap_deny`: same — wrap in `try/except` and re-raise as `EConfig`.
- `crates/kelir-engine/src/dispatch_plugin/registration.flow.rs` `TrapDenyCallback`: already `Result<(), PluginError>` — matches RS; document that Py/TS bridge to `raise`/`reject`.

---

17. fluent-builder-chaos.rs uses undocumented default_vm_config helper

```rust
pub fn build_chaos_host(config: ChaosConfig) -> ChaosHostBuilder {
    KelirBuilder::new(default_vm_config(None, None, None, None, None, None))
        .plugin_instance(ChaosPlugin::new(config))
}
```

default_vm_config is not mentioned anywhere in the spec. KelirBuilder::new(VmConfig) is the pinned form (02-plugin-model.md#Unified-builder), and every other example across all three languages constructs a VmConfig explicitly:

```rust
// hook-matrix.rs
KelirBuilder::new(VmConfig {
    vcpus: 2,
    memory_mb: 2048,
    kernel_path: ...,
    ...
})

// fluent-builder-chaos.py
kelir.builder(vcpus=1, memory_mb=256, kernel=..., rootfs_erofs=..., ...)
```

Only fluent-builder-chaos.rs uses the default_vm_config(None, None, None, None, None, None) form, and the six None arguments have no documented semantics. If it is a convenience helper it must be spec'd (or removed). As written, the "canonical chaos example" is demonstrating an undocumented alternate entry path that neither sibling language has.

**Codebase changes needed (document + mirror in Py/TS):**
- `cans/03-sdk-architecture.md` (or `01-architecture.md#Embedded-profile`): document `default_vm_config(vcpus, memory_mb, kernel, rootfs_erofs, tsc_locked, enable_watchdog) -> VmConfig` — it's already public in `crates/kelir-interface/src/driver_interface.flow.rs`.
- `cans/artifacts/examples/fluent-builder-chaos.rs`: replace the six `None`s with a named helper call (e.g. `default_vm_config()` with defaults) or construct `VmConfig` explicitly. If `default_vm_config` is kept, add a `see:` to the spec node.
- `packages/py/kelir/kelir_builder.py` `builder()`: already accepts `vcpus=None, memory_mb=None, ...` — document that it forwards to `default_vm_config` with the same defaults, so Py is parity-equal.
- `packages/ts/core/src/builder.config.ts` `KelirConfig`: optional fields already parity-equal to `default_vm_config`; add a `see:` in `session.lifecycle.ts`.

---

18. RepoPolicyPlugin::fork_state calls blocking_lock() inside a synchronous function — Tokio panic hazard

```rust
// autonomous-swe-agent.rs
impl KelirPlugin for RepoPolicyPlugin {
    fn fork_state(&self, _child_session_id: &str) -> Box<dyn AnyPlugin> {
        Box::new(Self {
            opts: self.opts.clone(),
            state: Arc::new(Mutex::new(self.state.blocking_lock().clone())),
        })
    }
}
```

fork_state is synchronous (02-plugin-model.md#KelirPlugin-trait: "Typed lifecycle: name, default_scope, is_enabled, opt-in fork_state"). Its body runs on whatever thread invoked it. tokio::sync::Mutex::blocking_lock() panics when called from within a Tokio runtime context:

Cannot block the current thread from within a runtime. This happens because a function attempted to block the current thread while the thread is being used to drive asynchronous tasks.

session.fork() is itself async and is called from inside #[tokio::main] in main(). If fork_state runs on the same task thread (it will, in the canonical implementation), the example panics at the first fork.

The example's own policy-matrix.rs handles this correctly by copying the state manually (AtomicBool::new(self.state.ready.load(...))) without blocking_lock. Either RepoPolicyPlugin should do the same, or the spec should mandate that fork_state runs outside the runtime — which it currently doesn't say.

**Codebase changes needed:**
- `cans/artifacts/examples/autonomous-swe-agent.rs`: replace `Arc::new(Mutex::new(self.state.blocking_lock().clone()))` with a non-blocking copy — either drop the `Mutex` around the copied state (`Arc::new(self.state.try_lock().map(|s| s.clone()).unwrap_or_default())`) or, better, restructure the state to `AtomicBool` (like `policy-matrix.rs`) so `fork_state` is sync-safe.
- `cans/02-plugin-model.md#KelirPlugin-trait`: add a one-line note — `fork_state` runs on the async runtime thread; plugin state must be cloned without blocking (`try_lock`, atomics, `Arc::clone`), never `blocking_lock`/`block_on`.
- `crates/kelir-engine/src/session_lifecycle.flow.rs` `Session::fork`: it calls `plugin.fork_state(...)` inside an `async` block; document that `fork_state` must not block. Optionally assert the current thread is not a runtime worker.
- `crates/kelir/src/builder_plugin.flow.rs` `fork_state` impl: same — currently `self.clone_box()`; safe.

---

19. unified-builder-surface.rs — return type doesn't grow with mount

```rust
pub type SurfaceHostBuilder = KelirBuilder<
    Host, Unsealed, (), HNil, HCons<LedgerCtx, HNil>, NoParams
>;

pub fn build_host() -> SurfaceHostBuilder {
    KelirBuilder::new(VmConfig { ... })
        .plugin_instance(LedgerPlugin::new(true))     // → LedgerCtx into Plugins
        .plugin(build_surface_plugin())               // → SurfaceCtx into Plugins
        .r#use(build_plain_plugin())                  // → ??? into Plugins
        .trap(...)
        ...
}
```

SurfaceHostBuilder claims Plugins = HCons<LedgerCtx, HNil>, but the body mounts three things:

1. LedgerPlugin → contributes LedgerCtx
2. build_surface_plugin() → returns a builder with Ctx = SurfaceCtx and Deps = HCons<LedgerCtx, HNil>
3. build_plain_plugin() → returns a builder with Ctx = ()

After mount, Plugins should be at least HCons<SurfaceCtx, HCons<LedgerCtx, HNil>> (per #Unified-builder's type-level growth contract and Satisfies<Deps>). The return type says otherwise.

This example is the pinned reference for the "mount is a merge" contract. If the declared type doesn't match the body, either:

· .plugin() and .use() in the example don't actually grow Plugins — breaking Satisfies<Deps> checking — or
· SurfaceHostBuilder is wrong.

Neither matches the spec. The Rust compile would either fail with SurfaceHostBuilder mismatch, or .plugin(build_surface_plugin()) wouldn't type-check against Plugins: Satisfies<Deps> because LedgerCtx ∈ Deps requires LedgerCtx ∈ Plugins, which holds only if the mount grew the list.

Compare unified-builder-surface.ts:

```ts
export function buildSurfaceHost() {
  return kelir({...})
    .use(buildLedgerPlugin())
    .plugin(buildSurfacePlugin())
    .use(buildPlainPlugin())
    ...
}
```

Return type is inferred, no annotation to break. Same Py example. Only the Rust reference has the mismatch.

**Codebase changes needed:**
- `cans/artifacts/examples/unified-builder-surface.rs`: fix `SurfaceHostBuilder` to the actual post-mount type — `HCons<(), HCons<SurfaceCtx, HCons<LedgerCtx, HNil>>>` (and `Deps` = `HCons<LedgerCtx, HNil>` per `build_surface_plugin`).
- `crates/kelir/src/builder_sdk.flow.rs`: `plugin()` / `r#use()` already return `Self` (no `Plugins` growth); the spec's "type-level growth" is a `Satisfies<Deps>` check at the call site, not a return-type change. Verify `Plugins: Satisfies<PDeps>` accepts `HCons<SurfaceCtx, HCons<LedgerCtx, HNil>>` when `PDeps = HCons<LedgerCtx, HNil>` — the `Satisfies` impl in `hlist_sdk.flow.rs` needs an `HNil`-tail + `HCons`-tail case; ensure the impl that matches `HCons<Head, Tail> for HCons<Head, OtherTail>` is reachable for this list (it is, provided `LedgerCtx` is `Clone + Send + Sync`).
- `crates/kelir/src/hlist_sdk.flow.rs`: the `Satisfies` TODO notes the order-insensitive form is `E0207`-blocked; the witness-threaded arity-2 form compiles but callers can't name the witness. This example will only type-check if the mounted order matches the declared `Deps` order (`LedgerCtx` first) — pin that in the spec and add a `compile_error!` test in `crates/kelir/tests/`.

---

20. Broken spec anchor: #Ledger-handler-API

multi-vm-swarm.py:

```python
# TODO (L3): count supply-chain denials via session.cap.ledger once LedgerCap grows the by-kind counter per cans/plugins/ledger.md#Ledger-handler-API.
```

plugins/ledger.md contains these anchors: #Pure-reactive-observer, #Merkle-WAL, #OpenTelemetry, #Receipts-(ledger), #OTLP-defaults-(ledger), #Causal-delta-access-(ledger), #Canonical-accessor-(SSOT-A1-pin), #Causal-deltas-are-produced-by-plugin-ledger-over-the-delta-chain, #Ledger-handler-API.

Actually — re-reading the spec dump: plugins/ledger.md has:

· Ledger handler API
  · c.ledger.flush() forces WAL flush and OTel export

So #Ledger-handler-API exists as a node title but has only a single item under it (c.ledger.flush()). It does not define "by-kind counter" for supply-chain denials. The TODO expects a node that enumerates by-kind counters; the actual node only says flush(). So the TODO's target anchor resolves (cans check --refs-only will pass) but references semantics that aren't there.

Same pattern as item 7 (#ExecutionReceipt-records-final-binary-blake3), but subtler: this one resolves structurally and fails semantically. multi-vm-swarm.rs carries the identical TODO — "once LedgerCtx grows the by-kind counter per cans/plugins/ledger.md#Ledger-handler-API" — so both RS and Py point to a node that doesn't answer their question. multi-vm-swarm.ts doesn't have the TODO at all.

The gate --unified-builder-coverage counts the anchor as good; the actual semantic reference is dead.

**Codebase changes needed:**
- `cans/plugins/ledger.md#Ledger-handler-API`: expand the node from the single `flush()` bullet to a full method table — `flush()`, `count_denials(ppid)`, `blake3_of_producing_process(path)`, `finalize(FinalizeRequest)`, `get_causal_delta_from_parent()` — each with the per-SDK spelling (RS `count_denials`, TS `countDenials`, Py `count_denials`). All five exist in `crates/kelir-plugins/plugin-ledger/src/lib.rs`.
- `cans/artifacts/examples/multi-vm-swarm.py` / `.rs`: after the node is expanded, change the TODO to `session.cap.ledger.count_denials(pid)` (Py) / `ledger_ctx.count_denials(pid)` (RS) and drop the "once … grows" caveat.
- `cans/artifacts/examples/multi-vm-swarm.ts`: add the same `countDenials` call the Py/RS examples use, so all three demonstrate the capability.
- `crates/kelir-plugins/plugin-ledger/src/lib.rs` `LedgerCtx::count_denials`: already returns `Result<u64, PluginError>` — expose it on the session facade (`crates/kelir/src/session_lifecycle.flow.rs` `Session`) for parity with Py `session.cap.ledger.count_denials` / TS `ledgerHandle.countDenials`.

---

## Severity ranking for 1–20

# Severity Issue
2 High 5 undocumented APIs (c.step, c.terminal.notice, await_winner, handleIngress, blake3_of_producing_process, count_denials)
3 High Rust RepoPolicyPlugin drops the ledger prerequisite
1 High session.terminate() in 6 TS examples
4 Medium Spec note about c.cap.db no longer matches db-backed-runtime.py
6 Medium Ledger finalize demonstrated three different ways
8 Medium Console-output masking lacks carryover required by 04-terminal-plane.md
5 Medium EdbProxySchema non-POSIX errno
7 Medium Broken #ExecutionReceipt-records-final-binary-blake3 anchor + missing artifacts/code/
9 Low session.vm vs session.vm() ambiguity
10 Low String-trap-filter stand-ins
11 High snoops undocumented, present in Py/TS, absent in RS
14 High on_fs_write can't transform bytes in Py/TS but spec says it does
19 High unified-builder-surface.rs return type doesn't match body after mount
18 High RepoPolicyPlugin::fork_state will panic at fork under Tokio
12 Medium SuspendReason three different value shapes
15 Medium set_watchdog three different argument shapes
16 Medium on_trap_deny returns Result (RS) vs void (TS/Py) — different failure contracts
13 Medium FilterAction::Suspend/Abort erases pinned field names
17 Low default_vm_config is an undocumented RS-only entry helper
20 Low #Ledger-handler-API resolves but doesn't answer the referenced question

Items 11, 14, and 18 are the ones a cargo check/mypy/tsc run over examples would not necessarily catch but a live test would — the parity gate and the "canonical examples compile and run" gate both need to see these.