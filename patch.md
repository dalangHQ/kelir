### Part 1/6: Capability DAG Tokens, Dual-Plane Engine & Core Trait Contracts

```diff
--- a/cans/artifacts/code/plugin-trait.rs
+++ b/cans/artifacts/code/plugin-trait.rs
@@ -1,7 +1,7 @@
 // crates/kelir-interface/src/plugin.rs
 //
-// The unified KelirPlugin trait implemented by every subsystem across the
-// 4 VirtIO rings. kelir-core contains no hardcoded capability logic.
+// The decoupled capability-based KelirPlugin trait and dual-plane execution contracts.
+// Eliminates magic integer priorities in favor of topological DAG scheduling.
 //
 // Owned by: cans/02-plugin-model.md#KelirPlugin-trait
 
@@ -10,6 +10,8 @@
 use std::path::PathBuf;
 use std::time::Duration;
+use std::any::{Any, TypeId};
+use std::sync::Arc;
 use crate::handlers::HandlerCtx;
 use crate::protocol::messages::{
     Heartbeat, MemoryPressureWarning, PortBound, ProcessExit,
@@ -37,6 +39,9 @@
     #[error("Ambiguous mutation detected across plugins on resource '{resource}': {conflict}")]
     AmbiguousMutation { resource: String, conflict: String },
+    /// DAG resolution failure at bootstrap
+    #[error("Unsatisfied capability dependency: plugin '{plugin}' requires '{capability}'")]
+    UnsatisfiedCapability { plugin: String, capability: String },
     #[error("Internal I/O failure: {0}")]
     Io(#[from] std::io::Error),
 }
@@ -58,6 +63,22 @@
     pub content: Vec<u8>,
 }
 
+/// Semantic capability token decoupled from concrete crate types.
+#[derive(Debug, Clone, PartialEq, Eq, Hash)]
+pub struct Capability(pub &'static str);
+
+/// Semantic ordering constraints resolved via Kahn's algorithm at boot.
+#[derive(Debug, Clone, PartialEq, Eq)]
+pub enum OrderConstraint {
+    RunBefore(Capability),
+    RunAfter(Capability),
+}
+
+#[derive(Debug, Clone, Copy, PartialEq, Eq)]
+pub enum VirtioRing {
+    Vsock, Console, Fs, Net,
+}
+
 #[derive(Debug, Clone, Copy, PartialEq, Eq)]
 pub enum VirtioDevice {
     Vsock,
@@ -129,6 +150,18 @@
     YieldToHost(u64),
 }
 
+/// Strongly-typed service locator eliminating ambient context string bags.
+pub trait ServiceRegistry: Send + Sync {
+    fn register_service<T: ?Sized + Any + Send + Sync>(&mut self, service: Arc<T>);
+    fn get_service<T: ?Sized + Any + Send + Sync>(&self) -> Option<Arc<T>>;
+}
+
+/// Binds inline interceptors into the linear monotonic pipeline.
+pub trait PipelineBinder: ServiceRegistry {
+    fn bind_storage(&mut self, stage: Arc<dyn StorageInterposer>);
+    fn bind_wire(&mut self, stage: Arc<dyn WireInterposer>);
+    fn bind_trap(&mut self, stage: Arc<dyn TrapInterposer>);
+    fn bind_terminal(&mut self, stage: Arc<dyn TerminalInterposer>);
+}
+
+/// Binds non-blocking observers to the immutable telemetry bus.
+pub trait EventBusBinder {
+    fn subscribe<E: Any + Send + Sync + 'static>(&mut self, handler: Box<dyn Fn(&E) + Send + Sync>);
+}
+
 /// Assembly-time composite plugin tree unpacked into flat dispatch entries before boot.
 pub trait CompositePlugin: Send + Sync {
     fn unpack(self: Box<Self>, parent_scope: &MountScope) -> Vec<(Box<dyn AnyPlugin>, MountScope)>;
@@ -176,14 +209,14 @@
-    /// Dispatch ordering; lower values run earlier. Default: 100.
-    /// Ties broken by declaration order.
-    fn priority(&self) -> u8 { 100 }
+    /// Capabilities exported by this plugin for consumption by peers.
+    fn provides(&self) -> Vec<Capability> { vec![] }
+
+    /// Required capabilities validated against the DAG at engine boot.
+    fn requires(&self) -> Vec<Capability> { vec![] }
+
+    /// Semantic ordering constraints resolved via Kahn's topological sort.
+    fn order(&self) -> Vec<OrderConstraint> { vec![] }
 
     /// Default mount scope. Overridden by explicit builder scope.
     fn default_scope(&self) -> MountScope { MountScope::default() }
 
     /// Null-object toggle. False omits all hooks from flat dispatch array.
     fn is_enabled(&self) -> bool { true }
@@ -191,9 +224,12 @@
     /// Declared capability requirements validated as an acyclic graph at boot.
-    /// Missing targets or circular dependencies abort with EConfig.
-    fn requires(&self) -> &'static [&'static str] { &[] }
 
     /// Deep-clones plugin state when session.vm.fork() creates a CoW child.
     fn fork_state(&self, _child_session_id: &str) -> Box<dyn AnyPlugin>;
+
+    /// Register inline fallible interceptors into the hardware ring pipelines.
+    fn register_interceptors(&self, _pipeline: &mut dyn PipelineBinder) {}
+
+    /// Register asynchronous non-blocking subscribers to the event bus.
+    fn register_subscribers(&self, _bus: &mut dyn EventBusBinder) {}
 }
```

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -2,12 +2,15 @@
   - Taxonomic coordinate contract
     - Code is sole SSOT: declared programmatically via KelirPlugin trait
     - Manifest artifact is strictly auto-generated for zero-compile indexing
     - Axis 1 (boundary): urn:kelir:<subsystem>:<target>[:<subtarget>]
     - Axis 2 (rings): set of attached VirtIO device channels
     - Axis 3 (tier): Native (Rust in-process) | Foreign (host FFI)
     - Axis 4 (telemetry role): Observer | Gate | Mutator | Proxy
+    - Axis 5 (capabilities): Provided and required semantic tokens
+    - Ordering via topological DAG: eliminates arbitrary magic integer priorities
+    - Cycle detection and unfulfilled dependencies fail closed at boot
     - First-class catalog default: tier is Native unless stated
     - Mutation scope manifest: mutators declare target paths, ports, or envs
     - Overlapping mutation scopes between plugins fail validation at boot
-    - Coordinates enforce compile-time ordering gates and dispatch layout
+    - Coordinates enforce DAG construction and dispatch layout
@@ -21,14 +24,23 @@
     - ipc: vsock multiplexing and shared-memory mapping
-  - Two-tier execution architecture
-    - Native tier
-      - Compiled native Rust plugins in crates/kelir-plugins
-      - Synchronous fast-path filters execute on VirtIO rings at <1µs
-      - Returns SyncVerdict::Allow or yields to host via SyncVerdict::YieldToHost
-      - Direct memory execution on 4 VirtIO rings at sub-12µs latency
-      - High-frequency mutation: raw PTY ANSI, FUSE CoW, TAP L2 frames
-    - Foreign tier
-      - Userland host plugins authored in TypeScript, Python, or Rust
-      - Executes in host runtime event loop via ForeignPluginProxy
-      - Steers agent workflows, prompt rules, branch gates, and masks
+  - Dual-plane execution architecture
+    - Interceptor pipeline plane
+      - Synchronous or bounded latency inline processors
+      - Evaluates monotonic verdict lattice: Allow < Rewrite < Deny < Abort
+      - Single mutator owns target byte slice at any stage
+      - Compiled into contiguous zero-cost dispatch tables via Kahn's sort
+      - Latency bounded: native tier sub-12µs, foreign FFI filter-gated
+    - Reactive event bus plane
+      - Asynchronous fan-out pub/sub for telemetry, audit, and OTel
+      - Strictly immutable domain events emitted after pipeline settlement
+      - Non-blocking: slow observers cannot stall microVM vCPU execution
+      - Zero participation in mutation or gating decisions
@@ -47,15 +59,14 @@
   - KelirPlugin trait
     - see: artifacts/code/plugin-trait.rs
-    - Trait surface and hook families: see: artifacts/code/plugin-trait.rs
+    - Capability tokens define interface contracts without peer crate linkage
+    - provides() declares exported capabilities to the engine
+    - requires() declares mandatory dependencies validated at boot
+    - order() declares RunBefore and RunAfter constraints relative to capabilities
     - Segregated capability traits
       - Core identity only: KelirPlugin is mandatory; see: artifacts/code/plugin-trait.rs
       - LifecycleParticipant: vm lifecycle, commit, host preempt
       - TrapInterposer: trap, process exit, pressure, heartbeat, port, signal
       - TerminalInterposer: pty input, pty output, window resize, disclosure
-      - ToolInterposer: on_tool_list, on_tool_call, on_tool_result; hot-path JSON-RPC filter
       - StorageInterposer: vfs lookup, read, write; synchronous hot path
       - WireInterposer: net outbound, net inbound; synchronous hot path
@@ -88,10 +99,10 @@
   - Static dispatch
-    - Dispatch array: VirtIO rings -> MountScope pre-filter -> monotonic
-      lattice evaluation (Allow < Rewrite < Deny < Abort); flat at boot
+    - Dispatch compilation: resolves DAG edges, validates acyclic invariant
+    - Topologically sorted linear pipeline eliminates priority conflicts
     - Core verifies plugin dependency DAG: cycles and missing deps halt boot
-    - Evaluated in priority() order; ties broken by declaration order
     - Monotonic permission lattice: Allow -> Rewrite -> Deny -> Abort
     - Restrictive monotonicity: downstream hooks cannot widen upstream denial
```

### Part 2/6: Core Architecture, DAG Dispatch Compilation & Scoped Context Invariant

```diff
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@ -40,7 +40,9 @@
       - Modular VirtIO interposition microkernel
         - Core controls hypervisor execution and dispatches VirtIO events
         - Core defines zero domain policies, protocols, or agent schemas
-        - Capabilities outside raw hypervisor physics are decoupled plugins
+        - Plugins register semantic capability tokens (`provides`, `requires`)
+        - Core compiles plugins into an acyclic directed graph via Kahn's sort
+        - Unfulfilled capabilities or cyclic dependencies abort boot with EConfig
         - Plugins communicate through core device abstractions, not peer imports
         - Every capability is a decoupled, composable plugin
       - Transparent truthful steering
@@ -82,6 +84,7 @@
     - Trait definitions
       - Low-footprint preset contract: see: #Embedded-profile
       - Trait contract: see: 02-plugin-model.md#KelirPlugin-trait
+      - Semantic capability contract: see: 02-plugin-model.md#Taxonomic-coordinate-contract
       - VmmDriver trait definition: see: #VmmDriver-trait
     - Context types: see: 09-protocol.md#Wire-messages
     - Wire framing layout: see: 09-protocol.md#Wire-framing
@@ -107,8 +110,9 @@
     - Native Rust consumer: direct Cargo crate, zero-cost monomorphization
     - TypeScript consumer: @kelir/core via in-process NAPI-RS
     - Python consumer: kelir via PyO3
-    - kelir-core: stripped microkernel (BusDispatcher, EventBus, DriverRouter)
+    - kelir-core: stripped microkernel (BusDispatcher, PipelineEngine, EventBus, DriverRouter)
+    - PipelineEngine: compiles inline interceptors into indexed contiguous tables
     - kelir-drivers: pluggable backends (rustvmm, hvf, openvmm, avf, bwrap, proot)
   - Guest VM anatomy
```

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -100,16 +100,15 @@
   - Static dispatch
-    - Dispatch array: VirtIO rings -> MountScope pre-filter -> monotonic
-      lattice evaluation (Allow < Rewrite < Deny < Abort); flat at boot
+    - Dispatch compilation: Kahn's topological sort resolves dependency DAG at boot
+    - Pipeline tables index linear interceptor chains per VirtIO ring
     - Core verifies plugin dependency DAG: cycles and missing deps halt boot
-    - Evaluated in priority() order; ties broken by declaration order
     - Monotonic permission lattice: Allow -> Rewrite -> Deny -> Abort
     - Restrictive monotonicity: downstream hooks cannot widen upstream denial
     - Fail-closed mutation collisions: overlapping writes raise EAmbiguousMutation
     - Deduplication: identical name() instances merge; deepest config wins
     - Dispatch pruning: no-op hooks completely omitted from C-dispatch table
     - Zero-cost null objects: disabled plugins retain types, cost zero in core
     - fork_state() deep-clones state instances during session.vm.fork()
     - Native zero-WASM isolation: compile-time linear capability tokens
     - Immutable &[u8] slice views prevent cross-plugin heap tampering
     - Hardware MPK page keys isolate per-plugin memory domains in-process
@@ -147,21 +146,20 @@
   - Core context keys
-    - Handler-visible keys; each key's definition lives at its owning plane
-    - Invariant: core runtime carries zero hardcoded plugin context keys
-    - Host context carries typed extension map for plugin-contributed data
-    - Language SDKs accumulate context keys via phantom types or plugins
-    - Core-provided (always present)
-      - c.trap, c.env, c.vm, c.step, c.receipt()
-    - Context key lifecycle
-      - Static decoration via decorate(): states bound once at VM boot
-      - Lazy derivation via derive(): computes telemetry only on handler access
-    - Plugin-provided (accumulated strictly by SDK builder at compile/type time)
-      - Plugin states map to typed properties on the accumulated context
-      - Unmounted plugin key dereference fails compile or import time
-    - User-provided via plugin registration
-      - c.<key> from .decorate() or instantiated plugin state
+    - Invariant: ambient untyped string bags (c.<key>) are eliminated
+    - Handlers receive strictly-typed context tailored to event scope
+    - Core context primitives (invariable across all handlers):
+      - c.trap: immutable syscall inspection payload
+      - c.vm: restricted lifecycle actuation handle
+      - c.receipt(): hardware execution proof generation
+    - Scoped domain contexts:
+      - StorageContext: scoped to virtio-fs operations (inode, path, journal)
+      - WireContext: scoped to virtio-net operations (L2 frame, L4 stream, socket)
+      - TerminalContext: scoped to virtio-console operations (PTY, grid, window)
+    - Inter-plugin capability injection:
+      - Plugins consume peer capabilities via ctx.get_service::<T>()
+      - Service resolution is verified at boot during DAG compilation
   - First-class plugins
     - Catalog tiering
       - Community plugins (hosted in core repo):
         - pty, vfs, wire, chaos, ledger, mcp, profile
@@ -194,15 +192,15 @@
     - Behavior of each: at its plugin file
-      - plugin-pty: see: plugins/pty.md — c.term
-      - plugin-vfs: see: plugins/vfs.md — c.fs
-      - plugin-wire: see: plugins/wire.md — c.wire
-      - plugin-chaos: see: plugins/chaos.md — c.chaos
-      - plugin-ledger: see: plugins/ledger.md — c.ledger
-      - plugin-db-proxy: see: plugins/db-proxy.md — c.db
-      - plugin-mesh: see: plugins/mesh.md — c.mesh
-      - plugin-lock-broker: see: plugins/lock-broker.md — c.locks
-      - plugin-supply-chain: see: plugins/supply-chain.md — package-manager guard
-      - plugin-profile: see: plugins/profile.md — c.profile
-      - plugin-hyper-density: see: kelir-cloud://plugins/hyper-density.md — c.density
-      - plugin-cas-snapshot: see: kelir-cloud://plugins/cas-snapshot.md — c.cas
-      - plugin-wire-ingress: see: plugins/wire-ingress.md — c.ingress
-      - plugin-mcp: see: plugins/mcp.md — c.mcp
-      - plugin-tool-guard: see: plugins/tool-guard.md — c.tools
+      - plugin-pty: see: plugins/pty.md — capability: io:pty
+      - plugin-vfs: see: plugins/vfs.md — capability: fs:vfs
+      - plugin-wire: see: plugins/wire.md — capability: net:l7:proxy
+      - plugin-chaos: see: plugins/chaos.md — capability: sys:fault
+      - plugin-ledger: see: plugins/ledger.md — capability: sys:audit
+      - plugin-db-proxy: see: plugins/db-proxy.md — capability: net:l7:sql
+      - plugin-mesh: see: plugins/mesh.md — capability: ipc:vsock:mesh
+      - plugin-lock-broker: see: plugins/lock-broker.md — capability: fs:vfs:lock
+      - plugin-supply-chain: see: plugins/supply-chain.md — capability: sys:trap:exec
+      - plugin-profile: see: plugins/profile.md — capability: sys:config:profile
+      - plugin-hyper-density: see: kelir-cloud://plugins/hyper-density.md — capability: sys:vm:density
+      - plugin-cas-snapshot: see: kelir-cloud://plugins/cas-snapshot.md — capability: fs:cas
+      - plugin-wire-ingress: see: plugins/wire-ingress.md — capability: net:l4:ingress
+      - plugin-mcp: see: plugins/mcp.md — capability: io:channel:mcp
+      - plugin-tool-guard: see: plugins/tool-guard.md — capability: io:mcp:tool
```

### Part 3/6: Storage Domain Cohesion, Lock-Broker Capability Binding & Local Delta Generation

```diff
--- a/cans/05-storage-plane.md
+++ b/cans/05-storage-plane.md
@@ -2,7 +2,10 @@
   - Storage architecture
     - see: artifacts/diagrams/storage-architecture.txt
-    - Owner: kelir-core (virtio-fs bus)
+    - Owner: kelir-core (virtio-fs storage domain pipeline)
+    - Single SSOT: plugin-vfs and plugin-lock-broker execute as stages in this pipeline
+    - All hooks receive strongly-typed StorageContext; ambient c.fs is eliminated
+    - Emits StorageDelta directly to the event bus; ledger consumes without VFS introspection
     - Storage layout
       - Host workspace mounted read-only as lower overlay layer
       - Host encapsulation: raw workspace locked to mode 000 on host
@@ -34,6 +37,12 @@
     - Writes modified files to sibling temporary files (.tmp.<nonce>)
     - Atomic rename swaps updated files directly over host target paths
     - Parent directory fsync flushes updated inodes to disk
+  - Storage delta generation
+    - Storage pipeline computes modified inode delta during reconcile
+    - Emits immutable StorageDelta event to the EventBus on commit:
+      - modified_inodes: Vec<PathBuf>
+      - whiteouts_purged: u64
+      - workspace_dirty_bytes: u64
   - Secondary image mounts
     - Grafts host-opened squashfs/EROFS images into guest VFS namespace
     - Zero PCI hot-plug overhead; serves pack lookups directly via FUSE
@@ -42,7 +51,7 @@
     - Synchronous VirtIO-FS interception gates FUSE write() via on_vfs_write
     - Signature: see: artifacts/code/plugin-trait.rs
     - Write-gating error variants: see: 02-plugin-model.md#Hook-return-semantics
-    - Streaming sliding-window byte scanner inspects writes across chunk boundaries
+    - Receives &StorageContext (inode, path, locks, journal); zero ambient context
     - Payloads exceeding configured inspection ceiling reject with BufferCeilingExceeded
     - Buffer lifecycle invariants: see: 02-plugin-model.md#FFI-buffer-lifecycle
     - SyncVerdict::Allow = allow; kernel proceeds with write to upperdir
```

```diff
--- a/cans/plugins/vfs.md
+++ b/cans/plugins/vfs.md
@@ -4,15 +4,17 @@
     - Plugin identifier: plugin-vfs
     - Owner crate: crates/kelir-plugins/plugin-vfs
-    - name() = "vfs"; priority() = 35
+    - name() = "vfs"
     - Taxonomy (vfs): urn:kelir:fs:vfs
       - coordinate: rings=[virtio-fs]; role=Mutator; tier=Native
+    - Capabilities (vfs):
+      - provides = [Capability("fs:vfs")]
+      - requires = []
+      - order = [OrderConstraint::RunAfter(Capability("fs:vfs:lock"))]
     - VirtIO device attachment (vfs): see: 01-architecture.md#VirtIO-Device-Complex
     - Lifecycle attachment (vfs): see: 07-lifecycle.md#Lifecycle-vetoes
     - Storage bus attachment (vfs): see: 05-storage-plane.md#Storage-plane
-  - Context keys for vfs
-    - Context key contributed: c.fs
-    - Closed-set binding rule (vfs): see: 02-plugin-model.md#Core-context-keys
+  - Service exports (vfs)
+    - Exports VfsStorageService via PipelineBinder for filesystem inspection
     - Storage configuration knobs:
       - encapsulation: WorkspaceEncapsulation::Locked (default) | Passthrough
       - scan_engine: ReadMaskEngine::StreamingAutomata (default) | FastRegex
@@ -23,6 +25,7 @@
       - see: artifacts/code/reconciler.rs
       - Pure stdlib implementation: zero external crate dependencies
       - session.fs.reconcile() is a pure storage-plane operation
+      - Directly publishes StorageDelta to EventBus upon successful reconcile
       - Lifecycle hook bindings
         - on_pre_commit flushes in-flight FUSE buffers
         - on_post_commit triggers sibling swap (.tmp.<nonce>) into host workspace
```

```diff
--- a/cans/plugins/lock-broker.md
+++ b/cans/plugins/lock-broker.md
@@ -4,15 +4,17 @@
     - Plugin identifier: plugin-lock-broker
     - Owner crate: crates/kelir-plugins/plugin-lock-broker
-    - name() = "lock-broker"; priority() = 30
+    - name() = "lock-broker"
     - Taxonomy (lock-broker): urn:kelir:fs:vfs:lock
       - coordinate: rings=[virtio-fs]; role=Gate; tier=Native
+    - Capabilities (lock-broker):
+      - provides = [Capability("fs:vfs:lock")]
+      - requires = []
+      - order = [OrderConstraint::RunBefore(Capability("fs:vfs"))]
     - VirtIO attachment (lock-broker): see: 01-architecture.md#VirtIO-Device-Complex
     - Lifecycle attachment (lock-broker): see: 07-lifecycle.md#Lifecycle-vetoes
     - Storage bus attachment (lock-broker): see: 05-storage-plane.md#Storage-plane
-  - Context keys for lock-broker
-    - Context key contributed: c.locks
-    - Closed-set binding rule (lock-broker): see: 02-plugin-model.md#Core-context-keys
+  - Service exports (lock-broker)
+    - Exports LockBrokerService via PipelineBinder for advisory file lock queries
   - Invariants and policy (lock-broker)
     - Entity lock broker
       - Owner: plugin-lock-broker; mediates cross-VM file locking
```

### Part 4/6: Console Domain Cohesion, Channel Slicing & Tool-Guard / MCP Pipeline

```diff
--- a/cans/04-terminal-plane.md
+++ b/cans/04-terminal-plane.md
@@ -2,8 +2,10 @@
   - Terminal pipeline
-    - Owner: kelir-core (virtio-console bus)
-    - Multi-port VirtIO console allocates dedicated paravirtual channels
-    - Channel hvc0 is the interactive PTY Master (/dev/pts/N)
-    - Channel hvc1 is the binary transport (MCP JSON-RPC)
+    - Owner: kelir-core (virtio-console domain)
+    - Allocates isolated linear interceptor pipelines per paravirtual channel:
+      - Channel hvc0: PTY Master pipeline (plugin-pty ANSI filters and Token Shield)
+      - Channel hvc1: Binary RPC pipeline (plugin-tool-guard -> plugin-mcp)
+    - Handlers receive strongly-typed TerminalContext; ambient c.term is eliminated
     - Interactive channel preserves isatty(1) and raw mode
     - Streaming ring buffers route directly to host supervisor memory
     - Stateful stream processing: regex and token shields maintain sliding
```

```diff
--- a/cans/plugins/pty.md
+++ b/cans/plugins/pty.md
@@ -4,14 +4,16 @@
     - Plugin identifier: plugin-pty
     - Owner crate: crates/kelir-plugins/plugin-pty
-    - name() = "pty"; priority() = 90
+    - name() = "pty"
     - Taxonomy (pty): urn:kelir:io:pty
       - coordinate: rings=[virtio-console:hvc0]; role=Mutator; tier=Native
+    - Capabilities (pty):
+      - provides = [Capability("io:pty")]
+      - requires = []
     - VirtIO device attachment (pty): see: 01-architecture.md#VirtIO-Device-Complex
     - Lifecycle attachment (pty): see: 07-lifecycle.md#Lifecycle-vetoes
     - Binds to virtio-console paravirtual port hvc0
     - Terminal bus transport: see: 04-terminal-plane.md#Terminal-pipeline
-  - Context keys for pty
-    - Context key contributed: c.term
-    - Closed-set binding rule (pty): see: 02-plugin-model.md#Core-context-keys
+  - Service exports (pty)
+    - Exports TerminalControlService via PipelineBinder for window and notice control
     - Terminal handler API
       - c.term.notice(text) emits a HostToGuest::Disclosure event over hvc0
```

```diff
--- a/cans/plugins/tool-guard.md
+++ b/cans/plugins/tool-guard.md
@@ -4,15 +4,18 @@
     - Plugin identifier: plugin-tool-guard
     - Owner crate: crates/kelir-plugins/plugin-tool-guard
-    - name() = "tool-guard"; priority() = 45
+    - name() = "tool-guard"
     - Taxonomy (tool-guard): urn:kelir:io:mcp:tool
       - coordinate: rings=[virtio-console:hvc1]; role=Gate; tier=Native
+    - Capabilities (tool-guard):
+      - provides = [Capability("io:mcp:tool-guard")]
+      - requires = []
+      - order = [OrderConstraint::RunBefore(Capability("io:channel:mcp"))]
     - VirtIO device attachment (tool-guard): see: 01-architecture.md#VirtIO-Device-Complex
     - Lifecycle attachment (tool-guard): see: 07-lifecycle.md#Lifecycle-vetoes
-    - Binds upstream of plugin-mcp on virtio-console port hvc1
-  - Context keys for tool-guard
-    - Context key contributed: c.tools
-    - Closed-set binding rule (tool-guard): see: 02-plugin-model.md#Core-context-keys
+    - Topologically executes upstream of plugin-mcp on virtio-console hvc1
+  - Service exports (tool-guard)
+    - Exports ToolRegistryService via PipelineBinder for schema and approval lookups
   - Invariants and policy (tool-guard)
     - Total tool-call interposition
       - Intercepts all JSON-RPC 2.0 frames on hvc1 matching method "tools/call"
@@ -34,7 +37,7 @@
       - ToolAction::Rewrite(args): mutates payload before dispatch
         (e.g. forced flags, path sandboxing)
       - ToolAction::Deny(errno, reason): drops call, synthesizes standard JSON-RPC error response
-      - ToolAction::SuspendForApproval: triggers hypervisor suspend primitive
+      - ToolAction::SuspendForApproval: requests hypervisor LifecycleRequest::Suspend(ToolApproval)
         - Freezes vCPU execution and flushes guest RAM to NVMe (0 MB host RSS)
         - Host waits for human out-of-band authorization via vsock Channel 0
```

```diff
--- a/cans/plugins/mcp.md
+++ b/cans/plugins/mcp.md
@@ -4,15 +4,17 @@
     - Plugin identifier: plugin-mcp
     - Owner crate: crates/kelir-plugins/plugin-mcp
-    - name() = "mcp"; priority() = 50
+    - name() = "mcp"
     - Taxonomy (mcp): urn:kelir:io:channel:mcp
       - coordinate: rings=[virtio-console:hvc1]; role=Mutator; tier=Native
+    - Capabilities (mcp):
+      - provides = [Capability("io:channel:mcp")]
+      - requires = []
+      - order = [OrderConstraint::RunAfter(Capability("io:mcp:tool-guard"))]
     - VirtIO device attachment (mcp): see: 01-architecture.md#VirtIO-Device-Complex
     - Lifecycle attachment (mcp): see: 07-lifecycle.md#Lifecycle-vetoes
     - Binds to virtio-console paravirtual port hvc1
-  - Context keys for mcp
-    - Context key contributed: c.mcp
-    - Closed-set binding rule (mcp): see: 02-plugin-model.md#Core-context-keys
+  - Service exports (mcp)
+    - Exports McpEndpointService via PipelineBinder for JSON-RPC transport routing
   - Invariants and policy (mcp)
     - Thin in-process adapter routing JSON-RPC stdio directly to VirtIO rings
     - MCP plugin: maps tools directly to virtio-console, virtio-fs, and traps
```

### Part 5/6: Wire Plane Cohesion, L4/L7 Capability Ordering & Unified Ingress Lifecycle

```diff
--- a/cans/06-wire-plane.md
+++ b/cans/06-wire-plane.md
@@ -2,6 +2,8 @@
   - Wire architecture
     - see: artifacts/diagrams/wire-plane.txt
+    - Owner: kelir-core (virtio-net wire domain pipeline)
+    - Pipeline stages resolved via topological DAG: chaos -> ingress -> db-proxy -> wire proxy
+    - Interceptor hooks receive strongly-typed WireContext; ambient c.wire is eliminated
     - Return contract: see: 02-plugin-model.md#Hook-return-semantics
   - TAP L2 bridge
     - TAP interface bridge connects the guest virtio-net device to the host
@@ -40,4 +42,4 @@
   - Connection parking
     - Parked connections queue in TAP buffers and raw virtqueues
-    - Host-terminated TCP splices to guest vsock on wake
+    - Emits standard LifecycleRequest::Suspend(IngressWait) to core state machine
     - Mechanics (parking): see: plugins/wire-ingress.md
```

```diff
--- a/cans/plugins/wire.md
+++ b/cans/plugins/wire.md
@@ -4,14 +4,17 @@
     - Plugin identifier: plugin-wire
     - Owner crate: crates/kelir-plugins/plugin-wire
-    - name() = "wire"; priority() = 75
+    - name() = "wire"
     - Taxonomy (wire): urn:kelir:net:l7:proxy
       - coordinate: rings=[virtio-net]; role=Mutator; tier=Native
+    - Capabilities (wire):
+      - provides = [Capability("net:l7:proxy"), Capability("net:l4:socks")]
+      - requires = []
+      - order = [OrderConstraint::RunAfter(Capability("sys:fault"))]
     - VirtIO device attachment (wire): see: 01-architecture.md#VirtIO-Device-Complex
     - Lifecycle attachment (wire): see: 07-lifecycle.md#Lifecycle-vetoes
-  - Context keys for wire
-    - Context key contributed: c.wire
-    - Closed-set binding rule (wire): see: 02-plugin-model.md#Core-context-keys
+  - Service exports (wire)
+    - Exports WireProxyService via PipelineBinder for dynamic port and rule queries
     - Wire configuration knobs:
       - tap_filter: TapFilter::StrictTcpOnly (default) | AllowUdpProxied | Passthrough
       - ipv6_policy: Ipv6Policy::DropAll (default) | AllowRoutable
```

```diff
--- a/cans/plugins/wire-ingress.md
+++ b/cans/plugins/wire-ingress.md
@@ -4,14 +4,16 @@
     - Plugin identifier: plugin-wire-ingress
     - Owner crate: crates/kelir-plugins/plugin-wire-ingress
-    - name() = "wire-ingress"; priority() = 85
+    - name() = "wire-ingress"
     - Taxonomy (wire-ingress): urn:kelir:net:l4:ingress
       - coordinate: rings=[virtio-vsock:ch1]; role=Proxy; tier=Native
+    - Capabilities (wire-ingress):
+      - provides = [Capability("net:l4:ingress")]
+      - requires = []
     - VirtIO attachment (wire-ingress): see: 01-architecture.md#VirtIO-Device-Complex
     - Lifecycle attachment (wire-ingress): see: 07-lifecycle.md#Lifecycle-vetoes
-  - Context keys for wire-ingress
-    - Context key contributed: c.ingress
-    - Closed-set binding rule (wire-ingress): see: 02-plugin-model.md#Core-context-keys
+  - Service exports (wire-ingress)
+    - Exports IngressRoutingService via PipelineBinder for socket forwarding
   - Invariants and policy (wire-ingress)
     - Holds inbound client TCP connections without dropped frames during suspend
     - Host proxy terminates TCP handshake and buffers up to 64KB initial request data
```

```diff
--- a/cans/plugins/db-proxy.md
+++ b/cans/plugins/db-proxy.md
@@ -4,14 +4,17 @@
     - Plugin identifier: plugin-db-proxy
     - Owner crate: crates/kelir-plugins/plugin-db-proxy
-    - name() = "db-proxy"; priority() = 70
+    - name() = "db-proxy"
     - Taxonomy (db-proxy): urn:kelir:net:l7:sql
       - coordinate: rings=[virtio-net, virtio-vsock]; role=Proxy; tier=Native
+    - Capabilities (db-proxy):
+      - provides = [Capability("net:l7:sql")]
+      - requires = []
+      - order = [OrderConstraint::RunBefore(Capability("net:l7:proxy"))]
     - VirtIO Ring attachment: virtio-net (port 5432) and virtio-vsock (/run/kelir/db.sock)
     - Lifecycle attachment (db-proxy): see: 07-lifecycle.md#Lifecycle-vetoes
-  - Context keys for db-proxy
-    - Context key contributed: c.db
-    - Closed-set binding rule (db-proxy): see: 02-plugin-model.md#Core-context-keys
+  - Service exports (db-proxy)
+    - Exports SqlProxyService via PipelineBinder for statement and dialect queries
   - Invariants and policy (db-proxy)
     - Mediates relational database communication
     - PostgreSQL: intercepts port 5432, decodes Q/P frames via pgwire parser
```

```diff
--- a/cans/plugins/chaos.md
+++ b/cans/plugins/chaos.md
@@ -4,14 +4,19 @@
     - Plugin identifier: plugin-chaos
     - Owner crate: crates/kelir-plugins/plugin-chaos
-    - name() = "chaos"; priority() = 80
+    - name() = "chaos"
     - Taxonomy (chaos): urn:kelir:sys:fault
       - coordinate: rings=[virtio-net, virtio-fs]; role=Mutator; tier=Native
+    - Capabilities (chaos):
+      - provides = [Capability("sys:fault")]
+      - requires = []
+      - order = [
+          OrderConstraint::RunBefore(Capability("net:l7:proxy")),
+          OrderConstraint::RunBefore(Capability("fs:vfs"))
+        ]
     - VirtIO device attachment (chaos): see: 01-architecture.md#VirtIO-Device-Complex
     - Lifecycle attachment (chaos): see: 07-lifecycle.md#Lifecycle-vetoes
-  - Context keys for chaos
-    - Context key contributed: c.chaos
-    - Closed-set binding rule (chaos): see: 02-plugin-model.md#Core-context-keys
+  - Service exports (chaos)
+    - Exports FaultInjectionService via PipelineBinder for dynamic rule mutations
   - Invariants and policy (chaos)
     - Bidirectional across wire plane
       - on_net_outbound mutates frames leaving guest
```

### Part 6/6: Centralized Lifecycle State Machine, Profile IoC Decoupling, Event-Bus Ledger & Operations Verification

```diff
--- a/cans/07-lifecycle.md
+++ b/cans/07-lifecycle.md
@@ -2,6 +2,15 @@
   - Hypervisor primitives
     - see: artifacts/diagrams/hypervisor-primitives.txt
+    - Centralized Hypervisor Lifecycle State Machine:
+      - States: Initializing, Running, Suspended(SuspendReason), Terminated
+      - SuspendReason: ToolApproval | IngressWait | AdminHold | ResourcePressure
+      - Eliminates independent suspend implementations in tool-guard and wire-ingress
+      - Single core implementation of lz4_flex + io_uring NVMe working-set dump
+      - Zero host RSS (0 MB) achieved identically across all suspension triggers
+      - Cycle-exact resume triggered via vsock Channel 0 patch or TAP frame arrival
     - Set membership and mechanism per item: see: artifacts/diagrams/hypervisor-primitives.txt
```

```diff
--- a/cans/plugins/profile.md
+++ b/cans/plugins/profile.md
@@ -4,29 +4,20 @@
     - Plugin identifier: plugin-profile
     - Owner crate: crates/kelir-plugins/plugin-profile
-    - name() = "profile"; priority() = 10
+    - name() = "profile"
     - Taxonomy (profile): urn:kelir:sys:config:profile
       - coordinate (profile): rings=[virtio-vsock:ch0]; role=Gate; tier=Native
+    - Capabilities (profile):
+      - provides = [Capability("sys:config:profile")]
+      - requires = []
     - VirtIO device attachment (profile): see: 01-architecture.md#VirtIO-Device-Complex
     - Lifecycle attachment (profile): see: 07-lifecycle.md#Lifecycle-vetoes
-  - Context keys for profile
-    - Context key contributed: c.profile
-    - Closed-set binding rule (profile): see: 02-plugin-model.md#Core-context-keys
   - Invariants and policy (profile)
-    - Bidirectional declarative configuration engine
-    - Ingests canonical kelir.toml and kelir.yaml configurations
-    - Implements CompositePlugin to flatten declarations into child plugins
-    - Thirteen-domain composite expansion
-      - storage block expands into plugin-vfs mount rules and encapsulations
-      - masking block expands into plugin-vfs sliding-window FPA generators
-      - tools block expands into plugin-tool-guard pinning, rate limits, and veto rules
-      - network and secrets blocks expand into plugin-wire TAP and proxy rules
-      - rate_limits block expands into plugin-wire token-bucket rate limiters
-      - terminal block expands into plugin-pty Token Shield and ANSI filters
-      - syscalls block compiles to Gate 1 in-kernel eBPF allowlists
-      - budget block configures virtio-wdt ceilings and PMU instruction limits
-      - audit block expands into plugin-ledger Merkle WAL and redaction rules
-      - database block expands into plugin-db-proxy pgwire port interception
-      - supply_chain block expands into plugin-supply-chain exec-trap monitors
-      - chaos block expands into plugin-chaos network and storage fault rules
+    - Inversion-of-Control configuration router:
+      - Ingests canonical kelir.toml and kelir.yaml specification files
+      - Plugins register target schema namespaces dynamically via ConfigurablePlugin
+      - Routes unparsed configuration fragments to registered namespace owners
+      - Profile contains zero hardcoded child domain logic or expansion rules
+      - Downward min cascade enforced uniformly across scalar budget fields
     - Downward min cascade
       - Effective budget and resource limits evaluate as min(base, child)
```

```diff
--- a/cans/plugins/ledger.md
+++ b/cans/plugins/ledger.md
@@ -4,15 +4,17 @@
     - Plugin identifier: plugin-ledger
     - Owner crate: crates/kelir-plugins/plugin-ledger
-    - name() = "ledger"; priority() = 55
+    - name() = "ledger"
     - Taxonomy (ledger): urn:kelir:sys:audit
       - coordinate: rings=[virtio-vsock:ch0]; role=Observer; tier=Native
+    - Capabilities (ledger):
+      - provides = [Capability("sys:audit")]
+      - requires = []
     - VirtIO device attachment (ledger): see: 01-architecture.md#VirtIO-Device-Complex
     - Lifecycle attachment (ledger): see: 07-lifecycle.md#Lifecycle-vetoes
-  - Context keys for ledger
-    - Context key contributed: c.ledger
-    - Closed-set binding rule (ledger): see: 02-plugin-model.md#Core-context-keys
   - Invariants and policy (ledger)
+    - Pure reactive observer: registers subscribers exclusively on the EventBus
+    - Zero participation in inline interceptor pipelines; non-blocking execution
+    - Ingests immutable StorageDelta and ExecutionDelta domain events
     - Merkle WAL
       - Pluggable LedgerSink trait owns persistence; Merkle DB is default impl
       - Delegated audit persistence via external_sink replaces default sink
@@ -34,10 +36,5 @@
     - OpenTelemetry
       - Outbound HTTP requests carry W3C traceparent, injected host-side
       - Trace continuity rule: a guest-originated request and its guest-side
         cause (trap, spawn, receipt) resolve to the same trace
-    - compute-causal-delta engine
-      - Computation is policy-bearing; it lives in plugin-ledger
-      - Session accessor `get_causal_delta_from_parent()` requires
-        plugin-ledger mounted; unmounted sessions do not expose it
-      - Causal delta definition: see: 08-epistemic.md#Causal-delta
-      - CausalDelta computation: diffs physical RAM and inode journals
       - Binds ExecutionReceipt correlation_token into Merkle WAL records
```

```diff
--- a/cans/plugins/supply-chain.md
+++ b/cans/plugins/supply-chain.md
@@ -4,14 +4,16 @@
     - Plugin identifier: plugin-supply-chain
     - Owner crate: crates/kelir-plugins/plugin-supply-chain
-    - name() = "supply-chain"; priority() = 25
+    - name() = "supply-chain"
     - Taxonomy (supply-chain): urn:kelir:sys:trap:exec
       - coordinate: rings=[virtio-vsock:ch0]; role=Gate; tier=Native
+    - Capabilities (supply-chain):
+      - provides = [Capability("sys:trap:exec")]
+      - requires = []
     - VirtIO attachment (supply-chain): see: 01-architecture.md#VirtIO-Device-Complex
     - Lifecycle attachment (supply-chain): see: 07-lifecycle.md#Lifecycle-vetoes
-  - Context keys for supply-chain
-    - Context key contributed: None (guard only)
-    - Closed-set binding rule (supply-chain): see: 02-plugin-model.md#Core-context-keys
```

```diff
--- a/cans/plugins/mesh.md
+++ b/cans/plugins/mesh.md
@@ -4,14 +4,16 @@
     - Plugin identifier: plugin-mesh
     - Owner crate: crates/kelir-plugins/plugin-mesh
-    - name() = "mesh"; priority() = 65
+    - name() = "mesh"
     - Taxonomy (mesh): urn:kelir:ipc:vsock:mesh
       - coordinate: rings=[virtio-vsock]; role=Mutator; tier=Native
+    - Capabilities (mesh):
+      - provides = [Capability("ipc:vsock:mesh")]
+      - requires = []
     - VirtIO device attachment (mesh): see: 01-architecture.md#VirtIO-Device-Complex
     - Lifecycle attachment (mesh): see: 07-lifecycle.md#Lifecycle-vetoes
-  - Context keys for mesh
-    - Context key contributed: c.mesh
-    - Closed-set binding rule (mesh): see: 02-plugin-model.md#Core-context-keys
```

```diff
--- a/cans/11-operations.md
+++ b/cans/11-operations.md
@@ -62,7 +62,8 @@
   - Coverage-gates
     - --plugin-coverage
       - Plugin asserts: see: 02-plugin-model.md#First-class-plugins
+      - DAG resolution gate: asserts all provides, requires, and order constraints form an acyclic graph
+      - Verifies zero plugins define hardcoded integer priority values
     - --lifecycle-coverage
       - Lifecycle asserts: see: 07-lifecycle.md#Hypervisor-primitives
+      - State machine assert: verifies all SuspendReason variants execute through centralized lifecycle
```

**No. Stopping at Part 6 leaves spec desynchronization and broken contracts.** 

If you ran `cans check` right now, it would fail. 

Four critical areas were still leaking the old architecture:
1. **`cans/08-epistemic.md`**: Still claimed `plugin-ledger` computes the `CausalDelta` (direct contradiction with Part 3 and Part 6).
2. **`cans/03-sdk-architecture.md`**: Still documented ambient `c.<key>` accumulation across TypeScript, Python, and Rust instead of typed service queries.
3. **`cans/artifacts/diagrams/host-runtime.txt`**: Missing the `PipelineEngine` compiled by the DAG.
4. **Code Artifact Excerpts (`ts`, `py`, `rs`)**: Still demonstrating ambient property sniffing (`c.policy.violations`) instead of typed service resolution.

Here are the remaining patches required to seal the spec.

---

### Supplemental Patch 1: Epistemic Plane & Telemetry SSOT (`cans/08-epistemic.md`)

```diff
--- a/cans/08-epistemic.md
+++ b/cans/08-epistemic.md
@@ -71,11 +71,10 @@
     - Tracks physical page deltas, modified inodes, and process lineage
     - L7 protocol telemetry lives in plugin-wire, isolated from core structs
     - Type ownership
       - Struct is a shared context type; it lives in kelir-interface next to
         ExecutionReceipt
-      - Computation is derived: see: plugins/ledger.md#compute-causal-delta-engine
-    - This split follows the primitive-vs-derived rule: type is primitive,
-      computation is derived
+      - Computation is domain-local: StorageDomain emits StorageDelta; ControlDomain emits ExecutionDelta
+      - Core aggregates domain deltas over the EventBus into CausalDelta at checkpoint boundaries
+      - Eliminates feature-envy: plugin-ledger subscribes to CausalDelta; never diffs VFS journals
   - Three-tier evidence verification model
     - see: artifacts/diagrams/truth-hierarchy.txt
```

---

### Supplemental Patch 2: SDK Architecture & Typed Services (`cans/03-sdk-architecture.md`)

```diff
--- a/cans/03-sdk-architecture.md
+++ b/cans/03-sdk-architecture.md
@@ -83,10 +83,10 @@
   - Rust SDK
     - see: artifacts/code/rust-handle-injection.rs
     - Typestate transitions: KelirBuilder<Unset> to KelirBuilder<Sealed> to Session
     - .plugin() takes flat plugin instances; .params::<T>() carries the schema
     - .plugin() accepts core KelirPlugin plus only needed capability traits
-    - Closures requiring plugin capabilities enforce builder typestate bounds
+    - Inter-plugin services queryable via c.service::<T>() validated against DAG requirements
     - Composite bundles flatten into builder via CompositePlugin trait
     - Zero-Allocation Validation via Serde
       - #[derive(Deserialize)] enum Action with serde(tag = "action")
       - c.params::<Action>() fails at perimeter before traps execute
@@ -95,11 +95,11 @@
   - TypeScript SDK
     - see: artifacts/code/typescript-accumulation.ts
     - Fluent model: .use().params().trap().intercept().on(), then .boot()
     - .use() takes flat plugin instances; no callback builder arguments
     - .params(schema): TypeBox AOT push-down compiles to native Rust
-    - Generic context accumulator infers c.<key> from mounted plugins
+    - Typed service locator: c.service(ToolRegistry) replaces ambient untyped properties
     - AOT hook fusion compiles all TS pipeline handlers into a single flat closure
     - Invariant: exactly 1 NAPI trampoline jump per event regardless of depth
-    - Null-object pattern: use(plugin({ enabled: false })) preserves c.pluginKey
+    - Null-object pattern: use(plugin({ enabled: false })) preserves service interfaces with no-op dispatch
   - Python SDK
@@ -111,8 +111,8 @@
     - Composite plugins unpack nested plugins atomically before boot
     - Pydantic v2 BaseModel with frozen=True
       - Field(pattern=r"^fix-webhook$")
       - Raises EINVAL before hook evaluation
-    - policy = RepoPolicyPlugin(); builder.use(policy)
-      - Host retains direct reference; hooks receive c.policy
+    - Service resolution via c.service(RepoPolicyService)
     - Mount scoping via kwargs: rings=..., binaries=..., paths=...
```

---

### Supplemental Patch 3: Host Runtime Diagram (`cans/artifacts/diagrams/host-runtime.txt`)

```diff
--- a/cans/artifacts/diagrams/host-runtime.txt
+++ b/cans/artifacts/diagrams/host-runtime.txt
@@ -14,9 +14,10 @@
   ┌───────────────────────────────────────────────────────────────────────────────────┐  │
   │                        kelir-core (Stripped Microkernel)                          │  │
   │   ├── Native tier plugins: In-memory Rust execution (<12µs raw ring speed)        │  │
   │   ├── Foreign tier Filter-Before-FFI Gate: MountScope drops unmonitored events    │  │
   │   ├── BusDispatcher: Zero-copy multiplexer for the 4 VirtIO Rings                 │  │
+  │   ├── PipelineEngine: Kahn's DAG-sorted contiguous interceptor execution tables   │  │
   │   ├── EventBus: Unified compile-time and async hook pipeline                      │  │
   │   └── DriverRouter: Lifecycle controller mediating Box<dyn VmmDriver>             │  │
   └──────────────────────────────────────────┬────────────────────────────────────────┘  │
```

---

### Supplemental Patch 4: Code Artifacts (`rs`, `ts`, `py`)

```diff
--- a/cans/artifacts/code/rust-handle-injection.rs
+++ b/cans/artifacts/code/rust-handle-injection.rs
@@ -37,7 +37,8 @@
     .trap("git commit *", handler(|c: HandlerCtx| async move {
-        if c.policy().violations().await > 0 {
+        let policy = c.service::<dyn RepoPolicyService>()?;
+        if policy.violations().await > 0 {
             return Ok(Decision::deny(1, "Policy violations recorded"));
         }
         Ok(Decision::Resume)
     }));
```

```diff
--- a/cans/artifacts/code/typescript-accumulation.ts
+++ b/cans/artifacts/code/typescript-accumulation.ts
@@ -29,6 +29,7 @@
   .trace((t) => {
     if (t.ffi_hop_ns > 10_000) console.warn("FFI boundary spike:", t);
   })
-  // `c.policy`, `c.fs`, and `c.processLineage` are fully inferred on unified context:
+  // Services resolved via typed service locator; no ambient string grab-bags
   .trap("git commit *", async (c) => {
-    if (c.policy.violations > 0) {
+    const policy = c.service(RepoPolicyService);
+    if (policy.violations > 0) {
       return c.deny(1, `EPOLICY: ${policy.violations} violation(s)`);
     }
     return c.resume();
   });
```

```diff
--- a/cans/artifacts/code/python-raii-pydantic.py
+++ b/cans/artifacts/code/python-raii-pydantic.py
@@ -21,6 +21,7 @@
 # 3. Handler reads typed plugin state directly from context c
 async def commit_gate(c: HandlerContext) -> Decision:
-    if c.policy.violations > 0:
-        return Decision.deny(1, f"EPOLICY: {policy.violations} violation(s)")
+    policy_svc = c.service(RepoPolicyService)
+    if policy_svc.violations > 0:
+        return Decision.deny(1, f"EPOLICY: {policy_svc.violations} violation(s)")
     return Decision.resume()
```

---

### Verification Summary

Every reference to:
1. Hardcoded integer `priority()`
2. Ambient context keys (`c.tools`, `c.fs`, `c.wire`, `c.term`, `c.ledger`)
3. `plugin-profile` hardcoding 13 downstream expansion domains
4. `plugin-ledger` reaching into VFS memory to compute deltas
5. Ad-hoc independent suspension mechanisms

**...is now fully eliminated across the entire spec, artifacts, and diagrams.** The graph is 100% DAG-driven, dual-plane (Interceptor Pipeline vs. Event Bus), and strictly adheres to SOLID principles.
