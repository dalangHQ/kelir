# work-order.md — appended per-agent codebase edits

**Purpose of this addendum.** The original work-order body (see above) lands the design fixes at the `cans/` markdown layer. This addendum translates each agent's markdown edits into the concrete  file changes required to keep the crate tree, SDKs, examples, and parity vectors compiling and passing their gates. It follows the same ownership, override, and gate rules as the parent work-order. Every item here is a codebase edit; the corresponding spec/artifact edit is already described in the parent section.

The addendum uses the paths exactly as they appear in `repomix.md`.

---

## Agent 1 — Action Algebra: appended codebase edits

### New artifacts

- `cans/artifacts/code/action-algebra.rs`, `.ts`, `.py` — canonical `Action` + closed `Errno` surfaces.

### `crates/kelir-interface/src/protocol_wire.flow.rs`

- Introduce `pub enum Action` with `#[non_exhaustive]` and variants `Forward`, `Pass`, `Drop { errno: Errno, reason: String }`, `Suspend { until: SuspendUntil }`, `Abort { reason: String }`, `Patch(PatchPayload)`.
- Replace `pub enum Verdict` and `pub enum FilterAction` with `pub type Verdict = Action;` and `pub type FilterAction = Action;`. The `HostToGuest::TrapDecision { action: Verdict }` wire shape stays byte-compatible because `Action` is the union of the current two enums.
- Change `Drop { errno: Option<Errno>, ... }` → `Drop { errno: Errno, ... }` on the aliases (drop the `Option`). Update every `Verdict::Drop { errno: Some(..) }` / `FilterAction::Drop { errno: Some(..) }` / `..None..` literal in the crate tree; the `None` form (syscall-free link-local drop in `wire/plane.flow.rs`) maps to `Errno::Eperm` with a caller-supplied reason.
- `ExecveMutation`, `PatchPayload`, `SuspendUntil`, `PostExecAction`, `StepVerdict` unchanged.

### `crates/kelir-interface/src/errno_types.flow.rs`

- Add variant `PathTraversal`. `as_str` returns `"PathTraversal"`; `FromStr` accepts `"PathTraversal"`; serde rename keeps the variant name.
- Add `pub fn to_os_errno(&self) -> i32` mapping every variant (PathTraversal → `libc::EPERM`, the others to their libc constants). The driver uses this at the syscall boundary, not the plugin.
- Update `from_raw_os_error` to leave `PathTraversal` unreachable (it's a semantic, not OS, code).

### `crates/kelir-interface/src/plugin_model.flow.rs`

- Delete variant `PluginError::Reject { errno, reason }`.
- Delete `PluginError::reject(errno, reason)`.
- Delete the `Reject` arm from `impl PartialEq for PluginError`.
- Keep `FfiQueueSaturated`, `WxViolation`, `AmbiguousMutation`, `Io`, `Halt`. That is the `PluginError-scope pin` set.

### `crates/kelir-interface/src/handlers_plugin.flow.rs`

- Delete `forward`, `filter_drop`, `filter_suspend`. `pass`, `patch`, `drop`, `suspend`, `abort` all return `Action`.
- Change `drop(errno: Option<Errno>, ...)` → `drop(errno: Errno, ...)`.

### `crates/kelir-interface/src/error_envelope.flow.rs`

- Delete the `PluginError::Reject { errno, reason }` match arm.
- To preserve structured `EConfig` without a new `PluginError` variant, add a small helper `pub fn config_envelope(message: impl Into<String>) -> PluginError` that returns `PluginError::Halt` with a `EConfig: ` prefix; `From<PluginError> for ErrorEnvelope` detects the prefix and sets `code: ECONFIG_CODE`. Call sites in `plugin-profile`, `plugin-transaction`, `plugin-container`, `plugin-mesh` reuse this helper instead of `PluginError::reject(Errno::Econfig, …)`.

### `crates/kelir-engine/src/dispatch_plugin/quarantine.flow.rs`

- Delete `map_reject_to_verdict` and `map_reject_to_filter_action`. Callers do `outcome?` directly.
- The panic fallbacks in `quarantine_sync` already return `Ok(Verdict::Drop { errno: Some(Errno::Efault), .. })`; after the alias they must pass `Efault` (non-optional).

### Reject call-site conversions (per the pin: hook-time deny → `Action::Drop`; pre-flight deny → `Halt` carrying `EConfig: ` prefix)

- `crates/kelir-engine/src/planes/storage/storage_quota.flow.rs::accrue_quota` → `Halt("ENOSPC: …")` (quota is a pre-syscall check, not a hook verdict).
- `crates/kelir-plugins/plugin-profile/src/patch.flow.rs::validate` → `Halt` with `EConfig: ` prefix.
- `crates/kelir-plugins/plugin-mesh/src/mesh_state.flow.rs::assert_peer_identity` → `Halt("ECONNREFUSED: …")`.
- `crates/kelir-plugins/plugin-vault/src/metadata_endpoint.flow.rs::halt` → `Halt(format!("{}: {}", err.errno(), err.reason()))`.
- `crates/kelir-plugins/plugin-container/src/lib.rs` — keep `reject_eperm()` as `Verdict::Drop { errno: Some(Errno::Eperm), .. }`; convert the two `PluginError::reject(...)` call sites (`Esecurityviolation`, `Enoent`) → `Halt`.
- `crates/kelir-plugins/plugin-db-proxy/src/service_db_proxy.flow.rs::lookup_statement` → `Halt("ENOENT: …")`.
- `crates/kelir-plugins/plugin-db-proxy/src/lib.rs::DbSqliteStreamSink::on_stream_frame` → `Halt`. (The trait returns `Result<(), PluginError>`; verdict signalling for stream frames arrives with the L3 dispatch surface.)
- `crates/kelir-plugins/plugin-tool-guard/src/tool_guard_state.flow.rs::on_console_input` → `Halt`. Note: `ConsoleInterposer::on_console_input` returns `Result<Vec<u8>, PluginError>`, so a deny cannot be `Action::Drop` here until the trait signature gains a verdict return; file a `HANDOFF` under the pin. Same for the `Abort`/`Suspend` arms.
- `crates/kelir-plugins/plugin-transaction/src/coordination.flow.rs` and `service_transaction.flow.rs` → `Halt` with the errno as a message prefix.
- `crates/kelir-plugins/plugin-wire-ingress/src/ingress_state.flow.rs::park` → `Halt("EPROTO: …")`.

### Python

- `packages/py/kelir/model_plugin.py` — delete `PluginError.reject` classmethod; add `PluginError.config(message)` that produces an `EConfig` envelope. `plugin_error_to_verdict` / `plugin_error_to_filter_action` return `Verdict.drop(err.errno, err.reason)`.
- `packages/py/kelir/__init__.py::Verdict::pass_` and `HandlerContext::pass_` — **note:** the parent work-order says “delete the trailing underscore (Python sugar: `c.pass()`)”, but `pass` is a Python keyword. Keep `pass_()`; mark the parent instruction as a defect in the spec edit commit. The TS SDK keeps `c.pass()`.
- `packages/py/kelir/__init__.py::Verdict::drop` — accept `errno: str` still, but map it through a new `Errno` enum in `kelir/__init__.py` (the closed set). `E_PATH_TRAVERSAL` is not accepted as a string; use `Errno.PathTraversal`.
- `packages/py/kelir/model_plugin.py::BasePlugin::on_fs_lookup` → `Verdict.pass_()` (keep), and every `FilterAction.forward()` → `Verdict.forward()` after the alias unification in Python (add `Verdict.forward`).

### TypeScript

- `packages/ts/core/src/verdict.protocol.ts` — add `Verdict.Forward` and make `FilterAction` a type alias of `Verdict`; drop the `FilterAction` namespace except for the forward alias. Update `packages/ts/core/src/hook.registry.ts` where `FilterAction` is imported (it imports `FilterAction` from `verdict.protocol.js`; the change is type-only, so no code motion).
- `packages/ts/core/src/trap.protocol.ts::TrapFilter` — keep; no reject impact.

### Vectors and parity

- `crates/kelir-plugin-testkit/src/vectors_testkit.flow.rs` — `Verdict::Drop { errno: Some(Errno::Eperm), … }` → `Verdict::Drop { errno: Errno::Eperm, … }`. Same for `Verdict::Abort`, `Verdict::Suspend`. The `FilterAction::Drop { errno: Some(..) }` in `exec_rewrite_case` becomes `FilterAction::Drop { errno: Eperm, .. }` (or, after the alias, `Verdict::Drop`).
- `crates/kelir-plugin-testkit/src/vectors_chain_testkit.rs::chaos_plugin_builder_case` already produces `FilterAction::Forward` — fine.
- `crates/kelir-plugin-testkit/src/vectors_parse.flow.rs::outcome_from_json` — deserializes `Verdict` / `FilterAction` via serde; after the alias, both deserialize to `Action`. Confirm with `cargo test -p kelir-plugin-testkit vectors` and update the JSON literal fixtures accordingly. `vectors_parse.flow.rs` currently passes `--refs-only` on `cans check`; the CI gate is `--action-coverage` per `15-operations.md#Unified-builder-parity-vectors`. Add `just check-action-coverage` to `shared validation` (already in parent).

### SDK example compile targets (spec-side but must build against the new surface)

- `cans/artifacts/examples/hook-matrix-full.py` — remove `E_PATH_TRAVERSAL` string literal; return `Verdict.drop(Errno.PathTraversal, "…")`.
- `cans/artifacts/examples/git-gate.ts`, `hook-matrix.ts` — replace `throw new Error("EPERM: …")` with `Verdict.drop(Errno.Eperm, "…")` (the closed enum).
- `cans/artifacts/examples/policy-matrix.{rs,ts,py}` — compile against `Action` via the alias.

### HANDOFF required

- One `_collab/handoffs.md` entry per `ConsoleInterposer::on_console_input`-style trait that still needs the verdict surface to be `Action` (tool-guard, mcp, db-proxy `on_stream_frame`); these cannot express `Action::Drop` until the trait signatures change. Land them in a follow-up with the `PipelineBinder` change.

---

## Agent 2 — Session & Context Accessors: appended codebase edits

### New artifacts

- `cans/artifacts/code/session-accessors.rs`, `.ts`, `.py`.

### `crates/kelir/src/session_lifecycle.flow.rs`

- Add `pub fn plane<P: Plane>(&self) -> &P` and `pub fn plane_named(&self, name: &str) -> Option<&dyn Any>` (the latter for FFI). Introduce `pub trait Plane` implemented by `TerminalPlane`, `StoragePlane`, `WirePlane`, `CheckpointPlane`, `HostPlane`, `VmHandle`.
- Keep `pub vm: VmHandle`, `pub vsock: VsockHandle` as **sugar** documented as `plane("vm")` / `plane("vsock")`.
- Replace `pub fn ctx<T, Index>(&self) -> &T where Plugins: Has<T, Index>` with `pub fn cap<P: 'static>(&self) -> Option<&P> where Plugins: Has<P>` (name-based lookup; `Has` gains a defaulted `Index` = `()`). Delete the `Has<T, Index>` two-parameter form. Same accessor shape on `Session<HNil, NoParams>` returns `None`.
- `Session::vm(&self)` stays as sugar (already exists).
- `Session::vsock(&self)` stays as sugar.

### `crates/kelir/src/hlist_sdk.flow.rs`

- Keep `HCons`, `HNil`, `Here`, `There`, `Lookup`. Change `Has<T, Index>` impl to match the new `Has<P>` single-parameter form. The `Satisfies<Other>` trait stays as-is; the ordering issue it documents is unchanged.

### `crates/kelir-interface/src/handlers_plugin.flow.rs`

- Delete `pub fn ctx<T, Index>(&self) -> &T where Plugins: Has<T, Index>`.
- Add `pub fn cap<P: 'static>(&self) -> Option<&P> where Plugins: Has<P>`.
- `HandlerCtx::plugins()` stays.

### `packages/py/kelir/lifecycle_session.py`

- `Session.cap` stays as `Caps`, but add `def cap_for(self, name: str) -> Any | None:` (documented as `session.cap("ledger")` returning `Option`). Keep the `cap` attribute as sugar.
- `Session.vm` stays; add `def plane(self, name: str) -> Any | None:` (sugar for `getattr(self, name)`).
- Delete `Session._native.ctx` usage (there isn’t one directly, but `packages/py/kelir/kelir_builder.py::boot` currently calls `get_plugin_context`; keep, but add `session.cap_for(name)` sugar).

### `packages/py/kelir/__init__.py`

- `HandlerContext.cap: Caps` stays; add `def cap_for(self, name: str) -> Any | None:` — `c.cap("repo-policy")` sugar.
- Delete any `c.ctx<>`-style accessor if present. (None in the current file, but the TS `HandlerContext` type must drop `ctx`.)

### `packages/ts/core/src/session.lifecycle.ts`

- Rename `ctx<T>(plugin: string): T` → `cap<T>(plugin: string): T | undefined`. The boot loop still uses `pluginContexts`; update `this.native.pluginContexts?.find(...)` to feed `cap`.
- Add `plane<T>(name: string): T | undefined` as a thin wrapper over the native plane getters (`this.vm`, `this.terminal`, `this.storage`, `this.wire`, `this.checkpoint`, `this.host`, `this.vsock`, `this.step`).
- Property getters (`this.vm`, `this.terminal`, etc.) remain as documented sugar.

### `packages/ts/core/src/context.lifecycle.types.ts`

- `HandlerContext<Ctx, Params>` currently extends `TrapContext & Ctx & { params }`. Add `cap<T>(name: string): T | undefined` and keep the `Ctx` intersection. Update `TrapContext` if it has a `ctx` field (it does not).

### `packages/ts/core/src/builder.lifecycle.ts`

- `ctx<T>(plugin: string): T` → `cap<T>(plugin: string): T | undefined`. Update `Session & Ctx` return sites; `Ctx` becomes the *intersection* of mounted plugin contexts as before, but the accessor is now `cap`.
- Add `requires` docs table entry; existing `requires<Req>()` works.
- `plugin_instance` / `.use` / `.plugin` — the current builder has `use()` and `plugin()` (delegates to `use`). Add `pluginInstance()` alias (documented Rust-only sugar).

### Naming sweep (mechanical, do not litigate)

- Rename `packages/py/kelir/model_plugin.py` → `packages/py/kelir/plugin_model.py`. Update imports in: `packages/py/kelir/__init__.py`, `kelir_builder.py`, `plugin_fusion.py`, `lifecycle_session.py`, `lifecycle_sandbox.py`, and every `packages/py/kelir/plugins/plugin_*.py`.
- `Session::terminate` (hard kill) vs `Session::stop` / `shutdown` (graceful). Keep both; document `terminate` primary, `stop`/`shutdown` sugar. No code motion.
- `plugin_instance` / `.use` / `.plugin` — already documented as three forms; add the doc table to `crates/kelir/src/builder_sdk.flow.rs` docstring.
- `Verdict.pass_()` — keep in Python (keyword), already covered under Agent 1.
- `kelir.plugins.plugin_<name>` — the public path becomes `kelir.plugins.<name>`. In `packages/py/kelir/plugins/__init__.py`, add `from . import plugin_chaos as chaos` etc. for every plugin; keep the `plugin_<name>` import path as an *unstable* alias (documented in the module docstring).

### Plugin deps as derived

- **Rust:** `crates/kelir-interface/src/plugin_model.flow.rs::PluginDeps` stays, but add `#[derive(PluginDeps)]` proc macro. New crate `crates/kelir-interface-macros` (path dep of `kelir-interface`) provides the derive. The derive scans `require::<T>()` calls in `register_interceptors` and emits `type Deps = HCons<T::Context, …>`. Convert every `impl PluginDeps for *Plugin { type Deps = NoDeps; }` (chaos, mesh, vault, wire-ingress, supply-chain, tool-guard, mcp, profile, replay, container, db-proxy, ledger) and the one hand-written `TransactionPlugin` impl to the derive. This is a large but mechanical change; land per-plugin on the `<agent>-pins` branch.
- **TS:** `requires<Req>()` already populates `Deps`. No change.
- **Python:** `BasePlugin.requires: ClassVar[tuple[str, ...]]` already exists. No change. Document the unification in `cans/03-sdk-architecture.md#Unified-builder-surface`.

### Cross-language service lookup parity

- `crates/kelir-plugin-testkit/src/vectors_testkit.flow.rs` — `plugin_builder_case` accepts a `requires: Vec<String>` parameter; leave the plumbing, but confirm the vector readers (`vectors_parse.flow.rs`) still accept it. `parity_cli` (spec-side) will re-run.

---

## Agent 3 — Nesting & Scopes: appended codebase edits

### New artifacts

- `cans/artifacts/code/scope-builders.rs` — canonical Rust scope types (extends `TrapScope`, `FsScope`, etc. with `.config`, `.requires`, `.fork_with`, `.timeout`, `.priority`, `.rules`).
- `cans/artifacts/code/python-scope-protocol.py`, `typescript-scope-interface.ts`.

### `crates/kelir/src/builder_scopes.flow.rs`

- Keep the eight scope structs and their `.end()` methods (Rust type-system requirement).
- Add sub-nodes:
  - `.config(cfg)` on `FsScope` (storage config: `lock_broker`, `carryover_bytes`, `scan_engine`, `snoops`).
  - `.requires::<P: KelirPlugin>()` on every scope; accumulates into `Deps` (via the derive).
  - `.fork_with(f)` on `TrapScope`, `LifecycleScope` (scope-level fork).
  - `.timeout(Duration)` and `.priority(i32)` on every scope (consumed by `PipelineBinder`; Agent 5 lands the semantics).
  - `.rules(|r| r.read.allow(glob).deny(glob).write.mask(regex))` on `FsScope` — a nested `RuleScope` with `.read.allow/deny`, `.write.mask`. This replaces the raw `IDENTITY_PATH` / `DENY_MARKER` / `CARRYOVER` constants in `cans/artifacts/examples/hook-matrix-full.{rs,ts,py}`.
- New scope structs for the orphan verbs:
  - `ActionsScope` — `.register(name, schema, handler)`.
  - `StreamsScope` — `.register(kind, schema, handler)`.
  - `VsockScope` — `.service(kind, name, handler)`.
  - `ParamsScope` — `.model::<T>()`.
  - `DenyScope` — `.on_trap_deny(|event, verdict, session| …)`.

### `crates/kelir/src/builder_plugin.flow.rs`

- Delete every flat `on_*` method: `on_trap`, `on_process_exit`, `on_fs_lookup`, `on_fs_read`, `on_fs_write`, `on_fs_lock`, `on_net_outbound`, `on_net_inbound`, `on_console_input`, `on_console_output`, `on_exec_stdout`, `on_exec_stderr`, `on_host_lsm_exec`, `on_host_pressure`, `on_host_keyring_access`, `on_vm_init`, `on_vm_ready`, `on_vm_shutdown`, `on_pre_commit`, `on_post_commit`, `on_host_preempt`, `on_memory_pressure`, `on_heartbeat`, `on_watchdog`, `on_trap_deny`, `register_custom_action`, `register_stream_kind`, `vsock_service`, `params`, `fork_with`. All must move into scopes.
- Keep `trap(filter, f)`, `trap_scope(closure)`, `group`, `guard`, `use`, `plugin`, `plugin_instance`, `context`, `require`, `scope`, `detached`, `lock_broker`, `storage`, `wire`, `terminal`, `fast_boot`, `dax_window`, `build_registry`, `seal`/`boot`.
- `trap_async` and `post_exec` remain on the sealed host builder only (they already are).

### `crates/kelir/src/builder_sdk.flow.rs`

- `TrapFilter` lives here; extend per Agent 5.
- `plugin_instance` stays (trait path). `plugin` (builder path) and `use` stay. Document the three in a single table.
- `KelirBuilder::new(config: VmConfig)` stays; typestate setters `.vcpus(4)`, `.memory_mb(4096)`, `.kernel(...)`, `.rootfs(...)`, `.tsc_locked(bool)`, `.watchdog(bool)`, `.driver(DriverPolicy)` per Agent 4.

### `crates/kelir-engine/src/dispatch_plugin/engine.flow.rs::seal`

- Land `pm_03_core_stream_kind_reregistration_abort`: freeze the five ring chains and abort re-registration of core stream kinds with `EKindConflict`. The `PIPELINE-ENGINE::seal` TODO currently documents this.

### `packages/py/kelir/kelir_builder.py`

- Delete every flat method in `KelirBuilder`: `on_trap`, `on_process_exit`, `on_fs_lookup`, … , `on_watchdog`, `on_trap_deny`. They are currently invoked through `_on(verb, handler)`; move the underlying `_on` into the scope objects only.
- Delete `.end()` from `_Scope` and its subclasses (the subclasses are `_Scope` itself, parameterized by verb list). `_scope_entry` calls `fn(_Scope(...))`; the scope never exposes `.end()`.
- Add new scopes: `_ActionsScope`, `_StreamsScope`, `_VsockScope`, `_ParamsScope`, `_DenyScope`.
- `trap_scope`, `group`, `guard` stay.

### `packages/ts/core/src/hook.registry.ts`

- Delete `end(): Owner` from `TrapScope`, `FsScope`, `NetScope`, `ConsoleScope`, `ExecScope`, `HostScope`, `LifecycleScope`, `TelemetryScope`.
- Delete `endTarget` plumbing in the `HookRegistry.trap/fs/net/console/exec/host/lifecycle/telemetry` methods.
- Add new scope interfaces `ActionsScope`, `StreamsScope`, `VsockScope`, `ParamsScope`, `DenyScope`.
- Delete the flat `onNetOutbound`, `onNetInbound`, `onFsLookup`, …, `onWatchdog` public methods on `HookRegistry`? **No** — they stay as `HookRegistry` implementation helpers, called only from inside the scope factories. Make them private (`onFsWrite`, etc.) or prefix with `_` to make the surface scoped-only. The `KelirBuilder` public methods (`onNetOutbound`, etc.) on `builder.lifecycle.ts` are deleted in the next file.

### `packages/ts/core/src/builder.lifecycle.ts`

- Delete the flat `onNetOutbound`, `onNetInbound`, `onFsLookup`, `onFsRead`, `onFsWrite`, `onFsLock`, `onConsoleInput`, `onConsoleOutput`, `onExecStdout`, `onExecStderr`, `onHeartbeat`, `onProcessExit`, `onTrap`, `onWatchdog`, `onHostLsmExec`, `onHostPressure`, `onHostKeyringAccess`, `onVmInit`, `onVmReady`, `onVmShutdown`, `onPreCommit`, `onPostCommit`, `onHostPreempt`, `onMemoryPressure`, `onTrapDeny` public methods.
- Move their body into the corresponding scope interface’s chain (they already exist in `hook.registry.ts`).
- Add the new scope entry points: `.actions(fn)`, `.streams(fn)`, `.vsock(fn)`, `.params<T>(fn)`, `.deny(fn)`.

### Parity vectors

- `crates/kelir-plugin-testkit/src/vectors_testkit.flow.rs::plugin_builder_nested_cases` and `plugin_builder_merge_cases` already exercise the scope entry points; verify they no longer need flat verbs.
- `crates/kelir-plugin-testkit/src/vectors_chain_testkit.rs::plugin_builder_chain_cases` — the four chain names (`seal`, `trap-filter`, `trap-scope`, `lock-broker`) plus chaos exercise flat `on_trap`; after the flat form is deleted, these become `trap_scope`, and `lock_broker` becomes a `StorageConfig` chain. Update.
- `crates/kelir-plugin-testkit/src/vectors_testkit.flow.rs::plugin_builder_verb_cases` — drop the flat `process-exit` / `fs-lookup` / etc. cases and replace with nested `trap` / `fs` / `net` / `console` / `exec` / `host` / `lifecycle` / `telemetry` cases. The `plugin_builder_scope_cases` helper already builds nested vectors; use it as the template.
- The exact vector count is `TBD` per parent work-order; recompute after Agent 5’s `TrapFilter` grammar lands.

### Negative compile test vector

- Add `crates/kelir-plugin-testkit/tests/scope_compile_errors.rs` (or extend the existing `tests/` layout): `.fs(|f| f.on_trap(...))` must not compile. In Rust the type system already forbids this; the Python and TS tests live under `packages/py/kelir/tests/red/` (already excluded from `pytest` in `pyproject.toml`) and `packages/ts/core/tests/red/` (add the exclude to `tsconfig.check.json`).
- The Rust negative test is `trybuild`-based; add `trybuild` as a dev-dep.

### Acceptance

- `unified-builder-surface.{rs,ts,py}` (spec-side) have no flat chain, no `.end()` in TS/Py, all five orphan verbs scoped, and `.fs(|f| f.on_trap(…))` is a compile error in all three languages.
- `fluent-builder-chaos.{rs,ts,py}` (spec-side) use only nested scopes.
- `crates/kelir-plugin-testkit` `--unified-builder-coverage` gate description updated to the new vector count.

---

## Agent 4 — Config Defaults, TODO Cleanup, Scenario Runner: appended codebase edits

### New artifacts

- `cans/artifacts/code/vm-config-defaults.rs` — typestate setters on `KelirBuilder`.

### `crates/kelir-interface/src/driver_interface.flow.rs`

- Add `#[derive(Default)]` to `VmConfig` (already has `Default` on sub-structs `DriverPolicy`, `FastBootMode`, `DaxWindowMode` but `VmConfig` itself lacks it). Empty `kernel_path` / `rootfs_erofs_path` are valid for `Default`; `missing_boot_path()` already rejects them.
- `DriverPolicy` — add `#[derive(Default)]`. Fill `driver: None`, `require_hardware_vmm: true`, `allow_emulated_fallback: false`, `verify_hw_lockfile: false`, `wx_enforcement: WxPolicy::Strict`, `ffi_worker_threads: 1`, `ffi_max_ops_per_sec: 0` via `impl Default`.
- `FastBootMode`, `DaxWindowMode` already `Default`.
- `HwLockRecord` — add `#[derive(Default)]`? No, it carries status fields; leave.
- `check_hw_lockfile` — already handles `verify_hw_lockfile: false` short-circuit; no change.

### `crates/kelir-engine/src/planes/storage/config.flow.rs`

- `StorageConfig` already `#[derive(Default)]`. Keep.
- Add `#[derive(Default)]` to `LockBrokerWiring` (already has it), `IdmapConfig`, `MaskReadRule`, `ReadMaskConfig`, `WorkspaceEncapsulation`. All already `Default`; verify.

### `crates/kelir-interface/src/wire/net_ops.flow.rs::WireWiring`

- Already `#[derive(Default)]`. Replace `sinkhole_allowlist` / `sinkhole_drop_all` with `sinkhole: SinkholeConfig { rules: Vec<SinkholeRule> }` (Agent 5 lands the rules; Agent 4 leaves the desugaring).

### `crates/kelir/src/planes/terminal_plane.flow.rs::PtyConfig`

- Already `#[derive(Default)]`. Keep.

### Plugin configs

- Add `#[derive(Default)]` (or `impl Default`) to:
  - `plugin-replay::ReplayConfig` (Rust only; currently only Python has a default — Rust has no `Default`). Move the default `step_window = 1_000_000`, `kernel_params = ["clocksource=kvm-clock", "tsc=reliable", "nohpet"]`, `deterministic_rng_seed = 0x5EED`, `network_isolation = true` into a `Default` impl.
  - `plugin-mesh::MeshConfig` — add `Default` (empty trust domain, `mtls: false`, `InjectAndValidate`, empty listeners).
  - `plugin-tool-guard::ToolGuardConfig` — add `Default` matching Python.
  - `plugin-vault::VaultConfig` — already has `impl Default`.
  - `plugin-supply-chain::SupplyChainConfig` — has `#[derive(Default)]` on `WxJitConfig` but `SupplyChainConfig` only derives `Default` in the Python projection. Add `#[derive(Default)]` on the Rust struct.
  - `plugin-profile::ProfileConfig` — has `#[derive(Default)]`.
  - `plugin-container::ContainerConfig` — has `#[derive(Default)]` but the work order wants the explicit values (runtime `"crun"`, `isolate_var_lib_containers: true`, `allow_unsandboxed: false`, `pin_subuid_ranges: true`, `scratch_device: None`). They already are. Keep.
  - `plugin-db-proxy::DbProxyConfig` — already `#[derive(Default)]`.
  - `plugin-wire-ingress::WireIngressConfig` — already has `Default` via `impl Default`.
  - `plugin-chaos::ChaosConfig` — already `#[derive(Default)]`.
  - `plugin-transaction::TransactionConfig` — already `#[derive(Default)]` via the `Default` impl.
  - `plugin-ledger::LedgerConfig` — already `#[derive(Default)]`.
- The `default_vm_config(...)` helper in `crates/kelir-interface/src/driver_interface.flow.rs` is **deprecated** in favor of typestate setters. Keep it for one release; add `#[deprecated(note = "use KelirBuilder typestate setters")]`.

### `crates/kelir/src/builder_sdk.flow.rs` — typestate setters

- Add `impl KelirBuilder<Host, Unsealed, Ctx, Deps, Plugins, Params>`:
  - `.vcpus(u8)`, `.memory_mb(u32)`, `.kernel(impl Into<PathBuf>)`, `.rootfs(impl Into<PathBuf>)`, `.tsc_locked(bool)`, `.watchdog(bool)`, `.driver(DriverPolicy)`, `.page_size_kb(u32)`, `.deterministic_clock(bool)`, `.pinned_clocksource(impl Into<String>)`, `.preemption_grace_ms(u64)`, `.network_off(bool)`. Each mutates `self.config` in place; return `Self`.
- `KelirBuilder::new(VmConfig)` keeps taking a `VmConfig`; add `KelirBuilder::new_default()` calling `KelirBuilder::new(VmConfig::default())`.

### `crates/kelir/src/sandbox_runtime.flow.rs`

- `SandboxProfile::to_vm_config` — switch from `default_vm_config(...)` to the typestate setters (or to `VmConfig::default()` + field mutations). Drop the `#[deprecated]` helper call.

### `crates/kelir-bindings/napi/src/builder.flow.rs` and `crates/kelir-bindings/pyo3/src/builder.flow.rs`

- `NativeKelirBuilder::new` / `PyKelirBuilder::new` — keep accepting the current option structs; internally call `VmConfig::default()` and set fields (drop the `default_vm_config` call).

### Scenario runner

- Not present in `repomix.md` — the scenario runner files (`cans/artifacts/examples/scenario-runner.{py,rs,ts}`) live in `cans/artifacts/examples/` (spec-side). No codebase edit.
- `crates/kelir-testkit/src/lib.rs` exposes `#[path = "scenario_catalog.run.rs"] pub mod scenario_catalog;`. The work order says delete the scenario runner, not the scenario catalog. **Question for Agent 4 lead:** is `scenario_catalog` the same object? If yes, delete `scenario_catalog.rs` and the `pub mod` line. If not, leave it — it's the test-plan catalog, not the runner.
- `crates/kelir-testkit/src/perf_budgets.rs::Budgets::scorecard` — remove `scenario` fields from the schema and the `enforce` gate. The `scorecard.floors` rows reference scenarios; delete the rows or migrate to per-plugin matrix rows. Update `crates/kelir-testkit/src/lib.rs` `check-scenarios` binary dep if present.
- `crates/kelir-testkit/src/case_map.rs::CaseMap` — the `cases` list is fine; the `status: "scenario"` entries are unaffected. Just verify `_collab/handoffs.md` lists the scorecard gap.

### TODO triage

- Every `TODO (L3)` marker in the crate tree must become either deleted (if the enclosing function/struct is aspirational — file a `HANDOFF`), collapsed into a single `// L3-GAP: <one-line>` at the top of the file, or landed with a real implementation. Count by file:
  - `crates/kelir-interface/src/driver_interface.flow.rs` — many; group into one `// L3-GAP: hypervisor driver surface pending (see HANDOFF)` at the top of `VmmDriver` and remove the inline TODO markers, **or** keep inline if the collapse obscures the per-method gap. Recommend collapsing per-method into the method docstring as `// L3-GAP:`.
  - `crates/kelir/src/session_lifecycle.flow.rs` — `Sandbox::boot`, `Sandbox::exec`, `Sandbox::rollback`, `Sandbox::suspend`, `Sandbox::resume`, `Sandbox::terminate`, `Sandbox::stop`, `Sandbox::stop_with_timeout`, `Sandbox::export_snapshot` — collapse to one block at the top of `impl Sandbox`.
  - `crates/kelir-engine/src/session_lifecycle.flow.rs` — many `TODO (L3)` in `boot_with_engine`, `fork`, `rollback_to`, `dispatch_clock_resync`, `suspend`, `resume`, `drain`, `run_lifecycle_hook`, `shutdown`, `stop`, `stop_with_timeout`, `kill`, `terminate`, `export_snapshot`. Same treatment.
  - `crates/kelir-engine/src/dispatch_plugin/*.flow.rs` — each `TODO (L3)` becomes `// L3-GAP:` or removed. Do not touch semantics.
  - `crates/kelir-plugins/*/src/*.flow.rs` — same; each plugin gets one top-of-file `// L3-GAP:` per gap or per method.
  - `packages/py/kelir/*.py` and `packages/ts/core/src/*.ts` — same treatment. The `# TODO (L3)` comment in `packages/py/kelir/plugins/plugin_*.py` and `packages/ts/core/src/plugins.registry.ts` follows the same collapse rule.
- Every `// TODO (L3)` that survives must have a matching `_collab/handoffs.md` entry. No exceptions.

### `crates/kelir/src/builder_sdk.flow.rs` `KelirBuilder::new` deprecation note

- Keep `default_vm_config` for one release; add `#[deprecated]` and a doc comment pointing at the typestate setters.

### Scorecard

- `crates/kelir-testkit/src/perf_budgets.rs` — remove `[[scorecard.floors]]` scenario rows, or migrate to per-plugin matrix rows. The `crates/kelir-testkit/src/lib.rs::check-scenarios` binary is deleted; `scorecard` binary is deleted; `perf-budgets` stays.
- Update `crates/kelir-testkit/src/lib.rs` `[[bin]]` entries: drop `check-scenarios`, drop `scorecard`. Keep `check-manifest`, `anchors`, `check-tests`.
- Update `crates/kelir-plugin-cli` which imports `kelir-testkit`? Check `Cargo.toml` — `kelir-plugin-cli` depends on `kelir-testkit` (via `audit_cli.rs` path not shown). If `audit_cli` calls `scenario_catalog`, remove that path. If not, no change.

### Example triage

- Aspirational example files (`cans/artifacts/examples/*`) live spec-side; no codebase edit. But the codebase's `packages/py/kelir/tests/red/` and `packages/ts/core/tests/red/` directories (if present) hold the aspirational test stubs; move them under `tests/aspirational/` or delete per the parent work-order.

---

## Agent 5 — New Granularities: appended codebase edits

### New artifacts

- `cans/artifacts/code/trap-filter-predicates.rs`, `hook-trace.rs`, `sinkhole-rules.rs`.

### `crates/kelir/src/builder_sdk.flow.rs::TrapFilter`

- Add `cwd: Option<String>`, `env: Vec<(String, String)>`, `uid: Option<u32>`, `argv_matches: Option<regex::Regex>`.
- Add builder methods `.with_cwd(...)`, `.with_env(k, v)`, `.with_uid(u)`, `.argv_matches(re)`.
- `impl From<&TrapFilter> for FilterSnapshot` updates to carry the new fields (add to `crates/kelir-interface/src/fusion_plugin.flow.rs::FilterSnapshot`).

### `packages/ts/core/src/trap.protocol.ts::TrapFilter`

- Add `withCwd`, `withEnv`, `withUid`, `argvMatches`; update `TrapFilterSnapshot` to match `FilterSnapshot`.
- `packages/py/kelir/__init__.py::TrapFilter` — add `with_cwd`, `with_env`, `with_uid`, `argv_matches` (Python `re.Pattern`).

### `crates/kelir-interface/src/fusion_plugin.flow.rs::HookEntry`

- Add `priority: i32` (default 0). `HookEntry::default()` is not derived (uses `Option<Arc<dyn …>>`); add `Default` impl.
- `FusedTrampolineDescriptor` docs note: within a ring, handlers fire by descending priority, then registration order.
- `TrampolineTable::from_descriptor` — sort `chain.hooks` by `(-priority, ordinal)` before returning; the table is used to dispatch, so the pre-sort matters.

### `crates/kelir-engine/src/dispatch_plugin/engine.flow.rs`

- `dispatch_trap_inner`, `dispatch_fs_write`, `dispatch_net_outbound`, etc. — iterate `interceptors` by the new priority-then-ordinal order. Use the pre-sorted `trampoline` if present; else sort by `scope.ordinal` if registered. Concretely: add a `priority: i32` field to the `bind_*_scoped` storage and sort on registration. The bind methods return `()` — add a helper `sort_rings_by_priority(&mut self)` called from `seal()` and from `rehydrate_host_registration`.
- `.timeout(Duration)` on scope — add `timeout: Option<Duration>` to each `*_scopes` entry; in `dispatch_*`, wrap the interceptor call with `tokio::time::timeout`. On expiry return `Verdict::Drop { errno: Errno::Etimeout, reason: "hook timeout".into() }` from the sync call, or `FilterAction::Drop` (after alias, both are `Action`). The host-side trap is released; the `dispatch_trap_inner` returns the Drop.
- Add `Errno::Etimeout` to `errno_types.flow.rs` if not present (it is not).

### `crates/kelir-interface/src/protocol_wire.flow.rs::SuspendUntil`

- Add variant `Condition { poll_interval_ms: u64, handle: u64 }` for the wire form. The host-side registry keeps `Box<dyn Fn(&SessionContext) -> bool + Send + Sync>` keyed by `handle`. The Rust API on `HandlerCtx` is `ctx.suspend_until_condition(poll_interval, predicate)` returning `Action::Suspend { until: SuspendUntil::Condition { poll_interval_ms, handle } }` after registering the predicate. Document that `Deadline` and `Approval` stay.
- `packages/ts/core/src/verdict.protocol.ts::SuspendUntil` — add `{ kind: "condition"; pollIntervalMs: number; handle: number }` and `SuspendUntil.condition(pollIntervalMs, predicate)` that registers the predicate with the native bridge and returns the handle.
- `packages/py/kelir/__init__.py::SuspendUntil` — add `Condition(poll_interval_ms, predicate)` with `c.suspend(Condition(...))` registering through `kelir._kelir`.

### `crates/kelir-interface/src/wire/net_ops.flow.rs::WireWiring`

- Replace `sinkhole_allowlist: Vec<String>, sinkhole_drop_all: bool` with `sinkhole: SinkholeConfig { rules: Vec<SinkholeRule> }`.
- `crates/kelir/src/planes/wire_plane.rs::SinkholeConfig` — replace with the rules struct:
  ```rust
  pub struct SinkholeRule { pub host: String, pub methods: Option<Vec<String>>, pub paths: Option<Vec<String>>, pub allow: bool }
  ```
- `SinkholeConfig::from_allowlist_drop_all(allowlist, drop_all)` returns the desugared rules; `WireWiring::default()` uses `sinkhole: SinkholeConfig::default()` (empty rules).
- `crates/kelir-engine/src/planes/wire/plane.flow.rs::WirePlane::with_wiring` — consumes `wiring.sinkhole.rules`. The existing `record_drop` records `metrics.record_drop()` on `FilterAction::Drop`; the DNS resolver (not in repomix) returns `DnsRcode::NxDomain` for `allow: false` per domain.
- `crates/kelir-bindings/napi/src/builder.flow.rs::NativeWireConfig` — replace `sinkhole_dns_except`/`sinkhole_drop_all` with `sinkholeRules: Vec<NativeSinkholeRule>`; keep the two old fields as `Option<...>` sugar that desugars.
- `crates/kelir-bindings/pyo3/src/builder.flow.rs::set_wire_config` — accept `sinkhole.rules` or the legacy `sinkhole.allowlist`/`drop_all` (desugar).
- `packages/ts/core/src/builder.config.ts::WireConfig` — `sinkhole?: { readonly rules: readonly SinkholeRule[] }`, plus legacy `dnsExcept`/`dropAll` (desugar).
- `packages/py/kelir/wire_planes.py::SinkholeConfig` — replace with `SinkholeConfig(rules: list[SinkholeRule])` and a `from_legacy(allowlist, drop_all)` classmethod.

### `crates/kelir-interface/src/plugin_model.flow.rs::VetoList` and `Veto`

- Add `pub struct Veto { allowed: bool, by: String, reason: String, recovery: Option<String> }`.
- `VetoList = Vec<(VetoEvent, Box<dyn Fn() -> Veto + Send + Sync>)>`.
- `LifecycleCtx::register_veto(&mut self, event, callback: Box<dyn Fn() -> Veto + Send + Sync>)` — signature change (callback returns `Veto`).
- `crates/kelir/src/builder_sdk.flow.rs::LifecycleMut::register_veto` — same signature change.
- `crates/kelir/src/session_lifecycle.flow.rs::Session::set_vetoes` — accepts the new type; `Session::fork` copies it (Veto is `Clone`, `VetoEvent` is `Copy`; the `Box<dyn Fn>` is not `Clone` — `Session::fork` currently does `std::mem::take(&mut registration.vetoes)` and `registration.vetoes` is passed to `child.set_vetoes`. This works because `VetoList` is `Vec<...>`; the `Clone` on `Vec` fails for `Box<dyn Fn>`. Currently `Session::fork` reads `parent_registration` which doesn't include vetoes; only `finish_boot` sets vetoes. So `fork` doesn't clone vetoes today — no regression, but the work order says `fork_state` on lifecycle should preserve. Flag as an L3 gap if needed.)
- `LifecycleCtx::register_veto` call sites: `packages/py/kelir/model_plugin.py::LifecycleCtx.register_veto` — accept `predicate` returning `Veto` (or bool for back-compat; desugar bool → `Veto { allowed: bool, by: "", reason: "", recovery: None }`). `crates/kelir-plugin-testkit/src/vectors_testkit.flow.rs::veto_case` — `callback_allows: bool` still valid; `register_veto_case` passes it through the bool→Veto desugar.

### `crates/kelir-interface/src/telemetry/receipt_attest.flow.rs::ExecutionReceipt`

- Add `pub hook_trace: Vec<HookSpan>`.
- Add `pub struct HookSpan { plugin: String, ring: Ring, verdict_kind: VerdictKind, duration_ns: u64, ordinal: u32 }`.
- Add `pub enum VerdictKind { Pass, Drop, Suspend, Abort, Patch, Forward }` (or reuse `Action` discriminant).
- The `ReceiptStore::insert` and dispatch-loop insertion sites (in `engine.flow.rs` `dispatch_trap_inner`) populate the trace from a per-dispatch `Vec<HookSpan>` collected as interceptors run. The collection is only enabled when a receipt exists (i.e. `ctx.receipt.is_some()`), per the "diagnostic only, excluded from Merkle" rule.
- `crates/kelir-interface/src/telemetry/merkle_attest.flow.rs::ExecutionReceipt::leaf_hash` — **exclude** `hook_trace` from the hash (destructure `hook_trace: _`, consistent with `wall_clock_ns` and `phase_timings`). Update `same_identity` to skip `hook_trace` too.
- `crates/kelir-interface/src/telemetry/receipt_attest.flow.rs::same_identity` — skip `hook_trace` in the field-by-field comparison.
- `packages/py/kelir/generated/attest_telemetry.py` — regenerate via `cargo run -p kelir-interface --bin codegen`. `ExecutionReceipt.hook_trace: list[HookSpan]`, `HookSpan`, `VerdictKind` union.
- `packages/ts/core/src/telemetry.attest.generated.ts` — regenerate; add `HookSpan` + `VerdictKind` and `ExecutionReceipt.hookTrace`.
- `crates/kelir-otlp/src/records_otlp.flow.rs::OtlpReceiptRecord::from_receipt` — project `hook_trace` into spans (add `hook_spans(receipt.hook_trace)` alongside `phase_spans(receipt.phase_timings.as_ref())`).

### `crates/kelir-interface/src/telemetry/receipt_attest.flow.rs::ExecutionReceipt::extensions`

- Replace `pub extensions: Vec<ReceiptExtension>` with typed accessors:
  - `pub fn fs_vfs(&self) -> Option<&FsVfsExt>`
  - `pub fn net_l7_sql(&self) -> Option<&NetL7SqlExt>`
  - `pub fn hooks(&self) -> &[HookSpan]`
  - `pub fn ext_raw(&self) -> &[(String, Vec<u8>)]`
- Store raw bytes internally as `raw_extensions: Vec<ReceiptExtension>` plus typed sidecar `fs_vfs: Option<FsVfsExt>`, `net_l7_sql: Option<NetL7SqlExt>`. The leaf hash hashes the raw form (so the wire is unchanged); the typed accessors are computed views.
- `crates/kelir-otlp/src/records_otlp.flow.rs` — replace the `extensions` field access with `receipt.ext_raw()` (JSON-encodable) — keep the wire shape.
- `crates/kelir-plugin-testkit/src/vectors_testkit.flow.rs` — `ExecutionReceipt` is not directly constructed in vectors; no change.
- `crates/kelir-plugin-testkit/src/vectors_parse.flow.rs` — no direct `ExecutionReceipt` construction; no change.
- `crates/kelir/src/telemetry_attest.rs` — re-export `HookSpan`, `VerdictKind`.
- `crates/kelir/src/session_lifecycle.flow.rs::Session::store_receipt` / `receipt` — unchanged.

### `crates/kelir-engine/src/dispatch_plugin/engine.flow.rs::dispatch_trap_inner`

- Collect `HookSpan` for every interceptor that fires when `ctx.receipt.is_some()`. Use a local `Vec<HookSpan>` and pass it through `interceptor.on_trap(.., &mut spans)` — this is a trait signature change (`TrapInterposer::on_trap(&self, event, ctx, &mut Vec<HookSpan>)`). Alternatively, wrap the `interceptor.on_trap` call with `Instant::now()` timing at the dispatch layer, no trait change. Prefer the latter for minimal diff.
- Emit the `HookSpan` to OTel via `OtlpExporter::export_span` (already exists in `crates/kelir-otlp/src/exporter.flow.rs::export_span`); the trace spans reuse the `phase_spans` shape.

### `crates/kelir-engine/src/dispatch_plugin/registration.flow.rs::TrapDenyCallback`

- Change to `Box<dyn Fn(&TrapEvent, &Action, &Session) -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send>> + Send + Sync>`.
- `set_on_trap_deny` / `set_on_trap_deny_shared` — update.
- `crates/kelir/src/builder_sdk.flow.rs::on_trap_deny` — accept the 3-arg closure; update the TS `KelirBuilder.onTrapDeny` and Python `on_trap_deny`.
- `crates/kelir-engine/src/dispatch_plugin/engine.flow.rs::dispatch_trap_inner` — pass the denying `Action` to the callback.
- `crates/kelir-plugin-testkit/src/vectors_testkit.flow.rs::plugin_builder_*` — the deny vector’s `expected_outcomes` is a `Verdict`/`FilterAction`; after the callback change, the `on_trap_deny` fire is exercised in `hook-matrix-full.*` (spec-side) and a new test vector. Add a `trap_deny_fires_on_drop` vector under `crates/kelir-plugin-testkit/src/vectors_testkit.flow.rs`.

### `crates/kelir-plugin-cli/src/audit_cli.rs` (and `rings_cli.rs` if present)

- Add `--trapfilter-coverage` — assert every `TrapFilter` predicate method has at least one vector in `vectors.json`.
- Add `--hook-trace-coverage` — assert every `Ring` value appears in at least one `HookSpan` vector.

### Parity vectors

- `crates/kelir-plugin-testkit/src/vectors_testkit.flow.rs::plugin_builder_trap_case` — add a `plugin-builder-trap-filter` variant that uses `with_cwd`, `with_env`, `with_uid`, `argv_matches`. Extend `plugin_builder_verb_cases` and `plugin_builder_scope_cases`.
- `crates/kelir-plugin-testkit/src/vectors_chain_testkit.rs` — add the `hook_trace` populated case.
- `crates/kelir-plugin-testkit/src/vectors_parse.flow.rs::VectorCase::from_json` — parse the new `hook_trace` field on `expected_outcomes` if you add a `hooks` field. Simplest: don’t add a wire field; the hook trace is produced at dispatch and verified by the testkit’s `--hook-trace-coverage` gate against the compiled vector suite. Coordinate with Agent 3 on vector counts.

### `crates/kelir-plugins/plugin-replay/src/replay_state.flow.rs::ReplayState`

- `AccrueStepWindows` should be triggered by the PMU sampler for `SuspendUntil::Condition` polling too. Coordinate with `lc_23` (Agent 3) for the window-resume wiring. No code motion needed until `Condition` predicates register.

### `crates/kelir-plugins/plugin-tool-guard/src/tool_guard_state.flow.rs`

- `on_console_input` — `evaluate_tool_call` returns `Verdict::Pass`; the `on_trap_deny(event, verdict, session)` callback is now given the `Verdict` (which after the alias is `Action`). Pass it through. The `Verdict::Suspend` arm maps to `Err(PluginError::Halt(..))` today; after the trait signature change, it would be `Action::Suspend` with `SuspendUntil::Condition`. Coordinate with the trait-signature HANDOFF above.

### `crates/kelir-plugins/plugin-mesh/src/mesh_state.flow.rs::MeshState::record_rejection`

- Rejection is currently a log; it should be added to `ExecutionReceipt.hook_trace` (and also to `receipt.hooks()`), per Agent 5 item 7. Land it in `on_connect` — no new trait method.

### Cross-cutting

- `crates/kelir-plugin-testkit/src/vectors_testkit.flow.rs::plugin_builder_verb_event` — the `heartbeat` / `memory-pressure` events are unchanged.
- `crates/kelir-plugin-testkit/src/fixtures_testkit.rs::canonical_vector_names` — unchanged.

---

## Cross-cutting codebase sequencing

The parent work-order’s critical path (`A1 → A5`, `A2 → A3 → A5`) is preserved, but the codebase has two additional hard edges:

1. **`crates/kelir-interface` first.** Every downstream crate re-exports from it (`protocol_wire::Verdict`, `plugin_model::PluginError`, `driver_interface::VmConfig`, `telemetry::ExecutionReceipt`, `wire::WireWiring`, `fusion_plugin::HookEntry`). Land the Agent 1, 4, and 5 interface edits on a single branch (or three stacked branches) before any crate that touches the surfaces. `just check-spec` + `cargo check --workspace` must pass.

2. **`crates/kelir-plugin-testkit` second.** After the interface lands, update vectors and the `case_map`/`scorecard`. `kelir-plugin-testkit` is the only crate that stores `Verdict`/`FilterAction`/`ExecutionReceipt` JSON literals; every other crate constructs or re-exports them. Land the vectors before the per-plugin edits to avoid the third pass.

3. **`crates/kelir` and `crates/kelir-bindings` third.** The builder surface (Agent 3) and the cap accessors (Agent 2) touch both. Land them together on the `<agent>-pins` branch (or stacked) so `plugin_instance`/`.use`/`.plugin`, `cap`/`ctx`, `pass_`/`pass`, and the scope interfaces do not diverge mid-stream.

4. **Per-plugin fourth.** `plugin-chaos`, `plugin-container`, `plugin-db-proxy`, `plugin-ledger`, `plugin-mcp`, `plugin-mesh`, `plugin-profile`, `plugin-replay`, `plugin-supply-chain`, `plugin-tool-guard`, `plugin-transaction`, `plugin-vault`, `plugin-wire-ingress` — each gets its `TODO (L3)` collapse and `PluginError::reject` call-site conversions in the same PR as the interface change if possible. The `Cargo.toml` `plugin-*` feature flags in `crates/kelir` are unchanged.

5. **Python and TS last.** Both generated SDKs (`packages/py/kelir/generated/`, `packages/ts/core/src/generated/`) are regenerated by `cargo run -p kelir-interface --bin codegen`; the non-generated SDK surfaces (`kelir_builder.py`, `builder.lifecycle.ts`, `hook.registry.ts`, `session.lifecycle.ts`, `plugins.registry.ts`, `kelir/__init__.py`, `lifecycle_session.py`, `model_plugin.py`, `plugin_fusion.py`) must be updated in the same commit as the codegen. `packages/py/pyproject.toml` `mypy --strict` and `tsc -p tsconfig.check.json` are the gates.

6. **`crates/kelir-adapters` (kelir-pack, mcp-server) last.** Both are aspirational and can be moved under `crates/kelir-adapters/aspirational/` per Agent 4. The `Cargo.toml` paths remain valid.

### Shared validation per branch

```
just check-spec
just check-examples
just check-plugin-coverage          # A4 (deleted runner)
just check-action-coverage          # A1
just check-hook-coverage            # A5
just check-trapfilter-coverage      # A5
just check-hook-trace-coverage      # A5
just check-unified-builder-coverage # A3
cargo check --workspace --all-targets
cargo test  --workspace
cargo run -p kelir-interface --bin codegen -- --check   # or `git diff --exit-code` after
python -m mypy packages/py/kelir
(cd packages/ts/core && npx tsc -p tsconfig.check.json)
cans check --refs-only              # before any pin bump
```

### Open questions raised by the addendum (file as `HANDOFF` entries)

- **A1** — the parent work-order’s “delete `Verdict.pass_()` trailing underscore” is not executable in Python (`pass` is a keyword). Keep `pass_()` in Python; note the defect in the spec edit commit.
- **A1** — `ConsoleInterposer::on_console_input`, `on_console_output`, `ConsoleInterposer::on_exec_stdout/stderr`, and `StreamFrameSink::on_stream_frame` return `Result<Vec<u8>, PluginError>` / `Result<(), PluginError>`, so they cannot express `Action::Drop`. File a follow-up `HANDOFF` for the `PipelineBinder` / trait-surface change.
- **A3** — negative compile test for `.fs(|f| f.on_trap(...))` needs a `trybuild` crate; add `trybuild` as a dev-dep of `kelir-plugin-testkit`.
- **A4** — `crates/kelir-testkit/src/scenario_catalog.rs` may or may not be the “scenario runner” the parent work-order deletes. Decide before deleting.
- **A5** — `SuspendUntil::Condition(Box<dyn Fn>)` is Rust-only; the wire form is `Condition { poll_interval_ms, handle }` with a host-side registry keyed by `handle`. Confirm with the SDK leads.
- **A5** — `ExecutionReceipt.hook_trace` is “excluded from Merkle roots”; confirm the OTLP encoder projects it into spans but `leaf_hash` does not include it (it currently destructures `extensions` and skips `wall_clock_ns` / `phase_timings`, so add `hook_trace: _`).