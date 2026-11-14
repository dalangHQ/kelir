# WO4-E Report — Artifact examples alignment (cans/artifacts/examples/)

Agent E, branch `temp/e`, base `scaffolding @ 9c01a66`. Exclusive scope: `cans/artifacts/examples/**` (this report file excepted, per task instruction). Every claimed call-site was verified against the live tree before editing; the work order's tables were accurate for every named call-site (none stale in the "already fixed" sense — see per-item notes for already-canonical sites).

## E1 — Item 2: capability key = plugin name (`repo-policy`)

- `autonomous-swe-agent.py:259` — `c.cap.policy.violations` → `c.cap.repo_policy.violations`.
- Verified against live tree: `packages/py/kelir/service_caps.py` still has `Caps.policy = PolicyCap()` and `_CAP_KEY_ALIASES = {"db-proxy": "db_proxy"}` — the py SDK rename to `repo_policy` (+ `"repo-policy": "repo_policy"` alias) is the item-2 codebase change owned elsewhere; the example now teaches the canonical key. The TS example (`c["repo-policy"].policy.violations`) was already correct — no change (as the work order states).

## E2 — Items 3+10: one-shot checkpoint surface `checkpoint.create(label)`

- `autonomous-swe-agent.py:511` — `await session.vm.checkpoint("clean_base")` → `await session.checkpoint.create("clean_base")`.
- `autonomous-swe-agent.ts:197` — `await session.vm.checkpoint("clean_base")` → `await session.checkpoint.create("clean_base")`.
- `policy-matrix.py:91` — `await c.vm.checkpoint("policy-yield")` → `await c.checkpoint.create("policy-yield")` (handler context form blessed by work-order item 3's `(or c.checkpoint.create("policy-yield"))`).
- `policy-matrix.ts:32` — same change.
- **Decision (consistency extension):** the trap-handler pre-step checkpoints in the same two files were the same surface on the same context type and were aligned the same way: `autonomous-swe-agent.py:190` and `autonomous-swe-agent.ts:108` `c.vm.checkpoint(f"pre:{c.trap.pid}")` → `c.checkpoint.create(...)`. Leaving them would teach two checkpoint surfaces on `HandlerContext`/`TrapContext` in one file family. Codebase follow-up implied: py `HandlerContext` / TS `TrapContext` gain a `checkpoint` handle (live tree today exposes only `vm.checkpoint` on those contexts; `Session.checkpoint.create` exists in both SDKs).
- Rust examples' `create_step`/`commit_step`/`lineage(1)` left unchanged (sanctioned Rust low-level surface), including `policy-matrix.rs` / `autonomous-swe-agent.rs` handler-ctx `ctx.vm.checkpoint(...)` — `VmHandle::checkpoint` is the sanctioned Rust handler-context surface (Rust has no one-shot `create` at session level).

## E3 — Item 4: `session.fork()`

- `autonomous-swe-agent.py:481` — `await parent.vm.fork()` → `await parent.fork()`.
- `autonomous-swe-agent.ts:210` — `await session.vm.fork()` → `await session.fork()`.
- `multi-vm-swarm.ts:64` — `await session.vm.fork()` → `await session.fork()`.
- **Already canonical, no change:** `multi-vm-swarm.py:100` (`await session.fork()`), `autonomous-swe-agent.rs:625`, `multi-vm-swarm.rs:166` (`session.fork().await?`). Verified live: py `Session.fork()` exists (`packages/py/kelir/lifecycle_session.py`), `VmHandle` carries no fork of its own (py `VmHandle.fork` delegates to `Session.fork`); the live TS `Session` class does not yet expose `fork()` (only `vm.fork`) — the example now teaches the canonical surface the work order decides (napi `NativeSessionHandle::fork` exists).

## E4 — Item 8: causal delta lives on the ledger

- `autonomous-swe-agent.py:491` — `await child.get_causal_delta_from_parent()` → `await child.cap.ledger.get_causal_delta_from_parent()`.
- `autonomous-swe-agent.ts:215` — `await child.getCausalDeltaFromParent()` → `await child.cap.ledger.getCausalDeltaFromParent()`.
- `db-backed-runtime.ts:118` — `await session.getCausalDeltaFromParent()` → `await session.cap.ledger.getCausalDeltaFromParent()`.
- **Already canonical, no change:** `db-backed-runtime.py:155` (`await ledger.get_causal_delta_from_parent()` on the ledger handle captured from `session.cap.ledger`).
- `db-backed-runtime.rs:180` — TODO re-pointed: "the session facade does not carry get_causal_delta_from_parent — plugin-ledger's LedgerCtx is the canonical home, so the request carries a zero delta." (Verified: the Rust session facade has no such method; only napi/pyo3 session-handle stubs erroring with "causal deltas come from plugin-ledger (L3)", and py `LedgerCap.get_causal_delta_from_parent` is the canonical home. `plugin-ledger`'s `LedgerCtx` currently has `finalize`/`flush`/`count_denials`/`blake3_of_producing_process` — adding `get_causal_delta_from_parent` is the item-8 codebase follow-up.)
- **Decision (consistency extension):** session-level ledger access in the TS examples was unified on the canonical `cap.ledger` path named by work-order item 8 ("the canonical path should be `session.cap.ledger.getCausalDeltaFromParent()`"), matching the py twins (`session.cap.ledger.count_denials` / `.finalize`):
  - `autonomous-swe-agent.ts:184` — `s.ledger.countDenials(e.ppid)` → `s.cap.ledger.countDenials(e.ppid)`.
  - `autonomous-swe-agent.ts:239` — `session.ledger.finalize(...)` → `session.cap.ledger.finalize(...)`.
  - `db-backed-runtime.ts:116` — `session.ledger.finalize(...)` → `session.cap.ledger.finalize(...)`.
  - Without this, db-backed-runtime.ts would teach `session.cap.ledger.getX()` and `session.ledger.finalize()` two lines apart. Trap-context `c.ledger.*` (plugin ctx type `{ ledger: LedgerHandle }`) left as-is — that is the TS plugin-ctx surface.

## E5 — Item 9: `ledger.flush()` reachability (capture-at-registration)

- `db-backed-runtime.py` — code fix mirroring `db-backed-runtime.rs`: `LedgerPlugin` construction hoisted to builder scope, `ledger = ledger_plugin.context()` captured at builder time, `_pre_commit(_ctx)` calls `await ledger.flush()`, `.use(ledger_plugin)` in the chain. `_post_commit(session: Session, committed)` → `_post_commit(_ctx, committed)` (same defect — the fused runner passes no `Session`); now-unused `Session` import dropped.
  - **Decision:** the py example teaches `LedgerPlugin.context()` as the builder-time ledger-handle capture, mirroring the Rust example's `ledger_plugin.context()` (Rust `KelirPlugin::context() -> LedgerCtx` at `crates/kelir-plugins/plugin-ledger/src/lib.rs:250`). The py `LedgerPlugin` does not yet expose `.context()` (its builder-mode `.context(fn)` registrar is a different, plugin-fragment-only surface) — exposing it is the py SDK follow-up. Artifacts own code signatures, so the example pins the intended surface.
- `db-backed-runtime.ts:80` — TODO updated to the decided wording: "capture the ledger handle at builder time (as in db-backed-runtime.rs); LifecycleContext carries cap.ledger once the fused lifecycle payload lands (L3)." **Comment-only** — the TS `ledger()` factory returns an anonymous builder fragment with no handle accessor (verified `builder.lifecycle.ts`; no `.context()` getter exists), so no small, obvious code change exists.
- `autonomous-swe-agent.py:303` / `.ts:180` — dropped-flush TODOs updated to name the capture-at-registration pattern (same wording idea).
- **Decision (consistency extension):** `serverless-webhook.{py,ts,rs}` carry the byte-identical dropped-flush TODO ("...ledger.flush dropped; live .on delivers LifecycleContext only...") — all three updated with the same capture-pattern wording so the artifact set stays internally consistent. Comment-only in all three.

## E6 — Rescan of hook-matrix*, container, db-backed-runtime + suspend/resume verification

Rescanned every example variant for `vm.checkpoint`, `vm.fork`, `cap.policy`, session-level causal delta, and session-level direct ledger access:

- `hook-matrix.py/.ts`, `hook-matrix-full.py/.ts`, `container.py/.ts` — **zero occurrences** of any changed surface (their only `suspend(...)` hits are `FilterAction.suspend(until=...)` verdict helpers, not session suspend). No change needed.
- Remaining `vm.checkpoint` occurrences are exactly the two sanctioned Rust handler-ctx sites (`policy-matrix.rs:195`, `autonomous-swe-agent.rs:416`) — intentionally unchanged.
- Remaining `vm.suspend(path)` sites (`autonomous-swe-agent.py:310`, `.ts:185`, break-glass quarantine to a named snapshot path) are the intentional low-level `VmHandle` surface per work-order item 1 ("Do not change `VmHandle`") — unchanged.
- `serverless-webhook.py:132` / `.ts:88` verified canonical: `session.suspend(External(ExternalSuspendKind.HUMAN_AUTHORIZATION_HOLD))` / `session.suspend(SuspendReason.external("human-authorization-hold"))`, reason-based, no path; no `resume()` call exists in that example (suspend-only wake-cycle pattern). No change, as expected — item 1 is right that the artifacts were already canonical here.
- All py/ts `session.suspend(reason)` / `session.resume()` call-sites across `autonomous-swe-agent.*`, `policy-matrix.*` verified canonical.
- Rust `session.suspend(SuspendReason::...)` / `session.resume()` verified canonical in `policy-matrix.rs`, `autonomous-swe-agent.rs`, `serverless-webhook.rs`.

## Checks run (fast mode)

- `python3 -m compileall` on every touched `.py` (`autonomous-swe-agent.py`, `policy-matrix.py`, `db-backed-runtime.py`, `serverless-webhook.py`) — clean after each item.
- `.ts`/`.rs` hunks re-read manually via `git diff scaffolding..temp/e` (no compilers available) — verified hunk-by-hunk.
- `bash scripts/check-examples-canonical.sh` — `OK: examples are canonical in cans/artifacts/examples` (completes in <1s, no installs).

## Files changed (10)

`autonomous-swe-agent.py`, `autonomous-swe-agent.ts`, `policy-matrix.py`, `policy-matrix.ts`, `multi-vm-swarm.ts`, `db-backed-runtime.py`, `db-backed-runtime.ts`, `db-backed-runtime.rs`, `serverless-webhook.py`, `serverless-webhook.ts`, `serverless-webhook.rs` (11 files; `multi-vm-swarm.py` verified, unchanged).

## Push status

E1–E5 each committed and pushed to `origin/temp/e` as they landed (see `git log scaffolding..temp/e`). E6 was verification-only (no additional occurrences found; nothing to commit).
