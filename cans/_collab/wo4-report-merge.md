# wo4 merge report (merge-lead)

Base: 9c01a66 (scaffolding). Lanes: temp/r (crates), temp/s (cans nodes), temp/e (artifact examples), temp/p (packages). Converged by four sequential `git merge --squash` into one commit on the final branch; lane reports: wo4-report-{r,s,e,p}.md; cross-lane decisions in decisions.md (2026-11-14 WO4 entry).

## Ripple fixes applied at convergence (not visible in any lane branch)

- TS/Py SDK suspend/resume re-pointed to the reason-based native session-handle signature from lane R (TS `native.suspend(reason)` / `native.resume()`; Py `native.suspend({"kind": ..., "detail": ...})` / `native.resume()`); path derivation now owned by the binding, matching cans/08.
- TS examples' session-level `*.cap.ledger.*` calls normalized to `c.<pluginKey>` (`session.ledger.*`) — `session.cap` does not exist in TS; `cap.ledger` there is the LifecycleContext hook field only.
- Trap/handler-ctx checkpoint calls reverted to the low-level `c.vm.checkpoint(label)` (autonomous-swe-agent.{py,ts}, policy-matrix.{py,ts}): work-order item 3's `c.checkpoint.create` prescription misread the ctx type; session-level `checkpoint.create` kept.
- TS `Session.fork()` added (delegates to the VmHandle projection, re-attaches the child's pluginContexts via the new shared `attachPluginContexts` helper also used by boot); autonomous-swe-agent.ts child cast `as typeof session` for the typed plugin caps.
- db-backed-runtime.py: `LedgerPlugin.context()` (never existed) removed; `_pre_commit(ctx)` reaches `ctx.cap.ledger.flush()` through the fused LifecycleCtx; db-backed-runtime.ts `.onPreCommit` now calls `ctx.cap?.ledger?.flush()`; autonomous-swe-agent.py `_shutdown` flushes via `ctx.cap.ledger`; stale "(L3)" clauses in lifecycle-payload TODOs refreshed to the landed L2 wiring.
- cans/08: bridge-layer suspend text split (session handles reason-based / VmHandle path-based); ledger.flush reachability split Rust capture-at-registration vs Py/TS ctx.cap.ledger (L2); TS fork spelling updated to session.fork().

## Gates at convergence

pytest 148 passed / 5 skipped; mypy strict clean (33 files); `tsc -p tsconfig.check.json` exit 0 (src + tests + artifact examples); `pnpm -r exec tsc --noEmit` exit 0; `pnpm test` 50/50; py examples compile + import against the live SDK; comment-law greps clean (rs, rs-doc, ts, py); check-canon, check-examples-canonical, check-plugin-coverage, check-builder-parity all OK. Rust cargo check/clippy/fmt/test skipped — no cargo toolchain in this environment; CI must run them (lane R changes are binding-layer only and pattern-matched to existing code).
