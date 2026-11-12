# CHAIN-A3 decisions (Rust track, L1/L2)

Record of decisions taken while fixing the Rust builder chain on branch
`chain-a3`. Spec pins live in `cans/` and are A1's job; nothing here edits
`cans/`. Live code behaviour that contradicts an artifact is kept as-is
(item 7 below) and flagged for A1.

## 1. `plugin()` returns `Self`, duplicate check moved to `seal()`

`plugin()` no longer returns `Result`. The duplicate-name check now runs
in `seal()` with the same `EConfig` / `plugin already mounted: <name>`
diagnostic, alongside the existing mounted vsock/scope checks.

## 2. Host `trap` / `trap_async` / `post_exec` return `Self`

Empty-filter validation is deferred to `seal()`. Each filter-form verb
records its `TrapFilter` in `HookRegistry.trap_filters`; `seal()` validates
the host registry's filters and every mounted plugin's filters, using the
mounted plugin's name in the diagnostic. Same rules as before (empty
binary, empty arg); nothing else changed.

## 3. `LifecycleMut` chains and exits; `lifecycle()` closure form

- `register_veto` now takes `self` and returns `Self`, so vetoes chain.
- `LifecycleMut` now holds `&mut KelirBuilder<Host, Unsealed, ...>` (it
  previously held only `&mut VetoList`) and gains `end(self) -> &mut
  KelirBuilder<...>`, so the caller gets the builder back.
- The `lifecycle(|l| ...)` closure form already existed (audit claim
  overstated: it was present, taking `LifecycleScope`); it was missing
  veto registration, so `LifecycleScope::register_veto` was added and the
  closure form now requires `.end()`, like the other scopes.

## 4. All 7 scopes gain `.end(self) -> KelirBuilder<...>`

Trap, Fs, Net, Console, Exec, Host, Lifecycle, Telemetry scopes all have
`end()`. The scope-entry closures (`trap_scope`, `fs`, `net`, `console`,
`exec`, `host`, `lifecycle`, `telemetry`) now require the closure to
return the builder, i.e. the closure must end with `.end()`; a scope can
no longer be left dangling. Call sites updated accordingly.

## 5. `trap` name collision: both modes get both forms

Decision: rename, don't overload. Rust has no overloading, so the two
same-name signatures could not coexist.

- Filter form `trap(filter, f) -> Self` is now defined once, generically
  over `Mode`, so Host and Plugin builders both have it.
- Scope form is renamed to `trap_scope(closure) -> Self`, also generic
  over `Mode`, so Host and Plugin builders both have it.
- The old Plugin-only `trap(closure)` is gone; call sites use
  `trap_scope`.
- `trap_async` / `post_exec` stay Host-only: they are host-ring verbs and
  the live code (compile error in Plugin mode) is kept per item 7.

## 6. `.use()`, `group()`, `guard()`, `CompositeBundle`

- `.use()` is an alias for `.plugin()`, spelled `r#use` (`use` is a
  keyword in Rust). Same signature, same deferred duplicate check.
- `group(f)` applies a `FnOnce(Self) -> Self` to the builder: a named
  block of chain steps. Cheap, added.
- `guard(enabled, f)` applies `f` only when `enabled` is true, else the
  builder passes through unchanged. Semantics chosen by this track: a
  boolean conditional group (Elysia's `.guard()` scopes hooks; there is
  no route table here to scope over, so conditional application is the
  honest equivalent).
- `CompositeBundle`: NO mount verb added, type NOT deleted. Its
  `CompositePlugin::unpack` is an L3 stub returning an empty vec, so a
  mount verb now would silently mount nothing. Mounting a
  `CompositeBundle` is unsupported until L3 fills `unpack`; the type
  stays as the declared shape for that work.

## 7. SSOT discrepancy left alone

The artifact/code mismatch on host-only verbs is A1's job. No code was
changed to match an artifact; Plugin-mode use of Host-only verbs
(`trap_async`, `post_exec`) still fails to compile.

## 8. `boot()`: `seal()?.boot()` is the only route

Decision: delete, don't document-around. `boot()` on
`KelirBuilder<Host, Unsealed, ...>` is deleted; `boot()` exists only on
`KelirBuilder<Host, Sealed, ...>`. Validation lives in `seal()` and
`boot()` is the single terminal. All call sites (library
`sandbox_runtime.rs`, all `crates/kelir/tests/` boot sites) now go
through `.seal()?.boot()`.

## Open gaps (deliberate, not faked)

- `CompositeBundle` mounting: unsupported until L3 fills `unpack` (see 6).
- No other open items: `group()`/`guard()` landed (see 6); every scope
  has `end()`; both `trap` forms exist in both modes.

---

# Round 2 (2026-11-12, branch kelir-nogap-a3, base d090f6e)

## 9. fork_state is plugin-mode-only by type (not Result)

- Choice: the `KelirPlugin` / `PluginClone` / `AnyPlugin` / `PluginDeps` impls for
  `KelirBuilder` are restricted from generic `Mode` to `Mode = Plugin`
  (`crates/kelir/src/builder_plugin.rs`, impl headers at :614, :647, :673, :701).
  A host-mode builder no longer implements `KelirPlugin`, so `.fork_state()` on
  it is a compile error; hosts fork via `Session::fork()`
  (`crates/kelir/src/session_lifecycle.rs:122`), which rebinds mounted plugins
  through their `fork_state`.
- Why type-level, not Result: it compiled cleanly across existing call sites.
  The only breakage was three test sites calling `register_interceptors` on host
  builders (`crates/kelir/tests/fluent_builder_plugin_test.rs:509,542,573`); they
  now call the inherent `bind_registry`, made `pub`
  (`crates/kelir/src/builder_sdk.rs:678`). No production call site mounts a host
  builder or forks one. A `Result` return would have churned the
  `KelirPlugin`/`AnyPlugin` traits in `kelir-interface` plus five plugin-crate
  overrides for a path `seal`/`boot` never calls.
- Regression test: `plugin_mode_fork_state_clones_and_fork_with_wins`
  (`crates/kelir/tests/builder_chain_test.rs`).

## 10. group() / guard(): verified, no change

- `guard(enabled: bool, configure: F)` (`builder_plugin.rs:149`) is
  boolean-conditional-only; `group(configure: F)` (`builder_plugin.rs:142`)
  takes `FnOnce(Self) -> Self`, the same closure-takes-the-builder shape as TS
  `group(fn: (builder: this) => this | void)`
  (`packages/ts/core/src/builder.lifecycle.ts:780`). No divergence, no fix.

## 11. Seal-time checks: verified, no change

- `seal()` (`crates/kelir/src/builder_sdk.rs:428`) already rejects duplicate
  mounts (`plugin already mounted: <name>`) and validates trap filters
  (host label `host`, mounted plugins named), vsock specs, and scopes.
  Missing prerequisites are compile-time in Rust via the
  `Plugins: Satisfies<PDeps>` bound on `plugin()` / `plugin_instance()`; no
  mount-time runtime check exists to move. Host-only verbs exist only on the
  `KelirBuilder<Host, Unsealed, ...>` impl (`builder_sdk.rs:399`); on a
  plugin-mode builder they are a compile error, no runtime check added.

## 12. Instance .plugin() mount alias: verified, no change

- Host `.plugin(builder)` mounts a `KelirBuilder<Plugin, Unsealed, ...>`
  (`builder_sdk.rs:468`); `.use()` (`builder_sdk.rs:498`) delegates to it.
  A string arg is not applicable in Rust (typed). `plugin_instance`
  (`builder_sdk.rs:510`) mounts a `KelirPlugin` instance.

## 13. vectors.json 58 -> 62: generator work, new module

- The four pinned chain cases live in a new module so the 996-line
  `vectors_testkit.rs` stays under 1000 LOC:
  `crates/kelir-plugin-testkit/src/vectors_chain_testkit.rs`
  (`plugin_builder_chain_cases`, seq 55-58, one trap event, `Verdict::Pass`,
  rings `[(Trap, 0)]`, matching `cans/artifacts/code/plugin-builder-vectors.rs`).
  `plugin_builder_case` / `plugin_builder_trap_json` widened to `pub(crate)`;
  `fixtures()` extends with the chain cases.
- Regen is not byte-stable (pre-existing): one frame (`trap-git-commit-pass`)
  churns on regen from the known `protocol_mux` CBOR map-ordering gap
  (`vectors_testkit.rs` TODO). The regen output is generator output, never
  hand-edited hex.

## 14. Lock-broker forwarding: L2 threading complete; live path is a NAMED L3 blocker

- The full `LockBrokerConfig` (`scope` globs + `non_blocking`) is threaded at
  L2: `lock_broker(config)` (`builder_sdk.rs:548`) -> `storage_config()` maps
  it to `LockBrokerWiring` -> `PlaneWiring.storage` -> `boot_with_engine`
  (`crates/kelir-core/src/session_lifecycle.rs:366`) ->
  `StoragePlane::with_wiring` -> `lock_broker_arc`
  (`crates/kelir-core/src/planes/storage/mod.rs:251`) builds
  `LockBroker::with_wiring`, and `StoragePlane::fuse_worker()` (`mod.rs:270`)
  passes that broker into `FuseWorker::new`.
- BLOCKER (named, not faked): `sp_15_host_inode_lock_broker_flock_bridge`
  (red, `cans/artifacts/test-plan/cases/storage_plane/sp_15_host_inode_lock_broker_flock_bridge.yaml`).
  `LockBroker::acquire` is an in-memory table; host flock delegation lives in
  the enclave (`lock_broker.rs:122,171,179`, `fuse_storage.rs:336`,
  `io_storage.rs:112`). Two further dead ends prove the live path is not L2
  wireable: `StoragePlane::fuse_worker()` has zero callers (no FUSE worker is
  ever started at boot), and `PipelineEngine::register_lock_broker`
  (`crates/kelir-core/src/dispatch_plugin/mod.rs:512`) has zero callers and its
  getter zero readers. Registering the broker with no consumer and no
  principled interposer would be dead wiring, so it was not done.

## 15. CompositeBundle mounting: NAMED L3 blocker, spec-pinned unsupported

- The spec pins it: "Mount verb: none; a CompositeBundle has no mount verb and
  mounting one is unsupported" (`cans/02-plugin-model.md#Composite-plugin-hierarchy`,
  restated at `#Chain-decisions`). `CompositeBundle` is a unit struct with no
  fields and `unpack` returns an empty vec with
  `// TODO (L3) [plugin_engine]` (`builder_sdk.rs:194`). A `KelirPlugin`
  delegate would mount an empty tree, so none was added.
- BLOCKER: domain `plugin_engine`, no dedicated case id exists — no test-plan
  case file mentions composite, and the TODO tag carries the domain only. The
  unpack work (nested tree the profile facade flattens at boot) is L3 meat.
