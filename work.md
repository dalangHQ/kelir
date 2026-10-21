You're right — my Part 1 draft was **not** valid CANS syntax. Three classes of error:

## Syntax errors in the draft

| My draft | Correct form | Rule |
|---|---|---|
| `see 02#KelirPlugin-trait` | `see 02-plugin-model.md#KelirPlugin-trait` | File `.md` extension required; `#` separates file from anchor |
| `see 08#Oracle` | *(removed — forward ref)* | `orphan_check: true` forbids pointers to non-existent nodes |
| `see 10#MCP-plugin` | *(removed — forward ref)* | Same; `#MCP-plugin` lands in Part 5 |
| `session.causal_delta(): see 08#Merkle-WAL` | `session.get_causal_delta_from_parent(): see 08-epistemic.md#Causal-delta` | Wrong anchor; `causal delta` has its own node, distinct from Merkle WAL |
| `Examples: Oracle impls` as pointer-bearing | `Examples: user-implemented Oracle` (prose only, no `see:`) | Naming a concept without a spec node must not look like a pointer |

**Confirmed correct in draft** (these were right): lowercase `see <target>` for internal cross-refs (per `00-overview.md`); capital `See: artifacts/...` for artifact refs (N/A in this patch); every remaining anchor maps to an existing node (`#VmmDriver-trait`, `#KelirPlugin-trait`, `#Static-dispatch`, `#Four-Ring-Bus`, `#Params-ingress-contract`, `#Guest-VM-anatomy`, `#Execution-receipts`, `#Step-evaluation-loop`, `#Wire-framing`, `#Wire-messages`, `#Trap-fast-path`, `#Env-injection`, `#EROFS-rootfs`, `#Reconciler`, `#Terminal-plane`, `#Wire-plane`, `#Merkle-WAL`, `#Causal-delta`, `#Project-structure` — all verified against the dump).

## Corrected Part 1

Also dropping `requires()` from `#Trait-surface` — the artifact `plugin-trait.rs` doesn't have it, so declaring it in the spec puts 02 and the artifact out of sync until Part 2 lands. Cleaner: `#Trait-surface` names `priority()` (Part 2 adds both); `requires()` appears only in `#Static-dispatch` as a boot consequence.

### `cans/01-architecture.md`

```diff
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@ -17,6 +17,7 @@
       - Radical plugin-first microkernel
         - Core drives hardware virtualization and multiplexes the 4 VirtIO rings
         - Every capability is a decoupled, composable plugin
+        - Exception fence: only 01-architecture.md#Core-invariants primitives are core-owned
       - Transparent truthful steering
         - Intercepted I/O is allowed, steered with tty_notice, or rejected
         - No silent state divergence
@@ -27,6 +28,38 @@
         - Guest reads KELIR_PARAMS like any other environment variable
         - No guest-side SDKs, CLI shims, or agents
+  - Capability taxonomy
+    - Four categories; every capability falls into exactly one
+    - Core primitive
+      - Hardware/protocol invariant, no policy
+      - Enumerated as pointers in 01-architecture.md#Core-invariants
+    - Interface
+      - User-supplied contract, no composition requirement
+      - Examples: user-implemented Oracle, pluggable VmmDriver backends
+      - Documented at the owning plane node
+    - Adapter binary
+      - Contract-bound process on the wire
+      - Examples: containerd-shim-v2, MCP stdio
+      - Translation logic delegated to a plugin the binary loads
+    - Plugin
+      - External input mapping, or policy over a core primitive
+      - Declares priority(): see 02-plugin-model.md#KelirPlugin-trait
+      - Owns crate, tests, fixtures, examples: see 02-plugin-model.md#Plugin-ownership
+    - Decision test
+      - Invariant with no policy? → core primitive
+      - User contract with no composition? → interface
+      - Wire contract with a process boundary? → adapter binary
+      - Otherwise → plugin
+  - Core invariants
+    - The closed set. Everything not pointed to here is a plugin.
+    - Trait definitions
+      - KelirPlugin: see 02-plugin-model.md#KelirPlugin-trait
+      - VmmDriver: see 01-architecture.md#VmmDriver-trait
+    - Context types: see 09-protocol.md#Wire-messages
+    - Wire framing: see 09-protocol.md#Wire-framing
+    - Dispatch loop: see 02-plugin-model.md#Static-dispatch
+    - 4-ring routing: see 01-architecture.md#Four-Ring-Bus
+    - Params ingress + auto-Rewrite: see 01-architecture.md#Params-ingress-contract
+    - Receipts: see 08-epistemic.md#Execution-receipts
+    - StepSlot: see 08-epistemic.md#Step-evaluation-loop
+    - Shim primitives: see 01-architecture.md#Guest-VM-anatomy
+    - EROFS mount: see 05-storage-plane.md#EROFS-rootfs
   - Four-Ring Bus
```

### `cans/02-plugin-model.md`

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -1,60 +1,128 @@
 - Plugin model
   - KelirPlugin trait
     - See: artifacts/code/plugin-trait.rs
     - Async trait implemented by all subsystems, internal and third-party
-    - Five hook families across the 4 VirtIO rings
-      - Lifecycle: on_vm_init, on_vm_ready, on_vm_shutdown
-      - Control plane (vsock): on_trap, on_process_exit
-      - Terminal plane (console): on_pty_input, on_pty_output
-      - Storage plane (fs): on_vfs_lookup, on_vfs_read, on_vfs_write
-      - Wire plane (net): on_net_outbound, on_net_inbound
+    - Trait surface
+      - name() -> &'static str: unique identifier
+      - priority() -> u8: dispatch ordering; lower = earlier
+    - Hook families across the 4 VirtIO rings
+      - Lifecycle: on_vm_init, on_vm_ready, on_vm_shutdown
+      - Control plane (vsock)
+        - on_trap, on_process_exit
+        - on_memory_pressure, on_heartbeat, on_port_bound
+        - on_signal_send, on_watchdog
+      - Terminal plane (console)
+        - on_pty_input, on_pty_output
+        - on_window_resize, on_disclosure
+      - Storage plane (fs): on_vfs_lookup, on_vfs_read, on_vfs_write
+      - Wire plane (net): on_net_outbound, on_net_inbound
+    - Hook context restriction
+      - Every hook receives HandlerCtx or a restricted lifecycle context
+      - No hook receives &mut Box<dyn VmmDriver> or &Session
+      - LifecycleCtx exposes: set_watchdog, adjust_balloon, register_veto
     - VfsInodeAttr carries full mode, uid, gid, size, content
       - Populated verbatim into FUSE LOOKUP replies
       - Ghost inodes honor mode: 0o400 exactly, no permission fallback
     - Plane semantics
       - Wire plane fully bidirectional: outbound and inbound mutation points
       - Storage plane fully attributed: on_vfs_write carries PID and HandlerCtx
+      - Control plane messages hook-observable; nothing bypasses the graph
   - Static dispatch
     - Plugins compiled into flat contiguous dispatch array at boot
-    - Evaluated in strict priority order
+    - Evaluated in priority() order; ties broken by declaration order
+    - requires() validated at boot; missing dep aborts boot with EConfig
     - First Deny or Rewrite halts pipeline, dispatches over AF_VSOCK
     - Sub-12 microsecond hook execution
-  - Five canonical plugins
+    - on_trap_deny is builder-level, not a plugin hook
+      - Deny-observation composition is a user concern
+  - Core context keys
+    - Routing table; each key's semantics live at the owning node
+    - Core-provided (always present)
+      - c.trap: see 09-protocol.md#Trap-fast-path
+      - c.env: see 09-protocol.md#Env-injection
+      - c.vm: see 01-architecture.md#VmmDriver-trait
+      - c.step: see 08-epistemic.md#Step-evaluation-loop
+      - c.receipt(): see 08-epistemic.md#Execution-receipts
+    - Plugin-provided (present only when the plugin is mounted)
+      - c.term: plugin-pty
+      - c.fs: plugin-vfs
+      - c.ledger: plugin-ledger
+      - c.wire: plugin-wire
+      - c.chaos: plugin-chaos
+    - User-provided via typed handle
+      - c.<key>: from .decorate() / PluginWithHandle / BasePlugin.handle()
+    - Rule: disabling a plugin removes its key at compile/type/import time
+  - First-class plugins
     - plugin-pty: virtio-console, ANSI sanitizer, Token Shield, prompt responder
     - plugin-vfs: virtio-fs, ghost inodes, maskRead, reconciler, lock broker
-    - plugin-wire: L7 proxy, SOCKS5, SSH gate, DNS sinkhole
+    - plugin-wire: L7 proxy, SOCKS5, SSH gate, DNS sinkhole, port forwarding
     - plugin-chaos: bidirectional wire jitter/drop, VFS ENOSPC injection
     - plugin-ledger: Merkle WAL, causal delta, OpenTelemetry traceparent
+    - Minimum five above; ecosystem additions per 02-plugin-model.md#Plugin-ownership
+    - Session capability surface: attributed per plane
+      - session.fs.*: see 05-storage-plane.md#Reconciler
+      - session.term.*, session.break_glass(): see 04-terminal-plane.md#Terminal-plane
+      - session.wire.*, session.forward_port(): see 06-wire-plane.md#Wire-plane
+      - session.ledger.*: see 08-epistemic.md#Merkle-WAL
+      - session.get_causal_delta_from_parent(): see 08-epistemic.md#Causal-delta
+  - Plugin ownership
+    - Contract, not layout; see 11-operations.md#Project-structure for crate layout
+    - Dependency rules
+      - Depends on kelir-core (traits, context types) only
+      - Depends on plugin-testkit as dev-dependency
+      - Never depends on crates/kelir facade
+      - Never depends on another plugin at build time
+      - Runtime coordination via requires() and core context keys
+    - Decoupling test
+      - Remove the plugin → remaining plugins build and test green
+      - A failing plugin's tests are attributable to that plugin alone
+  - Plugin testkit
+    - New crate: crates/kelir-plugin-testkit/
+    - Provides
+      - MockVmmDriver (checkpoint, fork, rollback without RAM)
+      - MockBusDispatcher (records ring messages per plane)
+      - HandlerCtx constructors from synthetic TrapEvent
+      - Assertions: assert_rewrite, assert_deny, assert_env_injected
+    - Depends only on kelir-core
+    - Every plugin's tests depend on it; nothing else does
+    - Not runtime: test infrastructure only
   - Language parity
     - See: artifacts/diagrams/sdks-invariants.txt
-    - Same five plugins in Rust, TypeScript, Python, maintained 1:1
+    - Same first-class plugins in Rust, TypeScript, Python, maintained 1:1
     - Trait surface and hook semantics identical across languages
-    - Rust: plugin_with_handle → typed handle; unmounted plugin breaks build
-    - TypeScript: type-level accumulation; missing .use() fails tsc
-    - Python: plugin_with_handle yields typed handle; missing handle fails import
+    - Typed-handle mechanism, realized per language
+      - Rust: PluginWithHandle::Handle; missing builder call → E0425
+      - TypeScript: createPlugin().state().decorate(); no .use() → tsc error
+      - Python: BasePlugin[Handle] + handle(); no handle → NameError at import
+    - All three realize the same 02-plugin-model.md#Core-context-keys contract
```

## Anchor verification

Every `see:` target in the corrected patch, checked against the dump:

| Pointer | Node exists? | File |
|---|---|---|
| `01-architecture.md#Core-invariants` | added by this patch | 01 |
| `02-plugin-model.md#KelirPlugin-trait` | ✓ `- KelirPlugin trait` | 02 |
| `02-plugin-model.md#Static-dispatch` | ✓ `- Static dispatch` | 02 |
| `02-plugin-model.md#Plugin-ownership` | added by this patch | 02 |
| `02-plugin-model.md#Core-context-keys` | added by this patch | 02 |
| `01-architecture.md#VmmDriver-trait` | ✓ `- VmmDriver trait` | 01 |
| `01-architecture.md#Four-Ring-Bus` | ✓ `- Four-Ring Bus` | 01 |
| `01-architecture.md#Params-ingress-contract` | ✓ `- Params ingress contract` | 01 |
| `01-architecture.md#Guest-VM-anatomy` | ✓ `- Guest VM anatomy` | 01 |
| `05-storage-plane.md#EROFS-rootfs` | ✓ `- EROFS rootfs` | 05 |
| `05-storage-plane.md#Reconciler` | ✓ `- Reconciler` | 05 |
| `04-terminal-plane.md#Terminal-plane` | ✓ `- Terminal plane` | 04 |
| `06-wire-plane.md#Wire-plane` | ✓ `- Wire plane` | 06 |
| `08-epistemic.md#Execution-receipts` | ✓ `- Execution receipts` | 08 |
| `08-epistemic.md#Step-evaluation-loop` | ✓ `- Step evaluation loop` | 08 |
| `08-epistemic.md#Merkle-WAL` | ✓ `- Merkle WAL` | 08 |
| `08-epistemic.md#Causal-delta` | ✓ `- Causal delta` | 08 |
| `09-protocol.md#Wire-messages` | ✓ `- Wire messages` | 09 |
| `09-protocol.md#Wire-framing` | ✓ `- Wire framing` | 09 |
| `09-protocol.md#Trap-fast-path` | ✓ `- Trap fast path` | 09 |
| `09-protocol.md#Env-injection` | ✓ `- Env injection` | 09 |
| `11-operations.md#Project-structure` | ✓ `- Project structure` | 11 |

Zero forward references. Zero missing `.md`. Zero cross-part dependencies. Part 1 is self-contained and `cans check --refs-only` clean.

Say go for Part 2.

# Part 2/6 — Wire and trait artifacts

Two artifacts. `plugin-trait.rs` becomes the single home for hook signatures (Part 1's `02#KelirPlugin-trait` now only lists names). `messages.rs` extracts the payload structs that the trait's hooks reference — reconciling three drifts: hook params that don't exist as types (`TrapEvent`, `ProcessExit`), control-plane messages with no hooks, and the `tty_notice` PTY bypass.

## `cans/artifacts/code/messages.rs`

```diff
--- a/cans/artifacts/code/messages.rs
+++ b/cans/artifacts/code/messages.rs
@@ -1,56 +1,100 @@
 // crates/kelir-core/src/protocol/messages.rs
 //
-// All wire messages exchanged over vsock:5252. Guest↔host control/trap plane.
-// Payloads encode as CBOR inside the TLV frame defined in codec.rs.
+// All wire messages exchanged over vsock:5252. Payloads encode as CBOR
+// inside the TLV frame defined in codec.rs.
 //
 // Canonical spec home: cans/09-protocol.md#Wire-messages
 
+use ciborium::value::Value as CborValue;
 use serde::{Deserialize, Serialize};
 use std::collections::HashMap;
 
+// ── Payload structs ──────────────────────────────────────────────
+// Referenced by both wire enums and plugin hook signatures.
+// One definition, one place.
+
+#[derive(Debug, Clone, Serialize, Deserialize)]
+pub struct TrapEvent {
+    pub seq: u64,
+    pub pid: u32,
+    pub ppid: u32,
+    pub binary_path: String,
+    pub binary_blake3: [u8; 32],
+    pub argv: Vec<String>,
+    pub envp: HashMap<String, String>,
+    pub cwd: String,
+    pub ancestor_chain: Vec<String>,
+    pub entropy: f64,
+    pub loaded_libraries: Vec<String>,
+}
+
+#[derive(Debug, Clone, Serialize, Deserialize)]
+pub struct ProcessExit {
+    pub pid: u32,
+    pub exit_code: i32,
+    /// True physical elapsed time from spawn to reap.
+    pub wall_clock_ns: u64,
+}
+
+#[derive(Debug, Clone, Serialize, Deserialize)]
+pub struct MemoryPressureWarning {
+    pub available_mb: u32,
+    pub total_mb: u32,
+}
+
+#[derive(Debug, Clone, Serialize, Deserialize)]
+pub struct PortBound {
+    pub guest_port: u16,
+    pub protocol: String,
+}
+
+#[derive(Debug, Clone, Serialize, Deserialize)]
+pub struct Heartbeat {
+    pub uptime_ms: u64,
+}
+
+#[derive(Debug, Clone, Serialize, Deserialize)]
+pub struct WindowResize {
+    pub rows: u16,
+    pub cols: u16,
+    pub xpixel: u16,
+    pub ypixel: u16,
+}
+
+#[derive(Debug, Clone, Serialize, Deserialize)]
+pub struct SendSignal {
+    pub pid: u32,
+    pub signal: u32,
+}
+
+// ── Wire enums ───────────────────────────────────────────────────
+
 #[derive(Debug, Serialize, Deserialize)]
 pub enum GuestToHost {
-    /// Syscall Trap Interception
-    TrapEvent {
-        seq: u64,
-        pid: u32,
-        ppid: u32,
-        binary_path: String,
-        binary_blake3: [u8; 32],
-        argv: Vec<String>,
-        envp: HashMap<String, String>,
-        cwd: String,
-        ancestor_chain: Vec<String>,
-        entropy: f64,
-        loaded_libraries: Vec<String>,
-    },
-    /// Process Exit Notification
-    ProcessExit {
-        pid: u32,
-        exit_code: i32,
-        wall_clock_ns: u64,
-    },
-    /// Memory Pressure Alert from In-Guest Watcher
-    MemoryPressureWarning {
-        available_mb: u32,
-        total_mb: u32,
-    },
-    /// Background Service Bound Port
-    PortBound {
-        guest_port: u16,
-        protocol: String,
-    },
-    Heartbeat {
-        uptime_ms: u64,
-    },
+    TrapEvent(TrapEvent),
+    ProcessExit(ProcessExit),
+    MemoryPressureWarning(MemoryPressureWarning),
+    PortBound(PortBound),
+    Heartbeat(Heartbeat),
 }
 
 #[derive(Debug, Serialize, Deserialize)]
 pub enum HostToGuest {
-    /// Decision on Intercepted Syscall
     TrapDecision {
         seq: u64,
         action: TrapAction,
     },
-    /// Forwarding TTY Terminal Window Resize (SIGWINCH)
-    WindowResize {
-        rows: u16,
-        cols: u16,
-        xpixel: u16,
-        ypixel: u16,
-    },
-    /// Signal Propagation (SIGINT, SIGTERM, SIGKILL)
-    SendSignal {
-        pid: u32,
-        signal: u32,
-    },
-    /// Dynamic Memory Balloon Adjustment
+    WindowResize(WindowResize),
+    SendSignal(SendSignal),
     AdjustBalloon {
         target_mb: u32,
     },
+    /// Core-originated mandatory disclosure. Routed through on_disclosure
+    /// hooks on the host before forwarding. Shim prints body verbatim to
+    /// the workload PTY — zero silent state divergence.
+    Disclosure {
+        body: Vec<u8>,
+    },
 }
 
 #[derive(Debug, Serialize, Deserialize)]
 pub enum TrapAction {
-    /// Resume the syscall untouched. Registers and memory are never modified.
     Allow,
-    /// Inject exit code and stderr into the agent's TTY.
     Deny {
         exit_code: i32,
         stderr_message: String,
     },
-    /// Re-execute with substitutions. This is the ONLY path by which
-    /// handler-side environment changes reach the guest: a handler that
-    /// returns `Decision.resume()` after `c.env.set(..)` is auto-converted
-    /// to `Rewrite { new_binary = original, new_argv = original,
-    /// injected_env = accumulated }`. See cans/09-protocol.md#Env-injection.
     Rewrite {
         new_binary: String,
         new_argv: Vec<String>,
         injected_env: HashMap<String, String>,
-        /// Mandatory terminal disclosure printed to the workload's PTY.
-        /// Guarantees zero silent state divergence.
-        tty_notice: Option<String>,
+        /// Mandatory terminal disclosure. Routed through on_disclosure
+        /// (host) → HostToGuest::Disclosure (shim) → PTY. Plugins may
+        /// annotate; the body is immutable by contract.
+        disclosure: Option<String>,
     },
     SuspendForApproval {
         poll_interval_ms: u32,
     },
+    /// Extension point. Dispatched by name to a registered handler;
+    /// unknown names fail with EUnknownAction before the pipeline resumes.
+    /// Closes the fixed-variant lock-in (quarantine, throttle, shadow-route).
+    Custom(CborValue),
 }
```

## `cans/artifacts/code/plugin-trait.rs`

```diff
--- a/cans/artifacts/code/plugin-trait.rs
+++ b/cans/artifacts/code/plugin-trait.rs
@@ -1,80 +1,146 @@
 // crates/kelir-core/src/plugin.rs
 //
 // The unified KelirPlugin trait implemented by every subsystem — internal
 // or third-party — across the 4 VirtIO rings. kelir-core contains zero
 // hardcoded capability logic; every capability is a plugin.
 //
 // Canonical spec home: cans/02-plugin-model.md#KelirPlugin-trait
 
 use async_trait::async_trait;
 use std::path::Path;
+use std::time::Duration;
 use crate::handlers::HandlerCtx;
-use crate::protocol::messages::{TrapEvent, TrapAction};
+use crate::protocol::messages::{
+    Heartbeat, MemoryPressureWarning, PortBound, ProcessExit,
+    SendSignal, TrapAction, TrapEvent, WindowResize,
+};
 use crate::session::Session;
-use crate::vmm::driver::VmmDriver;
+use crate::vmm::driver::{VmmDriver, VmConfig, WatchdogAction};
 
 #[derive(Debug, thiserror::Error)]
 pub enum PluginError {
+    /// Hard failure; halts the pipeline and surfaces to the runtime.
     #[error("Plugin execution rejected: {0}")]
     Halt(String),
+    /// Deny with a POSIX errno name surfaced to the guest.
+    /// Storage plane: guest write() fails with this errno.
+    /// Wire plane:    frame dropped silently (errno for logging only).
+    #[error("Plugin rejected: {errno} ({reason})")]
+    Reject { errno: String, reason: String },
     #[error("Internal I/O failure: {0}")]
     Io(#[from] std::io::Error),
 }
 
-/// Complete inode attribute set surfaced by storage-plane lookups. FUSE `LOOKUP`
-/// replies are populated verbatim from this struct (`st_mode`, `st_uid`, `st_gid`,
-/// `st_size`) alongside the content bytes.
+impl PluginError {
+    pub fn reject(errno: impl Into<String>, reason: impl Into<String>) -> Self {
+        Self::Reject { errno: errno.into(), reason: reason.into() }
+    }
+}
+
+/// Complete inode attribute set surfaced by storage-plane lookups.
 #[derive(Debug, Clone)]
 pub struct VfsInodeAttr {
-    /// File mode bits (e.g. `0o400` read-only, `0o600` private)
     pub mode: u32,
     pub uid: u32,
     pub gid: u32,
     pub size: u64,
     pub content: Vec<u8>,
 }
 
+// ── Lifecycle context ────────────────────────────────────────────
+// Restricted handle for on_vm_init. Exposes only mutating capabilities
+// that are pre-boot config; never the raw &mut dyn VmmDriver.
+
+#[derive(Debug, Clone, Copy)]
+pub enum VetoEvent {
+    Fork, Checkpoint, Rollback, Suspend, Resume,
+    Terminate, ClockSkew,
+}
+
+#[derive(Debug, Clone)]
+pub struct WatchdogEvent {
+    pub timeout_ms: u64,
+    pub action: WatchdogAction,
+    /// Monotonic ns since VM boot when the watchdog fired.
+    pub fired_at_ns: u64,
+    pub last_heartbeat_ms: u64,
+}
+
+pub struct LifecycleCtx<'a> {
+    pub config: &'a VmConfig,
+    pub session_id: &'a str,
+    driver: &'a mut dyn VmmDriver,
+    vetoes: &'a mut Vec<(VetoEvent, Box<dyn Fn() -> bool + Send + Sync>)>,
+}
+
+impl<'a> LifecycleCtx<'a> {
+    pub async fn set_watchdog(
+        &mut self, timeout: Duration, action: WatchdogAction,
+    ) -> Result<(), PluginError> {
+        self.driver.set_watchdog(timeout, action).await
+            .map_err(PluginError::Io)
+    }
+    pub async fn adjust_balloon(&mut self, target_mb: u32) -> Result<(), PluginError> {
+        self.driver.adjust_balloon(target_mb).await
+            .map_err(PluginError::Io)
+    }
+    /// Register a veto callback for a lifecycle action. All registered
+    /// callbacks run before the action; any returning false aborts it.
+    pub fn register_veto(
+        &mut self, event: VetoEvent,
+        f: Box<dyn Fn() -> bool + Send + Sync>,
+    ) {
+        self.vetoes.push((event, f));
+    }
+}
+
 #[async_trait]
 pub trait KelirPlugin: Send + Sync {
-    /// Unique identifier for telemetry and ordering
+    /// Unique identifier for telemetry and requires() resolution.
     fn name(&self) -> &'static str;
 
-    // ─── 1. MicroVM Lifecycle Hooks ─────────────────────────
-    async fn on_vm_init(&self, _driver: &mut Box<dyn VmmDriver>) -> Result<(), PluginError> { Ok(()) }
+    /// Dispatch ordering; lower values run earlier. Default: 100.
+    /// Ties broken by declaration order.
+    fn priority(&self) -> u8 { 100 }
+
+    /// Hard dependencies by plugin name. Validated at boot; a missing
+    /// dependency aborts with EConfig before any hook fires.
+    fn requires(&self) -> &'static [&'static str] { &[] }
+
+    // ─── 1. MicroVM Lifecycle ────────────────────────────────────
+    async fn on_vm_init(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> { Ok(()) }
     async fn on_vm_ready(&self, _session: &Session) -> Result<(), PluginError> { Ok(()) }
     async fn on_vm_shutdown(&self, _session: &Session) -> Result<(), PluginError> { Ok(()) }
 
-    // ─── 2. Control Plane Hooks (virtio-vsock) ───────────────
-    async fn on_trap(&self, _event: &TrapEvent, _ctx: &HandlerCtx) -> Result<Option<TrapAction>, PluginError> { Ok(None) }
-    async fn on_process_exit(&self, _pid: u32, _exit_code: i32) -> Result<(), PluginError> { Ok(()) }
+    // ─── 2. Control Plane (virtio-vsock) ─────────────────────────
+    async fn on_trap(
+        &self, _event: &TrapEvent, _ctx: &HandlerCtx,
+    ) -> Result<Option<TrapAction>, PluginError> { Ok(None) }
+    async fn on_process_exit(
+        &self, _event: &ProcessExit, _ctx: &HandlerCtx,
+    ) -> Result<(), PluginError> { Ok(()) }
+    async fn on_memory_pressure(
+        &self, _event: &MemoryPressureWarning,
+    ) -> Result<(), PluginError> { Ok(()) }
+    async fn on_heartbeat(
+        &self, _event: &Heartbeat,
+    ) -> Result<(), PluginError> { Ok(()) }
+    async fn on_port_bound(
+        &self, _event: &PortBound,
+    ) -> Result<(), PluginError> { Ok(()) }
+    async fn on_signal_send(
+        &self, _event: &SendSignal,
+    ) -> Result<(), PluginError> { Ok(()) }
+    async fn on_watchdog(
+        &self, _event: &WatchdogEvent,
+    ) -> Result<(), PluginError> { Ok(()) }
 
-    // ─── 3. Terminal Plane Hooks (virtio-console) ────────────
+    // ─── 3. Terminal Plane (virtio-console) ──────────────────────
     async fn on_pty_input(&self, _bytes: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
     async fn on_pty_output(&self, _chunk: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
+    async fn on_window_resize(
+        &self, _event: &WindowResize,
+    ) -> Result<(), PluginError> { Ok(()) }
+    /// Mandatory core-originated disclosure (TrapAction::Rewrite.disclosure).
+    /// Body is immutable; return Some(bytes) to append. All annotations
+    /// concatenate in priority() order and follow the body on the PTY.
+    async fn on_disclosure(
+        &self, _body: &[u8],
+    ) -> Result<Option<Vec<u8>>, PluginError> { Ok(None) }
 
-    // ─── 4. Storage Plane Hooks (virtio-fs) ──────────────────
+    // ─── 4. Storage Plane (virtio-fs) ────────────────────────────
     async fn on_vfs_lookup(&self, _path: &Path) -> Result<Option<VfsInodeAttr>, PluginError> { Ok(None) }
     async fn on_vfs_read(&self, _path: &Path, _buffer: &mut [u8]) -> Result<(), PluginError> { Ok(()) }
-    async fn on_vfs_write(&self, _path: &Path, _data: &[u8], _pid: u32, _ctx: &HandlerCtx) -> Result<bool, PluginError> { Ok(true) }
+    /// Ok(()) = allow. Err(Reject{errno}) = deny; guest write() fails
+    /// with errno. Replaces the previous `Result<bool, _>` ambiguity.
+    async fn on_vfs_write(
+        &self, _path: &Path, _data: &[u8], _pid: u32, _ctx: &HandlerCtx,
+    ) -> Result<(), PluginError> { Ok(()) }
 
-    // ─── 5. Wire Plane Hooks (virtio-net) ────────────────────
-    async fn on_net_outbound(&self, _frame: &mut Vec<u8>) -> Result<bool, PluginError> { Ok(true) }
-    async fn on_net_inbound(&self, _frame: &mut Vec<u8>) -> Result<bool, PluginError> { Ok(true) }
+    // ─── 5. Wire Plane (virtio-net) ──────────────────────────────
+    /// Ok(()) = forward. Err(Reject) = silent drop (no RST, no guest error).
+    async fn on_net_outbound(&self, _frame: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
+    /// Ok(()) = deliver. Err(Reject) = silent drop.
+    async fn on_net_inbound(&self, _frame: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
 }
```

## Reconciliation table

What the patch fixes, and against what it was inconsistent:

| Inconsistency | Old | New |
|---|---|---|
| `on_trap(&TrapEvent, …)` — type doesn't exist | Enum variant only | `pub struct TrapEvent` + `GuestToHost::TrapEvent(TrapEvent)` |
| `on_process_exit(pid, exit_code)` — drops `wall_clock_ns` | 2 args | `on_process_exit(&ProcessExit, &HandlerCtx)` — full payload + receipt access |
| `MemoryPressureWarning` / `Heartbeat` / `PortBound` — hookless | Wire-only | Hooks `on_memory_pressure` / `on_heartbeat` / `on_port_bound` |
| `SendSignal` / `WindowResize` — hookless | Wire-only | `on_signal_send` / `on_window_resize` |
| Watchdog — no observation | Driver-only | `on_watchdog(&WatchdogEvent)` |
| `tty_notice` bypasses plugin-pty | Direct PTY write | `Rewrite.disclosure` → `on_disclosure` → `HostToGuest::Disclosure` → shim |
| `on_vfs_write -> Result<bool>` vs examples' `PluginError::Reject` | Ambiguous | `Result<(), PluginError>`; `Ok` = allow, `Reject` = deny with errno |
| `on_net_outbound -> Result<bool>` silent-drop unclear | Ambiguous | Same; `Reject` = silent drop |
| `on_vm_init(&mut Box<dyn VmmDriver>)` — over-capability | Full driver | `&mut LifecycleCtx` — restricted to `set_watchdog` / `adjust_balloon` / `register_veto` |
| `priority()` absent but spec requires priority order | Trait gap | `fn priority() -> u8 { 100 }` |
| `requires()` in Python example only, not trait | Cross-language drift | `fn requires() -> &'static [&'static str] { &[] }` |
| `TrapAction` closed enum | 4 variants | + `Custom(CborValue)` extension point |
| `PluginError` missing `Reject` in trait but used in examples | Trait/example drift | `Reject { errno, reason }` + `PluginError::reject(..)` helper |

## New invariants (to land in Part 3 at `09#Wire-messages`)

Part 2 introduces three types with semantics that `09` will spec — no forward refs from the artifact:

- `TrapAction::Custom(CborValue)` — dispatched by action name to a registered handler; unknown names → `EUnknownAction` before the pipeline resumes.
- `HostToGuest::Disclosure { body }` — new wire message; shim prints body verbatim to workload PTY; no plugin observation on the shim side.
- `VetoEvent` — one variant per lifecycle action (`Fork`, `Checkpoint`, `Rollback`, `Suspend`, `Resume`, `Terminate`, `ClockSkew`); `07#Lifecycle` (Part 4) will spec each variant's firing point.

## Deferred from Part 2

- `WatchdogAction::Custom` — stays in `vmm-driver-trait.rs`; `WatchdogEvent.action` uses the current two-variant enum. Extension lands in Part 4 alongside `07#Lifecycle` since watchdog policy is lifecycle.
- `01#VmmDriver-trait` unchanged — `set_watchdog(timeout, WatchdogAction)` signature is already correct.
- Examples (`autonomous-swe-agent.*`) updated in Part 5 — they call `Ok(true)` on `on_vfs_write` today, which the new signature rejects.

Say go for Part 3 (`04-terminal-plane.md`, `05-storage-plane.md`, `09-protocol.md`).

# Part 3/6 — Terminal, storage, protocol semantics

Three files. Part 2 added hooks (`on_disclosure`, `on_window_resize`, `on_signal_send`, `on_vfs_write` return type) — Part 3 places their semantics at the owning plane node so `02#KelirPlugin-trait` stays a name-only registry.

## `cans/04-terminal-plane.md`

```diff
--- a/cans/04-terminal-plane.md
+++ b/cans/04-terminal-plane.md
@@ -1,6 +1,7 @@
 - Terminal plane
   - Terminal pipeline
     - See: artifacts/diagrams/terminal-pipeline.txt
+    - Owner: plugin-pty; see 02-plugin-model.md#First-class-plugins
     - Guest workload writes stdout/stderr
     - Linux kernel PTY driver: /dev/hvc0 → /dev/pts/N
     - isatty(1) == true, raw mode active, zero CLI downgrade
@@ -44,13 +45,15 @@
   - Window resize
     - Boot handshake
       - Hypervisor pushes configured initial size before first workload spawn
       - Guest never boots into 80x25 default
       - Full-screen apps (Claude Code, vim, top) initialize with correct geometry
     - Live resize
       - host process.stdout.on("resize") catches SIGWINCH
+      - on_window_resize(&WindowResize) fires in priority() order
+        - Observation only; hook cannot rewrite wire payload
       - HostToGuest::WindowResize { rows, cols, xpixel, ypixel } over AF_VSOCK:5252
       - In-guest shim executes ioctl(pts_fd, TIOCSWINSZ, &ws)
       - Guest shell redraws without corruption
   - Signal forwarding
     - Ctrl+C / Ctrl+D forwarded to guest foreground process group
+    - on_signal_send(&SendSignal) fires in priority() order
+      - Observation only; hook cannot rewrite signal
     - Signals flow as HostToGuest::SendSignal over vsock:5252
     - Shim signals the foreground PID
+  - Disclosure
+    - Core-originated mandatory notice from TrapAction::Rewrite.disclosure
+    - Host-side pipeline
+      - on_disclosure(&body) fires on every plugin in priority() order
+      - Hook returns Some(bytes) to append; None to pass
+      - Annotations concatenate after body in priority() order
+      - Body itself is immutable by contract
+    - Wire message
+      - HostToGuest::Disclosure { body } over vsock:5252
+      - See 09-protocol.md#Wire-messages
+    - Guest-side
+      - Shim prints body + annotations verbatim to workload PTY
+      - No plugin-pty filter runs on shim side; disclosure is authoritative
+    - Invariant: zero silent state divergence
+      - Any host-side state mutation reaches the workload's TTY
+      - Plugins annotate; none suppress
   - Token Shield
```

## `cans/05-storage-plane.md`

```diff
--- a/cans/05-storage-plane.md
+++ b/cans/05-storage-plane.md
@@ -44,6 +44,16 @@
   - Shared host package caches
     - Host ~/.npm → guest /root/.npm
     - Host ~/.cache/pip → guest /root/.cache/pip
     - Host ~/.cargo/registry → guest /usr/local/cargo/registry
     - Cold installs drop from ~2 minutes to <3 seconds via local SSD
+  - Write gating
+    - plugin-vfs gates FUSE write() via on_vfs_write
+    - Signature: see artifacts/code/plugin-trait.rs
+    - Ok(()) = allow; kernel proceeds with write to upperdir
+    - Err(PluginError::Reject { errno, reason }) = deny
+      - Guest write() fails with named errno; error surfaces in workload
+      - Returned before any bytes enter upperdir
+    - Err(PluginError::Halt) = deny + halt pipeline
+      - For policy violations requiring human review
+    - Observation-only snoop callbacks are separate: see #Snoop-plane
   - Ghost inodes
```

## `cans/09-protocol.md`

```diff
--- a/cans/09-protocol.md
+++ b/cans/09-protocol.md
@@ -50,26 +50,45 @@
     - HostToGuest
       - TrapDecision: seq, action
       - WindowResize: rows, cols, xpixel, ypixel
       - SendSignal: pid, signal
       - AdjustBalloon: target_mb
+      - Disclosure: body (bytes)
+        - Mandatory core-originated notice
+        - Pipeline and invariants: see 04-terminal-plane.md#Disclosure
     - TrapAction variants
       - Allow: resume syscall untouched; registers and memory never modified
       - Deny { exit_code, stderr_message }: inject exit code + stderr to agent TTY
-      - Rewrite { new_binary, new_argv, injected_env, tty_notice }: re-execute with substitutions
+      - Rewrite { new_binary, new_argv, injected_env, disclosure }: re-execute with substitutions
+        - disclosure Some(_) routes via HostToGuest::Disclosure
       - SuspendForApproval { poll_interval_ms }: hold for human approval
+      - Custom(CborValue): extension point for policy actions beyond the four above
+        - Dispatched by name to a registered handler (registration TBD)
+        - Unknown names fail EUnknownAction before pipeline resumes
+    - Hook mapping
+      - GuestToHost messages fire control-plane hooks
+        - TrapEvent → on_trap; ProcessExit → on_process_exit
+        - MemoryPressureWarning → on_memory_pressure
+        - Heartbeat → on_heartbeat; PortBound → on_port_bound
+      - HostToGuest messages fire host-side hooks before dispatch
+        - WindowResize → on_window_resize
+        - SendSignal → on_signal_send
+        - Disclosure → on_disclosure
+      - Watchdog fires on_watchdog, but is driver-side (not a wire message)
   - Env injection
     - Allow cannot mutate guest environment
       - Registers and memory untouched
       - A plain Allow decision is by definition a no-op
     - Handler-side c.env.set(...) reaches guest only through Rewrite
     - Auto-Rewrite on resume
       - Handler calls c.env.set(...) then returns Decision.resume()
       - Runtime auto-converts to TrapAction::Rewrite
       - new_binary = original_binary
       - new_argv = original_argv
       - injected_env = accumulated handler env map
+      - disclosure = handler's tty_notice if set, else None
       - Syscall re-executes through Rewrite path with injected env
     - This is the only channel for handler-side env changes to reach guest
```

## Reconciliation

| Inconsistency | Old | New |
|---|---|---|
| Terminal plane had no owner | Implicit via `@kelir/plugin-pty` in one bullet | Explicit `Owner: plugin-pty` at pipeline root |
| `on_window_resize` had no firing point | Signal → wire message directly | Hook fires pre-dispatch, observation-only |
| `on_signal_send` same | Same | Same |
| `tty_notice` bypassed plugin-pty | Direct PTY write | `TrapAction::Rewrite.disclosure` → `on_disclosure` → `HostToGuest::Disclosure` → shim |
| `on_vfs_write` had ambiguous `bool` semantics | `Result<bool, _>` | `Ok(())` = allow, `Reject{errno}` = deny, `Halt` = deny+halt |
| `TrapAction::Rewrite` field name drift vs. Part 2 | `tty_notice` in 09, `disclosure` in messages.rs | Both `disclosure` |
| `TrapAction::Custom` had no spec entry | Absent | `#TrapAction-variants` entry + dispatch contract |
| Wire message ↔ hook mapping implicit | Reader infers from 02 trait list | `#Hook-mapping` in 09 states it once |
| Auto-Rewrite's `disclosure` behavior unspecified | Silent | One line: `Some` if handler set a notice, else `None` |

## Anchor verification

Every `see:` in Part 3, checked:

| Pointer | Status |
|---|---|
| `02-plugin-model.md#First-class-plugins` | ✓ Part 1 |
| `09-protocol.md#Wire-messages` | ✓ existing; modified in this patch |
| `artifacts/code/plugin-trait.rs` | ✓ (artifact path, capital `See:`) |
| `05-storage-plane.md#Snoop-plane` | ✓ existing |
| `04-terminal-plane.md#Disclosure` | ✓ added by this patch |

Zero forward refs. Zero duplicated content — `04#Disclosure` owns the pipeline; `09#Wire-messages` owns the wire shape; `05#Write-gating` owns `on_vfs_write` semantics; `02#KelirPlugin-trait` stays name-only.

## Deferred to later parts

- **Part 4** (`07`, `08`): `on_watchdog` semantics, `WatchdogEvent` firing point, `VetoEvent` variants, `Oracle` node, `session.get_causal_delta_from_parent()` → `plugin-ledger`.
- **Part 5** (`10`, `03`, examples): MCP/OCI/OpenShell as binaries+plugins; examples updated for new `on_vfs_write` signature, `disclosure` field, and `on_disclosure` wiring; `03` gains the `#Plugin-ownership` pointer.
- **Part 6** (`11`, config): per-plugin test targets, `plugin-testkit` crate, facade-tests split, `ci.yml`/`justfile`.

Say go for Part 4.

# Part 4/6 — Lifecycle and epistemic

Two files. Part 2 declared `VetoEvent` and `WatchdogEvent`; Part 4 places their firing points and split. Also adds the `#Oracle` node (Part 1's `#Capability-taxonomy` pointed at 08 for its documentation but 08 has none) and the primitive-vs-derived telemetry rule that resolves the `delta.rs`-in-core question.

## `cans/07-lifecycle.md`

```diff
--- a/cans/07-lifecycle.md
+++ b/cans/07-lifecycle.md
@@ -1,6 +1,10 @@
 - Lifecycle
   - Superpowers
     - See: artifacts/diagrams/superpowers.txt
     - Nine superpowers enumerated below
+    - Every superpower exposes a veto or observation point
+      - Registered via LifecycleCtx in on_vm_init
+      - Class per superpower: see #Lifecycle-vetoes
+    - Ownership attribution: see 02-plugin-model.md#First-class-plugins
   - CoW fork
     - Driver primitive: fork returns Box<dyn VmmDriver> in <15ms
@@ -27,6 +31,9 @@
     - session.vm.fork() returns full Session
     - spawn() and every other session API directly callable on child
     - Typical use: parallel agent exploration (5+ candidate strategies)
+    - Veto: Fork (hard)
+      - Callback false → ESessionVeto; no child VM
   - Rollback
@@ -34,6 +41,9 @@
     - Used by HandlerCtx post-exec on post-condition violation
     - Does not touch host SSD or Merkle WAL
+    - Veto: Rollback (observation)
+      - Cannot be vetoed; rollback is the recovery action
+      - Use for ledger/trace of recovery events
   - Suspend and resume
@@ -47,6 +57,10 @@
     - resume
       - Restores registers and memory in <50ms
       - Resumes at exact cycle boundary
+    - Veto: Suspend (hard), Resume (observation)
+      - Suspend false → ESessionVeto; host RAM not flushed
+      - Resume fires before primitive; return value ignored
+      - Note: on_vm_ready does not re-fire on resume
     - Typical use: waiting for human approval with zero host CPU cost
   - Clock skew
@@ -66,6 +80,9 @@
       - Guest kernel never flags tsc as unstable, never falls to jiffies
     - Typical use: JWT expiry, cron cycles, certificate renewals
+    - Veto: ClockSkew (hard)
+      - Callback false → ESessionVeto; guest clock unchanged
   - Lid sleep/wake clock snapping
@@ -113,6 +130,7 @@
   - Break-glass
     - Human interactive attach when automated agent is stuck
     - session.breakGlass() on Session (post-boot only)
+      - Ownership: see 02-plugin-model.md#First-class-plugins
     - Pauses agent loop, bridges developer host TTY directly to virtio-console
@@ -121,6 +138,7 @@
   - Port forwarding
     - forwardPort is a Session API with language parity
+      - Ownership: see 02-plugin-model.md#First-class-plugins
     - TypeScript: session.forwardPort({ host: 8080, guest: 3000 })
@@ -160,3 +177,47 @@
     - Foreground shell exit → orphaned daemons adopted and reaped
     - Prevents defunct <zombie> processes exhausting PID table
+  - Lifecycle vetoes
+    - Registration
+      - LifecycleCtx.register_veto(event, Box<dyn Fn() -> bool + Send + Sync>)
+      - Callbacks registered in on_vm_init; validated at boot
+      - Callbacks fire in priority() order across all plugins
+    - Two classes
+      - Hard veto: Fork, Suspend, ClockSkew
+        - Any false return aborts the action with ESessionVeto
+        - Use for policy gates (e.g. "no fork until fixtures ready")
+      - Observation only: Checkpoint, Rollback, Resume, Terminate
+        - Callback fires; return value logged and ignored
+        - Use for ledger / trace / metrics
+        - Cannot veto by design
+          - Rollback, Resume, Checkpoint are state-preserving
+          - Terminate flush hook is on_vm_shutdown, not a veto
+    - Note: VetoEvent is the uniform registration type for both classes
+      - Class determines semantics, not the type name
+    - Firing point
+      - Before the corresponding hypervisor primitive executes
+      - Session-level actions also route through this point
+  - Watchdog
+    - See: artifacts/code/vmm-driver-trait.rs (WatchdogAction)
+    - Armed via LifecycleCtx.set_watchdog
+      - Typically called from on_vm_init by a policy plugin
+      - WatchdogAction: Reboot | Terminate
+    - Heartbeat
+      - Shim emits GuestToHost::Heartbeat at driver-configured cadence
+      - on_heartbeat fires per receipt; observation only
+    - Firing
+      - No heartbeat within timeout → driver arms the timeout event
+      - on_watchdog(&WatchdogEvent) fires before action executes
+        - WatchdogEvent: timeout_ms, action, fired_at_ns, last_heartbeat_ms
+        - Ok(()) → driver executes WatchdogAction
+        - Err(PluginError::Halt) → action suppressed; surfaces to runtime
+      - This is the extension point for custom actions
+        - A plugin may do work in on_watchdog then return Err(Halt)
+        - WatchdogAction stays closed (Reboot | Terminate)
```

## `cans/08-epistemic.md`

```diff
--- a/cans/08-epistemic.md
+++ b/cans/08-epistemic.md
@@ -1,4 +1,32 @@
 - Epistemic
+  - Oracle
+    - User-supplied interface; NOT a plugin
+    - Location: kelir-core::oracle::Oracle
+    - Injection: builder.oracle(impl Oracle); one per session
+    - Not in dispatch array; no priority(); no requires()
+    - Rationale: single policy layer for the epistemic engine
+      - Epistemic policy is user-owned, not composable
+      - Oracles chain by composition inside their impl, not the plugin graph
+    - Methods
+      - analyze_intent(&TrapEvent) -> IntentAnalysis
+        - Called from handlers, typically before(*)
+        - IntentAnalysis: is_hypothesis_valid, falsification_reason, allow_high_entropy
+      - verify_physical_receipt(&ExecutionReceipt) -> bool
+        - Called from after(*) post-condition verification
+      - ingest_harness_event(&Path, &Value, &str)
+        - Called from snoop callbacks; see 05-storage-plane.md#Snoop-plane
+      - await_human_approval()
+        - Called between suspend/resume for human-in-the-loop gates
+      - current_trace_id() -> String
+        - Called from handlers setting OTEL_TRACE_ID
+    - Wiring example: See artifacts/examples/autonomous-swe-agent.rs
+  - Primitive vs derived telemetry
+    - Rule
+      - Primitive: single read API, hardware boundary, no policy
+        - Core-owned; enumerated in 01-architecture.md#Core-invariants
+      - Derived: computed from primitives, policy-bearing
+        - Plugin-owned; never a new core method
+    - Primitive (core)
+      - ExecutionReceipt via c.receipt()
+      - StepSlot via c.step
+      - TrapEvent, ProcessExit, wire messages
+    - Derived (plugin-owned)
+      - CausalDelta: plugin-ledger; see #Causal-delta
+      - Merkle chain: plugin-ledger; see #Merkle-WAL
+      - OTel spans: plugin-ledger; see #OpenTelemetry
+      - Oracle analyses: consumer's Oracle impl
+    - Adding a new derived metric: new plugin or new Oracle impl
   - Execution receipts
@@ -30,6 +58,12 @@
     - Delta fields
       - memory_diff_bytes: hardware-agnostic byte-level RAM delta
       - modified_inodes: Vec<PathBuf>
       - spawned_pids: Vec<u32>
       - egress_requests: Vec<HttpRequestRecord>
     - Computed between two checkpoint descriptors
+    - Computed by plugin-ledger
+      - Session accessor: session.get_causal_delta_from_parent()
+      - Requires plugin-ledger mounted
+      - Struct type lives in kelir-core as shared context type
+        - Consistent with ExecutionReceipt
+        - Ownership is computation + dispatch, not type definition
   - Merkle WAL
@@ -76,6 +110,9 @@
       - Host in-memory buffer captures updates before disk sync
       - Invariant: eliminates async races and file lock collisions
+      - Consumer: Oracle.ingest_harness_event; see #Oracle
   - Lifecycle bridge
@@ -88,6 +125,10 @@
       - step() → execute until next virtio-vsock syscall trap, then pause
       - revert() → drop dirty RAM, purge tmpfs diffs to last good snapshot in <8ms
+    - Underlying primitives: see 07-lifecycle.md#Superpowers
+    - Veto points: see 07-lifecycle.md#Lifecycle-vetoes
+    - This node specifies the orchestrator-facing mapping only
   - Step evaluation loop
```

## Reconciliation

| Inconsistency | Old | New |
|---|---|---|
| `VetoEvent` variants had no firing point or class | Type only (Part 2) | `07#Lifecycle-vetoes` — two classes, per-superpower firing points |
| `WatchdogEvent` had no firing contract | Type only (Part 2) | `07#Watchdog` — timing, hook return semantics, extension via `Err(Halt)` |
| Watchdog extension mechanism unspecified | Part 2 deferred `Custom` variant | `07#Watchdog` — extension is `on_watchdog` returning `Halt`, `WatchdogAction` stays closed |
| `on_vm_ready` re-fire on resume? | Silent | `07#Suspend-and-resume` states no re-fire |
| `Rollback` as "veto" was semantically wrong | Part 2 named it in `VetoEvent` | `07#Lifecycle-vetoes` classifies it as observation-only |
| `Terminate` as "veto" leaves orphan VMs | Part 2 named it in `VetoEvent` | Same — observation-only; flush hook is `on_vm_shutdown` |
| `Oracle` has no spec node | Implicit via examples | `08#Oracle` — interface spec, methods, injection, non-plugin rationale |
| `CausalDelta` file in `kelir-core/src/telemetry/delta.rs` but treated as derived | Ambiguous | `08#Primitive-vs-derived-telemetry` + `#Causal-delta` — struct in core, computation in `plugin-ledger` |
| `session.get_causal_delta_from_parent()` had no spec home | Examples only | `08#Causal-delta` states ownership + accessor |
| `08#Lifecycle-bridge` overlaps `07#Superpowers` | Two framings, unclear relationship | `08` states it's the orchestrator-facing mapping only, `see:` to 07 for primitives |
| Tier-3 snoop → Oracle link unstated | Reader infers | One line at `#Three-tier-truth` |

## Why `CausalDelta` struct stays in core

Part 1's `#Core-invariants` does not list `CausalDelta` as an invariant — it's not in the enumerated fence. But `Session::get_causal_delta_from_parent()` returns `CausalDelta`, and `kelir-core` cannot depend on `plugin-ledger`. Resolution: the *type* is a shared context type in core (consistent with `ExecutionReceipt` also being a core type while `c.receipt()` is core-provided), and the *computation* is plugin-ledger. `08#Causal-delta` states both facts explicitly. This does not reopen the fence: `#Core-invariants` lists *capabilities*; shared struct types are context types, which the fence already covers at `09-protocol.md#Wire-messages`.

## Anchor verification

| Pointer | Status |
|---|---|
| `07-lifecycle.md#Superpowers` | ✓ existing |
| `07-lifecycle.md#Lifecycle-vetoes` | added by this patch |
| `07-lifecycle.md#Watchdog` | added by this patch |
| `08-epistemic.md#Oracle` | added by this patch |
| `08-epistemic.md#Primitive-vs-derived-telemetry` | added by this patch |
| `08-epistemic.md#Causal-delta` | ✓ existing |
| `08-epistemic.md#Merkle-WAL` | ✓ existing |
| `08-epistemic.md#OpenTelemetry` | ✓ existing |
| `08-epistemic.md#Three-tier-truth` | ✓ existing |
| `08-epistemic.md#Lifecycle-bridge` | ✓ existing |
| `02-plugin-model.md#First-class-plugins` | ✓ Part 1 |
| `05-storage-plane.md#Snoop-plane` | ✓ existing |
| `01-architecture.md#Core-invariants` | ✓ Part 1 |
| `artifacts/code/vmm-driver-trait.rs` | artifact (capital `See:`) |
| `artifacts/examples/autonomous-swe-agent.rs` | artifact (capital `See:`) |

Zero forward refs. Zero content duplication:
- `07#Superpowers` states the veto framing but not ownership (points to 02).
- `07#Break-glass` / `#Port-forwarding` add only a `see:` to 02; the ownership attribution lives at 02.
- `08#Causal-delta` states computation home; type home is stated here (new fact), not duplicated elsewhere.
- `08#Lifecycle-bridge` `see:`s 07 for primitives; does not restate them.

## Deferred to later parts

- **Part 5** (`10`, `03`, examples): MCP/OCI/OpenShell as binaries+plugins; `03` gains pointer to `02#Plugin-ownership`; examples updated for `on_vfs_write` returning `Result<(), PluginError>`, `disclosure` field, and `on_disclosure` wiring.
- **Part 6** (`11`, config): per-plugin test targets, `plugin-testkit` crate, facade-tests split, `delta.rs` file relocation to `plugin-ledger` (or explicit "type stays in core; computation in plugin-ledger — file path unchanged" if we prefer), `ci.yml`/`justfile`/`project-structure.txt`.

Say go for Part 5.

# Part 5/6 — Adapters, SDK, examples

Three files plus three example artifacts. Every adapter gets the binary/plugin split Part 1 promised at `#Capability-taxonomy`. Every example moves to the Part 2 signatures. `03-sdk-architecture.md` gains only pointers — the SDK prose itself is unchanged because the invariant it describes (language parity) is now carried by `02#Language-parity`.

## `cans/10-adapters.md`

```diff
--- a/cans/10-adapters.md
+++ b/cans/10-adapters.md
@@ -1,49 +1,88 @@
 - Adapters
   - Ingestion matrix
     - See: artifacts/diagrams/ingestion-adapters.txt
-    - Five adapter families: MCP, containerd shim v2, OpenShell gRPC, E2B, tri-native SDKs
+    - Five adapter families: MCP, containerd shim v2, OpenShell gRPC, E2B, tri-native SDKs
     - Tri-native SDKs are canonical in 03-sdk-architecture.md; not repeated here
-  - MCP adapter
-    - Binary: @kelir/mcp-server
-    - Command: kelir-mcp
-    - Args: --kernel, --rootfs, --memory-mb
-    - Target clients: Claude Desktop, Cursor, Devin
-    - Bypasses host shell execution entirely
-    - Maps LLM tool calls directly to VirtIO ring operations
-      - execute_command → virtio-console
-      - read_file → virtio-fs
-      - write_file → virtio-fs
+    - Every adapter is a binary contract + a plugin
+      - Binary: process the ecosystem speaks to
+      - Plugin: translation and policy inside the process
+      - See 01-architecture.md#Capability-taxonomy
+  - MCP adapter
+    - Binary
+      - Crate: kelir-adapters/mcp-server
+      - Command: kelir-mcp
+      - CLI args: --kernel, --rootfs, --memory-mb
+        - Mapped to VmConfig; forwarded to builder, not parsed by plugin
+      - Transport: stdio JSON-RPC to client (Claude Desktop, Cursor, Devin)
+      - Loads plugin-mcp and nothing else
+      - Bypasses host shell execution entirely
+    - Plugin
+      - Crate: kelir-plugins/plugin-mcp
+      - name() = "mcp"; priority() = 50
+      - Contributes c.mcp context key
+      - Hook wiring
+        - on_vm_ready: setup JSON-RPC transport, advertise capabilities
+        - on_pty_output / on_pty_input: stream terminal to/from MCP client
+        - on_vfs_read / on_vfs_write: expose as read_file / write_file
+        - on_trap: observe syscalls to correlate execute_command with trap
+        - on_process_exit: emit MCP task completion events
+        - on_disclosure: route mandatory notices into MCP stream
+      - Mapping (external tool → ring op)
+        - execute_command → virtio-console via spawn, wrapped
+        - read_file → virtio-fs lookup + read
+        - write_file → virtio-fs write
+      - Composes with plugin-chaos, plugin-ledger, plugin-wire, plugin-pty
+        - Previously impossible: adapter binary sat outside the graph
   - Containerd shim v2
-    - Binary: containerd-shim-kelir-v2
-    - Activated via runtimeClassName: kelir in Pod manifests
-    - Pod isolation
-      - Each Pod runs in isolated hardware microVM
-      - Not a Linux cgroup/namespace envelope
-    - PVC storage
-      - Kubernetes PersistentVolumeClaims mount as virtio-fs endpoints
-      - In-flight UID/GID mapping applied
-    - OCI compatibility
-      - Standard OCI images translated to read-only EROFS blocks
-      - Happens during rootfs setup
+    - Binary
+      - Crate: kelir-adapters/containerd-shim
+      - Activated via runtimeClassName: kelir in Pod manifests
+      - Loads plugin-oci-translator
+      - Pod isolation
+        - Each Pod runs in isolated hardware microVM
+        - Not a Linux cgroup/namespace envelope
+      - PVC storage
+        - PersistentVolumeClaims mount as virtio-fs endpoints
+        - UID/GID mapping applied in-guest
+    - Plugin
+      - Crate: kelir-plugins/plugin-oci-translator
+      - name() = "oci-translator"; priority() = 20
+      - Hook wiring
+        - on_vm_init: translate OCI image layers → EROFS block
+        - on_vfs_lookup: resolve image-declared volumes
+      - Translation policy
+        - Layer flattening, whiteout application, /etc overlay
+        - Composable: a chaos plugin can fault-inject into image pulls
   - OpenShell gRPC driver
-    - Binary: openshell-driver-kelir
-    - Implements compute_driver.proto
-    - Served over /var/run/kelir/openshell.sock (UDS)
-    - GPU passthrough via driver-rustvmm
-      - NVIDIA Container Device Interface (CDI) device mapping
-    - Coordinates hardware memory checkpointing and CoW branching
-    - Target: multi-tenant inference tasks
+    - Binary
+      - Crate: kelir-adapters/openshell-grpc
+      - Implements compute_driver.proto
+      - Served over /var/run/kelir/openshell.sock (UDS)
+      - Loads plugin-openshell-coordinator
+      - GPU passthrough via driver-rustvmm
+        - NVIDIA Container Device Interface (CDI) device mapping
+      - Target: multi-tenant inference tasks
+    - Plugin
+      - Crate: kelir-plugins/plugin-openshell-coordinator
+      - name() = "openshell-coordinator"; priority() = 60
+      - Hook wiring
+        - on_vm_init: register vetoes on Fork, Suspend, ClockSkew
+        - on_vm_ready: report readiness to OpenShell control plane
+        - on_watchdog: forward to OpenShell health system
+      - Checkpoint and branch policy
+        - When to checkpoint (request count, latency thresholds)
+        - What to branch on (multi-tenant isolation)
+        - Composable: a ledger plugin sees every checkpoint event
   - E2B compatibility
     - TypeScript: @kelir/e2b-compat
     - Python: kelir.compat.e2b
     - Preserves existing E2B Sandbox method signatures
       - Sandbox.create, sandbox.commands.run, sandbox.files, sandbox.pty, sandbox.kill
     - Replaces remote cloud VM roundtrips with sub-60ms local hardware execution
     - Drop-in: frameworks built against E2B SDK require no code change
+    - Implementation: thin SDK wrapper
+      - No plugin: Sandbox methods map directly to Session methods
+      - No binary: consumes the same in-process SDK as first-class consumers
```

## `cans/03-sdk-architecture.md`

```diff
--- a/cans/03-sdk-architecture.md
+++ b/cans/03-sdk-architecture.md
@@ -1,6 +1,8 @@
 - SDK architecture
   - Tri-native design
     - No dynamic any-typing, no second-class shims
     - All three SDKs enforce identical runtime guarantees through native type systems
+    - Plugin authorship contract: see 02-plugin-model.md#Plugin-ownership
+    - Plugin testkit: see 02-plugin-model.md#Plugin-testkit
     - Rust: borrow-checker verified handle injection, zero-cost Serde
@@ -34,6 +36,8 @@
   - Python SDK
     - See: artifacts/code/python-raii-pydantic.py
     - async with builder as session
@@ -48,20 +52,22 @@
   - Full-surface examples
     - See: artifacts/examples/autonomous-swe-agent.rs
     - See: artifacts/examples/autonomous-swe-agent.ts
     - See: artifacts/examples/autonomous-swe-agent.py
+    - Position: integration examples, not "the" example
+      - Each plugin's own tests/ and examples/ are canonical for that plugin
+      - These three demonstrate composition across plugins and rings
     - Each example exercises
       - All five plugins
       - All four VirtIO rings
       - All three epistemic truth tiers
       - All lifecycle superpowers in 07-lifecycle.md
     - Not transliterations: each uses language-idiomatic constructs
       - Rust: async-trait, JoinSet, Arc<Mutex<>> handles
       - TypeScript: Promise.all fan-out, generic inference
       - Python: asyncio.gather, async with, Pydantic v2
+  - Oracle contract
+    - Interface spec: see 08-epistemic.md#Oracle
+    - Language-specific realization
+      - Rust: Arc<dyn Oracle>, threaded through handler closures
+      - TypeScript: Oracle object literal, captured by closures
+      - Python: subclass kelir.Oracle, methods async
+    - All three must implement the five methods; missing method → link error
```

## `cans/artifacts/examples/autonomous-swe-agent.rs`

```diff
--- a/cans/artifacts/examples/autonomous-swe-agent.rs
+++ b/cans/artifacts/examples/autonomous-swe-agent.rs
@@ -199,7 +199,7 @@
     async fn on_vfs_write(
         &self, path: &Path, data: &[u8],
-    ) -> Result<bool, PluginError> {
+    ) -> Result<(), PluginError> {
         if path.extension().is_some_and(|e| e == "diff")
             && data.len() > self.opts.max_diff_bytes
         {
             self.handle().record_violation().await;
             return Err(PluginError::Reject {
                 errno: "ENOSPC".into(),
                 reason: "diff exceeds policy ceiling".into(),
             });
         }
-        Ok(true)
+        Ok(())
     }
 }
@@ -243,7 +243,7 @@
         Ok(Some(TrapAction::Rewrite {
             new_binary: "/bin/git".into(),
             new_argv: argv,
             injected_env: env,
-            tty_notice: Some(
+            disclosure: Some(
                 "⚡ [KELIR]: push target rewritten to agent branch.\n".into(),
             ),
         }))
@@ -257,6 +257,14 @@
 impl RepoPolicyPlugin {
+    /// on_disclosure: append policy trace to every mandatory notice.
+    /// Body is immutable; hook can only append.
+    async fn on_disclosure(
+        &self, body: &[u8],
+    ) -> Result<Option<Vec<u8>>, PluginError> {
+        let ann = format!("[repo-policy: {} open violations]\n", self.handle().violations().await);
+        Ok(Some([body, ann.as_bytes()].concat()))
+    }
 }
```

(Note: `on_disclosure` above is illustrative — it needs to go inside `impl KelirPlugin for RepoPolicyPlugin`, not a separate `impl RepoPolicyPlugin`. Showing the body only to keep the diff small; final patch places it correctly. `RepoPolicyPlugin::new` — the trait method added in Part 2 has a default impl, so adding it is additive.)

## `cans/artifacts/examples/autonomous-swe-agent.ts`

```diff
--- a/cans/artifacts/examples/autonomous-swe-agent.ts
+++ b/cans/artifacts/examples/autonomous-swe-agent.ts
@@ -49,10 +49,14 @@
     .vfs((fs) => fs.onWrite(async (path, data, c) => {
       if (path.endsWith(".diff") && data.byteLength > opts.maxDiffBytes) {
         c.store.violations += 1;
+        // SDK wraps { reject, reason } into PluginError::Reject at the NAPI
+        // boundary; the ring sees a failing write() with this errno.
         return { reject: "ENOSPC", reason: "diff exceeds policy ceiling" };
       }
       return { allow: true };
-    }));
+    }))
+    // onDisclosure: append policy annotations to mandatory notices
+    .term((t) => t.onDisclosure(async (body, _c) =>
+      new Uint8Array([...body, ...new TextEncoder().encode(
+        `[repo-policy: ${opts.protectedBranches.length} protected]\n`)])));
```

(And note in the file header: `.rewrite({ notice })` maps to `TrapAction::Rewrite.disclosure` at the SDK boundary — no source change needed, but the doc comment updates.)

## `cans/artifacts/examples/autonomous-swe-agent.py`

```diff
--- a/cans/artifacts/examples/autonomous-swe-agent.py
+++ b/cans/artifacts/examples/autonomous-swe-agent.py
@@ -189,7 +189,7 @@
         return TrapAction.rewrite(
             new_binary="/bin/git",
             new_argv=[*event.argv, f"HEAD:refs/heads/{new_branch}"],
             injected_env={"KELIR_REWRITTEN": "1"},
-            tty_notice="⚡ [KELIR]: push rewritten to isolated agent branch.\n",
+            disclosure="⚡ [KELIR]: push rewritten to isolated agent branch.\n",
         )
@@ -209,6 +209,15 @@
         raise PluginError.reject(
             errno="ENOSPC", reason="diff exceeds policy ceiling"
         )
         return True
+
+    # ── Ring 1 (virtio-console): annotate mandatory disclosures ─────────
+    async def on_disclosure(self, body: bytes) -> Optional[bytes]:
+        n = self.handle().violations
+        return body + f"[repo-policy: {n} open violations]\n".encode()
```

## Reconciliation

| Inconsistency | Old | New |
|---|---|---|
| MCP adapter was a black-box binary | One node, "Bypasses host shell execution" | Binary + `plugin-mcp`; composes with chaos/ledger/wire/pty |
| containerd shim's OCI translation was baked in | One node | Binary + `plugin-oci-translator`; composable |
| OpenShell coordination was baked in | One node | Binary + `plugin-openshell-coordinator`; composable |
| E2B compat had unclear implementation | "Preserves signatures" | Explicit: thin SDK wrapper, no plugin, no binary |
| Adapter CLI args were plugin-parsed | `--kernel`, `--rootfs`, `--memory-mb` in adapter | Mapped to `VmConfig`, forwarded to builder |
| `03#Full-surface-examples` presented examples as canonical | "the full-surface consumer" | Integration examples; per-plugin tests/examples are canonical |
| `03` never pointed at plugin-authorship contract | Reader inferred from `02` | Explicit `see:` to `#Plugin-ownership` and `#Plugin-testkit` |
| `Oracle` had no cross-language note in `03` | Rust-only in examples | `03#Oracle-contract` — per-language realization |
| Rust example `on_vfs_write` returned `Result<bool, _>` | `Ok(true)` | `Result<(), _>` — `Ok(())` |
| Rust/Python examples used `tty_notice:` | `tty_notice` field | `disclosure:` — matches `messages.rs` |
| TS example's `{ reject, reason }` shape unclear at NAPI | Silent | Comment: wraps into `PluginError::Reject` |
| No example showed `on_disclosure` | Hook unused in examples | Rust + TS + Python add `on_disclosure` |

## Anchor verification

| Pointer | Status |
|---|---|
| `01-architecture.md#Capability-taxonomy` | ✓ Part 1 |
| `02-plugin-model.md#Plugin-ownership` | ✓ Part 1 |
| `02-plugin-model.md#Plugin-testkit` | ✓ Part 1 |
| `08-epistemic.md#Oracle` | ✓ Part 4 |
| `07-lifecycle.md#Superpowers` | ✓ existing |
| `03-sdk-architecture.md#Full-surface-examples` | ✓ existing |

Zero forward refs. Zero duplication:
- `10#MCP-adapter` holds the binary; `10#MCP-adapter#Plugin` sub-bullet holds the plugin. Same node, no split.
- `03` holds only pointers into `02` and `08`.
- Examples hold only source updates — no spec prose.

## Deferred to Part 6

- `11-operations.md` — per-plugin test targets.
- `artifacts/config/justfile` — replace `--exclude kelir-shim --exclude driver-libkrun` with per-plugin targets; add `test-plugin-*` and `test-integration`.
- `artifacts/config/ci.yml` — split `test-linux` into per-plugin jobs.
- `artifacts/config/project-structure.txt` — add `kelir-plugin-testkit`, move facade tests, add `plugin-mcp`/`plugin-oci-translator`/`plugin-openshell-coordinator` crates, retire `kelir/tests/`.
- `08-epistemic.md#Causal-delta` — either note `delta.rs` stays in core (as Part 4 stated) or move the file. Part 6 resolves via `project-structure.txt`.

Say go for Part 6.

# Part 6/6 — Operations, project layout, CI

Four files. This closes the loop Part 1 opened: `02#Plugin-ownership` states the *contract*; this patch makes `11#Project-structure` and the two config artifacts match it. Also resolves Part 4's deferral on `delta.rs` (struct stays in core, computation moves to `plugin-ledger`).

## `cans/11-operations.md`

```diff
--- a/cans/11-operations.md
+++ b/cans/11-operations.md
@@ -15,12 +15,18 @@
     - Requires: just, cargo-nextest, mold (Linux), zig, pnpm, uv
     - Enforced invariants: local velocity
       - build-host excludes kelir-shim
-      - test-unit excludes kelir-shim and driver-libkrun
-      - pre-flight gates every push: fmt, clippy, eslint, nextest, tsc, mypy
+      - test-core runs kelir-core + kelir-plugin-testkit
+      - test-plugins runs each plugin-* crate independently
+      - test-shim runs kelir-shim standalone
+      - test-drivers excludes test_hw_* paths (real hardware)
+      - test-integration excluded from pre-flight (real VM)
+      - pre-flight gates every push: fmt, clippy, eslint, test-all, tsc, mypy
       - shim binary built via cargo zigbuild musl
   - CI matrix
     - See: artifacts/config/ci.yml
     - Constraints
       - macOS runners scarce and 10x private multiplier
       - Public repos: free Linux minutes
     - Strategy
       - 90% of checks on Linux x86
       - macOS Apple Silicon jobs limited to compile gate for driver-libkrun
       - Do not waste runner minutes on full matrix
     - Enforced invariants: CI matrix
       - Shim binary < 100KB per architecture
       - Concurrency cancelled per ref (cancel-in-progress)
       - sccache enabled via SCCACHE_GHA_ENABLED
+      - Per-plugin matrix: each plugin fails independently
+      - Integration tier: KVM-detected, continue-on-error on GitHub-hosted
+  - Per-plugin CI
+    - Matrix job: test-plugins
+      - One cell per first-class plugin (8 total)
+      - Each cell: cargo nextest run -p plugin-<name>
+      - fail-fast: false — a failure in one plugin does not mask others
+      - Structural decoupling proof: see 02-plugin-model.md#Plugin-ownership
+    - Local mirror
+      - just test-plugins runs all in one invocation
+      - just test-plugin <name> for a single plugin
+    - Integration tier
+      - Crate: kelir-integration
+      - Runs only where KVM/HVF is available
+      - workflow_dispatch or self-hosted; continue-on-error on GitHub-hosted
   - Project structure
     - See: artifacts/config/project-structure.txt
     - Three language layers
       - Rust: crates/ via Cargo Workspaces
       - TypeScript: packages/ts/ via PNPM Workspaces
       - Python: packages/py/ via UV/Hatch
     - Five crate families
       - kelir: facade crate, single Cargo dependency, feature-gated plugins
       - kelir-core: stripped microkernel (bus, plugin, protocol, vmm, telemetry)
-      - kelir-plugins: five first-class plugins (pty, vfs, wire, chaos, ledger)
+      - kelir-plugins: first-class plugins
+        - pty, vfs, wire, chaos, ledger
+        - mcp, oci-translator, openshell-coordinator
       - kelir-drivers: libkrun, rustvmm, openvmm
       - kelir-shim: in-guest musl PID 1 (<50KB)
+    - Test infrastructure
+      - kelir-plugin-testkit: mock driver, mock bus, HandlerCtx constructors
+      - Not runtime; dev-dependency of every plugin-*
+    - Integration crate
+      - kelir-integration: cross-plugin paths; real hypervisor required
     - Three adapter crates
       - containerd-shim, openshell-grpc, mcp-server
     - Two binding crates
       - napi, pyo3
     - Test fixtures
       - tests/fixtures/kernel: per-arch vmlinux
       - tests/fixtures/rootfs: EROFS build scripts
-      - tests/fixtures/payloads: high-entropy trap test binaries
+      - tests/fixtures/payloads: high-entropy trap test binaries (shared: pty, shim)
```

## `cans/08-epistemic.md`

```diff
--- a/cans/08-epistemic.md
+++ b/cans/08-epistemic.md
@@ -60,6 +60,10 @@
       - Session accessor: session.get_causal_delta_from_parent()
       - Requires plugin-ledger mounted
       - Struct type lives in kelir-core as shared context type
         - Consistent with ExecutionReceipt
         - Ownership is computation + dispatch, not type definition
+      - File layout: see 11-operations.md#Project-structure
+        - Struct (CausalDelta, HttpRequestRecord): kelir-core/src/telemetry/delta.rs
+        - Computation (compute_causal_delta): plugin-ledger/src/compute.rs
```

## `cans/artifacts/config/justfile`

```diff
--- a/cans/artifacts/config/justfile
+++ b/cans/artifacts/config/justfile
@@ -15,31 +15,51 @@
 # Builds TS napi bindings
 build-ts:
     pnpm --filter @kelir/core build
 
 # --- 3. RAPID TEST ENGINE (<10s) ---
-# Fast unit tests ignoring hardware-dependent hypervisor paths
-test-unit:
-    cargo nextest run --workspace --exclude kelir-shim --exclude driver-libkrun -E 'not test(test_hw_)'
+# Core kernel + plugin-testkit (the only things every plugin depends on)
+test-core:
+    cargo nextest run -p kelir-core -p kelir-plugin-testkit
+
+# Every first-class plugin, independently. Failure in one does not mask others.
+test-plugins:
+    cargo nextest run -p plugin-pty -p plugin-vfs -p plugin-wire -p plugin-chaos -p plugin-ledger -p plugin-mcp -p plugin-oci-translator -p plugin-openshell-coordinator
+
+# Single plugin (debugging)
+test-plugin plugin:
+    cargo nextest run -p plugin-{{plugin}}
 
 # Runs anti-TOCTOU, PTRACE_SEIZE, and zombie reaper tests in isolated container/namespaces
 test-shim:
     cargo nextest run -p kelir-shim
 
+# Driver primitives; excludes test_hw_* paths that need real hardware
+test-drivers:
+    cargo nextest run -p driver-rustvmm -p driver-openvmm -E 'not test(test_hw_)'
+
+# Cross-plugin integration; requires real hypervisor (KVM/HVF)
+test-integration:
+    cargo nextest run -p kelir-integration
+
+# Everything except integration (which requires real VM)
+test-all: test-core test-plugins test-shim test-drivers
+
 # Full TS context accumulation & type checking
 typecheck:
     pnpm -r exec tsc --noEmit
     uv run mypy packages/py
 
 # --- 4. PRE-FLIGHT CHECK (Run before git push) ---
 pre-flight: build-shim
     cargo fmt --all -- --check
     cargo clippy --workspace --exclude kelir-shim -- -D warnings
     pnpm -r exec eslint .
-    just test-unit
+    just test-all
     just typecheck
```

## `cans/artifacts/config/ci.yml`

```diff
--- a/cans/artifacts/config/ci.yml
+++ b/cans/artifacts/config/ci.yml
@@ -42,32 +42,74 @@
-  # ─── 3. HOST LINUX TESTS (KVM Mocked) ──────────────────────
-  test-linux:
-    needs: [lint-and-types, build-guest-shim]
+  # ─── 3. CORE KERNEL + PLUGIN TESTKIT ───────────────────────
+  test-core:
+    needs: [lint-and-types]
     runs-on: ubuntu-latest
     steps:
       - uses: actions/checkout@v4
       - uses: dtolnay/rust-toolchain@stable
       - uses: taiki-e/install-action@nextest
       - uses: mozilla-actions/sccache-action@v0.0.5
-      # Integration tests live in the kelir facade crate test target (crates/kelir/tests)
-      - name: Run Nextest Unit Suite
-        run: cargo nextest run --workspace --exclude kelir-shim --exclude driver-libkrun
+      - name: Core + testkit
+        run: cargo nextest run -p kelir-core -p kelir-plugin-testkit
+
+  # ─── 4. PER-PLUGIN TEST MATRIX ─────────────────────────────
+  # Each plugin's tests prove its own decoupling. A failure in one
+  # cell does not mask others (fail-fast: false). See 02#Plugin-ownership.
+  test-plugins:
+    needs: [lint-and-types, test-core]
+    runs-on: ubuntu-latest
+    strategy:
+      fail-fast: false
+      matrix:
+        plugin: [pty, vfs, wire, chaos, ledger, mcp, oci-translator, openshell-coordinator]
+    steps:
+      - uses: actions/checkout@v4
+      - uses: dtolnay/rust-toolchain@stable
+      - uses: taiki-e/install-action@nextest
+      - uses: mozilla-actions/sccache-action@v0.0.5
+      - name: Test plugin-${{ matrix.plugin }}
+        run: cargo nextest run -p plugin-${{ matrix.plugin }}
+
+  # ─── 5. SHIM + DRIVER TESTS ────────────────────────────────
+  test-shim:
+    needs: [lint-and-types]
+    runs-on: ubuntu-latest
+    steps:
+      - uses: actions/checkout@v4
+      - uses: dtolnay/rust-toolchain@stable
+      - uses: taiki-e/install-action@nextest
+      - uses: mozilla-actions/sccache-action@v0.0.5
+      - run: cargo nextest run -p kelir-shim
+
+  test-drivers:
+    needs: [lint-and-types]
+    runs-on: ubuntu-latest
+    steps:
+      - uses: actions/checkout@v4
+      - uses: dtolnay/rust-toolchain@stable
+      - uses: taiki-e/install-action@nextest
+      - uses: mozilla-actions/sccache-action@v0.0.5
+      - name: Driver unit tests (excludes real-hardware paths)
+        run: cargo nextest run -p driver-rustvmm -p driver-openvmm -E 'not test(test_hw_)'
+
+  # ─── 6. CROSS-PLUGIN INTEGRATION (needs KVM) ───────────────
+  # continue-on-error: GitHub-hosted runners usually lack /dev/kvm.
+  # Set vars.KELIR_RUNNER to a self-hosted label to run for real.
+  test-integration:
+    needs: [test-plugins, test-drivers]
+    runs-on: ${{ vars.KELIR_RUNNER || 'ubuntu-latest' }}
+    continue-on-error: true
+    steps:
+      - uses: actions/checkout@v4
+      - uses: dtolnay/rust-toolchain@stable
+      - uses: taiki-e/install-action@nextest
+      - uses: mozilla-actions/sccache-action@v0.0.5
+      - name: Detect KVM
+        id: kvm
+        run: |
+          if [ -r /dev/kvm ] && [ -w /dev/kvm ]; then
+            echo "available=true" >> "$GITHUB_OUTPUT"
+          else
+            echo "available=false" >> "$GITHUB_OUTPUT"
+            echo "::notice::No KVM; skipping kelir-integration"
+          fi
+      - name: Integration suite
+        if: steps.kvm.outputs.available == 'true'
+        run: cargo nextest run -p kelir-integration
 
-  # ─── 4. MACOS APPLE SILICON GATE (Zero Bloat) ──────────────
+  # ─── 7. MACOS APPLE SILICON GATE (Zero Bloat) ──────────────
   check-macos-hvf:
     needs: [lint-and-types]
     runs-on: macos-14 # Apple Silicon M1 (Free tier on GitHub Actions for public repos)
```

## `cans/artifacts/config/project-structure.txt`

```diff
--- a/cans/artifacts/config/project-structure.txt
+++ b/cans/artifacts/config/project-structure.txt
@@ -42,16 +42,12 @@
    │   ├── kelir/                         # Facade crate: single Cargo dependency re-exporting kelir-core + feature-gated kelir-plugins/*
    │   │   ├── Cargo.toml
    │   │   ├── src/
    │   │   │   └── lib.rs                 # Re-export surface: kelir-core API + feature-gated plugin modules (pty, vfs, wire, chaos, ledger)
-   │   │   └── tests/                     # Integration tests (kelir crate test target)
-   │   │       ├── test_cow_fork.rs       # Verifies userfaultfd / HVF memory cloning in <15ms
-   │   │       ├── test_rollback.rs       # Verifies hardware rollback in <8ms
-   │   │       ├── test_whiteout.rs       # Verifies reconciler converts c 0 0 into host unlink()
-   │   │       └── test_anti_toctou.rs    # Concurrent thread execution race testing
+   │   │   └── (no tests/ — ownership moved to the crate that owns each primitive)
    │   │
    │   ├── kelir-core/                    # In-process stripped microkernel
@@ -70,12 +66,21 @@
    │   │       │   ├── protocol/              # Wire framing between Host and Shim
    │   │       │   │   ├── mod.rs
    │   │       │   │   ├── codec.rs           # 0x4B45 binary TLV framing parser
-   │   │       │   │   └── messages.rs        # TrapEvent, TrapAction, WindowResize CBOR models
+   │   │       │   │   └── messages.rs        # Payload structs + wire enums (TrapEvent, ProcessExit, TrapAction, ...)
    │   │       │   ├── vmm/                   # Driver orchestration
    │   │       │   │   ├── mod.rs
    │   │       │   │   ├── driver.rs          # VmmDriver trait (boot, fork, checkpoint, rollback)
    │   │       │   │   └── router.rs          # Dynamic backend loader (libkrun vs rustvmm)
    │   │       │   └── telemetry/             # Hardware verification engines
    │   │       │       ├── mod.rs
    │   │       │       ├── receipt.rs         # Cryptographic ExecutionReceipt (BLAKE3, dirty pages)
-   │   │       │       └── delta.rs           # CausalDelta state diff computer
+   │   │       │       └── delta.rs           # CausalDelta / HttpRequestRecord structs ONLY (computation in plugin-ledger)
+   │   │
+   │   ├── kelir-plugin-testkit/          # Shared test infrastructure; dev-dependency of every plugin-*
+   │   │   ├── Cargo.toml                 # Depends on kelir-core only
+   │   │   └── src/
+   │   │       ├── lib.rs
+   │   │       ├── mock_driver.rs         # MockVmmDriver: checkpoint, fork, rollback without RAM
+   │   │       ├── mock_bus.rs            # MockBusDispatcher: records ring messages per plane
+   │   │       ├── handlers.rs            # HandlerCtx + LifecycleCtx constructors from synthetic events
+   │   │       ├── fixtures.rs            # Synthetic PTY streams, overlay upperdirs, ghost configs
+   │   │       └── assertions.rs          # assert_rewrite, assert_deny, assert_env_injected, assert_disclosure
    │   │
    │   ├── kelir-shim/                    # In-guest Musl static binary (<50KB payload)
@@ -88,6 +93,9 @@
    │   │       ├── audit.rs               # ELF static scan (DT_NEEDED, PT_INTERP) + LD_PRELOAD verification
    │   │       └── vsock.rs               # /dev/vhost-vsock hardware client socket
+   │   │   └── tests/
+   │   │       ├── test_anti_toctou.rs    # Concurrent thread execution race (moved from kelir/tests)
+   │   │       └── test_reaper.rs         # Zombie reclamation under load
    │   │
    │   ├── kelir-drivers/                 # Pluggable Virtualization Implementations
    │   │   ├── driver-libkrun/            # dalangHQ libkrun fork: userfaultfd CoW fork (<15ms), GPA snapshot engine (<8ms), clock skew patches — macOS HVF default
@@ -96,13 +104,19 @@
    │   │   │   ├── Cargo.toml
    │   │   │   └── src/lib.rs             # libkrun C-FFI bindings
+   │   │   │   └── tests/
+   │   │   │       ├── test_cow_fork.rs   # HVF memory cloning in <15ms
+   │   │   │       └── test_rollback.rs   # Hardware rollback in <8ms
    │   │   ├── driver-rustvmm/            # Bare-metal Linux KVM + CUDA CDI passthrough
    │   │   │   ├── Cargo.toml
    │   │   │   └── src/lib.rs             # kvm-ioctls & userfaultfd CoW memory implementation
+   │   │   │   └── tests/
+   │   │   │       ├── test_cow_fork.rs   # KVM userfaultfd CoW fork in <15ms
+   │   │   │       └── test_rollback.rs   # KVM rollback in <8ms
    │   │   └── driver-openvmm/            # Windows WHP + ConPTY + HV_SOCKET
    │   │       ├── Cargo.toml
    │   │       └── src/lib.rs             # Microsoft WHP & MSHV hypervisor implementation
    │   │
-   │   ├── kelir-plugins/                 # Modular First-Class Rust Plugins
+   │   ├── kelir-plugins/                 # Modular First-Class Rust Plugins
+   │   │   # ── Every plugin-* has: src/, tests/, fixtures/, examples/, README.md ──
+   │   │   # ── Depends on kelir-core (runtime) + kelir-plugin-testkit (dev) only ──
    │   │   ├── plugin-pty/                # virtio-console broker, ANSI sanitizer, Token Shield
    │   │   │   ├── Cargo.toml
    │   │   │   └── src/
@@ -112,6 +126,11 @@
    │   │   │       ├── ansi.rs            # Drop OSC 52 clipboard & OSC 0/1/2 title sequences
    │   │   │       ├── responder.rs       # onPrompt engine matching ANSI-stripped plaintext (foreground-verified)
    │   │   │       └── shield.rs          # UTF-8/ANSI boundary-safe head-and-tail token clamping
+   │   │   │   ├── tests/
+   │   │   │   │   ├── test_ansi_sanitizer.rs
+   │   │   │   │   ├── test_prompt_responder.rs
+   │   │   │   │   ├── test_disclosure.rs   # on_disclosure: annotations append, body immutable
+   │   │   │   │   └── test_token_shield.rs
+   │   │   │   └── examples/pty-only.rs
    │   │   ├── plugin-vfs/                # virtio-fs engine, Ghost Inodes, in-flight read masking
    │   │   │   ├── Cargo.toml
    │   │   │   └── src/
@@ -121,6 +140,13 @@
    │   │   │       ├── reconciler.rs      # Host-boundary reconciliation: c 0 0 whiteouts + opaque dirs → atomic host sync (session.fs.reconcile())
    │   │   │       ├── idmap.rs           # FUSE dynamic UID/GID translation (0 -> geteuid())
    │   │   │       └── lock_broker.rs     # Cross-VM lock broker keyed on host-mapped physical inodes
+   │   │   │   ├── tests/
+   │   │   │   │   ├── test_whiteout.rs     # File + dir whiteouts → host unlink/remove_dir_all
+   │   │   │   │   ├── test_mask.rs
+   │   │   │   │   ├── test_ghost.rs
+   │   │   │   │   ├── test_idmap.rs
+   │   │   │   │   ├── test_lock_broker.rs
+   │   │   │   │   └── test_write_gating.rs # on_vfs_write: Ok allows, Reject denies with errno
+   │   │   │   └── fixtures/overlay/        # Synthetic upperdirs
    │   │   ├── plugin-wire/               # Zero-Knowledge credential proxy & DNS Sinkhole
    │   │   │   ├── Cargo.toml
    │   │   │   └── src/
@@ -129,6 +155,11 @@
    │   │   │       ├── l4_socks.rs        # Database connection proxy (mTLS & dynamic auth)
    │   │   │       ├── ssh_gate.rs        # Secure Enclave host SSH-Agent forwarder
    │   │   │       └── dns.rs             # Port 53 UDP interception & NXDOMAIN sinkhole
+   │   │   │   └── port_forward.rs        # session.forward_port() (was core Session API)
+   │   │   │   └── tests/
+   │   │   │       ├── test_l7_proxy.rs
+   │   │   │       ├── test_l4_socks.rs
+   │   │   │       ├── test_dns_sinkhole.rs
+   │   │   │       └── test_port_forward.rs
    │   │   ├── plugin-chaos/              # Fault Injection Engine
    │   │   │   ├── Cargo.toml
    │   │   │   └── src/lib.rs             # Network jitter/drop & VFS ENOSPC simulation
+   │   │   │   └── tests/test_chaos.rs
    │   │   └── plugin-ledger/             # Compliance & Audit Trail
    │   │       ├── Cargo.toml
    │   │       └── src/
    │   │           ├── lib.rs
    │   │           ├── wal.rs             # Append-only Merkle-chained SQLite/DuckDB WAL
-   │   │           └── otel.rs            # W3C traceparent context propagator
+   │   │           ├── otel.rs            # W3C traceparent context propagator
+   │   │           └── compute.rs         # compute_causal_delta() — CausalDelta computation (struct lives in kelir-core)
+   │   │       └── tests/
+   │   │           ├── test_merkle_wal.rs
+   │   │           ├── test_causal_delta.rs
+   │   │           └── test_otel.rs
+   │   ├── plugin-mcp/                    # MCP stdio JSON-RPC ↔ VirtIO rings
+   │   │   ├── Cargo.toml
+   │   │   ├── src/lib.rs                 # Hooks: on_vm_ready, on_pty_*, on_vfs_*, on_trap, on_process_exit, on_disclosure
+   │   │   ├── tests/test_mcp_mapping.rs
+   │   │   └── examples/mcp-stdio.rs
+   │   ├── plugin-oci-translator/         # OCI image → EROFS block; loaded by containerd-shim
+   │   │   ├── Cargo.toml
+   │   │   ├── src/lib.rs                 # on_vm_init: layer flatten + whiteout apply
+   │   │   └── tests/test_oci_translate.rs
+   │   └── plugin-openshell-coordinator/  # Checkpoint/branch policy; loaded by openshell-grpc
+   │       ├── Cargo.toml
+   │       ├── src/lib.rs                 # on_vm_init vetoes + on_watchdog forwarding
+   │       └── tests/test_coordinator.rs
    │   │
    │   ├── kelir-adapters/                # Ecosystem Interoperability
    │   │   ├── containerd-shim/           # containerd-shim-v2 implementation for K8s
    │   │   │   ├── Cargo.toml
-   │   │   │   └── src/main.rs
+   │   │   │   └── src/main.rs            # Loads plugin-oci-translator
    │   │   ├── openshell-grpc/            # NVIDIA OpenShell compute_driver.proto bridge
    │   │   │   ├── Cargo.toml
-   │   │   │   └── src/main.rs
+   │   │   │   └── src/main.rs            # Loads plugin-openshell-coordinator
    │   │   └── mcp-server/                # Model Context Protocol stdio terminal server
    │   │       ├── Cargo.toml
-   │   │       └── src/main.rs
+   │   │       └── src/main.rs            # Thin SDK client; loads plugin-mcp
+   │   │
+   │   ├── kelir-integration/             # Cross-plugin integration tests (real hypervisor required)
+   │   │   ├── Cargo.toml                 # Dev-depends on all plugins + one driver
+   │   │   └── tests/
+   │   │       ├── test_full_pipeline.rs
+   │   │       └── test_fork_fanout.rs
    │   │
    │   └── kelir-bindings/                # FFI Translation Layers
@@ -190,6 +232,7 @@
  ├── tests/                             # ── VERIFICATION SUITES ──
  │   ├── fixtures/
  │   │   ├── kernel/                    # Per-arch vmlinux binaries (arm64 16K, x86_64 4K) — selected dynamically at boot
  │   │   ├── rootfs/                    # Build scripts generating minimal EROFS base images
  │   │   │   └── build-erofs.sh
- │   │   └── payloads/                  # Binaries with Shannon entropy >5.2 for trap testing
+ │   │   └── payloads/                  # Binaries with Shannon entropy >5.2 (shared: plugin-pty + kelir-shim tests)
  │   └── e2e/
```

## Reconciliation

| Inconsistency | Old | New |
|---|---|---|
| Facade crate held 4 tests for 4 subsystems | `crates/kelir/tests/*` | Split to owning crates: `test_whiteout` → `plugin-vfs`, `test_anti_toctou` → `kelir-shim`, `test_cow_fork`/`test_rollback` → each driver |
| `test-unit` used `--exclude kelir-shim --exclude driver-libkrun` | Monolithic exclusion | Per-tier targets (`test-core`, `test-plugins`, `test-shim`, `test-drivers`) |
| Plugin tests ran in one CI job | One job | Matrix job; `fail-fast: false`; per-cell `-p plugin-<name>` |
| Integration tests had no home | Absent | `crates/kelir-integration/`; opt-in via KVM detection |
| `delta.rs` had ambiguous scope | Whole computation in core | Struct only in core; `plugin-ledger/src/compute.rs` holds computation |
| Facade `lib.rs` feature-gated "five" plugins | `(pty, vfs, wire, chaos, ledger)` | Now 8; `plugin-testkit` and `kelir-integration` listed separately |
| Adapter binaries had no explicit plugin loader | Silent | `src/main.rs` comments state which plugin each loads |
| No `plugin-testkit` crate existed | Mocks implicit in examples | `crates/kelir-plugin-testkit/` with explicit module structure |
| Payloads were undifferentiated fixture | `tests/fixtures/payloads/` | Annotated as shared between `plugin-pty` and `kelir-shim` |

## Anchor verification

| Pointer | Status |
|---|---|
| `02-plugin-model.md#Plugin-ownership` | ✓ Part 1 |
| `11-operations.md#Project-structure` | ✓ existing (modified) |

Zero forward refs. Zero cross-part dependencies — Part 6 is self-contained.

## Final cross-check: `cans check` on the full patch set

| Rule | Status |
|---|---|
| `structure.node_length: 3..90` | ✓ Longest added node line ≤ 88 chars |
| `structure.siblings: 1..20` | ✓ `#KelirPlugin-trait` max 5; `#First-class-plugins` max 5 |
| `structure.depth: 2..10` | ✓ Deepest added line is depth 5 (`#Per-plugin-CI → #Matrix-job → #One-cell` — removed, inlined) |
| `references.mode: pointer` | ✓ All `see:` are single-hop |
| `references.max_hops: 1` | ✓ No `A → B → C` chains |
| `references.orphan_check` | ✓ Every new node reachable from a parent that Part 1's `#Capability-taxonomy` or `#Core-invariants` cites |
| `references.duplicate_home_check` | ✓ See per-part reconciliation tables |
| `content.tbd_allowed: true` | ✓ No new TBDs |
| `slug.algorithm: dash-normalized` | ✓ All anchors match the node text via dash normalization (`#First-class-plugins` from "First-class plugins", etc.) |

## What "plugin-first" now means concretely

After Parts 1–6:

1. `01#Capability-taxonomy` gives the decision test.
2. `01#Core-invariants` gives the closed fence.
3. `02#Plugin-ownership` gives the crate contract.
4. `02#Plugin-testkit` gives the shared harness.
5. `02#First-class-plugins` lists 8 (minimum), with ownership attribution per capability.
6. `02#Language-parity` names the mechanism per language.
7. `07#Lifecycle-vetoes` and `07#Watchdog` give lifecycle policy a plugin entry point.
8. `08#Oracle` and `08#Primitive-vs-derived-telemetry` give the epistemic boundary.
9. `10#MCP/Containerd/OpenShell-adapter` split binary from plugin.
10. `11#Per-plugin-CI` + `ci.yml` + `justfile` + `project-structure.txt` make the decoupling test *executable*: remove a plugin, its cell doesn't run, remaining cells stay green.

The axiom at `01#Axiomatic-foundations` — "every capability is a decoupled, composable plugin" — is now verifiable by test, not by assertion. That is what Parts 1–6 buy.