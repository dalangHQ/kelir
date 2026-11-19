# THE UNBREAKABLE BUILDER CHAIN & EXAMPLES CONSOLIDATION DIRECTIVE

---

## PART 1: THE UNBREAKABLE BUILDER CHAIN SPECIFICATION

The builder architecture across Rust, TypeScript, and Python operates under a single physical constraint: **an unbroken, single-expression typestate chain ending in execution**.

```
KelirBuilder::new(...) [ .verb(...) | .scope(...) ]* -> .seal() -> .boot()
```

### 1. Architectural Invariants
1. **Zero Mid-Chain Reassignment**: No `let mut builder = ...; builder.foo();` or `builder = KelirBuilder(...); builder.bar()`. The builder is never bound to a mutable local variable.
2. **Zero Mid-Chain Fallibility**: No chain verb returns `Result`, `void`, or `None`. The `?` operator is strictly illegal anywhere prior to `.seal()`.
3. **Dual-Mode Identity**: A plugin **is** a `KelirBuilder` instance in plugin mode. A host **is** a `KelirBuilder` instance in host mode. Mount is a value-merge (`host.plugin(plugin_builder)` or `host.use(plugin_builder)`).
4. **Single Validation Gate**: `.seal()` is the only validation phase across host configuration, mounted plugins, dependencies, and scopes. `.boot()` implicitly seals if unsealed in dynamic runtimes.
5. **Scoped Isolation**: Direct ring verbs outside of an explicit scope closure do not exist. Standalone orphan verbs are deleted from the root builder surface and exist only inside their designated scope closures.

---

### 2. Language-Specific Mechanical Enforcement

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                                  SDK TYPING ENGINES                                    │
├─────────────────────────┬──────────────────────────────┬───────────────────────────────┤
│ RUST                    │ TYPESCRIPT                   │ PYTHON                        │
├─────────────────────────┼──────────────────────────────┼───────────────────────────────┤
│ Typestate Parameterized │ Generic Monad Accumulation   │ Fluent Monad + Protocols      │
│ Self-by-value           │ Returns `this`               │ Returns `self`                │
│ Compile-time (E0599)    │ Compile-time (tsc) + Runtime │ Static (pyright) + Runtime    │
│ Scope requires `.end()` │ Scope returns `Scope | void` │ Scope returns `Scope | None`  │
└─────────────────────────┴──────────────────────────────┴───────────────────────────────┘
```

#### A. Rust: Typestate Machine
Enforced mechanically via the 6-type generic signature:
```rust
pub struct KelirBuilder<Mode, SealState, Ctx, Deps, Plugins, Params> {
    mode: PhantomData<Mode>,
    seal_state: PhantomData<SealState>,
    // Private fields
}
```
* **Modes**: `HostMode` | `PluginMode`.
* **Seal States**: `Unsealed` | `Sealed`.
* **State Progression**:
  * Construction: `KelirBuilder::new(VmConfig)` -> `KelirBuilder<HostMode, Unsealed, (), HNil, HNil, NoParams>`.
  * Plugin Construction: `KelirBuilder::named("name")` -> `KelirBuilder<PluginMode, Unsealed, Ctx, Deps, HNil, NoParams>`.
  * Chaining: All verbs consume `self` by value and return `KelirBuilder<Mode, Unsealed, ...>`.
  * Sealing: `seal(self) -> Result<KelirBuilder<Mode, Sealed, Ctx, Deps, Plugins, Params>, PluginError>`. This is the **only** mid-chain method returning `Result`.
  * Terminal Boot: `boot(self) -> Future<Output = Result<Session, EngineError>>` is defined **exclusively** on `KelirBuilder<HostMode, Sealed, ...>`.
    * Attempting `.boot()` on `Unsealed` -> Compiler Error `E0599: no method named 'boot' found for KelirBuilder<..., Unsealed, ...>`.
    * Attempting `.boot()` on `PluginMode` -> Compiler Error `E0599`.
    * Calling any configuration verb on `Sealed` -> Compiler Error `E0599`.
* **Prerequisite Gating**: Mount (`.plugin()`) requires `Plugins: Satisfies<Deps>`. A missing or out-of-order dependency fails at compile time.
* **Scope Closures**: Scopes take a closure `FnOnce(RingScope) -> RingScope`. To enforce that the scope closure yields back the builder, the scope struct exposes `.end() -> KelirBuilder<Mode, Unsealed, ...>`.

#### B. TypeScript: Generic Monad & Synchronous Call-Site Barrier
* **Chaining Signature**: Every builder verb returns `this`.
* **Type Signature**: `KelirBuilder<TPlugins = {}, TParams = undefined>`.
  * `.requires<P>()` accumulates prerequisite type tags.
  * `.use(plugin)` enforces at compile time via `tsc` that prerequisites exist in `TPlugins`.
* **Runtime Call-Site Traps (Synchronous `EConfig`)**:
  * Host-only verbs (`storage`, `terminal`, `wire`, `detached`, `boot`, `seal`, `use`) throw `EConfig` **immediately at the call site** if `this.mode === 'plugin'`.
  * `forkState()` throws `EConfig` immediately if `this.mode === 'host'`.
  * `host.plugin("string")` throws `EConfig` immediately (must pass a `KelirBuilder` instance).
* **Seal Locking**:
  * Internal `private _sealed: boolean = false`.
  * Any verb invocation when `this._sealed === true` synchronously throws `EConfig("Builder is sealed")`.
  * `seal(): this` performs validation (duplicate mounts, scope well-formedness, missing prerequisites) and sets `this._sealed = true`.
  * `boot(): Promise<Session>` calls `if (!this._sealed) this.seal();`.
  * `fuse()` is private to the package; calling it from userland is a TypeScript compilation error.
* **Scope Interfaces**: Scopes implement strict interfaces (`FsScope`, `NetScope`, `TrapScope`). Calling `.onTrap()` inside `.fs(...)` fails compilation under `tsc`. Scope closures return `Scope | void`.

#### C. Python: Fluent Monad & Protocol Validation
* **Chaining Signature**: Every builder verb returns `self`.
* **Runtime Call-Site Traps (Synchronous `EConfig`)**:
  * Host-only verbs (`storage`, `terminal`, `wire`, `detached`, `boot`, `seal`, `use`) raise `EConfig` **at the call site** if `self._mode == "plugin"`.
  * `context()` raises `EConfig` at the call site if `self._mode == "host"`.
  * `fork_state()` raises `EConfig` at the call site if `self._mode == "host"`.
* **Seal Locking**:
  * Internal `self._sealed: bool = False`.
  * Any mutation attempted when `self._sealed is True` raises `EConfig("Builder is sealed")`.
  * `seal() -> Self` executes validation (duplicate mounts, dependency satisfaction, vsock port ranges, scope validation) and sets `self._sealed = True`.
  * `boot() -> Session` invokes `self.seal()` if not yet sealed.
  * Context entry (`async with builder as session:`) calls `await builder.boot()`.
* **Static Typing**: Scopes are typed as runtime `Protocol` definitions (`FsScopeProtocol`, `TrapScopeProtocol`, etc.). Calling `.on_trap()` inside `.fs(...)` triggers static failure in `mypy` and `pyright`. Closures return `Scope | None`.

---

### 3. Canonical Scope Mapping & Orphan-Verb Elimination

All standalone "orphan" verbs are purged from the root builder surface. They are mapped into scope closures identically across all three SDKs:

| Legacy Deleted Standalone Verb | Mandatory Scoped Closure | Scope Target Method | Allowed Modes |
| :--- | :--- | :--- | :--- |
| `register_custom_action(name, s, h)` | `.actions(\|a\| ...)` | `a.register(name, schema, handler)` | Host |
| `register_stream_kind(kind, s, h)` | `.streams(\|s\| ...)` | `s.register(kind, schema, handler)` | Host |
| `vsock_service(port, s, h)` | `.vsock(\|v\| ...)` | `v.service(port, service, handler)` | Host & Plugin |
| `params(model)` | `.params(\|p\| ...)` | `p.model::<T>()` / `p.model(Schema)` | Host |
| `on_trap_deny(h)` | `.deny(\|d\| ...)` | `d.on_trap_deny(handler)` | Host |
| `lock_broker(config)` | `.fs(\|f\| ...)` | `f.config(FsScopeConfig { lock_broker, .. })` | Host |
| `on_trap`, `on_process_exit`, etc. | `.trap_scope(\|t\| ...)` (RS)<br>`.trap(\|t\| ...)` (TS/Py) | `t.on_trap(...)`, `t.on_process_exit(...)` | Host & Plugin |
| `on_fs_lookup`, `on_fs_read`, etc. | `.fs(\|f\| ...)` | `f.on_fs_lookup(...)`, `f.on_fs_read(...)` | Host & Plugin |
| `on_net_outbound`, `on_net_inbound` | `.net(\|n\| ...)` | `n.on_net_outbound(...)`, `n.on_net_inbound(...)` | Host & Plugin |
| `on_console_input`, `on_console_output`| `.console(\|c\| ...)` | `c.on_console_input(...)`, `c.on_console_output(...)`| Host & Plugin |
| `on_exec_stdout`, `on_exec_stderr` | `.exec(\|e\| ...)` | `e.on_exec_stdout(...)`, `e.on_exec_stderr(...)` | Host & Plugin |
| `on_host_lsm_exec`, `on_host_pressure` | `.host(\|h\| ...)` | `h.on_host_lsm_exec(...)`, `h.on_host_pressure(...)` | Host & Plugin |
| `on_vm_init`, `on_vm_ready`, etc. | `.lifecycle(\|l\| ...)` | `l.on_vm_init(...)`, `l.on_vm_ready(...)` | Host & Plugin |
| `on_memory_pressure`, `on_heartbeat` | `.telemetry(\|t\| ...)` | `t.on_memory_pressure(...)`, `t.on_heartbeat(...)` | Host & Plugin |

#### Flat-Fold Equivalence (`group`)
A flat builder chain exists solely as a `group()` fold over the scopes:
* **Rust**: `builder.group(|g| g.trap_scope(...).fs(...))`
* **TypeScript**: `builder.group((g) => { g.trap(...); g.fs(...); })`
* **Python**: `builder.group(lambda g: g.trap(...).fs(...))`
The descriptors generated by `group()` are strictly identical in ring assignments and ordinals to direct nested-scope calls.

---

### 4. CI AST & Parity Enforcement Mechanics

1. **AST Reassignment & Chain Scanner (`just check-examples`)**:
   A pre-test static analysis script parses `cans/artifacts/examples/**/*.{rs,ts,py}`. The build aborts if:
   * A variable assigned a `KelirBuilder` is reassigned or mutated after creation (`let mut builder = ...`, `builder = KelirBuilder(...)` followed by statement-level calls).
   * Any `?` operator occurs on a method other than `.seal()`.
   * Any deleted root-level verb (`register_custom_action`, `register_stream_kind`, `vsock_service`, `on_trap_deny`, `lock_broker`) appears outside its designated scope closure.
2. **Canonical Parity Test (`kelir-plugin-cli parity run`)**:
   Enforces all 96 builder canonical cases (38 ring verbs + 48 scope sub-nodes + 5 chain cases + 5 predicate cases) expressed as **single-expression chains** in Rust, TS, and Python. Output descriptors must match bitwise.
3. **Executable Negative Suites**:
   Negative invariants are moved from code comments to dedicated test runners:
   * **Rust**: `tests/compile_fail/` using `trybuild` asserting `E0599` on post-seal calls, scope bleeding, and host-verb calls on `PluginMode`.
   * **TypeScript**: `tests/compile_fail/` using `// @ts-expect-error` executed via `tsc --noEmit`.
   * **Python**: `tests/compile_fail/` asserting type-check rejections via `pyright --stats`.

---

## PART 2: COMPLETE AUDIT & REMEDIATION PLAN FOR `examples/`

The top-level `cans/artifacts/examples/` directory is an absolute SSOT contract in Kelir: **all top-level examples must compile, typecheck, and pass hygiene in Rust, TypeScript, and Python or CI fails.**

```
cans/artifacts/examples/
├── deferred/                       <── QUARANTINED (Bypassed by check-examples.sh)
│   ├── serverless-webhook.{rs,ts,py}
│   ├── multi-vm-swarm.{rs,ts,py}
│   └── db-backed-runtime.{rs,ts,py}
│
├── autonomous-swe-agent.{rs,ts,py} <── RETAINED & REFACTORED (Full chain + SSOT-A1 fixes)
├── container.{rs,ts,py}            <── RETAINED (Normalize seal/boot)
├── fluent-builder-chaos.{rs,ts,py} <── RETAINED (Canonical descriptor chain)
├── git-gate.{rs,ts,py}             <── RETAINED (Eliminate BasePlugin in Py)
├── hook-matrix-full.{rs,ts,py}     <── RETAINED (Eliminate BasePlugin, use scopes)
├── policy-matrix.{rs,ts,py}        <── RETAINED (Eliminate BasePlugin, use scopes)
├── profile.{rs,ts,py}              <── RETAINED (Normalize seal/boot)
├── replay.{rs,ts,py}               <── RETAINED (Normalize seal/boot)
└── unified-builder-surface.{rs,ts,py} <── RETAINED (Ground truth parity reference)
```

---

### 1. Quarantine Mandate (Phase A)
Per `cans/15-operations.md#Examples-compile`, `scripts/check-examples.sh` globs `cans/artifacts/examples/*.{rs,ts,py}` **non-recursively**. 

Move all examples that depend on un-landed L3 kernel drivers, deep hardware virtualization, or multi-VM coordination into `cans/artifacts/examples/deferred/`:
* `serverless-webhook.rs`, `serverless-webhook.ts`, `serverless-webhook.py`
* `multi-vm-swarm.rs`, `multi-vm-swarm.ts`, `multi-vm-swarm.py`
* `db-backed-runtime.rs`, `db-backed-runtime.ts`, `db-backed-runtime.py`

*Zero broken or mock-dependent examples may reside in the root of `examples/`.*

---

### 2. Comprehensive File-by-File Remediation Matrix (Phase B)

#### 1. `unified-builder-surface.{rs,ts,py}`
* **Role**: Primary reference implementation for the 38 ring verbs and 8 scopes.
* **Rust (`.rs`)**:
  * Un-comment the comment-pinned negative tests (`negative_fs_on_trap`, `host_only_verbs_rejected_on_plugin`) and migrate them into `crates/kelir/tests/compile_fail/builder_negative.rs`.
  * Ensure every scope call uses `.end()` to return `Self`.
* **TypeScript (`.ts`)**:
  * Move comment-pinned negative tests to `packages/core/test/compile_fail/builder.negative.ts`.
  * Validate that all 8 scopes (`trap`, `fs`, `net`, `console`, `exec`, `host`, `lifecycle`, `telemetry`) return `this | void`.
* **Python (`.py`)**:
  * Move comment-pinned negative tests to `packages/py/tests/test_compile_fail.py`.
  * Assert scope closures receive `Protocol` objects and return `self` or `None`.

#### 2. `fluent-builder-chaos.{rs,ts,py}`
* **Role**: Canonical descriptor-only plugin construction and host merge.
* **Remediation Across All 3 Languages**:
  * **Rust**: `build_chaos_host()` must return an unbroken chain ending in `.seal()?`.
  * **TypeScript**: Ensure `buildChaosHost()` executes `.seal()` cleanly. Ensure `buildChaosPlugin()` returns the unsealed, unbooted plugin builder directly.
  * **Python**: Normalize `build_chaos_host()` to chain through to `.seal()`.

#### 3. `autonomous-swe-agent.{rs,ts,py}`
* **Role**: Full-surface integration demonstrating branch forks, storage reconciliation, and attestation.
* **Rust (`.rs`) Remediation**:
  * **Break Chain Fix**: Convert `build_runtime()` to an uninterrupted fluent chain.
  * **SSOT-A1 Outlier Fix (Winner Selection)**:
    * *Current Defect*: Lines 626–633 use `while let Some(joined) = joins.join_next().await`, declaring the first branch that completes to be the winner.
    * *Fix*: Collect all results from `joins.join_next()`. Iterate across candidates in **declared branch order** (`1..=N`). Select the first branch where `exit_code == 0`.
* **TypeScript (`.ts`) Remediation**:
  * **SSOT-A1 Outlier Fix (Git Push Argv Rewrite)**:
    * *Current Defect*: Line 155 completely reconstructs `argv` from scratch, dropping options.
    * *Fix*: Preserve original remote and flags. Append `HEAD:refs/heads/agent/<hex>` to the existing `argv` array, matching Rust and Python.
* **Python (`.py`) Remediation**:
  * **BasePlugin Elimination**: Delete `class RepoPolicyPlugin(BasePlugin)`. Re-implement `repo_policy_plugin()` as a scoped builder:
    ```python
    def repo_policy_plugin(opts: RepoPolicyOptions) -> KelirBuilder:
        return (
            KelirBuilder.plugin("repo-policy")
            .require("ledger")
            .fs(lambda f: f.on_fs_write(handle_write).on_fs_unlink(handle_unlink))
            .trap(lambda t: t.on_trap(handle_trap))
        )
    ```

#### 4. `git-gate.{rs,ts,py}`
* **Role**: Focused interposition (terminal + trap interposition).
* **Rust (`.rs`)**:
  * Ensure `GitGatePlugin` mounts cleanly via `.plugin_instance(GitGatePlugin)` on the builder.
  * Chain: `KelirBuilder::new(config)...plugin_instance(GitGatePlugin).seal()?.boot().await?`.
* **TypeScript (`.ts`)**:
  * Ensure plugin is built via `createPlugin("git-gate")` and chained into the host via `host.use(gitGate)`.
* **Python (`.py`)**:
  * Eliminate `class GitGatePlugin(BasePlugin)`. Replace with `KelirBuilder.plugin("git-gate").trap(...)`.

#### 5. `hook-matrix-full.{rs,ts,py}`
* **Role**: Verification of all 38 ring verbs and `TrapFilter` predicate grammar.
* **Rust (`.rs`)**:
  * Maintain `HookMatrixFullPlugin` as the definitive trait-level regression fixture mounted via `.plugin_instance()`.
  * Ensure `.deny(|d| d.on_trap_deny(...))` and `.trap(TrapFilter::binary(...).with_arg(...)...)` are chained directly on the builder before `.seal()?`.
* **TypeScript (`.ts`)**:
  * Ensure the 5-predicate `TrapFilter` chain (`withArg`, `withCwd`, `withEnv`, `withUid`, `argvMatches`) is declared inline within `.trap(filter, handler)`.
* **Python (`.py`)**:
  * Eliminate `class HookMatrixFullPlugin(BasePlugin)`.
  * Replace the module-level constants `IDENTITY_PATH`, `DENY_MARKER`, and `CARRYOVER` with `.fs(lambda f: f.rules(...))` per `02-plugin-model.md#Nested-scopes`.

#### 6. `policy-matrix.{rs,ts,py}`
* **Role**: Exercises the unified `Action` (Verdict) lattice and watchdog suppression/mitigation.
* **Rust (`.rs`)**:
  * Ensure all 8 lattice variants (`Pass`, `Forward`, `Drop`, `Suspend`, `Abort`, `Patch`, custom, operator lease) are tested through an unbroken builder chain.
  * Ensure orphan scopes `.actions(|a| a.register(...))` and `.vsock(|v| v.service(...))` are chained inline.
* **TypeScript (`.ts`) & Python (`.py`)**:
  * Eliminate `BasePlugin` inheritance in Python.
  * Assert that watchdog return types (`WatchdogVerdict.Mitigate`, `WatchdogVerdict.Suppress`) execute through `.telemetry(lambda t: t.on_watchdog(...))`.

#### 7. `container.{rs,ts,py}`
* **Role**: Rootless OCI execution without nested hypervisor privileges.
* **Remediation Across All 3 Languages**:
  * Remove redundant local bindings.
  * Ensure `.seal()?.boot().await?` (RS) and `.seal().boot()` (TS/Py) form the singular terminal pipeline.

#### 8. `profile.{rs,ts,py}`
* **Role**: Declarative specification ingestion and round-trip export.
* **Remediation Across All 3 Languages**:
  * Ensure builder hydration occurs directly via `KelirBuilder.from_config("./fixtures/kelir.toml")`.
  * Verify that chaining continues immediately off `from_config()` before `.seal()` and `.boot()`.

#### 9. `replay.{rs,ts,py}`
* **Role**: Deterministic instruction-stepped clock execution.
* **Remediation Across All 3 Languages**:
  * Enforce network isolation in configuration: `pinned_clocksource: "kvm-clock"`, `deterministic_clock: true`.
  * Chain `.seal()?.boot()` as an unbroken expression.

---

### 3. Immediate Remediation of Quarantined Files (For Future Re-integration)

When the files in `cans/artifacts/examples/deferred/` are ready to be brought back to the root, apply these specific fixes:

#### A. `multi-vm-swarm.rs`
* **Current Defect**:
  ```rust
  let mut builder = swarm_builder();
  builder.lifecycle_mut().register_veto(Box::new(...));
  let session = builder.seal()?.boot().await?;
  ```
* **Remediation**:
  Fold the veto directly into the unbroken builder chain inside `boot_swarm()`:
  ```rust
  let session = swarm_builder()
      .lifecycle(|l| {
          l.on_vm_init(move |ctx| {
              ctx.register_veto(VetoEvent::Fork, Box::new(move || {
                  Veto::allowed(veto_flag.load(Ordering::SeqCst))
              }));
              Ok(())
          })
          .end()
      })
      .seal()?
      .boot()
      .await?;
  ```
* **Orphan Verb Fix**: Replace root-level `.lock_broker(LockBrokerConfig { ... })` with `.fs(|f| f.config(FsScopeConfig { lock_broker: Some(...), ..Default::default() }).end())`.

#### B. `db-backed-runtime.ts`
* **Current Defect**: Calls `ledger.finalize(...)` with a null or missing `includeDelta`.
* **Remediation**:
  Conform strictly to `03-sdk-architecture.md#Convergence-pins`:
  ```typescript
  const delta = await ledger.getCausalDeltaFromParent();
  const finalReceipt = await ledger.finalize({
    includeDelta: delta, // Required: cannot be omitted or undefined
    includeMerkleRoot: true,
  });
  ```

#### C. `serverless-webhook.rs`
* **Current Defect**: Uses standalone `.wire(...)` with custom L4 proxy declarations, string-matched traps, and un-scoped plugin mounts.
* **Remediation**:
  Chain `.wire(wire_config)` directly on the host builder. Chain plugins via `.plugin_instance(...)`. Replace string traps with typed `TrapFilter::binary("dispatch-webhook")`.

---

### 4. Verification & Gate Enforcement (Phase C)

Execute the three-stage gate locally and in CI to guarantee compliance:

```bash
# Stage 1: Absolute Typecheck & Compilation
# Verifies all top-level examples in RS, TS, and Py compile against live SDKs
just check-examples

# Stage 2: Physical Hardware & Emulated Execution
# Executes every top-level example; fails on any panic, unhandled error, or timeout
just check-examples-run

# Stage 3: Zero-Drift & Anti-Slop Enforcement
# Verifies zero dead verbs, zero reassigned builders, zero inline cfg(test)
just check-spec-drift
just check-hygiene
```

Any example violating the unbroken chain rule, attempting variable reassignment, using deleted orphan verbs, or failing language parity will be rejected at Stage 1.