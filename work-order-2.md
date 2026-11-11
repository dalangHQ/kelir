# AI Sloppiness in Kelir Codebase (L1/L2 Audit)

Below: concrete defects that are **not** L3 placeholders — things wired wrong, structurally broken, or semantically dishonest at L2. Order within each section is by blast radius.

---

## 1. L2 wiring bugs (things that pretend to work but don't)

### 1.1 First boot always fails — `~/.kelir/` parent directory never created
`crates/kelir-catalog/src/lib.rs::catalog_path()` returns `$HOME/.kelir/catalog.db` but the parent `.kelir` is never `create_dir_all`'d. `rusqlite::Connection::open` requires the parent to exist, so `Session::boot_with_engine` → `CatalogDb::open(...)` fails on a fresh checkout. Same for `kelir_core::session_lifecycle::session_dir(...)` → `workspace/` and `upper.img` are never mkdir'd, so the first FUSE write or reconcile hits `canonicalize()?` on a missing path.

### 1.2 `Session::fork` throws away the whole trap/lifecycle registration
`crates/kelir/src/session_lifecycle.rs::Session::fork` builds `Registration { traps: Vec::new(), on_trap_deny: None, custom_actions: Vec::new(), stream_kind_services: Vec::new(), vetoes: Vec::new(), .. }`. The child therefore:
- never fires any `.trap()` handler
- never fires `on_trap_deny`
- never dispatches custom actions
- never routes registered stream kinds
- never fires registered vetoes

This is the primary "parallel agent exploration" use case of the spec. `kelir_core::Session::fork` has the same defect — it builds a fresh `PipelineEngine::new()` with zero interceptors.

### 1.3 `Session::fork` loses parent data and never registers the child
`crates/kelir-core/src/session_lifecycle.rs::Session::fork` reuses `self.session_id` for the child while minting only a fresh `incarnation_id`. It never calls `CatalogDb::register_session` for the child. Consequences:
- `teardown_catalog()` on the child: `lookup_session(parent_id)` returns the **parent's** row, `incarnation_id` mismatch → `owned = false` → returns `Ok(())` (good). But when the parent terminates, `lookup_session` returns the parent row → `owned = true` → `unregister_session(parent_id)` + `reap_ephemeral_session_dir(parent_id)`. If a child had written to the shared workspace, that data is silently deleted.
- Two sessions with the same primary key violates `cans/06-storage-catalog.md#Entity-model`.

### 1.4 `stop()` silently skips `on_vm_shutdown`
`crates/kelir-core/src/session_lifecycle.rs::Session::stop` only flushes and tears down the catalog. The `shutdown` lifecycle hook lives exclusively in `Session::shutdown()`. The SDK-facing `Sandbox::stop` calls `session.stop()` — so every `Sandbox::stop` bypasses plugin teardown. Per `cans/08-lifecycle.md#Pre-teardown-flush`, "Terminate flush belongs to `on_vm_shutdown`."

### 1.5 `register_stream_kind` never reaches the engine or the bus
`crates/kelir/src/builder_sdk.rs::finish_boot` moves `registration.custom_actions` into `engine.register_custom_action(...)` but never touches `registration.stream_kind_services`. The registration vector is stored and dropped. On top of that, `crates/kelir-core/src/session_lifecycle.rs` never calls `bus.bind_stream_sink(...)` — the bus is constructed with empty `stream_sinks`. So plugin-registered kinds are unreachable at runtime.

### 1.6 `StreamFrameSink` is duplicated and incompatible
Two byte-identical traits:
- `crates/kelir-interface/src/plugin_model.rs::StreamFrameSink`
- `crates/kelir-core/src/bus/mod.rs::StreamFrameSink`

`PipelineBinder::bind_stream_kind_scoped` accepts the interface trait; `BusDispatcher::bind_stream_sink` accepts the bus trait. A plugin can implement the interface trait but cannot register with the bus. Even if §1.5 were fixed, the trait types wouldn't unify.

### 1.7 `E_NOHARDWAREVMM` is swallowed
`crates/kelir-core/src/session_lifecycle.rs::boot_with_engine`:
```rust
let driver = match crate::vmm::DriverRouter::load_native_driver(...) {
    Ok(loaded) => Some(...),
    Err(DriverError::Unsupported(_)) => { /* warn about unconsumed config */ None }
    Err(_) => None,   // <-- catches NoHardwareVmm, Io, InvalidState, ResourceExhausted
};
```
Per `cans/01-architecture.md#Fallback-policy` ("Absence halts with E_NOHARDWAREVMM"), a `NoHardwareVmm` should abort boot, not silently fall through to `driver = None`. Only `Unsupported` (backend not built) should warn-and-continue.

### 1.8 `MountScope::allows_*` treats `Some([])` as "allow all"
`crates/kelir-interface/src/plugin_model.rs`:
```rust
pub fn allows_binary(&self, binary: &str) -> bool {
    match &self.binaries {
        None => true,
        Some(list) => list.is_empty() || list.iter().any(...),
    }
}
```
An empty `Some(vec![])` is semantically ambiguous. The spec says "VirtIO ring mask: unselected rings bypass plugin completely" — so `Some([])` should mean *no rings allowed*. Current code means *all rings allowed*. Same for `allows_path` and `allows_device`. This defeats scope isolation for a plugin that tries to declare an empty allowlist.

### 1.9 Trap identity is matched on path/basename, not canonical `(st_dev, st_ino, blake3)`
- `crates/kelir/src/builder_sdk.rs::TrapFilter::matches` compares `event.binary_path` (string).
- `crates/kelir-interface/src/plugin_model.rs::MountScope::allows_binary` compares file-name.
- `crates/kelir-core/src/trap_gate.rs::is_trusted` compares path prefix.

`cans/11-protocol.md#Trap-fast-path` explicitly says "argv[0] is untrusted input; it never identifies the binary" and "Identity is (st_dev, st_ino, binary_blake3), never a path string." The L2 wiring silently uses the forbidden identity. Comments say `// TODO (L3): canonical (st_dev, st_ino, binary_blake3)` but L2 behavior is a security hole, not a stub. Should return `false`/`FallThrough` with `Err(Halt("...TODO"))` in the interim.

### 1.10 Vault metadata endpoint rejects every request
`crates/kelir-plugins/plugin-vault/src/lib.rs::is_session_peer` returns `self.session_guest_cid.is_some_and(...)`, but no constructor ever sets `session_guest_cid`. `VaultPlugin::new()` leaves it `None`. So `on_connect` rejects every peer. The `plugin-vault` spec ("Tier 2 governance, default enabled") is dead at L2.

---

## 2. Semantic mismatches with the spec

### 2.1 `phase_spans` order contradicts `cans/10-attestation.md`
`crates/kelir-otlp/src/lib.rs::phase_spans` emits `trap_ingress, policy, ffi_hop, verdict_egress`. The spec field order is `trap_ingress_ns, ffi_hop_ns, policy_ns, verdict_egress_ns` (policy follows ffi_hop). Span labels are therefore mis-assigned — the OTel span named `ffi_hop` is measuring what should be `policy`, and vice versa.

### 2.2 `stop_with_timeout` ignores the timeout
`crates/kelir-core/src/session_lifecycle.rs`:
```rust
pub fn stop_with_timeout(&mut self, timeout: Duration) -> Result<(), PluginError> {
    if timeout == Duration::ZERO { return self.kill(); }
    let _ = timeout;   // <-- discarded
    self.flush_workspace_before_terminate()?;
    ...
}
```
`stop_with_timeout(Duration::from_secs(10))` is identical to `stop_with_timeout(Duration::from_millis(1))`. Per spec, the timeout bounds `MSG_SHUTDOWN_REQUEST` waiting; the wiring pretends to accept a timeout but ignores it.

### 2.3 `is_synthetic_dns` parses UDP frames with `TcpView`
`crates/kelir-core/src/planes/wire/mod.rs::is_synthetic_dns` — after classifying the frame as UDP (`ip.proto() == IP_PROTO_UDP`), it calls `TcpView::new_unchecked(...)` and reads `src_port()`/`dst_port()`. For UDP these overlap correctly, but `TcpView::header_bytes()` (used elsewhere) reads `bytes[12] >> 4` — that's a checksum/payload byte in UDP. The helper is only safe by accident. It should use a dedicated `UdpView` or a generic port extractor.

### 2.4 `resolve_synthetic_dns` returns a fake `ServFail` success
`crates/kelir-core/src/planes/wire/mod.rs::resolve_synthetic_dns` returns `Ok(DnsResponse { wire_bytes: Vec::new(), rcode: ServFail, edns_downgraded: false })`. That's a **successful** `Result` with a synthetic error response — a caller can't distinguish "I tried and the upstream failed" from "I haven't implemented this." Should be `Err(Halt("DNS resolver pending (L3)"))`.

### 2.5 `allow_udp_proxied` never checked
`process_outbound_frame` drops all non-DNS UDP. But `WireWiring` has `allow_udp_proxied: bool`, and `cans/07-wire-plane.md#UDP-NAT-semantics` says permitted UDP egress is forwarded. The field is stored but never read.

### 2.6 `Session::fork` copies the parent's lifecycle `state`
`crates/kelir-core/src/session_lifecycle.rs::fork` does `let state = self.state;` and assigns it to the child. If the parent is `Suspended` or `Draining`, the child — a brand-new driver instance — starts in that state. Should be `Running` (or derived from a fresh driver).

### 2.7 `SyntheticFile::Dynamic` ignores the generator's `mode`
`crates/kelir-core/src/planes/storage/storage_synthetic.rs::lookup_attr` for `Dynamic` returns `mode: 0o400` regardless of `DynamicFile.mode`. `DynamicFile` exists solely to carry `content` and `mode`; the mode is silently dropped.

### 2.8 `FuseDispatcher::dispatch` mis-reports unimplemented ops
- `Getattr` returns `Err(Halt("TODO..."))` instead of falling through to the broker like `Lookup`/`Read` do.
- `Setlk`/`Setlkw` calls `lock_broker.acquire(...)` which is L3 TODO and always fails; the caller interprets `Err` as "lock denied" and returns `Errno(EWOULDBLOCK)` — a valid wire response that masks the "not implemented" reality.

### 2.9 `FuseDispatcher::Statfs` returns `ENOSYS`
The `cans/05-storage-plane.md#Storage-quotas` contract says statfs reports remaining broker quota "in O(1) time". `Errno(-ENOSYS)` tells the guest the filesystem doesn't support statfs. Should be `Err(Halt("TODO"))` until the quota math is wired.

---

## 3. Parity-gate dead-ends

### 3.1 `parity_cli` never executes a verdict
`crates/kelir-plugin-cli/src/parity_cli.rs::assert_vectors` iterates vectors, validates frame shape, arms watchdog/veto fixtures, then appends every case to `verdicts_not_executable` with a "pending engine dispatch (L3)" reason. `report.verdicts_executed == 0` for every vector set. The spec (`cans/15-operations.md#Multi-language-parity-gate`) says "Divergent verdicts block CI." At L2 the gate is inert.

### 3.2 `parity_cli` never parses `plugin_builder`
`parse_vectors` reads `watchdog` and `veto` but not `plugin_builder`. The ~46 builder cases produced by `vectors_testkit` are silently dropped before validation. The unified-builder coverage gate is unreachable.

### 3.3 Builder vector list is off by one
`crates/kelir-plugin-testkit/src/vectors_testkit.rs::plugin_builder_verb_cases`:
- Includes `("seal", ...)` — `seal` is not in the spec's verb list.
- Omits `("build", ...)` — `build` is required by `cans/02-plugin-model.md#Terminal-build`.

Result: 29 verb cases, not 30. The coverage gate (`--unified-builder-coverage`) would fail if executed.

### 3.4 `plugin_builder_merge_cases` duplicates `plugin_builder_nested_cases`
Both functions call `plugin_builder_scope_cases("...", seq)` with only the prefix differing. Merge cases should assert the host descriptor equals `[host_hooks ++ plugin_hooks]` with the plugin's scope; nested cases should assert descriptor equality with the flat chain. Both currently generate the *same* traces. The merge check is untested.

### 3.5 Builder events are not wire-compatible
`plugin_builder_verb_event` emits `serde_json::json!({"kind": "fs-write", ...})`. The real wire event for FsWrite is an inline synchronous hook payload (not a `GuestToHost` variant). Meanwhile the `trap` verb uses `serde_json::to_value(GuestToHost::TrapEvent(...))` — a real wire event. The family mixes two incompatible event encodings under one `events` array.

### 3.6 Every `expected_outcomes` is the default
Every builder case either has `outcomes: Vec::new()` (structural verbs) or `Some(Verdict::Pass)` / `Some(FilterAction::Forward)` (ring verbs). No case asserts a non-trivial verdict. The vectors cannot catch a divergence in policy evaluation even if `parity_cli` executed them.

### 3.7 `plugin_builder_fs_write_trap_case` requires `ledger` without mounting it
The case sets `requires: vec!["ledger"]` but the case doesn't include a `ledger` registration anywhere. `kelir_plugin_testkit::vectors_testkit` itself doesn't enforce prerequisites; a real consumer (the SDK builder) would throw `missing prerequisite: ledger required by host`. The case is structurally inconsistent.

---

## 4. Testkit honesty

### 4.1 `FixtureVmmDriver` is a stub, not a fixture
`crates/kelir-plugin-testkit/src/handlers_testkit.rs::FixtureVmmDriver` implements only `capabilities`, `boot`, `set_entropy_backend`, `entropy_position`, `restore_entropy_position`, `set_watchdog`. Every other `VmmDriver` method falls through to the trait default `Err(Unsupported)`. Any plugin test that calls `checkpoint`, `fork`, `adjust_balloon`, `rollback`, `export_snapshot`, `transport_connect`, etc. fails. The comment "every other primitive stays Unsupported" is honest, but "plugin tests run without a hypervisor" (`cans/02-plugin-model.md#Plugin-testkit`) is violated for any plugin that touches a real driver primitive.

### 4.2 `VectorCase::to_json` emits `plugin_builder` that the parser ignores
Confirmed by §3.2. If the parser and the encoder disagree on the wire format, the format isn't a contract — it's a schema that only one side knows about.

### 4.3 `poll_sync` panics on `Pending`
```rust
Poll::Pending => Err(VectorError::Execution("fixture future suspended ...".into()))
```
Fine for the watchdog setter, but `poll_sync` is used for `set_watchdog` and `register_veto` — both sync-complete. If someone later adds an async vector, it silently fails. Not a bug today, but the helper is dishonest about what it supports.

---

## 5. Tool/CLI sloppiness

### 5.1 `kelir-plugin-cli` tools are CWD-relative
- `audit_cli.rs::PLUGIN_DIR = "crates/kelir-plugins"`
- `audit_cli.rs::WORKSPACE_MANIFEST = "Cargo.toml"`
- `graph_cli.rs::Path::new("crates/kelir-plugins")`
- `layers_cli.rs::RULES = [("crates/kelir-core/src", ...), ...]`
- `budgets_cli.rs::run` defaults to `"perf/budgets.toml"`
- `bump_core_cli.rs::write_pin(Path::new(".kelir/core-channel.json"))`

`kelir-testkit` uses `env!("CARGO_MANIFEST_DIR")` for all four binaries. The CLI does not. `check -p plugin-x` from `/tmp` fails with "no plugin manifests found". The CI justfile happens to `cd` to the repo root, so the defect is invisible today.

### 5.2 `kelir-plugin-cli::main` returns `Ok(())` when no subcommand is given
```rust
if args.len() < 2 { println!("kelir-plugin-cli: missing subcommand"); return Ok(()); }
```
Exits 0 on usage error. `kelir-plugin-cli --bad-flag` would also exit 0 because `check` subcommand parsing is lenient (`check --layers --bad` works). Not catastrophic, but it doesn't distinguish "success" from "I don't understand."

### 5.3 Redundant subcommands
`check-layers` / `check-budgets` are exact aliases of `check --layers` / `check --budgets`. Two spellings of one thing; one will rot.

### 5.4 `spec check` always fails
`crates/kelir-plugin-cli/src/spec_cli.rs::validate_profile_aot` does file-exists and extension checks, then unconditionally `Err(NotImplemented)`. At L2, syntax parsing (Serde/`toml`/`serde_yaml`) is cheap and belongs here. This makes `just check-spec` pass only because it never invokes `spec check`.

### 5.5 `budgets check` doesn't check budgets
`crates/kelir-plugin-cli/src/budgets_cli.rs::check` validates:
- `enforce = true`
- required keys present
- each required value parses as `i64`

It never asserts that a measured latency is under a budget. The `--budgets` flag in `check --budgets` is a no-op relative to its name.

### 5.6 `doctor --enforce` always fails on Linux
`crates/kelir-plugin-cli/src/doctor_cli.rs` — `probe_page_size`, `probe_pidfd`, `probe_hvf`, `probe_whp` all return `ProbeOutcome::NotImplemented`. `ProbeOutcome::is_ready()` treats `NotImplemented` as not-ready, so `run --enforce` errors with `"host probes incomplete"` before it even reaches `write_hw_lockfile`. The `--enforce` path is dead on every platform except an Android device that has `/dev/avf`.

### 5.7 `fixtures --fetch` always fails
`crates/kelir-plugin-cli/src/fixtures_cli.rs::run` for `--fetch` returns `HashUnavailable` even when the fixture set is empty (the message says "no fixtures pinned, nothing to fetch", but `run` only checks `entries.is_empty()` in a separate branch; the non-empty branch unconditionally fails).

### 5.8 `scorecard --in-place` argument parsing
`crates/kelir-testkit/src/bin/scorecard_testkit.rs`:
```rust
let mode = match args.get(1) { Some(arg) => arg.as_str(), None => "generate" };
let in_place = args.iter().skip(2).any(|arg| arg == "--in-place");
```
`scorecard --in-place` is parsed as `mode = "--in-place"` and falls into `unknown => { eprintln!("Unknown mode"); exit(1) }`. The `--in-place` flag must be preceded by an explicit mode token. This is a user-facing bug.

---

## 6. Comment / structural sloppiness

### 6.1 Comment lies about available types
`crates/kelir-core/src/dispatch_plugin/mod.rs::dispatch_host_lsm_exec` has `// TODO (L3): no TrampolineRing tag exists for host LSM hooks`. `TrampolineRing::HostLsmExec` **does** exist in `crates/kelir-interface/src/fusion_plugin.rs`. The comment is stale.

### 6.2 `GateDecision::Allow` means "skip the interposer", not "allow"
`crates/kelir-core/src/trap_gate.rs::evaluate` returns `Allow` when gate 2 scope doesn't match — but that path means "this interposer should not fire", which the engine treats as `continue`. `Allow` reads as "let the syscall through" everywhere else in the codebase (see `filter-before-FFI`, `Verdict::Pass`). A reader will misread the gate. Rename to `Skip` or `Bypass`.

### 6.3 `error_envelope::From<PluginError>` misclassifies `Halt`
```rust
PluginError::Halt(message) => Self::econfig(message),
```
`Halt` is the generic internal-failure variant; mapping it to `EConfig` means every "not implemented yet" is reported to the SDK as a *config* error. Wrong category. Should be `EHALT` or `EInternal`.

### 6.4 Redundant `+ Send + Sync` on `Box<dyn VmmDriver>`
`VmmDriver` is `pub trait VmmDriver: Send + Sync`. Every `Box<dyn VmmDriver + Send + Sync>` in `handles_lifecycle.rs` and `session_lifecycle.rs` re-states bounds the trait already carries. Cosmetic but indicates the trait bounds weren't traced.

### 6.5 Two empty HList markers
`crates/kelir-interface/src/plugin_model.rs` has `pub struct NoDeps;` and `crates/kelir/src/hlist_sdk.rs` has `pub struct HNil;`. `hlist_sdk.rs` implements `Satisfies<NoDeps>` and `Satisfies<HNil>` separately. `Satisfies` is documented to be order-insensitive — but the two marker types are not aliases. Pick one.

### 6.6 `LifecycleCtx::update_dispatch(Arc<()>)`
`crates/kelir-interface/src/plugin_model.rs` takes `Arc<()>` and discards it. A caller has no way to know a real pipeline handle is expected at L3. Use a real placeholder type (`Arc<PipelineEngine>`) or delete the method until L3 can wire it.

### 6.7 `CompositePlugin` is dead
`crates/kelir-interface/src/plugin_model.rs::CompositePlugin` is defined, documented, and never implemented anywhere. Per `cans/02-plugin-model.md#Composite-plugin-hierarchy`, composite plugins are load-bearing. Either scaffold an impl in `kelir/src/builder_sdk.rs` (an empty `unpack` that splits hooks) or delete the trait.

### 6.8 `Session::bind_wire_imds_vault`, `bind_plugin`, `register_storage_lock_broker`, `seal_pipeline` are dead
All four public methods on `kelir_core::Session` are never called anywhere:
- `boot_with_engine` already binds the IMDS vault (duplicated logic).
- `bind_plugin` duplicates what `builder_sdk::finish_boot` does via `pipeline_mut()`.
- `register_storage_lock_broker` is intended to be called by `Session::boot` per its own TODO, but isn't.
- `seal_pipeline` is a no-op after `boot_with_engine` already sealed.

Four dead public methods on a shipped type invites two contradictory callers.

### 6.9 `Session::flush_workspace_before_terminate` has a dead branch
```rust
Err(PluginError::Halt(reason)) if reason.contains("needs the anchored reconciler") => {
    tracing::warn!("...skipping the pre-teardown workspace flush");
    Ok(())
}
```
`StoragePlane::reconcile` returns that error only when `self.reconciler.get()` is `None`. `boot_with_engine` always calls `storage.reconciler(...)`, so the `None` branch is unreachable. Dead code.

### 6.10 `run_lifecycle_hook` has 4× the same construction
`crates/kelir-core/src/session_lifecycle.rs::run_lifecycle_hook` repeats the `LifecycleCtx::new_with_watchdog_probe(...)` construction four times (dispatch × {Some, None}, callback × {Some, None}). A helper `with_ctx(&mut self, f)` would collapse this to two lines.

### 6.11 `VsockChannel` has no constructor
`crates/kelir/src/session_lifecycle.rs::VsockChannel` declares `pub struct VsockChannel { inner: tokio::io::DuplexStream }` and implements `AsyncRead`/`AsyncWrite`, but never provides `new`. The only producer, `VsockHandle::channel`, returns `Err`. The struct is dead.

### 6.12 `pty_master` binding naming
`crates/kelir-shim/src/main.rs`:
```rust
let (_pty_master, _pty_slave) = pty_terminal::open_pty_slave()?;
```
These are bound for the lifetime of `main` (the console), not unused. The `_` prefix signals "intentionally unused" — a future reader will delete them and break console. Should be `let (pty_master, pty_slave) = ...;` with a `// TODO: forward PTY master to the terminal plane` comment.

### 6.13 File-size overshoot
Target 500 LOC, hard ceiling 1000 per `cans/01-architecture.md`. Files clearly above target and likely at or above the ceiling:
- `crates/kelir-interface/src/plugin_model.rs` (~700–900)
- `crates/kelir/src/builder_sdk.rs` (~800)
- `crates/kelir-core/src/session_lifecycle.rs` (~800)
- `crates/kelir-core/src/dispatch_plugin/mod.rs` (~700)
- `crates/kelir-bindings/napi/src/lib.rs` (~700)
- `crates/kelir-bindings/pyo3/src/lib.rs` (~700)
- `crates/kelir-plugin-testkit/src/vectors_testkit.rs` (~700)
- `crates/kelir-interface/src/bin/codegen/model_codegen.rs` (~600)
- `crates/kelir-interface/src/bin/codegen/sdk_codegen.rs` (~600)
- `crates/kelir-shim/src/introspect_trap.rs` (~600)

Systemic — not one file, but a pattern of letting one "engine" function own everything (`boot_with_engine`, `finish_boot`, `Session::fork`).

---

## 7. Cross-binding asymmetry

### 7.1 `NativeSessionHandle` and `PySessionHandle` don't expose `session_dir` or `plugin_contexts`
The TS interface (`session.lifecycle.ts`) expects:
```typescript
readonly sessionDir?: string;
readonly pluginContexts?: readonly { readonly name: string; readonly ctx: unknown }[];
```
The NAPI struct has neither. `boot()` in `packages/ts/core/src/builder.lifecycle.ts` checks `pluginContexts === undefined && registrations.length > 0` and throws `EConfig`. Python's `boot()` checks for `get_plugin_context` and throws if missing. So:
- Any TS boot with at least one plugin throws.
- Any Python boot with at least one plugin throws.

This is the same bug as §1.5 from a different angle — the bindings can't materialize the plugin contexts because the core engine never produced them.

### 7.2 NAPI `NativeKelirBuilder::boot` calls `Session::boot_with_plugins(..., Some(descriptor), ...)` — but `boot_with_plugins` also calls `engine.with_trampoline(descriptor)`, which constructs a fresh `PipelineEngine` whose `trap_scopes`, `vsock_services`, etc. are empty. So the fused descriptor is bound to the pipeline, but the per-interceptor scopes are not. Combined with §1.5, the TS/Python bindings are structurally incomplete.

---

## 8. Spec discipline

### 8.1 `cans/plugins/profile.md` exceeds `max_tbd_per_file: 10`
`cans/_rules.yaml` sets `max_tbd_per_file: 10`. `profile.md` has eleven TBDs (`driver.ffi_boundary.queue_capacity`, `driver.ffi_boundary.max_buffer_bytes`, `driver.fast_boot`, `session.topology consumption`, `session.compute.max_vcpus`, `session.balloon config struct`, `session.watchdog.heartbeat_cadence_seconds`, `session.artifacts.kernel_boot_params`, `storage.dax_window`, `wire.l4_diversion.upstream_dial.dial_timeout_ms`, `terminal.wireframe_grid.export_on_trap`). The rule is violated by the canonical spec for the schema that the whole SDK reads.

### 8.2 `cans/01-architecture.md` claims `FUSE_GETATTR`/`READDIR`/`UNLINK` are "Mediation ops" but `fuse_storage.rs` returns `Err(Halt)` for all three
`cans/05-storage-plane.md#FUSE-opcode-table` lists GETATTR, READDIR, UNLINK, MKDIR, RENAME as broker ops with concrete replies (`FuseReply::Inode`, `FuseReply::Entries`, `FuseReply::Written`). `crates/kelir-core/src/planes/storage/fuse_storage.rs::FuseDispatcher::dispatch` returns `Err(Halt("TODO..."))` for every one. At L2, the broker's most basic read path (`Getattr` on an EROFS-rooted binary) is unreachable. Either the spec is aspirational or the wiring is incomplete — both are sloppy.

### 8.3 `cans/07-wire-plane.md#Synthetic-DNS-gateway` claims host resolver state is watched via `inotify`/`SCDynamicStore`
`crates/kelir-core/src/planes/wire/mod.rs::resolve_synthetic_dns` returns a fixed `ServFail`. No host resolver state is ever read. The spec is unambiguous about the mechanism ("Linux: /etc/resolv.conf watched via inotify; nameservers reloaded on change"). At L2, the resolver is a no-op; the spec should either be marked TBD or the resolver should at least track the current nameserver from `/etc/resolv.conf`. Current code claims a capability it doesn't have.

### 8.4 `cans/15-operations.md#Subcommands` lists `plugin manifest`, `plugin vectors`, `plugin rings --show`
`kelir-plugin-cli`'s `manifest`, `registry`, `rings`, `scaffold`, `logs` are all empty stubs that `return Ok(())`. The spec advertises them as developer tools. "Skeleton subcommands that succeed silently" is the opposite of a developer CLI; they should at least exit with a clear `E_NOT_IMPLEMENTED` (or be removed from the CLI).

---

## Priority ranking (for the agent that will fix these)

1. **§1.1** (`~/.kelir` not created) — zero-effort fix, unblocks all boot paths.
2. **§1.2, §1.3, §1.4, §1.6** — session fork and lifecycle hooks are the core promise. All four are wiring bugs; no L3 algorithm needed.
3. **§1.5, §7.1** — `register_stream_kind` not forwarded; TS/Python `boot()` throws. One fix covers both.
4. **§1.7, §1.8, §1.9** — driver/scope/trap identity. Safety-adjacent; correct behavior is "fail closed, don't pretend."
5. **§1.10** — vault is dead. Single field (`session_guest_cid`) needs to be set from the boot path.
6. **§2.1, §2.3, §2.4, §2.5** — observability and DNS. Cheap fixes; the mislabeling makes OTLP data unusable.
7. **§3.1, §3.2** — parity gate is inert. This is the L2 definition-of-done; without it, "1:1 behavioral parity" is untested.
8. **§3.3, §3.4** — builder vector list is wrong. One-line deletions + a real merge assertion.
9. **§4.1** — `FixtureVmmDriver` must implement the driver surface enough that plugin tests can run. That's the testkit contract.
10. **§5, §6** — CLI paths, dead code, misleading names, file sizes. Lower blast radius but cumulative.

Everything marked `// TODO (L3)` in the tree is *expected* and should be left alone. Everything in this report is what L2 was supposed to get right and didn't.