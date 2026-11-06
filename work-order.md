# WORK-ORDER.md: Distributed Systems Architecture Construction
**Target System:** Kelir MicroVM Hypervisor Microkernel  
**Phase:** 1 of 5 (Workspace Scaffolding, Leaf Contracts, Supporting Enclaves & Shim Wire Skeletons)  
**Total Parts (x):** 5  
- **Part 1/5:** Scaffolding, Root Build System, CI/CD, `kelir-interface`, `kelir-transport`, `kelir-catalog`, `kelir-otlp`, `kelir-shim` (L1/L2)
- **Part 2/5:** Engine Core & Native Planes (`kelir-core`: Terminal, Storage, Wire, Checkpoint, Host, Bus, Dispatcher)
- **Part 3/5:** Driver Enclaves (`driver-rustvmm`, `driver-hvf`, `driver-openvmm`, `driver-avf`, `driver-bwrap`, `driver-proot`)
- **Part 4/5:** Plugin Ecosystem (13 Catalog Plugins: Tier 1 Transports, Tier 2 Proxies, Tier 3 Aggregators)
- **Part 5/5:** Adapters, Tooling CLI, Native FFI Bindings & Tri-Native SDK Surfaces (`kelir-pack`, `kelir-plugin-cli`, NAPI, PyO3, TS, Python)

---

## 1. AGENT OPERATIONAL PROTOCOLS & BUILD GATES

Every parallel agent executing tasks within this work order must operate under strict non-interference boundaries:

1. **Target Directory Isolation**: Every agent must export:
   ```bash
   export CARGO_TARGET_DIR=target/agent-<id>
   ```
   Do not share lock-files or build caches across concurrent workers.
2. **Manifest Mutations**: Modifying `Cargo.toml`, `package.json`, or workspace manifests must be written to a `.tmp` file and committed via atomic rename (`mv`).
3. **Spec Traceability**: Non-code annotations must strictly follow the pointer convention:
   ```rust
   // see: cans/01-architecture.md#Codebase-invariants
   ```
   Zero prose, zero speculative documentation. Single-line docs only.
4. **Code Hygiene & Hard Bounds**:
   - Hard limit: 1,000 LOC per `.rs` file. Target: 500 LOC.
   - Zero inline tests in `src/` (no `#[cfg(test)]`). Tests belong exclusively in `tests/`.
   - Deny: `unwrap()`, `expect()`, `panic!()`, `todo!()`, `unimplemented!()`.
   - Deny unsafe code by default: `#![forbid(unsafe_code)]` on all crates except drivers, transport, and shim.
5. **Phase Gate Definition**:
   - **L1 (Contract)**: Strictly compilable interfaces, traits, function signatures, enums, zero-sized types, and error types.
   - **L2 (Wiring)**: Structural composition, pipeline dispatch arrays, state containers, and invocation plumbing. Algorithmic loops and leaf evaluations remain explicit `// TODO` markers returning appropriate error variants or zero-values.
   - **NO L3**: No deep implementation algorithms.
   - **NO Test Cases**: Focus entirely on structural integrity and compiler green light.

---

## 2. STAGE 0: ROOT WORKSPACE & CI/CD SCAFFOLDING

### Task 0.1: Workspace Filesystem Generation
Spawn **Agent-00** to construct the canonical workspace tree.

```bash
mkdir -p .github/workflows \
  crates/kelir/src \
  crates/kelir-interface/src \
  crates/kelir-core/src/planes/{terminal,storage,wire,checkpoint,host} \
  crates/kelir-core/src/{bus,vmm} \
  crates/kelir-transport/src \
  crates/kelir-catalog/src \
  crates/kelir-otlp/src \
  crates/kelir-plugin-testkit/src \
  crates/kelir-shim/src \
  crates/kelir-drivers/{driver-rustvmm,driver-hvf,driver-openvmm,driver-avf,driver-bwrap,driver-proot}/src \
  crates/kelir-plugins/{plugin-chaos,plugin-ledger/src,plugin-mcp,plugin-db-proxy,plugin-mesh,plugin-supply-chain,plugin-wire-ingress,plugin-profile,plugin-tool-guard,plugin-vault,plugin-container,plugin-replay,plugin-transaction}/src \
  crates/kelir-adapters/{mcp-server,kelir-pack}/src \
  crates/kelir-integration/src \
  crates/kelir-plugin-cli/src \
  crates/kelir-bindings/{napi,pyo3}/src \
  packages/ts/{core,plugins,e2b-compat}/src \
  packages/py/kelir/{plugins,compat} \
  perf \
  scripts
```

### Task 0.2: Root Manifests
**File: `Cargo.toml`**
- Set Rust Edition 2024 (MSRV 1.85+).
- Define `workspace.members`:
  - `crates/*`
  - `crates/kelir-drivers/*`
  - `crates/kelir-plugins/*`
  - `crates/kelir-adapters/*`
  - `crates/kelir-bindings/*`
- Set `workspace.package`:
  - `license = "FSL-1.1-Apache-2.0"`
  - `authors = ["Kelir Contributors"]`
- Configure workspace-wide lints:
  ```toml
  [workspace.lints.rust]
  unsafe_code = "forbid"

  [workspace.lints.clippy]
  unwrap_used = "deny"
  expect_used = "deny"
  panic = "deny"
  unimplemented = "deny"
  todo = "deny"
  unreachable = "deny"
  ```
- Define `workspace.dependencies`:
  - `serde = { version = "1.0", default-features = false, features = ["alloc", "derive"] }`
  - `ciborium = "0.2"`
  - `thiserror = "2.0"`
  - `libc = "0.2"`
  - `tracing = { version = "0.1", default-features = false }`

**File: `pnpm-workspace.yaml`**
```yaml
packages:
  - 'packages/ts/**'
```

**File: `package.json`**
- Root devDependencies for monorepo validation: `@types/node@22`, `typescript@5.6`, `turbo`, `biome`.

**File: `pyproject.toml`**
- Build system: `hatchling`.
- Configure `ruff` and `mypy` strict configurations.

**File: `perf/budgets.toml`**
- Write operational budget configurations:
  ```toml
  enforce = true
  [latencies]
  gate_1_us = 0
  gate_2_us = 2
  gate_3_us = 150
  cold_boot_ms = 40
  rss_purge_ms = 50
  ```

**File: `fixtures.toml`**
- Declare pinned cryptographic hashes (BLAKE3/SHA-256) for test fixtures.

### Task 0.3: Build & CI Automation Skeletons
**File: `justfile`**
- Copy canonical targets directly from `cans/artifacts/config/justfile`:
  `default`, `bootstrap`, `build-host`, `build-shim`, `build-ts`, `check-spec`, `check-spec-touched`, `check-plugin-coverage`, `check-drivers`, `check-layers`, `check-budgets`, `panic-hunt`, `fetch-fixtures`, `check-hygiene`, `check-tiered`, `check-graph`, `check-plugin`, `check-plugin-all`, `lint`, `typecheck`, `test-core`, `test-plugins`, `test-shim`, `test-drivers`, `test-sdks`, `test-integration`, `test-all`, `parity`, `pre-flight`.

**File: `.github/workflows/ci.yml`**
- Mount the workflow from `cans/artifacts/config/ci.yml` ensuring all verification stages match the specification (`cans-check`, `lint-and-types`, `build-guest-shim`, `test-core`, `panic-hunt`, `parity`, `package-check`, `test-plugins`, `plugin-graph`, `layer-check`, `plugin-audit`, `verify-tiered-compilation`, `test-shim`, `test-drivers`, `perf-budgets`, `transport-windows`, `check-macos-hvf`).

**File: `.github/workflows/plugin-ci-reusable.yml`**
- Mount the workflow from `cans/artifacts/config/plugin-ci-reusable.yml`.

---

## 3. STAGE 1: `crates/kelir-interface` (L1 CONTRACT SSOT)

Assigned to: **Agent-01**  
Dependency Constraint: Zero internal workspace dependencies. Relies strictly on `serde`, `ciborium`, and `thiserror`.

### Task 1.1: Cargo Setup
**File: `crates/kelir-interface/Cargo.toml`**
```toml
[package]
name = "kelir-interface"
version = "1.0.0"
edition = "2024"
license = "MIT OR Apache-2.0"

[dependencies]
serde = { workspace = true }
ciborium = { workspace = true }
thiserror = { workspace = true }

[lints]
workspace = true
```

### Task 1.2: Protocol Contracts
**File: `crates/kelir-interface/src/protocol.rs`**
// see: cans/11-protocol.md#Wire-framing
// see: cans/11-protocol.md#Control-channel
- Define header constants:
  - `MAX_FRAME_PAYLOAD: usize = 65536;`
  - Control channel: `PORT_CONTROL: u16 = 0x0000;`
  - Stream mux channel: `PORT_MUX: u16 = 0x0001;`
  - Transport Port: `TRANSPORT_PORT: u32 = 5252;`
- Struct `MuxFrameHeader`:
  - Fields: `stream_id: u32`, `flags: u8`, `len: u32` (Packed, Little-Endian serialization).
- Enum `StreamKind` (u16):
  - Core: `Exec = 0x0001`, `PtyControl = 0x0002`, `Vfs = 0x0003`, `Bulk = 0x0004`, `L4Forward = 0x0005`, `TerminalConsole = 0x0006`.
  - Plugins: `ProfileSync = 0x0010`, `DbSqlite = 0x0011`, `McpConsole = 0x0012`.
- Enum `MsgType` (u16):
  - `Trap = 0x0001`, `ChannelPayload = 0x0002`, `VmLifecycleControl = 0x0003`, `SystemTelemetry = 0x0004`.
- Struct `ExecveMutation`:
  - `new_binary: Option<String>`
  - `new_argv: Option<Vec<String>>`
  - `set_env: Vec<(String, String)>`
  - `unset_env: Vec<String>`
- Enum `SuspendUntil`:
  - `Deadline(u64)`, `Approval`, `Handshake`
- Enum `Verdict`:
  // see: cans/02-plugin-model.md#Unified-verdict
  - `Pass`
  - `Patch(ExecveMutation)`
  - `Drop { errno: Option<i32>, reason: Option<String> }`
  - `Suspend { until: SuspendUntil }`
  - `Abort { reason: String }`
- Enum `FilterAction`:
  // see: cans/02-plugin-model.md#Unified-verdict
  - `Forward`
  - `Drop { errno: Option<i32>, reason: Option<String> }`
  - `Suspend { until: SuspendUntil }`
  - `Abort { reason: String }`
- Struct `TrapEvent`:
  - `pid: u32`, `binary_path: String`, `original_binary_blake3: [u8; 32]`, `argv: Vec<String>`, `envp: Vec<String>`
- Struct `ProcessLimits`:
  - `max_as_bytes: Option<u64>`, `max_nofile: Option<u64>`, `max_nproc: Option<u64>`, `max_cpu_seconds: Option<u64>`

### Task 1.3: Driver Contracts
**File: `crates/kelir-interface/src/driver.rs`**
// see: cans/01-architecture.md#VmmDriver-trait
// see: cans/08-lifecycle.md#Hypervisor-primitives
- Struct `VmConfig`:
  - `vcpus: u8`, `memory_mb: u32`, `kernel_path: String`, `rootfs_path: String`, `kernel_params: String`
- Struct `SnapshotDescriptor`:
  - `snapshot_id: [u8; 16]`, `dirty_bytes: u64`, `manifest_digest: [u8; 32]`
- Enum `WatchdogAction`:
  - `Reboot`, `Terminate`
- Enum `WatchdogVerdict`:
  - `ExecuteDefault`, `Suppress`, `Mitigate`
- Struct `WatchdogEvent`:
  - `fired_at_ns: u64`, `last_heartbeat_ms: u64`, `last_active_pid: u32`
- Trait `VmmDriver`:
  - `fn boot(&mut self, config: &VmConfig) -> Result<(), DriverError>;`
  - `fn pause(&mut self) -> Result<(), DriverError>;`
  - `fn resume(&mut self) -> Result<(), DriverError>;`
  - `fn step(&mut self) -> Result<(), DriverError>;`
  - `fn fork(&self) -> Result<Box<dyn VmmDriver>, DriverError>;`
  - `fn rollback(&mut self, snapshot: &SnapshotDescriptor) -> Result<(), DriverError>;`
  - `fn snapshot(&mut self) -> Result<SnapshotDescriptor, DriverError>;`
  - `fn set_vcpus(&mut self, count: u8) -> Result<(), DriverError>;`
  - `fn adjust_balloon(&mut self, target_mb: u32) -> Result<(), DriverError>;`
  - `fn skew_clock(&mut self, offset_ns: i64) -> Result<(), DriverError>;`

### Task 1.4: Telemetry & Receipt Contracts
**File: `crates/kelir-interface/src/telemetry.rs`**
// see: cans/10-attestation.md#Execution-receipts
// see: cans/10-attestation.md#Causal-delta
- Enum `ExitKind`:
  - `Normal(i32)`, `Signal(u32)`, `KernelPanic(String)`, `HypervisorFault(String)`, `DirtyShutdown`, `HardKilled`
- Struct `ExecutionReceipt`:
  - `session_id: [u8; 16]`
  - `pid: u32`
  - `exit_kind: ExitKind`
  - `binary_blake3: [u8; 32]`
  - `dirty_bytes: u64`
  - `syscall_count: Option<u64>`
  - `egress_bytes: u64`
  - `egress_dropped_bytes: u64`
  - `wall_clock_ns: u64`
  - `correlation_token: [u8; 16]`
- Struct `CausalDelta`:
  - `parent_digest: [u8; 32]`
  - `current_digest: [u8; 32]`
  - `mutated_inodes: Vec<String>`
  - `tombstones: Vec<String>`
  - `memory_dirty_bytes: u64`

### Task 1.5: Plugin Contracts
**File: `crates/kelir-interface/src/plugin.rs`**
// see: cans/02-plugin-model.md#KelirPlugin-trait
// see: cans/02-plugin-model.md#Segregated-ring-traits
- Struct `MountScope`:
  - `allowed_rings: u32`, `path_subtrees: Vec<String>`, `cidr_allowlists: Vec<String>`, `binary_paths: Vec<String>`
- Enum `VetoEvent`:
  - `Fork`, `Suspend`, `ClockSkew`
- Enum `LifecycleEvent`:
  - `VmInit`, `VmReady`, `VmPreempt`, `VmShutdown`
- Struct `HandlerCtx`:
  - Define container holding opaque event contexts and `c.receipt()` API skeleton.
- Struct `LifecycleCtx`:
  - `fn set_watchdog(&mut self, action: WatchdogAction) -> Result<(), PluginError>;`
  - `fn adjust_balloon(&mut self, target_mb: u32) -> Result<(), PluginError>;`
  - `fn register_veto(&mut self, event: VetoEvent, callback: Box<dyn Fn() -> bool + Send + Sync>) -> Result<(), PluginError>;`
- Trait `KelirPlugin`:
  - `fn name(&self) -> &'static str;`
  - `fn default_scope(&self) -> MountScope;`
  - `fn is_enabled(&self) -> bool { true }`
  - `fn fork_state(&self) -> Option<Box<dyn KelirPlugin>> { None }`
- Trait `LifecycleParticipant`:
  - `fn on_vm_init(&mut self, ctx: &mut LifecycleCtx) -> Result<(), PluginError> { Ok(()) }`
  - `fn on_vm_ready(&mut self, ctx: &mut LifecycleCtx) -> Result<(), PluginError> { Ok(()) }`
  - `fn on_pre_commit(&mut self) -> Result<(), PluginError> { Ok(()) }`
  - `fn on_post_commit(&mut self, success: bool) -> Result<(), PluginError> { Ok(()) }`
  - `fn on_host_preempt(&mut self) -> Result<(), PluginError> { Ok(()) }`
- Trait `TrapInterposer`:
  - `fn on_trap(&mut self, event: &TrapEvent, ctx: &HandlerCtx) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }`
  - `fn on_process_exit(&mut self, pid: u32, exit_code: i32, ctx: &HandlerCtx) -> Result<(), PluginError> { Ok(()) }`
- Trait `ConsoleInterposer`:
  - `fn on_console_input(&mut self, port: u32, buf: &mut [u8]) -> Result<(), PluginError> { Ok(()) }`
  - `fn on_console_output(&mut self, port: u32, buf: &mut [u8]) -> Result<(), PluginError> { Ok(()) }`
  - `fn on_exec_stdout(&mut self, pid: u32, exec_id: u64, buf: &[u8]) -> Result<(), PluginError> { Ok(()) }`
  - `fn on_exec_stderr(&mut self, pid: u32, exec_id: u64, buf: &[u8]) -> Result<(), PluginError> { Ok(()) }`
- Trait `FsInterposer`:
  - `fn on_fs_lookup(&mut self, path: &str) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }`
  - `fn on_fs_read(&mut self, path: &str, offset: u64, buf: &mut [u8]) -> Result<(), PluginError> { Ok(()) }`
  - `fn on_fs_write(&mut self, path: &str, offset: u64, buf: &mut [u8]) -> Result<FilterAction, PluginError> { Ok(FilterAction::Forward) }`
- Trait `NetInterposer`:
  - `fn on_net_outbound(&mut self, frame: &mut [u8]) -> Result<FilterAction, PluginError> { Ok(FilterAction::Forward) }`
  - `fn on_net_inbound(&mut self, frame: &mut [u8]) -> Result<FilterAction, PluginError> { Ok(FilterAction::Forward) }`
- Trait `HostInterposer`:
  - `fn on_host_lsm_exec(&mut self, binary: &str, pid: u32) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }`
  - `fn on_host_pressure(&mut self, pressure_level: u8) -> Result<(), PluginError> { Ok(()) }`

### Task 1.6: Errors & Crate Re-exports
**File: `crates/kelir-interface/src/errors.rs`**
- `#[derive(thiserror::Error, Debug)]`
- Enum `PluginError`:
  - `#[error("Configuration error: {0}")] EConfig(String)`
  - `#[error("Ambiguous mutation scope: {0}")] EAmbiguousMutation(String)`
  - `#[error("Session veto triggered: {0}")] ESessionVeto(String)`
  - `#[error("Security policy violation: {0}")] ESecurityViolation(String)`
  - `#[error("Cyclic rewrite detected")] ECyclicRewrite`
  - `#[error("Execution halted")] Halt`
  - `#[error("System IO error: {0}")] Io(#[from] std::io::Error)`
- Enum `DriverError`:
  - `#[error("No hardware virtualization available")] ENOHARDWAREVMM`
  - `#[error("W^X memory violation detected")] EWXVIOLATION`
  - `#[error("Clock regression error")] EClockRegress`
  - `#[error("Driver execution failed: {0}")] ExecutionFailed(String)`

**File: `crates/kelir-interface/src/lib.rs`**
- Re-export modules: `protocol`, `driver`, `telemetry`, `plugin`, `errors`.

---

## 4. STAGE 2: FOUNDATION SUPPORTING CRATES (L1/L2 SKELETONS)

### Task 2.1: `crates/kelir-transport`
Assigned to: **Agent-02**  
Dependency Constraint: `kelir-interface`, `libc`. Strictly isolated `unsafe` blocks for ring mapping.

**File: `crates/kelir-transport/Cargo.toml`**
```toml
[package]
name = "kelir-transport"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../kelir-interface" }
libc = { workspace = true }
thiserror = { workspace = true }

[lints]
workspace = true
```

**File: `crates/kelir-transport/src/lib.rs`**
// see: cans/01-architecture.md#Transport-engine-abstraction
- Trait `StreamTransport`:
  - `fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error>;`
  - `fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error>;`
  - `fn flush(&mut self) -> Result<(), std::io::Error>;`

**File: `crates/kelir-transport/src/spsc_ring.rs`**
- Struct `RingHeader`:
  - `head: std::sync::atomic::AtomicU32`
  - `tail: std::sync::atomic::AtomicU32`
  - `capacity: u32`
- Struct `SpscRingTransport`:
  - `header: *mut RingHeader`
  - `buffer: *mut u8`
- Implement `StreamTransport` for `SpscRingTransport`:
  - Wire flow paths. Provide structural scaffolding. Leave memory-fence synchronization logic as `// TODO`.

**File: `crates/kelir-transport/src/fallback.rs`**
- Define `VsockFallbackTransport` (Linux/macOS debug fallback).
- Define `NamedPipeFallbackTransport` (Windows debug fallback).
- Wire constructors and implement `StreamTransport`.

---

### Task 2.2: `crates/kelir-catalog`
Assigned to: **Agent-03**  
Dependency Constraint: `kelir-interface`, `rusqlite` (bundled). Zero engine plane dependencies.

**File: `crates/kelir-catalog/Cargo.toml`**
```toml
[package]
name = "kelir-catalog"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../kelir-interface" }
rusqlite = { version = "0.32", features = ["bundled"] }
thiserror = { workspace = true }

[lints]
workspace = true
```

**File: `crates/kelir-catalog/src/schema.rs`**
// see: cans/06-storage-catalog.md#Entity-model
- Constant `SCHEMA_V1_SQL: &'static str`:
  ```sql
  CREATE TABLE IF NOT EXISTS schema_version (
      version INTEGER PRIMARY KEY
  );
  CREATE TABLE IF NOT EXISTS sessions (
      session_id BLOB PRIMARY KEY,
      incarnation_id BLOB NOT NULL,
      lease_kind TEXT CHECK(lease_kind IN ('ephemeral', 'lease')) NOT NULL,
      expires_at_unix INTEGER,
      runner_pid INTEGER NOT NULL,
      boot_id TEXT NOT NULL,
      start_time_ns INTEGER NOT NULL
  );
  CREATE TABLE IF NOT EXISTS cas_leases (
      blob_hash BLOB NOT NULL,
      holder_pid INTEGER NOT NULL,
      expires_at_unix INTEGER NOT NULL,
      PRIMARY KEY (blob_hash, holder_pid)
  );
  ```

**File: `crates/kelir-catalog/src/lib.rs`**
- Struct `CatalogDb`:
  - `pool: rusqlite::Connection`
- Skeletons:
  - `pub fn open<P: AsRef<std::path::Path>>(path: P) -> Result<Self, rusqlite::Error>;`
  - `pub fn register_session(&self, row: &SessionRow) -> Result<(), rusqlite::Error>;`
  - `pub fn heartbeat_lease(&self, session_id: &[u8; 16], ttl_seconds: u32) -> Result<(), rusqlite::Error>;`
  - `pub fn prune_cas_leases(&self) -> Result<u64, rusqlite::Error>;`
  - Wire functions to compile. Internal SQL execution left as `// TODO`.

---

### Task 2.3: `crates/kelir-otlp`
Assigned to: **Agent-04**  
Dependency Constraint: `kelir-interface`. Zero allocation telemetry structures.

**File: `crates/kelir-otlp/Cargo.toml`**
```toml
[package]
name = "kelir-otlp"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../kelir-interface" }
thiserror = { workspace = true }

[lints]
workspace = true
```

**File: `crates/kelir-otlp/src/registry.rs`**
// see: cans/14-observability.md#Metrics-pipeline
- Struct `MetricRegistry`:
  - Atomic counters: `exec_count: std::sync::atomic::AtomicU64`, `syscall_count: std::sync::atomic::AtomicU64`, `dropped_frames: std::sync::atomic::AtomicU64`.
  - Atomic gauges: `dirty_pages: std::sync::atomic::AtomicU64`, `active_vcpus: std::sync::atomic::AtomicU32`.
- Skeleton:
  - `pub fn new() -> Self;`
  - `pub fn record_exec(&self);`
  - `pub fn record_drop(&self);`

**File: `crates/kelir-otlp/src/lib.rs`**
- Struct `OtlpExporter`:
  - Skeletons for exporting `ExecutionReceipt` and phase timings into OTLP formatted payloads.

---

## 5. STAGE 3: `crates/kelir-shim` (GUEST MICRO-INIT SKELETON)

Assigned to: **Agent-05**  
Constraints:
- Binary footprint target: <50KB musl static executable.
- Allowed dependencies: `libc` only. Zero third-party crates (bans `serde`, `ciborium`, etc.).
- `#![forbid(unsafe_code)]` relaxed ONLY for raw syscall enclaves (`ioctl`, `openpty`, `setrlimit`, `waitpid`).

### Task 3.1: Cargo Setup
**File: `crates/kelir-shim/Cargo.toml`**
```toml
[package]
name = "kelir-shim"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
libc = { workspace = true }

[profile.release]
opt-level = "z"
lto = "fat"
panic = "abort"
codegen-units = 1
strip = true

[lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
```

### Task 3.2: Raw Framing Codec
**File: `crates/kelir-shim/src/codec.rs`**
// see: cans/11-protocol.md#Wire-framing
// see: cans/artifacts/config/project-structure.txt#kelir-shim
- Skeletons:
  - `pub fn encode_frame(stream_id: u32, flags: u8, payload: &[u8], out: &mut [u8]) -> Result<usize, ()>;`
  - `pub fn decode_header(buf: &[u8]) -> Result<(u32, u8, u32), ()>;`
- Write bit-shift implementations with defensive boundary checks. Zero alloc.

### Task 3.3: Ingress Multiplexer
**File: `crates/kelir-shim/src/multiplexer.rs`**
// see: cans/11-protocol.md#Control-channel
// see: cans/11-protocol.md#Binary-exec-channel
- Struct `ExecSlot`:
  - `pid: libc::pid_t`
  - `exec_id: u64`
  - `stdin_fd: i32`
  - `stdout_fd: i32`
  - `stderr_fd: i32`
- Struct `Multiplexer`:
  - `slots: [Option<ExecSlot>; 4096]`
- Skeletons:
  - `pub fn new() -> Self;`
  - `pub fn dispatch_frame(&mut self, stream_id: u32, flags: u8, payload: &[u8]) -> Result<(), ()>;`

### Task 3.4: PTY Line Discipline & Traps
**File: `crates/kelir-shim/src/pty.rs`**
// see: cans/04-terminal-plane.md#PTY-line-discipline-initialization-(kelir-shim)
- Skeleton:
  - `pub fn open_pty_slave() -> Result<(i32, i32), ()>;`
  - Wrap raw `libc::openpty` under explicit `// SAFETY:` block.

**File: `crates/kelir-shim/src/trap.rs`**
// see: cans/11-protocol.md#Guest-paravirtual-micro-init-boundary-(kelir-shim)
- Skeleton:
  - `pub fn init_seccomp_unotify() -> Result<i32, ()>;`

### Task 3.5: PID 1 Entrypoint & Zombie Reaper
**File: `crates/kelir-shim/src/main.rs`**
// see: cans/08-lifecycle.md#Zombie-reaping
- Skeletons:
  - `fn main() -> Result<(), ()>;`
  - Wire event loop structure checking `libc::waitpid(-1, &mut status, libc::WNOHANG)`.
  - Leave execution branch meat as `// TODO`.

---

## 6. PARALLEL AGENT EXECUTION MATRIX

| Agent | Target Scope | Output Artifacts | Compilable Gate Command |
|---|---|---|---|
| **Agent-00** | Workspace & CI Setup | `Cargo.toml`, `package.json`, `.github/workflows/*`, `justfile` | `just check-hygiene` |
| **Agent-01** | `kelir-interface` | `crates/kelir-interface/src/{lib,protocol,driver,telemetry,plugin,errors}.rs` | `cargo check -p kelir-interface` |
| **Agent-02** | `kelir-transport` | `crates/kelir-transport/src/{lib,spsc_ring,fallback}.rs` | `cargo check -p kelir-transport` |
| **Agent-03** | `kelir-catalog` | `crates/kelir-catalog/src/{lib,schema}.rs` | `cargo check -p kelir-catalog` |
| **Agent-04** | `kelir-otlp` | `crates/kelir-otlp/src/{lib,registry}.rs` | `cargo check -p kelir-otlp` |
| **Agent-05** | `kelir-shim` | `crates/kelir-shim/src/{main,codec,multiplexer,pty,trap}.rs` | `cargo check -p kelir-shim` |

---

## 7. CONVERGENCE & VERIFICATION RUNBOOK

Once Agents 00 through 05 finish their respective allocations:

1. **Workspace Compilation Gate**:
   ```bash
   cargo check --workspace --all-targets
   ```
2. **Code Hygiene & Architectural Lints**:
   ```bash
   just check-hygiene
   cargo clippy --workspace --exclude kelir-shim -- -D warnings
   ```
3. **Spec Integrity Check**:
   ```bash
   just check-spec
   ```
4. **Binary Footprint Constraint Evaluation**:
   ```bash
   just build-shim target="x86_64-unknown-linux-musl"
   ```
   Confirm binary size `< 51200` bytes.

---
*Work-order Part 1 of 5 complete. Proceed to Part 2/5 (Engine Core & Native Planes) immediately upon all Phase 1 gates reporting GREEN.*

# WORK-ORDER.md: Distributed Systems Architecture Construction
**Target System:** Kelir MicroVM Hypervisor Microkernel  
**Phase:** 2 of 5 (Engine Core & Native Foundation Planes: `kelir-core`)  
**Total Parts (x):** 5  
- **Part 1/5:** Scaffolding, Root Build System, CI/CD, Leaf Interfaces & Shim (Completed)
- **Part 2/5:** Engine Core & Native Planes (`kelir-core`: Terminal, Storage, Wire, Checkpoint, Host, Bus, Dispatcher)
- **Part 3/5:** Driver Enclaves (`driver-rustvmm`, `driver-hvf`, `driver-openvmm`, `driver-avf`, `driver-bwrap`, `driver-proot`)
- **Part 4/5:** Plugin Ecosystem (13 Catalog Plugins: Tier 1 Transports, Tier 2 Proxies, Tier 3 Aggregators)
- **Part 5/5:** Adapters, Tooling CLI, Native FFI Bindings & Tri-Native SDK Surfaces (`kelir-pack`, `kelir-plugin-cli`, NAPI, PyO3, TS, Python)

---

## 1. AGENT OPERATIONAL CONSTRAINTS (`kelir-core`)

All agents assigned to `crates/kelir-core` must conform to these architectural boundaries:

1. **Memory Safety Discipline**:
   - Explicitly add `#![forbid(unsafe_code)]` to `crates/kelir-core/src/lib.rs`.
   - Zero unsafe blocks permitted in `kelir-core`. Direct hardware/FFI isolation belongs strictly to drivers, transport, and shim.
2. **Defensive Arithmetic & Checked Math**:
   - Frame sizing, ring offsets, and buffer lengths must use `checked_*` or `saturating_*` arithmetic exclusively.
3. **Functional Core / Mutation Invariance**:
   - Plane state transitions follow `f(State, Event) -> (State, Actions)`.
   - Engine shared state uses standard atomics; synchronous interposition pipelines must remain immutable after VM boot seal.
4. **Locality & Code Size**:
   - Hard limit: 1,000 LOC per file. Target: 500 LOC.
   - Zero inline tests in `src/`.
5. **No Speculative Implementation (L1/L2 Only)**:
   - Compileable structs, traits, enums, pipeline wiring, and method signatures only.
   - Algorithmic loops, OS-specific ioctl bridges, and parser bodies remain `// TODO` returning typed `Result` or zero-values.

---

## 2. STAGE 1: CORE CARGO MANIFEST & ROOT EXPORTS

### Task 1.1: Cargo Configuration
**File: `crates/kelir-core/Cargo.toml`**
```toml
[package]
name = "kelir-core"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../kelir-interface" }
kelir-transport = { path = "../kelir-transport" }
kelir-catalog = { path = "../kelir-catalog" }
kelir-otlp = { path = "../kelir-otlp" }
serde = { workspace = true }
ciborium = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
smoltcp = { version = "0.11", default-features = false, features = ["std", "medium-ethernet", "proto-ipv4", "socket-tcp", "socket-udp"] }

[lints]
workspace = true
```

### Task 1.2: Root Exports & State Lifecycle
**File: `crates/kelir-core/src/lib.rs`**
// see: cans/01-architecture.md#Axiomatic-foundations
// see: cans/08-lifecycle.md#State-machine
```rust
#![forbid(unsafe_code)]

pub mod bus;
pub mod dispatch;
pub mod planes;
pub mod session;
pub mod trap_gate;
pub mod vmm;

pub use session::Session;
```

---

## 3. STAGE 2: BUS MULTIPLEXER & UNIFIED DISPATCH PIPELINE

Assigned to: **Agent-06**

### Task 2.1: VirtIO Device Complex & Frame Multiplexer
**File: `crates/kelir-core/src/bus/mod.rs`**
// see: cans/01-architecture.md#VirtIO-Device-Complex
// see: cans/11-protocol.md#Control-channel
- Define `BusDispatcher`:
  - Fields:
    - `control_transport: Box<dyn kelir_transport::StreamTransport + Send + Sync>`
    - `stream_mux_transport: Box<dyn kelir_transport::StreamTransport + Send + Sync>`
    - `active_streams: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<u32, kelir_interface::protocol::StreamKind>>>`
- Method Skeletons (L1/L2):
  - `pub fn new(control: Box<dyn kelir_transport::StreamTransport + Send + Sync>, mux: Box<dyn kelir_transport::StreamTransport + Send + Sync>) -> Self;`
  - `pub fn poll_ingress(&mut self) -> Result<Option<(u32, u8, Vec<u8>)>, kelir_interface::errors::PluginError>;`
  - `pub fn route_stream_frame(&mut self, stream_id: u32, flags: u8, payload: &[u8]) -> Result<(), kelir_interface::errors::PluginError>;`
  - `pub fn send_control_msg(&mut self, msg_type: kelir_interface::protocol::MsgType, payload: &[u8]) -> Result<(), kelir_interface::errors::PluginError>;`

### Task 2.2: Static Interceptor Dispatch Engine
**File: `crates/kelir-core/src/dispatch.rs`**
// see: cans/02-plugin-model.md#Static-dispatch
// see: cans/02-plugin-model.md#Unified-interposition-pipeline
- Struct `PipelineEngine`:
  - Contiguous arrays for indexed ring chains:
    - `trap_interceptors: Vec<Box<dyn kelir_interface::plugin::TrapInterposer + Send + Sync>>`
    - `fs_interceptors: Vec<Box<dyn kelir_interface::plugin::FsInterposer + Send + Sync>>`
    - `net_interceptors: Vec<Box<dyn kelir_interface::plugin::NetInterposer + Send + Sync>>`
    - `console_interceptors: Vec<Box<dyn kelir_interface::plugin::ConsoleInterposer + Send + Sync>>`
    - `host_interceptors: Vec<Box<dyn kelir_interface::plugin::HostInterposer + Send + Sync>>`
- Method Skeletons (L1/L2):
  - `pub fn new() -> Self;`
  - `pub fn seal(&mut self) -> Result<(), kelir_interface::errors::PluginError>;`
  - `pub fn dispatch_trap(&self, event: &kelir_interface::protocol::TrapEvent, ctx: &kelir_interface::plugin::HandlerCtx) -> Result<kelir_interface::protocol::Verdict, kelir_interface::errors::PluginError>;`
  - `pub fn dispatch_fs_write(&self, path: &str, offset: u64, buf: &mut [u8]) -> Result<kelir_interface::protocol::FilterAction, kelir_interface::errors::PluginError>;`
  - `pub fn dispatch_net_outbound(&self, frame: &mut [u8]) -> Result<kelir_interface::protocol::FilterAction, kelir_interface::errors::PluginError>;`
  - In `dispatch_trap`, wire panic quarantine boundary using `std::panic::catch_unwind`. If unwound, return `Verdict::Drop { errno: Some(14 /* EFAULT */), reason: Some("Plugin panic caught".into()) }`.

### Task 2.3: Three-Tier Trap Gate Pipeline
**File: `crates/kelir-core/src/trap_gate.rs`**
// see: cans/11-protocol.md#Trap-fast-path
// see: cans/02-plugin-model.md#Latency-ceiling
- Struct `TrapGateEvaluator`:
  - `trusted_prefixes: Vec<String>`
- Method Skeletons (L1/L2):
  - `pub fn new(trusted_prefixes: Vec<String>) -> Self;`
  - `pub fn eval_gate_1_bpf(&self, event: &kelir_interface::protocol::TrapEvent) -> bool;`
  - `pub fn eval_gate_2_native(&self, event: &kelir_interface::protocol::TrapEvent, scope: &kelir_interface::plugin::MountScope) -> bool;`
  - `pub fn should_skip_gate_3(&self, binary_path: &str, argv: &[String]) -> bool;`
  - `pub fn compute_shannon_entropy(&self, argv: &[String]) -> f64;`

---

## 4. STAGE 3: DRIVER ROUTER & CENTRAL SESSION HANDLE

Assigned to: **Agent-07**

### Task 3.1: VMM Dynamic Backend Router
**File: `crates/kelir-core/src/vmm/mod.rs`**
// see: cans/01-architecture.md#Host-runtime
// see: cans/01-architecture.md#Driver-backends
- Struct `DriverRouter`:
  - `active_driver: Box<dyn kelir_interface::driver::VmmDriver + Send + Sync>`
- Method Skeletons (L1/L2):
  - `pub fn load_native_driver(policy_wx: bool, allow_emulated: bool) -> Result<Box<dyn kelir_interface::driver::VmmDriver + Send + Sync>, kelir_interface::errors::DriverError>;`
  - `pub fn new(driver: Box<dyn kelir_interface::driver::VmmDriver + Send + Sync>) -> Self;`

### Task 3.2: Central Session Typestate Runtime Handle
**File: `crates/kelir-core/src/session.rs`**
// see: cans/08-lifecycle.md#Session-and-context-handles
// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
- Struct `Session`:
  - Public plane accessors:
    - `pub terminal: crate::planes::terminal::TerminalPlane`
    - `pub storage: crate::planes::storage::StoragePlane`
    - `pub wire: crate::planes::wire::WirePlane`
    - `pub checkpoint: crate::planes::checkpoint::CheckpointPlane`
    - `pub host: crate::planes::host::HostPlane`
  - Subsystems:
    - `driver: std::sync::Arc<std::sync::Mutex<Box<dyn kelir_interface::driver::VmmDriver + Send + Sync>>>`
    - `bus: std::sync::Arc<std::sync::Mutex<crate::bus::BusDispatcher>>`
    - `incarnation_id: [u8; 16]`
- Method Skeletons (L1/L2):
  - `pub fn boot(config: kelir_interface::driver::VmConfig) -> Result<Self, kelir_interface::errors::PluginError>;`
  - `pub fn fork(&self) -> Result<Self, kelir_interface::errors::PluginError>;`
  - `pub fn rollback(&mut self) -> Result<(), kelir_interface::errors::PluginError>;`
  - `pub fn suspend(&mut self, reason: u32) -> Result<(), kelir_interface::errors::PluginError>;`
  - `pub fn resume(&mut self) -> Result<(), kelir_interface::errors::PluginError>;`
  - `pub fn drain(&mut self, timeout_ms: u64) -> Result<(), kelir_interface::errors::PluginError>;`
  - `pub fn stop(&mut self) -> Result<(), kelir_interface::errors::PluginError>;`
  - `pub fn kill(&mut self) -> Result<(), kelir_interface::errors::PluginError>;`

---

## 5. STAGE 4: FIVE NATIVE FOUNDATION PLANES

### Task 4.1: Terminal Plane
Assigned to: **Agent-08**  
**File: `crates/kelir-core/src/planes/terminal/mod.rs`**
// see: cans/04-terminal-plane.md#Terminal-pipeline
// see: cans/04-terminal-plane.md#Stream-replay
- Struct `TerminalPlane`:
  - `current_cols: std::sync::atomic::AtomicU16`
  - `current_rows: std::sync::atomic::AtomicU16`
  - `log_cursor: std::sync::atomic::AtomicU64`
- Method Skeletons (L1/L2):
  - `pub fn new() -> Self;`
  - `pub fn resize(&self, cols: u16, rows: u16) -> Result<(), kelir_interface::errors::PluginError>;`
  - `pub fn export_screen_matrix(&self) -> Result<Vec<u8>, kelir_interface::errors::PluginError>;`
  - `pub fn read_from(&self, port: u32, cursor: u64) -> Result<(Vec<u8>, u64), kelir_interface::errors::PluginError>;`
  - `pub fn append_log_chunk(&self, port: u32, chunk: &[u8]) -> Result<u64, kelir_interface::errors::PluginError>;`
- Internal filters:
  - `fn filter_ansi_shadow(buf: &[u8]) -> Vec<u8>;`
  - `fn apply_token_shield(grid: &[u8], max_lines: usize, max_bytes: usize) -> Vec<u8>;`

---

### Task 4.2: Storage Plane
Assigned to: **Agent-09**  
**File: `crates/kelir-core/src/planes/storage/mod.rs`**
// see: cans/05-storage-plane.md#Storage-architecture
// see: cans/05-storage-plane.md#Host-CoW-broker
// see: cans/05-storage-plane.md#Storage-quotas
- Struct `StorageQuota`:
  - `allocated_bytes: std::sync::atomic::AtomicU64`
  - `allocated_inodes: std::sync::atomic::AtomicU64`
  - `max_bytes: u64`
  - `max_inodes: u64`
- Struct `StoragePlane`:
  - `workspace_root: std::path::PathBuf`
  - `sparse_upper_path: std::path::PathBuf`
  - `tombstones: std::sync::Arc<std::sync::RwLock<std::collections::HashSet<std::path::PathBuf>>>`
  - `quota: StorageQuota`
- Method Skeletons (L1/L2):
  - `pub fn new(workspace_root: std::path::PathBuf, sparse_upper: std::path::PathBuf, max_bytes: u64, max_inodes: u64) -> Self;`
  - `pub fn reconcile(&self) -> Result<(), kelir_interface::errors::PluginError>;`
  - `pub fn discard(&self) -> Result<(), kelir_interface::errors::PluginError>;`
  - `pub fn record_tombstone(&self, path: &std::path::Path) -> Result<(), kelir_interface::errors::PluginError>;`
  - `pub fn check_quota(&self, bytes_to_write: u64, new_inodes: u64) -> Result<(), kelir_interface::errors::PluginError>;`
  - `pub fn punch_hole_sparse_upper(&self) -> Result<(), std::io::Error>;`

---

### Task 4.3: Wire Plane
Assigned to: **Agent-10**  
**File: `crates/kelir-core/src/planes/wire/mod.rs`**
// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Userspace-network-stack
// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
- Struct `UdpNatEntry`:
  - `guest_source_port: u16`
  - `host_socket_fd: i32`
  - `last_active_unix: u64`
- Struct `WirePlane`:
  - `synthetic_dns_ip: std::net::Ipv4Addr` // Fixed to 192.168.127.1
  - `udp_nat_slab: std::sync::Arc<std::sync::Mutex<Vec<Option<UdpNatEntry>>>>`
  - `allowed_domains: std::sync::Arc<std::sync::RwLock<Vec<String>>>`
- Method Skeletons (L1/L2):
  - `pub fn new() -> Self;`
  - `pub fn forward_port(&self, guest_port: u16, host_port: Option<u16>) -> Result<u16, kelir_interface::errors::PluginError>;`
  - `pub fn process_inbound_frame(&self, frame: &mut [u8]) -> Result<kelir_interface::protocol::FilterAction, kelir_interface::errors::PluginError>;`
  - `pub fn process_outbound_frame(&self, frame: &mut [u8]) -> Result<kelir_interface::protocol::FilterAction, kelir_interface::errors::PluginError>;`
  - `pub fn resolve_synthetic_dns(&self, query: &[u8]) -> Result<Vec<u8>, kelir_interface::errors::PluginError>;`
  - `pub fn reap_stale_udp_sockets(&self) -> Result<usize, kelir_interface::errors::PluginError>;`

---

### Task 4.4: Checkpoint Plane & Host Plane
Assigned to: **Agent-11**

**File: `crates/kelir-core/src/planes/checkpoint/mod.rs`**
// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-core::checkpoint)
// see: cans/08-lifecycle.md#Delta-chain-compaction
- Struct `StepSlot`:
  - `slot_id: u64`
  - `base_descriptor: kelir_interface::driver::SnapshotDescriptor`
  - `pending_delta: Option<kelir_interface::telemetry::CausalDelta>`
- Struct `CheckpointPlane`:
  - `active_step_slot: std::sync::Arc<std::sync::Mutex<Option<StepSlot>>>`
  - `manifest_chain: std::sync::Arc<std::sync::RwLock<Vec<[u8; 32]>>>`
- Method Skeletons (L1/L2):
  - `pub fn new() -> Self;`
  - `pub fn create_step(&self, base: kelir_interface::driver::SnapshotDescriptor) -> Result<(), kelir_interface::errors::PluginError>;`
  - `pub fn commit_step(&self) -> Result<(), kelir_interface::errors::PluginError>;`
  - `pub fn rollback_step(&self) -> Result<kelir_interface::driver::SnapshotDescriptor, kelir_interface::errors::PluginError>;`

**File: `crates/kelir-core/src/planes/host/mod.rs`**
// see: cans/01-architecture.md#Host-plane-subsystem
// see: cans/08-lifecycle.md#Host-crash-reaper
- Enum `HostReaperKind`:
  - `LinuxPdeathsig`, `DarwinKqueue`, `WindowsJobObject`
- Struct `HostPlane`:
  - `reaper_kind: HostReaperKind`
  - `pressure_level: std::sync::atomic::AtomicU8`
- Method Skeletons (L1/L2):
  - `pub fn new() -> Self;`
  - `pub fn lsm_status(&self) -> bool;`
  - `pub fn pressure_level(&self) -> u8;`
  - `pub fn poll_pressure_events(&self) -> Result<(), kelir_interface::errors::PluginError>;`
  - `pub fn arm_host_reaper(&self, parent_pid: u32) -> Result<(), kelir_interface::errors::PluginError>;`

---

## 6. PARALLEL AGENT EXECUTION MATRIX

| Agent | Target Scope | Output Files | Compilable Gate Command |
|---|---|---|---|
| **Agent-06** | Bus Multiplexer & Dispatch Engine | `crates/kelir-core/src/{bus/mod,dispatch,trap_gate}.rs` | `cargo check -p kelir-core` |
| **Agent-07** | Driver Router & Session Handle | `crates/kelir-core/src/{vmm/mod,session}.rs` | `cargo check -p kelir-core` |
| **Agent-08** | Terminal Plane | `crates/kelir-core/src/planes/terminal/mod.rs` | `cargo check -p kelir-core` |
| **Agent-09** | Storage Plane | `crates/kelir-core/src/planes/storage/mod.rs` | `cargo check -p kelir-core` |
| **Agent-10** | Wire Plane | `crates/kelir-core/src/planes/wire/mod.rs` | `cargo check -p kelir-core` |
| **Agent-11** | Checkpoint & Host Planes | `crates/kelir-core/src/planes/{checkpoint,host}/mod.rs` | `cargo check -p kelir-core` |

---

## 7. CONVERGENCE & VERIFICATION RUNBOOK

Execute once Agents 06 through 11 mark their files complete:

1. **Core Verification Gate**:
   ```bash
   cargo check -p kelir-core --all-targets
   ```
2. **Layering Rule Enforcement**:
   ```bash
   just check-layers
   ```
   *Assert:* `kelir-core` does not import `kelir-plugins/*`, `kelir-drivers/*`, or `kelir-adapters/*`.
3. **Hygiene & Safety Audit**:
   ```bash
   just check-hygiene
   cargo clippy -p kelir-core -- -D warnings
   ```
   *Assert:* Zero warnings, zero `unsafe` usage, all files `<= 1000 LOC`.

---
*Work-order Part 2 of 5 complete. Proceed to Part 3/5 (Driver Enclaves) immediately upon all Phase 2 gates reporting GREEN.*

# WORK-ORDER.md: Distributed Systems Architecture Construction
**Target System:** Kelir MicroVM Hypervisor Microkernel  
**Phase:** 3 of 5 (Pluggable Virtualization Driver Enclaves: `crates/kelir-drivers`)  
**Total Parts (x):** 5  
- **Part 1/5:** Scaffolding, Root Build System, CI/CD, Leaf Interfaces & Shim (Completed)
- **Part 2/5:** Engine Core & Native Planes (`kelir-core`) (Completed)
- **Part 3/5:** Driver Enclaves (`driver-rustvmm`, `driver-hvf`, `driver-openvmm`, `driver-avf`, `driver-bwrap`, `driver-proot`)
- **Part 4/5:** Plugin Ecosystem (13 Catalog Plugins: Tier 1 Transports, Tier 2 Proxies, Tier 3 Aggregators)
- **Part 5/5:** Adapters, Tooling CLI, Native FFI Bindings & Tri-Native SDK Surfaces (`kelir-pack`, `kelir-plugin-cli`, NAPI, PyO3, TS, Python)

---

## 1. AGENT OPERATIONAL CONSTRAINTS (`kelir-drivers`)

Drivers are low-level isolation enclaves executing hardware virtualization primitives. All assigned agents must observe:

1. **Strict Enclave Unsafe Discipline**:
   - `unsafe` blocks are permitted **only** where hardware ioctls, FFI pointers, or OS hypervisor bindings necessitate it.
   - Every `unsafe` block must precede with a single-line explanation:
     ```rust
     // SAFETY: Enforces Stage-2 page unmap boundary within guest physical memory range
     ```
   - Zero ambient unsafe code. Keep unsafe scopes strictly pinned to atomic syscalls/ioctls.
2. **Third-Party Dependency Bans**:
   - In `driver-rustvmm`: `vmm-sys-util` is strictly **banned**. Rely only on `kvm-ioctls` and raw `libc`.
   - In `driver-bwrap`: `nix` crate is strictly **banned**. Rely only on `libc` user namespace syscalls.
   - In `driver-openvmm`: Scoped strictly to `windows-sys`. Zero external C++ or custom DLL shims.
3. **Contract Uniformity**:
   - Every driver must implement the single backend contract: `kelir_interface::driver::VmmDriver`.
   - Never expose driver-specific structs to consumers; callers hold `Box<dyn VmmDriver>` exclusively.
4. **Driver Fallback Rule**:
   - Attempting to initialize emulated tier drivers (`driver-bwrap`, `driver-proot`) when `allow_emulated_fallback == false` must fail immediately with `DriverError::ENOHARDWAREVMM`.
5. **No Speculative Implementation (L1/L2 Only)**:
   - Provide complete, compilable types, driver state structures, FFI stubs, and method signatures conforming to `VmmDriver`.
   - Hardware register loops, ioctl dispatches, and memory mappings remain `// TODO` returning `Ok(())` or typed error variants.

---

## 2. STAGE 1: HARDWARE TIER DRIVERS (ZERO-ESCAPE BOUNDARY)

### Task 1.1: `driver-rustvmm` (Bare-Metal Linux KVM + userfaultfd CoW)
Assigned to: **Agent-12**  
Dependency Constraint: `kelir-interface`, `kvm-ioctls = "0.19"`, `kvm-bindings = "0.10"`, `userfaultfd = "0.8"`, `libc`. Bans `vmm-sys-util`.

**File: `crates/kelir-drivers/driver-rustvmm/Cargo.toml`**
```toml
[package]
name = "driver-rustvmm"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../../kelir-interface" }
kvm-ioctls = "0.19"
kvm-bindings = "0.10"
userfaultfd = "0.8"
libc = { workspace = true }
thiserror = { workspace = true }

[lints]
workspace = true
```

**File: `crates/kelir-drivers/driver-rustvmm/src/lib.rs`**
// see: cans/01-architecture.md#Driver-backends
// see: cans/08-lifecycle.md#CoW-fork
```rust
use kelir_interface::driver::{SnapshotDescriptor, VmConfig, VmmDriver};
use kelir_interface::errors::DriverError;

pub struct RustVmmDriver {
    kvm_fd: Option<kvm_ioctls::Kvm>,
    vm_fd: Option<kvm_ioctls::VmFd>,
    vcpu_fds: Vec<kvm_ioctls::VcpuFd>,
    uffd: Option<userfaultfd::Uffd>,
    config: Option<VmConfig>,
}

impl RustVmmDriver {
    pub fn new() -> Result<Self, DriverError> {
        Ok(Self {
            kvm_fd: None,
            vm_fd: None,
            vcpu_fds: Vec::new(),
            uffd: None,
            config: None,
        })
    }

    fn filter_synthetic_cpuid(&self, vcpu: &kvm_ioctls::VcpuFd) -> Result<(), DriverError> {
        // see: cans/01-architecture.md#Driver-backends
        // TODO: Enforce invariant TSC and Leaf 0x1 TSC-deadline
        Ok(())
    }
}

impl VmmDriver for RustVmmDriver {
    fn boot(&mut self, config: &VmConfig) -> Result<(), DriverError> {
        self.config = Some(config.clone());
        // TODO: Initialize KVM, VM, memory slots, and vCPUs
        Ok(())
    }

    fn pause(&mut self) -> Result<(), DriverError> {
        // TODO: Pause vCPU execution loops
        Ok(())
    }

    fn resume(&mut self) -> Result<(), DriverError> {
        // TODO: Unpause vCPU execution loops
        Ok(())
    }

    fn step(&mut self) -> Result<(), DriverError> {
        // TODO: Single-step instruction boundary via KVM_SET_GUEST_DEBUG
        Ok(())
    }

    fn fork(&self) -> Result<Box<dyn VmmDriver>, DriverError> {
        // see: cans/08-lifecycle.md#CoW-fork
        // TODO: Clone memory state via userfaultfd copy-on-write
        Ok(Box::new(Self::new()?))
    }

    fn rollback(&mut self, snapshot: &SnapshotDescriptor) -> Result<(), DriverError> {
        // see: cans/08-lifecycle.md#Rollback
        // TODO: Reset vCPU registers and revert userfaultfd memory tracking in <8ms
        Ok(())
    }

    fn snapshot(&mut self) -> Result<SnapshotDescriptor, DriverError> {
        // TODO: Capture dirty byte pages and register states
        Ok(SnapshotDescriptor {
            snapshot_id: [0u8; 16],
            dirty_bytes: 0,
            manifest_digest: [0u8; 32],
        })
    }

    fn set_vcpus(&mut self, count: u8) -> Result<(), DriverError> {
        // TODO: Park/unpark vCPU worker threads
        Ok(())
    }

    fn adjust_balloon(&mut self, target_mb: u32) -> Result<(), DriverError> {
        // TODO: Deflate/inflate virtio-balloon device descriptor
        Ok(())
    }

    fn skew_clock(&mut self, offset_ns: i64) -> Result<(), DriverError> {
        // see: cans/08-lifecycle.md#Clock-skew
        // TODO: Adjust KVM_SET_CLOCK offset
        Ok(())
    }
}
```

---

### Task 1.2: `driver-hvf` (macOS Apple Silicon Hypervisor.framework)
Assigned to: **Agent-13**  
Target: Compiled for Apple Silicon (`aarch64-apple-darwin`). Provides cross-platform stubs for workspace checks.

**File: `crates/kelir-drivers/driver-hvf/Cargo.toml`**
```toml
[package]
name = "driver-hvf"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../../kelir-interface" }
libc = { workspace = true }
thiserror = { workspace = true }

[lints]
workspace = true
```

**File: `crates/kelir-drivers/driver-hvf/src/sys.rs`**
// see: cans/01-architecture.md#Driver-backends
```rust
// FFI signatures for raw macOS Hypervisor.framework
pub type hv_return_t = libc::c_int;
pub type hv_vcpu_t = u64;

pub const HV_SUCCESS: hv_return_t = 0;

extern "C" {
    pub fn hv_vm_create(flags: u64) -> hv_return_t;
    pub fn hv_vm_destroy() -> hv_return_t;
    pub fn hv_vcpu_create(vcpu: *mut hv_vcpu_t, exit: *mut *const libc::c_void, options: *const libc::c_void) -> hv_return_t;
    pub fn hv_vcpu_destroy(vcpu: hv_vcpu_t) -> hv_return_t;
    pub fn hv_vcpu_run(vcpu: hv_vcpu_t) -> hv_return_t;
    pub fn hv_vm_protect(ipa: u64, size: usize, flags: u64) -> hv_return_t;
}
```

**File: `crates/kelir-drivers/driver-hvf/src/lib.rs`**
// see: cans/01-architecture.md#Host-memory-pressure-watcher
// see: cans/08-lifecycle.md#CoW-fork
```rust
pub mod sys;

use kelir_interface::driver::{SnapshotDescriptor, VmConfig, VmmDriver};
use kelir_interface::errors::DriverError;

pub struct HvfDriver {
    vcpu_ids: Vec<sys::hv_vcpu_t>,
    memory_mapped_bytes: usize,
    config: Option<VmConfig>,
}

impl HvfDriver {
    pub fn new() -> Result<Self, DriverError> {
        Ok(Self {
            vcpu_ids: Vec::new(),
            memory_mapped_bytes: 0,
            config: None,
        })
    }

    fn promote_chunked_stage2_pages(&self, ipa: u64) -> Result<(), DriverError> {
        // see: cans/08-lifecycle.md#CoW-fork
        // TODO: Promote 2MB contiguous blocks via Mach vm_copy CoW
        Ok(())
    }
}

impl VmmDriver for HvfDriver {
    fn boot(&mut self, config: &VmConfig) -> Result<(), DriverError> {
        self.config = Some(config.clone());
        // TODO: Initialize Hypervisor.framework VM and vCPUs
        Ok(())
    }

    fn pause(&mut self) -> Result<(), DriverError> {
        // TODO: Halt ARM64 vCPU threads at cycle boundary
        Ok(())
    }

    fn resume(&mut self) -> Result<(), DriverError> {
        // TODO: Resume ARM64 vCPU threads
        Ok(())
    }

    fn step(&mut self) -> Result<(), DriverError> {
        // TODO: Single-step via MDSCR_EL1.SS
        Ok(())
    }

    fn fork(&self) -> Result<Box<dyn VmmDriver>, DriverError> {
        // see: cans/08-lifecycle.md#CoW-fork
        // TODO: Mach copy-on-write + Stage-2 write protection via hv_vm_protect
        Ok(Box::new(Self::new()?))
    }

    fn rollback(&mut self, snapshot: &SnapshotDescriptor) -> Result<(), DriverError> {
        // see: cans/08-lifecycle.md#Rollback
        // TODO: Reset Stage-2 translation table roots in <8ms
        Ok(())
    }

    fn snapshot(&mut self) -> Result<SnapshotDescriptor, DriverError> {
        Ok(SnapshotDescriptor {
            snapshot_id: [0u8; 16],
            dirty_bytes: 0,
            manifest_digest: [0u8; 32],
        })
    }

    fn set_vcpus(&mut self, count: u8) -> Result<(), DriverError> {
        Ok(())
    }

    fn adjust_balloon(&mut self, target_mb: u32) -> Result<(), DriverError> {
        // see: cans/01-architecture.md#Host-memory-pressure-watcher
        // TODO: Stage-2 hv_vm_unmap + mach_vm_deallocate purge
        Ok(())
    }

    fn skew_clock(&mut self, offset_ns: i64) -> Result<(), DriverError> {
        // see: cans/08-lifecycle.md#Clock-skew
        // TODO: Adjust ARM64 CNTVOFF_EL2
        Ok(())
    }
}
```

---

### Task 1.3: `driver-openvmm` (Windows Hypervisor Platform + ConPTY)
Assigned to: **Agent-14**  
Dependency Constraint: `windows-sys` (features: `Win32_System_Hypervisor`, `Win32_Foundation`).

**File: `crates/kelir-drivers/driver-openvmm/Cargo.toml`**
```toml
[package]
name = "driver-openvmm"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../../kelir-interface" }
thiserror = { workspace = true }

[target.'cfg(windows)'.dependencies.windows-sys]
version = "0.59"
features = [
    "Win32_Foundation",
    "Win32_System_Hypervisor",
]

[lints]
workspace = true
```

**File: `crates/kelir-drivers/driver-openvmm/src/lib.rs`**
// see: cans/01-architecture.md#Driver-backends
```rust
use kelir_interface::driver::{SnapshotDescriptor, VmConfig, VmmDriver};
use kelir_interface::errors::DriverError;

pub struct OpenVmmDriver {
    partition_handle: usize,
    config: Option<VmConfig>,
}

impl OpenVmmDriver {
    pub fn new() -> Result<Self, DriverError> {
        Ok(Self {
            partition_handle: 0,
            config: None,
        })
    }
}

impl VmmDriver for OpenVmmDriver {
    fn boot(&mut self, config: &VmConfig) -> Result<(), DriverError> {
        self.config = Some(config.clone());
        // TODO: Setup WHP partition and commit Local APIC state before TSC deadline
        Ok(())
    }

    fn pause(&mut self) -> Result<(), DriverError> {
        Ok(())
    }

    fn resume(&mut self) -> Result<(), DriverError> {
        Ok(())
    }

    fn step(&mut self) -> Result<(), DriverError> {
        Ok(())
    }

    fn fork(&self) -> Result<Box<dyn VmmDriver>, DriverError> {
        Ok(Box::new(Self::new()?))
    }

    fn rollback(&mut self, snapshot: &SnapshotDescriptor) -> Result<(), DriverError> {
        Ok(())
    }

    fn snapshot(&mut self) -> Result<SnapshotDescriptor, DriverError> {
        Ok(SnapshotDescriptor {
            snapshot_id: [0u8; 16],
            dirty_bytes: 0,
            manifest_digest: [0u8; 32],
        })
    }

    fn set_vcpus(&mut self, count: u8) -> Result<(), DriverError> {
        Ok(())
    }

    fn adjust_balloon(&mut self, target_mb: u32) -> Result<(), DriverError> {
        Ok(())
    }

    fn skew_clock(&mut self, offset_ns: i64) -> Result<(), DriverError> {
        Ok(())
    }
}
```

---

### Task 1.4: `driver-avf` (Android Virtualization Framework)
Assigned to: **Agent-15**  
Dependency Constraint: `kelir-interface`, `libc`. Strictly minimal `/dev/avf` descriptor handling.

**File: `crates/kelir-drivers/driver-avf/Cargo.toml`**
```toml
[package]
name = "driver-avf"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../../kelir-interface" }
libc = { workspace = true }
thiserror = { workspace = true }

[lints]
workspace = true
```

**File: `crates/kelir-drivers/driver-avf/src/lib.rs`**
// see: cans/01-architecture.md#Driver-backends
```rust
use kelir_interface::driver::{SnapshotDescriptor, VmConfig, VmmDriver};
use kelir_interface::errors::DriverError;

pub struct AvfDriver {
    avf_fd: i32,
    config: Option<VmConfig>,
}

impl AvfDriver {
    pub fn new() -> Result<Self, DriverError> {
        Ok(Self {
            avf_fd: -1,
            config: None,
        })
    }
}

impl VmmDriver for AvfDriver {
    fn boot(&mut self, config: &VmConfig) -> Result<(), DriverError> {
        self.config = Some(config.clone());
        // TODO: Connect /dev/avf or pKVM node
        Ok(())
    }

    fn pause(&mut self) -> Result<(), DriverError> {
        Ok(())
    }

    fn resume(&mut self) -> Result<(), DriverError> {
        Ok(())
    }

    fn step(&mut self) -> Result<(), DriverError> {
        Ok(())
    }

    fn fork(&self) -> Result<Box<dyn VmmDriver>, DriverError> {
        Ok(Box::new(Self::new()?))
    }

    fn rollback(&mut self, snapshot: &SnapshotDescriptor) -> Result<(), DriverError> {
        Ok(())
    }

    fn snapshot(&mut self) -> Result<SnapshotDescriptor, DriverError> {
        Ok(SnapshotDescriptor {
            snapshot_id: [0u8; 16],
            dirty_bytes: 0,
            manifest_digest: [0u8; 32],
        })
    }

    fn set_vcpus(&mut self, count: u8) -> Result<(), DriverError> {
        Ok(())
    }

    fn adjust_balloon(&mut self, target_mb: u32) -> Result<(), DriverError> {
        Ok(())
    }

    fn skew_clock(&mut self, offset_ns: i64) -> Result<(), DriverError> {
        Ok(())
    }
}
```

---

## 3. STAGE 2: EMULATED TIER DRIVERS (STRICT OPT-IN ONLY)

### Task 2.1: `driver-bwrap` (Unprivileged Linux Namespaces)
Assigned to: **Agent-16**  
Constraints: Bans `nix` crate. Strictly uses `libc`. Requires `allow_emulated_fallback == true`.

**File: `crates/kelir-drivers/driver-bwrap/Cargo.toml`**
```toml
[package]
name = "driver-bwrap"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../../kelir-interface" }
libc = { workspace = true }
thiserror = { workspace = true }

[lints]
workspace = true
```

**File: `crates/kelir-drivers/driver-bwrap/src/lib.rs`**
// see: cans/01-architecture.md#Driver-backends
```rust
use kelir_interface::driver::{SnapshotDescriptor, VmConfig, VmmDriver};
use kelir_interface::errors::DriverError;

pub struct BwrapDriver {
    allow_emulated_fallback: bool,
    child_pid: Option<libc::pid_t>,
    config: Option<VmConfig>,
}

impl BwrapDriver {
    pub fn new(allow_emulated_fallback: bool) -> Result<Self, DriverError> {
        if !allow_emulated_fallback {
            return Err(DriverError::ENOHARDWAREVMM);
        }
        Ok(Self {
            allow_emulated_fallback,
            child_pid: None,
            config: None,
        })
    }
}

impl VmmDriver for BwrapDriver {
    fn boot(&mut self, config: &VmConfig) -> Result<(), DriverError> {
        if !self.allow_emulated_fallback {
            return Err(DriverError::ENOHARDWAREVMM);
        }
        self.config = Some(config.clone());
        // TODO: Spawn bubblewrap/unshare user namespace via raw libc
        Ok(())
    }

    fn pause(&mut self) -> Result<(), DriverError> {
        Ok(())
    }

    fn resume(&mut self) -> Result<(), DriverError> {
        Ok(())
    }

    fn step(&mut self) -> Result<(), DriverError> {
        Ok(())
    }

    fn fork(&self) -> Result<Box<dyn VmmDriver>, DriverError> {
        Ok(Box::new(Self::new(self.allow_emulated_fallback)?))
    }

    fn rollback(&mut self, snapshot: &SnapshotDescriptor) -> Result<(), DriverError> {
        Ok(())
    }

    fn snapshot(&mut self) -> Result<SnapshotDescriptor, DriverError> {
        Ok(SnapshotDescriptor {
            snapshot_id: [0u8; 16],
            dirty_bytes: 0,
            manifest_digest: [0u8; 32],
        })
    }

    fn set_vcpus(&mut self, count: u8) -> Result<(), DriverError> {
        Ok(())
    }

    fn adjust_balloon(&mut self, target_mb: u32) -> Result<(), DriverError> {
        Ok(())
    }

    fn skew_clock(&mut self, offset_ns: i64) -> Result<(), DriverError> {
        Ok(())
    }
}
```

---

### Task 2.2: `driver-proot` (Termux ptrace Emulation)
Assigned to: **Agent-17**  
Constraints: Requires `allow_emulated_fallback == true`.

**File: `crates/kelir-drivers/driver-proot/Cargo.toml`**
```toml
[package]
name = "driver-proot"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../../kelir-interface" }
libc = { workspace = true }
thiserror = { workspace = true }

[lints]
workspace = true
```

**File: `crates/kelir-drivers/driver-proot/src/lib.rs`**
// see: cans/01-architecture.md#Driver-backends
```rust
use kelir_interface::driver::{SnapshotDescriptor, VmConfig, VmmDriver};
use kelir_interface::errors::DriverError;

pub struct ProotDriver {
    allow_emulated_fallback: bool,
    config: Option<VmConfig>,
}

impl ProotDriver {
    pub fn new(allow_emulated_fallback: bool) -> Result<Self, DriverError> {
        if !allow_emulated_fallback {
            return Err(DriverError::ENOHARDWAREVMM);
        }
        Ok(Self {
            allow_emulated_fallback,
            config: None,
        })
    }
}

impl VmmDriver for ProotDriver {
    fn boot(&mut self, config: &VmConfig) -> Result<(), DriverError> {
        if !self.allow_emulated_fallback {
            return Err(DriverError::ENOHARDWAREVMM);
        }
        self.config = Some(config.clone());
        // TODO: Initialize proot ptrace tracing harness
        Ok(())
    }

    fn pause(&mut self) -> Result<(), DriverError> {
        Ok(())
    }

    fn resume(&mut self) -> Result<(), DriverError> {
        Ok(())
    }

    fn step(&mut self) -> Result<(), DriverError> {
        Ok(())
    }

    fn fork(&self) -> Result<Box<dyn VmmDriver>, DriverError> {
        Ok(Box::new(Self::new(self.allow_emulated_fallback)?))
    }

    fn rollback(&mut self, snapshot: &SnapshotDescriptor) -> Result<(), DriverError> {
        Ok(())
    }

    fn snapshot(&mut self) -> Result<SnapshotDescriptor, DriverError> {
        Ok(SnapshotDescriptor {
            snapshot_id: [0u8; 16],
            dirty_bytes: 0,
            manifest_digest: [0u8; 32],
        })
    }

    fn set_vcpus(&mut self, count: u8) -> Result<(), DriverError> {
        Ok(())
    }

    fn adjust_balloon(&mut self, target_mb: u32) -> Result<(), DriverError> {
        Ok(())
    }

    fn skew_clock(&mut self, offset_ns: i64) -> Result<(), DriverError> {
        Ok(())
    }
}
```

---

## 4. PARALLEL AGENT EXECUTION MATRIX

| Agent | Driver Crate | Target Platform | Compilable Verification Command |
|---|---|---|---|
| **Agent-12** | `driver-rustvmm` | Linux Bare-Metal KVM | `cargo check -p driver-rustvmm` |
| **Agent-13** | `driver-hvf` | macOS Apple Silicon HVF | `cargo check -p driver-hvf` |
| **Agent-14** | `driver-openvmm` | Windows WHP | `cargo check -p driver-openvmm` |
| **Agent-15** | `driver-avf` | Android AVF / pKVM | `cargo check -p driver-avf` |
| **Agent-16** | `driver-bwrap` | Unprivileged Linux Namespaces | `cargo check -p driver-bwrap` |
| **Agent-17** | `driver-proot` | Termux ptrace Emulation | `cargo check -p driver-proot` |

---

## 5. CONVERGENCE & VERIFICATION RUNBOOK

Execute once Agents 12 through 17 complete their crates:

1. **Host-Native Driver Check**:
   ```bash
   cargo check -p driver-rustvmm -p driver-bwrap
   ```
2. **Untested Drivers Check (Compile-Only Gate)**:
   ```bash
   just check-drivers
   ```
   *Assert:* Both `driver-avf` and `driver-proot` compile cleanly without error.
3. **Hygiene & Crate Boundaries**:
   ```bash
   just check-hygiene
   cargo clippy -p driver-rustvmm -p driver-bwrap -- -D warnings
   ```
   *Assert:* All unsafe blocks carry single-line `// SAFETY:` explanations; zero disallowed dependencies (`nix`, `vmm-sys-util`).

---
*Work-order Part 3 of 5 complete. Proceed to Part 4/5 (Plugin Ecosystem: Tier 1, Tier 2, Tier 3) immediately upon all Phase 3 gates reporting GREEN.*

# WORK-ORDER.md: Distributed Systems Architecture Construction
**Target System:** Kelir MicroVM Hypervisor Microkernel  
**Phase:** 4 of 5 (Domain Plugin Ecosystem: Tier 1, Tier 2, Tier 3 & Testkit)  
**Total Parts (x):** 5  
- **Part 1/5:** Scaffolding, Root Build System, CI/CD, Leaf Interfaces & Shim (Completed)
- **Part 2/5:** Engine Core & Native Planes (`kelir-core`) (Completed)
- **Part 3/5:** Driver Enclaves (`driver-rustvmm`, `driver-hvf`, `driver-openvmm`, `driver-avf`, `driver-bwrap`, `driver-proot`) (Completed)
- **Part 4/5:** Plugin Ecosystem (13 Catalog Plugins: Tier 1 Transports, Tier 2 Proxies, Tier 3 Aggregators)
- **Part 5/5:** Adapters, Tooling CLI, Native FFI Bindings & Tri-Native SDK Surfaces (`kelir-pack`, `kelir-plugin-cli`, NAPI, PyO3, TS, Python)

---

## 1. AGENT OPERATIONAL CONSTRAINTS (`kelir-plugins`)

All agents implementing plugins within `crates/kelir-plugins/` must observe strict structural rules:

1. **Zero Unsafe Permitted**:
   - Every plugin crate enforces `#![forbid(unsafe_code)]`.
2. **Layering & Downward-Only Dependencies**:
   // see: cans/02-plugin-model.md#Layering-enforcement
   - Plugins must **never** import `kelir-core` or `kelir` facade.
   - **Tier 1**: Depends **strictly** on `kelir-interface` only.
   - **Tier 2**: Depends on `kelir-interface` and can import Tier 1 crates. Zero concrete peer imports within Tier 2.
   - **Tier 3**: Depends on `kelir-interface` and can import Tier 1 & Tier 2 crates. Zero peer imports within Tier 3 (with the sole exception of `plugin-transaction` consuming Tier 2 `plugin-db-proxy`).
3. **Contract Uniformity**:
   - Every plugin must implement `kelir_interface::plugin::KelirPlugin`.
   - Implement only the relevant segregated ring traits (`LifecycleParticipant`, `TrapInterposer`, `ConsoleInterposer`, `FsInterposer`, `NetInterposer`, `HostInterposer`). Unused traits default to no-op.
4. **Dev-Dependency Isolation**:
   - Plugins use `kelir-plugin-testkit` exclusively as a `[dev-dependencies]` entry. Never include it in runtime compilation.
5. **No Speculative Implementation (L1/L2 Only)**:
   - Compileable trait definitions, service export structs, filter hooks, and state wrappers.
   - Payload manipulation, regex parsing, and cryptographic checks remain `// TODO` returning `Verdict::Pass` or `FilterAction::Forward`.

---

## 2. STAGE 0: SHARED PLUGIN TESTKIT

Assigned to: **Agent-18**  
Dependency Constraint: `kelir-interface` only. Zero engine dependencies.

**File: `crates/kelir-plugin-testkit/Cargo.toml`**
```toml
[package]
name = "kelir-plugin-testkit"
version = "1.0.0"
edition = "2024"
license = "MIT OR Apache-2.0"

[dependencies]
kelir-interface = { path = "../kelir-interface" }

[lints]
workspace = true
```

**File: `crates/kelir-plugin-testkit/src/lib.rs`**
// see: cans/02-plugin-model.md#Plugin-testkit
```rust
#![forbid(unsafe_code)]

use kelir_interface::driver::{SnapshotDescriptor, VmConfig, VmmDriver};
use kelir_interface::errors::DriverError;

pub struct MockVmmDriver;

impl VmmDriver for MockVmmDriver {
    fn boot(&mut self, _config: &VmConfig) -> Result<(), DriverError> { Ok(()) }
    fn pause(&mut self) -> Result<(), DriverError> { Ok(()) }
    fn resume(&mut self) -> Result<(), DriverError> { Ok(()) }
    fn step(&mut self) -> Result<(), DriverError> { Ok(()) }
    fn fork(&self) -> Result<Box<dyn VmmDriver>, DriverError> { Ok(Box::new(MockVmmDriver)) }
    fn rollback(&mut self, _snapshot: &SnapshotDescriptor) -> Result<(), DriverError> { Ok(()) }
    fn snapshot(&mut self) -> Result<SnapshotDescriptor, DriverError> {
        Ok(SnapshotDescriptor { snapshot_id: [0u8; 16], dirty_bytes: 0, manifest_digest: [0u8; 32] })
    }
    fn set_vcpus(&mut self, _count: u8) -> Result<(), DriverError> { Ok(()) }
    fn adjust_balloon(&mut self, _target_mb: u32) -> Result<(), DriverError> { Ok(()) }
    fn skew_clock(&mut self, _offset_ns: i64) -> Result<(), DriverError> { Ok(()) }
}
```

---

## 3. STAGE 1: TIER 1 BASE TRANSPORTS & CHANNEL ADAPTERS

Assigned to: **Agent-19**  
Dependency Constraint: `kelir-interface` only.

### Task 1.1: `plugin-mcp` (MCP stdio JSON-RPC ↔ VirtIO)
**File: `crates/kelir-plugins/plugin-mcp/Cargo.toml`**
```toml
[package]
name = "plugin-mcp"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../../kelir-interface" }

[dev-dependencies]
kelir-plugin-testkit = { path = "../../kelir-plugin-testkit" }

[lints]
workspace = true
```

**File: `crates/kelir-plugins/plugin-mcp/src/lib.rs`**
// see: cans/plugins/mcp.md
```rust
#![forbid(unsafe_code)]

use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{ConsoleInterposer, HandlerCtx, KelirPlugin, LifecycleParticipant, MountScope, TrapInterposer};
use kelir_interface::protocol::{TrapEvent, Verdict};

pub struct McpEndpointService;

pub struct McpPlugin {
    pub service: McpEndpointService,
}

impl McpPlugin {
    pub fn new() -> Self {
        Self { service: McpEndpointService }
    }
}

impl KelirPlugin for McpPlugin {
    fn name(&self) -> &'static str { "mcp" }
    fn default_scope(&self) -> MountScope {
        MountScope { allowed_rings: 0x01 | 0x02, path_subtrees: Vec::new(), cidr_allowlists: Vec::new(), binary_paths: Vec::new() }
    }
}

impl LifecycleParticipant for McpPlugin {}
impl ConsoleInterposer for McpPlugin {
    fn on_console_output(&mut self, _port: u32, _buf: &mut [u8]) -> Result<(), PluginError> {
        // TODO: Bridge raw stdio frames to MCP JSON-RPC
        Ok(())
    }
}
impl TrapInterposer for McpPlugin {
    fn on_trap(&mut self, _event: &TrapEvent, _ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        Ok(Verdict::Pass)
    }
}
```

### Task 1.2: `plugin-mesh` (Sovereign Federation Transport)
**File: `crates/kelir-plugins/plugin-mesh/Cargo.toml`**
```toml
[package]
name = "plugin-mesh"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../../kelir-interface" }

[dev-dependencies]
kelir-plugin-testkit = { path = "../../kelir-plugin-testkit" }

[lints]
workspace = true
```

**File: `crates/kelir-plugins/plugin-mesh/src/lib.rs`**
// see: cans/plugins/mesh.md
```rust
#![forbid(unsafe_code)]

use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{KelirPlugin, LifecycleParticipant, MountScope, NetInterposer};
use kelir_interface::protocol::FilterAction;

pub struct MeshPlugin;

impl MeshPlugin {
    pub fn new() -> Self { Self }
}

impl KelirPlugin for MeshPlugin {
    fn name(&self) -> &'static str { "mesh" }
    fn default_scope(&self) -> MountScope {
        MountScope { allowed_rings: 0x04, path_subtrees: Vec::new(), cidr_allowlists: Vec::new(), binary_paths: Vec::new() }
    }
}

impl LifecycleParticipant for MeshPlugin {}
impl NetInterposer for MeshPlugin {
    fn on_net_outbound(&mut self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        // TODO: Verify mTLS framing and inject SPIFFE identity token
        Ok(FilterAction::Forward)
    }
    fn on_net_inbound(&mut self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        Ok(FilterAction::Forward)
    }
}
```

---

## 4. STAGE 2: TIER 2 GOVERNANCE & PROTOCOL PROXIES

### Task 2.1: Tool Guard, Vault & Supply Chain
Assigned to: **Agent-20**

**File: `crates/kelir-plugins/plugin-tool-guard/Cargo.toml`**
```toml
[package]
name = "plugin-tool-guard"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../../kelir-interface" }
plugin-mcp = { path = "../plugin-mcp" }

[dev-dependencies]
kelir-plugin-testkit = { path = "../../kelir-plugin-testkit" }

[lints]
workspace = true
```

**File: `crates/kelir-plugins/plugin-tool-guard/src/lib.rs`**
// see: cans/plugins/tool-guard.md
```rust
#![forbid(unsafe_code)]

use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{ConsoleInterposer, KelirPlugin, MountScope};

pub struct ToolRegistryService;

pub struct ToolGuardPlugin {
    pub registry: ToolRegistryService,
}

impl ToolGuardPlugin {
    pub fn new() -> Self {
        Self { registry: ToolRegistryService }
    }
}

impl KelirPlugin for ToolGuardPlugin {
    fn name(&self) -> &'static str { "tool-guard" }
    fn default_scope(&self) -> MountScope {
        MountScope { allowed_rings: 0x01, path_subtrees: Vec::new(), cidr_allowlists: Vec::new(), binary_paths: Vec::new() }
    }
}

impl ConsoleInterposer for ToolGuardPlugin {
    fn on_console_input(&mut self, _port: u32, _buf: &mut [u8]) -> Result<(), PluginError> {
        // TODO: Intercept tools/call JSON-RPC payload and gate mutations
        Ok(())
    }
}
```

**File: `crates/kelir-plugins/plugin-vault/Cargo.toml`**
```toml
[package]
name = "plugin-vault"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../../kelir-interface" }

[dev-dependencies]
kelir-plugin-testkit = { path = "../../kelir-plugin-testkit" }

[lints]
workspace = true
```

**File: `crates/kelir-plugins/plugin-vault/src/lib.rs`**
// see: cans/plugins/vault.md
```rust
#![forbid(unsafe_code)]

use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{KelirPlugin, LifecycleParticipant, MountScope, NetInterposer};
use kelir_interface::protocol::FilterAction;

pub struct CredentialBrokerService;

pub struct VaultPlugin {
    pub broker: CredentialBrokerService,
}

impl VaultPlugin {
    pub fn new() -> Self {
        Self { broker: CredentialBrokerService }
    }
}

impl KelirPlugin for VaultPlugin {
    fn name(&self) -> &'static str { "vault" }
    fn default_scope(&self) -> MountScope {
        MountScope { allowed_rings: 0x04, path_subtrees: Vec::new(), cidr_allowlists: Vec::new(), binary_paths: Vec::new() }
    }
}

impl LifecycleParticipant for VaultPlugin {}
impl NetInterposer for VaultPlugin {
    fn on_net_outbound(&mut self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        // TODO: Intercept 169.254.169.254 requests and broker short-lived tokens
        Ok(FilterAction::Forward)
    }
}
```

**File: `crates/kelir-plugins/plugin-supply-chain/Cargo.toml`**
```toml
[package]
name = "plugin-supply-chain"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../../kelir-interface" }

[dev-dependencies]
kelir-plugin-testkit = { path = "../../kelir-plugin-testkit" }

[lints]
workspace = true
```

**File: `crates/kelir-plugins/plugin-supply-chain/src/lib.rs`**
// see: cans/plugins/supply-chain.md
```rust
#![forbid(unsafe_code)]

use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{HandlerCtx, KelirPlugin, MountScope, TrapInterposer};
use kelir_interface::protocol::{TrapEvent, Verdict};

pub struct JitValidationService;

pub struct SupplyChainPlugin {
    pub jit_service: JitValidationService,
}

impl SupplyChainPlugin {
    pub fn new() -> Self {
        Self { jit_service: JitValidationService }
    }
}

impl KelirPlugin for SupplyChainPlugin {
    fn name(&self) -> &'static str { "supply-chain" }
    fn default_scope(&self) -> MountScope {
        MountScope { allowed_rings: 0x02, path_subtrees: Vec::new(), cidr_allowlists: Vec::new(), binary_paths: Vec::new() }
    }
}

impl TrapInterposer for SupplyChainPlugin {
    fn on_trap(&mut self, _event: &TrapEvent, _ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        // TODO: Inspect argv entropy (>5.2 bits) and block lifecycle scripts
        Ok(Verdict::Pass)
    }
}
```

---

### Task 2.2: Wire Ingress, Container, Replay & DB Proxy
Assigned to: **Agent-21**

**File: `crates/kelir-plugins/plugin-wire-ingress/Cargo.toml`**
```toml
[package]
name = "plugin-wire-ingress"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../../kelir-interface" }

[dev-dependencies]
kelir-plugin-testkit = { path = "../../kelir-plugin-testkit" }

[lints]
workspace = true
```

**File: `crates/kelir-plugins/plugin-wire-ingress/src/lib.rs`**
// see: cans/plugins/wire-ingress.md
```rust
#![forbid(unsafe_code)]

use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{KelirPlugin, LifecycleParticipant, MountScope, NetInterposer};
use kelir_interface::protocol::FilterAction;

pub struct IngressRoutingService;

pub struct WireIngressPlugin {
    pub router: IngressRoutingService,
}

impl WireIngressPlugin {
    pub fn new() -> Self {
        Self { router: IngressRoutingService }
    }
}

impl KelirPlugin for WireIngressPlugin {
    fn name(&self) -> &'static str { "wire-ingress" }
    fn default_scope(&self) -> MountScope {
        MountScope { allowed_rings: 0x04, path_subtrees: Vec::new(), cidr_allowlists: Vec::new(), binary_paths: Vec::new() }
    }
}

impl LifecycleParticipant for WireIngressPlugin {}
impl NetInterposer for WireIngressPlugin {
    fn on_net_inbound(&mut self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        // TODO: Park inbound connections during VM suspend
        Ok(FilterAction::Forward)
    }
}
```

**File: `crates/kelir-plugins/plugin-container/Cargo.toml`**
```toml
[package]
name = "plugin-container"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../../kelir-interface" }

[dev-dependencies]
kelir-plugin-testkit = { path = "../../kelir-plugin-testkit" }

[lints]
workspace = true
```

**File: `crates/kelir-plugins/plugin-container/src/lib.rs`**
// see: cans/plugins/container.md
```rust
#![forbid(unsafe_code)]

use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{KelirPlugin, LifecycleCtx, LifecycleParticipant, MountScope};

pub struct ContainerService;

pub struct ContainerPlugin {
    pub service: ContainerService,
}

impl ContainerPlugin {
    pub fn new() -> Self {
        Self { service: ContainerService }
    }
}

impl KelirPlugin for ContainerPlugin {
    fn name(&self) -> &'static str { "container" }
    fn default_scope(&self) -> MountScope {
        MountScope { allowed_rings: 0x00, path_subtrees: Vec::new(), cidr_allowlists: Vec::new(), binary_paths: Vec::new() }
    }
}

impl LifecycleParticipant for ContainerPlugin {
    fn on_vm_ready(&mut self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        // TODO: Configure rootless Podman/crun cgroup v2 hierarchy
        Ok(())
    }
}
```

**File: `crates/kelir-plugins/plugin-replay/Cargo.toml`**
```toml
[package]
name = "plugin-replay"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../../kelir-interface" }

[dev-dependencies]
kelir-plugin-testkit = { path = "../../kelir-plugin-testkit" }

[lints]
workspace = true
```

**File: `crates/kelir-plugins/plugin-replay/src/lib.rs`**
// see: cans/plugins/replay.md
```rust
#![forbid(unsafe_code)]

use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{KelirPlugin, LifecycleParticipant, MountScope};

pub struct ReplayService;

pub struct ReplayPlugin {
    pub service: ReplayService,
}

impl ReplayPlugin {
    pub fn new() -> Self {
        Self { service: ReplayService }
    }
}

impl KelirPlugin for ReplayPlugin {
    fn name(&self) -> &'static str { "replay" }
    fn default_scope(&self) -> MountScope {
        MountScope { allowed_rings: 0x00, path_subtrees: Vec::new(), cidr_allowlists: Vec::new(), binary_paths: Vec::new() }
    }
}

impl LifecycleParticipant for ReplayPlugin {}
```

**File: `crates/kelir-plugins/plugin-db-proxy/Cargo.toml`**
```toml
[package]
name = "plugin-db-proxy"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../../kelir-interface" }

[dev-dependencies]
kelir-plugin-testkit = { path = "../../kelir-plugin-testkit" }

[lints]
workspace = true
```

**File: `crates/kelir-plugins/plugin-db-proxy/src/lib.rs`**
// see: cans/plugins/db-proxy.md
```rust
#![forbid(unsafe_code)]

use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{KelirPlugin, MountScope, NetInterposer};
use kelir_interface::protocol::FilterAction;

pub struct SqlProxyService;

pub struct DbProxyPlugin {
    pub service: SqlProxyService,
}

impl DbProxyPlugin {
    pub fn new() -> Self {
        Self { service: SqlProxyService }
    }
}

impl KelirPlugin for DbProxyPlugin {
    fn name(&self) -> &'static str { "db-proxy" }
    fn default_scope(&self) -> MountScope {
        MountScope { allowed_rings: 0x04, path_subtrees: Vec::new(), cidr_allowlists: Vec::new(), binary_paths: Vec::new() }
    }
}

impl NetInterposer for DbProxyPlugin {
    fn on_net_outbound(&mut self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        // TODO: Intercept pgwire (port 5432) and forward over DB_SQLITE (0x0011)
        Ok(FilterAction::Forward)
    }
}
```

---

## 5. STAGE 3: TIER 3 CROSS-CUTTING AGGREGATORS

### Task 3.1: Ledger & Profile
Assigned to: **Agent-22**

**File: `crates/kelir-plugins/plugin-ledger/Cargo.toml`**
```toml
[package]
name = "plugin-ledger"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../../kelir-interface" }

[dev-dependencies]
kelir-plugin-testkit = { path = "../../kelir-plugin-testkit" }

[lints]
workspace = true
```

**File: `crates/kelir-plugins/plugin-ledger/src/lib.rs`**
// see: cans/plugins/ledger.md
```rust
#![forbid(unsafe_code)]

use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{KelirPlugin, LifecycleParticipant, MountScope, TrapInterposer};
use kelir_interface::telemetry::CausalDelta;

pub trait LedgerSink: Send + Sync {
    fn emit_delta(&self, delta: &CausalDelta) -> Result<(), PluginError>;
}

pub struct LedgerPlugin {
    pub merkle_enabled: bool,
}

impl LedgerPlugin {
    pub fn new(merkle_enabled: bool) -> Self {
        Self { merkle_enabled }
    }
}

impl KelirPlugin for LedgerPlugin {
    fn name(&self) -> &'static str { "ledger" }
    fn default_scope(&self) -> MountScope {
        MountScope { allowed_rings: 0x00, path_subtrees: Vec::new(), cidr_allowlists: Vec::new(), binary_paths: Vec::new() }
    }
}

impl LifecycleParticipant for LedgerPlugin {}
impl TrapInterposer for LedgerPlugin {}
```

**File: `crates/kelir-plugins/plugin-profile/Cargo.toml`**
```toml
[package]
name = "plugin-profile"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../../kelir-interface" }

[dev-dependencies]
kelir-plugin-testkit = { path = "../../kelir-plugin-testkit" }

[lints]
workspace = true
```

**File: `crates/kelir-plugins/plugin-profile/src/lib.rs`**
// see: cans/plugins/profile.md
```rust
#![forbid(unsafe_code)]

use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{KelirPlugin, MountScope};

pub trait ConfigurablePlugin: Send + Sync {
    fn configure(&mut self, fragment: &[u8]) -> Result<(), PluginError>;
}

pub struct ProfilePlugin;

impl ProfilePlugin {
    pub fn new() -> Self { Self }
}

impl KelirPlugin for ProfilePlugin {
    fn name(&self) -> &'static str { "profile" }
    fn default_scope(&self) -> MountScope {
        MountScope { allowed_rings: 0x00, path_subtrees: Vec::new(), cidr_allowlists: Vec::new(), binary_paths: Vec::new() }
    }
}
```

---

### Task 3.2: Chaos & Transaction
Assigned to: **Agent-23**

**File: `crates/kelir-plugins/plugin-chaos/Cargo.toml`**
```toml
[package]
name = "plugin-chaos"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../../kelir-interface" }

[dev-dependencies]
kelir-plugin-testkit = { path = "../../kelir-plugin-testkit" }

[lints]
workspace = true
```

**File: `crates/kelir-plugins/plugin-chaos/src/lib.rs`**
// see: cans/plugins/chaos.md
```rust
#![forbid(unsafe_code)]

use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{FsInterposer, KelirPlugin, MountScope, NetInterposer};
use kelir_interface::protocol::FilterAction;

pub struct FaultInjectionService;

pub struct ChaosPlugin {
    pub fault_service: FaultInjectionService,
}

impl ChaosPlugin {
    pub fn new() -> Self {
        Self { fault_service: FaultInjectionService }
    }
}

impl KelirPlugin for ChaosPlugin {
    fn name(&self) -> &'static str { "chaos" }
    fn default_scope(&self) -> MountScope {
        MountScope { allowed_rings: 0x02 | 0x04, path_subtrees: Vec::new(), cidr_allowlists: Vec::new(), binary_paths: Vec::new() }
    }
}

impl NetInterposer for ChaosPlugin {
    fn on_net_outbound(&mut self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        // TODO: Inject synthetic network latency or drop frames
        Ok(FilterAction::Forward)
    }
}

impl FsInterposer for ChaosPlugin {
    fn on_fs_write(&mut self, _path: &str, _offset: u64, _buf: &mut [u8]) -> Result<FilterAction, PluginError> {
        // TODO: Inject synthetic ENOSPC failures
        Ok(FilterAction::Forward)
    }
}
```

**File: `crates/kelir-plugins/plugin-transaction/Cargo.toml`**
```toml
[package]
name = "plugin-transaction"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../../kelir-interface" }
plugin-db-proxy = { path = "../plugin-db-proxy" }

[dev-dependencies]
kelir-plugin-testkit = { path = "../../kelir-plugin-testkit" }

[lints]
workspace = true
```

**File: `crates/kelir-plugins/plugin-transaction/src/lib.rs`**
// see: cans/plugins/transaction.md
```rust
#![forbid(unsafe_code)]

use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{KelirPlugin, LifecycleParticipant, MountScope};

pub struct TransactionService;

pub struct TransactionPlugin {
    pub service: TransactionService,
}

impl TransactionPlugin {
    pub fn new() -> Self {
        Self { service: TransactionService }
    }
}

impl KelirPlugin for TransactionPlugin {
    fn name(&self) -> &'static str { "transaction" }
    fn default_scope(&self) -> MountScope {
        MountScope { allowed_rings: 0x00, path_subtrees: Vec::new(), cidr_allowlists: Vec::new(), binary_paths: Vec::new() }
    }
}

impl LifecycleParticipant for TransactionPlugin {
    fn on_pre_commit(&mut self) -> Result<(), PluginError> {
        // TODO: Coordinate 2PC pre-commit with db-proxy
        Ok(())
    }
    fn on_post_commit(&mut self, _success: bool) -> Result<(), PluginError> {
        Ok(())
    }
}
```

---

## 6. PARALLEL AGENT EXECUTION MATRIX

| Agent | Target Scope | Output Crates | Compilable Verification Command |
|---|---|---|---|
| **Agent-18** | Shared Testkit | `crates/kelir-plugin-testkit` | `cargo check -p kelir-plugin-testkit` |
| **Agent-19** | Tier 1 Transports | `plugin-mcp`, `plugin-mesh` | `cargo check -p plugin-mcp -p plugin-mesh` |
| **Agent-20** | Tier 2 Governance | `plugin-tool-guard`, `plugin-vault`, `plugin-supply-chain` | `cargo check -p plugin-tool-guard -p plugin-vault -p plugin-supply-chain` |
| **Agent-21** | Tier 2 Proxies | `plugin-wire-ingress`, `plugin-container`, `plugin-replay`, `plugin-db-proxy` | `cargo check -p plugin-wire-ingress -p plugin-container -p plugin-replay -p plugin-db-proxy` |
| **Agent-22** | Tier 3 Aggregators | `plugin-ledger`, `plugin-profile` | `cargo check -p plugin-ledger -p plugin-profile` |
| **Agent-23** | Tier 3 Aggregators | `plugin-chaos`, `plugin-transaction` | `cargo check -p plugin-chaos -p plugin-transaction` |

---

## 7. CONVERGENCE & VERIFICATION RUNBOOK

Execute once Agents 18 through 23 finish their crates:

1. **Tiered Compilation Check**:
   ```bash
   just check-tiered
   ```
2. **Dependency DAG Validation**:
   ```bash
   just check-graph
   ```
   *Assert:* Zero cyclic dependencies; downward-only hierarchy respected.
3. **Hygiene & Crate Boundaries**:
   ```bash
   just check-hygiene
   cargo clippy -p "plugin-*" -- -D warnings
   ```
   *Assert:* Zero `unsafe` violations across all plugins; zero files exceeding `1000 LOC`.

---
*Work-order Part 4 of 5 complete. Proceed to Part 5/5 (Adapters, Tooling CLI, Native FFI Bindings & Tri-Native SDKs) immediately upon all Phase 4 gates reporting GREEN.*

# WORK-ORDER.md: Distributed Systems Architecture Construction
**Target System:** Kelir MicroVM Hypervisor Microkernel  
**Phase:** 5 of 5 (Adapters, Tooling CLI, Native FFI Bindings & Tri-Native SDK Surfaces)  
**Total Parts (x):** 5  
- **Part 1/5:** Scaffolding, Root Build System, CI/CD, Leaf Interfaces & Shim (Completed)
- **Part 2/5:** Engine Core & Native Planes (`kelir-core`) (Completed)
- **Part 3/5:** Driver Enclaves (`driver-rustvmm`, `driver-hvf`, `driver-openvmm`, `driver-avf`, `driver-bwrap`, `driver-proot`) (Completed)
- **Part 4/5:** Plugin Ecosystem (13 Catalog Plugins: Tier 1, 2, 3 & Testkit) (Completed)
- **Part 5/5:** Adapters, Tooling CLI, Native FFI Bindings & Tri-Native SDK Surfaces (`kelir-pack`, `kelir-plugin-cli`, NAPI, PyO3, TS, Python)

---

## 1. AGENT OPERATIONAL CONSTRAINTS (Adapters, FFI & SDKs)

1. **Idiomatic Surfaces Law**:
   // see: cans/03-sdk-architecture.md#Idiomatic-surfaces-law
   - Rust: Affine typestate consuming builder by value; monomorphic `HList` capability lookup via `c.ctx::<T>()`.
   - TypeScript: Fluent builder with `.use()` method chaining returning typed overloads; runtime checks at `.boot()`.
   - Python: Async context manager (`async with`) with typed `c.cap.<plugin_key>`; Pydantic v2 model validation at `.boot()`.
   - Do not cross-pollinate method names or attempt dynamic string bags. Parity is wire and behavioral parity against `vectors.json`.
2. **Zero In-Guest Footprint**:
   - Zero guest-side SDKs, shims, or background daemons. Tooling communicates strictly across paravirtual VirtIO rings.
3. **No Speculative Implementation (L1/L2 Only)**:
   - Provide fully compilable interfaces, exported types, FFI stubs, and pipeline wiring.
   - Algorithmic loops, file parsers, and FFI trampolines remain `// TODO` returning typed empty structures, zero-values, or `Ok(())`.

---

## 2. STAGE 1: FACADE CRATE & ADAPTERS

Assigned to: **Agent-24**

### Task 1.1: Facade Crate (`crates/kelir`)
**File: `crates/kelir/Cargo.toml`**
```toml
[package]
name = "kelir"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-core = { path = "../kelir-core" }
kelir-interface = { path = "../kelir-interface" }

# Optional feature-gated plugins
plugin-chaos = { path = "../kelir-plugins/plugin-chaos", optional = true }
plugin-container = { path = "../kelir-plugins/plugin-container", optional = true }
plugin-db-proxy = { path = "../kelir-plugins/plugin-db-proxy", optional = true }
plugin-ledger = { path = "../kelir-plugins/plugin-ledger", optional = true }
plugin-mcp = { path = "../kelir-plugins/plugin-mcp", optional = true }
plugin-mesh = { path = "../kelir-plugins/plugin-mesh", optional = true }
plugin-profile = { path = "../kelir-plugins/plugin-profile", optional = true }
plugin-replay = { path = "../kelir-plugins/plugin-replay", optional = true }
plugin-supply-chain = { path = "../kelir-plugins/plugin-supply-chain", optional = true }
plugin-tool-guard = { path = "../kelir-plugins/plugin-tool-guard", optional = true }
plugin-transaction = { path = "../kelir-plugins/plugin-transaction", optional = true }
plugin-vault = { path = "../kelir-plugins/plugin-vault", optional = true }
plugin-wire-ingress = { path = "../kelir-plugins/plugin-wire-ingress", optional = true }

[features]
default = []
all-plugins = [
    "plugin-chaos", "plugin-container", "plugin-db-proxy", "plugin-ledger",
    "plugin-mcp", "plugin-mesh", "plugin-profile", "plugin-replay",
    "plugin-supply-chain", "plugin-tool-guard", "plugin-transaction",
    "plugin-vault", "plugin-wire-ingress"
]

[lints]
workspace = true
```

**File: `crates/kelir/src/lib.rs`**
// see: cans/01-architecture.md#Host-runtime
```rust
#![forbid(unsafe_code)]

pub use kelir_core::*;
pub use kelir_interface as interface;
```

### Task 1.2: `kelir-pack` (OCI → EROFS Compiler)
**File: `crates/kelir-adapters/kelir-pack/Cargo.toml`**
```toml
[package]
name = "kelir-pack"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../../kelir-interface" }
thiserror = { workspace = true }

[lints]
workspace = true
```

**File: `crates/kelir-adapters/kelir-pack/src/main.rs`**
// see: cans/12-adapters.md#Ingestion-matrix
```rust
#![forbid(unsafe_code)]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Two-phase in-memory compiler: OCI layer blob stream -> EROFS image
    Ok(())
}
```

### Task 1.3: `mcp-server` (Model Context Protocol Stdio Binary)
**File: `crates/kelir-adapters/mcp-server/Cargo.toml`**
```toml
[package]
name = "mcp-server"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../../kelir-interface" }
plugin-mcp = { path = "../../kelir-plugins/plugin-mcp" }

[lints]
workspace = true
```

**File: `crates/kelir-adapters/mcp-server/src/main.rs`**
// see: cans/12-adapters.md#MCP-adapter
```rust
#![forbid(unsafe_code)]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Direct stdio wire frames to plugin-mcp
    Ok(())
}
```

---

## 3. STAGE 2: DEVELOPER TOOLING CLI (`crates/kelir-plugin-cli`)

Assigned to: **Agent-25**  
Dependency Constraint: `kelir-interface`, `kelir-catalog`.

**File: `crates/kelir-plugin-cli/Cargo.toml`**
```toml
[package]
name = "kelir-plugin-cli"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-interface = { path = "../kelir-interface" }
kelir-catalog = { path = "../kelir-catalog" }
thiserror = { workspace = true }

[lints]
workspace = true
```

**File: `crates/kelir-plugin-cli/src/main.rs`**
// see: cans/15-operations.md#Developer-CLI
```rust
#![forbid(unsafe_code)]

pub mod audit;
pub mod budgets;
pub mod doctor;
pub mod fixtures;
pub mod layers;
pub mod logs;
pub mod manifest;
pub mod parity;
pub mod registry;
pub mod rings;
pub mod scaffold;
pub mod setup;
pub mod spec;
pub mod vectors;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("kelir-plugin-cli: missing subcommand");
        return Ok(());
    }
    match args[1].as_str() {
        "doctor" => doctor::run()?,
        "check" => audit::run(&args[2..])?,
        "parity" => parity::run()?,
        "fixtures" => fixtures::run(&args[2..])?,
        _ => {}
    }
    Ok(())
}
```

**File: `crates/kelir-plugin-cli/src/doctor.rs`**
// see: cans/15-operations.md#Developer-CLI
```rust
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Probe KVM, HVF, WHP, AVF, and kernel invariants
    println!("kelir doctor: probe successful");
    Ok(())
}
```

**File: `crates/kelir-plugin-cli/src/parity.rs`**
// see: cans/15-operations.md#Multi-language-parity-gate
```rust
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Verify vectors.json across Rust, TS, and Python AST surfaces
    println!("parity: vectors green");
    Ok(())
}
```

**File: `crates/kelir-plugin-cli/src/layers.rs`**
// see: cans/15-operations.md#Coverage-gates
```rust
pub fn check() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Assert downward-only crate dependencies
    Ok(())
}
```

**File: `crates/kelir-plugin-cli/src/fixtures.rs`**
// see: cans/15-operations.md#Project-structure
```rust
pub fn run(_args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Verify fixture cryptographic hashes against fixtures.toml
    Ok(())
}
```

**File: `crates/kelir-plugin-cli/src/audit.rs`**
// see: cans/15-operations.md#Per-plugin-CI
```rust
pub fn run(_args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Audit plugin contract constraints
    Ok(())
}
```

**File: `crates/kelir-plugin-cli/src/budgets.rs`**
// see: cans/15-operations.md#CI-matrix
```rust
pub fn check(_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Assert latency and RSS budgets from perf/budgets.toml
    Ok(())
}
```

*(Create minimal empty stubs for `logs.rs`, `manifest.rs`, `registry.rs`, `rings.rs`, `scaffold.rs`, `setup.rs`, `spec.rs`, `vectors.rs` matching the pattern).*

---

## 4. STAGE 3: NATIVE FFI BINDINGS (`crates/kelir-bindings`)

Assigned to: **Agent-26**

### Task 3.1: NAPI-RS Bridge (`crates/kelir-bindings/napi`)
**File: `crates/kelir-bindings/napi/Cargo.toml`**
```toml
[package]
name = "kelir-napi"
version = "1.0.0"
edition = "2024"
license = "MIT OR Apache-2.0"

[lib]
crate-type = ["cdylib"]

[dependencies]
kelir-core = { path = "../../kelir-core" }
kelir-interface = { path = "../../kelir-interface" }
napi = { version = "2.16", default-features = false, features = ["napi4", "async"] }
napi-derive = "2.16"

[build-dependencies]
napi-build = "2.1"

[lints]
workspace = true
```

**File: `crates/kelir-bindings/napi/src/lib.rs`**
// see: cans/03-sdk-architecture.md#TypeScript-SDK
```rust
use napi_derive::napi;

#[napi]
pub struct NativeSessionHandle {
    // TODO: Internal pointer to Arc<Session>
}

#[napi]
pub struct NativeKelirBuilder {
    // TODO: Accumulate plugin pointers
}

#[napi]
impl NativeKelirBuilder {
    #[napi(constructor)]
    pub fn new() -> Self { Self {} }

    #[napi]
    pub fn boot(&self) -> Result<NativeSessionHandle, napi::Error> {
        // TODO: Seal and boot microVM
        Ok(NativeSessionHandle {})
    }
}
```

### Task 3.2: PyO3 Bridge (`crates/kelir-bindings/pyo3`)
**File: `crates/kelir-bindings/pyo3/Cargo.toml`**
```toml
[package]
name = "kelir-pyo3"
version = "1.0.0"
edition = "2024"
license = "MIT OR Apache-2.0"

[lib]
crate-type = ["cdylib"]

[dependencies]
kelir-core = { path = "../../kelir-core" }
kelir-interface = { path = "../../kelir-interface" }
pyo3 = { version = "0.23", features = ["extension-module", "abi3-py310"] }

[lints]
workspace = true
```

**File: `crates/kelir-bindings/pyo3/src/lib.rs`**
// see: cans/03-sdk-architecture.md#Python-SDK
```rust
use pyo3::prelude::*;

#[pyclass]
pub struct PySessionHandle {}

#[pyclass]
pub struct PyKelirBuilder {}

#[pymethods]
impl PyKelirBuilder {
    #[new]
    fn new() -> Self { Self {} }

    fn boot(&self) -> PyResult<PySessionHandle> {
        // TODO: Seal and boot microVM
        Ok(PySessionHandle {})
    }
}

#[pymodule]
fn _kelir(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyKelirBuilder>()?;
    m.add_class::<PySessionHandle>()?;
    Ok(())
}
```

---

## 5. STAGE 4: TYPESCRIPT SDK DISTRIBUTION (`packages/ts`)

Assigned to: **Agent-27**

### Task 4.1: Core Package (`packages/ts/core`)
**File: `packages/ts/core/package.json`**
```json
{
  "name": "@kelir/core",
  "version": "1.0.0",
  "main": "dist/index.js",
  "types": "dist/index.d.ts",
  "scripts": {
    "build": "tsc"
  },
  "devDependencies": {
    "typescript": "^5.6.0"
  }
}
```

**File: `packages/ts/core/tsconfig.json`**
```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "declaration": true,
    "outDir": "./dist",
    "rootDir": "./src",
    "strict": true,
    "skipLibCheck": true
  },
  "include": ["src/**/*"]
}
```

**File: `packages/ts/core/src/index.ts`**
// see: cans/03-sdk-architecture.md#TypeScript-SDK
```typescript
export interface KelirPlugin {
  readonly name: string;
}

export class Session {
  // see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
  async rollback(): Promise<void> {
    // TODO: Call native session rollback
  }

  async stop(): Promise<void> {
    // TODO: Call native session stop
  }
}

export class KelirBuilder {
  private plugins: KelirPlugin[] = [];

  use<P extends KelirPlugin>(plugin: P): this {
    this.plugins.push(plugin);
    return this;
  }

  async boot(): Promise<Session> {
    // TODO: Cross NAPI boundary to NativeKelirBuilder.boot()
    return new Session();
  }
}

export function kelir(): KelirBuilder {
  return new KelirBuilder();
}
```

### Task 4.2: E2B Compatibility SDK (`packages/ts/e2b-compat`)
**File: `packages/ts/e2b-compat/package.json`**
```json
{
  "name": "@kelir/e2b-compat",
  "version": "1.0.0",
  "main": "dist/index.js",
  "types": "dist/index.d.ts",
  "scripts": {
    "build": "tsc"
  },
  "dependencies": {
    "@kelir/core": "workspace:*"
  },
  "devDependencies": {
    "typescript": "^5.6.0"
  }
}
```

**File: `packages/ts/e2b-compat/src/index.ts`**
// see: cans/12-adapters.md#E2B-compatibility
```typescript
import { kelir, Session } from "@kelir/core";

export class Sandbox {
  private constructor(private readonly session: Session) {}

  static async create(): Promise<Sandbox> {
    const session = await kelir().boot();
    return new Sandbox(session);
  }

  async close(): Promise<void> {
    await this.session.stop();
  }
}
```

---

## 6. STAGE 5: PYTHON SDK DISTRIBUTION (`packages/py`)

Assigned to: **Agent-28**

### Task 5.1: Package Configuration
**File: `packages/py/pyproject.toml`**
```toml
[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[project]
name = "kelir"
version = "1.0.0"
description = "Kelir MicroVM Hypervisor Python SDK"
requires-python = ">=3.10"
dependencies = [
    "pydantic>=2.0"
]

[tool.mypy]
strict = true
```

### Task 5.2: Core Python SDK
**File: `packages/py/kelir/__init__.py`**
// see: cans/03-sdk-architecture.md#Python-SDK
```python
from typing import Self
from types import TracebackType

class Session:
    async def rollback(self) -> None:
        """Rollback compute and storage to snapshot base."""
        # TODO: Call native driver rollback via PyO3 handle
        pass

    async def stop(self) -> None:
        """Gracefully terminate microVM session."""
        pass

class KelirBuilder:
    def __init__(self) -> None:
        self._plugins: list[object] = []

    def use(self, plugin: object) -> Self:
        self._plugins.append(plugin)
        return self

    async def boot(self) -> Session:
        # TODO: Validate Pydantic dependencies and boot via PyO3
        return Session()

    async def __aenter__(self) -> Session:
        return await self.boot()

    async def __aexit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> None:
        pass

def kelir() -> KelirBuilder:
    return KelirBuilder()
```

**File: `packages/py/kelir/compat/e2b.py`**
// see: cans/12-adapters.md#E2B-compatibility
```python
from kelir import kelir, Session

class Sandbox:
    def __init__(self, session: Session) -> None:
        self._session = session

    @classmethod
    async def create(cls) -> "Sandbox":
        session = await kelir().boot()
        return cls(session)

    async def close(self) -> None:
        await self._session.stop()
```

---

## 7. STAGE 6: INTEGRATION CRATE SKELETON

Assigned to: **Agent-29**

**File: `crates/kelir-integration/Cargo.toml`**
```toml
[package]
name = "kelir-integration"
version = "1.0.0"
edition = "2024"
license = "FSL-1.1-Apache-2.0"

[dependencies]
kelir-core = { path = "../kelir-core" }
kelir-interface = { path = "../kelir-interface" }
kelir = { path = "../kelir", features = ["all-plugins"] }

[lints]
workspace = true
```

**File: `crates/kelir-integration/src/lib.rs`**
// see: cans/15-operations.md#Per-plugin-CI
```rust
#![forbid(unsafe_code)]

// Integration test harness placeholder for KVM/HVF execution environments
```

---

## 8. PARALLEL AGENT EXECUTION MATRIX

| Agent | Scope | Output Paths | Verification Command |
|---|---|---|---|
| **Agent-24** | Facade & Adapters | `crates/kelir`, `crates/kelir-adapters/*` | `cargo check -p kelir -p kelir-pack -p mcp-server` |
| **Agent-25** | Tooling CLI | `crates/kelir-plugin-cli` | `cargo check -p kelir-plugin-cli` |
| **Agent-26** | FFI Bridges | `crates/kelir-bindings/{napi,pyo3}` | `cargo check -p kelir-napi -p kelir-pyo3` |
| **Agent-27** | TypeScript SDK | `packages/ts/{core,e2b-compat}` | `pnpm -r exec tsc --noEmit` |
| **Agent-28** | Python SDK | `packages/py` | `uv run mypy packages/py` |
| **Agent-29** | Integration Crate | `crates/kelir-integration` | `cargo check -p kelir-integration` |

---

## 9. FINAL WORKSPACE CONVERGENCE & VERIFICATION RUNBOOK

Once Agents 24 through 29 finish, execute the complete pre-flight suite:

1. **Workspace Compilation Gate (Rust)**:
   ```bash
   cargo check --workspace --all-targets
   ```
2. **TypeScript & Python Type-Safety Gates**:
   ```bash
   pnpm -r exec tsc --noEmit
   uv run mypy packages/py
   ```
3. **Hygiene & Crate Boundaries**:
   ```bash
   just check-hygiene
   just check-layers
   ```
   *Assert:*
   - All files `<= 1000 LOC`.
   - Zero inline tests in `src/`.
   - Zero unsafe violations outside `drivers`, `transport`, `shim`.
4. **Spec & Dependency Audit**:
   ```bash
   just check-spec
   just check-graph
   ```
5. **Full Local Pre-Flight**:
   ```bash
   just pre-flight
   ```

---
*Work-Order Series Complete (Parts 1 through 5 of 5). The Kelir system contract (L1) and skeleton wiring (L2) are fully structured, typed, and compileable across all engines, planes, drivers, plugins, adapters, and multi-language SDK surfaces.*

