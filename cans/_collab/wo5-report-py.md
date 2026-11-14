# WO5 py lane report (Agent D, temp/wo5-py)

Scope: `packages/py/**` only. All commits pushed to origin/temp/wo5-py.

- Item 2 — done. No MeshHandle existed in packages/py; no PyO3 mesh binding
  exists (checked crates/kelir-bindings/pyo3), so no native delegation.
  Added `MeshHandle` in service_caps.py: `await_winner(timeout: timedelta)
  -> int` L2 stub raising EConfig, mirroring RS `MeshCtx::await_winner`.
  Wired as `Caps.mesh` (example artifact: "Py Caps expose no mesh handle"),
  exported via service_caps.__all__, kelir/__init__ and its __all__.
  Decision: `peers` NOT added (not pinned; materialize_cap can still attach
  native record fields).
- Item 6 — no-change (verified). `LedgerCap.get_causal_delta_from_parent`
  exists (service_caps.py:66) with the pinned name; also on Session
  (lifecycle_session.py:144).
- Item 12 — done. `ResourcePressure` was a bare class next to `@dataclass
  External(kind)`; now `@dataclass class ResourcePressure: pass` — both
  SuspendReason peers are instantiated classes per pinned contract. Existing
  `ResourcePressure()` call sites unaffected; __all__ already listed both.
- Item 13 — no-change (verified). Verdict/FilterAction `suspend(until=None)` /
  `abort(reason="")` already consistent kwargs with visible field names (also
  HandlerContext delegates). Positional bug is TS-only.
- Item 15 — done. Added `WatchdogAction(str, Enum)` REBOOT/"reboot",
  TERMINATE/"terminate" in kelir/__init__.py + __all__. `LifecycleCtx.
  set_watchdog` now keyword-only `(*, timeout: timedelta, action:
  WatchdogAction)`; `_watchdog` re-typed `tuple[timedelta, WatchdogAction] |
  None`; body unchanged. Decision: no int→timedelta coercion (no caller passes
  int; AGENTS.md decision 1) and `*` enforces the pinned kwargs shape.
  Import cycle safe: enum defined before first runtime model_plugin import.
- Item 16 — done. kelir_builder.py `on_trap_deny` registers `_wrap_trap_deny(
  handler)`: async wrapper, awaits awaitables, re-raises non-EConfig failures
  as `EConfig("on_trap_deny callback failed: ...") from exc` (EConfig passes
  through). Matches pinned Awaitable[None] raises-on-failure contract;
  _Scope.on_trap_deny delegates through the same wrap point.
- Item 20 — no-change (verified). `LedgerCap.count_denials(ppid: int) -> int`
  exists (service_caps.py:62); `session.cap.ledger.count_denials(pid)`
  callable, already exercised in tests/test_ssot_a5.py:76.

Gates: compileall clean; comment-law grep prints nothing; zero `"""` in
touched files; pytest 145 passed / 5 skipped (canonical config; tests/red
excluded — 3 red failures are pre-change by design, need L3 native _kelir).
Runtime smoke verified enum values, kwargs-only rejection, EConfig re-raise
with chained cause, sync+async handler pass-through.

Risks: (a) Caps.mesh placement is my call — if the spec lane pins mesh ctx
instead of a cap, MeshHandle moves (single class, cheap). (b) set_watchdog
now rejects positional/int at type level; no in-tree caller breaks.
(c) Examples lane should call `session.cap.mesh.await_winner(timedelta(...))`.
Env: packages/py/.venv lacked tomli-w (lock/env drift); installed tomli-w +
pytest into the gitignored venv for the gate run only.
