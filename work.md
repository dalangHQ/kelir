Here is the direct, unvarnished audit of contradictions and misalignments between the `cans/` specifications and the `artifacts/` codebase.

---

### 1. Blatant Trait Signature & Code Contradictions

* **`TrapInterposer::on_trap` Return Type Mismatch:**
  * **Artifact (`plugin-trait.rs` line 292):** Declared as returning `Result<Verdict, PluginError>`.
  * **All Rust Examples (`git-gate.rs`, `policy-matrix.rs`, `autonomous-swe-agent.rs`):** Implement `on_trap(...) -> Result<Option<Verdict>, PluginError>` returning `Ok(None)` or `Ok(Some(...))`. 
  * *Result:* None of the example plugins will compile against the trait definition.

* **Truncated / Broken `MountScope` Struct in `plugin-trait.rs`:**
  * **Artifact (`plugin-trait.rs` lines 105–116):** The struct definition header is completely missing. It starts directly with raw fields:
    ```rust
        pub devices: Option<Vec<VirtioDevice>>,
        pub binaries: Option<Vec<String>>,
        pub paths: Option<Vec<PathBuf>>,
    }
    ```
    This is invalid Rust syntax inside a canonical artifact.

* **`VmConfig` Struct Definition vs. Example Constructors:**
  * **Artifact (`vmm-driver-trait.rs`):** Declares `kernel_path: PathBuf`, `rootfs_erofs_path: PathBuf`, `page_size_kb: u32`, `driver_policy: DriverPolicy`, `deterministic_clock: bool`, etc.
  * **Examples (`git-gate.rs`, `multi-vm-swarm.rs`, `hook-matrix.rs`, `policy-matrix.rs`, `autonomous-swe-agent.rs`):** Pass `kernel`, `rootfs_erofs`, and `driver: DriverChoice::Auto`, while completely omitting mandatory fields like `page_size_kb`.

* **Incompatible `StorageConfig` Definition Across Examples:**
  * **`git-gate.rs` / `autonomous-swe-agent.rs`:** Fields are `idmap`, `shared_caches`, `lock_broker`, `synthetic_files`, `mask_read`, `snoops`.
  * **`multi-vm-swarm.rs`:** Instantiates `StorageConfig` with `scope: LockScope::Globs(...)`, `default_mode: LockMode::Exclusive`, `non_blocking: true`. Both import `kelir::planes::storage::StorageConfig`.

* **`FsInterposer::on_fs_write` Signature Mismatch:**
  * **Artifact (`plugin-trait.rs`):** Synchronous function taking 4 arguments: `fn on_fs_write(&self, path, data, pid: u32, ctx: &HandlerCtx) -> Result<Verdict, PluginError>`.
  * **Spec (`02-plugin-model.md#Plane-semantics`):** "Storage plane fully attributed: on_fs_write carries PID and HandlerCtx".
  * **Artifact (`autonomous-swe-agent.py`):** Declared as asynchronous with only 2 arguments: `async def on_fs_write(self, path: Path, data: bytes) -> Verdict:`, dropping `pid` and `ctx`.

---

### 2. Type & Value Level ABI Contradictions

* **`ExecutionReceipt.binary_blake3` Type Incompatibility:**
  * **`execution-receipt.rs`:** `pub binary_blake3: String`.
  * **`messages.rs` (`TrapEvent`):** `pub binary_blake3: [u8; 32]`.
  * **`autonomous-swe-agent.rs`:** Compares them directly:
    ```rust
    if receipt.binary_blake3 != c.trap.binary_blake3
    ```
    Comparing `String` to `[u8; 32]` will not compile.

* **`Verdict::Patch` Payload Invariant Breakdown:**
  * **`messages.rs`:** Strictly specifies `Verdict::Patch(PatchPayload)` where `PatchPayload` is an enum (`ExecveMutation(Vec<u8>)`, `Custom{..}`, etc.).
  * **`autonomous-swe-agent.rs`:** Passes `Verdict::Patch(Vec<u8>)`.
  * **`hook-matrix.rs`:** Passes `Verdict::Patch(FsInodeAttr)`.
  * **`policy-matrix.rs`:** Passes `Verdict::Patch(serde_json::Value)`.

* **`SuspendUntil` Enum vs. Standalone Types:**
  * **`messages.rs`:** `enum SuspendUntil { Deadline(u64), Approval, Handshake }`.
  * **`policy-matrix.rs` / `hook-matrix.rs`:** Uses `Approval` and `Deadline(500)` as standalone types / constructors imported directly (`Verdict::Suspend { until: Approval }`, `Deadline(0)`).

---

### 3. Protocol & Framing Violations

* **Illegal Reserved Stream Kind Registration in Policy Matrix Examples:**
  * **Spec (`11-protocol.md#Control-channel`):** Stream kinds `0x0001` through `0x000F` are **frozen core reserved**. Kind `0x0002` is `PTY_CONTROL`. Any custom re-registration aborts with `EKindReserved`. Plugin kinds start at `0x0010` / `0x0013`.
  * **Artifacts (`policy-matrix.rs`, `policy-matrix.py`, `policy-matrix.ts`):** All register a custom RPC service on stream kind `2`:
    ```rust
    builder.register_vsock_service(2, ...)
    ```
    This directly violates the protocol reservation gate.

* **Wire Framing Header & Flag Contradictions:**
  * **Spec (`11-protocol.md#Wire-framing`):**
    * Asserts: "Flags registry: 0x0001 FLAG_CONTINUATION; 0x0002-0xFFFF must be zero on send".
    * Asserts: "Layout (header offsets, magic, version): see: artifacts/code/wire-protocol.rs".
  * **Artifact (`wire-protocol.rs`):**
    * Explicitly states: "The header on the wire is 9 bytes ... with no magic, version, channel, or sequence field."
    * Flags are `u8`, where `0x01` is `FLAG_OPEN`, not `FLAG_CONTINUATION`:
      ```rust
      pub const FLAG_OPEN: u8 = 0x01;
      pub const FLAG_DATA: u8 = 0x02;
      ```

* **Signal Forwarding Targeting (`pgid` vs `exec_id`):**
  * **Spec (`11-protocol.md#Binary-exec-channel`) & Docstring in `plugin-trait.rs`:** "Signal targeting: Channel 0x0000 signals carry exec_id, never raw guest pid".
  * **Artifact (`plugin-trait.rs` line 25):** 
    ```rust
    pub enum PtyControlMessage {
        SignalForward { signal: u32, pgid: u32 },
    }
    ```
    Carries raw guest `pgid`, not `exec_id`.

---

### 4. API Reversals & Spec Misalignments

* **Port Forwarding Argument Order Inversion:**
  * **Spec (`07-wire-plane.md#Port-forwarding` and `03-sdk-architecture.md`):** Explicitly specifies:
    ```rust
    session.wire.forward_port(guest_port, host_port) -> Result<u16, WireError>
    ```
  * **Artifacts (`autonomous-swe-agent.rs`, `autonomous-swe-agent.py`):** Call:
    ```rust
    session.wire.forward_port(8080, 3000)
    ```
    Where `8080` is the host port and `3000` is the guest port (confirmed by `autonomous-swe-agent.ts`: `{ host: 8080, guest: 3000 }`). The argument order is inverted.

* **String Event Names in `policy-matrix.ts`:**
  * **Spec/Diagram (`sdks-invariants.txt`):** "Typed enum compilation: ... string event names are not accepted dispatch keys in any language."
  * **Artifact (`policy-matrix.ts`):** 
    ```typescript
    lifecycle.registerVeto("suspend", () => policyState.ready);
    lifecycle.registerVeto("clock-skew", () => policyState.ready);
    ```
    Passes string literals instead of typed enum members.

* **Non-existent `ExitKind` Variants in Lifecycle:**
  * **Spec (`08-lifecycle.md#Pre-teardown-flush`):**
    * "Timeout expiry records ExitReason::DirtyShutdown in ExecutionReceipt"
    * "session.kill() ... records ExitReason::HardKilled in ExecutionReceipt"
  * **Artifact (`execution-receipt.rs`) & Spec (`10-attestation.md`):** Enum is named `ExitKind` (not `ExitReason`), containing strictly:
    `Normal | Signal(u32) | KernelPanic(String) | HypervisorFault(String)`
    `DirtyShutdown` and `HardKilled` do not exist.

---

### 5. False Plugin Coverage & Matrix Claims

* **Missing `container` Plugin Implementation:**
  * **Spec (`02-plugin-model.md#First-class-plugins`):** Lists `multi-vm-swarm.rs` as the example demonstrating `mesh, container`.
  * **Artifact (`multi-vm-swarm.rs`):** Mounts `MeshPlugin` and `SupplyChainPlugin`. `container` / `ContainerPlugin` is never imported, mounted, or used.

* **Missing `transaction` Plugin Implementation:**
  * **Spec (`02-plugin-model.md#First-class-plugins`):** Lists `db-backed-runtime.py` as the example demonstrating `transaction`.
  * **Artifact (`db-backed-runtime.py`):** Mounts `DbProxyPlugin` and registers raw lifecycle hooks on the builder (`@builder.on_pre_commit`). `TransactionPlugin` (`plugin-transaction`) is never mounted.

* **Missing `vault` Plugin Implementation in SWE-Agent Family:**
  * **Spec (`02-plugin-model.md#First-class-plugins`):** Lists `autonomous-swe-agent.{rs,ts,py}` as the demonstration for `chaos, ledger, vault`.
  * **Artifacts (`autonomous-swe-agent.{rs,ts,py}`):** None of them mount `plugin-vault` (`VaultPlugin`). They only configure the native wire plane's `vaults` array.

---

### 6. Architectural Ghost Files

* **Deleted `Oracle` Trait Still Present in Project Tree:**
  * **Artifacts (`autonomous-swe-agent.rs`, `.ts`, `.py`):** Explicitly document: *"Example-local evaluator, not a core trait: 03/08 deleted the core Oracle contract in aa0e011. UserOracle is a plain struct here."*
  * **Artifact (`project-structure.txt`):** Still claims `crates/kelir-interface/src/oracle.rs # Oracle evaluator trait` exists.

* **Ghost Drivers in `project-structure.txt`:**
  * **Spec (`01-architecture.md`, `15-operations.md`):** Cites `driver-avf` and `driver-proot` as first-class driver crates.
  * **Artifact (`project-structure.txt`):** Omits `driver-avf` and `driver-proot` completely from `crates/kelir-drivers/`.

* **Corrupted Docstring in `plugin-trait.rs`:**
  * **Artifact (`plugin-trait.rs` lines 302–306):** The docstring for the deleted `on_disclosure` hook was left stranded immediately preceding `on_console_input`:
    ```rust
    /// Mandatory core-originated disclosure (HostToGuest::ChannelData on the pty-disclosure kind)...
    /// Raw paravirtual character stream input. Intercepts host -> guest port bytes.
    async fn on_console_input(...)
    ```

### 7. The Vault IMDS Routing Paradox

* **Spec (`07-wire-plane.md#Userspace-network-stack`) & Diagram (`wire-plane.txt`):**
  Unconditionally drops link-local and cloud metadata addresses before policy evaluation:
  > *"Link-local quarantine: 169.254.0.0/16 and fe80::/10 dropped before policy eval; cloud metadata IMDS endpoints dropped unconditionally at the L3 ingress ring."*
* **Plugin Spec (`plugins/vault.md#Invariants-and-policy`):**
  Defines the primary guest metadata interface at that exact IP:
  > *"Guest-visible endpoint: virtio-vsock 169.254.169.254 equivalents: `http://169.254.169.254/kelir/credentials/<name>` (HTTP over vsock)"*
* **Architecture Spec (`01-architecture.md#Axiomatic-foundations`):**
  Forbids guest network redirection:
  > *"Zero workload intrusion: ... Workload environment variables, libraries, and syscall paths remain untouched ... Zero guest-side proxy environment variables; zero guest awareness."*
* **The Contradiction:**
  An unmodified guest issuing a standard HTTP request to `http://169.254.169.254` outputs an IPv4 packet routed to the virtual Ethernet interface (`virtio-net`). That packet enters the L3 ingress ring, where the wire plane drops `169.254.0.0/16` before any policy engine or proxy can see it. It cannot be both unconditionally dropped at L3 and served to uninstrumented guests without routing modification.

---

### 8. Lifecycle Step Loop & Post-Hook Contract Divergence

* **Spec (`08-lifecycle.md#Autonomous-step-verification-loop`):**
  Mandates a dedicated hypervisor step lattice returning `StepVerdict`:
  > *"Step lattice: `StepVerdict::Commit`: flushes staging delta, advances active step slot; `StepVerdict::Rollback(reason)`: calls session.rollback(); `StepVerdict::YieldToHost`: halts vCPU registers awaiting external input."*
* **Artifact Examples (`autonomous-swe-agent.{rs,ts,py}`, `serverless-webhook.ts`):**
  None of the examples implement or return `StepVerdict`. Instead, all post-execution trap hooks return `PostExecAction`:
  ```rust
  PostExecAction::Proceed | PostExecAction::RollbackAndHalt
  ```
  `StepVerdict::Commit` and `StepVerdict::YieldToHost` are completely missing from the SDK interfaces and trap examples.

---

### 9. Binary Inspection: Raw Hashing vs. ELF Parsing Contradiction

* **Attestation Spec (`10-attestation.md#binary_blake3-identity`):**
  Explicitly forbids ELF parsing for binary verification:
  > *"No ELF parsing; bytes hashed as kernel opens them ... openat2(RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS)"*
* **Protocol Spec (`11-protocol.md#Measurement-rule`):**
  Explicitly requires ELF parsing at the trap boundary:
  > *"Pre-exec inspection: ELF is read from disk at trap time, not from /proc/$PID/maps ... Scans /proc/$PID/auxv, envp, and PT_INTERP before cgroup unfreeze."*
* **The Contradiction:**
  `10-attestation.md` defines binary attestation as pure streaming byte hashes without loader inspection to guarantee O(1) trap performance. `11-protocol.md` mandates reading ELF headers, parsing the `PT_INTERP` segment, and inspecting dynamic linker auxiliary vectors before unfreezing the cgroup.

---

### 10. Reconciler Implementation Deficits vs. Normative Storage Spec

* **Storage Spec (`05-storage-plane.md#Workspace-commit`):**
  Specifies atomic leaf-level platform swaps, lock broker verification, and fallback handling:
  * *"Per-entry atomic commit at leaf level: temp file + platform rename (Linux: `renameat2`; macOS: `renameatx_np(RENAME_SWAP)`; Windows: `SetFileInformationByHandle(FileRenameInfoEx)`)"*
  * *"Host Inode Broker pre-checks active host flock descriptors"*
  * *"Fallback on EBUSY or EPERM: in-place write with 2000ms backoff"*
* **Artifact Code (`reconciler.rs`):**
  * Uses standard `fs::rename(&tmp_swap, &target_path)`, bypassing `renameat2` exchange flags and macOS swap primitives.
  * Contains no host `flock` or lock broker checks prior to unlinking or overwriting targets.
  * Contains no retry loop or 2000ms backoff; any `EBUSY` or `EPERM` immediately aborts the commit and bubbles up a raw `std::io::Error`.

### 11. Phantom Abstraction: `StagingGuard` in `reconciler.rs`

* **Artifact Code (`reconciler.rs` lines 41–56):**
  Defines an RAII cleanup guard to guarantee atomic directory reconciliation:
  ```rust
  pub struct StagingGuard {
      pub staged: Vec<PathBuf>,
      pub disarmed: bool,
  }
  impl Drop for StagingGuard {
      fn drop(&mut self) {
          if !self.disarmed {
              for path in &self.staged { let _ = fs::remove_file(path); }
          }
      }
  }
  ```
  The header comments cite `StagingGuard` as the core mechanism of Phase 3 (Stage).
* **The Reality in `reconcile_upperdir` & `reconcile_diff` (`reconciler.rs` lines 68–144):**
  `StagingGuard` is **never instantiated, populated, or referenced anywhere in the executable functions**. Temp files (`.tmp.<nonce>`) are created directly via `fs::copy` and moved via `fs::rename`. If `reconcile_diff` fails midway (e.g., on permission error, disk full, or traversal detection), no guard triggers, leaving stale `.tmp.<nonce>` files stranded on the host filesystem.

---

### 12. Clock Skew Parity Drift Across SDK Examples (1 Day vs. 24 Days)

* **Spec (`03-sdk-architecture.md#Cross-SDK-guarantees` & `15-operations.md`):**
  Mandates identical cross-language behavioral parity and receipt verification:
  > *"Parity evaluated on identical wire frame inputs and outputs using vectors.json ... Identical vectors.json outcomes."*
* **Rust Example (`autonomous-swe-agent.rs` line 542):**
  Skews the forked child VM clock in **seconds** (1 day per branch index):
  ```rust
  child.vm.clock.skew(Duration::from_secs(24 * 3600 * (id as u64 - 64))).await?;
  ```
* **TypeScript & Python Examples (`autonomous-swe-agent.ts` line 236, `autonomous-swe-agent.py` line 448):**
  Skew the forked child VM clock in **days** (24 days per branch index):
  * TS: `await child.vm.clock.skew({ days: 24 * (id.charCodeAt(0) - 64) });`
  * Python: `await child.vm.clock.skew(timedelta(days=24 * (ord(hypothesis_id) - ord("A") + 1)))`
* **The Contradiction:**
  Branch `A` skews by 24 hours in Rust, but skews by 576 hours (24 days) in TypeScript and Python. These runs cannot produce matching receipts, JWT validity states, or attestation deltas.

---

### 13. Custom Action Wire Contract Contradiction (`PatchPayload::Custom`)

* **Spec (`11-protocol.md#Custom-dispatch-contract`):**
  Explicitly asserts that the action name is stripped and resolved host-side:
  > *"Wire Custom carries the CBOR payload only; the name resolves at SDK registration. Unknown name at trap time fails with EUnknownAction."*
* **Artifact Code (`messages.rs` line 161):**
  Carries the action name directly on the wire payload:
  ```rust
  pub enum PatchPayload {
      ...
      Custom { name: String, cbor: ciborium::value::Value },
  }
  ```
* **Artifact Examples (`policy-matrix.{rs,py,ts}`):**
  All three SDK examples serialize `name: "quarantine"` into the payload payload struct instead of transmitting raw CBOR bytes.

---

### 14. Incompatible Host Storage Schema Definitions (`SessionRow`)

* **Spec (`06-storage-catalog.md#Entity-model`) & Code (`vmm-driver-trait.rs` lines 122–131):**
  Defines the catalog row as a strictly minimal 8-column layout:
  ```rust
  pub struct SessionRow {
      pub session_id: String,
      pub status: String,
      pub incarnation_id: String,
      pub boot_id: String,
      pub start_time_ns: u64,
      pub pid: u32,
      pub lease_kind: LeaseKind,
      pub expires_at_unix: Option<u64>,
  }
  ```
* **Architecture Diagram (`storage-architecture.txt` Section 10):**
  Injects phantom fields that do not exist in the code struct or normative catalog spec:
  > *"sessions: EVERY session registers a row (session_id, status, pids, rootfs_hash, expires_at, lease_kind, incarnation_id, boot_id, start_time_ns, pid)."*
  `pids` and `rootfs_hash` do not exist in `SessionRow` or `06-storage-catalog.md`.

---

### 15. Broken Invariants in `replay.rs`

* **Plugin Spec (`plugins/replay.md#Invariants-and-policy`):**
  Mandates absolute network isolation and monotonic clock locking:
  > *"Replay mode requires network isolation for determinism ... Replay profiles declare network: off; wire plane drops all egress. I/O determinism draws from seeded inputs, not from live sockets."*
* **Artifact Example (`replay.rs` lines 40–56):**
  * Sets `deterministic_clock: false`:
    ```rust
    deterministic_clock: false,
    pinned_clocksource: Some("kvm-clock".into()),
    ns_per_instruction: Some(1),
    ```
  * Never declares `network: off`, never mounts a sinkhole, and leaves userspace networking wide open.
  The canonical example demonstrating deterministic replay explicitly disables deterministic clocks and fails to configure network isolation.

### 16. Direct Violation of Hook Context Restrictions (`&Session` in Lifecycle Hooks)

* **Spec (`02-plugin-model.md#Hook-context-restriction`):**
  Lays down a strict safety boundary forbidding lifecycle hooks from accessing session handles or hypervisor drivers:
  > *"Trap and lifecycle hooks receive HandlerCtx or a restricted lifecycle context. No hook receives &mut Box<dyn VmmDriver> or &Session. LifecycleCtx exposes: set_watchdog, adjust_balloon, register_veto."*
* **Artifact Examples (`autonomous-swe-agent.rs` lines 496–513, `autonomous-swe-agent.py` lines 417–432):**
  Directly pass the raw hypervisor driver and full `&Session` handles into lifecycle closures:
  ```rust
  .on_vm_init(|driver| async move { driver.set_watchdog(...).await })
  .on_vm_ready(|session: &Session| async move { session.exec_in_guest(...).await })
  .on_vm_shutdown(|session: &Session| async move { session.ctx::<LedgerCtx>().flush().await })
  .on_trap_deny(|event, session: &Session| async move { ... session.console().await?; })
  ```
* **The Contradiction:**
  The normative specification bans exposing `&Session` and `VmmDriver` to hooks. The primary reference implementations pass them directly as first-class callback arguments.

---

### 17. Unhandled `NameError` Crash in `db-backed-runtime.py`

* **Artifact Code (`db-backed-runtime.py` line 108):**
  Registers a binary migration trap filter:
  ```python
  builder.trap(TrapFilter.binary("migrate"), pre=pre_migrate, post=post_migrate)
  ```
* **The Defect (`db-backed-runtime.py` lines 29–38):**
  Top-level imports are strictly:
  ```python
  import kelir
  from kelir import HandlerContext, KelirBuilder, PostExecAction, Session, Verdict
  from kelir.plugins.db_proxy import (
      DbProxyConfig, DbProxyPlugin, EphemeralCredentials,
      PgWireRewrite, SqliteOverVsock,
  )
  ```
  `TrapFilter` is **never imported**. Running this file crashes immediately at runtime with `NameError: name 'TrapFilter' is not defined`.

---

### 18. Three-Way Architectural Contradiction on MCP Device Attachment

Three separate normative spec files assign the Model Context Protocol adapter to three mutually incompatible hardware devices:
1. **`12-adapters.md#MCP-adapter`:** Assigns MCP to the **terminal plane**:
   > *"Binds to dynamic console port 'kelir-mcp' without interfering with Port 0 PTY."*
2. **`plugins/mcp.md#Invariants-and-policy` & `11-protocol.md#Control-channel`:** Assigns MCP to the **vsock control plane**:
   > *"Registers stream kind MCP_CONSOLE on the mux: Kind 0x0012 ... Guest shim exposes /run/kelir/mcp.sock to the guest MCP server."*
3. **`plugins/tool-guard.md#Invariants-and-policy`:** Reverts MCP back to a **character console port**:
   > *"Interposition is complete ... executes upstream of plugin-mcp on the MCP console port."*

Virtio-console dynamic ports, virtio-vsock stream kinds (`0x0012`), and in-guest Unix domain sockets (`/run/kelir/mcp.sock`) are distinct transport mechanisms. The spec simultaneously claims all three mediate the same MCP traffic.

---

### 19. Undefined `VmmError` in `vmm-driver-trait.rs`

* **Artifact Code (`vmm-driver-trait.rs` lines 181–241):**
  Every single method on the universal `VmmDriver` trait returns `Result<..., VmmError>`:
  ```rust
  async fn boot(&mut self, config: VmConfig) -> Result<(), VmmError>;
  async fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError>;
  async fn checkpoint(&mut self, label: &str) -> Result<SnapshotDescriptor, VmmError>;
  async fn map_shared_region(...) -> Result<SharedMemoryHandle, VmmError>;
  async fn rollback(&mut self, snapshot: &SnapshotDescriptor) -> Result<(), VmmError>;
  async fn suspend(...) -> Result<(), VmmError>;
  async fn resume(...) -> Result<(), VmmError>;
  async fn terminate(&mut self) -> Result<(), VmmError>;
  ```
* **The Defect:**
  `VmmError` is **never defined, declared, or imported anywhere** in `vmm-driver-trait.rs`. The canonical trait definition is broken and cannot compile as an independent artifact.

---

### 20. Phantom API: `session.console()` Across All SWE-Agent Examples

* **Spec (`03-sdk-architecture.md#Runtime-surface` & `08-lifecycle.md#Break-glass`):**
  Defines the native plane surfaces on `Session`:
  > *"Core Session directly exposes the 4 planes: session.terminal, session.storage, session.wire, session.checkpoint ... PTY console master (Port 0) is the sole in-process break-glass surface ... Programmatic tooling uses 0x0003 exclusively; PTY bypassed."*
* **Artifact Examples (`autonomous-swe-agent.rs` lines 509, 532; `autonomous-swe-agent.ts` lines 219, 230; `autonomous-swe-agent.py` lines 431, 442):**
  Every example repeatedly invokes:
  ```rust
  await session.console();
  ```
  `console()` is not a method on `Session`, not a method on `session.terminal`, and does not exist in any driver trait, SDK spec, or interface file.

