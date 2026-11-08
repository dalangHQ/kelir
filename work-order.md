### 1. Contract & Spec Violations (Overt Lies)

* **String-Keyed Service Lookup in TS & Python SDKs**
  * *Spec (`cans/03-sdk-architecture.md#Service-lookup-across-languages`):* "No string-keyed service registry on any SDK. TypeScript: `c.<pluginKey>` typed by `.use()` overloads. Python: `c.cap.<plugin_key>` typed by `.use()` overloads. No `session.ext(T)`."
  * *Code (`packages/ts/core/src/context.ts`, `packages/py/kelir/__init__.py`):* TS uses `c.extension<LedgerHandle>("ledger")` and Python uses `c.cap.extension("ledger")`. The SDKs implemented the exact anti-pattern forbidden by the spec.
* **`FilterAction` Missing `Abort` Variant in SDKs**
  * *Spec (`cans/02-plugin-model.md#Unified-verdict`):* `FilterAction = { Forward, Drop{errno,reason}, Suspend{until}, Abort{reason} }`.
  * *Code (`packages/ts/core/src/verdict.ts`, `packages/py/kelir/__init__.py`):* TS `FilterAction` only defines `forward | drop | suspend`. Python `FilterAction` only defines `forward`, `drop`, `suspend`. Neither SDK can express an in-flight byte-transform abort.
* **`SuspendUntil` Missing `Handshake`**
  * *Spec (`cans/11-protocol.md#Wire-messages`):* `SuspendUntil: Deadline(ms) | Approval | Handshake`.
  * *Code (`packages/ts/core/src/verdict.ts`, `packages/py/kelir/__init__.py`):* Both SDKs omit `Handshake` completely.
* **`CausalDelta` Cannot Be Constructed by Any Plugin**
  * *Code (`crates/kelir-interface/src/telemetry/delta.rs`):* `CausalDelta` is marked `#[non_exhaustive]` and defines zero constructor methods.
  * *Consequence:* `plugin-ledger` (`crates/kelir-plugins/plugin-ledger/src/compute.rs`) explicitly cannot assemble a `CausalDelta` and throws an error at compile/runtime boundary. The L1 contract is broken.
* **Catalog State Machine Lie (`pending` -> `owned`)**
  * *Spec (`cans/08-lifecycle.md#Detached-lifecycle`):* "disk_lock_handoff: SQLite row transition from `pending` to `owned` by the runner; atomic".
  * *Code (`crates/kelir-catalog/src/lib.rs` & `schema.rs`):* `SCHEMA_V1_SQL` has no state or lock column. The code explicitly admits: `// TODO (L3): no pending/owned lock-state column exists in SCHEMA_V1_SQL or cans/06; claim is runner_pid plus lease refresh until the schema gains one.`

---

### 2. Phantom FFI Boundaries (The Binding Shells)

* **NAPI `NativeSessionHandle` is an Empty Shell**
  * *Code (`crates/kelir-bindings/napi/src/lib.rs`):* `NativeSessionHandle` exposes zero methods or fields across the NAPI bridge.
  * *Code (`packages/ts/core/src/session.ts`):* TS expects `native.driverName`, `native.bootMs`, `native.vm`, `native.storage`, `native.wire`, `native.checkpoint`, etc. Not a single plane or handle actually crosses the bridge.
* **PyO3 `PySessionHandle` is an Empty Shell**
  * *Code (`crates/kelir-bindings/pyo3/src/lib.rs`):* `PySessionHandle` exposes zero methods to Python.
  * *Code (`packages/py/kelir/__init__.py`):* Every session plane method (`rollback`, `spawn`, `suspend`, `resume`) unconditionally throws `raise EConfig("kelir native PyO3 bridge (_kelir) is missing...")`.
* **Universal Hard Halt on `Session::boot`**
  * *Code (`crates/kelir-core/src/session.rs`):* `Session::boot_with_engine` ignores its config and immediately returns `Err(PluginError::Halt("TODO (L3): Session::boot".into()))`.
  * *Consequence:* Despite L2 examples claiming runnable status (`autonomous-swe-agent`, `policy-matrix`, `git-gate`), calling `.boot().await` fails unconditionally.

---

### 3. Tautologies & Goodhart's Law (Fake Green Gates)

* **Parity Vector Tautology for Veto & Watchdog**
  * *Code (`crates/kelir-plugin-testkit/src/vectors.rs` & `crates/kelir-plugin-cli/src/parity.rs`):*
    * `veto_case()` generates a mock frame carrying `GuestToHost::Heartbeat` with `expected_verdict: Verdict::Pass`.
    * `watchdog_case()` generates a mock frame carrying `GuestToHost::Heartbeat` with `expected_verdict: Verdict::Pass`.
    * `assert_vectors` only checks if the string `"Fork"`, `"Suspend"`, `"ClockSkew"`, `"Suppress"`, `"Mitigate"` is present in the JSON.
    * No veto or watchdog logic is evaluated. Green tests, zero verification.
* **Scorecard Hardware Probe Fake**
  * *Code (`crates/kelir-testkit/src/bin/scorecard.rs`):* Claims `| AS-07 | Hardware Platform Tier | Strict KVM/HVF/WHP probing via doctor | pass |`.
  * *Reality:* `host_hypervisors()` hardcodes `vec!["hvf", "avf"]` on macOS and `vec!["whp"]` on Windows via `std::env::consts::OS` without invoking `doctor` or probing hardware.
* **Integration Example Oracles**
  * *Code (`crates/kelir-integration/examples/autonomous-swe-agent.rs`, `packages/py/examples/autonomous-swe-agent.py`):* `UserOracle` hardcodes `is_hypothesis_valid: true`, `verify_physical_receipt: true`, and `await_human_approval: sleep(50ms)`. The verification loop cannot fail.

---

### 4. Broken Flow Plumbing & Fake Accounting

* **Dead Quota Accounting in Storage Plane**
  * *Code (`crates/kelir-core/src/planes/storage/mod.rs`):*
    `check_quota` calculates:
    ```rust
    let bytes = self.quota.allocated_bytes.load(...).checked_add(bytes_to_write)?;
    ```
    It compares `bytes` against `max_bytes`, but **never stores it back** into `allocated_bytes`. Cumulative usage is never tracked; quota remains at zero indefinitely.
* **Fake Port Forwarding**
  * *Code (`crates/kelir-core/src/planes/wire/mod.rs`):*
    `WirePlane::forward_port` performs zero host socket binding, collision checking, or NAT table updates. It unconditionally returns `Ok(host_port.unwrap_or(guest_port))`.
* **Gate 3 Trusted-Prefix Skip Does Not Exist**
  * *Spec (`cans/11-protocol.md#Trap-fast-path`):* EROFS binaries bypass Gate 3 FFI crossing; dynamic interpreters check `argv[1..]`.
  * *Code (`crates/kelir-core/src/trap_gate.rs`):* `is_trusted` is computed but only used to decide whether to run an `entropy_hook`. If `gate_2_scope_match` succeeds, it returns `GateDecision::FallThrough` directly into the FFI queue. Interpreter inspection is nonexistent.
* **Tool-Guard DRY & Hash Hallucination**
  * *Spec (`cans/plugins/tool-guard.md`):* Depends on `plugin-mcp`, parses JSON-RPC via MCP AST.
  * *Code (`crates/kelir-plugins/plugin-tool-guard/`):* Does not depend on `plugin-mcp`; duplicates a private `jsonrpc.rs` module. In `service.rs`, `catalog_digest` returns hardcoded `[0u8; 32]` with an inline comment blaming a "Cargo.lock freeze" despite `blake3` already existing in the workspace.
 
### 5. Transport Engine & SPSC Ring Illusion

* **Spec (`cans/01-architecture.md#Shared-memory-channel`, `#Transport-engine-abstraction`):**
  * Both transport channels (Control `0x0000`, Stream Mux `0x0001`) terminate in lock-free SPSC rings mapped to supervisor memory.
  * MMIO/PCI doorbells notify via host `eventfd` and guest virtual interrupts under explicit memory fences without spin-polling.
* **Code (`crates/kelir-transport/src/spsc_ring.rs`, `crates/kelir-core/src/bus/mod.rs`):**
  * `SpscRingTransport::read` and `write` unconditionally return `Err(crate::unsupported("..."))`.
  * No `eventfd`, no guest doorbell, no interrupt injection, and zero memory fences exist in `kelir-transport`.
  * In `kelir-core/src/bus/mod.rs`, `BusDispatcher::poll_ingress` returns `Ok(None)`, while `route_stream_frame` and `send_control_msg` return `Ok(())` without ever reading, writing, or touching the rings. The transport bus is a disconnected stub that silently blackholes frames.

---

### 6. Deterministic Replay & PMU Clock-Lockdown Sham

* **Spec (`cans/plugins/replay.md#Invariants-and-policy-(replay)`, `cans/08-lifecycle.md#Clock-skew`):**
  * PMU counters step the instruction window across retired instructions (`step_window_instructions`).
  * `virtio-rng` backend swaps to a seeded ChaCha20 stream captured in snapshot manifests for byte-for-byte `getrandom()` determinism.
  * Kernel clocksource is locked via `clocksource=kvm-clock tsc=reliable nohpet`.
* **Code (`crates/kelir-plugins/plugin-replay/src/lib.rs`, `crates/kelir-drivers/`):**
  * `ReplayState::accrue_step_windows` ignores hardware counters and hardcodes a fixed window push: `pending.push(self.step_window)`.
  * Every single hardware driver backend (`driver-rustvmm`, `driver-hvf`, `driver-openvmm`, `driver-avf`) returns `Err(VmmError::Unsupported(...))` on:
    * `step_clock_window`
    * `register_pmu_sampler`
    * `set_entropy_backend`
    * `entropy_position`
  * Calling `ReplayPlugin::on_vm_init` crashes immediately on any real driver because the driver capabilities are completely unwritten.

---

### 7. FUSE Synthetic File Inode Collision & Read Chunking Bug

* **Spec (`cans/05-storage-plane.md#Synthetic-FUSE-inodes`, `cans/02-plugin-model.md#FsInodeAttr-carries-full-mode,-uid,-gid,-size,-content`):**
  * Injected configuration files (.env, .gitconfig) layer onto the host CoW broker at lookup.
  * Mode `0o400` is honored strictly with zero fallback; permissions cannot be widened.
* **Code (`crates/kelir-core/src/planes/storage/fuse.rs`, `synthetic.rs`):**
  * **Inode Identity Collision:** In `FuseDispatcher::dispatch` for `FuseOpcode::Lookup`, every synthetic file lookup returns `guest_inode: request.guest_inode`. It never allocates or maps virtual inodes. If multiple synthetic files are resolved, they all collide on the same caller-supplied inode number.
  * **Broken Chunked Reads:** In `FuseOpcode::Read`, the code evaluates `if request.data.is_empty() { content.len() }`. Incoming FUSE read requests allocate no buffer in `request.data`, meaning `request.data.is_empty()` is always `true`. The dispatcher returns the entire file content on every single chunk read regardless of offset or requested size.
  * **Arbitrary Mode Permissiveness:** `SyntheticFile::dynamic` masks with `file.mode & !0o6000`, allowing arbitrary permissions like `0o777` instead of enforcing strict `0o400`.

---

### 8. `kelir-shim` Anti-TOCTOU & Seccomp Trap Mirage

* **Spec (`cans/01-architecture.md#Axiomatic-foundations`, `cans/11-protocol.md#Trap-fast-path`):**
  * Minimal paravirtual micro-init (<50KB musl) traps monitored syscalls via in-kernel eBPF and seccomp-unotify.
  * Anti-TOCTOU: `cgroup v2` freezer pauses task groups on trap matches. `process_vm_readv` captures exact memory without thread attachment.
  * Executable identity BLAKE3 is computed from raw bytes at trap time before unfreeze.
* **Code (`crates/kelir-shim/src/trap.rs`, `introspect.rs`, `main.rs`):**
  * `SeccompFilter::install()` and `SeccompNotify::install()` are empty stubs returning `Ok(Self { fd: None })`. No seccomp filter is installed in the guest kernel.
  * In `main.rs`, `unotify.dispatch_pending()` immediately returns `Ok(None)` because `self.fd` is `None`. The event loop burns cycles doing zero syscall interception.
  * In `introspect.rs`, `read_argv` reads `/proc/{pid}/cmdline` and `read_envp` reads `/proc/{pid}/environ` directly from the procfs filesystem without freezing the target task via cgroups. This is the exact TOCTOU race condition the spec explicitly mandates preventing.

---

### 9. Checkpoint Plane DAG & CoW Compaction Fiction

* **Spec (`cans/08-lifecycle.md#Delta-chain-compaction`, `cans/09-snapshots.md#Restore-modes`):**
  * Checkpoint DAG supports unbounded depth.
  * `StepVerdict::Commit` advances delta DAG head in sparse extents.
  * `StepVerdict::Rollback` discards working delta in $\le$8ms.
  * Compaction flattens ancestor deltas in a single-pass block copy on export.
  * Restoration walks CAS manifest graphs.
* **Code (`crates/kelir-core/src/planes/checkpoint/mod.rs`):**
  * `CheckpointPlane::compact` unconditionally aborts:
    ```rust
    Err(PluginError::Halt("TODO (L3): CheckpointPlane::compact".into()))
    ```
  * `ManifestGraph` is a primitive in-memory `HashMap<u64, CheckpointNode>` with sequential integer IDs (`AtomicU64`), not content-addressed BLAKE3 digests or CAS graph walks.
  * `commit_step` simply inserts this node into RAM without flushing delta pages or talking to the broker upper backing file.
  * `rollback_step` removes the entry from the in-memory map without issuing hole-punching ioctls or reverting memory tracking slots.

---

### 10. Vault Secret Zeroization & Rollback Revocation Myth

* **Spec (`cans/plugins/vault.md#Memory-isolation-and-zeroization`, `cans/plugins/vault.md#Revocation`, `cans/08-lifecycle.md#Rollback`):**
  * Host keyring memory is pinned with `mlock(2)` and excluded from swap.
  * Issued token buffers zeroize on drop; TTL expiry revokes host-side.
  * **Hard Invariant:** `Session::rollback()` revokes all outstanding tokens for the session immediately.
* **Code (`crates/kelir-plugins/plugin-vault/src/service.rs`, `crates/kelir-core/src/session.rs`):**
  * `IssuedToken` stores secrets as raw, heap-allocated Rust `String`s with zero custom `Drop` zeroization and zero `mlock` pinning.
  * `CredentialBroker::issue` does not generate secrets at all: it mints `token: String::new()` (an empty string).
  * In `crates/kelir-core/src/session.rs#rollback_to`, the rollback sequence runs:
    ```rust
    self.wire.quiesce()?;
    if let Some(ingress) = self.engine.parked_connection_reset() { ... }
    self.storage.discard()?;
    self.driver.lock().await.rollback(snapshot)?;
    ```
    `plugin-vault` token revocation is **completely omitted** from the rollback pipeline. Issued credentials remain alive and valid across hypervisor rollbacks, violating the security contract.

### 11. Fake Typestate in `KelirBuilder`

* **Spec (`cans/03-sdk-architecture.md#Rust-SDK`):**
  * Affine typestate transitions enforce lifecycle correctness at compile time:
    $$\text{KelirBuilder}\langle\text{Unsealed}\rangle \longrightarrow \text{KelirBuilder}\langle\text{Sealed}\rangle \longrightarrow \text{Session}$$
* **Code (`crates/kelir/src/builder.rs`):**
  * `boot()` is implemented directly on `KelirBuilder<Unsealed, Plugins, Params>`:
    ```rust
    impl<Plugins: HList, Params: 'static> KelirBuilder<Unsealed, Plugins, Params> {
        pub async fn boot(self) -> Result<Session<Plugins, Params>, PluginError> {
            self.finish_boot().await
        }
    }
    ```
  * Callers can invoke `.boot()` directly on an `Unsealed` builder without ever calling `.seal()`. The typestate parameter `SealState` is decorative phantom data that enforces zero compile-time constraints.

---

### 12. `kelir-shim` Event Loop Self-Sabotage (Truncated Frame Loop)

* **Spec (`cans/11-protocol.md#Wire-framing`, `cans/11-protocol.md#Control-channel`):**
  * Paravirtual micro-init multiplexes Channel `0x0000` (Control) and Channel `0x0001` (Data) via 9-byte headers followed by message payloads.
* **Code (`crates/kelir-shim/src/main.rs`, `multiplexer.rs`, `codec.rs`):**
  * In `main.rs`, the polling loop decodes headers and dispatches empty slices:
    ```rust
    if let Err(_reason) = mux.dispatch_control(&[]) { ... }
    if let Err(_reason) = mux.dispatch_frame(stream_id, flags, &[]) { ... }
    ```
  * In `multiplexer.rs`, both `dispatch_control` and `dispatch_frame` call `codec::decode_msg_type(payload)`.
  * `codec::decode_msg_type` requires at least 2 bytes (`payload.len() < 2` returns `Err(())`).
  * Passing `&[]` guarantees that every single iteration of `kelir-shim`'s main loop errors out with `ShimError::TruncatedFrame`. The micro-init event loop fails its own wire protocol check on every cycle.

---

### 13. `ConsoleInterposer` Exec Stream Kind Missing in SDKs

* **Spec (`cans/02-plugin-model.md#KelirPlugin-trait`):**
  * `ConsoleInterposer` expands to include `on_exec_stdout(pid, exec_id, chunk)` and `on_exec_stderr(pid, exec_id, chunk)` for the `EXEC` stream kind.
* **Code (`packages/ts/core/src/plugin.ts`, `packages/py/kelir/plugin.py`):**
  * Rust interface (`crates/kelir-interface/src/plugin.rs`) has the hooks.
  * TypeScript's `PluginContract` has **zero** methods for `onExecStdout` or `onExecStderr`.
  * Python's `BasePlugin` and `HOOK_RINGS` omit `on_exec_stdout` and `on_exec_stderr`.
  * Neither TS nor Python SDK plugins can observe or intercept binary programmatic stdout/stderr execution streams.

---

### 14. Reconciler Path Traversal & Errno Inversion

* **Spec (`cans/05-storage-plane.md#Containment`):**
  * Strict containment: Symlink traversals escaping root fail with `EXDEV` at the host VFS layer.
* **Code (`crates/kelir-core/src/planes/storage/reconciler.rs`):**
  * **New File Bypass:** `Reconciler::contain` only runs path canonicalization if `target.exists()`:
    ```rust
    if target.exists() {
        let root = self.host_target.canonicalize()?;
        let resolved = target.canonicalize()?;
        if !resolved.starts_with(&root) { return Err(...); }
    }
    ```
    Any newly created or staged file that does not yet exist on the host filesystem skips resolution entirely, bypassing root escape checks.
  * **Wrong Errno:** When a symlink component is detected, it returns `PluginError::reject("EPERM", ...)` instead of the mandated `EXDEV`.

---

### 15. Inode Lock Broker Multi-Tenant State Wipe

* **Spec (`cans/05-storage-plane.md#Host-inode-lock-broker`):**
  * Cross-VM advisory file lock mediation maps guest virtual inodes to host physical inodes and scopes lock release per transaction/session.
* **Code (`crates/kelir-core/src/planes/storage/lock_broker.rs`):**
  * `release_transaction_locks()` clears the global lock state across **all** namespaces:
    ```rust
    async fn release_transaction_locks(&self) -> Result<(), PluginError> {
        let mut states = self.states.write()...;
        states.clear();
        Ok(())
    }
    ```
    Invoking release on one session wipes active file locks held by every other session on the host.
  * In `release_transaction_locks_for(namespace)`, entries are purged from `states`, but `self.cache` (`HostFdCache`) is never pruned, permanently leaking open host file descriptors.

---

### 16. Two-Phase Commit (2PC) Commit & Savepoint No-Ops

* **Spec (`cans/plugins/transaction.md#Multi-resource-2PC-coordination`, `#Temporal-savepoints`):**
  * `commit()` drives two-phase commit across broker upper files, db-proxy SQL connections, and lock brokers.
  * `rollback_to_savepoint()` restores intermediate upper/tombstone state and executes SQL `ROLLBACK TO SAVEPOINT`.
* **Code (`crates/kelir-plugins/plugin-transaction/src/service.rs`):**
  * `commit()` only validates active status and returns empty `Ok(())` without triggering the 2PC pipeline.
  * `rollback_to_savepoint()` takes the savepoint handle, executes `let _ = savepoint;`, and returns `Ok(())`.
  * In `db-backed-runtime.py`, transactions claim savepoint rollback resilience, but the underlying L2 service silently drops the call.

---

### 17. Phantom FFI Dispatch Queue & Rate Limiting

* **Spec (`cans/01-architecture.md#FFI-boundary-economics`, `cans/02-plugin-model.md#Foreign-plugin-proxy`):**
  * Rate limits evaluate in native memory before FFI queue insertion; queue capacity is bounded to 4,096 items to prevent foreign event loop thrashing.
* **Code (`crates/kelir-interface/src/plugin.rs`, `crates/kelir-core/src/dispatch.rs`):**
  * `FfiDispatchQueue` defines `try_submit()` and `complete()`.
  * In `crates/kelir-core/src/dispatch.rs`, `PipelineEngine::try_foreign_submit` is **never called anywhere** in any dispatch method (`dispatch_trap`, `dispatch_fs_write`, `dispatch_net_outbound`).
  * Events bypass the queue counter entirely. The 4,096 depth limit and `PluginError::FfiQueueSaturated` are unreferenced dead code.

---

### 18. Decorative Hardware Lockfile Attestation

* **Spec (`cans/15-operations.md#Developer-CLI`):**
  * `kelir doctor --enforce` writes an immutable `/etc/kelir/hw.lock`.
  * `verify_hw_lockfile()` attests that host hardware configuration matches the lockfile before boot.
* **Code (`crates/kelir-interface/src/driver.rs`, `crates/kelir-plugin-cli/src/doctor.rs`):**
  * `verify_hw_lockfile()` only checks if the file exists:
    ```rust
    match std::fs::read(HW_LOCKFILE_PATH) {
        Ok(_) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => ...
    ```
    An empty file created via `touch /etc/kelir/hw.lock` passes verification.
  * `write_hw_lockfile()` writes plain unauthenticated ASCII lines without setting immutable attributes (`chattr +i`) or computing cryptographic signatures.

---

### 19. Wire Plane IMDS Crash-on-Bind & Unbounded NAT Slab

* **Spec (`cans/07-wire-plane.md#Pre-policy-drop-zones`, `#UDP-socket-reaper`):**
  * Destination `169.254.169.254` intercepts at L3 and translates transparently to the vault vsock service.
  * UDP NAT slab enforces a strict 1,024-entry cap; oldest idle sockets reclaim on saturation.
* **Code (`crates/kelir-core/src/planes/wire/mod.rs`):**
  * In `route_imds_to_vault()`, if the vault sink **is** properly bound (`self.imds_sink_bound() == true`), it panics the caller:
    ```rust
    if self.imds_sink_bound() {
        return Err(PluginError::Halt("IMDS ... to vault vsock translation pending (L3)".into()));
    }
    ```
    Binding `plugin-vault` turns any cloud metadata fetch into an immediate hypervisor error.
  * `udp_nat_slab` is an unbounded `Vec` with zero cap checks and zero eviction logic.

---

### 20. Nonexistent Monotonic Clock Clamping & Power Event Sync

* **Spec (`cans/08-lifecycle.md#Clock-invariants`, `#Lid-sleep/wake-clock-snapping`):**
  * `CLOCK_MONOTONIC` clamps to $\max(\text{current}, \text{snapshot})$ across restore.
  * Host power event listeners (`IOPMrootDomain` on macOS, `systemd-logind` on Linux) dispatch clock resync frames over Channel `0x0000` on sleep/wake.
* **Code (`crates/kelir-core/src/planes/checkpoint/mod.rs`):**
  * `CheckpointPlane::restore()` performs a direct memory clone of the `SnapshotDescriptor` without querying host time or clamping guest monotonic counters.
  * Zero power event listeners or logind/IOPM bindings exist anywhere in the core or drivers. The sleep/wake resync mechanism is absent.
 
  * 
