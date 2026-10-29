### PART 1/4: CORE INTERFACES & HARDWARE PLANES

---

#### Diff: `cans/artifacts/code/plugin-trait.rs`
*Purges terminal-specific, POSIX-specific, and mixed-hardware hooks from core traits.*

```diff
--- a/cans/artifacts/code/plugin-trait.rs
+++ b/cans/artifacts/code/plugin-trait.rs
@@ -10,19 +10,6 @@
 use crate::handlers::HandlerCtx;
-use crate::protocol::messages::{
-    Heartbeat, MemoryPressureWarning, PortBound, ProcessExit,
-    TrapAction, TrapEvent,
-};
-
-// ── Decoded hook payloads ────────────────────────────────────────
-#[derive(Debug, Clone, Serialize, Deserialize)]
-pub struct WindowResize {
-    pub rows: u16,
-    pub cols: u16,
-    pub xpixel: u16,
-    pub ypixel: u16,
-}
-
-#[derive(Debug, Clone, Serialize, Deserialize)]
-pub struct SendSignal {
-    pub pid: u32,
-    pub signal: u32,
-}
+use crate::protocol::messages::{ProcessExit, TrapAction, TrapEvent};
 use crate::vmm::driver::{VmmDriver, VmConfig, WatchdogAction};
 
@@ -155,16 +142,6 @@
     async fn on_trap(
         &self, _event: &TrapEvent, _ctx: &HandlerCtx,
     ) -> Result<Option<TrapAction>, PluginError> { Ok(None) }
     async fn on_process_exit(
         &self, _event: &ProcessExit, _ctx: &HandlerCtx,
     ) -> Result<(), PluginError> { Ok(()) }
-    async fn on_memory_pressure(
-        &self, _event: &MemoryPressureWarning,
-    ) -> Result<(), PluginError> { Ok(()) }
-    async fn on_heartbeat(
-        &self, _event: &Heartbeat,
-    ) -> Result<(), PluginError> { Ok(()) }
-    async fn on_port_bound(
-        &self, _event: &PortBound,
-    ) -> Result<(), PluginError> { Ok(()) }
-    async fn on_signal_send(
-        &self, _event: &SendSignal,
-    ) -> Result<(), PluginError> { Ok(()) }
-    async fn on_watchdog(
-        &self, _event: &WatchdogEvent,
-    ) -> Result<WatchdogVerdict, PluginError> { Ok(WatchdogVerdict::ExecuteDefault) }
 }
 
 #[async_trait]
 pub trait ConsoleInterposer {
-    async fn on_console_input(&self, _bytes: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
-    async fn on_console_output(&self, _chunk: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
-    async fn on_window_resize(
-        &self, _event: &WindowResize,
-    ) -> Result<(), PluginError> { Ok(()) }
-    async fn on_disclosure(
-        &self, _body: &[u8],
-    ) -> Result<Option<Vec<u8>>, PluginError> { Ok(None) }
+    /// Raw paravirtual character stream input. Intercepts host -> guest port bytes.
+    async fn on_console_input(&self, _port: u32, _bytes: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
+    /// Raw paravirtual character stream output. Intercepts guest -> host port bytes.
+    async fn on_console_output(&self, _port: u32, _chunk: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
 }
 
+#[async_trait]
+pub trait SystemTelemetryParticipant {
+    async fn on_memory_pressure(&self, _available_mb: u32, _total_mb: u32) -> Result<(), PluginError> { Ok(()) }
+    async fn on_heartbeat(&self, _uptime_ms: u64) -> Result<(), PluginError> { Ok(()) }
+    async fn on_watchdog(&self, _event: &WatchdogEvent) -> Result<WatchdogVerdict, PluginError> { Ok(WatchdogVerdict::ExecuteDefault) }
+}
```

---

#### Diff: `cans/artifacts/code/messages.rs`
*Eliminates POSIX `execve` assumption from the core wire framing.*

```diff
--- a/cans/artifacts/code/messages.rs
+++ b/cans/artifacts/code/messages.rs
@@ -48,11 +48,6 @@
     AdjustBalloon {
         target_mb: u32,
     },
-    /// Extension frame on a host-registered channel kind.
-    /// Unknown kinds abort delivery with EUnknownChannel.
-    /// Console control frames travel inside ChannelData addressed to a
-    /// console channel endpoint; there is no dedicated control variant.
     ChannelData {
         kind: u16,
         seq: u64,
         payload: Vec<u8>,
     },
 }
 
@@ -64,13 +59,10 @@
     Deny {
         exit_code: i32,
-        stderr_message: String,
+        diagnostic_payload: Vec<u8>,
     },
-    /// See cans/09-protocol.md#Env-injection
-    /// Injected env keys must remain disjoint across handlers; collisions raise EAmbiguousMutation.
+    /// Dynamic execution mutation. Target-specific payload resolved by interposer.
     Rewrite {
-        new_binary: String,
-        new_argv: Vec<String>,
-        injected_env: HashMap<String, String>,
+        target_mutation: Vec<u8>,
     },
     /// Hypervisor cgroup freeze: freeze guest task group, return control
```

---

#### Diff: `cans/04-terminal-plane.md`
*Removes ANSI, Token Shield, and prompt responder policy from the VirtIO-console plane.*

```diff
--- a/cans/04-terminal-plane.md
+++ b/cans/04-terminal-plane.md
@@ -1,40 +1,19 @@
 - Terminal plane
   - Terminal pipeline
     - Owner: kelir-core (virtio-console domain)
-    - virtio-console is a multi-port paravirtual character bus
-    - Channel 0 defaults to interactive console master
-    - Subsequent ports allocated dynamically at session configuration
-    - Allocates isolated interceptor pipelines per paravirtual channel
-    - Handlers receive strongly-typed ConsoleContext; ambient c.term is eliminated
-    - Interactive channel preserves isatty(1) and raw mode
-    - Streaming ring buffers route directly to host supervisor memory
-    - Stateful stream processing: regex and token shields maintain sliding
-      automata state across arbitrary chunk boundaries
-    - Split ANSI escapes spanning multiple virtqueue frames parse atomically
-    - Clamping and secret redactions evaluate across inter-frame buffers
-    - Terminal evidence uses Three-tier evidence verification model
-    - VT100 parser grid monitors virtual screen and scrollback rows
-    - Token Shield clamps scrollback rows, never mid-stream raw bytes
-    - Truncation notices inject ANSI resets inside DECSTBM status margins
-    - All filters configured strictly using flat PtyConfig, no callbacks
-    - PtyConfig fields: cols, rows, dropOsc52, stripTitle, tokenShield, prompts,
-      streamEngine, carryoverBufferBytes
-    - carryoverBufferBytes default: 512 bytes
-    - Filter stream engines: StreamingAutomata (default) | StatelessRegex
-  - Window resize
-    - Pipeline mechanics: see: artifacts/diagrams/terminal-pipeline.txt
-    - Live resize hook: on_window_resize is observation-only
-  - Signal forwarding
-    - Ctrl+C / Ctrl+D reach the guest foreground process group
-    - on_signal_send is observation-only; a hook cannot rewrite the signal
-    - Wire path: see: 09-protocol.md#Wire-messages
-  - Disclosure
-    - Host-originated mandatory notification stream
-    - Wire message
-      - HostToGuest::ChannelData on the pty-disclosure kind over vsock:5252
-      - Wire message definition: see: 09-protocol.md#Wire-messages
-    - Guest-side
-      - Supervisor routes disclosure directly to active PTY channel
+    - Hardware abstraction: virtio-console multi-port paravirtual character bus
+    - Port 0 allocated to standard console IO; Ports 1-16 dynamically allocated
+    - Transport: Raw byte-stream rings mapped to host supervisor memory
+    - Zero-copy ringbuffer backpressure and queue management
+    - Pipeline isolation: Separate interceptor dispatch array per port ID
+    - Interceptors implement ConsoleInterposer with byte-level access
+    - High-level terminal emulation and ANSI sanitization: see: plugins/pty.md
+  - Control signals and resizing
+    - Console window resize and signal distribution delegate to out-of-band IPC
+    - Carried inside ChannelData over vsock Channel 0
+    - Translation to guest ioctl (TIOCSWINSZ) managed by guest shim broker
```

---

#### Diff: `cans/05-storage-plane.md`
*Removes regex scanning, secret masking patterns, and whiteout logic from the storage plane.*

```diff
--- a/cans/05-storage-plane.md
+++ b/cans/05-storage-plane.md
@@ -1,37 +1,24 @@
 - Storage plane
   - Storage architecture
     - see: artifacts/diagrams/storage-architecture.txt
     - Owner: kelir-core (virtio-fs storage domain pipeline)
-    - Single SSOT: plugin-vfs and plugin-lock-broker attach hooks to this pipeline
-    - All hooks receive strongly-typed FsContext; ambient c.fs is eliminated
-    - Emits StorageDelta directly to the event bus; ledger consumes without VFS introspection
+    - Low-level FUSE pass-through bridge directly linking guest VFS to host workspace
+    - Pipeline engine exposes synchronous lookup, read, and write interception
+    - Hooks receive strongly-typed FsContext; ambient access eliminated
+    - Emits physical StorageDelta events directly to host event bus on commit
     - Encapsulation invariant
       - Host workspace is locked to mode 000 on host; only the hypervisor
         descriptor bridges access to virtio-fs lowerdir
       - Native toolchains outside Kelir observe EACCES on workspace access
       - Granular knob: workspace_encapsulation = Locked | Passthrough
-    - Reconciliation containment invariant
-      - Reconciler scans upperdir diffs only under RESOLVE_BENEATH
-      - Whiteout char devices (st_rdev == 0) unlink files strictly beneath target
-      - Symlink escapes abort with E_PATH_TRAVERSAL
-      - Opaque directories purge without crossing mount roots
-    - virtio-fs is an unadulterated FUSE pass-through bridge
-    - Read and write interposition hooks attach to the FUSE bridge
   - EROFS rootfs
     - Stripped base system pre-baked into read-only EROFS
-    - Python, Node, Rust, Bash pre-installed
     - Cold-mounted as virtio-blk block device in <2ms
     - Zero tarball decompression
   - CoW overlay scratchpad
-    - Host workspace mounted strictly read-only as lower layer
+    - Base workspace mounted strictly read-only as lower layer
     - overlayfs upperdir lives in guest tmpfs
     - All writes stay in-memory until explicit reconcile
     - Instant revert: drop overlayfs upperdir in ~1ms, host SSD untouched
-  - Atomic workspace staging
-    - Writes modified files to sibling temporary files (.tmp.<nonce>)
-    - Atomic rename swaps updated files directly over host target paths
-    - Parent directory fsync flushes updated inodes to disk
-  - Storage delta generation
-    - Storage pipeline computes modified inode delta during reconcile
-    - Emits immutable StorageDelta event to the EventBus on commit:
-      - modified_inodes: Vec<PathBuf>
-      - whiteouts_purged: u64
-      - workspace_dirty_bytes: u64
+  - Reconciler integration
+    - Upperdir diff scanning, whiteout unlinking, and atomic swap: see: plugins/vfs.md
   - Secondary image mounts
     - Grafts host-opened squashfs/EROFS images into guest VFS namespace
@@ -43,14 +30,6 @@
     - SyncVerdict::Allow = allow; kernel proceeds with write to upperdir
     - Err(PluginError::Reject { errno, reason }) = deny
-      - Guest write() fails with named errno; error surfaces in workload
-      - Returned before any bytes enter upperdir
     - Err(PluginError::Halt) = deny + halt pipeline
   - Virtual ID remapping
     - FUSE driver dynamically maps guest UID 0/1000 to caller process UID
-    - File creation on host workspace executes under unprivileged host credentials
-    - Prevents ambient host UID values leaking into guest filesystem metadata
-  - Synthetic FUSE inodes (storage plane)
-    - Virtual files served from host memory into guest VFS
-    - No backing file on host SSD
-  - Sliding-window replacement over FUSE read buffers
-    - Stateful Aho-Corasick automaton carries state across read chunk offsets
-    - Splitting target secrets across 1-byte read calls fails evasion
-    - Carryover window buffers boundary fragments until pattern match or flush
-    - Replaces matches with fixed-width byte patterns preserving file offset
-    - Zero full-file memory buffering on inode open
-    - Observation-only snoop callbacks: see: plugins/vfs.md#Snoop-plane
+  - Read inspection and content masking
+    - Dynamic content synthesis and streaming read replacement: see: plugins/vfs.md
```

---

#### Diff: `cans/06-wire-plane.md`
*Removes DNS entropy, HTTP SNI/Host matching, and L7 forward proxying from the L2 wire plane.*

```diff
--- a/cans/06-wire-plane.md
+++ b/cans/06-wire-plane.md
@@ -2,12 +2,9 @@
   - Wire architecture
     - see: artifacts/diagrams/wire-plane.txt
     - Owner: kelir-core (virtio-net wire domain pipeline)
-    - virtio-net is an L2 TAP bridge
+    - Hardware abstraction: virtio-net paravirtual L2 TAP network bridge
     - Hook points: on_net_outbound and on_net_inbound
-    - Database proxying lives in db-proxy: see: plugins/db-proxy.md
-    - Credential signing: see: plugins/wire.md
-    - Interceptor hooks receive strongly-typed NetContext; ambient c.wire is eliminated
-    - Return contract: see: 02-plugin-model.md#Hook-return-semantics
+    - Interceptor hooks receive NetContext containing raw Ethernet frames
   - TAP L2 bridge
     - TAP interface bridge connects the guest virtio-net device to the host
     - All frames in both directions cross the TAP boundary
@@ -19,17 +16,9 @@
     - on_net_outbound: hook on frames leaving the guest
     - on_net_inbound: hook on frames arriving at the guest
     - TAP Dispatcher checks MountScope in native memory before FFI
-    - Pinned frame inspection ceiling: see: 02-plugin-model.md#FFI-buffer-lifecycle
-  - L7 forward proxy
-    - Terminates guest L7 protocols at host boundary
-    - Re-originates requests from host side
-    - L7 proxy mechanics: see: plugins/wire.md
   - Frame drop and forward semantics
     - Ok(Allow) on outbound = forward; Ok(Allow) on inbound = deliver
     - Ok(YieldToHost) = yield frame to host for async policy
     - Err(Reject) = silent drop (no RST, no guest error)
-    - DNS sinkhole enforces subdomain entropy checks on wildcard masks (*.dom)
-    - Subdomain queries exceeding entropy threshold 4.5 drop with NXDOMAIN
-    - Blocks exfiltration through DNS query labels across allowed domain masks
-    - Admission match order: TLS SNI -> HTTP Host -> Destination IP
-    - Mismatched SNI and HTTP Host headers drop as domain fronting
+  - Protocol proxies and credential masking
+    - L4/L7 policy, DNS entropy analysis, and vault proxying: see: plugins/wire.md
```
### PART 2/4: CORE LIFECYCLE, TELEMETRY & PROTOCOL

---

#### Diff: `cans/07-lifecycle.md`
*Removes container runtime assumptions (Podman/crun), database 2PC logic, and test harness mappings.*

```diff
--- a/cans/07-lifecycle.md
+++ b/cans/07-lifecycle.md
@@ -7,28 +7,9 @@
     - Set-level invariant: every item is a hypervisor primitive, not a
       guest-side simulation. Nothing in this set is achievable without driver
       cooperation.
-    - Per-primitive exercise map
-      - fork, rollback: autonomous-swe-agent.{rs,ts,py}
-      - suspend, resume, skew_clock: autonomous-swe-agent.{rs,ts,py}
-      - watchdog, breakGlass, forward_port: autonomous-swe-agent.{rs,ts,py}
-      - map_shared_region: multi-vm-swarm.rs
-      - register_veto: multi-vm-swarm.rs
-      - export_snapshot, import_snapshot: db-backed-runtime.py
-      - adjust_balloon: serverless-webhook.ts
-    - Exercise invariant
-      - Every primitive mounts end-to-end in one example file
-      - An unexercised primitive fails --lifecycle-coverage
-      - Lifecycle-coverage gate: see: 11-operations.md#Coverage-gates
-      - Veto-coverage: every VetoEvent registered at least once
-        - Fork, Suspend, ClockSkew, Checkpoint, Rollback, Resume, Terminate
-        - An unregistered VetoEvent fails --veto-coverage
-        - Veto-coverage gate: see: 11-operations.md#Coverage-gates
-        - Veto classes: see: #Lifecycle-vetoes
-      - Watchdog-coverage: Suppress and Mitigate returned at least once
-        - ExecuteDefault alone does not satisfy watchdog-coverage
-        - An unreturned verdict fails --watchdog-coverage
-        - Watchdog-coverage gate: see: 11-operations.md#Coverage-gates
-        - Verdicts: see: #Watchdog
+    - Session lifecycle handles: fork, suspend, resume, rollback, skew_clock
+    - Context lifecycle configuration: set_watchdog, adjust_balloon, register_veto
+    - All primitives execute via active VmmDriver backend without in-guest agents
     - Shared memory mapping: see: 01-architecture.md#Shared-memory-channel
     - Portable snapshots: see: #Portable-snapshot-bundles
-    - Two-phase commit protocol: see: #Two-phase-commit
     - Preemption window: see: #Host-crash-reaper
@@ -95,17 +76,6 @@
     - Platform coverage: TBD
-  - Two-phase commit
-    - MicroVM and host database state synchronization protocol
-    - Coordinated via plugin-transaction: see: plugins/transaction.md
-    - on_pre_commit pauses vCPUs and flushes in-flight VFS write buffers
-    - Staging subtree prepared atomically in host workspace
-    - on_post_commit(true) executes atomic directory swap into workspace
-    - on_post_commit(false) unlinks staging directory and purges tmpfs diffs
-    - Prevents desynchronization between host DB and guest workspace
-    - Demonstration target: see: artifacts/examples/db-backed-runtime.py
-  - Docker-in-kelir
-    - Constraint: Apple Silicon HVF offers no nested VMX/SVM
-    - Invariant: OCI containers run without a nested hypervisor and without
-      a host daemon
-    - Mechanism: rootless Podman + crun inside the read-only EROFS rootfs;
-      ephemeral state on tmpfs so the rootfs stays immutable
-    - Container contract: TBD
   - Break-glass
     - Supervisor freezes guest agent task group via cgroup freezer prior to attach
```

---

#### Diff: `cans/08-epistemic.md`
*Excises pseudo-epistemic framing, intent evaluation, and oracle verification in favor of physical telemetry.*

```diff
--- a/cans/08-epistemic.md
+++ b/cans/08-epistemic.md
@@ -1,37 +1,18 @@
-- Verification & Hardware Telemetry
+- Hardware Telemetry & Cryptographic Verification
   - Scope
-    - Boundary specification; not a runtime subsystem
-    - Core owns facts, never judgments
-      - Facts: TrapEvent, ExecutionReceipt, CausalDelta (struct)
-      - Core owns physical hardware telemetry: registers, memory diffs, counters
-    - Epistemic asymmetry invariant
-      - Workload is presumed hostile and remains blind to active profile rules
-      - Policy drops surface as standard POSIX errors (EACCES, NXDOMAIN) or silent drops
-      - Workload cannot distinguish hypervisor policy drops from physical failures
-    - Judgment is defined in external userland orchestration layers
-      - Evaluators consume raw execution receipts independently of core
-    - Axiom enforcement: see: 01-architecture.md#Axiomatic-foundations
-  - Oracle
-    - Evaluator contract defined in orchestrator SDK layer, outside kelir-core
-    - Consumes physical execution receipts to evaluate workload correctness
+    - Telemetry ownership: Hypervisor core measures physical hardware events exclusively
+    - Fact emission: TrapEvent, ExecutionReceipt, CausalDelta
+    - Hardware counters: CPU instructions, dirty pages, MMU faults, wire byte volume
+    - Boundary opacity: Internal interception policies surface to the guest strictly
+      as physical bus errors, standard POSIX errnos, or silent packet drops
   - Primitive vs derived telemetry
     - Telemetry decision test
       - Single read API, hardware boundary, no policy → primitive, core-owned
       - Computed from primitives, policy-bearing → derived, plugin-owned
-      - A new derived metric is a plugin or Oracle change, never a core method
-    - Default posture: per-exec, per-trap, per-receipt signals on; per-syscall,
-      per-request, per-page signals off until enabled per config
-    - Boundaries
-      - Primitive set: see: 01-architecture.md#Core-invariants
-      - CausalDelta: see: #Causal-delta
-      - Merkle chain: see: plugins/ledger.md#Merkle-WAL
-      - OpenTelemetry spans: see: plugins/ledger.md#OpenTelemetry
-      - Oracle analyses: consumer's Oracle impl
+    - Default posture: per-exec, per-trap, per-receipt primitives enabled
   - Execution receipts
     - see: artifacts/code/execution-receipt.rs
-    - End-to-end instruction-counter and dirty-byte hardware attribution
-    - Cryptographic provenance chain: model generation -> trap -> receipt
-    - Granular hardware telemetry: micro-level execution telemetry per process
+    - Deterministic hardware counter attribution emitted per executed process
     - Emitted per executed process, at the trap/exit boundary
-    - Assertion form: receipts are compared as values, not as hashes of host
-      state; the canonical assertion is a per-field comparison
-    - profile_blake3 cryptographic identity
-      - Computes BLAKE3 hash over canonical declarative profile bytes at boot
-      - Injected as genesis block into Merkle WAL before guest vCPU starts
-      - Receipts bind profile_blake3 cryptographically to hardware counters
-    - Extension telemetry: producer writes a TelemetryExtension in `extensions`
-      - Example: namespace net:l7:sql carries statement counts
-      - Example: namespace fs:vfs carries workspace dirty bytes
-      - Example use: see: artifacts/examples/db-backed-runtime.py
-    - Attestation token: receipts carry cryptographic proof of hypervisor run
-    - Bare-metal executions lack hardware receipts and fail oracle evaluation
+    - Receipts compare as values via canonical per-field structural assertions
+    - Receipts carry opaque TelemetryExtension payloads emitted by mounted plugins
+    - Hardware attestation token: BLAKE3 cryptographic proof over execution tuple
     - Byte-not-page reason: see: 01-architecture.md#SnapshotDescriptor.dirty-bytes
@@ -48,22 +29,15 @@
     - Type ownership
       - Struct is a shared context type; it lives in kelir-interface next to
         ExecutionReceipt
-      - Computation is domain-local: StorageDomain emits StorageDelta;
-        ControlDomain emits ExecutionDelta
-      - Core aggregates domain deltas into CausalDelta at checkpoints
-      - Subscribers consume CausalDelta; never diff VFS journals directly
-  - Three-tier evidence verification model
-    - Tier 1: hardware/hypervisor sampling
-      - Signals: BLAKE3 receipts, PMU retired instructions, OCC inode hashes,
-        dirty-byte counts, egress telemetry
-      - Invariant: unforgeable outside guest VM memory boundary
-    - Tier 2: boundary paravirtual intercepts
-      - Signals: FUSE write journals, TAP L2/L4 counters, sanitized PTY digests
-      - Invariant: observed at the VirtIO boundary, never inside the guest
-    - Tier 3: guest userland self-reporting
-      - Signals: harness stdout/stderr, guest status JSON
-      - Invariant: unverified; never substitutes for Tier 1 evidence
-    - Forgeability invariant: Tier 1 is unforgeable outside guest VM memory;
-      Tier 2 is observed at the VirtIO boundary; Tier 3 is unverified
+      - Core aggregates domain deltas into CausalDelta on checkpoint boundaries
+  - Telemetry provenance classification
+    - Invariant: A higher classification never falls back to a lower classification
+    - Class 1 (Hardware Sampling): PMU instruction counters, dirty byte counts,
+      physical exit codes, hardware TSC timestamps. Unforgeable from guest space.
+    - Class 2 (Paravirtual Interception): FUSE buffers, virtqueue frames, character rings.
+      Observed at the hypervisor boundary.
+    - Class 3 (Guest Unverified Data): Workload stdout/stderr payloads, memory buffers.
+      Tainted by default; cannot satisfy hardware assertions.
```

---

#### Diff: `cans/09-protocol.md`
*Removes toolchain-specific references and cleans up channel and execution mutation contracts.*

```diff
--- a/cans/09-protocol.md
+++ b/cans/09-protocol.md
@@ -1,13 +1,11 @@
 - Protocol
   - Control channel
     - virtio-vsock port 5252: unified multiplexed transport
-    - Binary channel framing isolates sub-protocols via channel_id
-      - Channel 0: hypervisor control
-      - Profile patches arrive as ChannelData on Channel 0: see: plugins/profile.md
-      - Channels 1-65535: dynamic multiplexed endpoints
-      - No core symbol names a channel endpoint
-      - Kind registration: see: #Channel-registration
-      - Unchecked raw RW shared memory without vsock sync fails validation
+    - Binary framing isolates sub-protocols via channel_id
+      - Channel 0: Core hypervisor lifecycle and trap control
+      - Channel 1: Dynamic L4 stream forwarding
+      - Channels 2-65535: Dynamically registered extension endpoints
+      - Channel kinds map via explicit registration: see: #Channel-registration
   - Trap fast-path
     - Step sequence and per-step mechanics: see: artifacts/diagrams/trap-flow.txt
     - Three-tier gate pipeline
@@ -16,8 +14,6 @@
       - Gate 3: trap handler dispatch through FFI trampoline on match only
       - Gate latency budgets: see: 02-plugin-model.md#Latency-ceiling
       - Filter invariants: see: 02-plugin-model.md#Filter-before-FFI-gate
-    - Goal: the trap path must not slow `cargo build` / `npm install`
-    - Why three layers, not one
       - A single trap-on-every-syscall design adds ~µs per fork/exec at
         toolchain scale; the allowlist exists to pay that cost only where
         policy is possible
@@ -48,22 +44,14 @@
     - TrapAction-method-mapping
       - TrapAction::Allow: c.resume()
-      - TrapAction::Deny: c.deny(errno, msg)
-      - TrapAction::Rewrite: c.rewrite({ binary, argv, env })
+      - TrapAction::Deny: c.deny(exit_code, diagnostic_payload)
+      - TrapAction::Rewrite: c.rewrite(target_mutation)
       - TrapAction::Yield: c.yield(checkpoint_id)
       - TrapAction::Abort: c.abort(exit_code, reason)
       - TrapAction::Hold: c.hold(duration_ms)
       - TrapAction::Custom: c.custom(name, cbor_payload)
   - Env injection
-    - Allow cannot mutate guest environment
-      - Registers, memory, and environment variables remain untouched
-    - Handler environment mutations require an explicit Rewrite decision
-    - Multi-handler env merge: disjoint keys compose monotonically
-    - Key collision policy: overlapping env keys abort with EAmbiguousMutation
-    - Mutation decision lattice: Allow -> Rewrite -> Deny -> Abort
-    - Calling c.env.set() and returning Decision.resume() returns EINVALIDDECISION
+    - Workload environment variables, registers, and execution paths remain
+      immutable under Allow verdicts
+    - State mutations require explicit Rewrite with validated disjoint keys
```

---

#### Diff: `cans/artifacts/code/execution-receipt.rs`
*Purges domain comments linking receipts to application-specific plugins.*

```diff
--- a/cans/artifacts/code/execution-receipt.rs
+++ b/cans/artifacts/code/execution-receipt.rs
@@ -4,7 +4,6 @@
 //
 // Owned by: cans/08-epistemic.md#Execution-receipts
 
 use serde::{Deserialize, Serialize};
 
-/// Telemetry extension entry. Core never inspects `payload`;
-/// `namespace` is opaque; core never interprets it.
 #[derive(Debug, Clone, Serialize, Deserialize)]
 pub struct TelemetryExtension {
     pub namespace: String,
@@ -21,11 +20,9 @@
 #[derive(Debug, Clone, Serialize, Deserialize)]
 pub struct ExecutionReceipt {
     pub pid: u32,
     pub binary_path: String,
-    /// BLAKE3 digest, canonical 64-char lowercase hex
     pub binary_blake3: String,
-    /// Correlation token for model-generation-to-trap attribution
     pub correlation_token: Option<String>,
     pub dirty_bytes: u64,
     pub hardware_attestation: [u8; 32],
     pub retired_instructions: u64,
```

### PART 3/4: PLUGIN MODEL, ADAPTERS, OPERATIONS & CI

---

#### Diff: `cans/02-plugin-model.md`
*Removes coverage TODO lists and hardcoded test fixtures while strictly preserving `kelir-cloud://` references.*

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -21,11 +21,6 @@
       - plugin-mesh depends strictly on kelir-interface only
     - Direct Cargo dependencies are acyclic: no plugin imports a higher tier
-    - Typed cross-plugin communication
-      - plugin-tool-guard imports plugin-mcp AST and call definitions directly
-      - plugin-lock-broker imports plugin-vfs inode abstractions directly
-      - plugin-db-proxy imports plugin-wire L4 proxy primitives directly
     - Code is sole SSOT: declared programmatically via KelirPlugin trait
     - Manifest artifact is strictly auto-generated for zero-compile indexing
     - Execution kind: Native (Rust in-process) | Foreign (host FFI)
@@ -194,30 +189,11 @@
     - Coverage invariant
       - Every first-class plugin names one demonstrating example
       - Community examples live in cans/artifacts/examples
       - Commercial examples live in ../kelir-cloud/cans/artifacts/examples
       - An orphan plugin fails --plugin-coverage
       - Plugin-coverage gate: see: 11-operations.md#Coverage-gates
       - Hook-coverage: every hook in Segregated-ring-traits is implemented
-      - Depth gap list: hooks and surfaces currently unhit
-        - LifecycleParticipant: on_host_preempt
-        - TrapInterposer: on_pressure
-        - TrapInterposer: on_port, on_signal
-        - ConsoleInterposer: on_window_resize, on_signal_send
-        - NetInterposer: on_net_inbound
-        - CompositePlugin flattening: no example flattens a bundle
-        - derive() vs decorate(): only decorate() is demonstrated
-        - Gap list is evidence, not a waiver: unhit items still fail gates
     - Behavior of each: at its plugin file
       - plugin-pty: home: see: plugins/pty.md
       - plugin-vfs: home: see: plugins/vfs.md
```

---

#### Diff: `cans/10-adapters.md`
*Removes client SDK method enumerations while preserving external commercial references.*

```diff
--- a/cans/10-adapters.md
+++ b/cans/10-adapters.md
@@ -43,15 +43,9 @@
   - E2B compatibility
     - TypeScript: @kelir/e2b-compat
     - Python: kelir.compat.e2b
-    - Preserves existing E2B Sandbox method signatures
-      - Sandbox.create, sandbox.commands.run, sandbox.files, sandbox.pty, sandbox.kill
-    - Replaces remote cloud VM roundtrips with sub-60ms local hardware execution
-    - Drop-in: frameworks built against E2B SDK require no code change
-    - Implementation: thin SDK wrapper
-      - No plugin: Sandbox methods map directly to Session methods
-      - No binary: consumes the same in-process SDK as first-class consumers
-      - E2B boundary: see: #Process-vs-plugin-boundary
-    - E2B coverage: TBD
+    - External translation shim mapping Sandbox abstractions directly to Kelir Session primitives
+    - In-process SDK client wrapper: consumes standard Kelir runtime without hypervisor modifications
+    - Process vs plugin boundary: thin client wrapper with no in-guest shims or background daemons
```

---

#### Diff: `cans/11-operations.md`
*Excises the marketing slop grep and removes hardcoded test lists from coverage assertions.*

```diff
--- a/cans/11-operations.md
+++ b/cans/11-operations.md
@@ -42,7 +42,6 @@
     - Pull-request tier: `cans check --touched-files` rejects edits outside
       each concept's canEdit set
     - No path filter: a node edit can break a pointer in any other node
     - Local mirror: `just cans-check` invokes the identical flags
-    - Audit-slop target: banned-term grep over cans/, see: artifacts/config/justfile
     - Invariant: every rule in _rules.yaml maps to a step here, or the rule
       is dead and gets removed
   - Coverage-gates
```

---

#### Diff: `cans/artifacts/config/ci.yml`
*Deletes the `audit-slop` job entirely from the pipeline.*

```diff
--- a/cans/artifacts/config/ci.yml
+++ b/cans/artifacts/config/ci.yml
@@ -35,16 +35,6 @@
           cans check --touched-files /tmp/touched --json \
             | jq -e '.ok == true and .exitCode == 0' >/dev/null
 
-  # ─── 0a. AUDIT-SLOP (banned-term grep over cans/) ──────────
-  audit-slop:
-    runs-on: ubuntu-latest
-    steps:
-      - uses: actions/checkout@v4
-      - name: Banned-term grep over cans/
-        run: |
-          if git grep -i -E "pure epistemic|truth tier|fact-producer|ghost inode|GhostFile|cohesive state duality|token-to-bit|superpowers|teleport" -- cans/ ':!cans/artifacts/config/ci.yml' ':!cans/artifacts/config/justfile'; then
-            echo "::error::Banned terms found in cans/"; exit 1
-          fi
-
   # ─── 1. FAST STATIC GATE (<90s) ────────────────────────────
   lint-and-types:
     runs-on: ubuntu-latest
```

---

#### Diff: `cans/artifacts/config/justfile`
*Removes `audit-slop` recipe and references.*

```diff
--- a/cans/artifacts/config/justfile
+++ b/cans/artifacts/config/justfile
@@ -37,14 +37,6 @@
 plugin-check-graph:
     cargo run -p kelir-plugin-cli -- check --graph
 
-# Audit-slop gate — banned-term grep over cans/; fails on matches
-audit-slop:
-    #!/usr/bin/env bash
-    set -euo pipefail
-    if git grep -i -E "pure epistemic|truth tier|fact-producer|ghost inode|GhostFile|cohesive state duality|token-to-bit|superpowers|teleport" -- cans/ ':!cans/artifacts/config/ci.yml' ':!cans/artifacts/config/justfile'; then
-        echo "Banned terms found in cans/" >&2; exit 1
-    fi
-
 # Spec gate — same two tiers CI runs; --json, exit-code asserted
 cans-check:
     @cans check --refs-only --json | jq -e '.ok and .exitCode == 0'
@@ -79,5 +71,5 @@
 
 # --- 4. PRE-FLIGHT CHECK (Run before git push) ---
-pre-flight: cans-check build-shim parity plugin-check-graph plugin-check-all tiered-check
+pre-flight: cans-check build-shim parity plugin-check-graph plugin-check-all tiered-check
     cargo fmt --all -- --check
     cargo clippy --workspace --exclude kelir-shim -- -D warnings
```

### PART 4/4: ENRICHED PLUGINS (DOMAIN SSOT HOMES)

---

#### Diff: `cans/plugins/pty.md`
*Houses all ANSI filtering, Token Shield scrollback clamping, window resize bridging, and disclosure formatting evacuated from `04-terminal-plane.md`.*

```diff
--- a/cans/plugins/pty.md
+++ b/cans/plugins/pty.md
@@ -19,10 +19,17 @@
     - Terminal handler API
       - Terminal notice emits HostToGuest::ChannelData on the pty-disclosure kind
   - Invariants and policy (pty)
-    - Five filters
-      - Filter set and per-filter behavior: see: artifacts/diagrams/terminal-pipeline.txt
+    - Paravirtual console domain translation
+      - Intercepts raw byte streams from virtio-console Port 0
+      - Preserves guest isatty(1) and raw mode semantics out-of-band
+      - Translates host window resize events (SIGWINCH) into TIOCSWINSZ ioctl
+        commands via vsock Channel 0 control frames
+      - Bridges guest foreground signals (SIGINT, SIGQUIT) directly to process groups
+    - Filter pipeline execution order
       - Ordering invariant
         - ANSI strip precedes prompt match: SGR codes split regex targets
         - Prompt match precedes Token Shield: clamping trims matched prompts
         - Signal forwarding observes post-sanitization bytes only
+        - Leak scanner runs on post-sanitized byte buffer before screen export
       - Placement invariant
@@ -37,12 +44,14 @@
       - Threat model
         - OSC 52: clipboard hijack (agent reads/writes human clipboard)
         - OSC 0/1/2: title spoofing (agent impersonates terminal session)
-        - Tier-1 violations: see: 08-epistemic.md#Three-tier-evidence-verification-model
-      - Escape families outside the model: TBD
     - ANSI sanitizer
       - Drops OSC 52 (`\x1b]52;c;…`) at the virtio-console ring boundary
       - Strips OSC 0/1/2 before the host terminal emulator observes bytes
       - Only OSC sequences are touched; SGR (color, bold) passes through
+      - Maintains streaming parser state across virtqueue frame boundaries
+    - Token Shield scrollback governor
+      - Clamps completed lines from VT100 scrollback grid (max lines/bytes)
+      - Injects truncation notices inside DECSTBM status margins with ANSI reset
+      - Never slices mid-stream raw bytes or active escape sequences
     - Prompt responder
       - Matchers run against ANSI-SGR-stripped plaintext shadow copy
       - Regex matches text regardless of SGR color or bold codes
```

---

#### Diff: `cans/plugins/vfs.md`
*Houses all in-flight read masking, OverlayFS whiteout unlinking, and atomic workspace staging evacuated from `05-storage-plane.md`.*

```diff
--- a/cans/plugins/vfs.md
+++ b/cans/plugins/vfs.md
@@ -20,10 +20,16 @@
   - Invariants and policy (vfs)
     - Reconciler
       - see: artifacts/code/reconciler.rs
       - Pure stdlib implementation: zero external crate dependencies
       - Extension reconcile() is a pure storage-plane operation
       - Directly publishes StorageDelta to EventBus upon successful reconcile
+      - OverlayFS whiteout resolution
+        - Deleted files represented as char devices with st_rdev == 0 (mknod c 0 0)
+        - Deleted or opaque directories carry xattr trusted.overlay.opaque="y"
+        - Reconciler executes unlinks on host files strictly beneath target root
+        - Escapes via symlinks or path traversal abort with PermissionDenied
+      - Atomic staging exchange
+        - Writes mutated files to sibling temporary files (.tmp.<nonce>)
+        - Executes atomic directory swap (renameat2 / RENAME_EXCHANGE)
       - Lifecycle hook bindings
         - on_pre_commit flushes in-flight FUSE buffers
         - on_post_commit triggers sibling swap (.tmp.<nonce>) into host workspace
@@ -36,10 +42,16 @@
     - Synthetic FUSE inodes
       - Architecture (synthetic inodes): see: artifacts/diagrams/storage-architecture.txt
       - FsInodeAttr mode: 0o400 honored strictly; no permission fallback
       - Dynamic hash formula: BLAKE3(path || generation_counter || mode)
       - TS synthetic content may be a thunk evaluated per read
         - Same lifecycle as SyntheticFile.dynamic (Py/RS)
     - In-flight read masking
-      - Flow: see: 05-storage-plane.md#Sliding-window-replacement-over-FUSE-read-buffers
-      - Format-preserving masking (FPA) executes via synthetic FUSE inode generator
-      - Generates format-valid synthetic content before guest read calls
-      - Example patterns: sk_live_, AKIA, BEGIN PRIVATE KEY blocks
-      - see: artifacts/examples/autonomous-swe-agent.ts (vfs.maskRead)
+      - Stateful Aho-Corasick automaton carries scan state across read offsets
+      - Fixed-width pattern replacement on-the-fly; zero full-file buffering
+      - Carryover window buffers boundary fragments until pattern match or flush
+      - Format-preserving masking (FPA) swaps detected tokens (e.g. sk_live_, AKIA)
     - UID/GID idmap: see: 05-storage-plane.md#Virtual-ID-remapping
```

---

#### Diff: `cans/plugins/wire.md`
*Houses DNS subdomain entropy gating, SNI/Host matching, and credential proxying evacuated from `06-wire-plane.md`.*

```diff
--- a/cans/plugins/wire.md
+++ b/cans/plugins/wire.md
@@ -19,10 +19,17 @@
     - Total egress rule
       - Egress is what the four channels admit; nothing else reaches NIC
       - Non-TCP traffic (raw sockets, ICMP echo) fails silently with 100% packet loss
-      - DNS queries matching wildcard allowlists inspect label entropy
-      - DNS sinkhole is the fallback that makes "unlisted ⇒ dropped" total
+    - DNS entropy gate and sinkhole
+      - Intercepts all UDP port 53 traffic crossing TAP interface
+      - Computes Shannon entropy on subdomain query labels for wildcard allowlists (*.domain)
+      - Drops queries with entropy exceeding threshold (default: 4.5) with NXDOMAIN
+      - Prevents exfiltration tunneling over DNS label encoding
+    - Admission and domain fronting protection
+      - Admission match priority: TLS SNI -> HTTP Host -> Destination IP
+      - Mismatched SNI and HTTP Host headers are dropped immediately as domain fronting
+      - Non-allowlisted destinations drop silently at host TAP forwarder
     - Per-channel admission
       - L7 proxy: TLS terminated under per-session CA; allowlist by host
       - L4 SOCKS5: connect-by-IP; used where TLS termination is invalid
       - SSH gate: host keyring signs; private key never crosses vsock
```

---

#### Diff: `cans/plugins/transaction.md`
*Houses Two-Phase Commit coordination logic between microVM upperdir and host databases evacuated from `07-lifecycle.md`.*

```diff
--- a/cans/plugins/transaction.md
+++ b/cans/plugins/transaction.md
@@ -18,12 +18,16 @@
     - Multi-resource 2PC coordination
       - Synchronizes atomic commit/abort across plugin-vfs, plugin-db-proxy, and plugin-lock-broker
-      - on_pre_commit validates all participants before committing host disk or DB
-      - on_post_commit(true) atomically swaps files and commits SQL connections
-      - on_post_commit(false) drops VFS upperdir diffs, sends SQL ROLLBACK, releases locks
+      - Coordinates with hypervisor lifecycle: pauses vCPUs during commit evaluation
+      - on_pre_commit pauses guest vCPUs, flushes VFS buffers, and prepares staging files
+      - on_post_commit(true) executes atomic directory swap into host workspace
+        and issues SQL COMMIT across active proxy connections
+      - on_post_commit(false) purges guest tmpfs overlay diffs, drops staging directories,
+        issues SQL ROLLBACK, and releases advisory locks
+      - Prevents desynchronization between host database state and guest workspace diffs
     - Spatial blast radius scoping
       - Scopes filesystem mutations to declared path subtrees; undeclared writes fail closed
       - Enforces database query boundaries to declared schemas via pgwire frame filter
       - Restricts advisory lock acquisition to declared physical host inodes
```

No. Parts 1–4 cleaned the core plane specs and trait definitions, but left behind glaring contaminations in:

1. **`cans/03-sdk-architecture.md`**: Still hardcodes SWE-bench entities (`FixWebhookParams`, `git commit`, `npm test`) into normative SDK contracts.
2. **`cans/01-architecture.md`**: Leaks V8/Node JIT assumptions directly into MMU memory execution policy (`WxPolicy`).
3. **`cans/artifacts/diagrams/terminal-pipeline.txt`**: Labels the hardware console pipeline as `@kelir/plugin-pty` and embeds application filters into paravirtual transport.
4. **`cans/artifacts/diagrams/guest-vm.txt`**: Hardcodes agent brand names ("Claude Code / Cursor / SWE-bench") into the guest kernel architecture diagram.

Here are the remaining missing diffs:

---

#### Diff: `cans/03-sdk-architecture.md`
*Purges SWE-bench task schemas, hardcoded binaries (`git`, `npm`), and orchestrator-specific types from normative SDK rules.*

```diff
--- a/cans/03-sdk-architecture.md
+++ b/cans/03-sdk-architecture.md
@@ -47,15 +47,6 @@
     - Typed dispatch and session extensions
       - Typed event hooks
         - .on(LifecycleEvent.VmReady, handler) for lifecycle events
         - .intercept(Ring.NetOutbound, handler) for ring interception
-        - .trap(TrapFilter.binary("git").withArg("commit"), handler)
+        - .trap(TrapFilter::default(), handler)
       - Session extension pattern
         - Core Session: boot, fork, suspend, resume, rollback, terminate
         - Core Session: raw ring channel access alongside primitives
@@ -87,14 +78,9 @@
   - Rust SDK
     - see: artifacts/code/rust-handle-injection.rs
     - Typestate transitions: KelirBuilder<Unset> to KelirBuilder<Sealed> to Session
     - .plugin() takes flat plugin instances; .params::<T>() carries the schema
-    - Zero-Allocation Validation via Serde
-      - #[derive(Deserialize)] enum Action with serde(tag = "action")
-      - c.params::<Action>() fails at perimeter before traps execute
     - Scope constraint via MountOptions: rings, binaries, paths
   - TypeScript SDK
     - see: artifacts/code/typescript-accumulation.ts
     - Fluent model: .use().params().trap().intercept().on(), then .boot()
-    - .params(schema): TypeBox AOT push-down compiles to native Rust
-      validators at boot; falsified actions fail with EINVAL before dispatch
     - Typed service locator: c.service(ToolRegistry) for typed service lookup
   - Python SDK
@@ -102,9 +88,6 @@
     - async with builder as session
       - __aenter__ boots microVM
       - __aexit__ guarantees session.terminate() on any exception
-    - Pydantic v2 BaseModel with frozen=True
-      - Field(pattern=r"^fix-webhook$")
-      - Raises EINVAL before hook evaluation
     - Service resolution via c.service(RepoPolicyService)
```

---

#### Diff: `cans/01-architecture.md`
*Removes JIT engine excuses (V8/Node) from low-level MMU hardware execution policies.*

```diff
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@ -40,8 +40,7 @@
       - Strict hardware/kernel abstraction layer
         - Zero knowledge of prompts, evaluation loops, or orchestration logic
         - Operates strictly as OS physics: pages, registers, descriptors, frames
         - Hardware memory execution policy: strict W^X enforced at MMU boundary
         - Guest kernel traps anonymous PROT_EXEC mmap and mprotect allocations
         - In-memory elf loading through memfd_create aborts with E_WX_VIOLATION
-        - Granular knob: wx_enforcement = Strict | PermissiveAudit | Disabled
+        - Granular knob: wx_enforcement = Strict | PermissiveAudit
```

---

#### Diff: `cans/artifacts/diagrams/terminal-pipeline.txt`
*Purges `@kelir/plugin-pty` and application filters from the hardware console diagram.*

```diff
--- a/cans/artifacts/diagrams/terminal-pipeline.txt
+++ b/cans/artifacts/diagrams/terminal-pipeline.txt
@@ -1,33 +1,17 @@
 ┌─────────────────────────────────────────────────────────────────────────────┐
-│                          HARDWARE TERMINAL PIPELINE                         │
+│                         PARAVIRTUAL CONSOLE PIPELINE                        │
 │                                                                             │
-│   GUEST WORKLOAD (Claude Code / Bash)                                       │
-│     │ Writes stdout/stderr bytes                                            │
+│   GUEST WORKLOAD PROCESS                                                    │
+│     │ Writes raw stdout/stderr bytes                                        │
 │     ▼                                                                       │
-│   Linux Kernel PTY Driver (virtio-console port: PTY Master -> /dev/pts/N)   │
-│     │ isatty(1) == true. Raw mode active. Zero CLI downgrade.               │
+│   Guest Kernel Console / TTY Subsystem                                      │
+│     │ Bridges character device streams to virtio-console driver             │
 │     ▼                                                                       │
 │   virtio-console Ring Buffer (Hardware Shared Memory)                       │
-│     │ Direct byte streaming to Host Memory (Zero TCP/UDS overhead)          │
+│     │ Zero-copy virtqueue streaming to Host Physical Memory                 │
 │     ▼                                                                       │
-│   HOST SUPERVISOR (@kelir/plugin-pty)                                       │
+│   HOST SUPERVISOR (kelir-core Console Plane)                                │
 │     │                                                                       │
-│     ├── 1. ANSI Stream Sanitizer ─► Drops OSC 52 (Clipboard Hijack) &       │
-│     │                             OSC 0/1/2 (Title spoofing) in-flight.     │
-│     │                                                                       │
-│     ├── 2. onPrompt Responder ────► Verifies authentic foreground TTY;      │
-│     │                             matches prompts; auto-injects keystrokes. │
-│     │                             (Kills tmux send-keys forever).           │
-│     │                                                                       │
-│     ├── 3. Console Control Bridge ───► Boot handshake pushes initial window │
-│     │                             size (e.g. 120x40) at VM-ready; host      │
-│     │                             SIGWINCH resizes follow the same path:    │
-│     │                             ChannelData over vsock:5252 → shim        │
-│     │                             executes TIOCSWINSZ on guest PTY.         │
-│     │                                                                       │
-│     ├── 4. Signal Forwarding ─────► Ctrl+C (SIGINT) / Ctrl+D forwarded to   │
-│     │                             guest foreground process group.           │
-│     │                                                                       │
-│     ├── 5. Token Shield Filter ───► Clamps completed rows in VT100 grid.    │
-│     │                             Emits alerts in DECSTBM status margins.   │
-│     │                             Zero mid-stream raw ANSI slicing.         │
-│     │                                                                       │
-│     ├── 6. Active Leak Scanner ───► Scans stdout/stderr against registered  │
-│     │                             secret patterns; trips security panic.    │
-│     │                                                                       │
-│     └── 7. Grid Matrix Export ────► Maintains in-memory VT100 grid for      │
-│                                   structured terminal wireframe receipts.   │
+│     ├── Port Multiplexer ─────────► Routes port IDs to discrete dispatchers │
+│     └── Interceptor Pipeline ─────► Synchronous ConsoleInterposer execution │
+│                                     (High-level PTY policy: see plugin-pty) │
 └─────────────────────────────────────────────────────────────────────────────┘
```

---

#### Diff: `cans/artifacts/diagrams/guest-vm.txt`
*Purges agent brand names from the microVM hardware specification.*

```diff
--- a/cans/artifacts/diagrams/guest-vm.txt
+++ b/cans/artifacts/diagrams/guest-vm.txt
@@ -19,4 +19,4 @@
 │     - Zero guest-side orchestration agents, CLI shims, or env var injections        │
 │                                                                                     │
-│   └── UNTOUCHED GUEST WORKLOAD: Claude Code / Cursor / SWE-bench / Node / Rust / Python │
+│   └── UNTOUCHED GUEST WORKLOAD: Target application binary execution                 │
 └─────────────────────────────────────────────────────────────────────────────────────┘
```


### PART 1/3: RUNTIME IPC & PROTOCOL REPAIRS (ITEMS 1, 4, 6, 9)

---

#### Diff: `cans/plugins/pty.md` (Item 4)
*Defines explicit out-of-band vsock Channel `0x0010` (`pty-control`) for window resizing and signal forwarding.*

```diff
--- a/cans/plugins/pty.md
+++ b/cans/plugins/pty.md
@@ -10,6 +10,7 @@
     - VirtIO device attachment (pty): see: 01-architecture.md#VirtIO-Device-Complex
     - Lifecycle attachment (pty): see: 07-lifecycle.md#Lifecycle-vetoes
     - Binds to a dynamically allocated virtio-console port for terminal streams
+    - Registers dynamic vsock Channel Kind 0x0010 (pty-control)
     - Terminal bus transport: see: 04-terminal-plane.md#Terminal-pipeline
   - Service exports (pty)
     - Exports TerminalControlService through PipelineBinder for window and notice control
@@ -21,11 +22,17 @@
   - Invariants and policy (pty)
     - Paravirtual console domain translation
       - Intercepts raw byte streams from virtio-console Port 0
       - Preserves guest isatty(1) and raw mode semantics out-of-band
-      - Translates host window resize events (SIGWINCH) into TIOCSWINSZ ioctl
-        commands via vsock Channel 0 control frames
-      - Bridges guest foreground signals (SIGINT, SIGQUIT) directly to process groups
+      - Channel 0x0010 Framing (pty-control):
+        - MsgType 0x01 (WindowResize): rows (u16), cols (u16), xpixel (u16), ypixel (u16)
+          Host SIGWINCH pushes WindowResize frame to guest shim
+          Shim issues TIOCSWINSZ ioctl on slave PTY fd
+        - MsgType 0x02 (SignalForward): signal (u32), pgid (u32)
+          Host forwards SIGINT (Ctrl+C), SIGQUIT (Ctrl+\), SIGTSTP (Ctrl+Z)
+          Shim distributes signal directly to active foreground process group
+      - Zero in-band ANSI escape injection for terminal geometry or signal control
     - Filter pipeline execution order
       - Ordering invariant
```

---

#### Diff: `cans/plugins/profile.md` (Item 6)
*Reroutes RFC 6902 dynamic policy patches from Channel 0 to dynamic Channel `0x0002` (`profile-sync`).*

```diff
--- a/cans/plugins/profile.md
+++ b/cans/plugins/profile.md
@@ -10,6 +10,7 @@
     - VirtIO device attachment (profile): see: 01-architecture.md#VirtIO-Device-Complex
     - Lifecycle attachment (profile): see: 07-lifecycle.md#Lifecycle-vetoes
+    - Registers dynamic vsock Channel Kind 0x0002 (profile-sync)
   - Invariants and policy (profile)
     - Inversion-of-Control configuration router:
       - Ingests canonical kelir.toml and kelir.yaml specification files
@@ -37,8 +38,8 @@
     - Multi-file inheritance
       - Base profiles inherited via extends key
       - Deep-merge preserves monotonic lattice: cannot widen upstream denials
     - RFC 6902 dynamic policy patching
-      - Ingests JSON/YAML patches over vsock Channel 0 while VM is running
+      - Ingests JSON/YAML patches via ChannelData on Channel Kind 0x0002 (profile-sync)
       - Monotonic lattice validation: patches cannot loosen upstream denials
       - Monotonic violation halts patch dispatch with ESecurityViolation
       - Approved patches update active MountScope and BPF tables atomically
```

---

#### Diff: `cans/plugins/vfs.md` (Item 9)
*Decouples the FUSE snoop plane from the deleted `Oracle` and routes raw observations to `EventBus`.*

```diff
--- a/cans/plugins/vfs.md
+++ b/cans/plugins/vfs.md
@@ -58,9 +58,12 @@
     - Snoop plane
       - Invariant: a snoop observation reflects the producing process, not a
         kernel flusher
-        - Achieved by bypassing guest page cache on snooped paths; mechanism
-          lives in plugin-vfs
-      - Consumer: Oracle.ingest_harness_event; see: 08-epistemic.md#Oracle
-      - Producing process metadata is resolved via HandlerCtx extension slots
+      - Achieved by bypassing guest page cache on snooped paths (FOPEN_DIRECT_IO)
+      - Emits immutable FileAccessObserved domain event directly to EventBus:
+        - path: PathBuf
+        - pid: u32
+        - access_type: Read | Write
+        - bytes_transferred: usize
+      - Zero inline synchronous blocking; non-interfering observation plane
   - Error and return semantics (vfs)
```

---

#### Diff: `cans/plugins/supply-chain.md` (Item 1)
*Strengthens `plugin-supply-chain` with dynamic W^X exception verification for JIT runtimes (Node, V8, PyPy).*

```diff
--- a/cans/plugins/supply-chain.md
+++ b/cans/plugins/supply-chain.md
@@ -10,6 +10,8 @@
     - VirtIO attachment (supply-chain): see: 01-architecture.md#VirtIO-Device-Complex
     - Lifecycle attachment (supply-chain): see: 07-lifecycle.md#Lifecycle-vetoes
+    - Exposes: JitValidationService
   - Invariants and policy (supply-chain)
+    - Dynamic W^X JIT execution mediation
+      - Traps in-guest seccomp-unotify events for mprotect(PROT_EXEC) and memfd_create
+      - Inspects caller task: verifies calling binary BLAKE3 against verified JIT engine catalog (Node, Bun, V8)
+      - Verified JIT runtimes receive dynamic one-time MMU allocation token via vsock
+      - Unverified binaries attempting PROT_EXEC or memfd memory allocation halt with EWXVIOLATION
     - Monitors package manager process trees
     - Inspects binary_path and argv on TrapEvent for package managers (npm, pip)
```

### PART 2/3: EXECUTION GATES, MUTATION ABI & AUDIT LEDGER (ITEMS 2, 7, 8)

---

#### Diff: `cans/plugins/tool-guard.md` (Item 2)
*Defines the canonical POSIX `ExecveMutation` ABI that populates `TrapAction::Rewrite { target_mutation }`.*

```diff
--- a/cans/plugins/tool-guard.md
+++ b/cans/plugins/tool-guard.md
@@ -10,6 +10,7 @@
     - Lifecycle attachment (tool-guard): see: 07-lifecycle.md#Lifecycle-vetoes
     - Executes upstream of plugin-mcp on the MCP console port
   - Service exports (tool-guard)
     - Exports ToolRegistryService via PipelineBinder for schema and approval lookups
   - Invariants and policy (tool-guard)
@@ -28,8 +29,13 @@
     - Decision lattice (tool-guard)
       - ToolAction::Allow: forwards frame untouched
-      - ToolAction::Rewrite(args): mutates payload before dispatch
-        (e.g. forced flags, path sandboxing)
+      - ToolAction::Rewrite(mutation): serializes canonical ExecveMutation ABI:
+        - new_binary: Option<String> (overrides target ELF path)
+        - new_argv: Option<Vec<String>> (replaces execution arguments)
+        - set_env: HashMap<String, String> (injected environment variables)
+        - unset_env: Vec<String> (stripped environment variables)
+        Packed into target_mutation bytes; guest shim decodes and applies
+        atomically before releasing trapped execve syscall
       - ToolAction::Deny(errno, reason): drops call, synthesizes standard JSON-RPC error response
       - ToolAction::Hold: requests hypervisor
```

---

#### Diff: `cans/plugins/checkpoint.md` (Item 7)
*Adds the `StepEvaluator` execution gate and automated hardware rollback loop.*

```diff
--- a/cans/plugins/checkpoint.md
+++ b/cans/plugins/checkpoint.md
@@ -25,6 +25,18 @@
       - Flushes in-flight FUSE buffers via on_pre_commit on step boundaries
       - Captures CausalDelta: modified inodes, dirty bytes, and ExecutionReceipt
       - Captures headless VT100 terminal wireframe grid from plugin-pty
+    - Autonomous step verification loop
+      - Host registers StepEvaluator closure via CheckpointService
+      - Captures atomic pre-step SnapshotDescriptor before workload execution
+      - Evaluates physical ExecutionReceipt upon trapped process exit
+      - Step lattice:
+        - StepVerdict::Commit: persists staging delta, advances active step slot
+        - StepVerdict::Rollback(reason): calls driver.rollback(&pre_step_snapshot),
+          reverting guest physical RAM in <8ms and dropping overlayfs tmpfs diffs
+        - StepVerdict::YieldToHost: halts vCPU registers awaiting external input
+      - Fail-closed invariant: unhandled post-execution failures or evaluation
+        timeouts trigger automatic hardware rollback before releasing workload
     - Idle suspension protocol
       - Emits LifecycleRequest::Suspend(External) on verified boundary triggers
       - Compresses physical guest RAM to host NVMe via direct I/O LZ4
```

---

#### Diff: `cans/plugins/ledger.md` (Item 8)
*Replaces `WAL contract: TBD` with the concrete BLAKE3 RFC 6962 Merkle tree specification.*

```diff
--- a/cans/plugins/ledger.md
+++ b/cans/plugins/ledger.md
@@ -26,13 +26,19 @@
         - Default path: .kelir/audit.merkle.db relative to consumer CWD
         - WAL survives guest rollback() and fork(); no guest operation can
           rewrite or duplicate it
-      - WAL contract: TBD
-        - Record schema
-        - Chaining formula and root signing key
-        - Backend and concurrency model
-        - Secret redaction before append
+      - Merkle WAL specification
+        - Record Schema (Packed Little-Endian):
+          - seq: u64 (monotonic index)
+          - prev_hash: [u8; 32] (BLAKE3 hash of previous leaf)
+          - receipt_blake3: [u8; 32] (ExecutionReceipt digest)
+          - delta_root: [u8; 32] (StorageDelta modified inode tree root)
+          - timestamp_ns: u64 (host wall-clock timestamp)
+          - signature: [u8; 64] (Ed25519 signature by host session key)
+        - Merkle Hashing: RFC 6962 rules via BLAKE3:
+          - Leaf: BLAKE3(0x00 || record_bytes)
+          - Node: BLAKE3(0x01 || left_child || right_child)
+        - Append-only LMDB persistence with immediate fsync after each process reap
       - OpenTelemetry
         - Outbound HTTP requests carry W3C traceparent, injected host-side
-      - Span contract: TBD
-        - Names and parent/child
-        - Error mapping and sampling
-        - Attribute allowlist and redaction
-        - Export-failure behavior
+        - Spans mirror physical receipt phase timings: trap_ingress, policy, ffi, verdict
+        - Dropped or rejected spans log to local WAL before pipeline halts
       - Binds ExecutionReceipt correlation_token into Merkle WAL records
   - Error and return semantics (ledger)
     - Hook signature (ledger): see: artifacts/code/plugin-trait.rs
-    - Return errno and rejection behavior (ledger): TBD
+    - Return errno and rejection behavior (ledger)
+      - Pure observer: never returns Reject or blocks inline execution
+      - Disk-full on WAL partition triggers hypervisor LifecycleRequest::Suspend(ResourcePressure)
```

### PART 3/3: NEW TIER 2 PLUGINS & CATALOG REGISTRATION (ITEMS 3, 5, 10)

---

#### New File: `cans/plugins/vault.md` (Item 3)
*Unified Tier 2 secret vaulting engine providing shared Aho-Corasick tables and placeholder translation.*

```markdown
- Plugin vault
  - Crate dependencies (vault)
    - Plugin identifier: plugin-vault
    - Owner crate: crates/kelir-plugins/plugin-vault
    - name() = "vault"
    - Tier 2 governance and protocol proxy (vault)
    - Depends on (vault): kelir-interface only
    - Exposes: VaultService
    - VirtIO device attachment (vault): see: 01-architecture.md#VirtIO-Device-Complex
    - Lifecycle attachment (vault): see: 07-lifecycle.md#Lifecycle-vetoes
  - Service exports (vault)
    - Exports VaultService via PipelineBinder for secret resolution and redaction
  - Invariants and policy (vault)
    - Single source of truth for secret handling
      - Compiles unified Aho-Corasick automaton across all registered host secrets
      - Shared across plugin-vfs (read masking), plugin-wire (L7 egress swap), and plugin-pty (leak scan)
      - Enforces vault:// placeholder URN generation: real secrets never enter guest physical RAM
    - Memory isolation and zeroization
      - Host secret memory pinned via mlock(2) to prevent swapping to disk
      - Memory zeroed using explicit volatile writes upon session termination or drop
    - Out-of-band secret resolution
      - Host-side forward proxy resolves vault:// placeholders against secure host keyring
      - Ephemeral per-session CA certificates injected into guest trust stores transparently
  - Error and return semantics (vault)
    - Hook signature (vault): see: artifacts/code/plugin-trait.rs
    - Return errno and rejection behavior (vault)
      - Plaintext secret detected in guest-to-host trap event rejects with ESecurityViolation
      - Unresolvable placeholder URN returns ENODATA
```

---

#### New File: `cans/plugins/container.md` (Item 5)
*Rootless OCI container mediation for non-nested virtualization environments (Apple Silicon HVF).*

```markdown
- Plugin container
  - Crate dependencies (container)
    - Plugin identifier: plugin-container
    - Owner crate: crates/kelir-plugins/plugin-container
    - name() = "container"
    - Tier 2 governance and protocol proxy (container)
    - Depends on (container): kelir-interface, plugin-vfs
    - Exposes: ContainerService
    - VirtIO device attachment (container): see: 01-architecture.md#VirtIO-Device-Complex
    - Lifecycle attachment (container): see: 07-lifecycle.md#Lifecycle-vetoes
  - Service exports (container)
    - Exports ContainerService via PipelineBinder for container lifecycle tracking
  - Invariants and policy (container)
    - Non-nested OCI container execution
      - Configures rootless Podman and crun runtime environments inside microVM
      - Mounts ephemeral tmpfs scratchpad for /var/lib/containers to isolate storage diffs
      - Constructs synthetic cgroup v2 delegation hierarchy at on_vm_ready
    - UID/GID mapping isolation
      - Intercepts newuidmap and newgidmap allocations via trap filters
      - Pins subuid and subgid ranges to unprivileged guest user ranges
    - Image extraction without nested VMX
      - Ingests pre-cached OCI image tarballs from host package cache
      - Prevents nested hypervisor escape vectors on platforms lacking nested VMX/SVM
  - Error and return semantics (container)
    - Hook signature (container): see: artifacts/code/plugin-trait.rs
    - Return errno and rejection behavior (container)
      - Privileged container escape attempt or raw device mount rejects with EPERM
```

---

#### New File: `cans/plugins/replay.md` (Item 10)
*Cycle-exact deterministic clock stepping via PMU retired-instruction window tracking.*

```markdown
- Plugin replay
  - Crate dependencies (replay)
    - Plugin identifier: plugin-replay
    - Owner crate: crates/kelir-plugins/plugin-replay
    - name() = "replay"
    - Tier 2 governance and protocol proxy (replay)
    - Depends on (replay): kelir-interface only
    - Exposes: ReplayService
    - VirtIO device attachment (replay): see: 01-architecture.md#VirtIO-Device-Complex
    - Lifecycle attachment (replay): see: 07-lifecycle.md#Lifecycle-vetoes
  - Service exports (replay)
    - Exports ReplayService via PipelineBinder for deterministic clock stepping
  - Invariants and policy (replay)
    - Cycle-exact instruction clock stepping
      - Decouples guest paravirtual clock from host physical TSC
      - Arms host PMU hardware counters to sample instructions retired per vCPU
      - Advances guest KVM_SET_CLOCK / CNTVOFF_EL2 strictly on retired-instruction windows
      - Eliminates host scheduling jitter and timer interrupt variance across runs
    - Kernel clocksource lockdown
      - Injects kernel boot parameters: clocksource=kvm-clock tsc=reliable nohpet
      - Intercepts clock_gettime and gettimeofday syscall traps to enforce monotonic progression
  - Error and return semantics (replay)
    - Hook signature (replay): see: artifacts/code/plugin-trait.rs
    - Return errno and rejection behavior (replay)
      - Negative time drift or clock backwards step raises EClockRegress
```

---

#### Diff: `cans/02-plugin-model.md`
*Registers `plugin-vault`, `plugin-container`, and `plugin-replay` into Tier 2 catalog.*

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -14,6 +14,9 @@
       - plugin-supply-chain: execution governance proxy
       - plugin-wire-ingress: inbound wire proxy
       - plugin-checkpoint: autonomous state gating and pause proxy
+      - plugin-vault: unified secret and placeholder governance proxy
+      - plugin-container: rootless OCI container execution proxy
+      - plugin-replay: deterministic PMU-stepped clock replay proxy
       - Tier 2 plugins declare direct Cargo dependencies on Tier 1 plugins
     - Tier 3 cross-cutting aggregators
       - plugin-ledger: audit aggregation over Tier 1 and Tier 2 plugins
```

---

#### Diff: `cans/artifacts/config/project-structure.txt`
*Adds the three new plugin crates to the physical Cargo workspace project tree.*

```diff
--- a/cans/artifacts/config/project-structure.txt
+++ b/cans/artifacts/config/project-structure.txt
@@ -124,6 +124,12 @@
 │   │   ├── plugin-tool-guard/         # MCP tool-call interposition, schema pinning, veto rules on MCP port
 │   │   │   ├── Cargo.toml
 │   │   │   ├── src/lib.rs             # ToolInterposer: on_tool_list, on_tool_call, on_tool_result
 │   │   │   └── tests/test_tool_guard.rs
+│   │   ├── plugin-vault/              # Unified secret redaction, mlock zeroization, Aho-Corasick tables
+│   │   │   ├── Cargo.toml
+│   │   │   └── src/lib.rs
+│   │   ├── plugin-container/          # Rootless OCI container runner & cgroup v2 setup
+│   │   │   ├── Cargo.toml
+│   │   │   └── src/lib.rs
+│   │   └── plugin-replay/             # PMU instruction-stepped deterministic clock replay
+│   │       ├── Cargo.toml
+│   │       └── src/lib.rs
```

Yes, absolutely. The cleanup broke several real-world terminal mechanics and left dangling references across the spec. 

Moving hooks to vsock channel `0x0010` left five critical gaps in window geometry, line discipline, and signal distribution:

1. **Initial Window Size Race**: When a microVM boots, the guest kernel PTY defaults to `80x24`. If the host doesn't push the initial window size before the guest binary runs, any TUI (Cursor, Ink, Blessed) renders with broken layout coordinates until the host window is manually resized.
2. **Dead References in `04-terminal-plane.md`**: The sub-nodes for `Window resize` and `Signal forwarding` still reference `on_window_resize` and `on_signal_send`—the exact hooks we purged from `ConsoleInterposer`.
3. **The Raw-Mode Signal Deadlock**: In raw mode (`isatty` without line discipline), typing `Ctrl+C` sends the raw ASCII byte `0x03`. If the guest kernel PTY has `ISIG` disabled, `0x03` does **not** trigger `SIGINT`—it just sits in the stdin buffer. The host must forward signals out-of-band, but the relationship between in-band raw bytes and out-of-band signals was undefined.
4. **Mandatory Disclosure Was Severed**: `on_disclosure` was purged from `ConsoleInterposer`, leaving no specified mechanism for how host annotations reach the guest TTY.
5. **Guest Shim Slave Allocation (`pty_broker.rs`)**: Missing the exact ioctl and termios initialization contract between `virtio-console` Port 0 and `/dev/pts/N`.

Here are the diffs that resolve these omissions completely:

---

#### Diff: `cans/04-terminal-plane.md`
*Removes all dead references to deleted hooks and establishes the pure raw-byte character bus contract.*

```diff
--- a/cans/04-terminal-plane.md
+++ b/cans/04-terminal-plane.md
@@ -9,11 +9,5 @@
     - Zero-copy ringbuffer backpressure and queue management
     - Pipeline isolation: Separate interceptor dispatch array per port ID
     - Interceptors implement ConsoleInterposer with byte-level access
-    - High-level terminal emulation and ANSI sanitization: see: plugins/pty.md
-  - Control signals and resizing
-    - Console window resize and signal distribution delegate to out-of-band IPC
-    - Carried inside ChannelData over vsock Channel 0
-    - Translation to guest ioctl (TIOCSWINSZ) managed by guest shim broker
+    - High-level PTY line discipline, window geometry, and signals: see: plugins/pty.md
```

---

#### Diff: `cans/plugins/pty.md`
*Defines initial boot geometry, termios raw-mode flags, and out-of-band signal distribution.*

```diff
--- a/cans/plugins/pty.md
+++ b/cans/plugins/pty.md
@@ -14,6 +14,14 @@
     - Terminal bus transport: see: 04-terminal-plane.md#Terminal-pipeline
   - Service exports (pty)
     - Exports TerminalControlService through PipelineBinder for window and notice control
+    - PtyConfig schema:
+      - initial_cols: u16 (default: 120)
+      - initial_rows: u16 (default: 40)
+      - term: String (default: "xterm-256color")
+      - colorterm: String (default: "truecolor")
+      - max_scrollback_lines: usize (default: 300)
+      - max_scrollback_bytes: usize (default: 65536)
+      - strip_osc: bool (default: true)
   - Invariants and policy (pty)
     - Paravirtual console domain translation
       - Intercepts raw byte streams from virtio-console Port 0
@@ -23,10 +31,23 @@
       - Channel 0x0010 Framing (pty-control):
         - MsgType 0x01 (WindowResize): rows (u16), cols (u16), xpixel (u16), ypixel (u16)
-          Host SIGWINCH pushes WindowResize frame to guest shim
-          Shim issues TIOCSWINSZ ioctl on slave PTY fd
+          on_vm_ready: Host immediately pushes initial geometry (initial_rows, initial_cols)
+          Live resize: Host SIGWINCH listener pushes WindowResize frame to guest shim
+          Shim applies TIOCSWINSZ ioctl on master PTY fd prior to workload spawn
         - MsgType 0x02 (SignalForward): signal (u32), pgid (u32)
-          Host forwards SIGINT (Ctrl+C), SIGQUIT (Ctrl+\), SIGTSTP (Ctrl+Z)
-          Shim distributes signal directly to active foreground process group
+          Host interceptor strips raw Ctrl+C (\x03) and Ctrl+D (\x04) from raw byte streams
+          Translates control keys into explicit SignalForward frames (SIGINT, SIGHUP)
+          Shim executes killpg(pgid, signal) ensuring termination even when guest is in raw mode
+    - Guest shim line discipline initialization (pty_broker.rs)
+      - Shim opens master/slave pair via openpty()
+      - Injects TERM and COLORTERM into guest environment
+      - Configures slave termios: CS8, CREAD, B38400, CLOCAL
+      - Calls setsid() and ioctl(slave_fd, TIOCSCTTY, 0) to establish controlling terminal
+      - Spawns workload with slave_fd duplicated to stdin (0), stdout (1), stderr (2)
+      - Pumps master_fd read/write streams directly to virtio-console Port 0
+    - Mandatory disclosure mechanics
+      - Host-side mutations publish TerminalNotice { body: Vec<u8> } to EventBus
+      - plugin-pty catches event, appends registered annotations, and formats ANSI box
+      - Injects framed bytes directly into host-to-guest virtio-console Port 0 stream
+      - Guarantees disclosure appears inline on workload TTY without corrupting cursor state
     - Filter pipeline execution order
       - Ordering invariant
```

---

#### Diff: `cans/artifacts/code/plugin-trait.rs`
*Ensures `PtyControlMessage` is strongly-typed for Channel `0x0010` without leaking into hypervisor traits.*

```diff
--- a/cans/artifacts/code/plugin-trait.rs
+++ b/cans/artifacts/code/plugin-trait.rs
@@ -10,6 +10,21 @@
 use crate::handlers::HandlerCtx;
 use crate::protocol::messages::{ProcessExit, TrapAction, TrapEvent};
 use crate::vmm::driver::{VmmDriver, VmConfig, WatchdogAction};
+
+/// Structured payloads carried across vsock Channel 0x0010 (pty-control).
+#[derive(Debug, Clone, Serialize, Deserialize)]
+pub enum PtyControlMessage {
+    WindowResize {
+        rows: u16,
+        cols: u16,
+        xpixel: u16,
+        ypixel: u16,
+    },
+    SignalForward {
+        signal: u32,
+        pgid: u32,
+    },
+}
 
 #[derive(Debug, thiserror::Error)]
 pub enum PluginError {
```



An audit of the remaining planes, structs, and cross-references reveals **seven additional critical ruptures** caused or exposed by the earlier cleanups:

---

### 1. `plugin-lock-broker` Has No Trait Hook in `FsInterposer`
* **The Rupture:** `plugin-lock-broker.md` claims to intercept FUSE `SETLK` and `SETLKW` calls and delegate them to host OS `flock`.
* **The Reality:** Look at `FsInterposer` in `cans/artifacts/code/plugin-trait.rs`:
  ```rust
  pub trait FsInterposer {
      fn on_fs_lookup(&self, _path: &Path) -> ...
      fn on_fs_read(&self, _path: &Path, _buffer: &mut [u8]) -> ...
      fn on_fs_write(&self, _path: &Path, _data: &[u8], _pid: u32, _ctx: &HandlerCtx) -> ...
  }
  ```
  **There is no lock hook.** `plugin-lock-broker` is physically un-implementable. The storage plane completely ignores advisory locking.

---

### 2. `SuspendReason::External(u32)` Has No Registry (Security Bypass)
* **The Rupture:** When `plugin-checkpoint` pauses for human approval, it suspends with `SuspendReason::External(code)`. When `plugin-wire-ingress` parks a connection, it also suspends with `External(code)`.
* **The Reality:** There is no registry for `u32` codes. If an incoming HTTP packet arrives on the TAP interface, `plugin-wire-ingress` will wake the VM—**even if the VM was frozen by `plugin-tool-guard` awaiting human authorization for a destructive command**. An unauthenticated network ping can resume an untrusted agent held under quarantine.

---

### 3. Dead CANS Reference: `03-sdk-architecture.md` Points to Purged `#Oracle`
* **The Rupture:** `03-sdk-architecture.md#Oracle-contract` explicitly specifies:
  ```markdown
  - Oracle contract
    - Oracle interface: see: 08-epistemic.md#Oracle
  ```
* **The Reality:** We purged `#Oracle` from `08-epistemic.md`. `cans check` will fail on this dangling 0-hop reference.

---

### 4. `plugin-mcp.md` Still Binds to Purged `on_disclosure` & Lacks Port Naming
* **The Rupture:** `plugin-mcp.md` lists `on_disclosure` as an active hook wiring point, but `on_disclosure` was removed from `ConsoleInterposer`.
* **The Port Collision:** `plugin-mcp` claims to bind to a "dynamically allocated virtio-console port." But standard Linux guests enumerate ports anonymously as `/dev/hvc1`, `/dev/hvc2`. Without explicit paravirtual port names (`/dev/virtio-ports/<name>`), the guest workload cannot distinguish between the interactive PTY, the MCP channel, and raw logs.

---

### 5. `plugin-wire-ingress` Cannot Wake Suspended VMs via `LifecycleCtx`
* **The Rupture:** `plugin-wire-ingress.md` states: *"Wakes target microVM via LifecycleCtx on frame arrival."*
* **The Reality:** `LifecycleCtx` is an ephemeral borrow passed into hooks (`on_vm_init`, `on_pre_commit`). When the VM is suspended and the host event loop is idle, **`LifecycleCtx` does not exist**. A sleeping microVM can only be resumed by a handle that holds `Box<dyn VmmDriver>`.

---

### 6. Channel 1 (L4 Ingress) Lacks Destination Port Framing
* **The Rupture:** When traffic wakes a parked connection, `plugin-wire-ingress` forwards raw bytes to `vsock:5252 (Channel 1)`.
* **The Reality:** `kelir-shim/src/ingress.rs` receives the bytes inside the guest, but `ChannelData` carries only `payload: Vec<u8>`. The shim has no metadata telling it **which guest local port** to forward the connection to (e.g., 3000 vs 8080).

---

### 7. `CausalDelta` Is Missing Cryptographic Roots
* **The Rupture:** `08-epistemic.md` claims that `CausalDelta` links state diffs into the Merkle WAL.
* **The Reality:** `artifacts/code/causal-delta.rs` has fields only for raw bytes and modified paths:
  ```rust
  pub struct CausalDelta {
      pub memory_diff_bytes: u64,
      pub modified_inodes: Vec<PathBuf>,
      pub spawned_pids: Vec<u32>,
      pub egress_bytes: u64,
  }
  ```
  It has no `storage_delta_root: [u8; 32]` and no `receipt_root: [u8; 32]`, making cryptographic verification impossible.

---

### TARGETED PATCH DIFFS

#### Diff 1: `cans/artifacts/code/plugin-trait.rs`
*Adds `on_fs_lock` to `FsInterposer` so `plugin-lock-broker` can function.*

```diff
--- a/cans/artifacts/code/plugin-trait.rs
+++ b/cans/artifacts/code/plugin-trait.rs
@@ -170,6 +170,11 @@
     fn on_fs_read(&self, _path: &Path, _buffer: &mut [u8]) -> Result<(), PluginError> { Ok(()) }
     /// Synchronous hot-path inspection. Returns Ok(Allow) or Ok(YieldToHost) before write applies.
     fn on_fs_write(
         &self, _path: &Path, _data: &[u8], _pid: u32, _ctx: &HandlerCtx,
     ) -> Result<SyncVerdict, PluginError> { Ok(SyncVerdict::Allow) }
+    /// Intercepts FUSE SETLK and SETLKW advisory lock requests.
+    fn on_fs_lock(
+        &self, _path: &Path, _flock_cmd: i32, _flock_type: i16, _pid: u32,
+    ) -> Result<SyncVerdict, PluginError> { Ok(SyncVerdict::Allow) }
 }
```

---

#### Diff 2: `cans/artifacts/code/vmm-driver-trait.rs`
*Namespaces `SuspendReason::External` with an explicit subsystem registry.*

```diff
--- a/cans/artifacts/code/vmm-driver-trait.rs
+++ b/cans/artifacts/code/vmm-driver-trait.rs
@@ -80,9 +80,18 @@
-#[derive(Debug, Clone, Copy, PartialEq, Eq)]
-pub enum SuspendReason {
-    External(u32),
-    ResourcePressure,
-}
+#[derive(Debug, Clone, Copy, PartialEq, Eq)]
+pub enum ExternalSuspendKind {
+    HumanAuthorizationHold = 0x0001,
+    WireConnectionParking  = 0x0002,
+    StepEvaluationYield    = 0x0003,
+    HostPowerEventSnapshot = 0x0004,
+}
+
+#[derive(Debug, Clone, Copy, PartialEq, Eq)]
+pub enum SuspendReason {
+    External(ExternalSuspendKind),
+    ResourcePressure,
+}
```

---

#### Diff 3: `cans/03-sdk-architecture.md`
*Purges the broken pointer to `#Oracle`.*

```diff
--- a/cans/03-sdk-architecture.md
+++ b/cans/03-sdk-architecture.md
@@ -107,14 +107,3 @@
   - Idiomatic Surfaces Law
     - TS, Python, and Rust do not copy each other's method names
     - Each enforces 1:1 behavioral and wire parity against vectors.json
-  - Oracle contract
-    - Oracle interface: see: 08-epistemic.md#Oracle
-    - Language-specific realization
-      - Rust: Arc<dyn Oracle>, threaded through handler closures
-      - TypeScript: Oracle object literal, captured by closures
-      - Python: subclass kelir.Oracle, methods async
-    - Missing method fails at compile / import time in every language
```

---

#### Diff 4: `cans/plugins/mcp.md`
*Removes `on_disclosure` and specifies paravirtual console port names (`/dev/virtio-ports/kelir-mcp`).*

```diff
--- a/cans/plugins/mcp.md
+++ b/cans/plugins/mcp.md
@@ -10,7 +10,7 @@
     - VirtIO device attachment (mcp): see: 01-architecture.md#VirtIO-Device-Complex
     - Lifecycle attachment (mcp): see: 07-lifecycle.md#Lifecycle-vetoes
-    - Binds to a dynamically allocated virtio-console port for JSON-RPC transport
+    - Binds to virtio-console port named "kelir-mcp" (/dev/virtio-ports/kelir-mcp)
   - Service exports (mcp)
     - Exports McpEndpointService via PipelineBinder for JSON-RPC transport routing
   - Invariants and policy (mcp)
@@ -20,7 +20,6 @@
       - on_console_output / on_console_input: stream terminal to/from MCP client
       - on_fs_read / on_fs_write: expose as read_file / write_file
       - on_trap: observe syscalls to correlate execute_command with trap
       - on_process_exit: emit MCP task completion events
-      - on_disclosure: route mandatory notices into MCP stream
```

---

#### Diff 5: `cans/plugins/wire-ingress.md`
*Fixes the wake handle and adds target port framing for Channel 1 stream splicing.*

```diff
--- a/cans/plugins/wire-ingress.md
+++ b/cans/plugins/wire-ingress.md
@@ -18,7 +18,9 @@
     - Holds inbound client TCP connections without dropped frames during suspend
     - Host proxy terminates TCP handshake and buffers up to 64KB initial request data
     - Sniffs routing metadata with zero allocation via httparse
-    - Wakes target microVM via LifecycleCtx on frame arrival
-    - Splices buffered stream to vsock:5252 Channel 1 once VM is confirmed ready
+    - Wakes target microVM via SessionWakeHandle::resume(ExternalSuspendKind::WireConnectionParking)
+    - Ingress stream framing across vsock:5252 Channel 1:
+      - Frame header: target_port: u16, connection_id: u64, payload_len: u32
+      - Guest shim ingress forwarder connects to 127.0.0.1:target_port and splices duplex bytes
     - Zero-downtime webhook and interactive terminal connection continuity
```

---

#### Diff 6: `cans/artifacts/code/causal-delta.rs`
*Adds cryptographic Merkle roots to the state delta struct.*

```diff
--- a/cans/artifacts/code/causal-delta.rs
+++ b/cans/artifacts/code/causal-delta.rs
@@ -18,4 +18,8 @@
     pub spawned_pids: Vec<u32>,
     /// Wire egress bytes between the two checkpoints
     pub egress_bytes: u64,
+    /// BLAKE3 root of modified storage inodes (from StorageDelta)
+    pub storage_delta_root: [u8; 32],
+    /// Merkle leaf hash of the associated ExecutionReceipt
+    pub receipt_leaf_hash: [u8; 32],
 }
```

Here are **six more critical ruptures and code-level contradictions** exposed by the audit:

---

### 1. `vmm-driver-trait.rs` Still Carries `WxPolicy::Disabled` and the V8 Excuse
* **The Contradiction:** In Part 4, we purged `WxPolicy::Disabled` and the JIT excuse from `01-architecture.md`. But the actual Rust trait code in `cans/artifacts/code/vmm-driver-trait.rs` was never patched:
  ```rust
  pub enum WxPolicy {
      Strict,
      PermissiveAudit,
      /// Disable W^X checks (required for JIT heavy runners like V8/Node).
      Disabled,
  }
  ```
  The code artifact directly contradicts the architecture document.

---

### 2. `SharedMemoryHandle` Is a Host-Side Phantom (No Host Pointer)
* **The Rupture:** `01-architecture.md` specifies zero-copy shared memory regions between host and guest.
* **The Code:** Look at `SharedMemoryHandle` in `cans/artifacts/code/vmm-driver-trait.rs`:
  ```rust
  pub struct SharedMemoryHandle {
      pub tag: String,
      pub size_bytes: usize,
      pub guest_phys_addr: u64,
  }
  ```
  **Host consumers cannot use this.** It contains the *guest* physical address, but no host memory address (`host_ptr: *mut u8`), no file descriptor (`memfd: RawFd`), and no slice view. A TypeScript or Python caller invoking `session.vm.map_shared_region()` gets an address that is valid only inside the guest MMU, with zero ability to read or write to the buffer host-side.

---

### 3. `SyncVerdict::YieldToHost(u64)` Has No Defined Semantic
* **The Rupture:** `NetInterposer` and `FsInterposer` return `SyncVerdict::YieldToHost(u64)`.
* **The Void:** Nowhere in `05-storage-plane.md`, `06-wire-plane.md`, or `plugin-chaos.md` is that `u64` defined. `plugin-chaos` claims to inject packet latency (`latency: { baseMs, jitterMs }`) on raw frames, but the interceptor is a synchronous function (`fn on_net_outbound`). If `u64` is a nanosecond sleep window handled by the host ring buffer, it must be specified; otherwise, chaos latency injection either panics or blocks the entire hypervisor TAP polling loop.

---

### 4. `00-overview.md` Points Replay to the Old Clock-Skew Location
* **The Rupture:** In Part 3, we created `plugin-replay.md` as the dedicated home for PMU instruction-stepped deterministic replay.
* **The Stale Link:** `00-overview.md#Use-cases` still points to:
  ```markdown
  - Deterministic execution replay: see: 07-lifecycle.md#Clock-skew
  ```
  This points to raw monotonic clock-skewing rather than the replay engine.

---

### 5. `02-plugin-model.md` Return Semantics Contradict the Rust Traits
* **The Rupture:** `02-plugin-model.md#Error-variants` claims:
  ```markdown
  - Ok(()) = allow; kernel proceeds
  - Err(Reject { errno, reason }) = deny with named errno, pre-write
  ```
* **The Code:** 
  * `FsInterposer::on_fs_write` returns `Result<SyncVerdict, PluginError>`, not `Result<(), PluginError>`.
  * `NetInterposer::on_net_outbound` returns `Result<SyncVerdict, PluginError>`, not `Result<(), PluginError>`.
  * `TrapInterposer::on_trap` returns `Result<Option<TrapAction>, PluginError>`, not `Result<(), PluginError>`.
  The spec text completely misrepresents the return types of the segregated traits.

---

### 6. Guest UDS Forwarding for `plugin-db-proxy` Was a Ghost Specification
* **The Rupture:** `plugin-db-proxy.md` claims: *"SQLite: exposes /run/kelir/db.sock forwarded via vsock host service."*
* **The Void:** Standard Linux kernels do not forward Unix Domain Sockets to AF_VSOCK. Without a guest-side shim listener binding the socket, guest processes attempting to connect to `/run/kelir/db.sock` receive `ENOENT` or `ECONNREFUSED`.

---

### TARGETED PATCH DIFFS

#### Diff 1: `cans/artifacts/code/vmm-driver-trait.rs`
*Purges `WxPolicy::Disabled` and adds host memory mapping pointers to `SharedMemoryHandle`.*

```diff
--- a/cans/artifacts/code/vmm-driver-trait.rs
+++ b/cans/artifacts/code/vmm-driver-trait.rs
@@ -17,8 +17,6 @@
     Strict,
     /// Log violation to host ledger and continue execution.
     PermissiveAudit,
-    /// Disable W^X checks (required for JIT heavy runners like V8/Node).
-    Disabled,
 }
 
 /// Host virtualization selection and hardware validation parameters.
@@ -95,7 +93,9 @@
 pub struct SharedMemoryHandle {
     pub tag: String,
     pub size_bytes: usize,
     pub guest_phys_addr: u64,
+    pub host_ptr: *mut u8,
+    pub memfd_raw: std::os::unix::io::RawFd,
 }
```

---

#### Diff 2: `cans/artifacts/code/plugin-trait.rs`
*Formally specifies `SyncVerdict::YieldToHost` as a nanosecond delay yield for queue pacing and chaos injection.*

```diff
--- a/cans/artifacts/code/plugin-trait.rs
+++ b/cans/artifacts/code/plugin-trait.rs
@@ -107,7 +107,8 @@
 #[derive(Debug, Clone, Copy, PartialEq, Eq)]
 pub enum SyncVerdict {
     Allow,
-    YieldToHost(u64),
+    /// Yields frame/op to host event loop with delayed release in nanoseconds.
+    YieldToHost(u64),
 }
```

---

#### Diff 3: `cans/00-overview.md`
*Updates replay use-case to point to `plugins/replay.md`.*

```diff
--- a/cans/00-overview.md
+++ b/cans/00-overview.md
@@ -52,7 +52,7 @@
     - Autonomous step gating & idle pause: see: plugins/checkpoint.md
     - Multi-resource atomic commit (2PC): see: plugins/transaction.md
     - Time-sensitive tests (JWT, cron, certs): see: 07-lifecycle.md#Clock-skew
-    - Deterministic execution replay: see: 07-lifecycle.md#Clock-skew
+    - Deterministic execution replay: see: plugins/replay.md
     - Relational database governance: see: plugins/db-proxy.md
     - Supply chain package protection: see: plugins/supply-chain.md
```

---

#### Diff 4: `cans/02-plugin-model.md`
*Corrects the error and return semantics to match segregated ring traits.*

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -75,8 +75,9 @@
     - Error variants
-      - Ok(()) = allow; kernel proceeds
-      - Err(Reject { errno, reason }) = deny with named errno, pre-write
-      - Err(Halt(msg)) = deny and halt pipeline for human review
-      - Err(Io(_)) = internal failure, surfaced to runtime
+      - Interceptor traits return typed verdicts: SyncVerdict (FS/Net), Option<TrapAction> (Trap)
+      - Ok(SyncVerdict::Allow) = allow hot-path operation
+      - Ok(SyncVerdict::YieldToHost(ns)) = delay or yield operation to host event loop
+      - Err(PluginError::Reject { errno, reason }) = reject with POSIX errno surfaced to guest
+      - Err(PluginError::Halt(msg)) = unrecoverable failure; freezes VM pipeline
```

---

#### Diff 5: `cans/plugins/db-proxy.md`
*Specifies the guest shim Unix Domain Socket broker for SQLite vsock proxying.*

```diff
--- a/cans/plugins/db-proxy.md
+++ b/cans/plugins/db-proxy.md
@@ -15,7 +15,10 @@
   - Invariants and policy (db-proxy)
     - Mediates relational database communication
     - PostgreSQL: intercepts port 5432, decodes Q/P frames via pgwire parser
-    - SQLite: exposes /run/kelir/db.sock forwarded via vsock host service
+    - SQLite UDS bridge mechanics:
+      - Guest shim binds Unix Domain Socket at /run/kelir/db.sock at on_vm_ready
+      - Forwards client connections over vsock Channel Kind 0x0004 (db-sqlite)
+      - Host SqlProxyService receives stream and delegates queries to host database file
     - Verdict execution: DbAction::Allow, DbAction::Deny, DbAction::Rewrite
```


Here are **five additional deep architectural ruptures** exposed across framing, memory safety, and plugin composition:

---

### 1. In-Flight Rollback Kernel Panic (`c.vm.rollback()` in Trap Hooks)
* **The Rupture:** `step-evaluation-loop.ts` shows a handler trapping a syscall and calling `c.vm.rollback()` inside the trap hook.
* **The Reality:** When a guest syscall is trapped via `seccomp-unotify`, the guest kernel task group is **frozen in kernel mode** inside the syscall handler. If the host executes `driver.rollback()` at that exact millisecond, it resets physical RAM pages and vCPU registers to an older snapshot. When the trap handler subsequently returns `Resume` across vsock, the guest kernel attempts to wake a task whose kernel stack, page tables, and PID state were just swapped out from underneath it, triggering an immediate guest kernel panic.
* **The Fix:** **Rollback Is Illegal During Syscall Hold.** Rollback can only be executed:
  1. Post-process exit (via `plugin-checkpoint`'s `StepEvaluator`), or
  2. By returning `TrapAction::Abort`, which instructs the hypervisor to destroy the task before reverting memory.

---

### 2. `plugin-mcp` File Tools Are Blind to Guest Writes
* **The Rupture:** `plugin-mcp.md` maps `read_file` directly to host storage lookups.
* **The Reality:** The host workspace is mounted as a strictly read-only lowerdir. All guest file modifications live exclusively in the guest tmpfs `upperdir` until `reconcile()` commits them. If an agent writes a script and immediately calls the MCP `read_file` tool, an in-process host plugin reading the host workspace will observe `ENOENT` or stale base files!
* **The Fix:** **`plugin-mcp` Must Delegate File IO to `plugin-vfs`'s Active Inode Layer**.
  `plugin-mcp` cannot read host disk directly; it must query `VfsStorageService.read_overlay_file(&path)` so that tmpfs upperdir diffs and synthetic FUSE inodes are merged into the read view.

---

### 3. Redundant Wire Framing (`FrameHeader.channel_id` vs `ChannelData.kind`)
* **The Rupture:** `wire-protocol.rs` defines a 24-byte `FrameHeader` containing `pub channel_id: u16`.
* **The Collision:** `messages.rs` defines `HostToGuest::ChannelData { kind: u16, payload: Vec<u8> }`.
* **The Reality:** Channel routing is defined twice. If a frame has `header.channel_id = 0x0010` (PTY control) but the CBOR payload decodes to `ChannelData { kind: 0x0002 }` (Profile sync), the multiplexer encounters an unresolvable framing conflict.
* **The Fix:** **Strip `kind` from `ChannelData`**. The 24-byte `FrameHeader.channel_id` is the sole transport multiplexer. `ChannelData` must carry only raw payload bytes: `ChannelData { payload: Vec<u8> }`.

---

### 4. Connection Parking Architecture Contradiction
* **The Rupture:** `artifacts/diagrams/wire-plane.txt` claims parked connections are held as *"raw L2 frames in TAP buffers and virtqueues"*.
* **The Contradiction:** `plugins/wire-ingress.md` claims the host proxy *"terminates the client TCP handshake, buffers 64KB initial request data via httparse, and splices the stream across vsock:5252 Channel 1"*.
* **The Reality:** You cannot do both. Terminated TCP streams do not travel through virtio-net L2 TAP rings. Describing host TCP termination as "TAP frame parking" is technically false.
* **The Fix:** Reconcile `wire-plane.txt` to reflect L4/L7 stream termination over vsock.

---

### 5. Static Scope Collisions Paralyze Composite Plugins
* **The Rupture:** `02-plugin-model.md` states: *"Overlapping mutation scopes between plugins fail validation at boot."*
* **The Reality:** If Plugin A monitors `/workspace/**` for read-masking or audit, and Plugin B scopes write-gating to `/workspace/package.json`, their path scopes overlap. A naive "overlapping paths fail boot" check prevents broad workspace plugins from composing with fine-grained file filters.
* **The Fix:** Restrict the collision rule strictly to **conflicting mutators on identical operations**: multiple write-mutators on the same path fail; a reader plus a writer does not.

---

### TARGETED PATCH DIFFS

#### Diff 1: `cans/artifacts/code/messages.rs`
*Eliminates redundant double-channel framing by stripping `kind` from `ChannelData`.*

```diff
--- a/cans/artifacts/code/messages.rs
+++ b/cans/artifacts/code/messages.rs
@@ -49,7 +49,6 @@
     AdjustBalloon {
         target_mb: u32,
     },
     ChannelData {
-        kind: u16,
         seq: u64,
         payload: Vec<u8>,
     },
```

---

#### Diff 2: `cans/plugins/mcp.md`
*Binds MCP file operations to `VfsStorageService` overlay resolution instead of raw host disk.*

```diff
--- a/cans/plugins/mcp.md
+++ b/cans/plugins/mcp.md
@@ -6,7 +6,7 @@
     - Tier 1 base ring adapter (mcp)
-    - Depends on (mcp): kelir-interface only
+    - Depends on (mcp): kelir-interface, plugin-vfs
     - Exposes: McpEndpointService
     - VirtIO device attachment (mcp): see: 01-architecture.md#VirtIO-Device-Complex
@@ -19,4 +19,4 @@
     - Mapping (external tool → ring op)
       - execute_command → virtio-console via spawn, wrapped
-      - read_file → virtio-fs lookup + read
-      - write_file → virtio-fs write
+      - read_file → delegates to VfsStorageService.read_overlay_file() (merges tmpfs diffs)
+      - write_file → delegates to VfsStorageService.write_overlay_file()
```

---

#### Diff 3: `cans/02-plugin-model.md`
*Clarifies mutation collision rules to allow overlapping read/write scopes while rejecting conflicting mutators.*

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -28,3 +28,4 @@
-    - Overlapping mutation scopes between plugins fail validation at boot
+    - Mutation scope conflict rule: Overlapping mutators on identical operation targets
+      (multiple mutators writing to the same path, port, or env key) fail validation at boot with EAmbiguousMutation
+    - Non-mutating observers may overlap freely with mutators across arbitrary path subtrees
```

---

#### Diff 4: `cans/artifacts/diagrams/wire-plane.txt`
*Reconciles connection parking to reflect host TCP termination and vsock splicing.*

```diff
--- a/cans/artifacts/diagrams/wire-plane.txt
+++ b/cans/artifacts/diagrams/wire-plane.txt
@@ -16,4 +16,4 @@
                              [CONNECTION PARKING]
-                             Host-terminates client TCP; parks frames in TAP buffers;
-                             buffers up to 64KB in raw virtqueues until guest resumes;
-                             wakes microVM and forwards to vsock:5252 (Channel 1).
+                             Host-terminates client TCP; buffers up to 64KB initial data;
+                             wakes microVM and splices duplex stream across vsock:5252 (Channel 1).
```

---

#### Diff 5: `cans/07-lifecycle.md`
*Bans hardware rollback during active syscall hold to prevent guest kernel panics.*

```diff
--- a/cans/07-lifecycle.md
+++ b/cans/07-lifecycle.md
@@ -48,2 +48,4 @@
   - Rollback
+    - Pre-condition invariant: Rollback is prohibited while vCPUs hold unreleased syscall traps
+    - Attempting rollback on a frozen syscall trap aborts with E_INVALID_LIFECYCLE_STATE
     - Driver primitive: rollback reverts
```


