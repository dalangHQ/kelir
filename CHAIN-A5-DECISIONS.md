# CHAIN-A5 decisions (Python builder-chain fix, L1/L2)

Branch `chain-a5`, base `8092fd5`. Scope: `packages/py/**` only.
Spec pins that need a cans/ change are recorded here, not edited in cans/.

## 1. Scopes
- Added `_Scope.end() -> KelirBuilder` (returns the parent builder).
- Added `on_trap_deny` to the trap scope verbs; the scope method delegates
  to `KelirBuilder.on_trap_deny` (it is not one of the 24 `_on` verbs, so it
  cannot go through `_scoped_on`).
- Verified against the live tree: scope callbacks returning None already
  worked (`_scope_entry` discards the callback return value). No code change
  for that half; a test pins it.

## 2. `.plugin()` is creation only
- `KelirBuilder.plugin(name)` stays a class-level creation entry point.
- The instance overload (`_plugin_mount` / `.use` alias) is deleted.
- `builder.plugin(...)` now raises EConfig at the call site:
  `.plugin() on a host builder mounts nothing: KelirBuilder.plugin(name)
  creates a plugin-mode builder; mount a plugin with .use(plugin)`.
  Implemented as a descriptor so class access still creates while instance
  access raises; a plain classmethod could not tell the two apart.
- `builder.plugin("chaos")` no longer falls through to the confusing
  "expected a BasePlugin" from `.use()`.

## 3. Host-only verbs raise immediately
- All 9 raise EConfig at the call site when mode == "plugin", with the same
  message shape as before (`x: host-only verb .terminal() called on
  plugin-mode builder`): detached, terminal, storage, wire, params, trap
  (non-callable path only; the callable path is the trap scope), 
  register_custom_action, register_stream_kind, on_trap_deny.
- `_host_only_calls` / `_record_host_only` deleted; `.use()` and
  `_validate_plugin_shape` no longer consult a deferred list.
- `test_plugin_fluent_builder.py` host-only test now asserts at the call
  site instead of at `.use()`.

## 4. Host-mode `.context()` raises (DECIDED: raise, not wire in)
- `.context()` on a host-mode builder raises EConfig (".context() is
  plugin-only ..."). Rationale: no consumer exists — `boot()` never reads
  `_context_fn` for the host, the native bridge takes no host-context
  parameter, and in Rust `context()` is a plugin-impl method
  (`crates/kelir/src/builder_plugin.rs`). Wiring it in would invent a
  consumer; the silent no-op is fixed by failing loudly instead.
- Plugin-mode `.context()` unchanged; all plugin factories use it.

## 5. `seal() -> Self`
- Validate-only: calls `_validate_all()` and returns self so the chain
  continues. `boot()` now calls `seal()` internally (single validation path).
- No sealed flag: a flag that blocked later verbs would break the chain.
- Audit correction: parity is with Rust only. Rust `seal()` exists
  (`crates/kelir/src/builder_sdk.rs:403`, host-only, returns a Sealed
  builder); TS has no `seal()` anywhere in `packages/ts/core/src`.
  Py has no type-state, so `seal()` validates in both modes instead of
  being host-only.

## 6. `lock_broker()` / `group()` / `fork_state()`
- Added `lock_broker(enabled: bool = True) -> Self`, host-only (raises
  immediately in plugin mode). It writes `lock_broker` into the storage
  config, which the PyO3 bridge already forwards as a bool.
- `storage()` now merges into the existing storage dict instead of
  replacing it, so `.lock_broker().storage(...)` order cannot silently
  drop the broker flag.
- OPEN GAP (not faked): Rust `LockBrokerConfig` also carries
  `scope: LockScope::Globs(Vec<String>)` and `non_blocking: bool`, but the
  PyO3 bridge accepts `lock_broker` as a bool only
  (`crates/kelir-bindings/pyo3/src/storage_bridge.rs`). Accepting scope /
  non_blocking and dropping them at the bridge would be the silent no-op
  this track removes. Forwarding them needs PyO3 `LockBrokerWiring`
  support (crates-owned, L3 bridge work).
- OPEN GAP (not faked): `group()` does not exist as a verb in Rust
  (`crates/kelir/src/builder_*.rs`) or TS (`packages/ts/core/src`), so
  there is no shape to mirror. It needs a spec pin (A1) before any SDK
  implements it.
- `fork_state()` kept as a factory: returns a new plugin-mode builder with
  copied handlers / scope / context / vsock. Test proves mutating the fork
  does not affect the original.

## 7. SSOT example `policy-matrix.py` — NOT edited (A1-owned)
- The file the task cites lives at `cans/artifacts/examples/policy-matrix.py`,
  not under `packages/py`. Its lines 118-119 (`builder =
  builder.plugin(PolicyMatrixPlugin())`) use the deleted instance overload
  and will break under this change; A1 must re-chain them to `.use()`.
- Same for `cans/artifacts/examples/hook-matrix.py:142` (`.plugin(plugin)`).
- The `packages/py` mirror (`packages/py/tests/test_policy_matrix_gate.py`)
  already uses `.use()`; no change needed there.

## Gates run (this worktree, before commit)
- `PYTHONPATH=packages/py python3 -m pytest packages/py`: 133 passed,
  4 skipped (base was 124 passed, 4 skipped; +9 new chain tests).
- Same suite via `uv run --no-project --with pytest --with pydantic
  --with tomli-w pytest packages/py`: 133 passed, 4 skipped.
- `python3 -m mypy` on the three touched files: no issues.
- Py comment-law grep over `packages/py/kelir` + `packages/py/tests`:
  no violations.
- Known pre-existing `test-sdks` boot `ValueError: kernel path is
  required` did NOT reproduce in this run (`test_require_satisfied`
  accepts `(EConfig, ValueError)` and passes). Bare justfile form
  `uv run --no-project pytest` fails on this host because no pytest
  binary exists in the uv env (pre-existing env quirk, unrelated).
