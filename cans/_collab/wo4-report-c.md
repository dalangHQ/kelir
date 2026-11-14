# WO4-C report — Python + TypeScript SDK lane (Agent C, packages/ only)

Branch `temp/wo4-c`, base 9c01a66 (= origin/scaffolding). Tasks C1–C4 (work-order items 2, 5/6, 8) done. Phase L1/L2 respected: no L3 bodies filled, no new `TODO (L3)` markers, no compat aliases (anti-plenger #1: `PolicyCap` deleted outright, zero shims).

## Per-task outcomes

- **C1 (item 2) `PolicyCap` → `RepoPolicyCap` — done, clean break.** `packages/py/kelir/service_caps.py`: class renamed, `Caps.repo_policy = RepoPolicyCap()`, `_CAP_KEY_ALIASES` now `{"db-proxy": "db_proxy", "repo-policy": "repo_policy"}` (verified consumption: `materialize_cap` does `_CAP_KEY_ALIASES.get(plugin_name, plugin_name)` then `getattr`/`setattr`, so plugin `repo-policy` lands on `cap.repo_policy` — functionally verified: `materialize_cap(cap, "repo-policy", {...})` populates `cap.repo_policy.violations`/`current_branch`). `__all__` in both files updated (alphabetical position preserved/adjusted). No `PolicyCap = RepoPolicyCap` alias, no property shim. TS side untouched (already generates `c["repo-policy"]` from the plugin name; no `repo-policy`/`PolicyCap` references existed in packages/ts).
- **C2 (items 5/6) `Sandbox.rollback(snapshot)` — done, both SDKs.** Py `lifecycle_sandbox.py`: `async def rollback(self, snapshot: SnapshotDescriptor | None = None)` → `self._session.rollback(snapshot)`; `SnapshotDescriptor` added to the existing `TYPE_CHECKING` import (matches file style; `from __future__ import annotations` present). TS `sandbox.runtime.ts`: `async rollback(snapshot?: SnapshotDescriptor): Promise<void>` → `this.booted.rollback(snapshot)`; type import extended on the existing `./types.core.js` line. TS `Session.rollback(snapshot?)` already had the optional-snapshot form (verified in session.lifecycle.ts:268 — snapshot → `vm.rollback(snapshot)`, no-snapshot → `native.rollback()`); no change needed there. Py `Session.rollback` signature verified: `snapshot: SnapshotDescriptor | None = None`.
- **C3 (item 8) causal-delta canonical home = ledger capability — done.** TS `context.lifecycle.types.ts`: `getCausalDeltaFromParent(): Promise<CausalDelta>;` added to `LedgerHandle` (`CausalDelta` was already imported from `./types.core.js`; re-exported to consumers via `export *` in index.ts). **Implementer sweep: zero object-literal implementers of `LedgerHandle` exist in packages/ts** (greps for `LedgerHandle`, `countDenials`, `blake3OfProducingProcess`, `merkleRoot`, `ledger: {` — only the interface itself, generic type params in plugins.registry.ts, and type-only usages in accumulation.monad.test.ts). No implementer stub was therefore required; the native-missing typed-error style lives in the Rust bindings (Agent A/B lane) and in the Py `LedgerCap` stub, which stays as-is. Py `lifecycle_session.py`: `Session.get_causal_delta_from_parent()` now a pure one-line delegation to `self.cap.ledger.get_causal_delta_from_parent()`; the session-level `TODO (L3)` + direct raise deleted (the canonical TODO/error stays on `LedgerCap`, per D7 — no duplicated error surface). Verified: `Session().get_causal_delta_from_parent()` raises `EConfig("... ledger get_causal_delta_from_parent (L3)")` from the LedgerCap stub.
- **C4 coherence check — verified, no code change.** Py `Session.rollback`: snapshot path delegates to `self.vm.rollback(snapshot)` (typed error "vm rollback (L3)" without native), no-snapshot path raises the existing typed error ("session rollback (L3)" — head-of-lineage form lands with PID tracking in L3 per D5). `VmHandle.rollback` and all bindings paths untouched.

## Rename fallout list (every file touched for C1)

1. `packages/py/kelir/service_caps.py` — class rename, `__all__`, `Caps.__init__`, `_CAP_KEY_ALIASES`
2. `packages/py/kelir/__init__.py` — re-export (`RepoPolicyCap as RepoPolicyCap`), `__all__` (moved to sorted position between `ReceiptExtension` and `ResourcePressure`)
3. `packages/py/tests/test_lifecycle_session.py` — `session.cap.repo_policy.violations`
4. `packages/py/tests/test_ssot_a5.py` — `cap.repo_policy.violations`, `HandlerContext().cap.repo_policy.violations`

Grep-verified: zero remaining `PolicyCap` / `cap.policy` references under packages/ (the `.policy` hits in matrix.hook.test.ts / hook-matrix examples are the literal guest path `/workspace/.policy/identity` — unrelated).

## Signature changes (canonical surfaces for other agents)

- Py: `Caps.repo_policy: RepoPolicyCap` (was `Caps.policy: PolicyCap`); `kelir.RepoPolicyCap` re-exported; `materialize_cap` accepts plugin name `"repo-policy"`.
- Py: `Sandbox.rollback(snapshot: SnapshotDescriptor | None = None) -> None`.
- Py: `Session.get_causal_delta_from_parent() -> CausalDelta` delegates to `self.cap.ledger.get_causal_delta_from_parent()` (canonical home).
- TS: `Sandbox.rollback(snapshot?: SnapshotDescriptor): Promise<void>`.
- TS: `LedgerHandle.getCausalDeltaFromParent(): Promise<CausalDelta>` (new interface member).

## Gates (all run unless noted)

- Comment law (AGENTS.md §3 greps, full `packages/ts` + `packages/py`): **clean, prints nothing**; every edit is a `// see:`-free signature/delegation change, no prose comments added, no docstrings.
- `python3 -m py_compile` on all 6 touched .py files: **PASS**.
- SDK import smoke: `import kelir` → **"import ok"** (note: `tomli_w` was missing in this sandbox, pre-existing at base 9c01a66 — verified via stash — installed locally to run the gate; `pydantic` was present).
- pytest (importable): full `packages/py/tests` incl. red/: **146 passed, 4 skipped, 3 failed** — byte-identical to the stashed baseline (same 3 intentionally-red bindings tests: test_bp_01/02/03_red). **Zero new failures.** Touched subset (test_lifecycle_session.py, test_ssot_a5.py, test_plugin_fluent_builder.py via full run) all green.
- `scripts/check-canon.sh`: **"OK: Corpus is 100% Day-1 canonical."** (no compat-alias language introduced).
- tsc / pnpm: **unavailable in sandbox — deferred to CI.** TS edits are minimal signature mirrors of existing style in the same files; no structural implementers affected (verified by grep).
- `git diff --stat 9c01a66..HEAD`:

```
 packages/py/kelir/__init__.py                   |  4 ++--
 packages/py/kelir/lifecycle_sandbox.py          | 13 ++++++++++---
 packages/py/kelir/lifecycle_session.py          |  3 +--
 packages/py/kelir/service_caps.py               | 11 +++++++----
 packages/py/tests/test_lifecycle_session.py     |  2 +-
 packages/py/tests/test_ssot_a5.py               |  4 ++--
 packages/ts/core/src/context.lifecycle.types.ts |  1 +
 packages/ts/core/src/sandbox.runtime.ts         |  6 +++---
 8 files changed, 27 insertions(+), 17 deletions(-)
```

## TODO (L3) count

Before: 76 in packages/. After: 75. Delta −1 (deleted the session-level causal-delta TODO now covered by the canonical `LedgerCap` stub; TODO count never rose).

## Push confirmations

- c7d586e — C1 (pushed; branch created on remote)
- e72c1d9 — C2 (first push attempt rejected transiently; `git pull --rebase` showed up-to-date, retry succeeded — no force-push used)
- c34410c — C3
- report commit — this file

## For the orchestrator / other lanes

1. **Agent D (examples):** `cans/artifacts/examples/autonomous-swe-agent.py:259` still reads `c.cap.policy.violations` — must become `c.cap.repo_policy.violations` (work-order item 2 already assigns this). Py SDK surface is `session.cap.repo_policy` / `kelir.RepoPolicyCap`. TS examples already correct (`c["repo-policy"].policy...` is the TS plugin-context shape, unchanged).
2. **Agent B (spec):** `cans/03-sdk-architecture.md` SSOT-A1 key list still says `policy` → should say `repo-policy` (item 2). Rollback surface rows (items 5/6) should now read: Py `session.vm.rollback(snapshot)` low-level + `session.rollback(snapshot)` / `sandbox.rollback(snapshot)` convenience; TS identical (`vm.rollback(snapshot)`, `session.rollback(snapshot?)`, `sandbox.rollback(snapshot?)`). No-arg head-of-lineage rollback stays a typed L3 error on both SDKs (bindings own the message).
3. **Agent A/B (Rust/bindings):** bindings' session-handle `rollback()` no-arg stubs and `get_causal_delta_from_parent` stubs are intentionally untouched — the Py `Session.get_causal_delta_from_parent()` no longer calls the native session handle at all (it routes through `cap.ledger`), so the bindings' session-handle causal-delta stub is now dead surface for the Py path; Rust-side cleanup of that binding stub is their call (work-order item 8 lists it).
4. **tsc CI:** the only TS type-surface additions are `Sandbox.rollback(snapshot?)` (call-compatible with the old no-arg form) and the `LedgerHandle.getCausalDeltaFromParent` member (no implementers in-tree). No compile breakage expected.
