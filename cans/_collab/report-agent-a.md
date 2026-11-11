# Report — agent-a (Rust engine/SDK/binding unification), branch temp/agent-a

Scope: `crates/kelir/**`, `crates/kelir-core/**`, `crates/kelir-interface/**`, `crates/kelir-bindings/**`. No edits outside scope. No cans/, justfile, or CI edits. Commits: `7f131a5` (unify trap pipeline + lifecycle + plane configs, +120/−417) and `10ed475` (trap filter validation, +38/−5). Net −264 LOC.

## Fix 1 — one trap pipeline (verified ✔ / done ✔)
- Audit verified against live tree: `Registration.traps`/`TrapEntry` was consumed only by `Session::dispatch_trap`'s own local loop; nothing in the tree called that SDK-level loop; the engine chain was the live path (used by the same method's tail, kelir-core tests, fused path).
- Design decision: **the engine path (HookRegistry → bind_trap_scoped → PipelineEngine::dispatch_trap_inner) survived**; it deletes the most code and the typed-Plugins TrapEntry path is fundamentally unbindable into the engine (erased `&HandlerCtx` at engine dispatch vs owned typed `HandlerCtx<Plugins, Params>`).
- Deleted: `TrapOutcome`, `IntoTrapOutcome`, `TrapFuture`, `BoxedTrap`, `TrapEntry` (+narrow), `Registration.traps` (+ its narrow/Default wiring), the SDK `Session::dispatch_trap` local loop and the `ctx.clone().retag::<NoParams>()` double retag; `Registration` de-generified (no `Plugins` param, `narrow()` deleted).
- Host `.trap(filter, handler)` (builder_plugin.rs, Host-mode impl per spec "Rust trap surface stays host-only") now wraps the closure with a `filter_matches(&FilterSnapshot, …)` check inside the existing `TrapClosureAdapter` — same handler shape as plugin-mode `.on_trap` (`Fn(&TrapEvent, &HandlerCtx) -> Result<Verdict, PluginError>`), same registration container (`HookRegistry.trap`), same dispatch point. Plugin-mode `.trap(|scope| …)` scope verb unchanged (spec pins the E0592 split; recorded in conflicts.md — full same-arity unification needs a spec change).
- Seal-time validation for the surviving path: `.trap()` is fallible (like `.plugin()`), EConfig on empty `binary`/empty `args` (`validate_trap_filter`). bind_registry stays infallible because the `KelirPlugin::register_interceptors` trait (interface SSOT) is infallible.
- Tests: no test pinned the old dual behavior (verified); added `host_trap_filter_registers_on_engine_ring` pinning the unified registration + the EConfig rejections.

## Fix 2 — lifecycle duality (verified ✔ / done ✔, deletion chosen)
- Deleted `PipelineEngine::{on_vm_init,on_vm_ready,on_vm_shutdown}` fields + `set_on_vm_init/ready/shutdown` + `take_lifecycle_callback` + the `LifecycleCallback` type — dead (no builder/binding caller; the rust-plugin-builder.rs artifact comment already declared them "supervised by the lifecycle vec and delete").
- `run_lifecycle_hook` rewritten: single `dispatch_lifecycle` pass with the watchdog probe, then the honest warning only when `enable_watchdog && VmInit && !probe_armed`. The misleading "no callback registered" warning is gone (~30 LOC deleted).
- Reorder: `boot_with_engine` now leaves the session in `Initializing`; `Session::boot`/`boot_with_plugins` advance to Running themselves (the `lifecycle_boot_completes_to_running` pin stays green); the SDK `finish_boot` runs VmInit → VmReady → `Session::begin_running()` (new pub method). Bindings (napi/pyo3 use boot_with_plugins) unchanged.

## Fix 3 — config convergence (verified ✔ / done ✔, moderate per artifact pins)
- SDK `TokenShield` deleted; `PtyConfig.token_shield: Option<kelir_core…TokenShieldConfig>` (canonical core type consumed directly, re-exported from terminal_plane). finish_boot token-shield conversion deleted.
- `SnoopRule` + `SnoopHandler` + `retag_snoop`/`narrow_snoop` + the `snoops` field + `StoragePlaneConfig<Plugins, Params>` generics all deleted (never consumed); `StoragePlaneConfig` is now a flat value config over core types; `.storage()` verb shape otherwise unchanged.
- `LockBrokerConfig.default_mode` + `LockMode` deleted (verified unconsumed); `scope` + `non_blocking` kept (consumed).
- `Mtls` + `L4Proxy.inject_mtls` deleted; `// TODO (L3): wire mTLS injection at the host connection boundary per cans/07-wire-plane.md.` left at the finish_boot l4_proxies conversion site. `SinkholeConfig`/`WireConfig`/`TapFilter` kept (artifact-example-pinned shapes).
- SDK `StorageHandle` deleted (dead). `kelir::driver_interface` facade module deleted (5-of-~20 re-export, zero consumers). Bindings verified to construct the canonical core types directly — **zero mechanical binding changes required**.

## Fix 7 — TrapFilter.pattern (verified ✔ / done ✔)
- `TrapFilter { binary, args, pattern: Option<String> }` + `TrapFilter::pattern(glob)` matching the TS shape (read `packages/ts/core/src/trap.protocol.ts`/`builder.lifecycle.ts`). `From<&TrapFilter> for FilterSnapshot` wires `pattern → FilterSnapshot.pattern` at `.trap()` registration; matching semantics stay `// TODO (L3)`.

## Fix 5 — entropy argv[1:] (verified ✔ / done ✔)
- Spec text verified (`cans/plugins/supply-chain.md#Entropy-analysis`: "Computes Shannon entropy of argv[1:]"). `entropy_veto` now passes `event.argv.get(1..).unwrap_or(&[])`.

## Skipped (audit claim wrong)
- `FfiQueueSaturated` ↔ `EFFIQUEUESATURATED_CODE` "double-counts the U": wrong — `cans/artifacts/code/messages.rs` has no such code, `case-to-test-map.yaml` pins the exact value, and the name follows the `E+UPPER(variant)` pattern of `EWXVIOLATION`/`EAMBIGUOUSMUTATION`. No change.

## Gates (all run, scoped `-p kelir -p kelir-core -p kelir-interface -p kelir-pyo3 -p kelir-napi --all-targets`, `--profile dev-min`)
- cargo check ✔ · clippy `-D warnings` ✔ (0 errors) · `cargo fmt --all -- --check` ✔ (only my files touched by fmt) · cargo test ✔ — every suite green including trap_dispatch_protocol_test (16) and fluent_builder_plugin_test (11). Pre-existing/environmental failures (verified pre-existing on base via `git stash` re-run): `kelir-pyo3` bp_04 ("built extension module missing after cargo build" — needs the built .so in this sandbox). An initial 5-test `lifecycle_state_test` catalog-open failure was transient (missing `/home/z/.kelir` catalog db at first run; created by a later suite; all 8 pass now).
- Comment-law greps on my crates: zero violations (see/TODO/SAFETY only, no doc comments, no unwrap/expect/panic/todo!/unimplemented!/unreachable!).
- `cargo run -p kelir-interface --bin codegen` ✔ — **zero churn** in packages/ (codegen parses driver_interface/fusion_plugin/plugin crates only, not the SDK builder files); nothing to commit from codegen.
- LOC: all touched files ≤ 975 lines (builder_plugin.rs 975, builder_sdk.rs 782, core session_lifecycle.rs 878).

## Merge-lead notes
- API breaks contained to the `kelir` crate public surface (verified zero in-tree consumers outside my scope, incl. plugin-cli/testkit/integration): `.trap()` now fallible + erased-ctx handler; `Session::dispatch_trap` returns `Verdict`; `StoragePlaneConfig` non-generic without snoops; `PtyConfig.token_shield` is core `TokenShieldConfig`; `L4Proxy` has no `inject_mtls`; `kelir::driver_interface` module and `StorageHandle` gone; core `Session::begin_running()` added; `boot_with_engine` returns Initializing.
- Artifact refresh needed in cans/ (my lane can't edit): `cans/artifacts/code/rust-plugin-builder.rs` (old `.trap` typed signature at ~1033; lifecycle-callback deletion already anticipated by its own comment) and `cans/artifacts/examples/autonomous-swe-agent.rs` / `multi-vm-swarm.rs` (TokenShield/SnoopRule/Mtls/LockMode/StorageHandle pins). All logged in `cans/_collab/conflicts.md`.
