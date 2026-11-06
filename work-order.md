This L1/L2 scaffolding will fail L3 execution. The current codebase contains critical structural, architectural, and semantic contradictions with the `cans` specification that will prevent compilation, violate performance budgets, or brick the hypervisor planes once real implementations replace the stubs.

---

### 1. The Typestate & Capability Inversion (Rust SDK)
* **Spec Mandate (`cans/03-sdk-architecture.md`):** `KelirBuilder` enforces typestate transitions (`KelirBuilder<Unsealed, Plugins, Params>` $\to$ `KelirBuilder<Sealed, Plugins, Params>` $\to$ `Session<Plugins, Params>`). `HandlerCtx` resolves plugin capabilities at compile time via `Plugins: Has<T>` without `Result` or runtime lookups.
* **Scaffolding Reality (`crates/kelir/src/builder.rs`, `crates/kelir/src/handler_ctx.rs`):**
  * `KelirBuilder` is an unparameterized struct storing an `Option<VmConfig>`.
  * `HandlerCtx::ctx<T: Default>(&self) -> T` returns `T::default()`.
  * `KelirBuilder::ctx<T>` returns `None`.
* **Why L3 Fails:** Compile-time prerequisite checking does not exist in the code. To implement L3 capability checking, the entire SDK surface (`KelirBuilder`, `Session`, `HandlerCtx`) must be discarded and rewritten with monomorphic HLists or type-level tuples.

### 2. Semantic Violation in Storage Plane FUSE Dispatch
* **Spec Mandate (`cans/02-plugin-model.md#Hook-return-semantics` & `cans/05-storage-plane.md`):**
  * Synthetic FUSE inodes are served directly at the lookup boundary: `on_fs_lookup -> Verdict::Pass = fall through; Patch(attr) = serve`.
  * `hook-matrix.rs` explicitly implements this: returning `Verdict::Patch(PatchPayload::FsInode { ... })` on lookup.
* **Scaffolding Reality (`crates/kelir-core/src/planes/storage/fuse.rs`, lines 68–70):**
  ```rust
  Verdict::Patch(_) => Err(PluginError::Halt(
      "FUSE LOOKUP interposer returned Patch, unsupported for lookup".into(),
  )),
  ```
* **Why L3 Fails:** The core FUSE dispatcher explicitly errors out on the exact mechanism specified for synthetic inodes and exercised by the integration examples.

### 3. Asynchronous Poisoning of the Synchronous Storage Path
* **Spec Mandate (`cans/02-plugin-model.md#Shape-of-the-surface` & `cans/05-storage-plane.md`):** Storage and wire hooks are synchronous hot-path interceptors to prevent sleeping host ring workers (`<2µs` budget).
* **Scaffolding Reality (`crates/kelir/src/planes/storage.rs`):**
  * `DynamicFileGenerator` and `SnoopHandler` are defined as `Arc<dyn Fn(...) -> Pin<Box<dyn Future...>>>`.
* **Why L3 Fails:** FUSE filesystem callbacks (`on_fs_read`, `on_fs_write`) in `kelir-interface` are synchronous (`fn on_fs_read(...) -> Result<(), PluginError>`). A synchronous FUSE loop cannot await arbitrary host async futures without blocking worker threads or deadlocking the guest VFS.

### 4. Non-Portable Types in `kelir-interface` Bricking Windows WHP
* **Spec Mandate (`cans/01-architecture.md#Transport-engine-abstraction` & `cans/15-operations.md#CI-matrix`):** Transport and driver traits must abstract across Linux (KVM), macOS (HVF), and Windows (WHP/Named Pipes). The interface crate must compile on all platforms.
* **Scaffolding Reality (`crates/kelir-interface/src/driver.rs`):**
  * `SharedMemoryHandle` contains `pub memfd_raw: std::os::unix::io::RawFd;` without platform cfg guards.
  * `VmmDriver` trait declares `fn transport_connect(&self, channel_id: u32) -> Result<tokio::net::UnixStream, VmmError>;`.
* **Why L3 Fails:**
  * `std::os::unix::io::RawFd` breaks Windows builds immediately.
  * Returning `tokio::net::UnixStream` forces OS Unix Domain Sockets into the hypervisor abstraction, directly violating the spec requirement that transport terminates in in-memory lock-free SPSC rings or named pipes on Windows.

### 5. `kelir-shim` Stream ID vs. Direct Slab Indexing Memory Bug
* **Spec Mandate (`cans/11-protocol.md#Wire-framing` & `cans/11-protocol.md#Guest-paravirtual-micro-init-boundary`):**
  * Stream IDs are 24-bit identifiers up to `MAX_STREAM_ID = 0x00FF_FFFF` (16,777,215).
  * ExecSlot slab is bounded by `MAX_EXEC_SLOTS = 4096`.
* **Scaffolding Reality (`crates/kelir-shim/src/multiplexer.rs`):**
  ```rust
  let idx = usize::try_from(stream_id).map_err(|_| ShimError::InvalidStreamId)?;
  if idx >= MAX_EXEC_SLOTS {
      return Err(ShimError::SlabSaturated);
  }
  self.slots.insert(idx, slot)?;
  ```
* **Why L3 Fails:** The shim treats the multiplexer's monotonic `stream_id` as the direct array index into `ExecSlotSlab`. Any stream ID $> 4095$ immediately fails with `SlabSaturated`, killing the shim even when zero execution slots are active. Stream IDs must be mapped to slots through a translation table, not by using the wire stream ID as a dense array index.

### 6. Information-Destroying Error Casting in Shim Binary
* **Spec Mandate (`cans/01-architecture.md#Axiomatic-foundations` & `cans/03-sdk-architecture.md`):** Zero-panic invariant with structured error envelopes bubbling driver, ring, pid, binary, and errno.
* **Scaffolding Reality (`crates/kelir-shim/src/trap.rs`, lines 51–53):**
  ```rust
  impl From<ShimError> for () {
      fn from(_: ShimError) -> Self {}
  }
  ```
  `crates/kelir-shim/src/main.rs` returns `Result<(), ()>`.
* **Why L3 Fails:** Error reasons (`SeccompInstallPending`, `SlabSaturated`, `InvalidStreamId`) are discarded into unit `()`. The host supervisor cannot construct attestation receipts or determine why a workload failed.

### 7. Facade Namespace Collision Under `--features all-plugins`
* **Scaffolding Reality (`crates/kelir/Cargo.toml` vs `crates/kelir/src/lib.rs`):**
  * `crates/kelir/src/plugins/` re-implements stub modules: `chaos.rs`, `ledger.rs`, `mesh.rs`, `replay.rs`, `supply_chain.rs`.
  * `crates/kelir/src/lib.rs` does:
    ```rust
    pub mod plugins;
    ...
    #[cfg(feature = "plugin-chaos")]
    pub use plugin_chaos::ChaosPlugin;
    ```
* **Why L3 Fails:** `crates/kelir/src/plugins/chaos.rs` defines a `ChaosPlugin`, and `plugin-chaos` crate defines `ChaosPlugin`. Compiling with `--features all-plugins` triggers ambiguous export collisions between the internal stub module and the external plugin crate.

### 8. Dynamic Service Downcasting Impossible Across Plugin Tiers
* **Spec Mandate (`cans/02-plugin-model.md#Plugin-tiering`):** Tier 2/3 plugins coordinate dynamically by looking up peer services via `ServiceRegistry` without cross-tier circular crate dependencies.
* **Scaffolding Reality (`crates/kelir-interface/src/service_registry.rs`):**
  ```rust
  pub fn lookup<S: Send + Sync + 'static>(&self) -> Option<Arc<S>> {
      self.services.get(&TypeId::of::<S>())?.clone().downcast::<S>().ok()
  }
  ```
* **Why L3 Fails:** Rust's `Any::downcast` only works on concrete types, not on trait objects (`dyn Service`). If plugin A exposes `TransactionService` and plugin B only imports the trait from `kelir-interface`, plugin B cannot look up `dyn TransactionService` using concrete downcasting.

### 9. Checkpoint State Machine Models String Branches Instead of PID Lineages
* **Spec Mandate (`cans/08-lifecycle.md#Native-checkpoint-plane`):**
  * "Single slot per active PID lineage; sub-steps fold into Base on `StepVerdict::Commit`, discard on `StepVerdict::Rollback`."
* **Scaffolding Reality (`crates/kelir-core/src/planes/checkpoint/mod.rs`):**
  * Indexes checkpoints by arbitrary Git-style branch strings (`branch: &str`, hardcoded default `"main"`).
* **Why L3 Fails:** Checkpoint and step slot state in the hypervisor must track guest task execution lineages. Indexing by string branch ignores concurrent PID lineages and fails autonomous step verification loops when multiple tasks fork or run in parallel.

### 10. Missing AOT Hook Fusion in Foreign SDKs
* **Spec Mandate (`cans/01-architecture.md#Foreign-runtime-embedding` & `cans/02-plugin-model.md#Foreign-plugin-proxy`):**
  * "AOT hook fusion compiles chained host hooks into a single flat closure. Exactly one FFI trampoline crossing per ring event regardless of hook count."
  * Gate 2 native rules evaluate in `<2µs`; foreign FFI trampoline dispatches in `50–150µs`.
* **Scaffolding Reality (`packages/ts/core/src/builder.ts`, `packages/py/kelir/__init__.py`):**
  * Handlers are pushed into procedural runtime lists (`this.plugins.push(...)`, `self._handlers.setdefault(...)`).
  * `NativeKelirBuilder` (NAPI/PyO3) contains no data structures or AST generation for compiling these hooks into a fused native trampoline.
* **Why L3 Fails:** Iterating through individual foreign handlers on every ring event crosses the FFI boundary $N$ times, exceeding the latency budget defined in `perf/budgets.toml` by orders of magnitude and causing CI budget gate failures.

* ### 11. `plugin-tool-guard`: Trait & Ring Attachment Mismatch
* **Spec Mandate (`cans/plugins/tool-guard.md`):** Total tool-call interposition. Executes upstream of `plugin-mcp` directly on the `MCP_CONSOLE` Stream Mux kind (`0x0012`). Evaluates JSON-RPC `tools/call` payloads and returns engine-level verdicts: `Verdict::Patch(ExecveMutation)`, `Verdict::Drop { errno, reason }`, `Verdict::Suspend`.
* **Scaffolding Reality (`crates/kelir-plugins/plugin-tool-guard/src/lib.rs`):**
  ```rust
  impl ConsoleInterposer for ToolGuardPlugin {
      async fn on_console_input(&self, _port: u32, _buf: &mut Vec<u8>) -> Result<(), PluginError> {
          Ok(())
      }
  }
  ```
* **Why L3 Fails:**
  * `ConsoleInterposer::on_console_input` intercepts character bytes on virtio-console hardware ports (Ports 0–16), not Stream Mux kind `0x0012` (`MCP_CONSOLE`) over virtio-vsock.
  * The method signature returns `Result<(), PluginError>`. It is type-system impossible to return a `Verdict` (`Drop`, `Patch`, `Suspend`) from `ConsoleInterposer`. Tool-guard is attached to the wrong ring trait.

---

### 12. `plugin-vault`: L2 Frame Interception vs. L7 Vsock Credential Brokerage
* **Spec Mandate (`cans/plugins/vault.md` & `cans/07-wire-plane.md`):** In-guest metadata endpoint at `169.254.169.254` is intercepted at L3 and forwarded as HTTP-over-vsock to `CredentialBrokerService`. Raw credentials never enter guest RAM or plaintext supervisor buffers.
* **Scaffolding Reality (`crates/kelir-plugins/plugin-vault/src/lib.rs`):**
  ```rust
  impl NetInterposer for VaultPlugin {
      fn on_net_outbound(&self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
          Ok(FilterAction::Forward)
      }
  }
  ```
* **Why L3 Fails:**
  * `NetInterposer` receives raw Ethernet L2 frames from virtio-net.
  * Brokering short-lived credentials inside `on_net_outbound` requires full TCP stream reassembly, TLS state inspection, and HTTP parsing directly inside an inline L2 packet filter.
  * `plugin-vault` completely lacks the vsock service registration required to serve `http://169.254.169.254/kelir/credentials/<name>`.

---

### 13. `plugin-supply-chain`: Inability to Trap Memory Syscalls via Execve-Only Events
* **Spec Mandate (`cans/plugins/supply-chain.md`):** Dynamic W^X JIT mediation. Intercepts guest `mprotect(PROT_EXEC)` and `memfd_create` via seccomp-unotify. Inspects caller binary BLAKE3 against an authorized catalog. Halts unverified callers with `EWXVIOLATION`.
* **Scaffolding Reality (`crates/kelir-plugins/plugin-supply-chain/src/lib.rs`):**
  ```rust
  #[async_trait]
  impl TrapInterposer for SupplyChainPlugin {
      async fn on_trap(&self, _event: &TrapEvent, _ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
          Ok(Verdict::Pass)
      }
  }
  ```
* **Why L3 Fails:**
  * `TrapEvent` (`crates/kelir-interface/src/protocol.rs`) only exposes `execve` metadata (`binary_path`, `argv`, `envp`, `cwd`).
  * `TrapEvent` contains zero fields for syscall numbers, memory protection bitmasks (`PROT_EXEC`), or file descriptor creation flags.
  * `SupplyChainPlugin` cannot intercept `mprotect` or `memfd_create` because the underlying interface only delivers process execution events.

---

### 14. `plugin-db-proxy`: In-Place L2 Packet Mutation of Fragmented L7 PGWire
* **Spec Mandate (`cans/plugins/db-proxy.md`):** Intercepts port 5432, decodes `Q`/`P` frames via pgwire parser, and rewrites SQL queries in place (`FilterAction::Forward`). Binds SQLite over Stream Mux `DB_SQLITE` (`0x0011`).
* **Scaffolding Reality (`crates/kelir-plugins/plugin-db-proxy/src/lib.rs`):**
  ```rust
  impl NetInterposer for DbProxyPlugin {
      fn on_net_outbound(&self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
          Ok(FilterAction::Forward)
      }
  }
  ```
* **Why L3 Fails:**
  * PGWire statements span multiple TCP segments and MTU-bounded L2 frames.
  * Mutating raw Ethernet frames in `NetInterposer` without TCP stream reconstruction invalidates TCP sequence/acknowledgment numbers and checksums, dropping the connection at the guest or upstream kernel.
  * `DbProxyPlugin` implements neither the `DB_SQLITE` vsock mux handler nor pipeline service exports via `PipelineBinder`.

---

### 15. `plugin-transaction`: Impossible Two-Phase Coordination via Context Starvation
* **Spec Mandate (`cans/plugins/transaction.md`):** Coordinates 2PC across the CoW storage broker, `db-proxy`, and lock broker. Freezes guest vCPUs, drains in-flight FUSE operations during `on_pre_commit`, and commits or purges broker upper files during `on_post_commit`.
* **Scaffolding Reality (`crates/kelir-plugins/plugin-transaction/src/lib.rs`):**
  ```rust
  #[async_trait]
  impl LifecycleParticipant for TransactionPlugin {
      async fn on_pre_commit(&self, ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
          Ok(())
      }
  }
  ```
* **Why L3 Fails:**
  * `LifecycleCtx` intentionally exposes only `watchdog`, `balloon_target_mb`, and `vetoes`.
  * The context provides zero API handles to freeze vCPUs, flush FUSE queues, or access the storage CoW broker.
  * By design (`cans/02-plugin-model.md#Hook-context-restriction`), plugins cannot receive `&mut Session` or driver handles. `TransactionPlugin` is structurally isolated from the very planes it is mandated to synchronize.

---

### 16. `plugin-chaos`: FUSE Error Injection Halts Microkernel Instead of Returning Errno
* **Spec Mandate (`cans/plugins/chaos.md`):** Synthetic filesystem fault injection. Returns errno values like `ENOSPC` to simulate disk-full conditions on guest `write()` syscalls.
* **Scaffolding Reality (`crates/kelir-plugins/plugin-chaos/src/lib.rs` & `crates/kelir-core/src/planes/storage/fuse.rs`):**
  * `ChaosPlugin::on_fs_write` returns `FilterAction::Drop { errno, reason }`.
  * `FilterAction::Drop.errno` is an `Option<String>`.
  * In `fuse.rs`, when `on_fs_write` returns `FilterAction::Drop`, the dispatcher executes:
    ```rust
    FilterAction::Drop { errno, reason } => Err(PluginError::Halt(format!(
        "FUSE WRITE denied: {reason} ({errno:?})"
    ))),
    ```
* **Why L3 Fails:** Instead of returning a FUSE error response with `-ENOSPC` (errno 28) to the guest kernel, `fuse.rs` converts the drop into a fatal `PluginError::Halt`. This crashes the host dispatch worker instead of failing the guest system call gracefully.

---

### 17. `plugin-container`: Missing TrapInterposer for `newuidmap`/`newgidmap`
* **Spec Mandate (`cans/plugins/container.md`):** Enforces rootless container execution. Intercepts `newuidmap` and `newgidmap` execution via trap gates to pin subuid/subgid ranges to unprivileged guest users. Privileged container escapes or raw device mounts must reject with `EPERM`.
* **Scaffolding Reality (`crates/kelir-plugins/plugin-container/src/lib.rs`):**
  ```rust
  impl KelirPlugin for ContainerPlugin { ... }
  impl LifecycleParticipant for ContainerPlugin { ... }
  // TrapInterposer NOT implemented
  ```
* **Why L3 Fails:**
  * `ContainerPlugin` does not implement `TrapInterposer`.
  * Without implementing `TrapInterposer`, it cannot register binary filters for `newuidmap`/`newgidmap` or evaluate `on_trap`.
  * All container execution commands bypass the plugin entirely, permitting unconfined rootless/privileged executions.

---

### 18. `plugin-mesh`: Ring Interception Divergence for Sovereign Vsock Mesh
* **Spec Mandate (`cans/plugins/mesh.md` & `cans/01-architecture.md`):** Sovereign peer federation routes point-to-point peer streams over virtio-vsock listener services (e.g., ports 7001, 7002). Injects and asserts SPIFFE identities and mTLS framing at session boundaries.
* **Scaffolding Reality (`crates/kelir-plugins/plugin-mesh/src/lib.rs`):**
  ```rust
  impl NetInterposer for MeshPlugin {
      fn on_net_outbound(&self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
          Ok(FilterAction::Forward)
      }
  }
  ```
* **Why L3 Fails:**
  * Mesh traffic runs over paravirtual vsock streams (`AF_VSOCK`), not virtio-net Ethernet frames.
  * Intercepting virtio-net L2 frames never captures vsock streams between microVMs.
  * `MeshPlugin` does not implement `register_vsock_service` or Channel 0 identity handshakes, leaving the mesh transport entirely disconnected.

---

### 19. `plugin-replay`: Clock and Entropy Decoupling
* **Spec Mandate (`cans/plugins/replay.md`):** Replaces host TSC/clocksource with deterministic PMU-stepped instruction counting. Substitutes `virtio-rng` backend with a deterministic ChaCha20 seeded stream. Detects negative time drift and raises `EClockRegress`.
* **Scaffolding Reality (`crates/kelir-plugins/plugin-replay/src/lib.rs`):**
  ```rust
  impl KelirPlugin for ReplayPlugin { ... }
  impl LifecycleParticipant for ReplayPlugin { ... }
  ```
* **Why L3 Fails:**
  * `ReplayPlugin` does not hook into early hypervisor boot configuration, `virtio-rng` seed initialization, or PMU instruction metrics.
  * `LifecycleParticipant` contains no hooks to intercept TSC deadline reads or advance `KVM_SET_CLOCK` / `CNTVOFF_EL2`.
  * The plugin cannot enforce determinism because it has no linkage to the hardware driver primitives in `kelir-drivers`.

---

### 20. `kelir-plugin-cli` & `testkit`: Hollow Audit and Mock Test Scaffolding
* **Spec Mandate (`cans/15-operations.md#Per-plugin-CI` & `cans/15-operations.md#Multi-language-parity-gate`):**
  * `kelir-plugin-cli check -p` verifies crate tier layering, downward-only dependencies, and declared ring masks.
  * `kelir-plugin-cli vectors` regenerates canonical `vectors.json` from `kelir-plugin-testkit` to assert identical wire/verdict execution across Rust, TypeScript, and Python.
* **Scaffolding Reality (`crates/kelir-plugin-cli/src/audit.rs`, `src/vectors.rs`, `crates/kelir-plugin-testkit/src/assertions.rs`):**
  * `audit.rs` only checks if `name = "<plugin>"` in `Cargo.toml`. It performs zero validation of Cargo dependency tiers, acyclic graphs, or ring traits.
  * `vectors.rs` fails immediately:
    ```rust
    Err("vectors: regenerating {out} from fixtures requires the testkit vector serializer (L3)".into())
    ```
  * `assertions.rs` in `kelir-plugin-testkit` stubs out core evaluation assertions:
    ```rust
    pub fn assert_deny(verdict: &Verdict) -> Result<(), PluginError> {
        Err(PluginError::Halt("TODO: assert_deny".into()))
    }
    ```
* **Why L3 Fails:**
  * The CI gate `just check-plugin-all` provides false positives because it validates only package names.
  * The multi-language parity gate (`just parity`) cannot run because testkit serialization is nonexistent and `vectors.json` cannot be generated.
  * Any real implementation dropped into L3 will immediately fail parity and CI validation because the test verification infrastructure is completely unbacked.
