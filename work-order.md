Are you sure current L1 and L2 is in enough coverage for the L3 in future?

# Short answer

**No — mostly yes for plumbing, but not entirely.** The L1/L2 tree gives roughly 80% of the structural coverage L3 will need. There are **~8 concrete L1/L2 gaps** that will require interface changes or new dispatch methods during L3, not just TODO body-fills. The biggest one is the **fused trampoline callback mechanism**, which is entirely missing from the L1 contract even though it's a headline feature.

Below is what I actually verified in the tree.

## Critical L1 gaps (interface-level; L3 cannot fill these cleanly)

### 1. Fused trampoline has no runnable callbacks anywhere in Rust
- `FusedTrampolineDescriptor` / `HookEntry` (`crates/kelir-interface/src/fusion_plugin.rs`) carry only metadata: `ordinal`, `stage`, `plugin_name`, `filter`, `lifecycle_tag`, `custom_action_name`, `vsock_kind`. **No `run`.** The TS/Python `HookDescriptor.run` is dropped at `convert_descriptor` (`crates/kelir-bindings/napi/src/fusion_plugin.rs:63-98` and `pyo3/src/fusion_plugin.rs:78-118`).
- `PipelineEngine::fused_crossing` (`crates/kelir-core/src/dispatch_plugin/mod.rs`) only does `try_foreign_submit()` and returns a guard; nothing is invoked.
- `NativeKelirBuilderBinding.fuseHooks(rings)` (`packages/ts/core/src/fusion.plugin.ts`) accepts only the descriptor. `NativeKelirBuilder::fuse_hooks` stores it and nothing else.
- **L3 will need:** a callback registry keyed by `(ring, ordinal)` inside the engine, a threadsafe-function registration surface on the NAPI/PyO3 builders, and an actual invocation in every dispatch method's `fused_crossing` branch. That's a new field on `PipelineEngine`, new FFI signatures, and probably a new trait. This is the single largest gap.

### 2. `SpawnBuilder` cannot send — no transport/session handle
`SpawnBuilder` (`crates/kelir/src/session_lifecycle.rs`) has `session_id: Option<[u8; 16]>` and `stream_kind`, but no reference to a `Session`, `BusDispatcher`, or transport. `poll()` will have to reach the session, but there is no session registry keyed by `session_id` anywhere. `Session::spawn_builder` returns `SpawnBuilder::default()...bind(session_id, Exec)` with nothing else attached.
**L3 will need** to either store an `Arc<…>` transport in `SpawnBuilder`, or restructure `Session::spawn` to return a future that captures the session. `NativeSessionHandle::spawn`/`execInGuest` and `PySessionHandle::spawn`/`exec_in_guest` also can't delegate cleanly until this is resolved.

### 3. Params plumbing is broken end-to-end
- `SpawnBuilder::params<T>(self, _params: T) -> Self` returns `self`; **no `Serialize` bound**, no encode.
- `HandlerCtx::params<T: DeserializeOwned>()` reads `params_buffer`, which the engine never populates (see `Session::dispatch_trap` — it passes the caller's ctx through with whatever buffer it already had).
- The `PipelineEngine::dispatch_trap_inner` never sets `params_buffer` per-exec.
**L3 will need** `SpawnBuilder::params<T: Serialize>`, plus a lookup table (`exec_id`/`stream_id` → serialized params) somewhere on the session, plus a way to hydrate `HandlerCtx` from it. The `params_buffer: Vec<u8>` field exists, but the wiring around it doesn't.

### 4. Engine has no `dispatch_fs_lookup` / `dispatch_fs_read` / `dispatch_fs_lock`; FUSE bypasses the pipeline
- `PipelineEngine` exposes `dispatch_fs_write` only. `FsInterposer::on_fs_lookup`, `on_fs_read`, `on_fs_lock` exist on the trait.
- `FuseDispatcher::dispatch` (`crates/kelir-core/src/planes/storage/fuse_storage.rs`) calls a **single** `Arc<dyn FsInterposer>` directly. So FUSE lookups/reads/locks don't go through scope filtering, don't go through `fused_crossing`, and don't respect multiple mounted interceptors.
- `LockBroker::acquire` (`crates/kelir-core/src/planes/storage/lock_broker.rs`) also calls `interposer.on_fs_lock` directly.
**L3 will need** `dispatch_fs_lookup`/`dispatch_fs_read`/`dispatch_fs_lock` on the engine, and either `FuseDispatcher`/`LockBroker` to take the engine, or a new `PipelineBinder`-style bridge. That's an L1 change to the plane constructors.

### 5. `HandlerCtx::receipt()` has no receipt store
`HandlerCtx::receipt()` returns an all-zero `ExecutionReceipt`. The struct has `session_id`, `pid`, `trap`, handles, and contexts — **no receipt registry**. There is no per-session or per-pid receipt store anywhere in the tree (search: the only `ExecutionReceipt` construction is in `handler_ctx`, plus `uart_terminal::decode_early_boot_exit`). `Session` also has no accessor.
**L3 will need** a receipt store (probably per-session, keyed by pid/task_id) and either a field on `HandlerCtx` or an accessor through `Session`. That's an L1 change to `HandlerCtx` (or at minimum to its constructor in `kelir-core`'s session path).

### 6. `PipelineEngine` has no accessor for its state, so `Session::fork` cannot rehydrate host callbacks
`kelir/src/session_lifecycle.rs` `fork()` says explicitly:
> `// TODO (L2): host on_trap_deny and custom-action callbacks are consumed into the parent engine at boot and are not Clone; child inheritance lands with the engine registration-state accessor`

`PipelineEngine` has no getter for `on_trap_deny`, `custom_actions`, `stream_kind_services`, or `lock_broker`/`token_revoker`. `Session::fork` currently re-registers plugin `instances` from `Registration`, but the host-mode callbacks are lost. That's an **L2 gap that L3 cannot fill without touching `PipelineEngine`**.

### 7. `dispatch_host_pressure`, `dispatch_host_keyring_access`, `dispatch_memory_pressure` skip `fused_crossing` — and one TODO is stale
- `dispatch_host_pressure` and `dispatch_host_keyring_access` iterate `host_interceptors` with no `fused_crossing`. But `TrampolineRing::HostPressure` and `TrampolineRing::HostKeyringAccess` **do exist** in `fusion_plugin.rs`.
- `dispatch_memory_pressure` has `// TODO (L3) [telemetry_audit]: no TrampolineRing tag exists for memory-pressure telemetry` — but `TrampolineRing::MemoryPressure` **does exist**. The comment is wrong, and the dispatch skips `fused_crossing` for no reason.
This is an L2 inconsistency; L3 will either have to live with it (inconsistent FFI accounting) or fix the dispatch methods. Either way it's not "just a TODO fill."

### 8. `MountScope::allows_binary` is a stub that disables scoped plugins
```rust
pub fn allows_binary(&self, _binary: &str) -> bool {
    self.binaries.is_none()
}
```
`plugin-container` sets `with_binaries([NEWUIDMAP, NEWGIDMAP, PODMAN, runtime])` in `default_scope`. `TrapGateEvaluator::gate_2_scope_match` returns `allows_binary(...) && allows_path(...)`, so **the container trap interceptor never runs for any binary in L2**, and `trusted_prefix_skip` never fires because `is_trusted` is also `false`. This is fixable in L3, but it means L2 tests can't be representative for scoped plugins.

## Secondary L2 issues that L3 will hit
- `runtime_block::block_on` is used across the PyO3 bindings for driver operations that will eventually do real async I/O. Whether the `ThreadWaker`-based block_on is sufficient for L3 driver semantics is unproven.
- `Session::boot_with_engine` swallows `DriverError::Unsupported` to `None` for the driver, so `driver = None` and `Session::boot` still succeeds. The path to `NoHardwareVmm`-abort exists but `DriverRouter::load_native_driver` always returns `Unsupported`. L3 will flip that, but you need to be sure `Session` construction still does the right thing when the driver is `None` only in test scenarios.
- `PyTerminalHandle::read_from`, `notice`, `PyStepHandle::last`, `PyVmHandle::map_shared_region`, `PySessionHandle::console` are all hard-Err TODOs — fine, but they sit behind public Python surfaces the user will call from day one.
- `HandlerCtx::new` never wires `vm`, `fs`, `step`, `terminal` to the live session; acknowledged TODO. `HandlerCtx::erased()` clones the params buffer but not session wiring. L3 will need `HandlerCtx` to be reachable from a live `Session`.

## What is in good shape
- Every plane (`terminal`, `wire`, `storage`, `checkpoint`, `host`) has a real struct with real fields and a clear construction path. `StorageConfig`, `WireWiring`, `TerminalWiring` all have concrete shapes and the builder already threads them through `PlaneWiring`.
- `BusDispatcher` genuinely routes stream frames via bound `StreamFrameSink`s; `bind_stream_kind_sinks` is wired in `boot_with_engine`.
- The dispatch pipeline for `trap`, `process_exit`, `fs_write`, `net_in/out`, `console_*`, `exec_*`, `lifecycle`, `heartbeat`, `watchdog`, `host_lsm_exec`, `pre/post_commit` is all present with `quarantine_*` + `fused_crossing`. That's the bulk of the work.
- `PluginError`, `Verdict`, `FilterAction`, `MountScope`, `KelirPlugin`, `PipelineBinder`, `VsockServiceHandler`, `LockBrokerCoordination`, `ParkedConnectionReset`, `TokenRevocation` are all defined with real trait methods.
- Session lifecycle (`LifecycleState`, `advance_lifecycle`, `run_lifecycle_hook`, `rollback_to`, `suspend`/`resume`, `teardown_catalog`) is real, not stubbed.
- Builder `Seal`/`Unsealed`/`Satisfies` HList machinery, plugin registration, vsock/scope validation, and `boot_with_engine` are all in place.
- The codegen (`codegen/main.rs`) and parity testing (`parity_cli`, `vectors_testkit`) are real and already check wire-format, frame-shape, and builder-ordinal consistency.

## Bottom line
The L1/L2 gives strong coverage for the **bulk of dispatch, planes, lifecycle, builder/plugin registration, and the FFI surfaces**. It does **not** currently give L3 a complete contract for:

1. Fused trampoline callback registration + invocation (biggest gap — feature-defining),
2. `SpawnBuilder` transport binding,
3. Params serialization/deserialization path,
4. `PipelineEngine` dispatch for `fs_lookup`/`fs_read`/`fs_lock` (FUSE + lock broker currently bypass the engine),
5. Receipt store on `HandlerCtx`/`Session`,
6. `PipelineEngine` state accessor for `Session::fork`,
7. Fused-crossing consistency for `host_pressure`, `host_keyring_access`, `memory_pressure`,
8. `MountScope::allows_binary` and `is_trusted` stubs that currently disable scoped plugins in L2.

Items 1–6 will require touching L1 types or L1 method signatures. Items 7–8 are L2 corrections that can land before L3 but shouldn't wait until L3. I'd add a small "L3-blocking" note to each of these in `cans/` or the work-order so L3 doesn't discover them mid-implementation.