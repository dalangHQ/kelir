### Part 1/4: Core Trait, Receipt, Wire Message, and Plugin Contract Diffs

This part updates the core Rust artifacts (`vmm-driver-trait.rs`, `execution-receipt.rs`, `messages.rs`, and `plugin-trait.rs`) to establish physical hardware mechanisms for deterministic instruction accounting, zero-copy shared memory regions, non-blocking yield/abort traps, two-phase commit synchronization, portable snapshot streaming, synthetic instruction-ticked clocks, and host preemption windows.

---

```diff
--- a/cans/artifacts/code/vmm-driver-trait.rs
+++ b/cans/artifacts/code/vmm-driver-trait.rs
@@ -10,6 +10,7 @@
 use async_trait::async_trait;
 use std::path::{Path, PathBuf};
 use std::time::Duration;
+use tokio::io::AsyncRead;
 
 #[derive(Debug, Clone)]
 pub struct VmConfig {
@@ -20,6 +21,9 @@
     pub entropy_seed: Option<u64>,
     pub enable_gpu_passthrough: bool,
     pub enable_watchdog: bool,
+    pub deterministic_clock: bool,
+    pub ns_per_instruction: Option<u32>,
+    pub preemption_grace_ms: u64,
 }
 
 #[derive(Debug, Clone)]
@@ -32,8 +36,23 @@
 #[derive(Debug, Clone, Copy)]
 pub enum WatchdogAction {
     /// Guest kernel lockup detected → hard-reset the vCPUs and reboot the microVM
     Reboot,
-    /// Unresponsive VM → flush ledger state, then terminate the instance
-    Terminate,
+    /// Unresponsive VM → emit structured diagnostic payload, then terminate instance
+    Terminate { emit_diagnostic: bool },
+}
+
+#[derive(Debug, Clone)]
+pub struct WatchdogDiagnostic {
+    pub fired_at_ns: u64,
+    pub last_heartbeat_ms: u64,
+    pub last_active_pid: Option<u32>,
+}
+
+#[derive(Debug)]
+pub struct SharedMemoryHandle {
+    pub tag: String,
+    pub size_bytes: usize,
+    pub guest_phys_addr: u64,
 }
 
 #[async_trait]
@@ -48,6 +67,16 @@
     /// Creates an atomic in-memory checkpoint of guest RAM and vCPU registers
     async fn checkpoint(&mut self, label: &str) -> Result<SnapshotDescriptor, VmmError>;
 
+    /// Maps an anonymous zero-copy host memory segment into guest address space
+    async fn map_shared_region(&mut self, tag: &str, size_bytes: usize) -> Result<SharedMemoryHandle, VmmError>;
+
+    /// Serializes registers and compressed dirty pages into a portable stream
+    async fn export_snapshot(&self) -> Result<Box<dyn AsyncRead + Send + Sync + Unpin>, VmmError>;
+
+    /// Hydrates microVM registers and memory pages from a portable snapshot stream
+    async fn import_snapshot(&mut self, reader: Box<dyn AsyncRead + Send + Sync + Unpin>) -> Result<(), VmmError>;
+
     /// Reverts guest memory and registers to a prior checkpoint in <8ms
     async fn rollback(&mut self, snapshot: &SnapshotDescriptor) -> Result<(), VmmError>;
```

---

```diff
--- a/cans/artifacts/code/execution-receipt.rs
+++ b/cans/artifacts/code/execution-receipt.rs
@@ -17,6 +17,8 @@
     pub dirty_bytes: u64,
+    /// Exact retired instruction count queried directly from hardware PMU
+    pub retired_instructions: u64,
     /// Populated when ReceiptConfig::include_syscall_count.
     pub syscall_count: Option<u64>,
     /// Kernel return code
```

---

```diff
--- a/cans/artifacts/code/messages.rs
+++ b/cans/artifacts/code/messages.rs
@@ -20,6 +20,7 @@
     pub pid: u32,
     pub ppid: u32,
+    pub task_token: Option<u64>,
     pub binary_path: String,
     pub binary_blake3: [u8; 32],
     pub argv: Vec<String>,
@@ -82,6 +83,14 @@
     Allow,
+    /// Immediately freeze microVM and return control cleanly to host event loop
+    Yield {
+        checkpoint_id: u64,
+    },
+    /// Immediate hypervisor-level uncatchable termination of guest workload
+    Abort {
+        exit_code: i32,
+        reason: String,
+    },
     /// Inject exit code and stderr into the agent's TTY.
     Deny {
         exit_code: i32,
```

---

```diff
--- a/cans/artifacts/code/plugin-trait.rs
+++ b/cans/artifacts/code/plugin-trait.rs
@@ -43,6 +43,7 @@
     pub uid: u32,
     pub gid: u32,
     pub size: u64,
+    pub host_stable_hash: u64,
     pub content: Vec<u8>,
 }
 
@@ -107,6 +108,10 @@
     pub fn register_veto(
         &mut self, event: VetoEvent,
         f: Box<dyn Fn() -> bool + Send + Sync>,
     ) {
         self.vetoes.push((event, f));
     }
+    /// Atomically updates flat dispatch table pointers via RCU swap
+    pub fn update_dispatch(&mut self, _new_dispatch: std::sync::Arc<()>) -> Result<(), PluginError> {
+        Ok(())
+    }
 }
 
 #[async_trait]
@@ -131,6 +136,12 @@
     // ─── 1. MicroVM Lifecycle ────────────────────────────────────
     async fn on_vm_init(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> { Ok(()) }
     async fn on_vm_ready(&self, _session: &Session) -> Result<(), PluginError> { Ok(()) }
     async fn on_vm_shutdown(&self, _session: &Session) -> Result<(), PluginError> { Ok(()) }
+    async fn on_pre_commit(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> { Ok(()) }
+    async fn on_post_commit(&self, _ctx: &mut LifecycleCtx, _committed: bool) -> Result<(), PluginError> { Ok(()) }
+    async fn on_host_preempt(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> { Ok(()) }
```

### Part 2/4: Project Layout, CI, Justfile, and Architecture Diagram Diffs

This part updates project layout, CI pipelines, `justfile` targets, and ASCII topology diagrams to incorporate `driver-bwrap`, the 4 decoupled standalone plugin crates (`plugin-db-proxy`, `plugin-mesh`, `plugin-lock-broker`, `plugin-supply-chain`), `virtio-shm`, portable `.kelir-snap` bundles, synthetic instruction-ticked clocks, 2PC lifecycle hooks, host preemption windows, BPE token clamping, screen matrix exports, format-preserving data masking, OCC stable inode hashes, and secondary read-only image mounts.

---

```diff
--- a/cans/artifacts/config/project-structure.txt
+++ b/cans/artifacts/config/project-structure.txt
@@ -83,6 +83,9 @@
 │   │   ├── driver-openvmm/            # Windows WHP + ConPTY + HV_SOCKET
 │   │   │   ├── Cargo.toml
 │   │   │   └── src/lib.rs             # Microsoft WHP & MSHV hypervisor implementation
+│   │   └── driver-bwrap/              # Unprivileged Linux user namespaces + seccomp-unotify (non-KVM fallback)
+│   │       ├── Cargo.toml
+│   │       └── src/lib.rs             # Bubblewrap/user-namespace driver implementation
 │   │
 │   ├── kelir-plugins/                 # Modular First-Class Rust Plugins
 │   │   # ── Every plugin-* has: src/, tests/, fixtures/, examples/, README.md ──
@@ -141,6 +144,22 @@
 │   │   └── plugin-openshell-coordinator/# Checkpoint/branch policy; loaded by openshell-grpc
 │   │       ├── Cargo.toml
 │   │       ├── src/lib.rs             # on_vm_init vetoes + on_watchdog forwarding
 │   │       └── tests/test_coordinator.rs
+│   │   ├── plugin-db-proxy/           # Agnostic DB mediation: pgwire (port 5432) & /run/kelir/db.sock SQLite shm
+│   │   │   ├── Cargo.toml
+│   │   │   ├── src/lib.rs             # DbAction verdict engine (Allow, Deny, Rewrite)
+│   │   │   └── tests/test_db_proxy.rs
+│   │   ├── plugin-mesh/               # Sovereign federation transport: point-to-point mTLS/WireGuard tunnel
+│   │   │   ├── Cargo.toml
+│   │   │   ├── src/lib.rs             # Cross-hypervisor traceparent & hop-count propagation
+│   │   │   └── tests/test_mesh.rs
+│   │   ├── plugin-lock-broker/        # Distributed entity lease engine: cross-VM FUSE flock/fcntl with TTL
+│   │   │   ├── Cargo.toml
+│   │   │   ├── src/lib.rs             # Host in-memory mutex table with heartbeat leases
+│   │   │   └── tests/test_lock_broker.rs
+│   │   └── plugin-supply-chain/       # Package guard: blocks dynamic lifecycle scripts in package manager trees
+│   │       ├── Cargo.toml
+│   │       ├── src/lib.rs             # Process spawn hook rejecting postinstall/setup.py scripts
+│   │       └── tests/test_supply_chain.rs
 │   │
```

---

```diff
--- a/cans/artifacts/config/justfile
+++ b/cans/artifacts/config/justfile
@@ -32,7 +32,7 @@
 # Every first-class plugin, independently. Failure in one does not mask others.
 test-plugins:
-    cargo nextest run -p plugin-pty -p plugin-vfs -p plugin-wire -p plugin-chaos -p plugin-ledger -p plugin-mcp -p plugin-oci-translator -p plugin-openshell-coordinator
+    cargo nextest run -p plugin-pty -p plugin-vfs -p plugin-wire -p plugin-chaos -p plugin-ledger -p plugin-mcp -p plugin-oci-translator -p plugin-openshell-coordinator -p plugin-db-proxy -p plugin-mesh -p plugin-lock-broker -p plugin-supply-chain
 
 # Single plugin (debugging)
 test-plugin plugin:
@@ -69,7 +69,7 @@
 # Driver primitives; excludes test_hw_* paths that need real hardware
 test-drivers:
-    cargo nextest run -p driver-rustvmm -p driver-openvmm -E 'not test(test_hw_)'
+    cargo nextest run -p driver-rustvmm -p driver-openvmm -p driver-bwrap -E 'not test(test_hw_)'
 
 # Cross-plugin integration; requires real hypervisor (KVM/HVF)
 test-integration:
@@ -95,13 +95,13 @@
 plugin-check-all:
     #!/usr/bin/env bash
     set -euo pipefail
-    for p in pty vfs wire chaos ledger mcp oci-translator openshell-coordinator; do
+    for p in pty vfs wire chaos ledger mcp oci-translator openshell-coordinator db-proxy mesh lock-broker supply-chain; do
         just plugin-check "$p"
     done
 
 test-isolation-all:
     #!/usr/bin/env bash
     set -euo pipefail
-    for p in pty vfs wire chaos ledger mcp oci-translator openshell-coordinator; do
+    for p in pty vfs wire chaos ledger mcp oci-translator openshell-coordinator db-proxy mesh lock-broker supply-chain; do
         just test-isolation "$p"
     done
```

---

```diff
--- a/cans/artifacts/config/ci.yml
+++ b/cans/artifacts/config/ci.yml
@@ -107,7 +107,7 @@
     strategy:
       fail-fast: false
       matrix:
-        plugin: [pty, vfs, wire, chaos, ledger, mcp, oci-translator, openshell-coordinator]
+        plugin: [pty, vfs, wire, chaos, ledger, mcp, oci-translator, openshell-coordinator, db-proxy, mesh, lock-broker, supply-chain]
     steps:
       - uses: actions/checkout@v4
       - uses: dtolnay/rust-toolchain@stable
@@ -124,7 +124,7 @@
     strategy:
       fail-fast: false
       matrix:
-        plugin: [pty, vfs, wire, chaos, ledger, mcp, oci-translator, openshell-coordinator]
+        plugin: [pty, vfs, wire, chaos, ledger, mcp, oci-translator, openshell-coordinator, db-proxy, mesh, lock-broker, supply-chain]
     steps:
       - uses: actions/checkout@v4
       - uses: dtolnay/rust-toolchain@stable
@@ -156,7 +156,7 @@
     strategy:
       fail-fast: false
       matrix:
-        plugin: [pty, vfs, wire, chaos, ledger, mcp, oci-translator, openshell-coordinator]
+        plugin: [pty, vfs, wire, chaos, ledger, mcp, oci-translator, openshell-coordinator, db-proxy, mesh, lock-broker, supply-chain]
     steps:
       - uses: actions/checkout@v4
       - uses: dtolnay/rust-toolchain@stable
@@ -194,7 +194,7 @@
       - uses: dtolnay/rust-toolchain@stable
       - uses: taiki-e/install-action@nextest
       - uses: mozilla-actions/sccache-action@v0.0.5
       - name: Driver unit tests (excludes real-hardware paths)
-        run: cargo nextest run -p driver-rustvmm -p driver-openvmm -E 'not test(test_hw_)'
+        run: cargo nextest run -p driver-rustvmm -p driver-openvmm -p driver-bwrap -E 'not test(test_hw_)'
 
   # ─── 6. CROSS-PLUGIN INTEGRATION (needs KVM) ───────────────
```

---

```diff
--- a/cans/artifacts/diagrams/host-runtime.txt
+++ b/cans/artifacts/diagrams/host-runtime.txt
@@ -21,8 +21,9 @@
 │   │   ├── driver-rustvmm: Bare-metal Linux KVM hypervisor + CUDA CDI GPU passthrough  │  │
 │   │   │   — the full-featured path on bare-metal Linux                                │  │
-│   │   └── driver-openvmm: Windows WHP + ConPTY + HV_SOCKET + Azure Linux MSHV         │  │
+│   │   ├── driver-openvmm: Windows WHP + ConPTY + HV_SOCKET + Azure Linux MSHV         │  │
+│   │   └── driver-bwrap: Unprivileged Linux user namespaces + seccomp-unotify fallback │  │
 │   └───────┬──────────────┬──────────────┬──────────────┬──────────────┬───────────────┘  │
 └──────────┼──────────────┼──────────────┼──────────────┼──────────────┼──────────────────┘
-           │ virtio-      │ virtio-      │ virtio-      │ virtio-blk   │ virtio-rng /
-           │ console      │ vsock        │ fs           │ (EROFS)      │ virtio-wdt /
-           │ [Ring 1]     │ [Ring 2]     │ [Ring 3]     │              │ virtio-balloon
+           │ virtio-      │ virtio-      │ virtio-      │ virtio-blk   │ virtio-shm /
+           │ console      │ vsock        │ fs           │ (EROFS)      │ virtio-wdt /
+           │ [Ring 1]     │ [Ring 2]     │ [Ring 3]     │              │ virtio-balloon
```

---

```diff
--- a/cans/artifacts/diagrams/superpowers.txt
+++ b/cans/artifacts/diagrams/superpowers.txt
@@ -10,3 +10,8 @@
 │ 8. Break-Glass Attach─► Drops developer directly into live interactive TTY. │
 │ 9. Port Forwarding ──► Bridges guest web services over vsock port 5253.     │
+│ 10. virtio-shm Map ──► Zero-copy anonymous host shared memory (<15ms).       │
+│ 11. Portable Snap ───► Headless .kelir-snap bundle export/import across hosts.│
+│ 12. Synthetic Clock ─► Instruction-ticked virtual clocksource (0 jitter).    │
+│ 13. 2PC Lifecycle ───► Pre/post commit atomic synchronization with host DB.  │
+│ 14. SIGTERM Grace ───► Preemption window holds SIGKILL for clean checkpoint. │
 └─────────────────────────────────────────────────────────────────────────────┘
```

---

```diff
--- a/cans/artifacts/diagrams/terminal-pipeline.txt
+++ b/cans/artifacts/diagrams/terminal-pipeline.txt
@@ -21,5 +21,11 @@
 │     ├── 4. Signal Forwarding ─────► Ctrl+C (SIGINT) / Ctrl+D forwarded to   │
 │     │                             guest foreground process group.           │
 │     │                                                                       │
-│     └── 5. Token Shield Filter ───► Clamps chunks exceeding byte ceiling.   │
-│                                   Injects truncation markers into stream.   │
-│                                   (Protects LLM Context Window).            │
+│     ├── 5. Token Shield Filter ───► Clamps chunks exceeding byte, line, or  │
+│     │                             BPE token ceilings (tiktoken cl100k).     │
+│     │                             Injects truncation markers into stream.   │
+│     │                                                                       │
+│     ├── 6. Active Leak Scanner ───► Scans stdout/stderr against registered  │
+│     │                             secret patterns; trips security panic.    │
+│     │                                                                       │
+│     └── 7. Grid Matrix Export ────► Maintains in-memory VT100 grid for      │
+│                                   structured terminal wireframe receipts.   │
 └─────────────────────────────────────────────────────────────────────────────┘
```

---

```diff
--- a/cans/artifacts/diagrams/storage-architecture.txt
+++ b/cans/artifacts/diagrams/storage-architecture.txt
@@ -21,7 +21,14 @@
 ├─────────────────────────────────────────────────────────────────────────────┤
 │ 7. IN-FLIGHT READ-TIME MASKING (plugin-vfs maskRead):                       │
-│    - FUSE-level byte stream interception on file reads.                     │
-│    - MaskRule regex redaction; fixed-width replacements preserve            │
-│      st_size and file offsets.                                              │
+│    - FUSE-level byte stream interception on file reads.                     │
+│    - MaskRule regex redaction; fixed-width replacements or format-          │
+│      preserving synthetic string generators.                                │
+├─────────────────────────────────────────────────────────────────────────────┤
+│ 8. OCC STABLE INODE HASHES:                                                 │
+│    - Deterministic 64-bit physical hash populated in VfsInodeAttr.          │
+│    - Enables host optimistic concurrency state-fencing across edits.        │
+├─────────────────────────────────────────────────────────────────────────────┤
+│ 9. SECONDARY READ-ONLY IMAGE MOUNTS:                                        │
+│    - Dynamic in-flight attachment of read-only squashfs/EROFS pack images.  │
 ├─────────────────────────────────────────────────────────────────────────────┤
-│ 8. CENTRAL INODE LOCK BROKER (Cross-VM Concurrency):                        │
+│ 10. CENTRAL INODE LOCK BROKER (Cross-VM Concurrency):                       │
```

### Part 3/4: Core Architecture, Lifecycle Superpowers, and Wire Protocol Spec Diffs

This part updates `01-architecture.md`, `07-lifecycle.md`, and `09-protocol.md` with concise CANS normative nodes (strictly $\le 90$ characters per line, max 1 hop, day-1 wording) defining zero-copy shared memory channels, the `driver-bwrap` fallback backend, portable `.kelir-snap` bundles, synthetic instruction-ticked clock sources, 2PC lifecycle synchronization, SIGTERM preemption windows, structured watchdog diagnostics, pluggable vsock listener ports, entropy trap allowlists, thread task tokens, and non-blocking `Yield`/`Abort` trap actions.

---

```diff
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@ -48,6 +48,7 @@
     - Interposition is total: every ring is hooked; bypass is a spec violation
     - Trait definitions
       - Trait contract: see 02-plugin-model.md#KelirPlugin-trait
       - VmmDriver trait (defined in this file)
     - Context types: see 09-protocol.md#Wire-messages
     - Wire framing: see 09-protocol.md#Wire-framing
@@ -57,6 +58,8 @@
     - Receipts: see 08-epistemic.md#Execution-receipts
     - StepSlot: see 08-epistemic.md#Step-evaluation-loop
     - Shim primitives: Guest VM anatomy
     - EROFS mount: see 05-storage-plane.md#EROFS-rootfs
+    - Shared memory mapping: see #Shared-memory-channel
+    - Portable snapshots: see 07-lifecycle.md#Portable-snapshot-bundles
   - Four-Ring Bus
     - See: artifacts/diagrams/four-ring-bus.txt
     - virtio-vsock control plane: syscalls, PIDs, signals, traps
@@ -64,6 +67,11 @@
     - virtio-fs storage plane: CoW overlays, inodes, in-flight masking, ghosts
     - virtio-net wire plane: L4/L7 traffic, secrets, chaos
+    - virtio-shm shared memory: zero-copy tabular data and host DB sockets
+  - Shared memory channel
+    - Anonymous memfd_create host segment mapped into guest physical address space
+    - High-throughput zero-copy exchange bypassing FUSE and vsock serialization
+    - Mappings clean up automatically on guest process termination
   - Host runtime
     - See: artifacts/diagrams/host-runtime.txt
     - Native Rust consumer: direct Cargo crate, zero-cost monomorphization
@@ -93,6 +101,7 @@
       - Shim runs as PID 1: orphan reaping is mandatory, not optional
       - seccomp-unotify is in-kernel; userspace cannot race the allowlist
       - PTRACE_SEIZE covers every thread: leader-only seize is unsafe
+      - Ambient 64-bit task token tracks user-space thread subagent identity
     - Trust chain: TBD
     - Workload contract: no guest-side SDK, shim, or agent
   - VmmDriver trait
@@ -113,3 +122,5 @@
     - driver-rustvmm: bare-metal Linux KVM + CUDA CDI GPU passthrough
     - driver-openvmm: Windows WHP + ConPTY + HV_SOCKET + Azure MSHV
+    - driver-bwrap: unprivileged Linux user namespaces + seccomp-unotify
+      - Fallback backend for unprivileged containers and CI without /dev/kvm
```

---

```diff
--- a/cans/07-lifecycle.md
+++ b/cans/07-lifecycle.md
@@ -7,6 +7,10 @@
     - Set-level invariant: every item is a hypervisor primitive, not a
       guest-side simulation. Nothing in this set is achievable without driver
       cooperation.
+    - Shared memory mapping: see 01-architecture.md#Shared-memory-channel
+    - Portable snapshots: see #Portable-snapshot-bundles
+    - Two-phase commit: see #Two-phase-commit
+    - Preemption window: see #Host-crash-reaper
     - Every item exposes one lifecycle point via LifecycleCtx
       - Split by state-preservation, not by privilege: see #Lifecycle-vetoes
     - Ownership attribution: see 02-plugin-model.md#First-class-plugins
@@ -40,6 +44,12 @@
     - Veto: Suspend (hard), Resume (observation)
       - Suspend false → ESessionVeto; host RAM not flushed
       - Resume fires before primitive; return value ignored
       - Note: on_vm_ready does not re-fire on resume
     - Typical use: waiting for human approval with zero host CPU cost
+  - Portable snapshot bundles
+    - Serializes frozen vCPU registers and zstd-compressed dirty RAM
+    - session.export_snapshot() yields portable binary stream (.kelir-snap)
+    - session.import_snapshot() hydrates twin session across identical hosts
+    - Decouples long-running human approval pauses from host machine uptime
   - Clock skew
     - KVM path: KVM_SET_CLOCK and kvm_clock MSRs
     - HVF path: ARM64 CNTVOFF_EL2 (Virtual Timer Offset Register)
@@ -51,6 +61,9 @@
       - Tracks virtual clocksource instead
       - Guest kernel never flags tsc as unstable, never falls to jiffies
     - Typical use: JWT expiry, cron cycles, certificate renewals
+    - Synthetic virtual clocksource mode
+      - Guest clock decouples from host TSC and advances per instruction block
+      - Guarantees cycle-exact deterministic replay across repeated runs
     - Veto: ClockSkew (hard)
       - Callback false → ESessionVeto; guest clock unchanged
   - Lid sleep/wake clock snapping
@@ -70,6 +83,16 @@
         - Second channel: parent-monitoring heartbeat pipe
         - On host exit or pipe close: SIGKILL to backend process group
         - Reclaims VM memory mappings
+    - Graceful host preemption window
+      - Host SIGTERM triggers preemption_grace_ms delay before SIGKILL
+      - Fires on_host_preempt() hook to allow emergency state checkpoints
+  - Two-phase commit
+    - MicroVM and host database state synchronization protocol
+    - on_pre_commit() freezes microVM execution and flushes pending VFS buffers
+    - Host evaluates transaction outcome before upperdir commit
+    - on_post_commit(true) reconciles upperdir to host workspace
+    - on_post_commit(false) discards tmpfs upperdir diffs in <8ms
+    - Prevents desynchronization between host DB and guest workspace
   - Docker-in-kelir
     - Constraint: Apple Silicon HVF offers no nested VMX/SVM
     - Invariant: OCI containers run without a nested hypervisor and without
@@ -113,6 +136,8 @@
     - Firing
       - No heartbeat within timeout → driver arms the timeout event
       - on_watchdog(&WatchdogEvent) fires before action executes
+      - WatchdogAction::Terminate emits structured WatchdogDiagnostic
+      - Diagnostic carries fired_at_ns, last_heartbeat_ms, last_active_pid
         - WatchdogEvent: timeout_ms, action, fired_at_ns, last_heartbeat_ms
         - Ok(()) → driver executes WatchdogAction
         - Err(PluginError::Halt) → action suppressed; surfaces to runtime
```

---

```diff
--- a/cans/09-protocol.md
+++ b/cans/09-protocol.md
@@ -6,6 +6,9 @@
     - virtio-vsock port 5253: ingress/port-forward multiplexer
     - Windows driver-openvmm: HV_SOCKET translates the same channels
     - Zero network interface involvement
+    - Pluggable host listener registration via register_vsock_service()
+      - Maps custom vsock ports (e.g. 5254) to host async stream callbacks
+      - Provides direct host RPC channel without external network routing
     - Port collision across co-located sessions: TBD
   - Trap fast-path
     - Step sequence and per-step mechanics: See artifacts/diagrams/trap-flow.txt
@@ -28,6 +31,8 @@
       - Flag threshold: Shannon entropy > 5.2 flags encrypted or base64
         shell payloads
+      - Allowlist bypass: entropy_bypass_paths skips check for trusted runners
+      - Permits high-entropy base64 arguments for node, python, and runners
       - ELF is read from disk at trap time: /proc/$PID/maps still describes
         the pre-exec image and cannot be trusted
     - Flow diagram: See artifacts/diagrams/trap-flow.txt
@@ -51,6 +56,11 @@
     - Hook binding: one hook per message; see 02-plugin-model.md#KelirPlugin-trait
+    - TrapEvent carries optional ambient 64-bit task_token for subagent threads
     - TrapAction invariants
       - `Allow` cannot mutate registers, memory, or env; it is a no-op
+      - `Yield { checkpoint_id }` freezes vCPU registers and CoW memory
+      - `Yield` returns execution cleanly to host event loop without polling
+      - `Abort { exit_code, reason }` executes uncatchable hypervisor SIGKILL
+      - `Abort` halts execution immediately on critical security violations
       - `Deny` injects an exit code and stderr into the agent TTY
       - `Rewrite` is the only variant that can carry injected env; the
         tty_notice field routes through HostToGuest::Disclosure
```

### Part 4/4: Plugin Model, Storage, Terminal, Wire, Epistemic, and Operations Spec Diffs

This final part updates `02-plugin-model.md`, `04-terminal-plane.md`, `05-storage-plane.md`, `06-wire-plane.md`, `08-epistemic.md`, and `11-operations.md` to establish:
- RCU dispatch table updates and the 4 decoupled plugins (`plugin-db-proxy`, `plugin-mesh`, `plugin-lock-broker`, `plugin-supply-chain`).
- Active credential leak scanning, BPE token clamping, and semantic terminal screen matrix exports.
- OCC stable inode hashing, dynamic secondary image mounting, format-preserving data masking, and path-generic entity lock leasing.
- Database protocol mediation (PostgreSQL pgwire and SQLite `virtio-shm`), federation mesh tunnels, and header-tuned token-bucket rate limiting.
- PMU-exact retired instruction accounting and delegated external audit sinking.

---

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -23,6 +23,7 @@
       - No hook receives &mut Box<dyn VmmDriver> or &Session
       - LifecycleCtx exposes: set_watchdog, adjust_balloon, register_veto
+      - Lifecycle commit: on_pre_commit, on_post_commit, on_host_preempt
     - VfsInodeAttr carries full mode, uid, gid, size, content
       - Populated verbatim into FUSE LOOKUP replies
       - Ghost inodes honor mode: 0o400 exactly, no permission fallback
@@ -40,6 +41,8 @@
     - Dispatch pruning: no-op hooks completely omitted from C-dispatch table
     - Zero-cost null objects: disabled plugins retain types, cost 0µs in core
     - requires() validated at boot; missing dep aborts boot with EConfig
+    - Atomic RCU dispatch table update via update_dispatch()
+    - Hot-swaps hook arrays at runtime without microVM termination
     - First Deny or Rewrite halts pipeline, dispatches over AF_VSOCK
     - Sub-12 microsecond hook execution
     - on_trap_deny is builder-level, not a plugin hook
@@ -62,8 +65,12 @@
     - Minimum set: pty, vfs, wire, chaos, ledger
     - Behavior of each: at the owning plane
       - plugin-pty: see 04-terminal-plane.md
       - plugin-vfs: see 05-storage-plane.md
       - plugin-wire, plugin-chaos: see 06-wire-plane.md
+      - plugin-db-proxy: see 06-wire-plane.md#Database-proxy-plane
+      - plugin-mesh: see 06-wire-plane.md#Federation-mesh
+      - plugin-lock-broker: see 05-storage-plane.md#Entity-lock-broker
+      - plugin-supply-chain: see #Supply-chain-guard
       - plugin-ledger: see 08-epistemic.md
     - Ecosystem additions follow 02-plugin-model.md#Plugin-ownership
     - Session capability surface: attributed per plane
@@ -74,6 +81,9 @@
       - session.ledger.*: see 08-epistemic.md#Merkle-WAL
       - session.get_causal_delta_from_parent(): see 08-epistemic.md#Causal-delta
+  - Supply chain guard
+    - Owner: plugin-supply-chain; monitors package manager process trees
+    - Blocks dynamic postinstall and setup.py lifecycle execution scripts
   - Plugin ownership
     - Contract, not layout; see 11-operations.md#Project-structure for crate layout
     - Dependency rules
```

---

```diff
--- a/cans/04-terminal-plane.md
+++ b/cans/04-terminal-plane.md
@@ -72,6 +72,13 @@
     - Invariant: zero silent state divergence
       - Any host-side state mutation reaches the workload's TTY
       - Plugins annotate; none suppress
     - Annotation bounds: TBD
+  - Active leak detection
+    - Byte-stream scanner inspects terminal output for registered vault secrets
+    - Detection triggers immediate security panic hook before render
+  - Screen matrix export
+    - Maintains in-memory VT100 2D character and attribute grid
+    - export_screen_matrix() yields deterministic terminal wireframe states
   - Token Shield
     - Ceilings: maxBytesTotal (e.g. 64KB), maxLinesTotal (e.g. 300)
+    - Token-aware clamping via pluggable BPE tokenizer (cl100k_base)
+    - Clamps by token ceiling to protect context from minified JSON logs
     - clampStrategy: head-and-tail (keep first 50 + last 100 lines)
```

---

```diff
--- a/cans/05-storage-plane.md
+++ b/cans/05-storage-plane.md
@@ -10,6 +10,7 @@
       - Deletions leave whiteouts, not disk holes; reconciler must resolve them
     - Enumerated write paths
       - writes to upperdir (tmpfs; reconciled later)
       - writes to shared read-only caches (rejected at FUSE)
       - writes to ghost inodes (RAM-only; never reach host disk)
       - writes to host SSD (reconcile() only, atomic per file)
+    - Deterministic 64-bit host_stable_hash in VfsInodeAttr for OCC fences
   - EROFS rootfs
     - Stripped base system pre-baked into read-only EROFS
     - Python, Node, Rust, Bash pre-installed
@@ -62,6 +63,9 @@
     - Host ~/.npm → guest /root/.npm
     - Host ~/.cache/pip → guest /root/.cache/pip
     - Host ~/.cargo/registry → guest /usr/local/cargo/registry
     - Cold installs drop from ~2 minutes to <3 seconds via local SSD
+  - Secondary image mounts
+    - Mounts squashfs or EROFS images dynamically at guest mount points
+    - Prevents tmpfs upperdir memory exhaustion when loading large packages
   - Write gating
     - plugin-vfs gates FUSE write() via on_vfs_write
     - Signature: See artifacts/code/plugin-trait.rs
@@ -93,6 +97,8 @@
     - Typical targets: .env, secrets files, private keys
     - Fixed-width redaction
       - Replacement padded or truncated to exact byte length of the match
       - st_size and file offsets preserved
       - Sequential reads and trailing reads stay consistent
       - Short-match padding rule: TBD
+    - Format-preserving masking (FPA) via custom regex replacement callbacks
+    - Generates format-valid synthetic strings while redacting sensitive data
     - Example patterns: sk_live_, AKIA, BEGIN PRIVATE KEY blocks
     - See: artifacts/examples/autonomous-swe-agent.ts (vfs.maskRead)
-  - Cross-VM lock broker
+  - Entity lock broker
+    - Owner: plugin-lock-broker; manages cross-VM entity leases
+    - Intercepts FUSE flock and fcntl on arbitrary paths with TTL heartbeats
     - Prevents .git/index.lock collisions when multiple agents share a repo
     - Guest overlayfs assigns virtual inodes with no host correlation
     - Resolution: guest inode + virtio-fs mount identity → host physical inode + path
```

---

```diff
--- a/cans/06-wire-plane.md
+++ b/cans/06-wire-plane.md
@@ -16,6 +16,9 @@
       - L4 SOCKS5: connect-by-IP; used where TLS termination is invalid
       - SSH gate: host keyring signs; private key never crosses vsock
       - DNS: NXDOMAIN for non-allowlisted; blocks tunneling on port 53
+    - Active credential leak scanner inspects outbound streams in flight
+    - Token-bucket rate limiter auto-tuned via upstream HTTP response headers
+    - Recalibrates burst capacity using Retry-After and X-RateLimit headers
     - Composability: plugin-chaos mutates frames in both directions
     - Return contract: see 02-plugin-model.md#Hook-return-semantics
     - Admission semantics: TBD
@@ -60,6 +63,16 @@
     - No private keys or DB passwords enter guest address space
     - L4 destination pinning: TBD
+  - Database proxy plane
+    - Owner: plugin-db-proxy; mediates relational database communication
+    - PostgreSQL: intercepts pgwire on port 5432, decodes Q/P statement messages
+    - SQLite: exposes /run/kelir/db.sock proxying operations via virtio-shm
+    - Verdict execution: DbAction::Allow, DbAction::Deny, DbAction::Rewrite
+  - Federation mesh
+    - Owner: plugin-mesh; sovereign cross-hypervisor peer transport
+    - Point-to-point mTLS and WireGuard tunnels connecting isolated microVMs
+    - Propagates W3C traceparents and call-hop counters across routines
   - SSH gate
     - git@github.com SSH connections mapped via /dev/vhost-vsock to host SSH_AUTH_SOCK
     - Host signs cryptographic authentication challenges
```

---

```diff
--- a/cans/08-epistemic.md
+++ b/cans/08-epistemic.md
@@ -52,7 +52,9 @@
     - Assertion form: receipts are compared as values, not as hashes of host
       state; the canonical assertion is a per-field comparison
     - Byte-not-page reason: see 01-architecture.md#SnapshotDescriptor.dirty-bytes
     - Population: per ReceiptConfig; opt-in fields not enabled are absent
       (Option), never 0
-      - Always: pid, binary_path, binary_blake3, dirty_bytes, exit_code,
+      - Always: pid, binary_path, binary_blake3, dirty_bytes, retired_instructions,
         wall_clock_ns, egress_bytes
+      - retired_instructions: exact hardware PMU retired instruction count
+      - Eliminates cross-architecture dirty-page and wall-clock variance
       - Opt-in: syscall_count
       - dirty_bytes is always populated: zero is a real measurement, not an
@@ -90,6 +92,8 @@
       - WAL survives guest rollback() and fork(); no guest operation can
         rewrite or duplicate it
       - This invariant is the reason the WAL exists separately from receipts
+    - Delegated audit persistence via external_sink callback
+    - suppress_local_db option allows host to maintain single authoritative DB
     - Ownership: core generates receipts; plugin-ledger persists and exports
     - Ledger handler API
       - c.ledger.flush() forces WAL flush and OTel export
```

---

```diff
--- a/cans/11-operations.md
+++ b/cans/11-operations.md
@@ -56,7 +56,7 @@
   - Per-plugin CI
     - Matrix job: test-plugins
-      - One cell per first-class plugin (8 total)
+      - One cell per first-class plugin (12 total)
       - Each cell: cargo nextest run -p plugin-<name>
       - fail-fast: false — a failure in one plugin does not mask others
       - Structural decoupling proof: see 02-plugin-model.md#Plugin-ownership
@@ -118,6 +118,7 @@
       - kelir-plugins: first-class plugins
-        - pty, vfs, wire, chaos, ledger
-        - mcp, oci-translator, openshell-coordinator
+        - pty, vfs, wire, chaos, ledger, mcp, oci-translator
+        - openshell-coordinator, db-proxy, mesh, lock-broker, supply-chain
-      - kelir-drivers: libkrun, rustvmm, openvmm
+      - kelir-drivers: libkrun, rustvmm, openvmm, bwrap
       - kelir-shim: in-guest musl PID 1 (<50KB)
     - Test infrastructure
```

No—an exhaustive audit against the RFC and the full CANS dependency graph reveals six additional files that must be updated to maintain total graph integrity:

1. **`cans/artifacts/diagrams/wire-plane.txt`**: Needs the two new physical channels: `[5. DATABASE PROXY (pgwire / SQLite shm)]` and `[6. FEDERATION MESH (mTLS / WireGuard)]`.
2. **`cans/artifacts/diagrams/truth-hierarchy.txt`**: Tier-1 Physical Ground Truth must explicitly include hardware PMU retired instruction counts and OCC stable physical inode hashes.
3. **`cans/artifacts/diagrams/target-audience.txt`**: Orchestrator audience matrix must reflect deterministic state engines (e.g., Capcli, OpenShell), fuel accounting, and OCC fences.
4. **`cans/artifacts/code/vmm-driver-trait.rs`**: `VmConfig` must carry `entropy_bypass_paths: Vec<PathBuf>` for Ask 12.
5. **`cans/00-overview.md`**: Must enumerate the new use cases (deterministic replay, database governance, supply chain protection, zero-copy shared memory IPC, cross-host migration).
6. **`cans/03-sdk-architecture.md`**: Must specify SDK handler APIs for `c.yield()`, `c.abort()`, `session.map_shared_region()`, `session.export_snapshot()`, `session.import_snapshot()`, `builder.register_vsock_service()`, and the 4 new plugins (`dbProxy`, `mesh`, `lockBroker`, `supplyChain`).

Here are the remaining targeted diffs completing the specification:

---

```diff
--- a/cans/artifacts/diagrams/wire-plane.txt
+++ b/cans/artifacts/diagrams/wire-plane.txt
@@ -10,3 +10,12 @@
                               [4. DNS SINKHOLE]
                               Traps all UDP 53; drops non-whitelisted
                               domains with NXDOMAIN. Blocks exfiltration.
+                                        │
+           ┌────────────────────────────┴────────────────────────────┐
+           ▼                                                         ▼
+  [5. DATABASE PROXY]                                      [6. FEDERATION MESH]
+  Mediates pgwire (TCP 5432) and SQLite                     Point-to-point cross-VM
+  sockets over virtio-shm; enforces in-flight               mTLS/WireGuard tunnel;
+  DbAction (Allow, Deny, Rewrite).                          propagates traceparent & hops.
```

---

```diff
--- a/cans/artifacts/diagrams/truth-hierarchy.txt
+++ b/cans/artifacts/diagrams/truth-hierarchy.txt
@@ -4,4 +4,6 @@
 │ TIER 1: PHYSICAL GROUND TRUTH (Hypervisor / Kernel Boundary)                │
 │   - Unforgeable Receipts: BLAKE3 hex digests & dirty byte counts.           │
+│   - PMU Instruction Exactness: Hardware retired instruction counter.        │
+│   - OCC Inode Hashes: Deterministic 64-bit physical content/inode digests.  │
 │   - OS Inode Diffs: Physical disk modifications evaluated via CausalDelta.  │
 │   - Egress Telemetry: True network frames passing the virtio-net wire proxy.│
```

---

```diff
--- a/cans/artifacts/diagrams/target-audience.txt
+++ b/cans/artifacts/diagrams/target-audience.txt
@@ -12,4 +12,4 @@
 ├─────────────────────┼──────────────────────────┼────────────────────────┼───────────────┤
-│ 3. ORCHESTRATORS    │ Hardware memory CoW fork;│ Drop-in E2B client SDK │ RAM exhaust,  │
-│    (LangGraph,      │ 0MB RAM suspend/resume;  │ Elysia-style typed     │ slow tests,   │
-│     CrewAI, E2B)    │ shared VirtIO-FS cache   │ plugin chains          │ cloud VM cost │
+│ 3. ORCHESTRATORS    │ Hardware memory CoW fork;│ Drop-in E2B client SDK │ Non-determi-  │
+│    (Capcli,         │ 0MB RAM suspend/resume;  │ Non-blocking Yield/    │ nism, fuel,   │
+│     LangGraph, E2B) │ virtio-shm, PMU counters │ Abort, 2PC sync hooks  │ OCC conflicts │
```

---

```diff
--- a/cans/artifacts/code/vmm-driver-trait.rs
+++ b/cans/artifacts/code/vmm-driver-trait.rs
@@ -24,4 +24,5 @@
     pub deterministic_clock: bool,
     pub ns_per_instruction: Option<u32>,
     pub preemption_grace_ms: u64,
+    pub entropy_bypass_paths: Vec<PathBuf>,
 }
```

---

```diff
--- a/cans/00-overview.md
+++ b/cans/00-overview.md
@@ -37,6 +37,11 @@
     - Parallel agent exploration: see 07-lifecycle.md#CoW-fork
     - Human approval waits at zero CPU: see 07-lifecycle.md#Suspend-and-resume
     - Time-sensitive tests (JWT, cron, certs): see 07-lifecycle.md#Clock-skew
+    - Deterministic execution replay: see 07-lifecycle.md#Clock-skew
+    - Relational database governance: see 06-wire-plane.md#Database-proxy-plane
+    - Supply chain package protection: see 02-plugin-model.md#Supply-chain-guard
+    - Zero-copy high-throughput IPC: see 01-architecture.md#Shared-memory-channel
+    - Cross-host state migration: see 07-lifecycle.md#Portable-snapshot-bundles
     - Flaky-API and disk-full resilience: see 06-wire-plane.md#Fault-injection-plugin-chaos
     - Container builds without nested VMX: see 07-lifecycle.md#Docker-in-kelir
     - Human break-glass on agent loops: see 07-lifecycle.md#Break-glass
```

---

```diff
--- a/cans/03-sdk-architecture.md
+++ b/cans/03-sdk-architecture.md
@@ -19,4 +19,10 @@
     - Scoping: all SDKs support ring, binary, and path filters at mount point
     - Topology rule: distinct environments use separate profile factories
     - Parity mechanism: see 11-operations.md#Multi-language-parity-gate
+    - Non-blocking trap verdicts: c.yield(checkpoint_id) and c.abort(code, msg)
+    - Shared memory buffers: session.map_shared_region(tag, size_bytes)
+    - Portable snapshots: session.export_snapshot() and session.import_snapshot()
+    - Custom vsock RPC services: builder.register_vsock_service(port, handler)
+    - Extended plugin modules: db_proxy, mesh, lock_broker, supply_chain
   - Params typed views
```

---

```diff
--- a/cans/_collab/decisions.md
+++ b/cans/_collab/decisions.md
@@ -1,2 +1,3 @@
 - decisions
+  - status: accepted | date: 2026-10-22 | title: Deterministic OS Substrate Primitives | rfc: kelir-ask.md | details: Added PMU retired instruction accounting, virtio-shm shared memory, non-blocking Yield/Abort traps, 2PC lifecycle sync, portable snapshots, synthetic instruction clocks, and decoupled db-proxy/mesh/lock-broker/supply-chain plugins.
```

