# WO4-D report — examples artifact lane (Agent D, 2026-11-11)

Branch `temp/wo4-d`, base 9c01a66 (origin/scaffolding). Scope held strictly to
`cans/artifacts/examples/` (+ this report). No Rust example touched. Phase L1/L2
respected: one new `TODO (L3)` line added (D5, db-backed-runtime.py), no L3 bodies filled.

## Per-task outcomes

- **D1 (item 2, capability key) — done.** `autonomous-swe-agent.py`
  `make_commit_gate`: `c.cap.policy.violations` → `c.cap.repo_policy.violations`.
  Grepped the whole examples dir: exactly one `cap.policy` occurrence existed; zero remain.
  TS side untouched — `c["repo-policy"].policy.violations` (autonomous-swe-agent.ts:165)
  was already keyed by plugin name.
- **D2 (items 3/10, one-shot checkpoint surface) — done.** Four call sites moved off the
  undocumented `vm.checkpoint` one-shot path:
  `autonomous-swe-agent.py:511` and `autonomous-swe-agent.ts:197`
  `session.vm.checkpoint("clean_base")` → `session.checkpoint.create("clean_base")`;
  `policy-matrix.py:91` and `policy-matrix.ts:32`
  `c.vm.checkpoint("policy-yield")` → `c.checkpoint.create("policy-yield")`.
  Verified already-canonical (no change): git-gate.py:75 / git-gate.ts:58,
  replay.py:44 / replay.ts:35, multi-vm-swarm.py:88,97 / multi-vm-swarm.ts:48,59.
  Deliberately untouched: the trap-handler pre-exec checkpoints
  `c.vm.checkpoint(f"pre:{c.trap.pid}")` (autonomous-swe-agent.py:190 / .ts:108) —
  that is the documented low-level Py/TS `VmHandle::checkpoint` surface, not the
  one-shot path the work order flags.
- **D3 (item 4, fork surface) — done.** `autonomous-swe-agent.py:481`
  `await parent.vm.fork()` → `await parent.fork()`; `autonomous-swe-agent.ts:210` and
  `multi-vm-swarm.ts:64` `await session.vm.fork()` → `await session.fork()`.
  Verified already-canonical (no change): multi-vm-swarm.py:100, autonomous-swe-agent.rs:625,
  multi-vm-swarm.rs:166. Zero `vm.fork()` remain in py/ts examples.
- **D4 (item 8, causal delta home = ledger cap) — done.** `autonomous-swe-agent.py:491`
  `await child.get_causal_delta_from_parent()` → `await child.cap.ledger.get_causal_delta_from_parent()`;
  `autonomous-swe-agent.ts:215` → `await child.cap.ledger.getCausalDeltaFromParent()`;
  `db-backed-runtime.ts:118` → `await session.cap.ledger.getCausalDeltaFromParent()`.
  Verified already-on-ledger (no change): db-backed-runtime.py:147-152
  (`ledger = session.cap.ledger` then `await ledger.get_causal_delta_from_parent()`).
  db-backed-runtime.rs:180 TODO untouched (Rust lane keeps its TODO pointing at the session-facade gap).
- **D5 (item 9, ledger.flush capture-at-registration) — done, honest-fallback branch.**
  See "D5 capture-pattern outcome" below.
- **D6 — PASS.** `bash scripts/check-examples-canonical.sh` →
  "OK: examples are canonical in cans/artifacts/examples" (file presence + no scattered
  examples dirs; content is not checked by this gate).
- **D7 (item 1, suspend/resume verify) — CONFIRMED canonical, no changes.** Spot-checked
  live files: serverless-webhook.py:132 `session.suspend(External(ExternalSuspendKind.HUMAN_AUTHORIZATION_HOLD))`;
  serverless-webhook.ts:88 `session.suspend(SuspendReason.external("human-authorization-hold"))`;
  policy-matrix.py:174-175 `session.suspend(ResourcePressure())` / `session.resume()`;
  policy-matrix.ts:112-113 `session.suspend(SuspendReason.resourcePressure)` / `session.resume()`.
  Bonus confirmation: autonomous-swe-agent.py:543,546 / .ts:224,226 also reason-based.
  Every py/ts example uses the canonical `suspend(reason)` / `resume()` form; the spec's
  "non-conforming sides" block is stale (Agent B's lane to delete).

## D5 capture-pattern outcome (db-backed-runtime.py)

The Rust pattern (db-backed-runtime.rs:65 `let ledger_ctx = ledger_plugin.context();`
captured into the `on_pre_commit` closure at :121-127) requires a **pre-boot ledger
handle**. Verified against the live Py SDK: `packages/py/kelir/plugins/plugin_ledger.py`
`LedgerPlugin` is a bare `BasePlugin[None, None]` holding only a config — no
`context()`/handle accessor; `Caps`/`LedgerCap` materialize only inside
`KelirBuilder.boot()` (kelir_builder.py:909 `materialize_cap(session.cap, ...)`).
So the Python SDK offers **no pre-boot ledger handle to capture**, and capturing a
locally-instantiated `LedgerCap()` would be a dead stub even after the bridge lands
(band-aid, not the live handle). Per the agreed fallback:

- `_pre_commit(session: Session)` (whose body called `session.cap.ledger.flush()` —
  would AttributeError at runtime: the fused runner passes a payload/ctx, never a
  Session; plugin_fusion.py:117-128 `hook.run({"ring": ..., "ordinal": ...})`) became
  `async def _pre_commit(_ctx) -> None: return None` with a one-line
  `# TODO (L3): flush the ledger here; capture a pre-boot ledger handle at registration
  as db-backed-runtime.rs does via LedgerPlugin.context(), but the Py LedgerPlugin
  exposes no such handle yet, so the flush is dropped.`
- **Judgment call (reported after the fact, per AGENTS §0):** `_post_commit(session: Session, committed: bool)`
  in the same file also claimed a Session parameter. D8's canonical rule says lifecycle
  handler signatures must not claim a Session parameter, and the Rust sibling's
  `on_post_commit(|_ctx, committed| ...)` takes `_ctx`. Renamed to
  `_post_commit(_ctx, committed: bool)` (body unchanged — prints only). `Session`
  removed from the now-unused import line.
- db-backed-runtime.ts TODO reworded from "not in reach at registration time" to the
  capture-at-registration guidance pointing at the .rs pattern
  (`LedgerPlugin.context()`), with the `LifecycleContext.cap.ledger` future path.
- autonomous-swe-agent.py:303 / .ts:180 shutdown-flush TODOs realigned to the same
  capture-pattern guidance (one line each, existing comment style, `TODO (L3)` level tag).

## Verification results

- `python3 -m py_compile` on every touched .py — PASS.
- Python import check against the live SDK, harness-identical to scripts/check-examples.sh
  py leg (module registered in `sys.modules` before `exec_module`): **all** examples in
  cans/artifacts/examples/*.py compile AND import — PASS, including the three touched
  .py files. (Note: naive one-liner exec_module without sys.modules registration trips
  a dataclasses `KW_ONLY` introspection AttributeError — harness artifact, not a file defect.)
- `bash scripts/check-examples-canonical.sh` — PASS (see D6).
- Grep audit: zero `cap.policy`, zero py/ts `vm.checkpoint(`/`vm.fork(`/session-level
  `get_causal_delta_from_parent`/`getCausalDeltaFromParent` remain; Rust examples retain
  `create_step`/`commit_step`/`lineage(1)`/`ctx.vm.checkpoint` as ordered.

### Merge-time cross-checks (expected red in MY tree, green after Agent C lands)

- `c.cap.repo_policy` — my tree's `Caps` still has `policy` (Agent C renames
  `PolicyCap`→`RepoPolicyCap`, `Caps.repo_policy`, `_CAP_KEY_ALIASES` entry). Attribute
  name matches the agreed contract; runtime resolution is inside a function body so the
  import gate passes either way; the live-SDK cross-check lands at merge.
- `c.checkpoint.create` in policy-matrix.py/.ts — the live Py `HandlerContext` and TS
  `TrapContext` expose `vm` but not `checkpoint` yet; per contract D3 the one-shot
  surface on handler contexts is canonical and the SDK side is Agent C's lane.
- `session.cap.ledger.getCausalDeltaFromParent()` in the two .ts examples — TS
  `LedgerHandle` gains the method in Agent C's tree; TS legs of check-examples.sh are
  "deferred to CI" (no cargo/pnpm in this sandbox).

## Gates run / skipped

| Gate | Result |
|---|---|
| py_compile (touched .py) | PASS |
| py import check vs live SDK (all examples) | PASS |
| scripts/check-examples-canonical.sh | PASS |
| Rust leg of check-examples.sh | skipped — no cargo in sandbox, deferred to CI (no .rs touched anyway) |
| TS leg of check-examples.sh | skipped — no pnpm in sandbox, deferred to CI |

## Files changed (9c01a66..HEAD, 3 pushes: ee1f156 → 54f37f4 → 79b64c5)

```
cans/artifacts/examples/autonomous-swe-agent.py | 10 +++++-----  (D1 D2 D3 D4 D5-comment)
cans/artifacts/examples/autonomous-swe-agent.ts |  8 ++++----   (D2 D3 D4 D5-comment)
cans/artifacts/examples/db-backed-runtime.py    |  9 +++++----   (D5 handler rewrite + import prune)
cans/artifacts/examples/db-backed-runtime.ts    |  4 ++--      (D4 D5-comment)
cans/artifacts/examples/multi-vm-swarm.ts        |  2 +-        (D3)
cans/artifacts/examples/policy-matrix.py        |  2 +-        (D2)
cans/artifacts/examples/policy-matrix.ts        |  2 +-        (D2)
cans/_collab/wo4-report-d.md                    |  (this file)
```

## Observations for the orchestrator (untouched, out of lane)

1. `policy-matrix.rs:195` uses `ctx.vm.checkpoint("policy-yield")` while every other
   Rust example uses the `create_step`/`commit_step` pair — Rust examples were declared
   no-change, so left as-is; may deserve a Rust-lane look.
2. `serverless-webhook.py:130` / `serverless-webhook.ts:74` still carry the older
   "SSOT .on(VmReady)/.on(VmShutdown) ledger.flush dropped; live .on delivers
   LifecycleContext only" TODO wording — not listed in item 9's change set nor in my
   task list, so left untouched; trivial one-line alignment if wanted later.
3. `db-backed-runtime.ts:116` reads `session.ledger.finalize(...)` (plugin-context-by-name
   materialization) while the py twin uses `session.cap.ledger.finalize(...)` — D7/D4
   contract only moved the causal-delta accessor; flagging the finalize-access asymmetry
   for the TS SDK owner (Agent C) in case `cap` grouping is meant to become canonical there.
