# RFC: Evolving Kelir into the Definitive Hypervisor for Deterministic Agentic Operating Systems

* **Document:** `kelir-ask.md`
* **Target:** `kelir-core`, `kelir-drivers`, `kelir-plugins`
* **Context:** Host-side Deterministic Orchestrators (e.g., Capcli, OpenShell) wrapping Kelir as their physical execution substrate.

---

## 1. The Context: Why This RFC Exists

### The Emerging Stack
Autonomous software engineering agents (Claude Code, Cursor, Devin, custom LLM loops) are fundamentally untrusted POSIX workloads. They execute arbitrary bash scripts, compile code, spawn sub-processes, query databases, and hit external APIs.

To safely govern these agents, the industry is converging on a two-tier architecture:
1. **The Semantic / Governance Layer (Host Orchestrator, e.g., Capcli):** Manages policy files, AST query validation, integer compute budgets ("fuel"), Optimistic Concurrency Control (OCC) state fences, cryptographic trust rungs, and append-only causal DAG audit ledgers.
2. **The Physical Isolation Layer (Hypervisor Middleware, e.g., Kelir):** Boots sub-60ms Linux microVMs, manages hardware virtualization (KVM/HVF), intercepts syscalls, and mediates the 4 VirtIO rings (vsock, console, fs, net).

```
┌────────────────────────────────────────────────────────────────────────┐
│ HOST ORCHESTRATOR (e.g., Capcli / OpenShell)                           │
│ • SQLite SSOT, C-Authorizer, AST Scanner, Fuel Math, OCC State Fences   │A
│ • Trust Rungs, Merkle-DAG Audit Spine, Human Approval Cards (Cockpit)  │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │ Host-Side Rust/NAPI C-FFI
┌───────────────────────────────────▼────────────────────────────────────┐
│ KELIR: THE PHYSICAL MICROKERNEL (In-Process Hypervisor)                │
│ • 4-Ring VirtIO Bus (vsock, console, fs, net)                          │
│ • Hardware traps, memory CoW, FUSE overlays, wire proxy, PTY clamping   │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │ Hardware Isolation (KVM/HVF)
┌───────────────────────────────────▼────────────────────────────────────┐
│ GUEST (Opaque Agent Workload)                                          │
│ • Claude Code, Cursor Agent, Bash, Python, npm, git                    │
│ • Zero harness awareness, standard POSIX, untrusted execution         │
└────────────────────────────────────────────────────────────────────────┘
```

### The Problem: Why Current Kelir Falls Short
Kelir was originally designed as an **interactive terminal/developer sandbox**. When an orchestrator attempts to wrap Kelir as a **deterministic, enterprise-grade state machine substrate**, it hits fundamental gaps:
* **Non-deterministic telemetry:** Wall-clock nanoseconds and page dirty bytes vary across host CPUs.
* **Database blindspot:** SQLite's prepare-time C-authorizer cannot inspect in-guest queries, while PostgreSQL network traffic bypasses AST governance.
* **Crude preemption:** Kelir hard-kills VMs (`SIGKILL`) instead of providing non-blocking checkpoints for human approval.
* **Token blindness:** Kelir's Token Shield measures raw bytes and lines, allowing minified JSON to blow LLM context windows with tens of thousands of tokens.

### The Scope Discipline: Preventing Kelir Over-Engineering
**Kelir must remain a stripped, radical microkernel.** 

To prevent feature bloat, every ask in this RFC strictly adheres to one axiom:
> **Mechanism belongs in Kelir. Policy belongs in the Host Orchestrator.**

Kelir **must not** know about Capcli's fuel currencies, AST rules, trust ladders, or database schemas. Kelir only provides the **generic trap, register, VirtIO descriptor, and byte stream**. The host orchestrator provides the brain.

---

## 2. Master Implementation Matrix

```
┌────────────────────────────────────────────────────────┐
│ TIER A: KELIR CORE / HYPERVISOR ENGINE                 │
│ Hardware invariants, VirtIO bus, VMM drivers, Shim     │
├────────────────────────────────────────────────────────┤
│ TIER B: EXISTING PLUGIN ENHANCEMENTS                   │
│ Targeted upgrades to wire, vfs, pty, and ledger        │
├────────────────────────────────────────────────────────┤
│ TIER C: DECOUPLED STANDALONE PLUGINS                   │
│ New composable crates in `kelir-plugins/`              │
└────────────────────────────────────────────────────────┘
```

---

## 3. Tier A: Core Hypervisor & VMM Primitives (14 Must-Dos)

### Ask 1: Deterministic vCPU PMU Retired Instruction Counter
* **The Case & Why:** Host orchestrators enforce strict execution quotas (e.g., Capcli's integer "fuel"). Current `ExecutionReceipt` tracks `wall_clock_ns` and `dirty_bytes` (`08-epistemic.md`). Wall clock fluctuates based on host load; dirty bytes differ by page size (4KB on x86 vs 16KB on Apple Silicon). Two runs of the exact same code yield different metrics, breaking deterministic replay.
* **In-Scope for Kelir:** Query the CPU Performance Monitoring Unit (PMU) via KVM perf events or HVF counters. Expose `retired_instructions: u64` in `ExecutionReceipt`.
* **Out-of-Scope (Host's Job):** Converting instructions to "fuel" or business cost metrics.
* **Proposed Signature:**
  ```rust
  pub struct ExecutionReceipt {
      // ... existing fields ...
      pub retired_instructions: u64, // Hardware-exact instruction count
  }
  ```

### Ask 2: Zero-Copy Shared Memory Channel (`virtio-shm`)
* **The Case & Why:** Orchestrators passing tabular data (e.g. 10,000 database rows) or large blobs between host and guest experience massive latency over FUSE or serialized vsock CBOR. Capcli requires sub-15ms fast-path execution.
* **In-Scope for Kelir:** Implement `virtio-shm` or map an anonymous host `memfd_create` segment into the guest’s physical address space pre-exec, unmapping it on process exit.
* **Out-of-Scope (Host's Job):** Schema serialization, column parsing, or row validation.
* **Proposed Signature:**
  ```rust
  impl Session {
      pub fn map_shared_region(&mut self, tag: &str, size_bytes: usize) -> Result<SharedMemoryHandle, VmmError>;
  }
  ```

### Ask 3: Non-Blocking Yield Checkpoint (`TrapAction::Yield`)
* **The Case & Why:** When an agent invokes a long operation (>15ms) or triggers a human approval card, the host orchestrator must checkpoint the task, release database transaction locks, and free the thread. Kelir’s `SuspendForApproval` polls synchronously (`poll_interval_ms`), locking up host threads (`09-protocol.md`).
* **In-Scope for Kelir:** Add `TrapAction::Yield { checkpoint_id: u64 }`. When returned, the hypervisor immediately captures vCPU registers, freezes guest dirty pages via CoW, and returns control cleanly to the host event loop.
* **Out-of-Scope (Host's Job):** Managing the task queue, OCC state fences, or resuming the task.
* **Proposed Signature:**
  ```rust
  pub enum TrapAction {
      // ... existing variants ...
      Yield { checkpoint_id: u64 },
  }
  ```

### Ask 4: Hypervisor-Level Immediate Abort (`TrapAction::Abort`)
* **The Case & Why:** When a host authorizer catches an unrecoverable security breach (e.g., unauthorized DDL or tenant traversal), execution must stop instantly. Kelir’s `TrapAction::Deny` only injects an exit code and fake stderr into the guest TTY (`09-protocol.md`). The guest process continues running and can catch signals.
* **In-Scope for Kelir:** Add `TrapAction::Abort { exit_code: i32, reason: String }` that dispatches an uncatchable hypervisor-level `SIGKILL` directly to the guest PID via the shim.
* **Out-of-Scope (Host's Job):** Evaluating the policy that triggered the denial.
* **Proposed Signature:**
  ```rust
  pub enum TrapAction {
      // ... existing variants ...
      Abort { exit_code: i32, reason: String },
  }
  ```

### Ask 5: Two-Phase Commit (2PC) Lifecycle Synchronization Hooks
* **The Case & Why:** If an agent script crashes or hits a policy ceiling mid-flight, Kelir reverts guest RAM and tmpfs diffs in `<8ms` (`07-lifecycle.md#Rollback`). But if host SQLite writes already committed, state becomes desynchronized.
* **In-Scope for Kelir:** Add a 2PC hook pair to `LifecycleCtx`:
  1. `on_pre_commit()`: Freezes the microVM, flushes pending VFS buffers, and waits for a host verdict.
  2. `on_post_commit(success: bool)`: If true, commits tmpfs upperdir to host workspace; if false, wipes tmpfs upperdir instantly.
* **Out-of-Scope (Host's Job):** Arbitrating host database transactions or rollback cascades.
* **Proposed Signature:**
  ```rust
  pub trait LifecycleHook {
      async fn on_pre_commit(&self, ctx: &mut LifecycleCtx) -> Result<(), PluginError>;
      async fn on_post_commit(&self, ctx: &mut LifecycleCtx, committed: bool) -> Result<(), PluginError>;
  }
  ```

### Ask 6: Portable Headless Snapshot Bundles (`.kelir-snap`)
* **The Case & Why:** Interactive human approvals can take hours. Host daemons reboot, and spot cloud instances get preempted. Kelir’s memory dumps are currently tied to local NVMe and an active host process (`07-lifecycle.md#Suspend-and-resume`).
* **In-Scope for Kelir:** Expose an export/import primitive: serialize frozen registers + zstd-compressed dirty memory diffs into an opaque binary stream (`.kelir-snap`), and hydrate a new session from that stream on any machine with identical vCPU architecture.
* **Out-of-Scope (Host's Job):** Uploading to S3, encryption-at-rest, or cluster scheduling.
* **Proposed Signature:**
  ```rust
  impl Session {
      pub async fn export_snapshot(&self) -> Result<Box<dyn AsyncRead + Unpin>, VmmError>;
      pub async fn import_snapshot(driver: Box<dyn VmmDriver>, reader: Box<dyn AsyncRead + Unpin>) -> Result<Session, VmmError>;
  }
  ```

### Ask 7: Synthetic Virtual Clocksource (Instruction-Ticked Time)
* **The Case & Why:** Capcli mandates 100% deterministic replay. Kelir’s `clock_skew` offsets wall time, but the underlying CPU clock (`CNTVCT_EL0` / TSC) still ticks in real time while code executes (`07-lifecycle.md#Clock-skew`). Repeated runs observe different clock values.
* **In-Scope for Kelir:** Add a driver mode where the guest virtual clocksource decouples entirely from host TSC and only advances by fixed nanoseconds per retired instruction block or explicit tick injection.
* **Out-of-Scope (Host's Job):** Providing logical business timestamps (`ctx.now()`).
* **Proposed Signature:**
  ```rust
  pub struct VmConfig {
      // ... existing fields ...
      pub deterministic_clock: bool,
      pub ns_per_instruction: Option<u32>,
  }
  ```

### Ask 8: Graceful Host SIGTERM Preemption Window
* **The Case & Why:** On Kubernetes/cloud runner shutdown, nodes receive `SIGTERM` with a 30s grace period. Kelir’s crash reaper currently triggers an immediate `SIGKILL` via `PR_SET_PDEATHSIG` or kqueue (`07-lifecycle.md#Host-crash-reaper`), destroying all un-checkpointed memory.
* **In-Scope for Kelir:** Catch host `SIGTERM`, hold `SIGKILL` for a configurable window (e.g. 2000ms), and dispatch an emergency `on_host_preempt` hook so host orchestrators can checkpoint state.
* **Out-of-Scope (Host's Job):** Persisting application state during the grace window.
* **Proposed Signature:**
  ```rust
  pub struct VmConfig {
      pub preemption_grace_ms: u64, // e.g. 2000
  }
  ```

### Ask 9: Pluggable Host vsock Listener Port Registration
* **The Case & Why:** Guest scripts running under an orchestrator need direct, high-speed RPC to host services (database proxies, audit notaries) without treating it as outbound external network traffic. Ports 5252 and 5253 are hardcoded (`09-protocol.md`).
* **In-Scope for Kelir:** Allow host builders to register custom vsock listener ports (e.g. port 5254) mapped to host async stream callbacks (`AsyncRead + AsyncWrite`).
* **Out-of-Scope (Host's Job):** Defining the protocol running over the custom port.
* **Proposed Signature:**
  ```rust
  impl SessionBuilder {
      pub fn register_vsock_service<F>(&mut self, port: u32, handler: F) -> &mut Self 
      where F: Fn(VsockStream) + Send + Sync + 'static;
  }
  ```

### Ask 10: Intra-Process User-Space Thread Tagging
* **The Case & Why:** Multiple asynchronous subagents sharing a single Node.js or Python PID can bleed claims and tokens. Kelir binds identity strictly to guest OS PID/TID (`02-plugin-model.md`).
* **In-Scope for Kelir:** Support an ambient 64-bit task token set by the guest shim (via thread-local storage or seccomp cookie) that attaches to all VirtIO ring descriptor headers.
* **Out-of-Scope (Host's Job):** Generating or validating task tokens.
* **Proposed Signature:**
  ```rust
  pub struct TrapEvent {
      pub pid: u32,
      pub task_token: Option<u64>, // Ambient thread-level context
      // ... existing fields ...
  }
  ```

### Ask 11: Structured Diagnostic Payload on Watchdog Expiry
* **The Case & Why:** When a 300s compute watchdog fires, the orchestrator must return a structured diagnostic JSON envelope. Kelir’s `virtio-wdt` only supports raw `Reboot` or `Terminate` (`07-lifecycle.md#Watchdog`), dropping the socket without explanation.
* **In-Scope for Kelir:** Before tearing down the VM, allow `WatchdogAction::Terminate` to emit an emergency diagnostic event carrying `fired_at_ns`, `last_heartbeat_ms`, and `last_active_pid`.
* **Out-of-Scope (Host's Job):** Formatting or displaying the diagnostic error to users.
* **Proposed Signature:**
  ```rust
  pub enum WatchdogAction {
      Reboot,
      Terminate { diagnostic_callback: Option<Box<dyn Fn(WatchdogDiagnostic) + Send + Sync>> },
  }
  ```

### Ask 12: Allowlist Bypass for Shannon Entropy Trap
* **The Case & Why:** Kelir’s Layer 3 trap fast-path flags any command where `argv[1:]` Shannon entropy > 5.2 as an encrypted/base64 payload (`09-protocol.md#Trap-fast-path`). Legitimate base64 JSON params and hashes easily exceed 5.8 entropy.
* **In-Scope for Kelir:** Add a configurable regex or path allowlist to bypass the entropy check for trusted runner binaries (e.g., `capcli`, `node`, `python`).
* **Out-of-Scope (Host's Job):** Validating the internal structure of the bypassed arguments.
* **Proposed Signature:**
  ```rust
  pub struct TrapConfig {
      pub entropy_threshold: f64, // default 5.2
      pub entropy_bypass_paths: Vec<PathBuf>,
  }
  ```

### Ask 13: Dynamic RCU Dispatch-Table Hot-Swapping
* **The Case & Why:** Orchestrators hot-swap routine versions and policy cages at runtime (`plugins.md#Generations`). Kelir compiles plugins into a flat C-dispatch array at boot (`02-plugin-model.md#Static-dispatch`), requiring a microVM reboot to update policies.
* **In-Scope for Kelir:** Support atomic RCU (Read-Copy-Update) pointer swapping for the dispatch table, allowing dynamic hook updates without microVM termination.
* **Out-of-Scope (Host's Job):** Calculating when a generation swap is safe.
* **Proposed Signature:**
  ```rust
  impl Session {
      pub fn update_plugin_dispatch(&self, new_dispatch: Arc<DispatchArray>) -> Result<(), VmmError>;
  }
  ```

### Ask 14: Non-KVM Fallback Driver (`driver-bwrap`)
* **The Case & Why:** Kelir strictly requires hardware virtualization (KVM, HVF, WHP). In unprivileged Docker containers or standard GitHub Actions runners without `/dev/kvm`, Kelir fails boot (`01-architecture.md`).
* **In-Scope for Kelir:** Implement a `driver-bwrap` backend satisfying the `VmmDriver` trait using unprivileged Linux user namespaces and seccomp-unotify.
* **Out-of-Scope (Host's Job):** Deciding whether degraded container isolation is acceptable for production.
* **Proposed Signature:**
  ```rust
  // Inside crates/kelir-drivers/
  pub struct BwrapDriver { /* implements VmmDriver */ }
  ```

---

## 4. Tier B: Existing Plugin Enhancements (8 Upgrades)

### Ask 15: Active Credential Leak Detection (`plugin-wire`, `plugin-pty`)
* **The Case & Why:** If an agent prints an injected vault secret to stdout or echoes it in outbound network payloads, the orchestrator must trigger an immediate security panic. Kelir currently only swaps `vault://` placeholders and strips incoming response headers (`06-wire-plane.md`).
* **In-Scope for Kelir:** Add an in-flight byte-stream scanner to `plugin-wire` and `plugin-pty` that evaluates registered secret patterns and triggers an emergency panic hook on detection.
* **Out-of-Scope (Host's Job):** Managing secret values or key rotations.
* **Proposed Signature:**
  ```rust
  impl WirePlugin {
      pub fn register_secret_pattern(&mut self, pattern: &str, on_leak: LeakAction);
  }
  ```

### Ask 16: Token-Bucket Rate Limiter with Auto-Tuning (`plugin-wire`)
* **The Case & Why:** Orchestrators enforce priority rate classes (Background, Standard, Critical). Kelir’s wire plane is currently binary: allow or silent drop (`06-wire-plane.md`).
* **In-Scope for Kelir:** Add an in-memory leaky token bucket to `plugin-wire` that recalibrates capacity from upstream `Retry-After`, `X-RateLimit-Reset`, and `X-RateLimit-Remaining` headers.
* **Out-of-Scope (Host's Job):** Allocating priority budgets across agents.
* **Proposed Signature:**
  ```rust
  pub struct WireRateLimitConfig {
      pub domain: String,
      pub refill_rate_per_sec: f64,
      pub burst_capacity: u32,
      pub auto_tune_headers: bool,
  }
  ```

### Ask 17: Stable Host-Correlated Inode Hashes (`plugin-vfs`)
* **The Case & Why:** Optimistic Concurrency Control (`state_fence`) verifies that read files have not mutated before committing writes. Virtual tmpfs inodes generated by guest overlayfs have zero host correlation (`05-storage-plane.md`).
* **In-Scope for Kelir:** Compute a deterministic 64-bit host inode/content hash and expose it in `VfsInodeAttr` on every `on_vfs_lookup`.
* **Out-of-Scope (Host's Job):** Storing or verifying the OCC state fence.
* **Proposed Signature:**
  ```rust
  pub struct VfsInodeAttr {
      // ... existing fields ...
      pub host_stable_hash: u64, // Correlated to host physical file identity
  }
  ```

### Ask 18: Dynamic Multi-Device Read-Only Image Mounting (`plugin-vfs`)
* **The Case & Why:** Orchestrators package capability libraries and pre-compiled wheels into `.cap` packs (`marketplace.md#Packs`). Installing them into guest tmpfs upperdir exhausts microVM RAM.
* **In-Scope for Kelir:** Allow attaching secondary read-only squashfs/EROFS images as discrete guest mount points (e.g. `/opt/packs/<id>`) on a running microVM.
* **Out-of-Scope (Host's Job):** Building or signing pack images.
* **Proposed Signature:**
  ```rust
  impl VfsPlugin {
      pub async fn mount_readonly_image(&self, guest_path: &Path, image_path: &Path) -> Result<(), VfsError>;
  }
  ```

### Ask 19: Format-Preserving Data Masking (FPA) (`plugin-vfs`)
* **The Case & Why:** Testing routines against production-seeded data requires redacting sensitive columns (emails, SSNs) while preserving type and format validity. Kelir’s `maskRead` only does fixed-width padding (`XXXX`) (`05-storage-plane.md`).
* **In-Scope for Kelir:** Support custom regex replacement callbacks in `maskRead` to emit format-valid synthetic strings (e.g. `anon_*@sim.local`).
* **Out-of-Scope (Host's Job):** Defining which columns or tables require masking.
* **Proposed Signature:**
  ```rust
  pub struct MaskRule {
      pub pattern: Regex,
      pub replacer: Arc<dyn Fn(&str) -> String + Send + Sync>,
  }
  ```

### Ask 20: BPE Token-Count Clamping in Token Shield (`plugin-pty`)
* **The Case & Why:** Kelir’s Token Shield clamps by raw bytes (64KB) and lines (300) (`04-terminal-plane.md`). Minified stack traces or compact JSON can compress 16,000+ LLM tokens into 64KB, overflowing the agent's context window.
* **In-Scope for Kelir:** Embed an optional BPE tokenizer (e.g. `cl100k_base` via `tiktoken-rs`) into `plugin-pty` to clamp terminal streams strictly by token ceiling.
* **Out-of-Scope (Host's Job):** Prompt assembly or context window allocation.
* **Proposed Signature:**
  ```rust
  pub struct TokenShieldConfig {
      pub max_tokens: Option<usize>, // Pluggable BPE token ceiling
      pub tokenizer: Option<TokenizerModel>,
      // ... existing byte/line limits ...
  }
  ```

### Ask 21: Semantic PTY Screen Buffer Matrix Export (`plugin-pty`)
* **The Case & Why:** Orchestrators building structured UI receipts (wireframe screens) must scrape raw ANSI escape streams, which is brittle and non-deterministic.
* **In-Scope for Kelir:** Maintain an in-memory 2D virtual terminal grid (VT100 state machine) and expose a structured JSON screen matrix representing rendered characters, lines, and cursor positions.
* **Out-of-Scope (Host's Job):** Rendering UI screens or user formatting.
* **Proposed Signature:**
  ```rust
  impl PtyPlugin {
      pub fn export_screen_matrix(&self) -> TerminalGrid; // Structured visible terminal state
  }
  ```

### Ask 22: External Audit Sink Delegation (`plugin-ledger`)
* **The Case & Why:** Orchestrators maintain an authoritative, append-only SQLite `audit.db` with continuous SHA-256 hash chaining. Kelir’s `plugin-ledger` writes an independent `.kelir/audit.merkle.db` (`08-epistemic.md`), creating a split-brain.
* **In-Scope for Kelir:** Allow `plugin-ledger` to delegate persistence entirely to a host-supplied callback, streaming execution events out without writing to the local Merkle DB.
* **Out-of-Scope (Host's Job):** Hashing, notarizing, or persisting the audit chain.
* **Proposed Signature:**
  ```rust
  pub struct LedgerConfig {
      pub external_sink: Option<Arc<dyn ExternalAuditSink + Send + Sync>>,
      pub suppress_local_db: bool,
  }
  ```

---

## 5. Tier C: Decoupled Standalone Plugins (4 New Crates)

*These capabilities do not belong in Kelir core. They should be built as independent, composable crates in `kelir-plugins/`.*

### Ask 23: `plugin-db-proxy` (The Agnostic Database Plane)
* **The Case & Why:**
  * If an agent runs PostgreSQL, it queries over TCP port 5432.
  * If an agent runs SQLite, it compiles SQLite into its guest binary, completely bypassing host `sqlite3_set_authorizer` hooks.
  * To govern databases agnostically, Kelir must mediate the database protocol.
* **Crate Name:** `kelir-plugins/plugin-db-proxy`
* **In-Scope for Kelir:**
  1. **Postgres Wire Protocol (`pgwire`):** Intercept TCP 5432, decode frontend 'Q' (Query) and 'P' (Parse) messages, extract SQL string + parameters, and fire `on_db_statement`.
  2. **Host-Brokered SQLite Socket:** Expose `/run/kelir/db.sock` proxying SQLite operations over `virtio-shm` directly to the host.
  3. **Verdict Execution:** Execute `DbAction::Allow`, `DbAction::Deny { code, msg }` (injecting native protocol error packets), or `DbAction::Rewrite { new_sql, new_params }` (in-flight statement replacement).
* **Out-of-Scope (Host's Job):** The actual SQLite engine, C-authorizer rules, AST parsing, RLS logic, and table schemas.
* **Proposed Contract:**
  ```rust
  pub enum DbAction {
      Allow,
      Deny { sqlstate: String, message: String },
      Rewrite { new_sql: String, new_params: Vec<Value> },
  }

  #[async_trait]
  pub trait DbProxyPlugin: Send + Sync {
      async fn on_db_statement(&self, event: &DbStatementEvent) -> Result<DbAction, PluginError>;
  }
  ```

### Ask 24: `plugin-mesh` (Sovereign Federation Transport)
* **The Case & Why:** Enterprise agent clusters call routines across sovereign workspaces (`ws://<workspace>/<routine>@<version>`). Kelir’s wire plane is strictly client-to-upstream (`06-wire-plane.md`).
* **Crate Name:** `kelir-plugins/plugin-mesh`
* **In-Scope for Kelir:** Secure point-to-point mTLS/WireGuard tunnel between discrete Kelir hypervisor instances, propagating W3C traceparents and call-hop counters.
* **Out-of-Scope (Host's Job):** Workspace routing, RBAC authorization, or routine discovery.
* **Proposed Contract:**
  ```rust
  pub struct MeshPacket {
      pub source_workspace: String,
      pub target_workspace: String,
      pub hop_count: u8,
      pub trace_id: String,
      pub payload: Vec<u8>,
  }
  ```

### Ask 25: `plugin-lock-broker` (Distributed Entity Lease Engine)
* **The Case & Why:** Multiple agent microVMs operating concurrently on shared state require cooperative exclusion locks. Kelir’s current broker only intercepts `.git/index.lock` (`05-storage-plane.md#Entity-lock-broker`).
* **Crate Name:** `kelir-plugins/plugin-lock-broker`
* **In-Scope for Kelir:** Intercept `FUSE_SETLK` and `FUSE_SETLKW` on arbitrary configurable lock paths, routing lock acquisition through a centralized host mutex table with TTL heartbeat tracking.
* **Out-of-Scope (Host's Job):** Business claim lifecycles or lease allocation policies.

### Ask 26: `plugin-supply-chain` (Package Guard)
* **The Case & Why:** When an agent runs `npm install` or `pip install`, malicious dependencies can execute untrusted code via `postinstall.sh` or `setup.py` hooks.
* **Crate Name:** `kelir-plugins/plugin-supply-chain`
* **In-Scope for Kelir:** Hook process spawn events within package manager process trees and block execution of dynamic lifecycle scripts.
* **Out-of-Scope (Host's Job):** Dependency vulnerability scanning or registry policy.

---

## 6. Implementation Sequence & Next Steps

```
PHASE 1: Core Determinism & Control (Month 1)
  ├── Ask 1: vCPU PMU retired instructions in ExecutionReceipt
  ├── Ask 3: Non-blocking TrapAction::Yield
  ├── Ask 4: Immediate TrapAction::Abort (SIGKILL)
  ├── Ask 7: Synthetic virtual clocksource
  └── Ask 12: Allowlist bypass for Shannon entropy trap

PHASE 2: Data Plane & Storage (Month 2)
  ├── Ask 2: virtio-shm shared memory buffers
  ├── Ask 9: Pluggable host vsock listener registration
  ├── Ask 17: Stable host-correlated inode hashes for OCC
  ├── Ask 18: Dynamic read-only image mounting
  └── Ask 23: Implementation of `plugin-db-proxy` crate

PHASE 3: Security & Governance (Month 3)
  ├── Ask 5: 2PC lifecycle commit/abort hooks
  ├── Ask 14: Non-KVM fallback driver (driver-bwrap)
  ├── Ask 15: Active credential leak detection (panic hook)
  ├── Ask 16: Token-bucket rate limiter with header tuning
  └── Ask 20: BPE token clamping in Token Shield

PHASE 4: Distributed State & Federation (Month 4)
  ├── Ask 6: Portable headless snapshot bundles (.kelir-snap)
  ├── Ask 8: Graceful host SIGTERM preemption window
  ├── Ask 22: External audit sink delegation in plugin-ledger
  └── Ask 24: Implementation of `plugin-mesh` crate
```

---

## 7. Conclusion

By implementing these **mechanisms** while leaving **policy** strictly to host orchestrators, Kelir avoids bloat and achieves a massive strategic objective:

**Kelir becomes the universally recognized, standard low-level hypervisor for every deterministic agentic operating system in the industry.**