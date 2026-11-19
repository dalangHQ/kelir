fluent-chain.work-order.md 

**`x = 3` parts.**

- **Part 1/3**: Unbreakable Builder Chain Specification & Enforcement Mechanics (this message)
- **Part 2/3**: `examples/` Triage, Spec Contradictions, & File-by-File Remediation (Core Surface + Policy/Gate/Matrix Examples)
- **Part 3/3**: File-by-File Remediation (SWE Agent, DB, Swarm, Webhook, Minimal Examples) + Verification Gate

---

```markdown
# THE UNBREAKABLE BUILDER CHAIN & EXAMPLES CONSOLIDATION DIRECTIVE

---

## PART 1: THE UNBREAKABLE BUILDER CHAIN SPECIFICATION

The builder architecture across Rust, TypeScript, and Python operates under a single physical constraint: **an unbroken, single-expression typestate chain ending in execution**.

```
KelirBuilder::new(...) [ .verb(...) | .scope(...) ]* -> .seal() -> .boot()
```

### 1. Architectural Invariants
1. **Zero Mid-Chain Reassignment**: No `let mut builder = ...; builder.foo();`, no `builder.lifecycle_mut()`, and no `builder = KelirBuilder(...); builder.bar()`. The builder is never mutated via a side-channel reference or statement-level reassignment.
2. **Zero Mid-Chain Fallibility**: No chain verb returns `Result`, `Promise`, `void`, or `None`. The `?` operator is illegal anywhere prior to `.seal()`.
3. **Dual-Mode Identity**: A plugin **is** a `KelirBuilder` in plugin mode. A host **is** a `KelirBuilder` in host mode. Mounting is a value-merge (`host.plugin(plugin_builder)` / `host.use(plugin_builder)`).
4. **Single Validation Gate**: `.seal()` is the sole validation phase for host config, duplicate mounts, `requires` satisfaction, vsock port ranges, and scopes. `.boot()` seals implicitly only in dynamic runtimes (TS/Py) if unsealed; `fuse()` is package-private.
5. **Scoped Isolation**: All ring verbs and orphan verbs exist exclusively inside typed scope closures. Standalone/flat verbs are deleted from the root builder surface; a flat chain exists only as a `group()` fold producing identical ring/ordinal descriptors.

---

### 2. Language-Specific Mechanical Enforcement

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                                  SDK TYPING ENGINES                                    │
├─────────────────────────┬──────────────────────────────┬───────────────────────────────┤
│ RUST                    │ TYPESCRIPT                   │ PYTHON                        │
├─────────────────────────┼──────────────────────────────┼───────────────────────────────┤
│ Typestate Parameterized │ Mode+Seal Generic Monad      │ Host/Plugin Typed + Protocols │
│ Consumes `self` by val  │ Returns `this` (re-typed)    │ Returns `self` (re-typed)     │
│ Compile-time (E0599)    │ Compile-time (tsc) + Runtime │ Static (pyright) + Runtime    │
│ Scope requires `.end()` │ Scope returns `Scope | void` │ Scope returns `Scope | None`  │
└─────────────────────────┴──────────────────────────────┴───────────────────────────────┘
```

#### A. Rust: Typestate Machine
Enforced mechanically via the 6-parameter generic signature:
```rust
pub struct KelirBuilder<Mode, SealState, Ctx, Deps, Plugins, Params> {
    mode: PhantomData<Mode>,
    seal_state: PhantomData<SealState>,
    // Private fields
}
```
* **Modes**: `Host` | `Plugin`. **Seal States**: `Unsealed` | `Sealed`.
* **State Progression**:
  * Host: `KelirBuilder::new(VmConfig)` -> `KelirBuilder<Host, Unsealed, (), HNil, HNil, NoParams>`.
  * Plugin: `KelirBuilder::named("name")` -> `KelirBuilder<Plugin, Unsealed, Ctx, Deps, HNil, NoParams>`.
  * Verbs consume `self` by value on `Unsealed` and return `KelirBuilder<Mode, Unsealed, ...>`.
  * `seal(self)` exists **only** on `KelirBuilder<Host, Unsealed, ...>` -> `Result<KelirBuilder<Host, Sealed, ...>, PluginError>`.
  * `boot(self)` exists **exclusively** on `KelirBuilder<Host, Sealed, ...>`. Unsealed `.boot()`, `Plugin` `.boot()`, or post-seal verb calls fail with `E0599`.
* **Break-Chain Removal**: Delete `lifecycle_mut(&mut self)` entirely; lifecycle hooks and vetoes register exclusively via `.lifecycle(|l| ... .end())`.
* **Mode & Prerequisite Gating**: Host-only verbs (`storage`, `terminal`, `wire`, `detached`, `fast_boot`, `dax_window`, `plugin`, `plugin_instance`, `trap_async`, `post_exec`, `actions`, `streams`, `params`, `deny`) exist only on `Host`. `.plugin()` / `.r#use()` require `Plugins: Satisfies<Deps>`.
* **Scope Closures**: `FnOnce(RingScope) -> RingScope`. Scope structs expose `.end() -> KelirBuilder<Mode, Unsealed, ...>`.

#### B. TypeScript: Typestate Monad & Synchronous Call-Site Barrier
* **Current Gap to Close**: `KelirBuilder` currently tracks `<TName, TCtx, TRequires, TPlugins>` without mode or seal state, allowing `plugin.terminal(...)` to pass `tsc` without `@ts-expect-error` and relying only on runtime `EConfig`.
* **Compile-Time Fix**:
  * Parameterize `KelirBuilder<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", TName, TCtx, TRequires, TPlugins, TParams>`.
  * Constrain config verbs with `this: KelirBuilder<TMode, "unsealed", ...>`, host-only verbs with `this: KelirBuilder<"host", "unsealed", ...>`, and plugin-only verbs (`context`, `forkWith`) with `this: KelirBuilder<"plugin", "unsealed", ...>`.
  * `.seal()` returns `KelirBuilder<TMode, "sealed", ...>`. `.boot()` requires `KelirBuilder<"host", TSealed, ...>`.
  * Scopes implement strict interfaces (`FsScope`, `NetScope`, `TrapScope`, etc.) returning `Scope | void`. Calling `.onTrap()` inside `.fs(...)` fails `tsc`.
* **Runtime Call-Site Traps (Synchronous `EConfig` for JS/untyped callers)**:
  * Host-only verbs throw `EConfig` immediately if `#mode === "plugin"`. `context()`/`forkWith()` throw `EConfig` if `#mode === "host"`. `host.plugin("string")` throws `EConfig`.
  * `#sealed === true` causes any config verb to throw `EConfig("Builder is sealed")`.
  * `group(fn)` validates that `fn` returns `void` or `this`; any other return throws `EConfig`.
  * `boot()` calls `if (!this.#sealed) this.seal();`. `fuse()` remains package-private.
* **Legacy Surface Deletion**: Delete flat ring verbs on `createPlugin` (`onFsWrite`, `onNetOutbound`, `onTrap`, etc.), top-level `.on(...)`, `.onTrapDeny(...)`, `.registerCustomAction(...)`, `.registerStreamKind(...)`, `.vsockService(...)`, and top-level `.params(...)` from `@kelir/core`.

#### C. Python: Typed Mode/Seal Surface, Protocol Scopes & Runtime Traps
* **Current Gap to Close**: `just check-examples` currently runs only syntax + import checks, so `Protocol` scopes (`FsScopeProtocol`) and mode mismatches are never verified statically in CI.
* **Static Fix**:
  * Enforce `pyright` (or `mypy --strict`) inside `just check-examples`.
  * Split `HostBuilder` / `PluginBuilder` (or parameterize `KelirBuilder[Mode, Sealed]` with `@overload`s) so `seal()` transitions type state and plugin builders omit host-only methods statically.
  * Scopes are typed via `Protocol` (`FsScopeProtocol`, `TrapScopeProtocol`, etc.) returning `Scope | None`. Calling `.on_trap()` inside `.fs(...)` fails `pyright`.
* **Runtime Call-Site Traps (Synchronous `EConfig`)**:
  * Host-only verbs (`storage`, `terminal`, `wire`, `detached`, `boot`, `seal`, `use`, `plugin`, `actions`, `streams`, `params`, `deny`) raise `EConfig` immediately if `self._mode == "plugin"`.
  * `context()` and `fork_state()`/`fork_with()` raise `EConfig` immediately if `self._mode == "host"`.
  * Any mutation when `self._sealed is True` raises `EConfig("Builder is sealed")`.
  * `boot()` invokes `self.seal()` if unsealed; `async with builder as session:` invokes `await builder.boot()`.
* **Legacy Surface Deletion**: Remove `BasePlugin` subclassing from examples, delete top-level `on_vm_init`, `on_vm_ready`, `on_vm_shutdown`, `on_pre_commit`, `on_post_commit`, `on_trap_deny`, `register_custom_action`, `register_stream_kind`, `vsock_service`, `wire_config`, and drop `kelir.plugins.plugin_*` aliases in favor of canonical `kelir.plugins.<name>`.

---

### 3. Canonical Scope Mapping & Orphan-Verb Elimination

All standalone/flat/orphan verbs are purged from the root builder surface across all three SDKs:

| Legacy Deleted Standalone Verb | Mandatory Scoped Closure | Scope Target Method | Allowed Modes |
| :--- | :--- | :--- | :--- |
| `register_custom_action(name, s, h)` | `.actions(\|a\| ...)` | `a.register(name, schema, handler)` | Host |
| `register_stream_kind(kind, s, h)` | `.streams(\|s\| ...)` | `s.register(kind, schema, handler)` | Host |
| `vsock_service(port, s, h)` | `.vsock(\|v\| ...)` | `v.service(port, service, handler)` | Host & Plugin |
| `params(model)` | `.params(\|p\| ...)` | `p.model::<T>()` / `p.model(Schema)` | Host |
| `on_trap_deny(h)` | `.deny(\|d\| ...)` | `d.on_trap_deny(handler)` | Host |
| `lock_broker(config)` | `.fs(\|f\| ...)` | `f.config(FsScopeConfig { lock_broker, .. })` | Host |
| `on_trap`, `on_process_exit`, `on_signal`, `on_fork` | `.trap_scope(\|t\| ...)` (RS)<br>`.trap(\|t\| ...)` (TS/Py) | `t.on_trap(...)`, `t.on_process_exit(...)`, etc. | Host & Plugin |
| `on_fs_lookup`, `on_fs_read`, `on_fs_write`, etc. | `.fs(\|f\| ...)` | `f.on_fs_lookup(...)`, `f.on_fs_write(...)`, etc. | Host & Plugin |
| `on_net_outbound`, `on_net_inbound`, `on_dns_query`, etc. | `.net(\|n\| ...)` | `n.on_net_outbound(...)`, `n.on_dns_query(...)`, etc. | Host & Plugin |
| `on_console_input`, `on_console_output` | `.console(\|c\| ...)` | `c.on_console_input(...)`, `c.on_console_output(...)` | Host & Plugin |
| `on_exec_stdout`, `on_exec_stderr`, `on_exec_stdin` | `.exec(\|e\| ...)` | `e.on_exec_stdout(...)`, `e.on_exec_stderr(...)`, etc. | Host & Plugin |
| `on_host_lsm_exec`, `on_host_pressure`, etc. | `.host(\|h\| ...)` | `h.on_host_lsm_exec(...)`, `h.on_host_pressure(...)`, etc. | Host & Plugin |
| `on_vm_init`, `on_vm_ready`, `on_pre_commit`, etc. | `.lifecycle(\|l\| ...)` | `l.on_vm_init(...)`, `l.on_pre_commit(...)`, etc. | Host & Plugin |
| `on_memory_pressure`, `on_heartbeat`, `on_watchdog` | `.telemetry(\|t\| ...)` | `t.on_memory_pressure(...)`, `t.on_watchdog(...)`, etc. | Host & Plugin |

*Note*: Filtered trap registration `.trap(TrapFilter..., handler)` (host-only) remains alongside the `.trap((t) => ...)` / `.trap_scope(|t| ...)` ring scope.

#### Flat-Fold Equivalence (`group`)
A flat chain exists solely as a `group()` fold delegating to scopes:
* **Rust**: `builder.group(|g| g.trap_scope(...).fs(...))`
* **TypeScript**: `builder.group((g) => g.trap(...).fs(...))`
* **Python**: `builder.group(lambda g: g.trap(...).fs(...))`
Descriptors produced by `group()` must match direct nested-scope calls in ring assignments and ordinals.

---

### 4. CI, AST & Negative-Test Enforcement Mechanics

1. **AST Surface Diff (`15-operations.md#Multi-language-parity-gate`)**:
   Compare Rust, TS, and Python exported surfaces against `artifacts/config/builder-verbs.yaml`:
   * Every chain verb returns the builder (`Self` / `this` / `self`), never `Result`/`Promise`/`None`/`void`.
   * Deleted standalone/orphan/flat verbs are absent from exported root builder surfaces.
2. **AST Reassignment & Chain Scanner (`just check-examples` / `scripts/check-spec-drift.py`)**:
   Parses `cans/artifacts/examples/*.{rs,ts,py}` and fails if:
   * A `KelirBuilder` binding is reassigned or mutated post-construction (`let mut builder`, `lifecycle_mut()`, statement-level mutation).
   * Any `?` operator appears on a builder method other than `.seal()`.
   * Any deleted root-level verb, `BasePlugin`, `kelir.plugins.plugin_*` alias, or unapproved `@ts-expect-error` appears in root examples.
3. **Canonical Parity Runner (`kelir-plugin-cli parity run`)**:
   Executes all 96 canonical cases (38 ring verbs + 48 scope sub-nodes + 5 chain cases + 5 predicate cases) as single-expression chains across RS/TS/Py and asserts bitwise-identical output descriptors.
4. **Executable Negative Suites (Out of Comments)**:
   Migrate comment-pinned negative vectors from `unified-builder-surface.*` into:
   * **Rust**: `crates/kelir/tests/compile_fail/` (`trybuild` asserting `E0599` on post-seal calls, scope bleeding, and host verbs on `Plugin`).
   * **TypeScript**: `packages/core/test/compile_fail/` (`// @ts-expect-error` executed via `tsc --noEmit`).
   * **Python**: `packages/py/tests/test_compile_fail.py` (asserting `pyright` diagnostics on scope bleeding and host verbs on plugin builders).
```

---

Reply **`next`** (or **`2`**) for **Part 2/3**.## PART 2: SPEC RESOLUTION, QUARANTINE MANDATE & REMEDIATION (CORE SURFACE & MATRIX EXAMPLES)

---

### 1. Architectural Contradiction Resolution: `lock_broker`

* **Contradiction**: `02-plugin-model.md#Chain-decisions (i)` historically allowed a standalone `.lock_broker(...)` host verb, whereas `#Nested-scopes` and `#Host-only-verbs` state: *"lock_broker folds into `.fs(|f| f.config(…))`; the standalone verb deletes"*. Currently, files like `unified-builder-surface.py` duplicate this by calling both `.storage(lock_broker=True)` and `.fs(...)`.
* **Canonical Decision**: **The standalone `.lock_broker(...)` verb and `.storage(lock_broker=...)` parameter are deleted.** Cross-VM lock brokering configuration belongs strictly inside the filesystem scope:
  * **Rust**: `.fs(|f| f.config(FsScopeConfig { lock_broker: Some(LockBrokerConfig { ... }), ..Default::default() }).end())`
  * **TypeScript**: `.fs((f) => f.config({ lockBroker: { globs: [...], nonBlocking: true } }))`
  * **Python**: `.fs(lambda f: f.config(lock_broker={"globs": [...], "non_blocking": True}))`

---

### 2. Directory Layout & Quarantine Mandate (Phase A)

Per `cans/15-operations.md#Examples-compile`, the CI entry point `scripts/check-examples.sh` globs `cans/artifacts/examples/*.{rs,ts,py}` **non-recursively**. Root-level examples must compile, pass strict typechecks (`tsc --noEmit`, `pyright`, `cargo check`), and boot without mocks.

Examples requiring un-landed L3 kernel drivers, external PostgreSQL databases, or complex distributed daemons must be quarantined immediately:

```
cans/artifacts/examples/
├── deferred/                          <── QUARANTINED (Ignored by non-recursive check-examples.sh)
│   ├── serverless-webhook.{rs,ts,py}
│   ├── multi-vm-swarm.{rs,ts,py}
│   └── db-backed-runtime.{rs,ts,py}
│
├── autonomous-swe-agent.{rs,ts,py}    <── RETAINED (Full-surface integration)
├── container.{rs,ts,py}               <── RETAINED (Rootless OCI)
├── fluent-builder-chaos.{rs,ts,py}    <── RETAINED (Descriptor parity reference)
├── git-gate.{rs,ts,py}                <── RETAINED (Trap + redaction gate)
├── hook-matrix-full.{rs,ts,py}        <── RETAINED (All 38 ring verbs & scopes)
├── policy-matrix.{rs,ts,py}           <── RETAINED (Unified Verdict lattice)
├── profile.{rs,ts,py}                 <── RETAINED (Declarative config ingestion)
├── replay.{rs,ts,py}                  <── RETAINED (PMU cycle-exact execution)
└── unified-builder-surface.{rs,ts,py} <── RETAINED (Canonical reference implementation)
```

Quarantined files in `deferred/` are refactored to the unbreakable chain standard (detailed in Part 3) but isolated from the primary CI gate until their driver/network prerequisites land.

---

### 3. File-by-File Remediation Matrix: Core & Matrix Suites (Phase B)

#### 1. `unified-builder-surface.{rs,ts,py}`
*Reference implementation for all 38 ring verbs, 8 scopes, and 5 orphan closures.*

* **Rust (`.rs`)**:
  * Un-comment the comment-pinned negative vectors (`negative_fs_on_trap`, `host_only_verbs_rejected_on_plugin`) and migrate them into `crates/kelir/tests/compile_fail/builder_negative.rs` using `trybuild`.
  * Ensure every scope call terminates with `.end()`.
  * Remove standalone `lock_broker` from `.storage(...)` and keep it strictly in `.fs(|f| f.config(...).end())`.
* **TypeScript (`.ts`)**:
  * Align `hostOnlyVerbsRejectedOnPlugin()`: add missing probes for `actions`, `streams`, and `deny` to achieve 1:1 probe parity with Python (totaling 9 probed verbs).
  * Wrap probe calls in `(plugin as any)` or annotate with `// @ts-expect-error` once typestate rejects host verbs on `PluginMode`.
  * Extract negative vector `negativeFsOnTrap` into `packages/core/test/compile_fail/builder.negative.ts`.
* **Python (`.py`)**:
  * Extract comment-pinned `negative_fs_on_trap` into `packages/py/tests/test_compile_fail.py` and run against `pyright`.
  * Remove redundant `.storage(lock_broker=True)`; retain solely `.fs(lambda f: f.config(lock_broker=...))`.
  * Enforce that scope closures return `Scope | None`.

#### 2. `fluent-builder-chaos.{rs,ts,py}`
*Canonical descriptor-only plugin construction and host merge.*

* **Rust (`.rs`)**:
  * `build_chaos_host()` must return an unbroken expression:
    ```rust
    pub fn build_chaos_host(config: ChaosConfig) -> Result<KelirBuilder<Host, Sealed, (), HNil, HCons<Arc<dyn FaultInjectionService>, HNil>, NoParams>, PluginError> {
        KelirBuilder::new(VmConfig::default())
            .vcpus(1)
            .memory_mb(256)
            // ...
            .plugin_instance(ChaosPlugin::new(config))
            .seal()
    }
    ```
* **TypeScript (`.ts`)**:
  * `buildChaosHost()` must execute as an unbroken chain terminating in `.seal()`.
  * `buildChaosPlugin()` returns the unsealed plugin builder directly without `.seal()` or `.boot()`.
* **Python (`.py`)**:
  * Normalize `build_chaos_host()` to return `kelir.builder(...).use(...).seal()`.

#### 3. `git-gate.{rs,ts,py}`
*Trap interposition and in-flight redaction.*

* **TypeScript (`.ts`)**:
  * **Defect**: Uses deleted flat verbs `createPlugin("git-gate").onFsWrite(...).onTrap(...)`.
  * **Remediation**: Wrap handlers inside `.fs(...)` and `.trap(...)`:
    ```typescript
    const gitGatePlugin = createPlugin("git-gate")
      .fs((f) =>
        f.onFsWrite((_path, buffer, _pid, _ctx) => {
          const masked = buffer.toString("utf8").replace(/sk_live_[0-9a-zA-Z]{24}/g, (m) => "*".repeat(m.length));
          buffer.set(Buffer.from(masked, "utf8"), 0);
          return FilterAction.Forward;
        })
      )
      .trap((t) =>
        t.onTrap(async (event, c) => {
          // ...
        })
      );
    ```
  * Host execution: chain inline as `kelir({...}).storage({...}).use(gitGatePlugin).seal().boot()`.
* **Python (`.py`)**:
  * **Defect**: Subclasses `BasePlugin` and passes deleted storage options (`mask_read`, `scan_engine`, `carryover_bytes`, `synthetic_files`) directly to `.storage(...)`.
  * **Remediation**: Eliminate `BasePlugin`. Rewrite as a scoped plugin builder:
    ```python
    def git_gate_plugin() -> KelirBuilder:
        return (
            KelirBuilder.plugin("git-gate")
            .fs(lambda f: f.on_fs_write(mask_fs_write))
            .trap(lambda t: t.on_trap(gate_git_trap))
        )
    ```
  * Strip `mask_read`, `scan_engine`, `carryover_bytes`, and `synthetic_files` from `.storage()`. Mount storage redaction via `use(redaction(...))` or `.fs(lambda f: f.rules(...))`.
* **Rust (`.rs`)**:
  * Retain `GitGatePlugin` as a typed trait struct, or rewrite via `.named("git-gate").fs(...).trap_scope(...)`.
  * Host builder must chain without intermediate bindings: `KelirBuilder::new(...).storage(...).plugin_instance(GitGatePlugin).seal()?.boot().await?`.

#### 4. `hook-matrix-full.{rs,ts,py}`
*All 38 ring verbs and TrapFilter predicate grammar.*

* **TypeScript (`.ts`)**:
  * **Defect**: Calls all 24 ring hooks flat on `createPlugin("hook-matrix-full").onNetOutbound(...).onFsSetattr(...)`.
  * **Remediation**: Map all hooks into their 8 segregated scopes (`.net()`, `.fs()`, `.console()`, `.exec()`, `.host()`, `.lifecycle()`, `.telemetry()`, `.trap()`):
    ```typescript
    const hookMatrixFullPlugin = createPlugin("hook-matrix-full")
      .net((n) => n.onNetOutbound(...).onNetInbound(...))
      .fs((f) => f.onFsLookup(...).onFsRead(...).onFsWrite(...).onFsLock(...).onFsUnlink(...).onFsRename(...).onFsSetattr(...))
      .console((c) => c.onConsoleInput(...).onConsoleOutput(...))
      .exec((e) => e.onExecStdout(...).onExecStderr(...).onExecStdin(...))
      .host((h) => h.onHostLsmExec(...).onHostPressure(...).onHostKeyringAccess(...).onHostPreempt(...).onHostContainmentBreach(...))
      .lifecycle((l) => l.onVmInit(...).onVmReady(...).onVmShutdown(...).onPreCommit(...).onPostCommit(...))
      .telemetry((tm) => tm.onHeartbeat(...).onWatchdog(...).onMemoryPressure(...))
      .trap((t) => t.onTrap(...).onProcessExit(...).onSignal(...).onFork(...));
    ```
  * Host predicates: declare `TrapFilter.binary("git").withArg("push").withCwd("/workspace/**").withEnv("CI", "1").withUid(0).argvMatches("^--force$")` directly in `.trap(filter, handler)`.
* **Python (`.py`)**:
  * **Defect**: Subclasses `BasePlugin`. Uses global mutable state and loose constants (`IDENTITY_PATH`, `DENY_MARKER`).
  * **Remediation**: Delete `BasePlugin`. Rewrite using `KelirBuilder.plugin("hook-matrix-full")` with the 8 scope closures.
  * Move identity file injection and path traversal guards into `.fs(lambda f: f.rules(...))` and synthetic FUSE scopes.
  * Align the `pre=` trap handler signature to `(ctx)` across TS and Python.
* **Rust (`.rs`)**:
  * Maintain `HookMatrixFullPlugin` trait implementation for deep trait-level parity, or convert to `.group(|g| ...)`.
  * Chain host setup in one pass:
    ```rust
    let mut session = KelirBuilder::new(VmConfig { ... })
        .plugin_instance(plugin)
        .trap(TrapFilter::binary("git").with_arg("push")..., |_, _| { ... })
        .deny(|d| d.on_trap_deny(|event, verdict, session| Box::pin(async move { Ok(()) })).end())
        .seal()?
        .boot()
        .await?;
    ```

#### 5. `policy-matrix.{rs,ts,py}`
*Exercises the full Verdict lattice, watchdog suppression/mitigation, vsock, and custom actions.*

* **TypeScript (`.ts`)**:
  * **Defect**: Calls deleted root verbs `.registerCustomAction(...)` and `.vsockService(...)`, and attaches flat `.onWatchdog(...)` to `watchdogSilencer`.
  * **Remediation**:
    * Move custom action to `.actions((a) => a.register("quarantine", schema, handler))`.
    * Move vsock service to `.vsock((v) => v.service(0x0013, "echo", handler))`.
    * Wrap `watchdogSilencer` in `.telemetry((t) => t.onWatchdog(...))`.
    * Wrap `policyPlugin` in `.trap((t) => t.onTrap(...))` and `.lifecycle((l) => l.onVmInit(...).onVmReady(...))`.
    * Chain host without mutable assignment: `const session = await kelir({...}).use(policyPlugin).use(watchdogSilencer).actions(...).vsock(...).trap(...).seal().boot();`.
* **Python (`.py`)**:
  * **Defect**: Subclasses `BasePlugin` and calls deleted `.register_custom_action(...)` and `.vsock_service(...)`.
  * **Remediation**:
    * Delete `BasePlugin`. Implement `policy_matrix_plugin()` and `watchdog_silencer_plugin()` via `KelirBuilder.plugin(...)`.
    * Chain orphan verbs through `.actions(lambda a: a.register(...))` and `.vsock(lambda v: v.service(...))`.
    * Move watchdog handling into `.telemetry(lambda t: t.on_watchdog(...))`.
    * Eliminate root `.vsock_service` and `.register_custom_action`.
* **Rust (`.rs`)**:
  * Chain builder directly:
    ```rust
    let (builder, shared) = {
        let policy = PolicyMatrixPlugin::new();
        let shared = Arc::clone(&policy.state);
        let b = KelirBuilder::new(VmConfig { ... })
            .plugin_instance(policy)
            .plugin_instance(WatchdogSilencerPlugin::new(Arc::clone(&shared)))
            .actions(|a| a.register("quarantine", ...).end())
            .vsock(|v| v.service(..., EchoVsock).end());
        (b, shared)
    };
    let mut session = builder.seal()?.boot().await?;
    ```

---

Reply **`next`** (or **`3`**) for **Part 3/3** (SWE Agent, Minimal Examples, Quarantined Remediation, and CI Verification Gates).

## PART 3: REMEDIATION (SWE-AGENT, MINIMAL & DEFERRED SUITES) & VERIFICATION GATES

---

### 1. File-by-File Remediation: Retained Complex & Minimal Examples

#### 1. `autonomous-swe-agent.{rs,ts,py}`
*Full-surface integration demonstrating branch forks, storage reconciliation, and hardware receipts.*

* **Rust (`.rs`)**:
  * **Unbroken Chain**: `build_runtime()` must return the unsealed builder directly. Combine host config, storage, terminal, wire, plugins, scopes, traps, lifecycle, and deny into a single chained expression.
  * **SSOT-A1 Winner Selection Defect**:
    * *Defect*: Lines 626–633 take the first completing branch from `joins.join_next()`, causing nondeterministic selection under varying branch runtimes.
    * *Fix*: Drain all joined tasks into a collection. Select the winner by scanning candidates in **declared branch order** (`['A', 'B', 'C', 'D']`):
      ```rust
      let mut results = Vec::new();
      while let Some(joined) = joins.join_next().await {
          results.push(joined??);
      }
      let (winning_id, winning_delta) = ['A', 'B', 'C', 'D']
          .into_iter()
          .find_map(|id| results.iter().find(|(b_id, res)| *b_id == id && res.is_ok()))
          .map(|(id, res)| (*id, res.as_ref().unwrap().clone()))
          .ok_or("no hypothesis succeeded")?;
      ```
  * **Scope Convergence**: Fold `.storage(lock_broker=true)` into `.fs(|f| f.config(...).end())`.

* **TypeScript (`.ts`)**:
  * **SSOT-A1 Git Push Argv Defect**:
    * *Defect*: Line 155 rebuilds `argv` from scratch, dropping user flags and remotes.
    * *Fix*: Preserve original arguments and append the target refspec:
      ```typescript
      return c.patch({
        binary: "/bin/git",
        argv: [...c.trap.argv, "HEAD:refs/heads/agent/demo-branch"],
        env: { KELIR_REWRITTEN: "1" },
      });
      ```
  * **Orphan Verb Purge**:
    * Wrap `repoPolicyPlugin` hooks inside `.fs(...)` and `.trap(...)`.
    * Move top-level `.params(...)` into `.params((p) => p.model(...))`.
    * Move lifecycle hooks (`.on(LifecycleEvent.VmInit, ...)`) into `.lifecycle((l) => l.onVmInit(...).onVmReady(...).onVmShutdown(...))`.
    * Move `.onTrapDeny(...)` into `.deny((d) => d.onTrapDeny(...))`.
  * **Chain Unification**: Chain builder setup through `.seal().boot()` without intermediate mutable assignments.

* **Python (`.py`)**:
  * **BasePlugin Elimination**: Delete `class RepoPolicyPlugin(BasePlugin)`. Replace with:
    ```python
    def repo_policy_plugin(opts: RepoPolicyOptions) -> KelirBuilder:
        return (
            KelirBuilder.plugin("repo-policy")
            .require("ledger")
            .fs(lambda f: f.on_fs_write(handle_write).on_fs_unlink(handle_unlink))
            .trap(lambda t: t.on_trap(handle_trap))
        )
    ```
  * **Critical Runtime Bug Fix**:
    * *Defect*: Winner lookup attempts to access `b.id` and `b.exit_code` on a 3-tuple `(hypothesis_id, run, delta)`, raising `AttributeError`.
    * *Fix*: Unpack and evaluate the tuple properly in declared order:
      ```python
      branches = await asyncio.gather(*(fork_and_verify(session, hid) for hid in "ABCD"))
      winner = next((b for target in "ABCD" for b in branches if b[0] == target and b[1].exit_code == 0), None)
      if winner is None:
          await session.vm.rollback(base)
          return
      winning_id, test_run, winning_delta = winner
      ```
  * **Orphan Verb Purge**:
    * Move `.on_vm_init`, `.on_vm_ready`, `.on_vm_shutdown` into `.lifecycle(...)`.
    * Move `.on_trap_deny` into `.deny(...)` and correct the handler signature to 3 arguments: `(event, verdict, session)`.
    * Move `lock_broker=True` from `.storage()` into `.fs(lambda f: f.config(lock_broker={...}))`.
    * Change `builder = await build_runtime(...)` and `async with builder as session:` to chain `.seal()` inside context entry or return an unsealed builder.

#### 2. `container.{rs,ts,py}`
*Rootless OCI container execution.*

* **Rust (`.rs`)**:
  * Eliminate `container_builder()` intermediate assignment.
  * Direct execution:
    ```rust
    let mut session = KelirBuilder::new(VmConfig { ... })
        .plugin_instance(ContainerPlugin::new(...))
        .seal()?
        .boot()
        .await?;
    ```
* **TypeScript (`.ts`)**:
  * Replace `const runtime = kelir({...}).use(...); runtime.seal(); const session = await runtime.boot();` with:
    ```typescript
    const session = await kelir({ ... })
      .use(container({ ... }))
      .seal()
      .boot();
    ```
* **Python (`.py`)**:
  * Delete local assignment inside `build_runtime()`. Chain:
    ```python
    async with kelir.builder( ... ).use(ContainerPlugin( ... )).seal() as session:
    ```

#### 3. `profile.{rs,ts,py}`
*Declarative TOML/YAML profile ingestion.*

* **Rust (`.rs`)**:
  * Ensure `KelirBuilder::from_config("./fixtures/kelir.toml")?.seal()?.boot().await?` executes in an unbroken pipeline.
* **TypeScript (`.ts`)**:
  * Replace multi-step `const builder = ...; builder.seal(); const session = ...;` with:
    ```typescript
    const session = await (await KelirBuilder.fromConfig("./fixtures/kelir.toml")).seal().boot();
    ```
* **Python (`.py`)**:
  * Replace statement mutation with:
    ```python
    async with KelirBuilder.from_config("./fixtures/kelir.toml").seal() as session:
    ```

#### 4. `replay.{rs,ts,py}`
*Deterministic PMU instruction-stepped replay.*

* **All Languages**:
  * Assert mandatory isolation: `deterministic_clock: true`, `pinned_clocksource: "kvm-clock"`, `network_off: true`.
  * Eliminate multi-statement configuration; terminate chain directly in `.seal()?.boot()` / `.seal().boot()`.

---

### 2. Remediation for Quarantined Files (`cans/artifacts/examples/deferred/`)

These examples remain in `deferred/` until driver and mock dependencies land. They must conform to the unbreakable chain standard:

#### 1. `multi-vm-swarm.{rs,ts,py}`
* **Rust (`.rs`)**:
  * **Critical Break-Chain Fix**:
    * *Defect*: Calls `builder.lifecycle_mut().register_veto(...)` on a mutable local variable.
    * *Fix*: Delete `lifecycle_mut()`. Pass the veto closure directly into `.lifecycle(...)`:
      ```rust
      let session = swarm_builder(veto_flag)
          .lifecycle(|l| {
              l.on_vm_init(move |ctx| {
                  ctx.register_veto(VetoEvent::Fork, Box::new(move || Veto::allowed(flag.load(Ordering::SeqCst))));
                  Ok(())
              })
              .end()
          })
          .seal()?
          .boot()
          .await?;
      ```
  * Replace root `.lock_broker(...)` with `.fs(|f| f.config(FsScopeConfig { lock_broker: Some(...), ..Default::default() }).end())`.
* **TypeScript (`.ts`)**:
  * Move `.on(LifecycleEvent.VmInit, ...)` into `.lifecycle((l) => l.onVmInit(...))`.
  * Move `lockBroker` configuration into `.fs((f) => f.config({ lockBroker: { ... } }))`.
* **Python (`.py`)**:
  * Move `lock_broker(...)` from builder root into `.fs(lambda f: f.config(lock_broker={...}))`.
  * Move `.on_vm_init(_init)` into `.lifecycle(lambda l: l.on_vm_init(_init))`.

#### 2. `db-backed-runtime.{rs,ts,py}`
* **Rust (`.rs`)**:
  * Register `MigrateParams` via `.params(|p| p.model::<MigrateParams>().end())` to ensure parity with TS/Py.
* **TypeScript (`.ts`)**:
  * Move root `.params(...)` into `.params((p) => p.model(...))`.
  * Move `.onPreCommit(...)` and `.onPostCommit(...)` into `.lifecycle((l) => l.onPreCommit(...).onPostCommit(...))`.
  * Ensure `ledger.finalize(...)` enforces non-null `includeDelta`:
    ```typescript
    const delta = await ledgerHandle.getCausalDeltaFromParent();
    const final = await ledgerHandle.finalize({
      includeDelta: delta,
      includeMerkleRoot: true,
    });
    ```
* **Python (`.py`)**:
  * Move `.on_pre_commit` and `.on_post_commit` into `.lifecycle(lambda l: l.on_pre_commit(...).on_post_commit(...))`.
  * Register parameters via `.params(lambda p: p.model(MigrateParams))`.
  * Guard `session.cap("transaction")` against `None` before acquiring scope.

#### 3. `serverless-webhook.{rs,ts,py}`
* **Rust (`.rs`)**:
  * Replace string traps with typed filters: `.trap_async(TrapFilter::binary("webhook-worker"), ...)`.
  * Register parameters via `.params(|p| p.model::<DispatchWebhookParams>().end())`.
  * Chain `.wire(...)` and plugins inline.
* **TypeScript (`.ts`)**:
  * Move root `.params(...)` into `.params((p) => p.model(...))`.
  * Standardize `.wire(...)` to use the typed configuration object matching `autonomous-swe-agent.ts`.
  * Move `.on(LifecycleEvent.VmReady, ...)` and `VmShutdown` into `.lifecycle(...)`.
* **Python (`.py`)**:
  * Eliminate deleted `.wire_config(_wire)`; configure wire networking directly via `.wire(WireConfig(...))`.
  * Move `.params(DispatchWebhook)` into `.params(lambda p: p.model(DispatchWebhook))`.
  * Replace unstable imports (`from kelir.plugins.plugin_mcp import ...`) with canonical package paths (`from kelir.plugins.mcp import ...`).

---

### 3. Verification & CI Gate Enforcement (Phase C)

Execute the three-stage verification pipeline to ensure zero drift:

```bash
# ==============================================================================
# STAGE 1: ABSOLUTE STATIC COMPILATION & PARITY GATES
# ==============================================================================

# Rust: Enforce zero compiler warnings and typestate adherence across all retained examples
cargo check --examples --all-targets

# TypeScript: Enforce strict typechecking without emit across packages and examples
pnpm --filter @kelir/core build
pnpm --filter @kelir/examples exec tsc --noEmit --strict

# Python: Enforce static typing and protocol conformity across all retained examples
pyright cans/artifacts/examples/*.py

# Negative Suites: Verify compile-fail invariants (scope bleeding, post-seal mutation)
cargo test --test compile_fail
pnpm --filter @kelir/core test:compile-fail
pytest packages/py/tests/test_compile_fail.py

# ==============================================================================
# STAGE 2: UNBROKEN CHAIN & ORPHAN-VERB AST SCANNER
# ==============================================================================

# Runs AST scanner verifying:
# 1. Zero variable reassignment on KelirBuilder
# 2. Zero mid-chain fallibility (? before seal)
# 3. Zero deleted orphan verbs outside scope closures
# 4. Zero BasePlugin subclassing in Python
# 5. Non-recursive scan ignores cans/artifacts/examples/deferred/
python3 scripts/check-builder-chain-ast.py cans/artifacts/examples/

# Verify 96 canonical parity cases match bitwise descriptors across RS, TS, and Py
kelir-plugin-cli parity run --vectors vectors.json

# ==============================================================================
# STAGE 3: EXECUTION & HYGIENE VERIFICATION
# ==============================================================================

# Execute all retained examples in root directory; fails on panic, error, or drift
just check-examples-run

# Enforce zero specification drift and hygiene compliance
just check-spec-drift
just check-hygiene
```

Any example that breaks the fluent chain, reassigns builder references, invokes deleted orphan methods, or introduces cross-SDK divergence fails the pipeline at Stage 1.

