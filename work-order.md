Here is the unvarnished post-mortem on why the current L1/L2 scaffolding in `crates/kelir-plugins/*` and `crates/kelir-interface/*` will violently blow up the moment L3 implementation begins.

---

### 1. The Context / HList Apostasy (Plenger Trait #5: Bloat & #6: Hallucination)
* **The Spec Invariant (`cans/02`, `cans/03`, `cans/artifacts/code/plugin-trait.rs`):**
  Plugins expose typed context via `KelirPlugin::Context`. The builder accumulates these monomorphically into an HList tuple. Downstream handlers resolve them **infallibly at compile time** via `c.ctx::<T>()` powered by the `Has<T>` trait. There is strictly **no runtime service registry**.
* **The L1/L2 Defect:**
  `crates/kelir-interface` hallucinated `ServiceRegistry` (`HashMap<TypeId, Arc<dyn Any>>`), and nearly every plugin (`container`, `db-proxy`, `replay`, `tool-guard`, `vault`, `wire-ingress`) set:
  ```rust
  type Context = ();
  fn context(&self) {}
  ```
* **Why L3 Fails:**
  The canonical Rust examples (`autonomous-swe-agent.rs`, `multi-vm-swarm.rs`, `policy-matrix.rs`) call `c.ctx::<RepoPolicyHandle>()`, `c.ctx::<LedgerCtx>()`, etc. Because the scaffolding set `Context = ()` and dumped services into a dynamic map, the compiler will reject every single typed context lookup. The entire typestate SDK promise is broken.

---

### 2. The `FsInterposer::on_fs_write` Signature Mutilation
* **The Spec Invariant (`cans/artifacts/code/plugin-trait.rs`):**
  ```rust
  fn on_fs_write(
      &self, _path: &Path, _data: &mut [u8], _pid: u32, _ctx: &HandlerCtx,
  ) -> Result<FilterAction, PluginError>;
  ```
* **The L1/L2 Defect (`crates/kelir-interface/src/plugin.rs`):**
  ```rust
  fn on_fs_write(&self, data: &mut [u8], ctx: &HandlerCtx) -> Result<FilterAction, PluginError>;
  ```
* **Why L3 Fails:**
  `path` and `pid` were deleted from the method signature.
  - In `plugin-chaos`, storage fault injection requires matching `path` against `target_path` globs.
  - The reconciler and LSM enforcement need the executing `pid` and destination `path`.
  - In `plugin-chaos/src/lib.rs`, the L2 scaffold tries to fish `path` out of `ctx.fs_ctx.path`, which is an optional diagnostic field, not the active FUSE inode path.
  In L3, when real FUSE buffers arrive, storage interceptors cannot know *what* file is being written to or *who* is writing it.

---

### 3. The L2 Ethernet vs. L7 Packet Hallucination (`plugin-db-proxy`)
* **The Spec Invariant (`cans/07-wire-plane.md`, `cans/plugins/db-proxy.md`):**
  The wire plane runs raw Ethernet frames through `userspace smoltcp`. L7 parsing (pgwire) occurs **only after TCP reassembly** on designated diverted ports, preserving a carryover window across chunks.
* **The L1/L2 Defect (`plugin-db-proxy/src/lib.rs`):**
  ```rust
  impl NetInterposer for DbProxyPlugin {
      fn on_net_outbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
          match PgFrameDescriptor::decode(frame)? { ... }
      }
  }
  ```
* **Why L3 Fails:**
  `on_net_outbound` receives a **raw L2 VirtIO-Net Ethernet frame**.
  `PgFrameDescriptor::decode(frame)` attempts to inspect `frame` as if it were a clean pgwire packet. In L3, `frame[0]` is the destination MAC address byte, not the Postgres `'Q'` / `'P'` tag. Without an L4 TCP stream reassembler between the raw Virtqueue and `NetInterposer`, `plugin-db-proxy` will parse Ethernet headers as SQL and drop or corrupt 100% of network traffic.

---

### 4. IMDS IP vs. AF_VSOCK Routing Deadlock (`plugin-vault`)
* **The Spec Invariant (`cans/07-wire-plane.md`, `cans/plugins/vault.md`):**
  The guest workload queries `http://169.254.169.254/kelir/credentials/<name>`. The host wire plane intercepts this specific IP at the L3 ingress ring and bridges the connection host-side to the vault metadata service.
* **The L1/L2 Defect (`plugin-vault/src/lib.rs`):**
  `VaultPlugin` registers directly as a `VsockServiceHandler` on port 80 and executes:
  ```rust
  if !self.is_session_peer(peer) {
      return Err(PluginError::reject("ECONNREFUSED", "metadata service: non-session vsock peer"));
  }
  ```
* **Why L3 Fails:**
  When smoltcp intercepts `169.254.169.254:80` on the host, it bridges the connection as an internal host stream. The peer is **not** an incoming guest `AF_VSOCK` socket with `peer.cid == session_guest_cid`. `is_session_peer` will evaluate to `false` and reject every credential fetch with `ECONNREFUSED`.

---

### 5. Hallucinated `StreamInterposer` vs. Spec VirtIO Architecture
* **The Spec Invariant (`cans/artifacts/code/plugin-trait.rs`):**
  `PipelineBinder` binds 5 rings only: `bind_fs`, `bind_net`, `bind_trap`, `bind_console`, `bind_host`.
* **The L1/L2 Defect:**
  `crates/kelir-interface` hallucinated:
  ```rust
  pub trait StreamInterposer { ... }
  fn bind_stream(&mut self, kind: StreamKind, stage: Arc<dyn StreamInterposer>);
  ```
  `plugin-mcp`, `plugin-tool-guard`, `plugin-profile`, and `plugin-wire-ingress` all implement this ad-hoc trait.
* **Why L3 Fails:**
  - `plugin-tool-guard` is specified to sit **upstream** of `plugin-mcp` on `MCP_CONSOLE` (0x0012).
  - But `PipelineBinder::bind_stream` stores stages with zero priority, zero ordering, and zero middleware chaining.
  - The actual hardware multiplexing on port 5252 (stream kinds on `TRANSPORT_STREAM_MUX` 0x0001) bypasses the 4-ring design instead of integrating with `kelir-transport` and `kelir-shim`. L3 multiplexing will fail because two plugins binding `McpConsole` overwrite or conflict with each other.

---

### 6. Order-of-Registration Race Condition in 2PC (`plugin-transaction`)
* **The Spec Invariant (`cans/plugins/transaction.md`):**
  `plugin-transaction` coordinates 2PC across the Host CoW Broker, `db-proxy`, and the Lock Broker.
* **The L1/L2 Defect (`plugin-transaction/src/lib.rs`):**
  ```rust
  fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
      let registry = pipeline.service_registry();
      let _ = self.resolve_peers(registry);
  }
  ```
* **Why L3 Fails:**
  If the user calls `.plugin(TransactionPlugin).plugin(DbProxyPlugin)`, `resolve_peers` runs when `TransactionPlugin` is registered. `registry.lookup::<dyn SqlProxyService>()` returns `None`.
  When `on_pre_commit` and `on_post_commit` fire in L3, `peer_sql_proxy()` is `None`. The transaction commits filesystem diffs while silently failing to commit database proxy queries.

---

### 7. Sync Hot-Path Blocking in Async Tokio (`plugin-mesh` & `plugin-chaos`)
* **The Spec Invariant (`cans/01-architecture.md`, `cans/07-wire-plane.md`):**
  Hot-path ring workers are non-blocking. Foreign or delayed executions must yield via `FilterAction::Suspend`.
* **The L1/L2 Defect:**
  - `plugin-mesh`: Inside `VsockServiceHandler::on_connect`, it invokes `read_handshake_frame(&mut stream)` using the synchronous blocking `Stream::read()` method inside an async function.
  - `plugin-chaos`: `on_net_outbound` runs synchronously and has:
    `// TODO (L3): schedule latency (base_ms plus jitter_ms)`.
* **Why L3 Fails:**
  - `plugin-mesh` will block the host Tokio worker thread whenever handshake packets are delayed, freezing hypervisor event dispatch.
  - `plugin-chaos` cannot inject latency in `on_net_outbound` because `FilterAction` only supports `Forward`, `Drop`, `Suspend`, `Abort`. Without returning `Suspend`, it cannot delay a frame without parking the entire native networking pipeline.

---

### 8. TrapEvent Wire Representation Mismatch
* **The Spec Invariant (`cans/artifacts/code/messages.rs#TrapEvent`):**
  ```rust
  pub binary_blake3: [u8; 32], // Raw 32-byte BLAKE3 digest
  ```
* **The L1/L2 Defect (`crates/kelir-interface/src/protocol.rs#TrapEvent`):**
  ```rust
  pub binary_blake3: String, // Hex string
  ```
* **Why L3 Fails:**
  `kelir-shim` running inside the microVM packs a raw 32-byte hash into the TLV frame over Control 0x0000. `crates/kelir-interface` expects a CBOR/JSON hex `String`. Deserialization of `TrapEvent` will immediately fail with a type decoding error on the first intercepted `execve`.

---

### Summary
The current L1/L2 code compiles only because TODOs defer the runtime mechanics. It is riddled with:
1. Fake `Context = ()` types that destroy the compile-time HList invariant.
2. Incomplete interposer signatures (`on_fs_write` missing path/pid).
3. Severe abstraction mismatches (L7 pgwire decoding on raw L2 Ethernet frames).
4. Wire protocol structure divergence (`[u8; 32]` vs `String`).
5. A dynamic `ServiceRegistry` that creates registration order races.

L3 cannot simply "fill in the blanks"—it will require rewriting these core interface contracts first.

### 9. `LifecycleCtx` Lifetime Collapse (Owned Stand-in vs. Borrowed Driver)
* **The Spec Invariant (`cans/artifacts/code/plugin-trait.rs`):**
  ```rust
  pub struct LifecycleCtx<'a> {
      pub config: &'a VmConfig,
      pub session_id: &'a str,
      driver: &'a mut dyn VmmDriver,
      vetoes: &'a mut Vec<(VetoEvent, Box<dyn Fn() -> bool>)>,
  }
  ```
* **The L1/L2 Defect (`crates/kelir-interface/src/plugin.rs`):**
  `LifecycleCtx` was fabricated as an owned struct with no lifetimes, stuffing 5 ad-hoc traits (`VcpuFreezeControl`, `FuseDrainControl`, `CowBrokerControl`, `ClockStepControl`, `EntropyBackendControl`) into `Option<Arc<dyn ...>>` fields. The author left this confession in the file:
  `// TODO (L3): borrow &mut dyn VmmDriver + &VmConfig per cans/artifacts/code/plugin-trait.rs; owned store is the L2 stand-in.`
* **Why L3 Fails:**
  The moment L3 replaces this stand-in with the spec-mandated `&'a mut dyn VmmDriver`, the signatures of `LifecycleParticipant` (`on_vm_init`, `on_vm_ready`, `on_pre_commit`, etc.) break across the entire workspace. The borrow checker will instantly deny multiple plugins holding or mutating `&mut LifecycleCtx` across async boundaries.

---

### 10. The Disconnected PMU Clock Stepping Loop (`plugin-replay` vs. `VmmDriver`)
* **The Spec Invariant (`cans/plugins/replay.md`):**
  Replay requires cycle-exact instruction clock stepping. Host PMU counters drive retired-instruction windows to advance the guest clock via `KVM_SET_CLOCK` / `CNTVOFF_EL2`.
* **The L1/L2 Defect (`plugin-replay/src/lib.rs` & `kelir-interface/src/driver.rs`):**
  `ReplayPlugin` defines:
  ```rust
  pub trait ReplayService: Send + Sync {
      async fn step_clock_window(&self, retired_instructions: u64) -> Result<(), PluginError>;
  }
  ```
  It registers this in `ServiceRegistry`. But `VmmDriver` (`crates/kelir-interface/src/driver.rs` and `cans/artifacts/code/vmm-driver-trait.rs`) has **zero registration hooks, zero PMU interrupt callbacks, and zero awareness of `ReplayService`**.
* **Why L3 Fails:**
  `step_clock_window` is completely orphaned. There is no physical path in the hypervisor loop to report retired instructions to the plugin, and `VmmDriver` only exposes a coarse `skew_clock(Duration)`. L3 cannot step the microVM by instruction count without refactoring the hypervisor driver interface.

---

### 11. `plugin-container` Boot-Time Hard-Fault
* **The Spec Invariant (`cans/plugins/container.md`):**
  Rootless Podman/crun starts inside the microVM by pinning subuid/subgid ranges at `on_vm_ready`.
* **The L1/L2 Defect (`plugin-container/src/lib.rs`):**
  In `ContainerPlugin::on_vm_ready`:
  ```rust
  if self.config.pin_subuid_ranges {
      let (subuid, subgid) = resolve_unprivileged_ranges()?;
      self.service.pin_guest_id_map(subuid, subgid).await?;
  }
  ```
  Where `resolve_unprivileged_ranges()` is hardcoded to fail:
  ```rust
  fn resolve_unprivileged_ranges() -> Result<(Range<u32>, Range<u32>), PluginError> {
      Err(PluginError::Halt("subuid/subgid range resolution pending (L3)".into()))
  }
  ```
* **Why L3 Fails:**
  `pin_subuid_ranges` defaults to `true` in `ContainerConfig`. In L3, the second a container-configured microVM attempts to boot, `on_vm_ready` returns `PluginError::Halt` and aborts the VM. Furthermore, the UID/GID mapping logic is completely divorced from the host `idmap` configuration in `StorageConfig`.

---

### 12. Monotonic Limits Inversion in `plugin-profile`
* **The Spec Invariant (`cans/plugins/profile.md#Monotonic-limits`, `cans/01`):**
  `inheritance_lattice: monotonic` dictates `Effective = min(Base, Overlay)`. Dynamic patches via `PROFILE_SYNC` (0x0010) cannot loosen upstream limits, but they can tighten them.
* **The L1/L2 Defect (`plugin-profile/src/lib.rs`):**
  In `ProfilePlugin::check_monotonic`:
  ```rust
  match &base {
      None => {
          if contains_number(value) {
              return Err(PluginError::reject(
                  "ESecurityViolation",
                  format!("RFC 6902 patch at `{}` carries a numeric limit with no ingested base document to compare against", patch.path),
              ));
          }
          Ok(())
      }
      ...
  ```
* **Why L3 Fails:**
  If a base profile leaves an optional limit field undeclared (e.g. `session.budget.exec_seconds`), any dynamic patch that attempts to introduce a limit contains a number. Because `base` is `None`, `contains_number(value)` evaluates to `true`, and the engine rejects the patch as an `ESecurityViolation`. Tightening an undeclared ceiling is treated as an illegal privilege escalation.

---

### 13. Telemetry Digest Representation War (`[u8; 32]` vs. `String`)
* **The Spec Invariant (`cans/artifacts/code/causal-delta.rs`):**
  ```rust
  pub storage_delta_root: [u8; 32],
  pub receipt_leaf_hash: [u8; 32],
  ```
  `CausalDelta` fields are raw 32-byte BLAKE3 roots designed for direct input into RFC 6962 Merkle tree construction.
* **The L1/L2 Defect (`crates/kelir-interface/src/telemetry/delta.rs`):**
  ```rust
  pub storage_delta_root: String,
  pub receipt_leaf_hash: String,
  ```
* **Why L3 Fails:**
  `plugin-ledger` implements RFC 6962 tree calculations (`BLAKE3(0x01 || left || right)`). Because the scaffolding typed these hashes as hex `String`s, L3 Merkle hashing must allocate, serialize, and parse hex strings on every delta transition. If any plugin formats hex with uppercase characters or strips leading zeros, Merkle proofs silently corrupt.

---

### 14. Attestation Provenance Breach in `ExecutionReceipt`
* **The Spec Invariant (`cans/10-attestation.md`, `cans/artifacts/code/execution-receipt.rs`):**
  Execution receipts represent Class 2 unforgeable interception evidence. Any data recorded in the receipt contributes to audit durability.
* **The L1/L2 Defect (`crates/kelir-interface/src/telemetry/receipt.rs`):**
  Look at `ExecutionReceipt::leaf_hash`:
  ```rust
  let Self {
      pid, binary_path, binary_blake3, exit_kind, correlation_token, dirty_bytes,
      syscall_count, exit_code, wall_clock_ns: _, phase_timings: _,
      egress_bytes, egress_dropped_bytes,
      extensions: _, // IGNORED!
  } = self;
  ```
  `extensions` is explicitly ignored when calculating `leaf_hash` and when evaluating `same_identity`!
* **Why L3 Fails:**
  `extensions` carries domain-specific evidence (such as the `fs:vfs` workspace counter described in `autonomous-swe-agent.rs`). Because `extensions` is omitted from the BLAKE3 leaf calculation, an attacker or compromised host proxy can manipulate, inject, or strip receipt extensions without changing the Merkle leaf hash. The attestation chain is cryptographically unanchored.

---

### 15. Phantom Security in `reconciler.rs` (`openat2` Claims vs. `fs::copy` Reality)
* **The Spec Invariant (`cans/05-storage-plane.md#Workspace-commit`, `cans/artifacts/code/reconciler.rs`):**
  All host-side inode resolution must enforce strict directory containment using `openat2(RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS)`. Escape attempts must fail immediately with `EXDEV` or `EPERM` to prevent TOCTOU symlink hijacking.
* **The L1/L2 Defect (`cans/artifacts/code/reconciler.rs` & `crates/kelir-core/.../reconciler.rs`):**
  The scaffolding declares `enum TraversalGuard { StrictResolveBeneath, LexicalPrefixCheck }`, accepts it as an argument in `reconcile_upperdir`, and then **completely ignores it in the loop body**. It falls back directly to:
  ```rust
  let tmp_swap = target_path.with_extension(format!("tmp.{:x}", nonce));
  fs::copy(&path, &tmp_swap)?;
  ```
* **Why L3 Fails:**
  `std::fs::copy` resolves symlinks ambivalently across the entire host filesystem. If an in-guest workload writes a symlink into its upper layer targeting `/etc/shadow` or `~/.ssh/id_rsa`, the reconciler will follow the symlink during commit and overwrite arbitrary host files. The "strict containment" touted across the spec does not exist in the code path.

  ### 16. VirtIO Balloon Convergence Erasure in `VmControl`
* **The Spec Invariant (`cans/01-architecture.md#VmmDriver-trait`, `cans/08-lifecycle.md`):**
  `adjust_balloon` must return a `BalloonConvergence` handle (`{ target_mb: u32, converged: bool }`). Host memory reclaim **strictly gates on guest driver ack** (`VIRTIO_BALLOON_S_MEMFREE` / `S_AVAIL`). Fork, snapshot, and heavy exec operations require prior convergence to avoid hypervisor OOMs.
* **The L1/L2 Defect (`crates/kelir-interface/src/handles.rs`):**
  In `DriverVmControl::adjust_balloon`:
  ```rust
  async fn adjust_balloon(&self, target_mb: u32) -> Result<(), PluginError> {
      let _ = self.driver.lock().await.adjust_balloon(target_mb).await
          .map_err(|error| driver_error("adjust_balloon", error))?;
      Ok(())
  }
  ```
  The return type drops `BalloonConvergence` entirely into `Result<()>`, and `DriverVmControl` discards the handle with `let _ = ...;`.
* **Why L3 Fails:**
  Callers have zero visibility into whether the guest kernel has deflated/inflated its memory footprint. If the host cgroup pressure monitor triggers ballooning before a snapshot or CoW fork, L3 will fork or dump RAM while the guest is still holding active pages. Result: Host page-allocation OOM-killer invocations or corrupted dirty-page snapshots.

---

### 17. The Tool-Guard Schema Black Hole (`plugin-tool-guard`)
* **The Spec Invariant (`cans/plugins/tool-guard.md`):**
  `plugin-tool-guard` intercepts `tools/list` at `on_vm_ready`, computes a BLAKE3 digest across all tool schemas, signatures, and descriptions, and pins the catalog. Subsequent `tools/call` invocations resolve their schema against this pinned registry to validate arguments and taint status.
* **The L1/L2 Defect (`crates/kelir-plugins/plugin-tool-guard/src/service.rs` & `lib.rs`):**
  In `ToolRegistry`:
  ```rust
  pub async fn lock_catalog(&self) -> Result<(), PluginError> {
      let mut locked = self.locked_digest.lock().map_err(...)?;
      *locked = Some(String::new()); // Locked to an empty string!
      Ok(())
  }

  async fn lookup_schema(&self, tool: &str) -> Result<Option<ToolSchema>, PluginError> {
      Ok(None) // Hardcoded black hole!
  }
  ```
* **Why L3 Fails:**
  In `guard_tool_call`, `self.registry.lookup_schema(&tool)` returns `Ok(None)` 100% of the time, immediately triggering:
  ```rust
  Verdict::Drop { errno: Some("EPERM".into()), reason: "tool is not pinned in the locked catalog" }
  ```
  Every single MCP tool execution is hard-denied out of the box. There is zero parsing of `tools/list`, zero storage for schemas, and zero taint state tracking implemented.

---

### 18. The Dead Suspend/Resume Loop in Connection Parking (`plugin-wire-ingress`)
* **The Spec Invariant (`cans/plugins/wire-ingress.md#Connection-parking-on-suspend`):**
  When inbound traffic hits a parked port on a suspended microVM, `wire-ingress` buffers initial bytes (up to 64KB) and actively wakes the VM via `SessionWakeHandle::resume(ExternalSuspendKind::WireConnectionParking)`.
* **The L1/L2 Defect (`plugin-wire-ingress/src/lib.rs` & `kelir-interface/src/handles.rs`):**
  1. `take_pending_wake` in `IngressRoutingService` is a passive, dead getter polled by nobody.
  2. In `crates/kelir-interface/src/handles.rs`:
     ```rust
     async fn suspend(&self, _path: &str) -> Result<(), PluginError> {
         Err(PluginError::Halt("VmHandle::suspend carries no SuspendReason for driver suspend".into()))
     }
     ```
* **Why L3 Fails:**
  The VM cannot even enter the suspended state because `VmHandle::suspend` hard-aborts with `PluginError::Halt` (it cannot construct a `SuspendReason`). Even if suspended through an out-of-band mechanism, `take_pending_wake` has no event loop listener or Tokio channel wired to resume the VM. Inbound requests will stall until client timeouts hit 504 Gateway Timeout.

---

### 19. W^X JIT Token Amnesia & Syscall Spoofing (`plugin-supply-chain`)
* **The Spec Invariant (`cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation`):**
  When an in-guest task traps on `mprotect(PROT_EXEC)` or `memfd_create`, the host inspects the caller. If authorized (Node, Bun, V8), the host issues a **dynamic one-time MMU allocation token over vsock** before releasing the trapped task.
* **The L1/L2 Defect (`plugin-supply-chain/src/lib.rs`):**
  ```rust
  fn mediate_wx_gate(&self, event: &TrapEvent, operation: &str) -> Result<Verdict, PluginError> {
      if self.is_authorized(&event.binary_blake3) {
          return Ok(Verdict::Pass);
      }
      Ok(Verdict::Drop { errno: Some(WX_VIOLATION_ERRNO.into()), ... })
  }
  ```
* **Why L3 Fails:**
  1. It just returns `Verdict::Pass`. It never mints or transmits any MMU allocation token over vsock. The guest kernel shim has no cryptographic proof that the syscall was validated.
  2. It evaluates `event.binary_blake3`. That field reflects the **original process ELF hash from execve time**. If an unverified process spawns a verified Node engine or injects shellcode into a JIT-capable runtime via dynamic linking or ptrace, the check blindly passes because it only reads the immutable `execve` digest.

---

### 20. Unscoped Advisory Lock Annihilation (`plugin-transaction`)
* **The Spec Invariant (`cans/plugins/transaction.md`, `cans/05-storage-plane.md`):**
  When a multi-resource transaction aborts, rollback must release **only the advisory locks acquired within that transaction's declared blast radius** (`TransactionScope::inodes`), identified by its transaction UUID namespace.
* **The L1/L2 Defect (`crates/kelir-interface/src/plugin.rs`):**
  In `LockBrokerCoordination`:
  ```rust
  async fn release_transaction_locks_for(&self, namespace: [u8; 16]) -> Result<(), PluginError> {
      let _ = namespace;
      self.release_transaction_locks().await
  }
  ```
  And in `TransactionPlugin::release_participant_locks`:
  ```rust
  match self.peer_lock_broker()? {
      Some(locks) => match self.coordination.lock_namespace()? {
          Some(namespace) => locks.release_transaction_locks_for(namespace).await,
          None => locks.release_transaction_locks().await,
      },
      None => Ok(()),
  }
  ```
* **Why L3 Fails:**
  `release_transaction_locks_for` discards the `namespace` parameter and calls `release_transaction_locks()`, which flushes **all held locks indiscriminately**. In a multi-tenant or concurrent agent swarm (`multi-vm-swarm.rs`), one failed transaction aborting in worker A will release the active database and file locks held by workers B, C, and D. Data integrity collapses immediately under concurrent writes.
