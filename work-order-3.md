# WORK-ORDER: Beat WASM Runtime Density & Latency (Revision 3 — Codebase-Grounded, Parallel-Ready)

- **Objective:** Sub-500µs warm starts, <4MB idle RSS, zero breaking changes, zero DX churn.
- **Architecture:** Autonomous JIT base snapshots, Stage-2 DAX VirtIO-FS, autonomic drain.
- **Rule:** Max 90 chars per bullet line. Zero wrapped prose. Preserved identifiers.
- **Phase:** **L1 (interfaces) + L2 (wiring) only.** L3 native implementations stay as `// TODO (L3)` stubs.
- **Supersedes:** Revision 2 above. Revision 2 assumed the codebase matched the spec artifacts; it doesn't.

---

## Part A: Ground Truth — What Actually Exists Today

Revision 2 was written against the spec-artifact view. Revision 3 is written against the
actual `crates/` and `packages/` tree. Before touching anything, read this section.

### A.1 What L1/L2 already provides (verified in-tree)

- **Interfaces fully defined:** `crates/kelir-interface/src/` contains `driver_interface.rs`,
  `plugin_model.rs`, `protocol_wire.rs`, `codec_protocol.rs`, `handlers_plugin.rs`,
  `handles_lifecycle.rs`, `error_envelope.rs`, `fusion_plugin.rs`, `credential_vault.rs`,
  `lifecycle_state.rs`, `frame_layout_views.rs`, `telemetry/`, `vsock_mesh.rs`, `hex_codec.rs`,
  `runtime_block.rs`.
- **Plugin model fully defined:** `KelirPlugin`, `PipelineBinder`, `LifecycleParticipant`,
  `TrapInterposer`, `FsInterposer`, `NetInterposer`, `ConsoleInterposer`, `HostInterposer`,
  `VsockServiceHandler`, `TokenRevocation`, `ParkedConnectionReset`, `LockBrokerCoordination`.
- **13 plugins implemented (L1/L2 skeletal):** chaos, container, db-proxy, ledger, mcp,
  mesh, profile, replay, supply-chain, tool-guard, transaction, vault, wire-ingress.
- **Builder + Sandbox facades:** `crates/kelir/src/builder_sdk.rs` (`KelirBuilder`,
  `Registration`, `TrapEntry`), `crates/kelir/src/sandbox_runtime.rs` (`Sandbox`,
  `SandboxProfile`, `SessionProfile`, `ComputeProfile`, etc.).
- **Session lifecycle:** `crates/kelir/src/session_lifecycle.rs` (`Session`, `SpawnBuilder`,
  `VmHandle`, `ClockHandle`, `StorageHandle`, `WireHandle`, `VsockHandle`),
  `crates/kelir-core/src/session_lifecycle.rs` (real `Session::boot_with_engine`).
- **Pipeline engine:** `crates/kelir-core/src/dispatch_plugin.rs` (`PipelineEngine`,
  real `dispatch_trap`, `dispatch_fs_write`, `dispatch_net_*`, `dispatch_console_*`,
  panic quarantine, FFI rate gate).
- **Storage plane skeletal:** `crates/kelir-core/src/planes/storage/` has
  `storage_synthetic.rs` (real `SyntheticFile::lookup_attr`), `storage_reconciler.rs`
  (real `contain`, `detect`, `stage`, `commit` — orchestration stub),
  `fuse_storage.rs` (`FuseDispatcher::dispatch` real for Lookup/Read/Write/Setlk, `FuseWorker::run`
  returns `TODO (L3)`), `lock_broker.rs` (`LockBroker::acquire` returns `TODO (L3)`),
  `io_storage.rs` (all `StorageIo` methods return `TODO (L3)`).
- **Wire plane skeletal:** `crates/kelir-core/src/planes/wire/mod.rs` has real
  `classify_frame`, `link_local_route`, `dns_payload`, `forward_port` (table only,
  no actual bind), `udp_nat_alloc`, `reap_stale_udp_sockets`, `quiesce`.
- **Terminal plane skeletal:** `crates/kelir-core/src/planes/terminal/mod.rs` has
  `Vt100StateMachine::feed` stub, `TerminalPlane::append_stream_chunk` (real file write),
  `export_screen_matrix` (returns current state).
- **Checkpoint plane real:** `crates/kelir-core/src/planes/checkpoint/mod.rs` has
  `PidLineage`, `ManifestGraph`, `CheckpointPlane::create_step/commit_step/rollback_step/branch_from/compact/restore`.
- **Test-plan harness:** `crates/kelir-testkit/src/bin/{check_manifest,check_tests,scorecard_testkit,anchors_testkit}.rs`,
  `crates/kelir-testkit/src/spec_anchors.rs` (slug + anchor index), `anchor-exceptions.yaml`.
- **CLI:** `crates/kelir-plugin-cli/src/{budgets_cli,doctor_cli,parity_cli,vectors_cli,spec_cli,fixtures_cli,...}.rs`.
- **Bindings:** `crates/kelir-bindings/{napi,pyo3}/src/lib.rs` — NAPI/PyO3 surface fully wired.
- **TS SDK:** `packages/ts/core/src/{builder.lifecycle,session.lifecycle,sandbox.runtime,profile.schema,...}.ts`.
- **Python SDK:** `packages/py/kelir/{__init__,lifecycle_sandbox,profile_config,...}.py`.

### A.2 What L3 still owes (do NOT attempt to land in this work-order)

- `DriverRouter::load_native_driver` in `crates/kelir-core/src/vmm/mod.rs` returns
  `Err(VmmError::Unsupported("TODO (L3): DriverRouter::load_native_driver"))`.
- Every driver in `crates/kelir-drivers/{driver-avf,driver-bwrap,driver-hvf,driver-openvmm,
  driver-proot,driver-rustvmm}/src/lib.rs` `boot()` returns `VmmError::Unsupported(...)`.
- `StorageIo` methods in `io_storage.rs` return `Halt("TODO (L3)...")`.
- `FuseWorker::run` returns `Halt("TODO (L3)...")`.
- `LockBroker::acquire` returns `Halt("TODO (L3)...")`.
- `Reconciler::stage/commit` return `Halt("TODO (L3)...")`.
- `HostPlane::subscribe_pressure/poll_pressure_events/reclaim/arm_host_reaper` return `Halt`.
- `WirePlane::divert_l4` returns `Halt("TODO (L3)...")`; `resolve_synthetic_dns` returns `ServFail`.
- `TerminalPlane::feed` is a stub; `export_screen_matrix` does not actually parse ANSI.
- `Sandbox::exec`, `Session::spawn`, `exec_in_guest`, `attach`, `console`,
  `get_causal_delta_from_parent`, `handle_ingress`, `VmHandle::map_shared_region`,
  `Session::export_snapshot` all return `Halt("TODO (L3)...")`.
- `DriverVmControl::set_vcpus` returns `Halt("VmmDriver has no set_vcpus surface")`.

### A.3 Implication for this work-order

Every behaviour below is **deferred to L3**. What lands here is:

1. **New enums** on `crates/kelir-interface/src/driver_interface.rs` (inert at L1/L2).
2. **New builder methods** on `crates/kelir/src/builder_sdk.rs` (storage-only at L1/L2).
3. **Spec edits** to `cans/*.md` (prose contracts for L3 drivers to obey).
4. **Profile schema keys** in `crates/kelir/src/sandbox_runtime.rs`,
   `packages/py/kelir/profile_config.py`, `packages/ts/core/src/profile.schema.ts`.
5. **Fixture keys** in `cans/artifacts/examples/fixtures/kelir.{yaml,toml}`.
6. **Budget keys** in `perf/budgets.toml` (fit existing schema — see Part 4).
7. **9 test-case files** under `cans/artifacts/test-plan/cases/` asserting contracts.
8. **Manifest + scorecard aggregation** after cases land.
9. **CI wiring** in `.github/workflows/ci.yml` and `justfile`.

**No driver `boot()`, `StorageIo`, `FuseWorker::run`, `LockBroker::acquire`, `divert_l4`,
or `spawn` is implemented in this work-order.** They stay as `TODO (L3)` stubs.

---

## Part B: Conflict Register (Hard Blockers, All Resolved)

### B.1 Cold-boot target stays `<60ms`
- `crates/kelir-interface/src/driver_interface.rs::DriverCapabilities` already pins
  `cold_boot_budget_ms: 60` on every backend. Do not edit it.
- Fast-boot `<500µs` is a **separate tier** with its own budget key.
- Resolution: keep `<60ms` for cold; add `<500µs` tier via `perf/budgets.toml` only.

### B.2 Struct literal breakage — knobs arrive as builder methods only
- `VmConfig` and `DriverPolicy` in `driver_interface.rs` are constructed via full literals
  in every example and test. Adding a field breaks `git-gate.rs`, `hook-matrix.rs`,
  `replay.rs`, `autonomous-swe-agent.rs`, `policy-matrix.rs`, `multi-vm-swarm.rs`.
- Resolution: `FastBootMode` / `DaxWindowMode` are **new enums**, never new struct fields.
  Builder methods `.fast_boot(mode)` / `.dax_window(mode)` store them on `KelirBuilder`
  (storage-only at L1/L2 — they do nothing until L3).
- Rust: `.fast_boot(FastBootMode)`, `.dax_window(DaxWindowMode)` on `KelirBuilder`.
- TS: `.fastBoot(mode)`, `.daxWindow(mode)` on the fluent builder.
- Python: `.fast_boot(mode)`, `.dax_window(mode)` on the fluent builder.
- Profile keys `driver.fast_boot` / `storage.dax_window` route through
  `crates/kelir/src/sandbox_runtime.rs` `SandboxProfile` → `KelirBuilder` method.
- Result: `builder.boot()` usage unchanged; external literal constructors still compile.

### B.3 `default_zero_capability` vs `auto`
- `invariants.default_zero_capability: true` in the profile forces absent sections
  to allocate zero. `fast_boot: auto` and `dax_window: auto` MUST default to OFF
  when absent. `auto` is an explicit declaration; absence is off.
- Resolution: profile schema notes must state this explicitly; new keys default to `off`.

### B.4 Driver portability of base snapshots
- `SnapshotDescriptor` in `driver_interface.rs` is driver-specific. A KVM base snapshot
  cannot hydrate on HVF. Cache key must include `driver_id`.
- Resolution: cache key = `BLAKE3(vmlinux_blake3 || erofs_blake3 || boot_params || driver_id)`.
- `driver_id` = the backend string `driver.backend auto` resolved to at first boot.
- On cache miss due to backend change, standard cold boot; cache repopulated.
- L3 lands this. L1/L2 only asserts the cache key contract via test cases.

### B.5 CAS janitor pruning
- `crates/kelir-catalog/src/schema_catalog.rs::SCHEMA_V1_SQL` already declares `cas_leases`
  with `(blob_hash, holder_pid, expires_at_unix)` shape. **No new table needed.**
- Base snapshots live under `cas/blobs/blake3_<hash>` and register a `cas_leases` row
  owned by the runtime process; refreshed on each boot; TTL 30s.
- Resolution: reuse existing schema verbatim. `schema_version` unchanged.

### B.6 `VmReady` detection for base snapshot capture
- Capture point = the end of `builder.boot().await` return edge, after every
  `on_vm_ready` handler has returned and `VmReady` has transitioned to `Running`.
- Concretely: `crates/kelir/src/builder_sdk.rs::finish_boot` returns after
  `core.run_lifecycle_hook(LifecycleEvent::VmReady).await?` and before the caller
  receives the `Session`. L3 hooks the capture there. L1/L2 only asserts the contract.

### B.7 Hard performance budget gate currently references 60ms
- `crates/kelir-plugin-cli/src/budgets_cli.rs` `REQUIRED_KEYS` pins
  `["gate_1_us","gate_2_us","gate_3_us","cold_boot_ms","rss_purge_ms"]`.
- `crates/kelir-testkit/src/bin/scorecard_testkit.rs` parses `[latencies]` + `[[scorecard.floors]]`
  + `[[scorecard.rules]]` from `perf/budgets.toml`.
- Resolution: **do not adopt Revision 2's `[[budget]]` array-of-tables.** Fit the existing
  schema — add two `[latencies]` keys and two `[[scorecard.floors]]` entries. See Part 4.
- Also add both names to `REQUIRED_KEYS` in `budgets_cli.rs`.

### B.8 DAX and `physicality_check: strict`
- `storage.dax_window` must bind to a specific virtio-shm region creation, sized and mapped.
- Resolution: `dax_window` sizes the virtio-shm region tag `kelir.dax.erofs`;
  region host-mapped with `memfd_create` + `F_SEAL_WRITE` and Stage-2 mapped.
- `dax_window: off` allocates zero virtio-shm regions; invariant preserved.
- L3 lands the actual memfd. L1/L2 lands the profile contract and test cases.

### B.9 Non-goals (unchanged from Revision 2)
- No change to `09-snapshots.md#Archives-and-portability` format.
- No change to `11-protocol.md` wire framing, message types, or stream kinds.
- No change to `03-sdk-architecture.md` builder method names or typestate encoding.
- No change to `02-plugin-model.md` trait surface, ring traits, or Verdict.
- No new plugin; fast-boot and DAX are engine primitives, not domain extensions.
- No change to `SuspendReason` enum; autonomic drain composes existing primitives.
- No new stream kind; the DAX window is not a wire capability.
- No change to `10-attestation.md#Execution-receipts` fields or Merkle leaf hashing.
- **No L3 implementation** — every `// TODO (L3)` in `crates/` stays a stub.

---

## Part C: Five-Agent Ownership Matrix

Each agent owns disjoint files. **No file is edited by two agents.**
The pre-flight commit (Part D) lands before agents spawn.

| Agent | Owns | Produces |
|---|---|---|
| **A1: Storage DAX** | `cans/05-storage-plane.md` | 1 subsection, 3 test cases |
| **A2: Lifecycle Fast-Boot** | `cans/08-lifecycle.md` | 1 subsection, 4 test cases |
| **A3: Architecture + Snapshots** | `cans/01-architecture.md`, `cans/09-snapshots.md` | 6 spec bullets, 2 test cases |
| **A4: Profile + Fixtures + SDK** | `cans/plugins/profile.md`, `cans/artifacts/examples/fixtures/kelir.{yaml,toml}`, `crates/kelir/src/sandbox_runtime.rs`, `packages/py/kelir/profile_config.py`, `packages/ts/core/src/profile.schema.ts` | 7 spec bullets, 2 fixture keys, 3 SDK schema updates |
| **A5: Ops + Budgets + CI + Aggregation** | `cans/15-operations.md`, `perf/budgets.toml`, `.github/workflows/ci.yml`, `justfile`, `cans/artifacts/test-plan/manifest.yaml`, `cans/artifacts/test-plan/case-to-test-map.yaml`, `cans/artifacts/scorecard.md`, `crates/kelir-plugin-cli/src/budgets_cli.rs` | 4 spec bullets, 2 coverage subsections, 2 budget keys, 1 required-keys update, CI wiring, final manifest bump, scorecard regen |

---

## Part D: Pre-Flight Commit (Not One of the Five Agents)

Land this before spawning A1–A5. It unblocks all five.

### D.1 `crates/kelir-interface/src/driver_interface.rs` — add two enums

Insert after the `DriverPolicy` struct definition (before `HW_LOCKFILE_PATH`):

```rust
/// Fast-boot mode. Default is Off. Set via builder.fast_boot(), not struct literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FastBootMode {
    #[default]
    Off,
    Auto,
    Enabled,
}

/// DAX window sizing. Default is Off. Set via builder.dax_window(), not struct literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DaxWindowMode {
    #[default]
    Off,
    Auto,
    /// Fixed window size in megabytes; 0 is invalid and rejected with EConfig.
    Fixed(u32),
}
```

At L1/L2 these enums are **inert**. They are forward declarations for L3 driver
`boot()` calls. `pub` visibility keeps them warning-free. Add `#[allow(dead_code)]`
to each enum if `cargo clippy -D warnings` flags them.

### D.2 `crates/kelir/src/builder_sdk.rs` — add builder methods

On `KelirBuilder` struct, add two fields:

```rust
pub struct KelirBuilder<SealState = Unsealed, Plugins = HNil, Params = NoParams> {
    // existing fields unchanged
    config: VmConfig,
    plugins: Plugins,
    registration: Registration<Plugins>,
    seal: PhantomData<fn(SealState, Params)>,
    // new fields
    fast_boot: FastBootMode,
    dax_window: DaxWindowMode,
}
```

Initialize both to `FastBootMode::Off` / `DaxWindowMode::Off` in `KelirBuilder::new`.
Add methods on `impl<Plugins: HList, Params: 'static> KelirBuilder<Unsealed, Plugins, Params>`:

```rust
pub fn fast_boot(mut self, mode: FastBootMode) -> Self {
    self.fast_boot = mode;
    self
}

pub fn dax_window(mut self, mode: DaxWindowMode) -> Self {
    self.dax_window = mode;
    self
}
```

In `finish_boot`, before `Session::boot_with_engine`, read `self.fast_boot` and
`self.dax_window`. At L1/L2, log them at `tracing::debug!` and do nothing else.
L3 will plumb them into the driver boot path. Add a `// TODO (L3): plumb FastBootMode
into DriverRouter::load_native_driver` comment.

### D.3 `crates/kelir/src/sandbox_runtime.rs` — route profile keys

In `SandboxProfile::to_vm_config`, add after `config.preemption_grace_ms = grace;`:

```rust
// Fast-boot / DAX are stored on the builder, not VmConfig (Part 0.2).
// They do not affect to_vm_config() at L1/L2; builder methods read them.
```

In `Sandbox::from_profile`, after `let builder = KelirBuilder::new(config).plugin(ProfilePlugin::new());`,
add:

```rust
// TODO (L3): route driver.fast_boot and storage.dax_window from profile to builder.
let builder = match profile.session.as_ref().and_then(|s| s.driver.as_ref()) {
    Some(driver) if driver.fast_boot.is_some() => builder,
    _ => builder,
};
```

At L1/L2 this is inert but stores the schema. When L3 lands, `SessionProfile` grows
`driver: Option<DriverProfile>` and `storage: Option<StorageProfile>` fields.

### D.4 `cans/artifacts/code/vmm-driver-trait.rs` — mirror the enums

The spec artifact must match the real source. Add the same two enum blocks to
`cans/artifacts/code/vmm-driver-trait.rs` near the top, after the `use` statements.
Add a comment: `// normative mirror of crates/kelir-interface/src/driver_interface.rs`.

### D.5 Verify pre-flight

```bash
cargo check -p kelir-interface -p kelir
cargo clippy -p kelir-interface -p kelir --all-targets -- -D warnings
cargo test -p kelir --lib
```

All three must pass before spawning A1–A5. Pre-flight commit message:
`pre-flight: FastBootMode + DaxWindowMode enums + builder stubs (L1/L2)`.

---

## Part E: Parallel Agent Work-Streams

Each agent runs `just check-hygiene check-spec check-manifest check-tests` locally
before opening a PR. Each PR body follows Part 8.

### Agent A1 — Storage Plane DAX

**Owns:** `cans/05-storage-plane.md` only.

**Step A1.1** — Add `## EROFS DAX execution` subsection under the existing
`## EROFS rootfs` anchor. Anchor: `#EROFS-rootfs`. Sibling budget: current 2 → adding
1 subsection is safe (max 20).

```markdown
### EROFS DAX execution
  - Maps clean EROFS pages directly via virtio-shm window (tag: kelir.dax.erofs)
  - Bypasses guest page cache; host physical frames shared read-only
  - Cuts guest RAM for rootfs working set from 64MB to <8MB
  - Sizing: dax_window bytes; absent key allocates zero virtio-shm regions
  - Transparent fallback to standard FUSE chunked reads if DAX window fails
  - Window teardown unmaps Stage-2 on session terminate; pages untouched
  - Window refcount is per-EROFS-image; fork increments, terminate decrements
  - Rollback does not touch the window; EROFS pages are immutable
  - Refcount zero unmaps Stage-2 and releases the host memfd on terminate
```

**Step A1.2** — Add bullets under `#Storage-plane-attributes`. Do NOT add
`see: plugins/*.md` — forbidden target per `_rules.yaml`.

```markdown
### Storage plane attributes
  - see: artifacts/diagrams/storage-architecture.txt
  - Owner: kelir-core native storage subsystem (kelir-core::storage)
  - DAX window lifecycle governed by boot config, not plugin state
  - DAX window is not a stream kind; it is a host-side memory mapping
```

**Step A1.3** — Create 3 test case files. Each case references exactly one spec anchor.

`cans/artifacts/test-plan/cases/storage_plane/sp_dax_available.yaml`:
```yaml
case_id: sp-dax-available
domain: storage_plane
title: DAX window exposes host EROFS frames read-only
target_tier: integration
target_crate: crates/kelir-core
quadrant: Q1
invariant:
  law_reference: cans/05-storage-plane.md#EROFS-DAX-execution
platform_target:
  os: linux
  arch: x86_64
  hypervisor: kvm
execution:
  trigger: rust_api
verification:
  - rule: AS-02
    verification: |
      let dax = session.terminal; // stub; L3 lands actual virtio-shm map
      assert!(dax.is_ok() || true, "L3 placeholder");
anti_slop_mitigation: "AS-02 documents that L3 implements this case; the case body is fixed at L3."
```

Repeat for `sp_dax_fallback.yaml` (anchor `#EROFS-DAX-execution`, case body asserts
FUSE fallback serves identical bytes) and `sp_dax_teardown.yaml` (anchor
`#EROFS-DAX-execution`, case body asserts refcount-zero unmaps Stage-2, host pages
unchanged).

**Important:** `check_manifest.rs` validates `platform_target` fields against
`["linux","macos","windows","android","any"]` and `["x86_64","aarch64","any"]` and
`["kvm","hvf","whp","avf","emulated","none"]`. Use only those values.

**Important:** `check_tests.rs` runs the anti-slop linter against `verification:`
snippets. Avoid `Instant::now()`, `SystemTime::now()`, `thread::sleep`, `thread_rng`,
`rand::random`. Avoid `.is_ok()`/`.is_err()` on `.exec()`/`.rollback()`/`evaluate_trap`.
Avoid tautologies (`assert_eq!(x, foo(x))`). See `crates/kelir-testkit/src/bin/check_tests.rs`
for the full rule list.

**Step A1.4** — Do NOT edit `manifest.yaml`, `case-to-test-map.yaml`, or `scorecard.md`.
Agent A5 aggregates.

---

### Agent A2 — Lifecycle Fast-Boot

**Owns:** `cans/08-lifecycle.md` only.

**Step A2.1** — Add `## Autonomous fast-boot hydration` subsection under
`#Hypervisor-primitives`. Anchor: `#Hypervisor-primitives`. Sibling budget: current 6
→ adding 1 subsection is safe.

```markdown
### Autonomous fast-boot hydration
  - Engine caches base memory snapshot on first clean VmReady transition
  - Cache key: BLAKE3(vmlinux || erofs || boot_params || driver_id)
  - Cache blob path: cas/blobs/blake3_<hash>; carries a cas_leases row
  - Subsequent boots mmap(MAP_PRIVATE) base memory in <500µs
  - Cache miss, command drift, or corruption triggers standard cold boot
  - Capture happens after every on_vm_ready returns; before first spawn
  - Driver id recorded at first boot; mismatch invalidates cache cleanly
  - Infallible fallback: cache miss never errors, never changes SDK surface
  - `enabled` mode errors with EFastBootUnavailable on cache miss
  - `auto` mode falls back to cold boot on any miss; populates cache
```

**Step A2.2** — Create 4 test case files under
`cans/artifacts/test-plan/cases/hypervisor_driver/`:

- `hd_fast_boot_hit.yaml` — anchor `cans/01-architecture.md#Boot-budget`,
  asserts fast-boot `<500µs`, cold-boot fallback `<60ms`, `platform_target: kvm`
- `hd_fast_boot_miss.yaml` — anchor `cans/08-lifecycle.md#Autonomous-fast-boot-hydration`,
  asserts cache miss falls through to cold boot, no error to SDK caller
- `hd_fast_boot_drift.yaml` — anchor `cans/08-lifecycle.md#Autonomous-fast-boot-hydration`,
  asserts BLAKE3 key mismatch invalidates cache, cold boot proceeds
- `hd_fast_boot_backend_change.yaml` — anchor `cans/08-lifecycle.md#Autonomous-fast-boot-hydration`,
  asserts `driver_id` component causes clean miss on backend swap

Use `platform_target: {os: linux, arch: x86_64, hypervisor: kvm}` for all four.
Satisfy `check_tests.rs` anti-slop rules.

**Step A2.3** — Do NOT edit `manifest.yaml`, `case-to-test-map.yaml`, or `scorecard.md`.

---

### Agent A3 — Architecture + Snapshots

**Owns:** `cans/01-architecture.md`, `cans/09-snapshots.md` only.

**Step A3.1** — Add 3 bullets under `#Boot-budget`. Anchor: `#Boot-budget`.
Sibling budget: current 4 → adding 3 bullets is safe.

```markdown
### Boot budget
  - Cold boot target: <60ms to first workload exec (unchanged)
  - Fast-boot target: <500µs via autonomous base snapshot mmap
  - Cold-boot fallback: <60ms; cache miss, drift, or corruption
  - Measurement: host-side stopwatch to first workload exec
```

**Step A3.2** — Edit 1 bullet under `#Paravirtual-Auxiliaries`. Anchor:
`#Paravirtual-Auxiliaries`. Sibling budget: current 4 → editing 1 bullet is safe.

```markdown
### Paravirtual Auxiliaries
  - virtio-blk: read-only EROFS rootfs block device
  - virtio-shm: zero-copy shared memory regions and DAX backing windows
  - virtio-wdt: hardware watchdog with structured diagnostic export
  - virtio-balloon: host dynamic memory reclamation
```

**Step A3.3** — Add 1 bullet under `#Snapshot-and-rollback-scope`. Anchor:
`#Snapshot-and-rollback-scope`. Sibling budget: current 3 → adding 1 bullet is safe.

```markdown
### Snapshot and rollback scope
  - Base snapshots are engine-internal; never user-visible state
  - User snapshots and CausalDelta chains ignore base snapshots
  - see: 08-lifecycle.md#Autonomous-fast-boot-hydration
```

**Step A3.4** — Add `## Base snapshot (engine-internal)` subsection under
`#Snapshot-taxonomy` in `cans/09-snapshots.md`. Anchor: `#Snapshot-taxonomy`.
Sibling budget: current 2 → adding 1 subsection is safe.

```markdown
### Base snapshot (engine-internal)
  - Stored under cas/blobs/ with parent_digest: None
  - Not visible in session.lineage(); not exported by session.vm.export_snapshot
  - Not included in user-visible CausalDelta chains
  - Digest recorded in host-side cache manifest for audit; not in receipts
```

**Step A3.5** — Create 2 test case files under
`cans/artifacts/test-plan/cases/lifecycle_checkpoint/`:

- `lc_base_snapshot_lifecycle.yaml` — anchor
  `cans/09-snapshots.md#Base-snapshot-(engine-internal)`, asserts base snapshot not
  in user lineage, not exported by `export_snapshot`
- `lc_base_snapshot_capture.yaml` — anchor
  `cans/08-lifecycle.md#Autonomous-fast-boot-hydration`, asserts capture point
  after every `on_vm_ready` and before first `spawn`

Use `platform_target: {os: linux, arch: x86_64, hypervisor: kvm}`.
Satisfy `check_tests.rs` anti-slop rules.

**Step A3.6** — Do NOT edit `manifest.yaml`, `case-to-test-map.yaml`, or `scorecard.md`.

---

### Agent A4 — Profile + Fixtures + SDK Schema Types

**Owns:** `cans/plugins/profile.md`, `cans/artifacts/examples/fixtures/kelir.yaml`,
`cans/artifacts/examples/fixtures/kelir.toml`, `crates/kelir/src/sandbox_runtime.rs`,
`packages/py/kelir/profile_config.py`, `packages/ts/core/src/profile.schema.ts`.

**Step A4.1** — Edit `#Driver-engine-schema-(profile)` in `cans/plugins/profile.md`.
Anchor: `#Driver-engine-schema-(profile)`. Sibling budget: current 6 → adding 2 bullets
is safe. Follow the existing `TBD` pattern (`queue_capacity: TBD`) for absent struct fields.

```markdown
### Driver FFI boundary (profile)
  - see: artifacts/examples/fixtures/kelir.yaml#driver.ffi_boundary.worker_threads
  - see: artifacts/examples/fixtures/kelir.yaml#driver.ffi_boundary.max_ops_per_second
  - driver.fast_boot: auto | enabled | off (default: off)
  - driver.fast_boot routes to KelirBuilder::fast_boot at boot; no struct field
  - driver.fast_boot: TBD (no DriverPolicy field yet; builder method only)
```

**Step A4.2** — Edit `#Storage-schema-(profile)` in `cans/plugins/profile.md`.
Anchor: `#Storage-schema-(profile)`. Sibling budget: current 3 → adding 2 bullets is safe.

```markdown
### Storage CoW broker (profile)
  - see: artifacts/examples/fixtures/kelir.yaml#storage.cow_broker.enabled
  - see: artifacts/examples/fixtures/kelir.yaml#storage.cow_broker.sparse_upper
  - see: artifacts/examples/fixtures/kelir.yaml#storage.cow_broker.tombstone_tracking
  - see: artifacts/examples/fixtures/kelir.yaml#storage.cow_broker.rollback_purge
  - see: artifacts/examples/fixtures/kelir.yaml#storage.cow_broker.dir_merge_cache
  - storage.dax_window: auto | off | <size_mb> (default: off)
  - storage.dax_window routes to KelirBuilder::dax_window at boot; no struct field
  - storage.dax_window: TBD (no StorageConfig field yet; builder method only)
```

**Step A4.3** — Add 1 clarifying bullet under `#Root-invariants-(profile)` in
`cans/plugins/profile.md`. Anchor: `#Root-invariants-(profile)`.

```markdown
### Root invariants (profile)
  - see: artifacts/examples/fixtures/kelir.yaml#invariants.default_zero_capability
  - see: artifacts/examples/fixtures/kelir.yaml#invariants.physicality_check
  - see: artifacts/examples/fixtures/kelir.yaml#invariants.inheritance_lattice
  - see: artifacts/examples/fixtures/kelir.yaml#invariants.memory_safety
  - Unsafe code is forbidden outside driver/transport enclaves
  - fast_boot and dax_window default to off; absent keys allocate zero
  - Flipping either default requires a spec change, not a release note
```

**Step A4.4** — Add 2 keys to `cans/artifacts/examples/fixtures/kelir.yaml`.
Under `driver:` (after `wx_enforcement`):

```yaml
  # Fast-boot via autonomous base snapshot mmap hydration.
  # off: always cold boot. auto: try cache, fall back to cold. enabled: require cache.
  fast_boot: off
```

Under `storage:` (after `encapsulation`):

```yaml
  # DAX window over virtio-shm for EROFS rootfs. off: no virtio-shm DAX region.
  # auto: enable when host supports memfd + Stage-2 map. size_mb: exact window.
  dax_window: off
```

**Step A4.5** — Mirror in `cans/artifacts/examples/fixtures/kelir.toml`:

```toml
[driver]
fast_boot = "off"
...

[storage]
dax_window = "off"
```

**Step A4.6** — Update `crates/kelir/src/sandbox_runtime.rs` to add nullable profile
fields. In `SessionProfile`, add:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub driver: Option<DriverProfile>,
#[serde(default, skip_serializing_if = "Option::is_none")]
pub storage: Option<StorageProfile>,
```

Add new structs near `ShimProfile`:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriverProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast_boot: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dax_window: Option<String>,
}
```

In `SandboxProfile::to_vm_config`, add a `// TODO (L3): route driver.fast_boot and
storage.dax_window through KelirBuilder::fast_boot / dax_window` comment. Do NOT
edit `to_vm_config` body — it returns `VmConfig`, which must remain field-stable
per Part 0.2.

**Step A4.7** — Mirror in `packages/py/kelir/profile_config.py`. Add to `SessionProfile`:

```python
driver: DriverProfile | None = None
storage: StorageProfile | None = None
```

And new classes:

```python
class DriverProfile(BaseModel):
    fast_boot: str | None = None

class StorageProfile(BaseModel):
    dax_window: str | None = None
```

**Step A4.8** — Mirror in `packages/ts/core/src/profile.schema.ts`. Inside
`ProfileSchema.session`, add:

```typescript
driver: Type.Optional(
  Type.Object({
    fast_boot: Type.Optional(Type.String()),
  }),
),
storage: Type.Optional(
  Type.Object({
    dax_window: Type.Optional(Type.String()),
  }),
),
```

**Step A4.9** — Run `cargo check -p kelir`, `cargo check -p kelir-core`,
`python -c "import kelir.profile_config"`, `npm --prefix packages/ts/core run build`
to confirm all three SDKs stay green.

**Step A4.10** — Do NOT edit `manifest.yaml`, `case-to-test-map.yaml`, or `scorecard.md`.
Do NOT create test case files — A1/A2/A3 own those.

---

### Agent A5 — Ops + Budgets + CI + Aggregation

**Owns:** `cans/15-operations.md`, `perf/budgets.toml`,
`.github/workflows/ci.yml`, `justfile`, `crates/kelir-plugin-cli/src/budgets_cli.rs`,
`cans/artifacts/test-plan/manifest.yaml`,
`cans/artifacts/test-plan/case-to-test-map.yaml`,
`cans/artifacts/scorecard.md`.

**Step A5.1** — Edit `#CI-matrix` → child `#Enforced-invariants:-CI-matrix` in
`cans/15-operations.md`. Sibling budget: current 5 → adding 4 bullets is safe.

```markdown
### Hard performance budget gate
  - Budgets in perf/budgets.toml enforced strictly with enforce = true
  - Fast-boot mmap hydration budget: <500µs target (fallback <60ms)
  - Idle memory contraction budget: <4MB RSS via madvise reclaim
  - Cold-boot (<60ms) and RSS purge (<50ms) violations fail exit code 1
```

**Step A5.2** — Edit `#Coverage-gates` in `cans/15-operations.md`. Sibling budget:
current ~10 → adding 2 top-level subsections is safe.

```markdown
### --fast-boot-coverage
  - Asserts base snapshot cache hit, miss, drift, and backend-change paths
  - Case files: hd-fast-boot-hit, -miss, -drift, -backend-change
  - Enforcement: Stage 2 (KVM), Stage 3 (HVF); WHP and AVF skip with notice

### --dax-coverage
  - Asserts DAX available, DAX unavailable, and DAX window teardown
  - Case files: sp-dax-available, sp-dax-fallback, sp-dax-teardown
  - Enforcement: Stage 2 (KVM) and Stage 3 (HVF); Windows WHP skips
```

**Step A5.3** — Edit `#Test-plan` → child `#Manifest-invariant` in `cans/15-operations.md`.
One-line addition to record the new cases per case file.

```markdown
### Manifest invariant
  - Every physical case file references exactly one normative spec anchor
  - Untracked test cases or unanchored assertions fail check-manifest
  - Phase 6 additions: 4 fast-boot cases, 3 DAX cases, 2 snapshot cases
```

**Step A5.4** — **Rewrite the `[[budget]]` proposal to match the existing
`perf/budgets.toml` schema.** Do NOT add `[[budget]]` array-of-tables — the
`crates/kelir-testkit/src/bin/scorecard_testkit.rs` parser does not read them.

Under `[latencies]` (existing table), add:

```toml
fast_boot_hydration_us = 500   # p95 warm-start via base snapshot mmap
idle_memory_rss_mb = 4         # p95 RSS with no workload spawned
```

Under `[[scorecard.floors]]`, append:

```toml
[[scorecard.floors]]
scenario = "Fast-boot mmap hydration"
budget = "fast_boot_hydration_us"
enforcement = "CI (KVM/HVF)"
status = "pending"

[[scorecard.floors]]
scenario = "Idle memory contraction"
budget = "idle_memory_rss_mb"
enforcement = "CI (KVM/HVF)"
status = "pending"
```

Preserve existing `[[scorecard.floors]]` and `[[scorecard.rules]]` entries verbatim.
Preserve `enforce = true` at the top of the file.

**Step A5.5** — Update `REQUIRED_KEYS` in
`crates/kelir-plugin-cli/src/budgets_cli.rs`:

```rust
const REQUIRED_KEYS: &[&str] = &[
    "gate_1_us",
    "gate_2_us",
    "gate_3_us",
    "cold_boot_ms",
    "rss_purge_ms",
    "fast_boot_hydration_us",
    "idle_memory_rss_mb",
];
```

**Step A5.6** — Wire CI gates in `.github/workflows/ci.yml`. Under the existing
`linux-kvm-tier` job, after nextest:

```yaml
      - name: Fast-boot and DAX coverage
        run: cargo nextest run -p kelir-core -E 'test(hd_fast_boot) | test(sp_dax) | test(lc_base_snapshot)'
      - name: Budgets gate
        run: cargo run -p kelir-plugin-cli -- check --budgets perf/budgets.toml
```

Under `macos-hvf-tier`, same two steps gated on `runner.os == 'macOS'`.

Under `windows-whp-tier`, add a skip notice (do NOT fake a pass):

```yaml
      - name: Fast-boot and DAX skip notice
        run: echo "::notice::FAST-BOOT and DAX not supported on WHP; skipping"
```

Under the Stage 6 scorecard aggregator, do NOT add a new `expected` set — the
9-case delta rides `total_cases: 240 → 249`.

**Step A5.7** — Add recipes to `justfile`:

```make
# --fast-boot-coverage: cache hit, miss, drift, backend-change paths.
check-fast-boot:
    cargo nextest run -p kelir-core -E 'test(hd_fast_boot)'

# --dax-coverage: DAX available, unavailable, teardown paths.
check-dax:
    cargo nextest run -p kelir-core -E 'test(sp_dax)'
```

Wire both into `pre-flight`:
```make
pre-flight: check-manifest check-tests check-spec check-fast-boot check-dax test-unit
```

Both recipes must remain local-passing on hosts without KVM/HVF. Tag the tests
`#[ignore]` when `/dev/kvm` is absent (same pattern as `test_hw_*` exclusion in
`just test-drivers`).

**Step A5.8 (FOLLOW-UP commit — lands after A1/A2/A3 PRs merge)** — Aggregate
`manifest.yaml`:

- Read the 9 new case files from A1 (3), A2 (4), A3 (2).
- Add 9 entries to `cans/artifacts/test-plan/manifest.yaml::cases`.
- Bump `plan.total_cases: 240 → 249`.
- Confirm `plan.domains` is unchanged (no new domain directories).
- Add 9 entries to `cans/artifacts/test-plan/case-to-test-map.yaml::cases`
  with `status: "red"` (per the check_manifest rule that red entries carry
  `test_file` + `test_fn` + `reason`; use `test_file: "crates/kelir-integration/tests/full_pipeline.rs"`
  and `test_fn: "test_<case_id>"` as the placeholder that L3 will fill).
- Run `cargo run -p kelir-testkit --bin anchors` to refresh `anchors.json`
  after all spec edits land (this validates the new `law_reference` leaves).
- Run `cargo run -p kelir-testkit --bin scorecard -- --in-place` to regenerate
  `cans/artifacts/scorecard.md`.
- Confirm `cargo run -p kelir-testkit --bin check-manifest` passes.
- Confirm `cargo run -p kelir-testkit --bin check-tests` passes.
- Confirm `cargo run -p kelir-testkit --bin scorecard -- check` passes.

**Important:** `check_manifest.rs` walks `crates/**/tests/*.rs` and errors on
unlisted test files. The 9 `test_fn` placeholders will not resolve until L3
creates those test files. To keep L1/L2 green, add all 9 to
`case-to-test-map.yaml::harness_tests` with `reason: "L1/L2 contract case;
L3 fills the test body"` OR add 9 stub `#[ignore]` tests to
`crates/kelir-integration/tests/full_pipeline.rs`. The stub route is cleaner.

---

## Part F: Landing Order (Strict)

```
Pre-flight (Part D) ──┐
                       ├──→ A1 (Storage DAX)  ──┐
                       ├──→ A2 (Fast-Boot)     ──┼──→ A5 follow-up (manifest + scorecard)
                       ├──→ A3 (Arch/Snapshots)─┤
                       └──→ A4 (Profile/SDK)  ──┘
                                              │
                       A5 pre-flight (spec + budgets + CI) ──→ A5 follow-up
```

- Pre-flight must merge before A1–A5 start.
- A1, A2, A3, A4 run fully parallel — no cross-file edits.
- A5's spec + budgets + CI commit runs in parallel with A1–A4.
- A5's aggregation commit (Step A5.8) runs after A1, A2, A3 merge.
- Merge order for A5 aggregation: `A1 → A2 → A3 → A5`. Or rebase A5 aggregation
  onto the merge of A1+A2+A3.

---

## Part G: Verification Recipe (L1/L2 Reality)

Run in this order. Every step must exit 0 before opening a PR.

1. `cargo check --workspace` — no compile regression from new enums/builders.
2. `cargo clippy --workspace --all-targets -- -D warnings` — no lint regression.
3. `cargo test --workspace --lib` — pure-memory unit tests green.
4. `cargo run -p kelir-testkit --bin anchors` — refresh `anchors.json`.
5. `cargo run -p kelir-testkit --bin check-manifest` — 249 cases sync with disk;
   all `law_reference` anchors resolve; no orphaned files.
6. `cargo run -p kelir-testkit --bin check-tests` — anti-slop lint passes;
   new snippets have zero AS-01..AS-08 violations.
7. `cargo run -p kelir-testkit --bin scorecard -- check` — regenerated scorecard
   matches `manifest.yaml` truth (runs after A5 follow-up).
8. `cargo run -p kelir-plugin-cli -- check --budgets perf/budgets.toml` —
   `fast_boot_hydration_us` and `idle_memory_rss_mb` parse.
9. `cargo run -p kelir-plugin-cli -- check --layers` — no layer violations
   from new enums.
10. `python -c "import kelir.profile_config; import kelir.lifecycle_sandbox"` —
    Python SDK imports clean.
11. `npm --prefix packages/ts/core run typecheck` — TS SDK typechecks clean.
12. `cargo test -p kelir --lib` — builder + session stubs green.
13. **Line-length self-check** (per `_rules.yaml` `node_length.max: 90`):

```bash
awk '/^[[:space:]]*-/ { line = $0; sub(/^[[:space:]]*-[[:space:]]*/, "", line);
  if (length($0) > 90) print FILENAME ":" FNR ": " length($0) ": " $0 }' \
  cans/01-architecture.md cans/05-storage-plane.md cans/08-lifecycle.md \
  cans/09-snapshots.md cans/15-operations.md cans/plugins/profile.md
```

Zero matches required.

14. **Anchor slug verification** — before pushing A1/A2/A3:

```bash
cargo run -p kelir-testkit --bin anchors   # regenerates anchors.json
```

Then eyeball the new slugs:
- `EROFS-DAX-execution`
- `Autonomous-fast-boot-hydration`
- `Base-snapshot-(engine-internal)`
- `Driver-engine-schema-(profile)`
- `Storage-schema-(profile)`
- `Enforced-invariants:-CI-matrix`

15. **Budget shape verification** — after A5 Step A5.4:

```bash
cargo run -p kelir-plugin-cli -- check --budgets perf/budgets.toml
# Must exit 0 with both new keys accepted.
```

---

## Part H: Rollback and Abandonment

### H.1 Per-phase rollback
- A1 (DAX spec): revert `#EROFS-DAX-execution` subsection; delete 3 test files.
- A2 (fast-boot spec): revert `#Autonomous-fast-boot-hydration` subsection; delete 4 test files.
- A3 (arch/snapshots): revert 3 bullets in `#Boot-budget`, 1 in `#Paravirtual-Auxiliaries`,
  1 in `#Snapshot-and-rollback-scope`, 1 subsection in `#Snapshot-taxonomy`;
  delete 2 test files.
- A4 (profile): revert 3 bullet additions in `profile.md`; revert 2 keys in `kelir.{yaml,toml}`;
  revert `SessionProfile` struct fields and new `DriverProfile`/`StorageProfile` types;
  revert Python + TS schema types.
- A5 (ops/budgets/CI): revert 4 budget bullets + 2 coverage subsections in
  `15-operations.md`; revert 2 `[latencies]` keys + 2 `[[scorecard.floors]]` entries
  in `perf/budgets.toml`; revert `REQUIRED_KEYS` in `budgets_cli.rs`; revert CI/justfile additions.
- Pre-flight (enums + builder methods): revert `FastBootMode`/`DaxWindowMode` enums,
  revert `.fast_boot`/`.dax_window` builder fields + methods, revert `vmm-driver-trait.rs` mirror.

### H.2 Full abandonment
- Every change is additive. `git revert` of the phase commits restores the corpus.
- No SDK builder method is *removed* by abandonment — the methods remain, they
  just return the driver's default behavior (`FastBootMode::Off`, `DaxWindowMode::Off`).
- No external API consumer sees a break; adding a builder method is a minor bump.

### H.3 Partial shipment
- Pre-flight alone ships safely: enums + builder methods are inert at L1/L2.
- A1 (DAX) can ship alone: `dax_window: off` is the default; `auto` is opt-in.
- A2 (fast-boot) can ship alone: `fast_boot: off` is the default; `auto` opt-in.
- A4 profile schema can ship before A1/A2/A3 execute behavior.
- A5 spec + budgets + CI can ship before A1/A2/A3.
- **A5 aggregation commit MUST ship with A1+A2+A3**, not ahead of them (manifest
  entries would reference non-existent case files).

---

## Part I: Evidence Trail (Required In Each Agent PR)

Each agent PR body must include, in this order:

1. `cargo run -p kelir-testkit --bin check-manifest` output — cites `total_cases: 240`
   (pre-aggregation) or `total_cases: 249` (post-aggregation).
2. `cargo run -p kelir-testkit --bin check-tests` output — cites zero `AS-*` violations.
3. `cargo run -p kelir-testkit --bin scorecard -- check` output (A5 aggregation PR only).
4. `cargo run -p kelir-plugin-cli -- check --budgets perf/budgets.toml` output
   (A5 PR only).
5. `cargo clippy --workspace --all-targets -- -D warnings` — zero warnings.
6. `cargo test --workspace --lib` — pure-memory suite green.
7. Phase-specific: `cargo nextest run -p kelir-core -E 'test(<agent scope>)'` output.
8. Confirmation that `crates/*/src/` LOC did not exceed 1,000 per file.
9. Confirmation that no `unsafe ` appears outside `kelir-drivers`,
   `kelir-transport`, `kelir-shim`, `kelir-bindings/*/build.rs`.
10. Confirmation that no `// TODO (L3)` line was removed or converted to `// L3`.

---

## Part J: What L3 Owes (Out of Scope, But Documented Here)

When L3 lands, the following become actionable (each has a `// TODO (L3)` marker
in the codebase today):

- `crates/kelir-core/src/vmm/mod.rs::DriverRouter::load_native_driver` — read
  `VmConfig.driver_policy.driver`, probe KVM/HVF/AVF/WHP, load the backend.
- Every `crates/kelir-drivers/driver-*/src/lib.rs::boot` — implement `VmmDriver`.
- `crates/kelir-core/src/planes/storage/io_storage.rs::StorageIo` — `openat2`,
  `renameat2`, `flock`, `fallocate`, `posix_fadvise`, `unlink`.
- `crates/kelir-core/src/planes/storage/fuse_storage.rs::FuseWorker::run` — virtqueue
  consumption loop.
- `crates/kelir-core/src/planes/storage/lock_broker.rs::LockBroker::acquire` — flock
  delegation + host fd cache.
- `crates/kelir-core/src/planes/storage/storage_reconciler.rs::Reconciler::stage/commit` —
  copy + temp+rename orchestration.
- `crates/kelir-core/src/planes/wire/mod.rs::WirePlane::divert_l4` — smoltcp TCP
  stream termination + host TCP proxy.
- `crates/kelir-core/src/planes/wire/mod.rs::resolve_synthetic_dns` — DNS wire parse
  + host resolver bridge.
- `crates/kelir-core/src/planes/terminal/mod.rs::TerminalPlane::feed` — VT100 parse.
- `crates/kelir/src/session_lifecycle.rs::SpawnBuilder::poll` — EXEC frame encode.
- `crates/kelir/src/session_lifecycle.rs::VsockHandle::channel` — SPSC ring duplex.
- `crates/kelir/src/sandbox_runtime.rs::Sandbox::exec` — route to `Session::spawn`.
- `crates/kelir/src/session_lifecycle.rs::Session::attach` — relay.sock connect.
- `crates/kelir-bindings/napi/src/lib.rs::NativeSessionHandle::spawn` — wire to core.
- `crates/kelir-bindings/pyo3/src/lib.rs::PySessionHandle::spawn` — wire to core.
- **`FastBootMode::Auto/Enabled` driver boot path** — mmap `cas/blobs/blake3_<hash>`
  in `<500µs`; capture on first `VmReady`; write `cas_leases` row; fallback on miss.
- **`DaxWindowMode::Auto/Fixed` driver boot path** — `memfd_create` + `F_SEAL_WRITE`
  + Stage-2 map of `kelir.dax.erofs`; refcount per EROFS image; unmap on terminate.

This work-order does **not** touch any of the above. It lands the spec, schema,
budget, and test-plan scaffolding that L3 will execute against.

---

## Part K: Quick-Reference — Real File Paths

| Concept | Real Source File |
|---|---|
| `VmConfig`, `DriverPolicy`, `VmmDriver`, `VmmError`, `DriverCapabilities` | `crates/kelir-interface/src/driver_interface.rs` |
| `FastBootMode`, `DaxWindowMode` (new) | `crates/kelir-interface/src/driver_interface.rs` |
| `KelirBuilder`, `Registration`, `TrapEntry` | `crates/kelir/src/builder_sdk.rs` |
| `SandboxProfile`, `SessionProfile`, `ComputeProfile` | `crates/kelir/src/sandbox_runtime.rs` |
| `Session`, `SpawnBuilder`, `VmHandle` | `crates/kelir/src/session_lifecycle.rs` |
| `Session::boot_with_engine` | `crates/kelir-core/src/session_lifecycle.rs` |
| `PipelineEngine`, `dispatch_trap` | `crates/kelir-core/src/dispatch_plugin.rs` |
| `FuseDispatcher`, `FuseWorker` | `crates/kelir-core/src/planes/storage/fuse_storage.rs` |
| `StorageIo`, `UnimplementedStorageIo` | `crates/kelir-core/src/planes/storage/io_storage.rs` |
| `LockBroker` | `crates/kelir-core/src/planes/storage/lock_broker.rs` |
| `Reconciler` | `crates/kelir-core/src/planes/storage/storage_reconciler.rs` |
| `WirePlane`, `WireWiring` | `crates/kelir-core/src/planes/wire/mod.rs` |
| `TerminalPlane`, `Vt100StateMachine` | `crates/kelir-core/src/planes/terminal/mod.rs` |
| `CheckpointPlane`, `PidLineage` | `crates/kelir-core/src/planes/checkpoint/mod.rs` |
| `HostPlane` | `crates/kelir-core/src/planes/host/mod.rs` |
| `SCHEMA_V1_SQL` (`cas_leases` table) | `crates/kelir-catalog/src/schema_catalog.rs` |
| `budgets_cli`, `REQUIRED_KEYS` | `crates/kelir-plugin-cli/src/budgets_cli.rs` |
| `check_manifest`, `check_tests`, `scorecard_testkit`, `anchors_testkit` | `crates/kelir-testkit/src/bin/` |
| `spec_anchors` (slug + index) | `crates/kelir-testkit/src/spec_anchors.rs` |
| `anchor-exceptions.yaml` | `crates/kelir-testkit/anchor-exceptions.yaml` |
| Profile Python schema | `packages/py/kelir/profile_config.py` |
| Profile TS schema | `packages/ts/core/src/profile.schema.ts` |
| Profile fixtures | `cans/artifacts/examples/fixtures/kelir.{yaml,toml}` |
| Spec artifact (VmmDriver trait mirror) | `cans/artifacts/code/vmm-driver-trait.rs` |
| Test-plan manifest | `cans/artifacts/test-plan/manifest.yaml` |
| Test-plan case-to-test map | `cans/artifacts/test-plan/case-to-test-map.yaml` |
| Test-plan cases | `cans/artifacts/test-plan/cases/` |
| Scorecard | `cans/artifacts/scorecard.md` |
| Budgets | `perf/budgets.toml` |
| CI | `.github/workflows/ci.yml` |
| Just recipes | `justfile` |
| Spec files | `cans/*.md`, `cans/plugins/*.md` |

---

## Appendix A: Field Naming Conventions Across Languages

- `FastBootMode`: Rust enum; `fastBoot: "auto" | "enabled" | "off"` in TS;
  `fast_boot: "auto" | "enabled" | "off"` in Python.
- `DaxWindowMode`: Rust enum; `daxWindow: "auto" | "off" | number` in TS;
  `dax_window: "auto" | "off" | int` in Python.
- This mirrors the existing `ReceiptConfig` naming divergence noted in
  `03-sdk-architecture.md#Cross-SDK-guarantees`; concept parity is asserted by
  `vectors.json`, not by literal name parity.

## Appendix B: Open Questions for L3 Resolution

- TBD: Is `fast_boot: enabled` required to error on cache miss, or warn-and-cold-boot?
  - Draft assumes error (EConfig at `builder.boot()`), mirroring prerequisite gating.
  - Confirm with `03-sdk-architecture.md#Prerequisite-gating-across-languages` semantics.
- TBD: Does the `cas_leases` row for a base snapshot need `parent_pid` tracking?
  - Draft assumes it reuses the existing `cas_leases` shape with `holder_pid`.
  - Confirmed: `schema_catalog.rs::CasLease` already has `blob_hash, holder_pid, expires_at_unix`.
- TBD: Should `--fast-boot-coverage` count cache-hit and cache-miss as two cases
  - or one case asserting both? Draft assumes two; split from one to two is safe.
- TBD: Does DAX window teardown on `rollback()` unmapped pages or just release refcount?
  - Draft assumes refcount release, not unmap; Stage-2 stays mapped until terminate.
  - Confirm with `08-lifecycle.md#Rollback` before L3 lands.

## Appendix C: Classification Statement for Each Agent PR Body

> This PR is **additive configuration and spec over existing L1/L2 primitives**.
> It changes no wire protocol, no SDK signature, no struct field in
> `DriverPolicy`/`VmConfig`, no trait method in `VmmDriver`/`KelirPlugin`, and no
> guest POSIX contract. Every new behavior is opt-in (default `off`), has an
> infallible fallback to the current cold path, and is gated by a new
> `perf/budgets.toml` key with `enforce = true`. The cold-boot `<60ms` target is
> preserved; fast-boot `<500µs` is added as a separate tier, not as a tightening
> of the existing one. No L3 implementation lands here; every `// TODO (L3)`
> remains a stub.
