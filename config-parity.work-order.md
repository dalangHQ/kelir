# work-order.md — Canonical Configuration & Plane Parity

## Architectural Invariants

* **Isomorphic Config Law**: Every declarative profile table (`[session]`, `[storage]`, `[wire]`, `[terminal]`, `[host]`, `[checkpoint]`, `[plugins.*]`) maps 1:1 to typed engine and plane config structs deriving `Default`, `Serialize`, and `Deserialize`.
* **Zero Displaced State**: Plane configurations describe only plane transport and drivers. Cross-cutting features (Vault, Auth, Dynamic Policies) belong strictly to their respective plugin crates.
* **L1/L2 Execution Boundary**: Implement only compilable types, builder wiring, descriptor mapping, and pipeline plumbing. All deep kernel/hypervisor logic, cryptographic signatures, and transport execution remain explicit `// TODO (L3)` tags registered in `l3-slop-register.yaml`.

---

## 1. Vault Isolation from Wire Plane

Wire plane configuration describes purely Layer 2–4 network transport. It does not carry credentials, secret descriptors, or vault injection metadata.

* **Specification**:
  * Strip `vaults: Vec<CredentialDescriptor>` and credential injection structs entirely from `WireConfig`.
  * Secret injection and token management are owned strictly by `VaultPlugin` via `[plugins.vault]`.
  * Wire proxies reference named upstream routes; vault placeholders are resolved out-of-band by the credential broker.
* **Target Files**:
  * `crates/kelir-interface/src/wire/net_ops.flow.rs`
  * `crates/kelir/src/planes/wire_plane.rs`
  * `packages/ts/core/src/builder.config.ts`
  * `packages/py/kelir/wire_planes.py`

---

## 2. Canonical Chaos Plugin Schema

Chaos fault injection mirrors declarative fault tables for network and storage planes.

* **Specification**:
  * Root table exposes `network: Vec<NetworkFaultRule>` and `storage: Vec<StorageFaultRule>` (not `fs`).
  * `StorageFaultRule` fields: `target_glob: String`, `simulate_errno: Errno`, `probability: f64`.
  * `NetworkFaultRule` fields: `target: String`, `direction: FaultDirection`, `latency_ms: u64`, `jitter_ms: u64`, `drop_rate: f64`, `inject_rst: bool`, `bandwidth_kbps: Option<u64>`.
  * All fault execution paths return typed verdicts or actions; simulation triggers remain `// TODO (L3) [plugin_engine]`.
* **Target Files**:
  * `crates/kelir-plugins/plugin-chaos/src/lib.rs`
  * `crates/kelir-plugins/plugin-chaos/src/fault_table.flow.rs`
  * `packages/py/kelir/plugins/plugin_chaos.py`
  * `packages/ts/core/src/plugins.registry.ts`

---

## 3. Structured Replay Configuration

Replay configuration defines deterministic instruction windows and clock pin policies.

* **Specification**:
  * Field naming: `step_window_instructions: u64` (replaces `step_window`).
  * Clocksource policy: `pinned_clocksource: String` (replaces raw string arrays of kernel params).
  * Invariant fields: `deterministic_rng_seed: u64`, `network_isolation: bool`.
  * Lifecycle preemption hooks validate these fields during `on_vm_init`; PMU instruction cycle stepping remains `// TODO (L3) [lc_23_replay_deterministic_seed_reproduces_step_windows]`.
* **Target Files**:
  * `crates/kelir-plugins/plugin-replay/src/lib.rs`
  * `crates/kelir-plugins/plugin-replay/src/replay_state.flow.rs`
  * `packages/py/kelir/plugins/plugin_replay.py`
  * `packages/ts/core/src/plugins.registry.ts`

---

## 4. Ledger WAL Storage Descriptors

Ledger persistence requires explicit file path naming matching the Merkle WAL specification.

* **Specification**:
  * Field naming: `store_path: PathBuf` / `store_path: str` in `LedgerConfig` and `MerkleWalConfig` (replaces bare `store`).
  * Persist canonical fields: `merkle_chain: bool`, `hashing_standard: HashingStandard`, `record_pre_redaction_hash: bool`, `include_hook_spans_in_root: bool`.
  * WAL block append, LMDB integration, and leaf cosigning remain `// TODO (L3) [telemetry_audit]`.
* **Target Files**:
  * `crates/kelir-plugins/plugin-ledger/src/lib.rs`
  * `crates/kelir-plugins/plugin-ledger/src/wal_ledger.flow.rs`
  * `packages/py/kelir/plugins/plugin_ledger.py`
  * `packages/ts/core/src/plugins.registry.ts`

---

## 5. Relational Database Proxy Schema Rewrites

Database statement interposition relies on typed schema mapping records rather than raw tuples.

* **Specification**:
  * Struct `PgSchemaRewrite` exposes `from_schema: String` and `to_schema: String`.
  * `PgWireRewrite` config holds `schema_rewrites: Vec<PgSchemaRewrite>` (replaces `rewrite_rules: Vec<(String, String)>`).
  * In-place SQL AST mutation and ErrorResponse synthesis remain `// TODO (L3) [rt_10_pgwire_frame_rewrite_in_place]`.
* **Target Files**:
  * `crates/kelir-plugins/plugin-db-proxy/src/lib.rs`
  * `crates/kelir-plugins/plugin-db-proxy/src/service_db_proxy.flow.rs`
  * `packages/py/kelir/plugins/plugin_db_proxy.py`
  * `packages/ts/core/src/plugins.registry.ts`

---

## 6. Canonical Session Balloon Configuration

MicroVM memory ballooning is a first-class session compute property.

* **Specification**:
  * Define `VmBalloonConfig` deriving `Default`, `Serialize`, `Deserialize`:
    * `enabled: bool`
    * `initial_mb: u32`
    * `pressure_reclaim: bool`
    * `reclaim_strategy: String`
  * `VmConfig` exposes `balloon: Option<VmBalloonConfig>`.
  * `profile_validate.flow.rs` maps `session.balloon` directly to `VmConfig.balloon`.
  * `madvise` and hypervisor frame reclamation remain `// TODO (L3) [hd_16_virtio_balloon_pressure_reclaim]`.
* **Target Files**:
  * `crates/kelir-interface/src/driver_interface.flow.rs`
  * `crates/kelir/src/profile_schema.flow.rs`
  * `crates/kelir/src/profile_validate.flow.rs`
  * `packages/py/kelir/profile_config.py`

---

## 7. Declarative Builder Ingestion (`KelirBuilder::from_config`)

The builder is the unified entry point for both declarative files and programmatic hook chains.

* **Specification**:
  * Expose static constructor `KelirBuilder::from_config(path)` across Rust, TypeScript, and Python.
  * Ingestion parses the TOML/YAML profile, runs schema validation, and sets plane descriptors (`storage`, `wire`, `terminal`, `host`, `checkpoint`) before exposing the builder for fluent registration.
  * `Sandbox` operates strictly as a high-level lifecycle facade wrapping a booted `Session`.
* **Target Files**:
  * `crates/kelir/src/builder_sdk.flow.rs`
  * `crates/kelir/src/sandbox_runtime.flow.rs`
  * `packages/ts/core/src/builder.lifecycle.ts`
  * `packages/ts/core/src/sandbox.runtime.ts`
  * `packages/py/kelir/kelir_builder.py`
  * `packages/py/kelir/lifecycle_sandbox.py`

---

## 8. Full-Fidelity Specification Export (`export_spec`)

Exporting active topology must preserve all declarative namespaces.

* **Specification**:
  * `export_spec(path)` on `Session` and `Sandbox` serializes the full effective configuration: `invariants`, `driver`, `session`, `storage`, `wire`, `terminal`, `host`, `checkpoint`, and `plugins`.
  * Format is deduced from extension (`.toml` or `.yaml`).
  * Round-trip invariant: `KelirBuilder::from_config(a).seal()?.boot().await?.export_spec(b)` must yield a specification functionally identical to `a`.
* **Target Files**:
  * `crates/kelir/src/sandbox_runtime.flow.rs`
  * `packages/ts/core/src/profile.schema.ts`
  * `packages/ts/core/src/sandbox.runtime.ts`
  * `packages/py/kelir/lifecycle_sandbox.py`

---

## 9. Hierarchical Terminal Plane Descriptors

Terminal settings reflect PTY allocation, sanitization rules, and wireframe grid tracking.

* **Specification**:
  * `PtyConfig` / `TerminalConfig` mirrors the canonical hierarchy:
    * `cols: u16`, `rows: u16`
    * `wireframe_grid: TerminalWireframeGridConfig { track_screen_matrix: bool, export_on_trap: bool }`
    * `sanitizer: TerminalSanitizerConfig { drop_osc52: bool, strip_title: bool }`
  * Real-time VT100 escape sequence parsing and ANSI shadow stripping remain `// TODO (L3) [tp_03_ansi_sanitizer_osc52_clipboard_drop]`.
* **Target Files**:
  * `crates/kelir/src/planes/terminal_plane.flow.rs`
  * `crates/kelir-terminal/src/real_terminal.run.rs`
  * `packages/ts/core/src/builder.config.ts`
  * `packages/py/kelir/wire_planes.py`

---

## 10. Canonical Host and Checkpoint Plane Descriptors

Host management and snapshot orchestration use typed plane descriptors.

* **Specification**:
  * Define `HostPlaneConfig` in `kelir-interface`:
    * Fields: `reaper: HostReaperConfig`, `cgroups: CgroupConfig`, `catalog: CatalogConfig`, `telemetry_ring: TelemetryRingConfig`.
  * Define `CheckpointPlaneConfig` in `kelir-interface`:
    * Fields: `step_dag: StepDagConfig`, `compaction: CompactionConfig`, `snapshot_boundaries: BoundaryConfig`.
  * Expose `.host(HostPlaneConfig)` and `.checkpoint(CheckpointPlaneConfig)` on `KelirBuilder`.
  * Kernel pressure monitoring and delta-chain block compaction remain `// TODO (L3) [hd_16_virtio_balloon_pressure_reclaim]` and `// TODO (L3) [lc_20_portable_snapshot_archive_export]`.
* **Target Files**:
  * `crates/kelir-interface/src/host/ops.rs`
  * `crates/kelir-interface/src/checkpoint/ops.rs`
  * `crates/kelir-engine/src/planes/host/plane.flow.rs`
  * `crates/kelir-engine/src/planes/checkpoint/plane.flow.rs`
  * `crates/kelir/src/builder_typestate.flow.rs`

---

## 11. Host-Only Verb Enforcement Across Languages

Host builder verbs are inaccessible when configuring plugins.

* **Specification**:
  * **Rust**: Typestate enforces boundaries at compile time (`KelirBuilder<Host, ...>` vs `KelirBuilder<Plugin, ...>`).
  * **TypeScript & Python**: Runtime builders enforce mode checks dynamically. Invoking `.storage()`, `.wire()`, `.terminal()`, `.host()`, `.checkpoint()`, or `.boot()` on a plugin-mode builder throws/raises `EConfig` synchronously at the call site.
* **Target Files**:
  * `packages/ts/core/src/builder.lifecycle.ts`
  * `packages/py/kelir/kelir_builder.py`
  * `packages/py/kelir/kelir_builder_scope.py`

---

## 12. Structured Storage Cache Descriptors and FFI Bridges

Shared host directories and cache mounts enforce explicit mount permissions and mapping points.

* **Specification**:
  * Define `SharedCacheConfig`:
    * `host_path: PathBuf`
    * `guest_mount: PathBuf`
    * `read_only: bool`
  * `StoragePlaneConfig` holds `shared_caches: Vec<SharedCacheConfig>` and `dax_window: Option<String>`.
  * NAPI and PyO3 `storage_bridge` parsers unpack dictionary arrays into `SharedCacheConfig` instances.
  * Real container image caching via external `kelir-pack` remains `// TODO (L3) [plugin_engine]`.
* **Target Files**:
  * `crates/kelir/src/planes/storage_plane.rs`
  * `crates/kelir-bindings/napi/src/storage_bridge.rs`
  * `crates/kelir-bindings/pyo3/src/storage_bridge.rs`
  * `packages/ts/core/src/builder.config.ts`
  * `packages/py/kelir/storage_planes.py`

---

## 13. Single-Crossing FFI Trampoline Architecture

Interception rings preserve performance by traversing the foreign language boundary exactly once per ring event.

* **Specification**:
  * The NAPI and PyO3 bridges compile all registered hooks for a given ring into a single fused dispatcher.
  * `NapiFusedCallback` and `PyFusedCallback` execute the full hook pipeline within the host runtime, returning an aggregate `Verdict`.
  * Hooks do not cross the C/GIL boundary individually per ordinal.
  * Event marshaling and mutable buffer copyback remain typed stubs returning `Ok(Verdict::Pass)` with `// TODO (L3) [bt_03_typescript_fused_trampoline_single_crossing_chained_hooks]` and `// TODO (L3) [bp_03_python_fused_trampoline_single_crossing_chained_hooks]`.
* **Target Files**:
  * `crates/kelir-bindings/napi/src/fusion_plugin.flow.rs`
  * `crates/kelir-bindings/pyo3/src/fusion_plugin.flow.rs`
  * `crates/kelir-engine/src/dispatch_plugin/engine.flow.rs`

---

## 14. Typed L4 Diversions and Upstream Dialing

Network interception cleanly separates transparent proxying, UNIX domain socket diversion, and upstream timeouts.

* **Specification**:
  * `L4Proxy` explicitly separates `destination` (IP:port string) from `forward_unix_socket` (`Option<PathBuf>`).
  * `inject_mtls` is structured as `Option<MtlsConfig>` holding `cert_path: PathBuf` and `key_path: PathBuf`.
  * Wire plane configuration defines `dial_timeout_ms: u64` (defaulting to `5000`) and `ssh_agent_bridge: bool`.
  * Raw packet splicing and streaming TLS termination remain `// TODO (L3) [wp_12_l4_diversion_passthrough_tls]`.
* **Target Files**:
  * `crates/kelir-interface/src/wire/net_ops.flow.rs`
  * `crates/kelir/src/planes/wire_plane.rs`
  * `packages/ts/core/src/builder.config.ts`
  * `packages/py/kelir/wire_planes.py`

---

## 15. Test Manifest Parity and Anti-Slop Compliance

The test suite, verification harness, and specification artifacts must maintain zero-drift alignment.

* **Specification**:
  * Reconcile all law references in `cans/artifacts/test-plan/manifest.yaml` to ensure every anchor maps to valid spec sections.
  * Register every new `// TODO (L3)` gap tag in `cans/artifacts/test-plan/l3-slop-register.yaml` with explicit risk, anti-slop rules, and acceptance criteria.
  * Verify full test plan integrity via `cargo run -p kelir-testkit --bin check-manifest` and `cargo run -p kelir-testkit --bin check-tests`.
* **Target Files**:
  * `cans/artifacts/test-plan/manifest.yaml`
  * `cans/artifacts/test-plan/l3-slop-register.yaml`
  * `crates/kelir-testkit/src/bin/check_manifest.run.rs`
  * `crates/kelir-testkit/src/bin/check_tests.run.rs`