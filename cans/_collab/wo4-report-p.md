# WO4-P report — packages/ (py + ts SDK) items 2, 8, 9 (Agent P, 2026-11-11)

Branch `temp/p`, base 9c01a66 (scaffolding). Scope held strictly to `packages/` (py + ts, incl. tests). L1/L2 only; no L3 bodies filled. One new `TODO (L3)` comment added (TS veto/watchdog forwarding, pre-existing deferral now visible on the TS side). Commits: `52523ca` (P1), `5db4fad` (P2), `c86aca2` (P3), plus this report commit.

## Per-item outcome

### P1 — Item 2: canonical `repo_policy` cap key. DONE.

- `packages/py/kelir/service_caps.py`: `class PolicyCap` → `class RepoPolicyCap`; `Caps.__init__` now sets `self.repo_policy = RepoPolicyCap()`; `"repo-policy": "repo_policy"` added to `_CAP_KEY_ALIASES` (alias map + `materialize_cap` verified live: alias resolves the hyphen key onto the typed cap instead of planting an inaccessible `setattr(cap, "repo-policy", SimpleNamespace)` attribute — that was the actual bug behind item 2). `policy` kept as a read-only `@property` returning `self.repo_policy` (no setter; comment-law-compliant `# see:` instead of the work order's "document in class docstring" — docstrings are banned by AGENTS.md §3). `__all__` updated.
- `packages/py/kelir/__init__.py`: re-export + `__all__` entry renamed `PolicyCap` → `RepoPolicyCap` (deprecation class-alias **not** kept — plenger trait 1; the Caps-level `policy` property is the only alias, per work order).
- Call sites/tests: `test_ssot_a5.py` and `test_lifecycle_session.py` updated to canonical `cap.repo_policy`, plus alias assertions (`cap.policy is cap.repo_policy`) kept/desirable. `test_materialize_cap_aliases_db_proxy_and_keeps_methods` extended + renamed to `test_materialize_cap_aliases_hyphen_keys_and_keeps_methods` (now covers `repo-policy` materialization; no pinned test-name references exist outside packages/ — checked `cans/artifacts/test-plan/manifest.yaml`, matrix.yaml, scripts/).
- TS side verified, **no change needed**: `plugins.registry.ts` keys come from plugin names via `createPlugin<Name, Ctx>`, and capability structs are materialized verbatim from native names at `builder.lifecycle.ts:920` (`(session)[name] = ctx`). `c["repo-policy"]` works by construction.
- Everything else matching `policy` in packages/py is `policy-matrix` plugin names / `EPOLICY` errno / `policy_ns` telemetry — unrelated, untouched.

### P2 — Item 8: causal delta canonical home = ledger cap. DONE.

- Verified live: `packages/py/kelir/service_caps.py` `LedgerCap.get_causal_delta_from_parent()` already exists as the fail-closed L3 stub — kept as canonical home, unchanged. `Session.get_causal_delta_from_parent()` in `lifecycle_session.py` **removed** (no py callers existed anywhere in packages/ — grepped src + tests; `CausalDelta` import dropped with it). `test_ssot_a5.py::test_cap_skeleton_methods_raise_econfig_l3` extended to pin the canonical path (`cap.ledger.get_causal_delta_from_parent()` raises EConfig L3).
- `packages/ts/core/src/context.lifecycle.types.ts`: `getCausalDeltaFromParent(): Promise<CausalDelta>` added to `LedgerHandle`. `CausalDelta` located in `telemetry.attest.generated.ts` (re-exported via `types.core.js`, already imported in that file) — no new type invented; `NativeCausalDeltaRecord` is field-identical (camelCase), so the ledger cap typing matches the existing `finalize(includeDelta?: CausalDelta)` pattern.
- `packages/ts/core/src/session.lifecycle.ts`: `Session.getCausalDeltaFromParent()` removed; now-unused `CausalDelta`/`fromNativeCausalDelta` imports dropped. **Decision:** `NativeSessionHandle.getCausalDeltaFromParent(): Promise<NativeCausalDeltaRecord>` kept — it documents the live NAPI addon surface (the bindings' fail-closed stub stays per work order item 8; its removal would be a bindings change outside my scope). `fromNativeCausalDelta` in the generated file is now unused by src — generated SSOT projection, left alone.
- `packages/ts/core/tests/session.lifecycle.test.ts`: the session-delegation delta test rewritten to the canonical path — `fakeNativeSession` now provides `pluginContexts` with a `ledger` ctx; the test asserts the ledger cap carries the causal-delta accessor and maps all six fields. Narrowing done with `if (ledger === undefined) throw` (no non-null assertions in this codebase; `check()` is not an assertion function).

### P3 — Item 9: `ledger.flush()` reachability wiring (L2). DONE (no unreachable plumbing left as TODO on the SDK side).

- `packages/py/kelir/model_plugin.py`: `LifecycleCtx.__init__(self, cap: Caps | None = None)` stores `self.cap`; `set_watchdog`/`register_veto` untouched. `Caps` imported under `TYPE_CHECKING` (lazy annotations; zero cycle risk).
- `packages/py/kelir/plugin_fusion.py`: `FusedTrampoline.runs(cap: Caps | None = None)` — lifecycle-ring hooks now receive a fresh `LifecycleCtx(cap=cap)` per ring event instead of the `{"ring", "ordinal"}` dict (which never implemented `register_veto`/`set_watchdog` — the runtime crash behind item 9); all non-lifecycle rings keep the ring/ordinal payload. `LifecycleCtx` imported locally in `runs()` to avoid the model_plugin → plugin_fusion module cycle.
- `packages/py/kelir/kelir_builder.py` + `lifecycle_session.py`: the cap **is** reachable at L2 — boot creates `Caps()` *before* `native.fuse_hooks(descriptor, fused.runs(caps))` and injects the same instance via new `Session.from_native(handle, cap=None)` param, so `materialize_cap(session.cap, ...)` populates the very object the fused lifecycle closures hold. Post-boot lifecycle events (vm-shutdown, pre-commit, post-commit, trap-deny) see materialized caps; vm-init/vm-ready fire during `native.boot()` before materialization and see the unmaterialized skeleton (fail-closed L3 stubs) — that is the honest L2 ceiling, native population is L3. `fork()`/other `from_native` callers unchanged (cap defaults to None).
- `packages/ts/core/src/context.lifecycle.types.ts`: `LifecycleContext` extended with `readonly cap?: { readonly ledger?: LedgerHandle }` (per work order).
- `packages/ts/core/src/fusion.plugin.ts`: new `createLifecycleContext(cap?)` factory (records vetoes + watchdog in closure state, mirroring py `LifecycleCtx` fields) + `// TODO (L3) [plugin_engine]: forward the recorded vetoes and watchdog arming to the engine lifecycle dispatch.; slop: recorded veto/watchdog never enforced` — the only new TODO; it names the same deferral py already carries. `runs(cap?)` delivers `createLifecycleContext(cap)` to lifecycle-ring hooks, ring/ordinal payload to everything else. `import type` only for `LifecycleContext` (no runtime cycle with session.lifecycle).
- `packages/ts/core/src/builder.lifecycle.ts`: same reachability wired — a `cap` holder is created pre-`fuseHooks`, passed into `fused.runs(cap)`, and `cap.ledger = ctx as LedgerHandle` is populated from the native `pluginContexts` materialization loop post-boot (late-bound: the closures read it at invocation time).
- Sandbox rollback (items 5/6 consolidated): `packages/py/kelir/lifecycle_sandbox.py` `Sandbox.rollback(snapshot: SnapshotDescriptor | None = None)` → `self._session.rollback(snapshot)`; `packages/ts/core/src/sandbox.runtime.ts` `rollback(snapshot?: SnapshotDescriptor)` → `this.booted.rollback(snapshot)`. **Verified already done:** `Session.rollback(snapshot=None)` (py) and `Session.rollback(snapshot?)` (ts) both already accept the optional snapshot and preserve the no-arg native-L3-error path — no change needed there.
- Tests added: py `test_runs_delivers_lifecycle_ctx_with_cap_to_lifecycle_hooks` (ctx type + cap identity + bare-runs cap None), `test_session_from_native_reuses_injected_caps`, `test_sandbox_rollback_routes_snapshot_to_session_rollback` (stub-session pattern); ts new `tests/fusion.trampoline.test.ts` (non-lifecycle payload unchanged; lifecycle ctx verbs + cap identity + bare cap undefined) and `sandbox.runtime.test.ts` rollback forwarding case.

## Decisions (recorded)

1. No `PolicyCap = RepoPolicyCap` class alias (backward-compat slave); the read-only `Caps.policy` property is the sole alias surface, as instructed.
2. `NativeSessionHandle.getCausalDeltaFromParent` (TS interface member) kept — it mirrors the still-live NAPI stub; only the SDK-level `Session` delegation was removed.
3. Fresh `LifecycleCtx`/`LifecycleContext` per ring-event invocation (per-event ctx semantics, matching how the engine dispatches); caps are the shared, long-lived part.
4. py shares one pre-boot `Caps` instance between fused closures and `Session` (via `from_native(cap=...)`); TS shares a late-bound `{ ledger? }` holder populated from native `pluginContexts`. Both are the maximal wiring reachable at L2 without touching native/binding contracts.
5. `uv.lock` churn from `uv sync --extra dev` (pre-existing staleness: `tomli-w` missing from the committed lock) reverted every time — not part of any work item; flagging here so the owner can refresh the lock deliberately.

## Cross-agent handoffs (must land before the final gate)

- `cans/artifacts/examples/autonomous-swe-agent.ts:215` (`child.getCausalDeltaFromParent()`) and `db-backed-runtime.ts:118` (`session.getCausalDeltaFromParent()`) still call the removed Session method. `tsconfig.check.json` includes `cans/artifacts/examples/*.ts`, so `tsc -p tsconfig.check.json` fails until the artifacts agent rewrites them to `session.cap.ledger...`-style / plugin-ctx paths (work order item 8 already assigns this). `autonomous-swe-agent.py:491` (`child.get_causal_delta_from_parent()`) is runtime-only breakage (no py test executes that main path — suite stays green) and needs the same rewrite; `c.cap.policy` in the py examples keeps working through the alias property.
- TS tests were **not** executed (no pnpm/tsc/dist in fast mode): every touched `.ts` file was amaro-parse-checked (all OK; `fusion.plugin.ts` has a pre-existing strip-only limitation from its `private constructor(readonly chains: ...)` parameter property — present at base, unrelated) and the full changed hunks were hand-verified against imports/interfaces. `tests/fusion.trampoline.test.ts` + the sandbox/session test additions need `pnpm build && pnpm test` at convergence.

## Gate results

| Check | Result |
|---|---|
| `python3 -m compileall packages/py` | PASS |
| `uv run python -m pytest packages/py/tests` (repo-root invocation, red/ excluded per addopts) | PASS 148 passed / 5 skipped (145→148: three new tests; 3 `tests/red/*` failures are pre-existing at base, verified by stash-run on clean tree) |
| `mypy` strict on `kelir` + `tests` (venv from `uv sync --extra dev`) | PASS, 45 files, no issues |
| Comment-law greps (py + ts, AGENTS.md §3) | both print nothing |
| TS parse check (amaro strip) on all 8 touched files | PASS (fusion.plugin.ts: pre-existing parameter-property limitation, unchanged from base) |
| Manual TS diff review vs imports/interfaces | done, per file |

git log scaffolding..temp/p: `52523ca` (P1), `5db4fad` (P2), `c86aca2` (P3), report commit on top. Pushed to `origin/temp/p`, no force.
