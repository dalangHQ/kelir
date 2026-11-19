# WORK ORDER: THE UNBREAKABLE BUILDER CHAIN CONSOLIDATION

**Scope**: Repository-wide builder refactor across Rust (`crates/kelir`), TypeScript (`packages/ts/core`), Python (`packages/py/kelir`), Codegen (`crates/kelir-interface`), and Canonical Examples (`cans/artifacts/examples`).  
**Stage Target**: L1 / L2 only. L3 logic remains deferred as structured `// TODO (L3)` markers.  
**Concurrency Mode**: 5 Isolated Worktree Agents with zero overlapping write paths.

---

## 0. GLOBAL LAWS & OPERATIONAL CONSTRAINTS (ALL AGENTS)

1. **The Unbroken Chain Law**:
   * Builder assembly is a single unbroken expression: `KelirBuilder::new(...) ... .seal() ... .boot()`.
   * **Zero mid-chain variable reassignment**: No `let mut builder = ...`, no `builder.foo()`, no `b = b.bar()`.
   * **Zero mid-chain fallibility**: No builder verb returns `Result`, `Promise`, `None`, or `void`. The `?` operator is strictly illegal prior to `.seal()`.
2. **Dual-Mode Typestate Law**:
   * A builder is either in `Host` mode or `Plugin` mode.
   * Host-only verbs are inaccessible on Plugin builders at compile-time (Rust `E0599`, TS `tsc`) and raise immediate synchronous `EConfig` at runtime (TS, Python).
   * Seal exists only on `Host, Unsealed` builders.
   * Boot exists only on `Host, Sealed` builders (or dynamic TS/Py host builders auto-sealing on `.boot()`).
3. **Scoped Isolation Law**:
   * All 38 ring verbs and orphan verbs exist **strictly inside scope closures** (`.fs()`, `.net()`, `.trap()`, `.console()`, `.exec()`, `.host()`, `.lifecycle()`, `.telemetry()`, `.actions()`, `.streams()`, `.vsock()`, `.params()`, `.deny()`).
   * Flat ring verbs and orphan methods on the root builder or `createPlugin` are **deleted**.
   * Flat-fold equivalence exists exclusively via `.group(|g| ...)`.
4. **`lock_broker` Canonical Scoping Law**:
   * Standalone `.lock_broker(...)` and `.storage(lock_broker=...)` are **deleted**.
   * Cross-VM lock brokering configuration lives solely inside `.fs(...)`:
     * Rust: `.fs(|f| f.config(FsScopeConfig { lock_broker: Some(...), ..Default::default() }).end())`
     * TS: `.fs((f) => f.config({ lockBroker: { ... } }))`
     * Py: `.fs(lambda f: f.config(lock_broker={...}))`
5. **Level 3 (L3) Deferral Law**:
   * Do not implement hypervisor/kernel meat. Do not write full algorithms.
   * Keep deferrals strictly formatted:
     * Rust/TS: `// TODO (L3) [<case_id_or_domain>]: <what>; slop: <consequence>`
     * Python: `# TODO (L3) [<case_id_or_domain>]: <what>; slop: <consequence>`
6. **Comment Law**:
   * Exact one-liners only: `see:`, `TODO:`, `SAFETY:`.
   * No `///`, `//!`, Python docstrings, multi-line prose, banners, or trailing comments.
7. **Call-Site Generics Ban**:
   * `<>` is mandatory in type definitions (typestate generics enforce the machine) and **banned at the call-site**: if an end-user, example, or test ever has to type `<...>` / `::<...>` / `[...]` on a fluent chain, the SDK is broken.
   * Inference does 100% of the work: `KelirBuilder::new(...).plugin(...).seal().boot()`, `createPlugin("name", ...)`, `.params(|p| p.model(schema).end())`.
   * A verb whose type parameter cannot infer from its arguments is a design flaw, not a caller obligation: redesign it to take a value the parameter infers from (Py precedent: `p.model(Model)`), or delete the verb.
   * Tests are end-user code: negative tests reach the illegal state via the chain (`KelirBuilder::new(...)`, `KelirBuilder::named("p")`), never via `KelirBuilder::<...>`.

---

## 1. AGENT WORK ASSIGNMENTS (PARALLEL EXECUTION)

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│                               PARALLEL AGENT SPLIT                               │
├─────────┬──────────────────────┬─────────────────────────────────────────────────┤
│ AGENT   │ DOMAIN               │ TARGET PATHS                                    │
├─────────┼──────────────────────┼─────────────────────────────────────────────────┤
│ AGENT 1 │ Rust Core Engine     │ crates/kelir/src/                               │
│         │                      │ crates/kelir/tests/compile_fail/                │
├─────────┼──────────────────────┼─────────────────────────────────────────────────┤
│ AGENT 2 │ TypeScript Core SDK  │ packages/ts/core/src/                           │
│         │                      │ packages/ts/core/tests/compile-fail/            │
├─────────┼──────────────────────┼─────────────────────────────────────────────────┤
│ AGENT 3 │ Python Core SDK      │ packages/py/kelir/                              │
│         │                      │ packages/py/tests/                              │
├─────────┼──────────────────────┼─────────────────────────────────────────────────┤
│ AGENT 4 │ Codegen & Parity     │ crates/kelir-interface/src/bin/codegen/         │
│         │                      │ crates/kelir-plugin-cli/src/parity_cli.run.rs   │
├─────────┼──────────────────────┼─────────────────────────────────────────────────┤
│ AGENT 5 │ Canonical Examples   │ cans/artifacts/examples/                        │
│         │ & Quarantine         │ cans/artifacts/examples/deferred/               │
└─────────┴──────────────────────┴─────────────────────────────────────────────────┘
```

---

### AGENT 1: Rust Core Typestate & Scopes

**Exclusive Target Files**:
* `crates/kelir/src/builder_sdk.flow.rs`
* `crates/kelir/src/builder_scopes.flow.rs`
* `crates/kelir/src/builder_plugin.flow.rs`
* `crates/kelir/src/builder_typestate.flow.rs`
* `crates/kelir/src/planes/storage_plane.rs`
* `crates/kelir/tests/compile_fail/builder_negative.rs`

**Directives**:
1. **Typestate Machine Enforcement**:
   * Parameterize `KelirBuilder<Mode, SealState, Ctx, Deps, Plugins, Params>`.
   * Implement host verbs on `KelirBuilder<Host, Unsealed, ...>` only.
   * `seal(self)`: Defined only on `KelirBuilder<Host, Unsealed, ...>`. Returns `Result<KelirBuilder<Host, Sealed, ...>, PluginError>`.
   * `boot(self)`: Defined only on `KelirBuilder<Host, Sealed, ...>`.
   * Delete `lifecycle_mut(&mut self)` and any function returning `&mut Self`.
2. **Orphan Verb Purge & Scope Confinement**:
   * Delete from root builder: `register_custom_action`, `register_stream_kind`, `vsock_service`, `on_trap_deny`, `params(model)`.
   * Ensure scoped entry methods consume scopes and each scope struct (`TrapScope`, `FsScope`, etc.) implements `.end() -> KelirBuilder<Mode, Unsealed, ...>`.
   * Add dedicated scopes:
     * `.actions(|a| a.register(...).end())`
     * `.streams(|s| s.register(...).end())`
     * `.vsock(|v| v.service(...).end())`
     * `.params(|p| p.model(schema).end())`
     * `.deny(|d| d.on_trap_deny(...).end())`
3. **`lock_broker` Relocation**:
   * Delete `lock_broker` from `.storage(...)` and `StoragePlaneConfig`.
   * Move lock brokering into `FsScopeConfig`, reachable via `.fs(|f| f.config(FsScopeConfig { lock_broker: Some(...), .. }).end())`.
4. **Negative Compilation Harness**:
   * Create `crates/kelir/tests/compile_fail/builder_negative.rs` using `trybuild` to assert compile failure (`E0599`) when:
     * Calling `.boot()` on `Unsealed`.
     * Calling host verbs on `KelirBuilder<Plugin, ...>`.
     * Calling scope-specific verbs (e.g. `on_trap`) inside `FsScope`.
5. **Call-Site Generics (Law 7)**:
   * `ParamsScope::model<M>` and `ParamsScope::requires<P>` take no value today, so their type parameters cannot infer and the caller is forced into `model::<T>()` / `requires::<P>()`. `model` is worse: it ignores `M` entirely (`builder_scopes.flow.rs:856`, returns `Self` unchanged) and has zero callers; `requires` only records `type_name::<P>()` (`builder_plugin.flow.rs:107`).
   * Redesign both to take a value the parameter infers from (Py precedent: `p.model(Model)`), or delete the verb. No `::<...>` at any call-site, no exception.
   * Negative tests reach the illegal state via the chain (`KelirBuilder::new(...)`, `KelirBuilder::named("p")`); the three existing `compile_fail` files were already de-turbofished on `primitives-addition` before this run, do not reintroduce `KelirBuilder::<...>`.
   * The `cans` anchor pinning `.params(|p| p.model::<T>())` moves with this redesign: spec text, artifacts, and code move together, the anchor is updated to the inference form, not kept as an excuse for the turbofish.

**Agent Verification**:
```sh
cargo check -p kelir --all-targets
cargo test -p kelir --test compile_fail
```

---

### AGENT 2: TypeScript Core SDK

**Exclusive Target Files**:
* `packages/ts/core/src/builder.lifecycle.ts`
* `packages/ts/core/src/hook.registry.ts`
* `packages/ts/core/src/builder.types.ts`
* `packages/ts/core/src/context.lifecycle.types.ts`
* `packages/ts/core/src/builder.config.ts`
* `packages/ts/core/src/fusion.plugin.ts`
* `packages/ts/core/tests/compile-fail/builder.negative.ts`

**Directives**:
1. **Generic Monad Typestate**:
   * Expand `KelirBuilder<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", Name, Ctx, Deps, Params>`.
   * Enforce `this` constraints:
     * Host-only (`terminal`, `storage`, `wire`, `detached`, `actions`, `streams`, `params`, `deny`, `plugin`, `use`): `this: KelirBuilder<"host", "unsealed", ...>`.
     * Plugin-only (`context`, `forkWith`): `this: KelirBuilder<"plugin", "unsealed", ...>`.
     * Unsealed configuration verbs: `this: KelirBuilder<TMode, "unsealed", ...>`.
     * `.seal()`: Returns `KelirBuilder<TMode, "sealed", ...>`.
     * `.boot()`: Requires `KelirBuilder<"host", ...>`. If `#sealed` is false, auto-seals.
2. **Synchronous Runtime Call-Site Traps**:
   * If `#mode === "plugin"` and host verb is called -> throw `EConfig`.
   * If `#mode === "host"` and plugin verb (`context`, `forkWith`) is called -> throw `EConfig`.
   * If `#sealed === true` and any mutating verb is called -> throw `EConfig("Builder is sealed")`.
3. **Purge Flat Verbs from `HookRegistry` and `createPlugin`**:
   * Delete flat ring hooks (`onFsWrite`, `onNetOutbound`, `onTrap`, etc.) from `createPlugin` and root `HookRegistry`.
   * Scope interfaces (`FsScope`, `NetScope`, `TrapScope`, etc.) must return `Scope | void`.
   * Calling `.onTrap()` inside `.fs(...)` must fail both `tsc` and runtime lookup.
4. **Orphan Verb Scoping & `lock_broker`**:
   * Delete root `.registerCustomAction`, `.registerStreamKind`, `.vsockService`, `.params`, `.onTrapDeny`.
   * Delete `lockBroker` from `StorageConfig`. Accept it exclusively inside `.fs((f) => f.config({ lockBroker: { ... } }))`.
   * Standardize `onTrapDeny` signature: `(event: TrapDenyEvent, verdict: Verdict, session: Session & Ctx) => Promise<void> | void`.
5. **Negative Test Suite**:
   * Add compile-fail cases in `packages/ts/core/tests/compile-fail/builder.negative.ts` verified by `tsc -p tsconfig.compile-fail.json`.
6. **Call-Site Generics (Law 7)**:
   * `createPlugin<Name, Ctx, ...>(name, options?: CreatePluginOptions<Opts> | Ctx)` cannot infer `Ctx`: the `{ opts }` argument matches `CreatePluginOptions`, never `Ctx`, and bare `createPlugin("x")` offers no inference source at all. That is why tests and canonical examples are forced into `createPlugin<"x", Record<string, never>>("x")`.
   * Redesign so every parameter infers from an argument (e.g. a `ctx` field inside options) or drop the parameter. Then strip every explicit `<...>` from `packages/ts/core/tests/` and the canonical examples, including full 6-param `KelirBuilder<...>` return-type annotations — infer them or delete them, keep only load-bearing annotations.

**Agent Verification**:
```sh
pnpm --filter @kelir/core build
pnpm --filter @kelir/core typecheck
pnpm --filter @kelir/core test:compile-fail
```

---

### AGENT 3: Python Core SDK

**Exclusive Target Files**:
* `packages/py/kelir/kelir_builder.py`
* `packages/py/kelir/kelir_builder_scope.py`
* `packages/py/kelir/plugin_model.py`
* `packages/py/kelir/service_caps.py`
* `packages/py/kelir/lifecycle_session.py`
* `packages/py/kelir/storage_planes.py`
* `packages/py/kelir/wire_planes.py`
* `packages/py/tests/test_compile_fail.py`

**Directives**:
1. **Mode & Sealed Runtime Traps**:
   * Add `_mode: Literal["host", "plugin"]` and `_sealed: bool` to `KelirBuilder`.
   * Raise immediate synchronous `EConfig` when:
     * Calling host verbs on `_mode == "plugin"`.
     * Calling `context()` or `fork_with()` on `_mode == "host"`.
     * Calling any configuration verb when `_sealed is True`.
   * Auto-seal inside `.boot()` if not sealed; support `async with kelir.builder(...).use(...).seal() as session:`.
2. **Purge Flat Verbs & `BasePlugin`**:
   * Delete `BasePlugin` subclassing pattern from user flows. Plugins are created solely via `KelirBuilder.plugin("name")`.
   * Delete root orphan verbs: `register_custom_action`, `vsock_service`, `on_vm_*`, `on_trap_deny`, `lock_broker`, standalone `params`.
   * Route all hooks into scope closures (`.fs()`, `.net()`, `.trap()`, `.actions()`, `.streams()`, `.vsock()`, `.params()`, `.deny()`).
3. **Scope Protocols & Return Constraints**:
   * Scope protocols (`FsScopeProtocol`, `TrapScopeProtocol`, etc.) must return `Self | None`.
   * `storage()` keyword arguments reject `lock_broker`, `mask_read`, `carryover_bytes`, `scan_engine` with `EConfig`.
   * `.fs(lambda f: f.config(lock_broker={...}))` is the sole entry point for lock brokering.
   * Standardize `on_trap_deny` to take `(event, verdict, session)`.
4. **Negative Test Suite**:
   * Create `packages/py/tests/test_compile_fail.py` asserting `pyright` diagnostics on mode violations and scope bleeding.

**Agent Verification**:
```sh
uv run mypy packages/py
pyright packages/py/kelir
pytest packages/py/tests/test_compile_fail.py
```

---

### AGENT 4: Codegen Alignment & Parity Gates

**Exclusive Target Files**:
* `crates/kelir-interface/src/bin/codegen/plugin_builder_codegen.flow.rs`
* `crates/kelir-interface/src/bin/codegen/sdk_ts.flow.rs`
* `crates/kelir-interface/src/bin/codegen/sdk_py.flow.rs`
* `crates/kelir-interface/src/bin/codegen/fusion_codegen.flow.rs`
* `crates/kelir-plugin-cli/src/parity_cli.run.rs`

**Directives**:
1. **Plugin Factory Codegen**:
   * Update `plugin_builder_codegen.flow.rs`: Generated factories must emit scoped chains:
     `KelirBuilder.plugin("<name>").require("<dep>").context(lambda: cfg)`
   * Ensure generated factories do not call deleted root verbs or standalone hooks.
2. **SDK Projection Codegen**:
   * Ensure `sdk_ts.flow.rs` and `sdk_py.flow.rs` omit `lockBroker` from `StorageConfig` and emit strictly scoped interfaces.
   * Verify all 38 ring verbs and 13 plugin facades match `cans/artifacts/config/builder-verbs.yaml`.
3. **Parity Harness Update**:
   * Update `crates/kelir-plugin-cli/src/parity_cli.run.rs` to validate single-expression fluent chains across the 96 parity vectors.
   * Ensure the parity suite verifies that `group()` folds yield identical descriptors to nested scopes.
4. **Call-Site Generics Gate (Law 7)**:
   * Add a grep gate that fails on `KelirBuilder::<`, `createPlugin<`, `.model::<`, `.requires::<`, and `KelirBuilder<` used as a call (definition sites excluded) in `crates/*/tests/`, `packages/*/tests/`, `cans/artifacts/examples/`, and generated factories. Wire it into CI alongside the parity suite.

**Agent Verification**:
```sh
cargo run -p kelir-interface --bin codegen
git status --porcelain packages/py/kelir/generated packages/ts/core/src/generated # must be clean or cleanly updated
cargo run -p kelir-plugin-cli -- parity run --vectors vectors.json
```

---

### AGENT 5: Canonical Examples Quarantine & Remediation

**Exclusive Target Files**:
* Move: `cans/artifacts/examples/{serverless-webhook,multi-vm-swarm,db-backed-runtime}.{rs,ts,py}` -> `cans/artifacts/examples/deferred/`
* Remediate:
  * `cans/artifacts/examples/unified-builder-surface.{rs,ts,py}`
  * `cans/artifacts/examples/fluent-builder-chaos.{rs,ts,py}`
  * `cans/artifacts/examples/git-gate.{rs,ts,py}`
  * `cans/artifacts/examples/hook-matrix-full.{rs,ts,py}`
  * `cans/artifacts/examples/policy-matrix.{rs,ts,py}`
  * `cans/artifacts/examples/autonomous-swe-agent.{rs,ts,py}`
  * `cans/artifacts/examples/container.{rs,ts,py}`
  * `cans/artifacts/examples/profile.{rs,ts,py}`
  * `cans/artifacts/examples/replay.{rs,ts,py}`

**Directives**:
1. **Quarantine Execution**:
   * Relocate `serverless-webhook`, `multi-vm-swarm`, and `db-backed-runtime` into `cans/artifacts/examples/deferred/`.
   * Apply unbroken chain cleanup inside `deferred/` files as well, but isolate them from the root CI scan.
2. **Retained Examples Unbroken Chain Transformation**:
   * Eliminate all `let mut builder = ...`, `builder.x()`, and `builder = KelirBuilder(...)`.
   * Transform each example into an unbroken expression:
     `KelirBuilder::new(...)...seal()?.boot().await?`
3. **Fix Critical Domain Bugs**:
   * **`autonomous-swe-agent`**:
     * Rust: Replace first-finish `join_next()` with deterministic iteration over `['A', 'B', 'C', 'D']`.
     * TS: Append destination branch to `c.trap.argv` without dropping user flags.
     * Python: Fix tuple unpacking of `(hypothesis_id, run_result, delta)`; remove invalid `b.id` property access.
   * **`git-gate` & `hook-matrix-full`**:
     * Move all flat `.onFs*`, `.onNet*`, `.onTrap*` calls into `.fs(...)`, `.net(...)`, `.trap(...)`.
   * **`policy-matrix`**:
     * Move `.register_custom_action` to `.actions(...)` and `.vsock_service` to `.vsock(...)`.
   * **`unified-builder-surface`**:
     * Align TS `hostOnlyVerbsRejectedOnPlugin` to assert all 9 rejected verbs.
     * Delete redundant `.storage(lock_broker=True)` calls.
4. **Call-Site Generics (Law 7)**:
   * No `<>` in any canonical example: strip every `createPlugin<"name", Ctx>` call and every full 6-param `KelirBuilder<...>` return-type annotation (e.g. `autonomous-swe-agent.ts`, `fluent-builder-chaos.ts`, `unified-builder-surface.ts`). Do this after Agent 2 lands `createPlugin` inference; a stripped example that no longer compiles is Agent 2's signature to fix, not a reason to restore the `<>`.

**Agent Verification**:
```sh
cargo check --examples
pnpm --filter @kelir/examples exec tsc --noEmit
pyright cans/artifacts/examples/*.py
python3 scripts/check-builder-chain-ast.py cans/artifacts/examples/
```

---

## 2. INTEGRATION CHECKLIST (CONVERGENCE GATE)

Run sequentially once all 5 agent worktrees squash-merge into the target branch:

```sh
# 1. Workspace compilation & clippy
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

# 2. SDK build & typechecks
pnpm -r exec tsc --noEmit
uv run mypy packages/py
pyright packages/py/kelir cans/artifacts/examples/*.py

# 3. Compile-fail negative tests
cargo test -p kelir --test compile_fail
pnpm --filter @kelir/core test:compile-fail
pytest packages/py/tests/test_compile_fail.py

# 4. AST chain scanner & parity vectors
python3 scripts/check-builder-chain-ast.py cans/artifacts/examples/
cargo run -p kelir-plugin-cli -- parity run --vectors vectors.json

# 5. Repo hygiene & comment law
just check-hygiene
just check-spec-drift
```