# work-order.md — Parallel CANS alignment for kelir

```markdown
# Work Order: Spec → Artifact → Code Alignment (5 parallel agents)

Phase: L1/L2 only. No L3 implementations. Deferred work is a `// TODO (L3):` comment.
Merge owner: one human reviewer. Agents push to isolated branches; no direct push to main.

## 0. Scope

All L1 type-contract violations (A-series) and L2 wiring gaps (B-series) identified in the
prior audit. L3 TODOs are explicitly out of scope. Every change follows the mandatory order:

    cans/*.md            (spec)
    cans/artifacts/*     (canonical content: code sketches, diagrams, fixtures)
    crates/*, packages/* (code)

A change that updates only one layer is incomplete.

## 1. Hard rules (all agents)

1. **File ownership is exclusive.** An agent edits only files listed under its "Owns"
   section. Any needed edit to a file owned by another agent goes through a
   `_collab/handoffs.md` entry (see §6) or the pre-agreed contract in §3.
2. **Preserve real identifiers exactly** (AGENTS.md §Writing): `Session::vm_handle`,
   `MCP_CONSOLE`, `PROFILE_SYNC`, `DB_SQLITE`, `SuspendReason::External`, etc.
3. **No silent spec drift.** If spec and code disagree and the agent cannot tell which is
   canonical, add a `_collab/conflicts.md` entry (one line per conflict; format in §6) and
   do not overwrite either side.
4. **`cans check --refs-only` must pass** after every spec edit. `cans budget write` gates
   touched files; the agent's touched-file set must equal its "Owns" set.
5. **Pre-flight gates that apply to every agent:** `cargo check --workspace --all-targets`,
   `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`,
   `pnpm -r exec tsc --noEmit` (agent 5 only), `mypy packages/py` (agent 5 only),
   `just check-hygiene`, `just check-spec`, `just check-manifest`.
6. **No new `// TODO (L3)` beyond what already exists.** If a fix requires L3 work, note it
   in the PR body and leave the existing L3 TODO in place.
7. **Commit message** records the rule number (e.g. `A1`, `B3`) and, if any conflict was
   resolved by precedent, the precedent used.
8. **Parallel discipline:** each agent works on its own worktree `target/agent-<id>` with
   `CARGO_TARGET_DIR=target/agent-<id>` to avoid build-lock contention.

## 2. Ownership matrix

| Agent | Owns (code) | Owns (spec/artifacts) |
|---|---|---|
| **Agent 1** — Plugin contract | `crates/kelir-interface/src/plugin_model.rs`, `crates/kelir/src/plugin_builder.rs`, `crates/kelir/src/lib.rs`, `crates/kelir-core/src/dispatch_plugin.rs`, all 13 `crates/kelir-plugins/*/src/lib.rs` | `cans/02-plugin-model.md`, `cans/11-protocol.md`, all `cans/plugins/*.md`, `cans/artifacts/code/plugin-trait.rs`, `cans/artifacts/code/plugin-builder-vectors.rs` |
| **Agent 2** — Session lifecycle | `crates/kelir-core/src/session_lifecycle.rs`, `crates/kelir/src/session_lifecycle.rs`, `crates/kelir/src/sandbox_runtime.rs` | `cans/08-lifecycle.md`, `cans/09-snapshots.md` |
| **Agent 3** — Storage FUSE | `crates/kelir-core/src/planes/storage/{fuse_storage,io_storage,lock_broker,mod}.rs` | `cans/05-storage-plane.md`, `cans/artifacts/code/reconciler.rs` |
| **Agent 4** — Driver + terminal + CLI | `crates/kelir-interface/src/driver_interface.rs`, `crates/kelir-core/src/planes/terminal/mod.rs`, `crates/kelir/src/builder_sdk.rs`, `crates/kelir-plugin-cli/src/{main.rs, bump_core_cli.rs, attest_cli.rs}` | `cans/04-terminal-plane.md`, `cans/15-operations.md`, `cans/artifacts/code/vmm-driver-trait.rs`, `cans/artifacts/config/{justfile,ci.yml}` |
| **Agent 5** — SDK parity | `packages/py/kelir/{__init__.py, lifecycle_sandbox.py}`, `packages/ts/core/src/{builder.lifecycle.ts, sandbox.runtime.ts, session.lifecycle.ts}` | `cans/03-sdk-architecture.md`, `cans/artifacts/code/{typescript-accumulation.ts, python-raii-pydantic.py}` |

**Untouched by all agents:** `cans/artifacts/examples/fixtures/kelir.yaml` (fixtures already correct; if a fixture edit is required, request it via `_collab/handoffs.md` — the reviewer applies it), `cans/_rules.yaml`, `AGENTS.md`.

**No agent touches a file outside its column.** If two agents need the same file, §3 pins the interface.

## 3. Cross-agent contracts (pre-agreed, frozen)

These signatures are added by the owning agent and consumed by others. Do not change without a `_collab/conflicts.md` entry.

### 3.1 `PipelineBinder` additions (Agent 1 → Agent 2 consumes)

Added to `crates/kelir-interface/src/plugin_model.rs`:

```rust
fn bind_lifecycle(&mut self, stage: Arc<dyn LifecycleParticipant>) {
    self.bind_lifecycle_scoped(stage, MountScope::default());
}
fn bind_lifecycle_scoped(&mut self, stage: Arc<dyn LifecycleParticipant>, scope: MountScope);

fn bind_telemetry(&mut self, stage: Arc<dyn SystemTelemetryParticipant>) {
    self.bind_telemetry_scoped(stage, MountScope::default());
}
fn bind_telemetry_scoped(&mut self, stage: Arc<dyn SystemTelemetryParticipant>, scope: MountScope);

fn bind_stream_kind(&mut self, kind: StreamKind, sink: Arc<dyn StreamFrameSink>) {
    self.bind_stream_kind_scoped(kind, sink, MountScope::default());
}
fn bind_stream_kind_scoped(&mut self, kind: StreamKind, sink: Arc<dyn StreamFrameSink>, scope: MountScope);
```

`StreamFrameSink` trait is added in the same file:

```rust
pub trait StreamFrameSink: Send + Sync {
    fn on_stream_frame(
        &self,
        stream_id: u32,
        kind: StreamKind,
        flags: u8,
        payload: &[u8],
    ) -> Result<(), PluginError>;
}
```

### 3.2 `PipelineEngine` lifecycle dispatch (Agent 1 → Agent 2 callsite)

Added to `crates/kelir-core/src/dispatch_plugin.rs`:

```rust
impl PipelineEngine {
    pub async fn dispatch_lifecycle(
        &self,
        event: LifecycleEvent,
        ctx: &mut LifecycleCtx<'_>,
    ) -> Result<(), PluginError>;

    pub async fn dispatch_memory_pressure(
        &self,
        available_mb: u32,
        total_mb: u32,
    ) -> Result<(), PluginError>;

    pub async fn dispatch_heartbeat(&self, uptime_ms: u64) -> Result<(), PluginError>;

    pub async fn dispatch_watchdog(
        &self,
        event: &WatchdogEvent,
    ) -> Result<WatchdogVerdict, PluginError>;
}
```

**Agent 2 must call these four from `crates/kelir-core/src/session_lifecycle.rs::run_lifecycle_hook` and `Session::shutdown` / `Session::stop`.** Agent 2 does not implement them.

### 3.3 `Session::vm_handle` remains (Agent 2 → Agent 1 does not touch)

Agent 1's `crates/kelir/src/lib.rs` re-exports do **not** add a `vm` field or method — that is Agent 2's work (A10). Agent 1 re-exports only plugin config and context types.

### 3.4 `EventBusBinder` object-safety (Agent 1 only)

Fix in `crates/kelir-interface/src/plugin_model.rs`. `KelirPlugin::register_subscribers(&self, _bus: &mut impl EventBusBinder)` becomes `&mut dyn EventBusBinder` once object-safe. If no workable design lands in 30 minutes, leave the current `impl` form and add a `_collab/conflicts.md` entry. Do not invent a workaround that changes `subscribe` behavior.

## 4. Per-agent task lists

Each task cites its issue ID from the prior audit. Do spec first, artifact next, code last.

### Agent 1 — Plugin contract, binder, engine

**Spec edits** (`cans/02-plugin-model.md`, `cans/11-protocol.md`, `cans/plugins/*.md`):
- A2: in `02-plugin-model.md#Segregated-ring-traits`, add one line under "Ring traits" noting `LifecycleParticipant` and `SystemTelemetryParticipant` are bound via `bind_lifecycle` / `bind_telemetry` on `PipelineBinder`. Add one line under `11-protocol.md#Kind-registration` noting `bind_stream_kind`.
- A1: in each `cans/plugins/<name>.md#Crate-dependencies-(<name>)`, ensure the `Depends on:` line matches the real cross-crate dependency the plugin will encode: tool-guard → `kelir-interface, plugin-mcp`; transaction → `kelir-interface, plugin-db-proxy`; supply-chain → `kelir-interface and Tier 1 crates`; profile → `kelir-interface, Tier 1 and Tier 2 crates`. If spec and code disagree because spec is stale, edit spec; if because code is stale, edit code (both under Agent 1's control).
- A7: add `suppress_local_db` to `cans/plugins/ledger.md#Invariants-and-policy-(ledger)` as an explicit key, and to the bullet list under `LedgerConfig`.
- A9: in `cans/plugins/tool-guard.md#Host-extensible-hook-pipeline`, keep the `ToolGuardExt` mention; no spec edit needed if the trait is added to code — but verify spec syntax references an existing method.
- B11: in `cans/plugins/container.md#Non-nested-OCI-container-execution` and `cans/plugins/transaction.md#Multi-resource-2PC-coordination`, tighten "required" language so the corresponding config flag defaults to `true` and cannot be silently disabled without an explicit `ESecurityViolation` opt-out. If spec already says invariant, no edit; add a `_collab/conflicts.md` entry instead.

**Artifact edits**:
- `cans/artifacts/code/plugin-trait.rs`: add `StreamFrameSink` trait; add `bind_lifecycle` / `bind_telemetry` / `bind_stream_kind` to `PipelineBinder`; change `register_subscribers` to `&mut dyn EventBusBinder` if the object-safe design lands.
- `cans/artifacts/code/plugin-builder-vectors.rs`: no change unless new hooks are added.

**Code edits**:
- A2: `kelir-interface/src/plugin_model.rs` — add `bind_lifecycle_scoped`, `bind_telemetry_scoped`, `bind_stream_kind_scoped`, `StreamFrameSink` trait.
- A2/B1: `kelir-core/src/dispatch_plugin.rs` — implement the four dispatch methods in §3.2; store `Arc<dyn LifecycleParticipant>` and `Arc<dyn SystemTelemetryParticipant>` vectors plus stream-kind sinks; wire them in `PipelineBinder` impl.
- A2/B12: `kelir/src/plugin_builder.rs` — `PluginWrapper::register_interceptors` calls the new bind methods for each collected `LifecycleParticipant` and `SystemTelemetryParticipant`; drop the `let _ = (&self.lifecycle, &self.telemetry);` line.
- A1: all 13 `kelir-plugins/*/src/lib.rs` — add `impl PluginDeps for <Plugin> { type Deps = ...; }` with the real dependency graph: `chaos → NoDeps`, `profile → NoDeps` (Agent 1 may tighten to real deps if the DAG is easy; else keep `NoDeps` and note), `tool-guard → HCons<McpCtx, HNil>`, `transaction → HCons<SqlProxyCtx, HNil>`, `supply-chain → NoDeps` for now, others `NoDeps`.
- A1 (per plugin dep verification): `plugin-tool-guard/Cargo.toml` and `plugin-transaction/Cargo.toml` already declare the crate deps; ensure `Deps` type matches the crate dep.
- B2: `plugin-mcp/src/lib.rs` — add `bind_stream_kind_scoped(StreamKind::McpConsole, Arc::new(McpCtxStreamSink { service }), scope)` in `register_interceptors`; the sink impls `StreamFrameSink` and calls `McpEndpointService::route_request`.
- B2: `plugin-profile/src/lib.rs` — impl `register_interceptors`; bind `StreamKind::ProfileSync` to a `ProfileCtxStreamSink` that calls `ProfileCtx::dispatch_profile_sync`.
- B2: `plugin-db-proxy/src/lib.rs` — bind `StreamKind::DbSqlite` to a `SqliteBridge`-backed `StreamFrameSink`.
- A7: `plugin-ledger/src/lib.rs` — add `pub suppress_local_db: bool` to `LedgerConfig`; `LedgerPlugin::sink()` returns a no-op `LedgerSink` when `suppress_local_db` is `true`.
- A9: `plugin-tool-guard/src/lib.rs` — add `pub trait ToolGuardExt { fn tool(self, spec: ToolSpec) -> Self; }` and blanket-impl for `KelirBuilder`. If the blanket-impl requires `kelir::` and creates a cargo cycle, add the trait to `crates/kelir/src/plugin_builder.rs` and re-export from `plugin-tool-guard` — note in PR.
- B10: `plugin-tool-guard/src/lib.rs` — move the `McpMethod::ToolsList` interception from `on_console_output` to a new `impl LifecycleParticipant for ToolGuardPlugin { async fn on_vm_ready(...) }` that issues an explicit `tools/list` query through `self.mcp.route_request` and locks the catalog. `on_console_output` keeps schema drift detection only.
- B11: `plugin-container/src/lib.rs` — default `isolate_var_lib_containers` to `true`; return `PluginError::reject("ESecurityViolation", ...)` when `false` AND the caller has not set an explicit opt-out field (add `allow_unsandboxed: bool` defaulting to `false`).
- B11: `plugin-transaction/src/lib.rs` — `flush_fuse_before_commit` defaults to `true`; if `false`, `on_pre_commit` returns `PluginError::reject("ESecurityViolation", ...)`.
- A12: `kelir/src/lib.rs` — re-export every plugin's `Config` and `Ctx` type: `pub use plugin_mcp::{McpConfig, McpCtx}`, `pub use plugin_tool_guard::{ToolGuardConfig, ToolGuardCtx}`, etc. Mirror the pattern used for `plugin-ledger`.
- A4: `kelir-interface/src/plugin_model.rs` — if a workable object-safe `EventBusBinder` design exists (e.g. `subscribe<F: for<'a> Fn(&'a dyn Any) + Send + Sync + 'static>` with a `TypeId`-keyed dispatcher), implement it. Otherwise `_collab/conflicts.md`.

**Definition of done:** All 13 plugins impl `PluginDeps`; `plugin-mcp`, `plugin-profile`, `plugin-db-proxy` bind their stream kinds; `PipelineEngine` has four new dispatch methods; `PluginBuilder` routes lifecycle/telemetry; `kelir::*` re-exports every plugin's config and context; A7, A9, B10, B11 landed. `cargo check --workspace` green.

---

### Agent 2 — Session lifecycle, CoW fork, teardown, storage flush

**Spec edits** (`cans/08-lifecycle.md`, `cans/09-snapshots.md`):
- B3: in `08-lifecycle.md#CoW-fork`, tighten "Driver primitive: `fork` returns `Box<dyn VmmDriver>`" to require the child's driver handle come from `driver.fork()`, not from an `Arc` clone of the parent.
- B5: in `08-lifecycle.md#Pre-teardown-flush`, confirm the graceful path requires `session.storage.reconcile()` (through the anchored reconciler) before `MSG_SHUTDOWN_REQUEST`, and `stop_with_timeout(0)` explicitly follows the abrupt path (`session.kill()`). If spec says otherwise, note in `_collab/conflicts.md`.
- A10: in `09-snapshots.md#Archives-and-portability`, only if the spec mentions the accessor; otherwise no change.

**Artifact edits**:
- `cans/artifacts/code/vmm-driver-trait.rs`: add `fork` as a documented primitive (already present — verify); no other change.

**Code edits**:
- A10: `kelir-core/src/session_lifecycle.rs` — add `pub fn vm(&self) -> VmHandle` returning `self.vm_handle()` (thin alias), so `session.vm.checkpoint(...)` compiles on the Rust SDK. Update `crates/kelir/src/session_lifecycle.rs:Session` to forward `pub fn vm(&self) -> VmHandle { self.core.vm() }`.
- B1: `kelir-core/src/session_lifecycle.rs::run_lifecycle_hook` — replace the current builder-callback-only body with:
  1. Call `self.engine.dispatch_lifecycle(event, &mut ctx).await?` for plugin participants, using a `LifecycleCtx` built from the live driver.
  2. Then call the builder-supplied `on_vm_init` / `on_vm_ready` / `on_vm_shutdown` callback (existing code) if present.
  Both must run; if either fails, propagate.
- B3: `kelir-core/src/session_lifecycle.rs::fork` — replace `driver: self.driver.clone()` with:
  ```rust
  let child_driver = match &self.driver {
      Some(d) => Some(Arc::new(AsyncMutex::new(d.lock().await.fork().await.map_err(...)?))),
      None => None,
  };
  ```
  Then pass `child_driver` into the child `Session`. `session.fork()` must build a fresh `OwnedBusDispatcher`, fresh `WirePlane`, fresh `CheckpointPlane` per the spec, and call `plugin.fork_state()` on each plugin (Agent 1 provides `AnyPlugin::fork_state`; Agent 2 calls it).
- B4: `kelir/src/session_lifecycle.rs::fork` — propagate `Registration`:
  - `traps`: clone the `Vec<TrapEntry<Plugins>>` — handlers are `Box<dyn Fn>` requiring `Clone`, which `TrapEntry` does not currently derive. Add `#[derive(Clone)]` to `TrapEntry` in `builder_sdk.rs`? **That file is Agent 4's.** Coordinate: Agent 4 adds `impl Clone for TrapEntry` (or `#[derive(Clone)]`); Agent 2 uses `.traps.clone()`.
  - `storage_cfg`, `terminal_cfg`, `wire_cfg`, `lock_broker_cfg`: `Clone` — already `Clone` on their config types.
  - `on_vm_init` / `on_vm_ready` / `on_vm_shutdown` / `on_trap_deny`: `Arc` clone.
  - `custom_actions`, `vsock_services`, `stream_kind_services`, `vetoes`, `detached`: clone.
  - Child `Registration` is a full clone, not `Registration::default()`.
- B5: `kelir-core/src/session_lifecycle.rs::stop` — before `advance_lifecycle(Terminated)`:
  ```rust
  let stats = self.storage.reconcile(FsReconcileRequest {
      cwd: self.storage.workspace_root().display().to_string(),
      atomic_staging: true,
      fsync: true,
  })?;
  ```
  If `Reconciler` is not anchored (L2 default), skip silently and log a warning; if anchored, reconcile. `stop_with_timeout(timeout)`: if `timeout == Duration::ZERO`, skip reconcile and call `kill()` semantics. Otherwise reconcile with `timeout`.
- B5: `kelir/src/sandbox_runtime.rs::Sandbox::stop` and `stop_with_timeout` — forward to `Session::stop` / `stop_with_timeout` (which now reconcile).

**Definition of done:** `session.vm.checkpoint(...)` compiles on `kelir::Session`; `session.fork()` returns a child with its own driver handle and full Registration; `session.stop()` reconciles storage when a reconciler is anchored; plugin lifecycle hooks fire through `PipelineEngine::dispatch_lifecycle`. `cargo check --workspace` green; `just check-hygiene` green.

---

### Agent 3 — Storage FUSE, IO, lock broker

**Spec edits** (`cans/05-storage-plane.md`):
- A5: in `#Host-CoW-broker` (or a new sibling node `#FUSE-opcode-table`), list the eight required opcodes explicitly: `lookup`, `getattr`, `read`, `write`, `readdir`, `unlink`, `mkdir`, `rename`, plus `statfs`, `setlk`, `setlkw`. This is the canonical home.
- A6: in `#Host-CoW-broker`, add one line: "Host I/O is expressed through `StorageIo`; the broker write primitive is `StorageIo::write(path, offset, data)`."
- B6: in `#Storage-quotas`, add "Every `FUSE WRITE`, `MKNOD`, `CREATE`, and `MKDIR` dispatch calls `StoragePlane::check_quota` before the payload reaches the upper file."
- B7: in `#Host-inode-lock-broker`, add "`SETLK` and `SETLKW` dispatch calls `LockBroker::acquire` before the interposer; a denial replies `FuseReply::Errno(EWOULDBLOCK)`."
- B8: in `#Host-CoW-broker` or the new FUSE-opcode node, add "FUSE `WRITE` reply carries the requested chunk size (`fuse_write_out.size`), not the payload bytes."

**Artifact edits**:
- `cans/artifacts/code/reconciler.rs`: add a `StorageIo::write` sketch comment if the file touches it; no structural change unless the spec edit demands.

**Code edits**:
- A5: `kelir-core/src/planes/storage/fuse_storage.rs` — extend `FuseOpcode` to:
  ```rust
  pub enum FuseOpcode {
      Lookup, Getattr, Read, Write, Readdir,
      Unlink, Mkdir, Rename, Statfs,
      Setlk, Setlkw,
  }
  ```
  Update `from_u32` with the UAPI numbers (getattr 3, readdir 28, unlink 10, mkdir 9, rename 12, statfs 17). Add `FuseReply::Attr(FsInodeAttr)` for getattr and `FuseReply::Entries(Vec<...>)` for readdir; return `ENOSYS` for statfs until L3 fills quota reporting. All non-L3 opcodes return a typed response shape now.
- A6: `kelir-core/src/planes/storage/io_storage.rs` — add to `StorageIo` trait:
  ```rust
  fn write(&self, path: &Path, offset: u64, data: &[u8]) -> Result<u64, PluginError>;
  fn mkdir(&self, path: &Path, mode: u32) -> Result<(), PluginError>;
  fn unlink(&self, path: &Path) -> Result<(), PluginError>;
  fn rename(&self, from: &Path, to: &Path) -> Result<(), PluginError>;
  ```
  Implement each as `Err(PluginError::Halt("TODO (L3): ..."))` — the interface addition is L1, the bodies are L3 deferred.
- B6: `fuse_storage.rs::FuseDispatcher::dispatch` — in the `Write` arm, call `self.quota.check_quota(...)` before the interposer (add a `quota: Arc<StorageQuota>` field to `FuseDispatcher`; plumb it through `FuseWorker::new`). Return `Ok(FuseResponse { reply: FuseReply::Errno(-ENOSPC) })` on quota rejection. Same for `Mknod`, `Create`, `Mkdir` arms (once A5 adds them).
- B7: `fuse_storage.rs::FuseDispatcher::dispatch` — in `Setlk`/`Setlkw` arms, call `self.lock_broker.acquire(&LockRequest { ... })`. On error, `Ok(FuseResponse { reply: FuseReply::Errno(-EWOULDBLOCK) })` — **not** `Err(PluginError::Halt)`. Plumb `Arc<LockBroker>` into `FuseDispatcher`.
- B8: `fuse_storage.rs::FuseDispatcher::dispatch` — in the `Write` arm:
  - `FilterAction::Forward` → `reply: FuseReply::Written(request.data.len() as u64)` (add a new `FuseReply::Written(u64)` variant or reuse `Payload` with an explicit wire framing — choose `Written(u64)` for clarity).
  - `FilterAction::Drop { errno: None, .. }` (silent blackhole per `cans/plugins/chaos.md`) → same `Written` with `request.size`.
  - `FilterAction::Drop { errno: Some(...) }` → `Errno(-posix_errno)` (already implemented).
- B8b: `kelir-core/src/planes/storage/mod.rs` — expose `StoragePlane::quota() -> Arc<StorageQuota>` so `FuseWorker::new` can take it.

**Definition of done:** `FuseOpcode` has 11 variants; `StorageIo` has the four new methods; quota and lock-broker are on the write and lock dispatch paths; write reply carries `size`. `cargo check --workspace` green; `cargo test -p kelir-core` still passes for existing tests (none at L2 in this file — verify).

---

### Agent 4 — Driver interface, terminal wiring, CLI

**Spec edits** (`cans/04-terminal-plane.md`, `cans/15-operations.md`):
- A8: in `04-terminal-plane.md#Terminal-pipeline`, add explicit bullet: "The Token Shield and prompt stages are part of the fixed pipeline; `TerminalWiring` must carry `token_shield` and `prompts` from the builder into the plane."
- A13: in `15-operations.md#Subcommands`, add `kelir-plugin-cli bump-core --channel <released|main>` and `kelir-plugin-cli attest --sign` as first-class subcommands (matching the existing `plugin new|manifest|check|vectors|rings --show` block).
- A3: **do not edit `cans/08-lifecycle.md`.** Add a `_collab/conflicts.md` entry: `- status: unresolved | file: cans/08-lifecycle.md | issue: spec declares SuspendReason::External(u32); code and cans/artifacts/code/vmm-driver-trait.rs declare External(ExternalSuspendKind). Which is canonical?` Align code to spec only if the reviewer resolves.

**Artifact edits**:
- `cans/artifacts/code/vmm-driver-trait.rs`: only if the reviewer resolves A3 in favor of `External(u32)` — then update the artifact's `SuspendReason` enum to `External(u32)` and remove `ExternalSuspendKind`. Otherwise leave the artifact as is and note the conflict.
- `cans/artifacts/config/justfile`: add `check-cli` target: `kelir-plugin-cli check --layers && kelir-plugin-cli check --graph`.
- `cans/artifacts/config/ci.yml`: add a `bump-core` and `attest` job placeholder with the actual `kelir-plugin-cli` invocation.

**Code edits**:
- A8: `kelir-core/src/planes/terminal/mod.rs::TerminalWiring` — extend to:
  ```rust
  pub struct TerminalWiring {
      pub cols: u16,
      pub rows: u16,
      pub drop_osc52: bool,
      pub strip_title: bool,
      pub token_shield: Option<kelir_interface::plugin_model::TokenShieldConfig>,
      pub prompts: Vec<kelir_interface::plugin_model::PromptRuleConfig>,
  }
  ```
  Add the two new config structs to `kelir-interface/src/plugin_model.rs`? **That file is Agent 1's.** Coordinate: Agent 4 adds the structs to `kelir-core/src/planes/terminal/mod.rs` (owns the file); Agent 1's `plugin_model.rs` is not touched.
- A8: `kelir/src/builder_sdk.rs::finish_boot` — build `TerminalWiring` from the full `PtyConfig`; no fields dropped. Any L3 gap (e.g. `prompts` action closures) becomes `// TODO (L3):` inside `TerminalPlane::with_wiring`.
- A3: `kelir-interface/src/driver_interface.rs` — align `SuspendReason` with the resolver's decision. If unresolved, leave as is and note in `_collab/conflicts.md`.
- A13: `kelir-plugin-cli/src/main.rs` — add `"bump-core" => bump_core_cli::run(&args[2..])?` and `"attest" => attest_cli::run(&args[2..])?`. Also change `"scaffold"` to `"new"` and keep `"scaffold"` as a hidden alias.
- A13: `kelir-plugin-cli/src/bump_core_cli.rs` (new) — parse `--channel <released|main>`, resolve to the pinned kelir-version string, write `.kelir/core-channel.json`, exit `Ok(())` on success. Body may be `// TODO (L3):` for the actual pin resolution, but the flag parser is real.
- A13: `kelir-plugin-cli/src/attest_cli.rs` (new) — parse `--sign`, require a `--plugin-path` flag, exit `Ok(())`. Signing is L3.

**Definition of done:** `TerminalWiring` carries `token_shield` and `prompts`; `builder_sdk::finish_boot` no longer silently drops them; `kelir-plugin-cli` parses `bump-core` and `attest` flags; A3 either resolved (with the artifact updated) or filed in `_collab/conflicts.md`. `cargo check --workspace` green; `just check-hygiene` green.

---

### Agent 5 — Python + TypeScript SDK parity

**Spec edits** (`cans/03-sdk-architecture.md`):
- A11: in `#Sandbox-high-level-facade`, pin the `suspend()` signature explicitly:
  ```
  - box.suspend() takes no argument; the suspend image path is derived from
    session state, never user-supplied
  ```
  (Confirm this matches the intended semantics — the current spec example `box.suspend()` supports this reading.)
- B13: in `#Service-lookup-across-languages`, add:
  ```
  - .use(plugin) populates c.cap (Python) / c.<pluginKey> (TS) at boot,
    not before
  - the capability struct is materialized from the plugin's Context
    before the first handler invocation; it is not a stub
  ```
- B9: in `#Python-SDK`, keep "`__aexit__` guarantees `session.terminate()` on any exception" as is; no spec change needed, code fix only.

**Artifact edits**:
- `cans/artifacts/code/typescript-accumulation.ts`: if the file shows `.use()` binding a `Session & Ctx` that is not materialized, add a note that the pattern requires the plugin context to be bound at `boot()`. Small, illustrative.
- `cans/artifacts/code/python-raii-pydantic.py`: same, `__aexit__` calls `terminate()`.

**Code edits**:
- A11 + B9: `packages/py/kelir/__init__.py`:
  - `Session.__aexit__` → `await self._session.terminate()`.
  - `Sandbox.suspend` in `lifecycle_sandbox.py` → drop the `path` parameter; derive from `session.session_dir() / "suspend.img"` (call into native `session.suspend()` which uses that path).
- A11: `packages/ts/core/src/sandbox.runtime.ts::Sandbox.suspend` → drop the `path` parameter; call `this.booted.suspend(path)` with a path derived from `sessionDir`. Mirror the Rust behavior Agent 2 lands.
- B13 (Python): `packages/py/kelir/__init__.py` — `KelirBuilder.use(plugin)` collects the plugin; `KelirBuilder.boot()` after `native.boot()` calls `native.get_plugin_context(name)` for each registered plugin (new method on the PyO3 bridge — **Agent 1 owns the plugin side; Agent 5 owns the Python side and requests the PyO3 method via `_collab/handoffs.md`**). Capability structs are constructed from the returned dict and assigned to `session.cap` — no more stub classes.
- B13 (TypeScript): `packages/ts/core/src/builder.lifecycle.ts::KelirBuilder.boot()` — replace the cast:
  ```ts
  return new Session(await nativeBuilder.boot()) as Session & Ctx;
  ```
  with a loop that reads `nativeSession.pluginContexts` (a new object the NAPI addon must expose) and assigns each onto the `Session` instance:
  ```ts
  const nativeSession = await nativeBuilder.boot();
  const session = new Session(nativeSession);
  for (const { name, ctx } of nativeSession.pluginContexts ?? []) {
      (session as unknown as Record<string, unknown>)[name] = ctx;
  }
  return session as Session & Ctx;
  ```
  The NAPI `pluginContexts` is a `_collab/handoffs.md` request to Agent 1 (plugin side).

**Definition of done:** `Sandbox.suspend()` takes no argument in Rust, Python, TS; `__aexit__` calls `terminate()`; both non-Rust SDKs materialize plugin capability structs at boot (or, if the NAPI/PyO3 additions slip, the handler context throws `EConfig` with a specific "native bridge missing" message instead of returning undefined/stub — that is an acceptable intermediate but must be marked as such). `pnpm -r exec tsc --noEmit` green; `mypy packages/py` green.

## 5. Coordination and handoffs

Agents use `_collab/handoffs.md` (one line each, format below):

```
- from: agent-<id> | to: agent-<id> | file: <path> | request: <one-line ask> | blocking: yes|no
```

Pre-agreed handoffs in this work order (no file edit needed):
- Agent 1 provides `PipelineEngine::dispatch_lifecycle`, `dispatch_memory_pressure`, `dispatch_heartbeat`, `dispatch_watchdog` — Agent 2 calls them.
- Agent 1 provides `impl Clone for TrapEntry` in `builder_sdk.rs`? **No** — that file is Agent 4's. Agent 4 adds it; Agent 2 uses it.
- Agent 1 provides `pluginContexts` on the NAPI/PyO3 native session handle; Agent 5 consumes it.
- Agent 3 provides `StoragePlane::quota()`; only Agent 3 uses it (`FuseWorker::new` lives in the same crate — Agent 3 owns both).

Any other cross-agent need: `_collab/handoffs.md`, then proceed on the non-blocked portion.

## 6. Conflict log format

`_collab/conflicts.md`, one line per entry:

```
- status: unresolved | file: <path> | issue: <one-line description of the conflict>
- status: resolved-by-precedent | file: <path> | issue: <desc> | precedent: <pointer to canonical home>
```

Reviewer resolves unresolved entries. Agents never overwrite one side to make the other pass.

## 7. Merge order (if merging sequentially is required)

If the reviewer merges branches one at a time (to re-run gates after each), use this order to minimize re-work:

1. Agent 1 (interface + plugins) — largest blast radius; everything else builds on it.
2. Agent 3 (storage FUSE) — independent of Agent 1 and Agent 2.
3. Agent 4 (driver + terminal + CLI) — independent of Agent 1 and Agent 3.
4. Agent 2 (session lifecycle) — depends on Agent 1's dispatch methods and Agent 4's `Clone for TrapEntry`.
5. Agent 5 (SDK parity) — depends on Agent 1's NAPI/PyO3 `pluginContexts` and Agent 2's `Sandbox::suspend` shape.

If merged in parallel (recommended), the reviewer resolves conflicts in the order above during the rebase of each branch.

## 8. Definition of done for the work order (all agents)

- Every A-series and B-series item from the audit is landed (code + spec + artifact) or deferred to `_collab/conflicts.md` with a reviewer action.
- `just pre-flight` green on the merged branch.
- `cans check` full (not just `--refs-only`) green.
- `kelir-plugin-cli parity run vectors.json` still passes for the plugin_builder case family (Agent 1 must extend `vectors.json` if new plugin contexts change hook chains).
- `cargo test --workspace` green.
- `pnpm -r exec tsc --noEmit` green.
- `mypy packages/py` green.
- No file in `crates/*/src` or `packages/*/src` exceeds 1000 LOC (`just check-hygiene` asserts this).
- No new `// TODO (L3)` beyond the pre-existing set (count is machine-checkable: `grep -c '// TODO (L3)' crates packages | diff` against the baseline commit's counts).

---

## Appendix: issue ID → agent map

| Issue | Agent |
|---|---|
| A1, A2, A4, A7, A9, A12, B1 (engine side), B2, B10, B11 (plugin config), B12 | Agent 1 |
| A10, B1 (callsite), B3, B4, B5 | Agent 2 |
| A5, A6, B6, B7, B8 | Agent 3 |
| A3, A8, A13 | Agent 4 |
| A11, B9, B13 | Agent 5 |
```

---

**Note on the A3 conflict:** `SuspendReason::External` is the one place where I did not pre-decide the canonical shape. The audit shows spec says `External(u32)`, code says `External(ExternalSuspendKind)`, and the artifact `cans/artifacts/code/vmm-driver-trait.rs` is the tiebreaker — but since the artifact wasn't fully visible in the input, Agent 4 reads it first and either aligns code to artifact (no spec edit) or files the conflict. This is the AGENTS.md "Never silently resolve conflicts" rule applied to the one genuinely ambiguous case.
