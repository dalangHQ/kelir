# Redundancies & Uncohesiveness Audit

## Critical redundancies (behavioral duplication)

### 1. Two trap-registration pipelines on the same `KelirBuilder`
- `crates/kelir/src/builder_plugin.rs::on_trap` pushes into `HookRegistry.trap: Vec<Arc<dyn TrapInterposer>>`; this is bound into the engine via `bind_registry` → `engine.dispatch_trap`.
- `crates/kelir/src/builder_sdk.rs::trap(filter, handler)` pushes into `Registration.traps: Vec<TrapEntry<Plugins>>`; this is **never bound into the engine** but is instead invoked by `Session::dispatch_trap`'s own local loop (`crates/kelir/src/session_lifecycle.rs`).
- Net effect: a user calling `.on_trap(...)` and `.trap(...)` registers two independent chains that fire in an undocumented order (registration traps first, then engine traps). `bind_registry` also never validates `Registration.traps` at seal.

### 2. `PipelineEngine::on_vm_init/on_vm_ready` callback fields are dead
- `crates/kelir/src/builder_plugin.rs::on_vm_init` pushes a `LifecycleClosureAdapter` into `registry.lifecycle`, which is dispatched by `engine.dispatch_lifecycle`.
- `PipelineEngine::set_on_vm_init` / `take_lifecycle_callback` (in `crates/kelir-core/src/dispatch_plugin/mod.rs`) are the *other* lifecycle path — never invoked from any builder. `run_lifecycle_hook` therefore always hits the `else` branch and emits the misleading warning:
  > `enable_watchdog=true reached on_vm_init with no callback registered to arm ctx.set_watchdog`
  even when a `LifecycleParticipant` did arm the watchdog probe (the probe is discarded after `dispatch_lifecycle` returns).

### 3. `L4Proxy.inject_mtls` silently dropped
- `crates/kelir/src/planes/wire_plane.rs::L4Proxy` carries `inject_mtls: Option<Mtls>`.
- `crates/kelir/src/builder_sdk.rs::finish_boot` converts to `(listen_port, destination)` only, throwing away the mTLS material. The spec (`07-wire-plane.md`) advertises mTLS injection at the host connection boundary — the wiring does not.

### 4. `SpawnBuilder::params()` is a no-op
- `crates/kelir/src/session_lifecycle.rs::SpawnBuilder::params<T>` takes `_params: T` and returns `Self` without storing it (`// TODO (L3)`). `Session::spawn` therefore cannot carry typed params to the guest, contradicting `Params-typed-views` (`03-sdk-architecture.md`).

### 5. `compute_shannon_entropy` uses `argv[0]`
- `crates/kelir-core/src/trap_gate.rs::compute_shannon_entropy(&event.argv)` iterates the full argv. Spec (`plugins/supply-chain.md#Entropy-analysis`) says entropy is computed over `argv[1:]`. Minor but spec-divergent.

### 6. `L4Proxy` mTLS, `SnoopRule`, `LockBrokerConfig.scope`/`default_mode`, `StorageHandle` (in `kelir::session_lifecycle`) — all dead spec-shape types that no code path consumes.

---

## Duplicated types across layers

| Concept | SDK crate (`kelir`) | Core crate (`kelir-core`) |
|---|---|---|
| Terminal config | `PtyConfig` + `TokenShield` | `TerminalWiring` + `TokenShieldConfig` |
| Wire config | `WireConfig` + `SinkholeConfig` | `WireWiring` |
| Storage config | `StoragePlaneConfig` + `SnoopRule` + `LockBrokerConfig` | `StorageConfig` + `LockBrokerWiring` |
| Plane modules | `storage_plane`, `terminal_plane`, `wire_plane` | `storage`, `terminal`, `wire` |
| Session type | `kelir::Session<Plugins, Params>` | `kelir_core::Session` |
| Session handles | `StorageHandle`, `WireHandle`, `VsockHandle`, `VsockChannel`, `SpawnResult` (in `kelir::session_lifecycle`) | `StoragePlane`, `WirePlane`, `HostPlane`, `CheckpointPlane` (no handle wrappers) |
| Trap filter | `TrapFilter { binary, args }` (no `pattern`) | `FilterSnapshot { binary, args, pattern }` |

The conversion happens ad-hoc in `finish_boot`; the SDK-only types (`SnoopRule`, `LockBrokerConfig`, `StorageHandle`) have no consumer.

Additionally, `crates/kelir/src/driver_interface.rs` is a facade that re-exports only 5 of the ~20 types in `kelir-interface::driver_interface` — two import paths for the same `VmmDriver`.

---

## Duplicated surface areas on the builder

`KelirBuilder` now exposes **three** ways to register a trap:
1. `.on_trap(f)` (`builder_plugin.rs`) — adapter pushes to `HookRegistry.trap`.
2. `.trap(filter, handler)` (`builder_sdk.rs`) — pushes to `Registration.traps` (different dispatch).
3. `.trap(|scope| scope.on_trap(f))` (`builder_scopes.rs`, plugin mode only) — a scope wrapper over (1).

`.trap(...)` itself has **different signatures per mode**:
- Host mode (`builder_sdk.rs`): `trap(filter: impl Into<TrapFilter>, f: Fn(HandlerCtx<_, Params>) -> ...)`.
- Plugin mode (`builder_plugin.rs`): `trap(configure: FnOnce(TrapScope) -> TrapScope)`.

A `.trap("git", ...)` written for host mode won't compile on a plugin-mode builder; the spec claims identical verbs in both modes.

---

## Redundant validation utilities in `kelir-plugin-cli`

| Concern | Duplicate implementations |
|---|---|
| TOML manifest parsing | `audit_cli::run` (via `toml::from_str`), `graph_cli::manifest_name`/`path_deps` (line-prefix scan), `fixtures_cli::parse_fixtures` (manual `split('=')`) |
| `perf/budgets.toml` validation | `budgets_cli::check` (`enforce` + `REQUIRED_KEYS`) and `scorecard_testkit::validate_scorecard_budgets` (`floors`/`rules`) |
| `case-to-test-map.yaml` | `check_manifest.rs::CaseMap` and `check_tests.rs::CaseMap` (separate structs, separate parsing) |
| Vectors format | `vectors_testkit::VectorCase` (serializer) and `parity_cli::ParityVector` (deserializer) — no `VectorCase::from_json`; `parity_cli` re-parses manually and `expected_verdict_kind`/`event_kind` are dead fields |
| `check` subcommand | Both `check --layers` and top-level `check-layers`; same for `--budgets`/`check-budgets` |

`parity_cli::parse_vectors` is ~200 lines of manual `serde_json::Value` walking that shadows the already-typed `VectorSuite`.

---

## Redundant / dead fields in `kelir-core` and `kelir`

- `StoragePlane::with_io` exists; `UnimplementedStorageIo` errors on every op. `punch_hole_sparse_upper` therefore always fails.
- `TerminalPlane::prompts`/`output_filters`/`token_shield` are stored but no dispatch consumes them (`feed`, `apply_token_shield`, `filter_ansi_shadow` are all TODO).
- `CheckpointPlane::StepSlot.pending_delta` is never set; `create_step` never records a delta; `commit_step` therefore always stores `delta: None`.
- `HostPlane::subscribe_pressure`/`poll_pressure_events`/`reclaim`/`arm_host_reaper` are all `Err(TODO)`; `HostPlane` is effectively a `reaper_kind` + `metrics` container.
- `kelir_core::Session::export_snapshot` is on `Session` while `import_snapshot` is on `VmHandle` — asymmetric surface.
- `bump_core_cli::resolve_pin` uses `env!("CARGO_PKG_VERSION")` (the CLI crate's version) as the "kelir core version" pin.

---

## Uncohesive conventions

### Error handling
- `thiserror` in core/interface (`PluginError`, `VmmError`, `CatalogError`, `WireError`, `OtlpError`).
- `Box<dyn std::error::Error>` in most `kelir-plugin-cli` subcommands.
- Silent `Ok(())` in `kelir-plugin-cli` stubs (`logs`, `manifest`, `registry`, `rings`, `new`/`scaffold`, `attest`) vs explicit `Err(TODO)` elsewhere. The silent-success stubs violate the same TODO discipline the rest of the tree follows.

### Naming
- `FfiQueueSaturated` (`PluginError`) ↔ `EFFIQUEUESATURATED_CODE` (`error_envelope.rs`) — the constant double-counts the `U` vs the variant name.
- `kelir::planes::storage_plane` vs `kelir_core::planes::storage` (and `_plane` suffix on the other two); the rest of the SDK drops the suffix.
- `set_lock_namespace` vs `with_*` builder methods on `StoragePlane`.
- `pressure_level() -> u8` and `pressure_level_typed() -> PressureLevel` on `HostPlane`.

### Testkit vector event shapes drift from actual hook signatures
`crates/kelir-plugin-testkit/src/vectors_testkit.rs::plugin_builder_verb_event` emits synthetic events:
- `fs-write` → `{kind, path, pid}` — the hook is `on_fs_write(path, data, pid, ctx)` (missing `data`, `ctx`).
- `exec-stdout` → `{kind, pid, exec_id}` — hook is `on_exec_stdout(pid, exec_id, chunk)` (missing `chunk`).
- `memory-pressure` → `{kind, used, total}` — hook is `on_memory_pressure(available_mb, total_mb)` (wrong key name).

The parity gate does not execute these events, so it does not catch the drift, but the pinned "expected_outcomes" describe a contract the synthetic events don't satisfy.

### `--unified-builder-coverage` case count mismatch
`plugin_builder_verb_cases` includes `"seal"` (host-only) but not `"build"` (plugin-only). Spec (`03-sdk-architecture.md` §Unified-builder-surface) counts `build` among the 30 verbs; the testkit swaps it for `seal`, so the 30-case count only matches by accident.

### Veto/register surface
Three ways to register a veto:
- `kelir::builder_sdk::LifecycleMut::register_veto` (host builder).
- `kelir_interface::plugin_model::LifecycleCtx::register_veto` (hook-time).
- `Registration.vetoes: VetoList` (field, consumed in `finish_boot`).

### `HandlerCtx` type family
Four shapes coexist:
- `HandlerCtx<Plugins, Params>` (typed, generic).
- `HandlerCtx<(), ()>` (`restricted`).
- `HandlerCtx<Plugins, NoParams>` (the closure-erased form used by `BoxedTrap`).
- `HandlerCtx<Plugins, Params>` retagged through `retag::<T>()` (no-op; only `params_buffer` is copied).

### Python / TypeScript / Rust `Verdict::Patch` shapes
- Rust: `Verdict::Patch(PatchPayload)` where `PatchPayload` is `#[non_exhaustive]`.
- TS: `Verdict.Patch(attr: PatchInput)` (object with `kind: "patch"`).
- Python: `Verdict.patch(*args, **kwargs)` with `binary`/`argv`/`env`/`unset_env`/`name`/`cbor`/`attr` keyword bag; `args[0]` is coerced into `attr`. Three different shapes for the same semantic.

### `TrapFilter` divergence
- Rust `TrapFilter`: `binary`, `args` — no `pattern`.
- TS `TrapFilter`: `binary`, `args`, `globPattern` via `.pattern(glob)`.
- Python `TrapFilter`: `binary`, `args` — no `pattern`.
- Fused descriptor `FilterSnapshot`: `binary`, `args`, `pattern` in all three.

Only TS can construct a `pattern`-based SDK filter; Rust and Python cannot produce a non-`None` `FilterSnapshot.pattern`. The wire descriptor's capabilities exceed the SDK's.

---

## Smaller notes

- `Session::stop` in `kelir-core/src/session_lifecycle.rs` bails out of the catalog teardown when `flush_workspace_before_terminate` errors; only the `"needs the anchored reconciler"` message is swallowed, so an L3 `Reconciler` TODO leaves the session row orphaned.
- `Session::boot_with_engine` transitions to `LifecycleState::Running` before `run_lifecycle_hook(VmInit/VmReady)` is called — the state machine says `Running` while `VmReady` hasn't run.
- `Session::dispatch_trap` calls `entry.handler(ctx.clone().retag::<NoParams>())`, so `builder.trap(...)` handlers always receive `NoParams` at the closure boundary; the boxed closure retags to `Params` inside (works, but two retags of the same typed context).
- `StoragePlane::lock_broker_arc` and `fuse_worker` are `OnceLock`s keyed by the first call only; changing the interposer later is silently ignored.
- `python _load_native_bridge` and `ts loadNativeAddon` both hard-code `_kelir` / `./native/kelir.node` and both catch `ImportError` → `EConfig`; the two "missing bridge" error paths are never tested (no L2 tests), so a broken addon ships silently.
- `Session::attach` (Rust/TS/Py) is only implemented enough to validate the id and reject with `TODO`; `relay_socket_path` in both bindings is the only half that works.
- `kelir-plugin-cli` depends on `driver-rustvmm` solely for `host_probe::os_hypervisors` and `device_present`. That coupling belongs in a small `kelir-host-probe` crate — `kelir-plugin-cli` is otherwise platform-agnostic tooling.
- `check_tests.rs::is_tautology` scans each *line* of Rust tests for `assert_eq!`/`assert!` and reconstructs paren nesting; it cannot catch tautologies split across lines, and it fires on any `assert_eq!(foo, foo_bar())` where `foo_bar` is unrelated. The heuristic is loose in both directions.

---

## Suggested highest-impact fixes

1. Collapse `on_trap` and `trap(filter, handler)` into one registration path — either drop `TrapClosureAdapter` and route everything through `TrapEntry`/`Registration.traps`, or drop `TrapEntry` and route everything through `HookRegistry`. As-is, the two paths dispatch at different points.
2. Wire `PipelineEngine::set_on_vm_init/ready/shutdown` OR delete the dead fields and make the warning logic honest.
3. Move `TrapFilter`/`FilterSnapshot`, `PtyConfig`/`TerminalWiring`, `WireConfig`/`WireWiring`, `StoragePlaneConfig`/`StorageConfig` conversions to a single `kelir-interface` shared type, or delete the SDK-only layer.
4. Add a `VectorCase::from_json` and delete `parity_cli::ParityVector`; pin the roster (`vectors_testkit::fixtures`) and the artifact to one parser.
5. Replace `"seal"` with `"build"` in `plugin_builder_verb_cases` and stop advertising `build` as a chain verb if it is terminal-only — but at least keep the 30 count aligned with the spec.
6. Use `toml::from_str` in `graph_cli` and `fixtures_cli`; delete `budgets_cli::REQUIRED_KEYS` in favor of `scorecard_testkit::Budgets`-driven validation (one file, one parser).
7. Add `pattern: Option<String>` to `TrapFilter` in Rust and Python, or delete `TrapFilter.pattern` from TS — the wire descriptor already carries it.