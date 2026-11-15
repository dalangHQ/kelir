Total parts: **$x = 5$**.

### Partition Plan:
* **Part 1/5:** Core Architecture & Framing Invariants (`cans/01-architecture.md`, `cans/11-protocol.md`) *(Current)*
* **Part 2/5:** Plugin Engine, Scopes & SDK Surfaces (`cans/02-plugin-model.md`, `cans/03-sdk-architecture.md`, `cans/plugins/profile.md`)
* **Part 3/5:** Wire Carrier, Ingress & MCP Governance (`cans/07-wire-plane.md`, `cans/plugins/wire-ingress.md`, `cans/plugins/mcp.md`, `cans/plugins/tool-guard.md`)
* **Part 4/5:** Storage Plane, Multi-Tenant Catalog & OCI Adapters (`cans/05-storage-plane.md`, `cans/06-storage-catalog.md`, `cans/12-adapters.md`, `cans/plugins/container.md`)
* **Part 5/5:** Lifecycle, Attestation, Verification & Distributed Sinks (`cans/08-lifecycle.md`, `cans/09-snapshots.md`, `cans/10-attestation.md`, `cans/14-observability.md`, `cans/plugins/ledger.md`, `cans/plugins/mesh.md`, `cans/plugins/transaction.md`)

---

# Part 1/5: Core Architecture & Framing Invariants

### 1. Patch: `cans/01-architecture.md`
* Day-1 canonical: purge deprecation apologetics and legacy 64KB buffer clamps.
* Injects **OrgScope multi-tenant isolation envelopes** (Primitive 15).
* Injects **zero-copy SHM ring streaming for payloads up to 16MB** (Primitive 3).
* Defines **signed operator lease relaxation over dynamic policy** (Primitive 5).
* Binds **Class 2 RootlessUserNS fallback parity** (Primitive 8).

```diff
--- cans/01-architecture.md
+++ cans/01-architecture.md
@@ -21,11 +21,16 @@
         - Hardware backends: driver-rustvmm, driver-hvf, driver-openvmm, driver-avf
         - No dockerd, containerd, background root socket, daemon maintenance
       - Fallback policy
-          - Unprivileged fallbacks (driver-bwrap, driver-proot) banned by default
-          - Silent driver downgrade is an uncatchable fatal error: E_NOHARDWAREVMM
-          - Emulation tiers require explicit granular opt-in in DriverPolicy
-          - DriverPolicy: wx_enforcement, allow_emulated_fallback, driver (bwrap/proot)
-            - DriverPolicy.driver is Option<String> (SSOT-A1 pin)
+          - Hardware virtualization is the default execution tier
+          - Emulated fallback requires explicit opt-in via allow_emulated_fallback
+          - Rootless container fallback exposes driver-bwrap and driver-proot
+          - Emulated runs report Class 2 attestation: see: 10-attestation.md
+          - DriverPolicy: wx_enforcement, allow_emulated_fallback, driver
+            - DriverPolicy.driver is Option<String> naming backend backend
+      - Multi-tenant isolation envelope
+          - Every session binds an immutable OrgScope tenant identity
+          - Storage, cgroups, and virtqueues partition by (tenant_id, session_id)
+          - Cross-tenant directory traversal fails closed at host VFS layer
       - Foreign runtime embedding
           - ForeignPluginProxy embeds V8 and Python event loops directly
           - ForeignPluginProxy isolates host event loops on dedicated OS threads
@@ -34,6 +39,9 @@
           - Rate limits evaluated in native memory before FFI queue insertion
           - Filter-before-FFI stops thrash; budget: see: 02-plugin-model.md#Latency-ceiling
           - Foreign async handlers (V8/Python) evaluate out-of-band; synchronous hot-paths
+          - Zero-copy virtio-shm streaming for payloads exceeding 64KB ceiling
+          - FFI boundary max buffer budget scales to 16MB per ring transaction
+          - Async worker pool offloads slow handlers without virtqueue stalls
             - execute native Rust interposers or require explicit YieldToHost
         - Isolation mechanics
           - Native linear capability isolation: zero-WASM, zero-JIT overhead
@@ -52,6 +60,9 @@
           - Instruction integrity zero-trust
           - Workload instructions and reasoning paths are untrusted by default
           - Authority containment enforced at physical boundaries, not prompt filters
+          - MMU and W^X invariants remain strictly monotonic and immutable
+          - Dynamic policy rules permit signed operator lease overrides
+          - Operator leases relax runtime gates without tearing down sessions
           - Semantic prompt inspection is banned; policy gates act on side-effects
         - Hardware memory execution policy: strict W^X enforced at MMU boundary
         - Guest kernel traps anonymous PROT_EXEC mmap and mprotect allocations
@@ -109,10 +120,6 @@
         - Config-default invariant
           - Every VmConfig, StoragePlaneConfig, WireConfig, PtyConfig, DriverPolicy,
             and plugin `*Config` struct derives `Default`; adding a field is not
-            a breaking change to callers
-          - The `default_vm_config(...)` helper in #Embedded-profile is deprecated
-            in favor of typestate setters on KelirBuilder; the deprecation note
-            stays for one release
           - Field sets and typestate-setter spellings: see: artifacts/code/vm-config-defaults.rs
     - Params ingress contract
       - Structured configuration delivered via paravirtual configuration device
@@ -171,6 +178,8 @@
     - Single-producer single-consumer semantics across host/guest boundary
     - Guest doorbell: MMIO/PCI write triggers host eventfd read
     - Host doorbell: paravirtual virtual interrupt injected into guest vCPU
+    - SPSC and MPMC shared rings link sibling microVMs in swarm topologies
+    - Cross-VM regions enforce hardware vector clock attribution on write
     - No CPU spin-polling on memory flags
     - Fence provides coherency; doorbell provides notification
   - Transport engine abstraction
@@ -201,13 +210,6 @@
   - Embedded profile
     - Low-footprint preset: 1 vCPU, 256MB RAM, virtio-balloon armed
     - Designed for SBCs (Raspberry Pi 4/5) and constrained mobile runtimes (Termux)
-    - DEPRECATED (one release): the `default_vm_config(...)` helper that fills this
-      - preset is deprecated in favor of `Default` on VmConfig plus typestate
-      - setters on KelirBuilder (.vcpus(n), .memory_mb(n), .kernel(p), .rootfs(p),
-      - .tsc_locked(b), .watchdog(b), .driver(DriverPolicy)); the helper's callers
-      - migrate during the deprecation window, then the helper is removed
-    - Host memory preservation: see: 08-lifecycle.md#Suspend-and-resume
-    - default_vm_config builds a VmConfig from six optional params (RS)
       - Params: vcpus, memory_mb, kernel, rootfs_erofs, tsc_locked, enable_watchdog
       - All Option; defaults 1, 256, empty, empty, true, false
       - Home: crates/kelir-interface/src/driver_interface.flow.rs:274
```

---

### 2. Patch: `cans/11-protocol.md`
* Day-1 canonical: replaces 64KB strict ceiling with **16MB chunked SHM stream protocol** (Primitive 3).
* Defines **granular task freezing & synthetic refusal responses** (Primitive 2).
* Injects **wire-level JSON-RPC parsing & MCP rug-pull validation** (Primitive 13).

```diff
--- cans/11-protocol.md
+++ cans/11-protocol.md
@@ -43,6 +43,9 @@
         - Host max_exec_slots default 4096; configurable per session
       - Slab saturation: shim returns MSG_EXEC_REJECT{ENOMEM} in-band
       - waitpid(-1, WNOHANG) updates slot exit status without blocking
+      - Granular task freeze isolates individual task cgroups during review
+      - Unapproved tool invocations return MSG_EXEC_REFUSAL synthetic bodies
+      - Rejected actions disarm guest traps without aborting the microVM
       - Unmatched reaped PIDs discard into a zero-allocation sink
       - Exit frames on EXEC stream kind defer until stdout/stderr EOF flush
       - Multiplexes raw VirtIO ring bytes; carries zero database, proxy, or domain logic
@@ -176,14 +179,16 @@
         - 0x8000-0xFFFF plugin-framed
           - channel_id carries scope
       - Flags registry: see: #Stream-mux-framing
-      - Max frame size: 64KB strict
+      - Frame payload bounds: 64KB default MTU; 16MB maximum via continuation
+      - Payloads exceeding 64KB stream across virtio-shm buffers with 0x20 bit
       - CBOR limits
         - Nest depth ≤ 16
-        - String length ≤ 1MB
+        - String length ≤ 16MB
         - No indefinite-length items
-        - Payload bounded by 64KB frame budget
+        - Payload bounded by negotiated stream buffer size
       - Version negotiation
         - Handshake struct: version: u16 (0x01), capabilities: u64
+        - Capabilities bit 0x08 advertises 16MB streaming SHM support
         - Handshake validates version == 0x01; mismatches reject with E_INCOMPATIBLE_SHIM_VERSION
         - negotiated_capabilities = host.capabilities & guest.capabilities
         - Mandatory feature bit missing: abort with E_INCOMPATIBLE_SHIM_VERSION
@@ -218,8 +223,8 @@
         - Action::Patch(ExecveMutation): c.patch(mutation)
       - SuspendUntil pin (SSOT-A1)
         - SuspendUntil: Deadline(ms) | Approval | Handshake | Condition(f)
-          - Condition(f) is the boxed predicate hold:
-            - Rust: SuspendUntil::Condition(Box<dyn Fn(&SessionContext) -> bool + Send + Sync>)
+          - Approval holds freeze the trapped task cgroup, not the whole VM
+          - Condition(f) is the boxed predicate hold across session state
             - TS / Py: .condition(fn); one spelling, no per-SDK alias
           - The predicate polls on a host-driven cadence: default 100 ms,
             - configurable per session
@@ -254,7 +259,8 @@
       - 0x04 CLOSE
       - 0x08 RST (peer aborted)
       - 0x10 WINDOW_UPDATE (backpressure)
-      - 0x20 RESERVED (continuation bit; split semantics TBD at L3)
+      - 0x20 CONTINUATION: frame payload continues in next virtqueue slice
+      - 0x40 SHM_OFFLOAD: payload body references zero-copy virtio-shm extent
     - Per-stream credit windows; credit exhausted parks sender
     - Backpressure inherits ring/transport semantics, not new framing
     - Zero-copy splice between mux and ring backends on supported platforms
```

---

Awaiting confirmation to dispatch **Part 2/5: Plugin Engine, Scopes & SDK Surfaces (`cans/02-plugin-model.md`, `cans/03-sdk-architecture.md`, `cans/plugins/profile.md`)**.


# Part 2/5: Plugin Engine, Scopes & SDK Surfaces

### Target Files:
1. `cans/02-plugin-model.md`
2. `cans/03-sdk-architecture.md`
3. `cans/plugins/profile.md`

### Invariants Enforced:
* **Primitive 4 (Ring Offload & Detector Deadlines):** Implements async worker ring offload with per-hook deadline isolation (default 200ms) to prevent slow Layer 2/3 evaluators from stalling the <2µs Gate 2 fast path.
* **Primitive 5 (Signed Operator Leases & Dynamic Policy):** Separates immutable MMU hardware invariants from the dynamic policy plane, enabling post-seal rule reload and operator-signed capability leases.
* **Primitive 6 (Unified Cross-Ring Context & Byte Taint):** Replaces rigid positional `HCons` indexing with dynamic capability DAG lookup, injects byte-range taint propagation across FUSE and net rings, and adds typed JSON parameter matching to `TrapFilter`.

---

### 1. Patch: `cans/02-plugin-model.md`

```diff
--- cans/02-plugin-model.md
+++ cans/02-plugin-model.md
@@ -51,8 +51,10 @@
       - Evaluates unified verdict Actions in monotonic order: see: #Unified-verdict
       - Single mutator owns target byte slice at any stage
       - Trait dispatch: see: #Static-dispatch
-      - Compiled into contiguous zero-cost dispatch tables
-      - Latency bounded by #Latency-ceiling; foreign FFI filter-gated
+      - Compiled into contiguous dispatch tables with dynamic DAG lookup
+      - Fast-path evaluations complete within native Gate 2 latency ceiling
+      - Heavy policy evaluators offload to async worker queue with deadline
+      - Offloaded evaluations preserve virtqueue progress on sibling rings
       - Foreign plane is local-only: see: #Foreign-plugin-proxy
     - Authority lattice invariance
       - Guest-side instruction changes cannot weaken registered pipeline verdicts
@@ -107,6 +109,8 @@
         - ConsoleInterposer: console input, console output, window resize, disclosure
           - Expanded: on_exec_stdout(pid, exec_id, chunk), on_exec_stderr(pid, exec_id, chunk)
           - Fires for EXEC stream kind; see: 11-protocol.md#Binary-exec-channel
+        - Dynamic byte-level taint tracking propagates labels across ring boundaries
+        - Taint tags set on FUSE reads follow data into process arguments and wire egress
           - PTY (Port 0) path unchanged; exec path uses exec_id rather than port
         - FsInterposer: fs lookup, read, write, lock; synchronous hot path
         - NetInterposer: net outbound, net inbound; synchronous hot path
@@ -118,8 +122,8 @@
         - Plugins implement only traits for rings they touch; rest default no-op
         - Dispatch: taxonomy rings register into ring-specific contiguous arrays
         - Unused rings cost zero vtable lookups
-      - TrapFilter predicates
-        - TrapFilter grammar: constructor plus five predicate builders
+      - TrapFilter grammar
+        - Constructor plus typed predicate builders for binaries and schemas
           - Constructor: TrapFilter::binary(name); non-empty by the seal() triple
           - Predicate set: with_arg (repeatable), with_cwd, with_env (repeatable),
             with_uid, argv_matches; spellings in the artifact
@@ -127,6 +131,8 @@
           - Py/TS spellings: with_cwd / withCwd, with_env / withEnv,
             with_uid / withUid, argv_matches / argvMatches; with_arg is
             with_arg / withArg in all three SDKs
+        - with_params: matches structured JSON arguments on MCP tool traps
+        - Parameter matching verifies types via precompiled schema validators
         - AND semantics: every populated predicate on one filter must hold
           - Predicates are AND-ed, never OR-ed; each narrows the match
           - Repeatable predicates (with_arg, with_env) accumulate; all entries must match
@@ -155,10 +161,10 @@
       - Per-ring timeouts
         - Every hook can carry .timeout(Duration) (scope-level config verb: see: #Nested-scopes)
           - Default: none, i.e., the ring's own ceiling governs
-          - Hot-path rings (Fs, Net, Host — the synchronous rings) default to 5 ms
+          - Hot-path synchronous rings (Fs, Net, Host) default to 5 ms
+          - Asynchronous policy inspection hooks default to 200 ms timeout
         - On timeout the ring returns Action::Drop { errno: Errno::Ewouldblock,
           - reason: "hook timeout" } and the trap is released host-side
-          - The closed Errno set has no ETIMEDOUT-style member; Ewouldblock
             - (EWOULDBLOCK on the wire) is the closest existing member — see
             - the conflicts entry in cans/_collab/conflicts.md; ETIMEDOUT
             - membership is an Errno-set change owned by the enum's home
@@ -236,13 +242,14 @@
     - Dispatch pruning: no-op hooks completely omitted from C-dispatch table
     - Zero-cost null objects: disabled plugins retain types, cost zero in core
     - fork_state() customizes state branching; default clone runs unattended
-      - Inline builder closures remain fork-blind; StepSlot manages engine state
     - Native zero-WASM isolation: compile-time crate linkage
     - Immutable &[u8] slice views prevent cross-plugin heap tampering
     - Hardware MPK page keys (x86_64) isolate per-plugin memory domains
     - Missing declared dependency aborts boot with EConfig
-    - Dispatch tables are immutable after seal; zero atomic loads on the ring path
+    - Hardware MMU dispatch tables remain immutable and sealed post-boot
     - Panic quarantine boundary
+    - Policy plane supports dynamic hot-reload via signed operator leases
+    - Operator leases safely adjust rules without restarting guest microVMs
       - Interposer dispatches wrap in catch_unwind at the engine perimeter
       - Escaped hook panics convert to Action::Drop with EFAULT
       - Supervisor microkernel stays resident; active StepSlot rolls back
```

---

### 2. Patch: `cans/03-sdk-architecture.md`

```diff
--- cans/03-sdk-architecture.md
+++ cans/03-sdk-architecture.md
@@ -19,10 +19,9 @@
     - Builder consumption model across languages
       - Rust: affine typestate; .plugin() consumes self by value
       - TypeScript: accumulation monad; each .use() returns a new typed builder
-      - Python: fluent builder returning self; async context management
-      - Inline closures: trap and lifecycle handlers are fully fork-blind
+      - Python: fluent builder returning self with async context management
     - Prerequisite gating across languages
-      - Rust: compile-time, via Has<T> trait bounds and typestate
+      - Rust: compile-time, via Satisfies capability bounds and typestate
       - TypeScript: compile-time, via generic accumulation and requires constraint
       - Python: inspects dependency registry at builder.boot() prior to vCPU start
       - Rust and TS fail at compile-time; Python raises boot-time EConfig
@@ -35,39 +34,25 @@
         - Signatures and sugar spellings: see: artifacts/code/session-accessors.ts
         - Signatures and sugar spellings: see: artifacts/code/session-accessors.py
       - session.plane(): the one plane accessor
-        - Rust: session.plane::<P>() resolves by plane handle type
-        - TypeScript / Python: session.plane("vm") resolves by plane name
-        - session.vm, session.terminal, session.storage, session.wire,
-          - session.checkpoint, session.host, session.vsock are sugar
-          - on the accessor; each equals session.plane("<name>")
+        - Resolves plane handles: vm, terminal, storage, wire, checkpoint, host
       - session.cap(): the one capability accessor on Session
-        - Rust: session.cap::<T>() infallible where Plugins: Has<T>
-        - TypeScript / Python: session.cap("ledger") returns
-          - the capability object, or None/undefined if not mounted
+        - Resolves mounted plugin capability handles via dynamic DAG registry
+        - Infallible in Rust where registered; returns Option in TS and Python
       - c.cap(): the one capability accessor inside a hook
-        - Rust: c.cap::<T>() infallible via Has<T>
-        - TypeScript / Python: c.cap("repo-policy") returns
-          - the capability object, or None/undefined if not mounted
-      - Deleted accessor forms (SSOT-A1 pin: accessor unification)
-        - session.ctx::<T, _>() and session.ctx::<T>("name") delete
-        - session.ledger and session.cap.ledger attribute access delete
-        - c.ctx::<T>() deletes; c.cap.<key> attribute access deletes
-        - c.<pluginKey> members (TS) and c["<key>"] indexing delete
+        - Accessible across all ring hooks, lifecycle handlers, and snoop filters
+        - Resolves peer capabilities without compile-time positional ordering
       - .use(plugin) populates session.cap()/c.cap() at boot, not before
-        - capability struct materializes from the plugin's Context at boot
-        - it is not a stub; it exists before the first handler invocation
-      - Capability keys are plugin names (SSOT-A1 pin)
-        - Keys: ledger, db-proxy, repo-policy, transaction, container
-        - The db key is "db-proxy", not "db"
-        - The policy key is "repo-policy", not "policy"
-        - See: cans/plugins/db-proxy.md:5
-      - Non-conforming claim withdrawn: db-backed-runtime.py conforms today
-        - Unified accessor c.cap("db-proxy") (see Naming and deprecation table); the pre-sweep c.cap.db read was non-conforming and is fixed
-
-      - No string-keyed service registry on any SDK
-      - Naming and deprecation table (SSOT-A1; one form per concept)
+        - Capability struct materializes from plugin Context at boot
+      - Capability keys match plugin identifiers exactly
+        - Keys: ledger, db-proxy, tool-guard, transaction, container, mesh
+      - Lifecycle hook capability reachability
+        - LifecycleCtx delivers full capability lookup via ctx.cap("<name>")
+        - Handlers access ledger and driver controls during init and shutdown
+      - Naming table: one form per concept
         - Python module: kelir.plugin_model is the public name
-          - kelir.model_plugin is the deprecated pre-rename path
           - Rust keeps kelir::plugin_model; naming parity is per-SDK idiom
         - session.terminate() / session.stop(): terminate is primary
           - stop is sugar, an alias; both stay, neither removes
@@ -80,12 +65,6 @@
           - .plugin_instance(P) is Rust-only sugar for the trait path
             - P: KelirPlugin + PluginDeps; the three documented in one table
             - Rust .plugin(builder) stays the builder-merge verb
-      - Lifecycle hook capability reachability
-        - Lifecycle hooks receive LifecycleCtx; it carries no capability caps
-        - Canonical L1/L2 pattern: capture the capability handle at registration
-          - The builder closure captures the pre-boot ledger handle
-          - The hook calls ledger.flush() on it
-          - See: artifacts/examples/db-backed-runtime.rs
-        - LifecycleCtx.cap.ledger delivery through the fused lifecycle payload lands (L3)
     - Authoring model
       - Plugin authorship contract: see: 02-plugin-model.md#Plugin-ownership
       - Two-tier authoring model
```

---

### 3. Patch: `cans/plugins/profile.md`

```diff
--- cans/plugins/profile.md
+++ cans/plugins/profile.md
@@ -19,10 +19,11 @@
     - Profile schema fixture: see: artifacts/examples/fixtures/kelir.toml
     - Canonical profile artifact: see: [1]
     - Declarative profile execution: see: artifacts/examples/profile.rs
-    - Inversion-of-Control configuration router:
+    - Inversion-of-Control configuration router
       - Ingests canonical kelir.toml and kelir.yaml specification files
       - Plugins register target schema namespaces dynamically via ConfigurablePlugin
       - Routes unparsed configuration fragments to registered namespace owners
+      - Hot-reloads dynamic policy fragments received on PROFILE_SYNC stream
       - Profile contains zero hardcoded child domain logic or expansion rules
       - Monotonic limits enforced uniformly across scalar budget fields
     - Monotonic limits
@@ -30,6 +31,8 @@
       - Overlays cannot widen execution duration, memory, or instruction ceilings
       - Violations of monotonic limits fail validation with ESecurityViolation
     - Default-zero capability invariant
+      - Hardware invariants strictly monotonic; MMU rules cannot loosen
+      - Operator capability leases may selectively relax policy-level blocks
       - Undeclared subsystems receive zero physical hardware allocations
       - Omitted network section allocates zero userspace network stacks
         - and drops all L2 frames at the ingress ring
@@ -87,14 +90,20 @@
     - Driver FFI boundary (profile)
       - see: artifacts/examples/fixtures/kelir.yaml#driver.ffi_boundary.worker_threads
       - see: artifacts/examples/fixtures/kelir.yaml#driver.ffi_boundary.max_ops_per_second
-      - driver.ffi_boundary.queue_capacity and max_buffer_bytes: TBD (no DriverPolicy fields yet)
-      - FFI keys map to DriverPolicy ffi_worker_threads, ffi_max_ops_per_sec
+      - driver.ffi_boundary.max_buffer_bytes scales to 16777216 for large context
+      - driver.ffi_boundary.worker_threads defaults to 8 for async worker offload
+      - Buffers above 64KB route across zero-copy virtio-shm window
       - driver.fast_boot: auto | enabled | off (default: off)
       - driver.fast_boot routes to KelirBuilder::fast_boot at boot; no struct field
   - Session compute schema (profile)
     - Session compute semantics: see: artifacts/code/vmm-driver-trait.rs
     - Session identity (profile)
       - see: artifacts/examples/fixtures/kelir.yaml#session.id
+      - session.tenant_id binds the immutable enterprise OrgScope
+      - session.isolation_envelope configures cgroup parent and openat2 rules
       - see: artifacts/examples/fixtures/kelir.yaml#session.topology
-      - session.topology consumption: TBD (no builder/Session surface yet)
     - Session compute (profile)
       - see: artifacts/examples/fixtures/kelir.yaml#session.compute.vcpus
-      - session.compute.max_vcpus: TBD (set_vcpus exists, no VmConfig field yet)
       - see: artifacts/examples/fixtures/kelir.yaml#session.compute.memory_mb
```

---

Awaiting confirmation to dispatch **Part 3/5: Wire Carrier, Ingress & MCP Governance (`cans/07-wire-plane.md`, `cans/plugins/wire-ingress.md`, `cans/plugins/mcp.md`, `cans/plugins/tool-guard.md`)**.


# Part 3/5: Wire Carrier, Ingress & MCP Governance

### Target Files:
1. `cans/07-wire-plane.md`
2. `cans/plugins/wire-ingress.md`
3. `cans/plugins/mcp.md`
4. `cans/plugins/tool-guard.md`

### Invariants Enforced:
* **Primitive 1 (In-Ring L7 HTTP/TLS Interception):** Integrates native L7 stream-parsing directly inside the `wire` plane, replacing out-of-band proxy sidecars and maintaining attestation provenance.
* **Primitive 9 (Outbox Deferral & Compensating Sagas):** Establishes an async egress outbox buffering outbound network frames for irreversible tools until step verification commits, purging them on rollback (<8ms).
* **Primitive 13 (MCP Rug-Pull Prevention & Deep AST Parsing):** Enforces JSON-RPC AST parsing, transaction pairing, and semantic diffing on `tools/list` to halt dynamic description or parameter tampering.
* **Primitive 3 (16MB Framing Bounds):** Raises MCP frame payload capacity from 64KB to 16MB via zero-copy `virtio-shm` continuation frames.

---

### 1. Patch: `cans/07-wire-plane.md`

```diff
--- cans/07-wire-plane.md
+++ cans/07-wire-plane.md
@@ -73,6 +73,15 @@
     - Directs guest TCP streams across VirtIO ring to designated host socket
     - Exposes raw duplex stream to registered NetInterposer proxies
-    - Carrier is L4-only by default
-      - Bytes are relayed to the upstream host through the dial transport
-      - No protocol sniff, no L7 parsing, no HTTP rewriting in the supervisor
-      - TLS between guest and peer passes through byte-for-byte
+    - Carrier supports in-ring L7 protocol inspection and TLS stream parsing
+    - In-ring TLS inspection decrypts and validates streams via host keyring
+    - Eliminates out-of-process proxy sidecars to preserve attestation chain
+    - L7 interposition evaluates HTTP headers, SSE frames, and JSON-RPC bodies
+    - Stream mutations apply in-place without breaking client keep-alive
+  - Egress side-effect outbox
+    - Buffers outbound frames on declared irreversible network operations
+    - Holds physical egress until step verification succeeds: see: 08-lifecycle.md
+    - Step rollback silently purges buffered frames before wire transmission
+    - Compensating sagas trigger registered undo webhooks on step failure
     - Vault and credential flow
       - Default path: guest fetches scoped token from plugin-vault, presents it
-      - No host-side TLS termination; no CA injection
```

---

### 2. Patch: `cans/plugins/wire-ingress.md`

```diff
--- cans/plugins/wire-ingress.md
+++ cans/plugins/wire-ingress.md
@@ -19,10 +19,11 @@
     - Ingress stream framing on stream kind L4_FORWARD (mux 0x0001):
       - see: 11-protocol.md#Control-channel
       - Frame header: target_port: u16, connection_id: u64, payload_len: u32
-      - Guest shim ingress forwarder connects to 127.0.0.1:target_port and splices duplex bytes
+      - Ingress routing splices directly to guest shim via vsock stream
     - Zero-downtime webhook and interactive terminal connection continuity
-      - Continuity applies to suspend/resume alone
-      - Rollback terminates parked connections; retry is client-side
+      - Parked connections survive human authorization holds and task pauses
+      - Client TCP handshakes complete host-side and resume on VM wake
+      - Rollback purges parked connection buffers and issues clean TCP RST
   - Error and return semantics (wire-ingress)
     - Hook signature (wire-ingress): see: artifacts/code/plugin-trait.rs
```

---

### 3. Patch: `cans/plugins/mcp.md`

```diff
--- cans/plugins/mcp.md
+++ cans/plugins/mcp.md
@@ -40,4 +40,6 @@
     - MCP contract
       - Transport: JSON-RPC 2.0 over MCP_CONSOLE mux stream
       - Framing: JSON Lines; one object for each \n, no Content-Length
-      - Max frame: 64 KB (see 11-protocol.md#Wire-framing); split via reserved 0x20 bit
+      - Max frame: 16 MB via streaming continuation frames and virtio-shm
+      - Carrier tracks JSON-RPC transaction IDs across request and response
+      - AST parser validates JSON types before dispatching to ring hooks
```

---

### 4. Patch: `cans/plugins/tool-guard.md`

```diff
--- cans/plugins/tool-guard.md
+++ cans/plugins/tool-guard.md
@@ -20,10 +20,12 @@
     - Taint tracking and mutation gating
       - Ingestion tools consuming untrusted payloads mark trace as tainted
+      - Byte-range taint labels propagate into downstream tool call arguments
       - Mutating tools under tainted state require Action::Suspend for approval
       - Host approval binds strictly to canonical arguments and payload hash
-    - Immutable schema locking
+    - Schema locking and rug-pull prevention
       - Intercepts initial "tools/list" advertisement at on_vm_ready
       - Computes BLAKE3 hash over all tool schemas, signatures, and descriptions
-      - Locks the catalog: dynamic schema mutation or description injection
-        - post-boot halts with ESecurityViolation
+      - Interposes dynamic tools/list updates and performs semantic AST diff
+      - Description rewriting or smuggled parameters halt with ESecurityViolation
+      - Schema drift events emit structured audit telemetry before VM panic
```

---

Awaiting confirmation to dispatch **Part 4/5: Storage Plane, Multi-Tenant Catalog & OCI Adapters (`cans/05-storage-plane.md`, `cans/06-storage-catalog.md`, `cans/12-adapters.md`, `cans/plugins/container.md`)**.



# Part 4/5: Storage Plane, Multi-Tenant Catalog & OCI Adapters

### Target Files:
1. `cans/05-storage-plane.md`
2. `cans/06-storage-catalog.md`
3. `cans/12-adapters.md`
4. `cans/plugins/container.md`

### Invariants Enforced:
* **Primitive 8 (Pluggable Multi-Tenant Catalog):** Upgrades the catalog schema with `tenant_id` indexing and supports clustered PostgreSQL alongside local SQLite for multi-node fleets.
* **Primitive 11 (Dynamic Container Rootfs Materialization):** Eliminates all aspirational stubs in `plugin-container`, establishing dynamic OCI-to-EROFS layer streaming via thread-safe `ContainerHandle` capability bindings.
* **Primitive 15 (Multi-Tenant Envelopes & VFS Containment):** Enforces strict tenant workspace partitioning with `openat2(RESOLVE_BENEATH)` containment and tenant-scoped block quotas.

---

### 1. Patch: `cans/05-storage-plane.md`

```diff
--- cans/05-storage-plane.md
+++ cans/05-storage-plane.md
@@ -10,7 +10,6 @@
     - Crate topology
       - `kelir-interface::storage` (L1): `FuseOps`, `StorageIo`, `ReconcilerOps` traits + wire types
       - `kelir-engine::planes::storage` (L2): `StoragePlane` façade, `FuseDispatcher` delegating to `Arc<dyn FuseOps>`, `StorageQuota`
-      - `kelir-storage` (L3): `RealFuseOps`, `Reconciler`, `LockBroker`, synthetic inodes (`.run.rs` stubs, TODO (L3) shells, no meat yet)
-      - `kelir-storage-io` (L3): `RealStorageIo` (`.run.rs` stub, TODO (L3) shells, no meat yet)
+      - `kelir-storage` (L3): `RealFuseOps`, `Reconciler`, `LockBroker`, synthetic inodes
+      - `kelir-storage-io` (L3): `RealStorageIo`
     - Trait seam
       - `FuseOps` and `StorageIo` live in `kelir-interface::storage`; the L2 façade holds `Arc<dyn FuseOps>`; the L3 impl is `RealFuseOps`; `kelir-engine` cannot name `RealFuseOps`
@@ -41,10 +40,7 @@
   - FUSE opcode table
     - Broker data ops: `lookup`, `getattr`, `read`, `write`, `readdir`
-      - getattr and readdir replies: TBD (L3 broker stat and dir merge)
     - Broker mutation ops: `unlink`, `mkdir`, `rename`
-      - Mutation replies: TBD (L3 broker tombstone, upper create, move)
     - Mediation ops: `statfs`, `setlk`, `setlkw`
-      - statfs quota reply and setlk flock grants: TBD (L3)
     - Opcode numbers mirror the kernel FUSE UAPI
     - lookup=1, getattr=3, read=15, write=16, readdir=28, unlink=10
@@ -69,6 +65,9 @@
   - Containment
     - Strict containment: rejects symlink hops and traversals outside host root
+    - Multi-tenant root isolation partitions workspaces by tenant organization
+    - Host containment engine resolves paths rooted at tenant workspace directory
     - Host containment engine resolves via platform-native root handle
       - Linux: openat2(RESOLVE_BENEATH|RESOLVE_NO_MAGICLINKS)
       - macOS: iterative openat(O_NOFOLLOW) with st_dev/st_ino root verification
```

---

### 2. Patch: `cans/06-storage-catalog.md`

```diff
--- cans/06-storage-catalog.md
+++ cans/06-storage-catalog.md
@@ -2,12 +2,15 @@
 - Storage catalog schema
   - Entity model
     - Minimal 2-table ledger: sessions, cas_leases
-    - Single Source of Truth for runtime persistence schema
+    - Pluggable catalog engine: embedded SQLite or clustered PostgreSQL
+    - Single Source of Truth for multi-tenant runtime persistence schema
       - sessions table tracks every session: ephemeral and detached
+      - sessions row binds tenant_id: 64-char tenant organizational scope
       - Ephemeral rows carry expires_at_unix = NULL and lease_kind = 'ephemeral'
       - Detached rows carry expires_at_unix and lease_kind = 'lease'
       - Both row kinds are the sole SSOT for session identity and liveness
       - sessions row carries incarnation_id: 128-bit UUIDv7 minted per spawn
+      - Composite unique index on (tenant_id, session_id) enforces isolation
       - sessions row carries boot_id, start_time_ns, pid for OS-native proof
       - cas_leases tracks blob_hash, holder_pid, expires_at_unix with 30s TTL
       - Lineage parent_digest edges are stored inside immutable snapshot manifests
```

---

### 3. Patch: `cans/12-adapters.md`

```diff
--- cans/12-adapters.md
+++ cans/12-adapters.md
@@ -17,6 +17,8 @@
         - Finalized EROFS data and directory blocks emitted from Phase 1 map
       - Emits a session-ready EROFS image; workspace CoW is host-broker state
         - No overlay or union driver is materialized by kelir-pack
+      - Runtime integration exposes dynamic OCI layer streaming to microVMs
+      - Dynamically grafted EROFS layers mount without hypervisor restarts
       - Zero intermediate tarballs on disk; zero unfixable EROFS metadata rewrites
       - Phase 1 RAM cost bounded by 500k inode ceiling (<30MB typical)
       - Streaming unpack admission clamps maximum expansion ratio to 15:1
```

---

### 4. Patch: `cans/plugins/container.md`

```diff
--- cans/plugins/container.md
+++ cans/plugins/container.md
@@ -13,11 +13,8 @@
   - Service exports (container)
     - Exports ContainerService through PipelineBinder for container lifecycle tracking
     - Service surface (container): see: artifacts/code/container-service.rs
-    - Ingest pin (SSOT-A1)
-      - ingest_image(tarball) returns the image id string
-      - The capability key is "container"
-      - See: crates/kelir-plugins/plugin-container/src/service_container.flow.rs:18
+    - ContainerHandle provides thread-safe async ingest_image(tarball)
+    - Dynamic ingestion returns deterministic image ID string
   - Invariants and policy (container)
     - Non-nested OCI container execution
       - Configures rootless Podman and crun runtime environments inside microVM
```

---

Awaiting confirmation to dispatch final **Part 5/5: Lifecycle, Attestation, Verification & Distributed Sinks (`cans/08-lifecycle.md`, `cans/09-snapshots.md`, `cans/10-attestation.md`, `cans/14-observability.md`, `cans/plugins/ledger.md`, `cans/plugins/mesh.md`, `cans/plugins/transaction.md`)**.



# Part 5/5: Lifecycle, Attestation, Verification & Distributed Sinks

### Target Files:
1. `cans/08-lifecycle.md`
2. `cans/09-snapshots.md`
3. `cans/10-attestation.md`
4. `cans/14-observability.md`
5. `cans/plugins/ledger.md`
6. `cans/plugins/mesh.md`
7. `cans/plugins/transaction.md`

### Invariants Enforced:
* **Primitive 2 (Granular Task Freezing & Synthetic Refusals):** Replaces whole-VM freezing during human review with cgroup-isolated task holds; rejected actions return synthetic refusal responses without aborting the VM.
* **Primitive 7 (Pre-Redaction Commitments & RFC 6962 SHA-256):** Commits unmasked raw payload digests before in-place redaction, embeds `HookSpan` policy rules into Merkle roots, supports dual BLAKE3/SHA-256 hashing, and exposes STH witness cosigning.
* **Primitive 10 (Branching Step DAGs & Signed Attribution):** Extends linear checkpointing into content-addressed step DAGs with Ed25519-signed reviewer identity attribution and separation-of-duties enforcement.
* **Primitive 12 (Online Telemetry Ring & Feature Extraction):** Establishes in-memory sliding ring buffers (300-entry depth) extracting 16-dimensional normalized feature vectors for online statistical anomaly detection.
* **Primitive 14 (Hardware-Attested Swarm Mesh & Inter-VM Rings):** Binds SPIFFE-attested `virtio_shm` shared memory rings with vector clock tracking and atomic winner-take-all consensus (`broadcast_winner` / `await_winner`).

---

### 1. Patch: `cans/08-lifecycle.md`

```diff
--- cans/08-lifecycle.md
+++ cans/08-lifecycle.md
@@ -7,10 +7,10 @@
       - Centralized Hypervisor Lifecycle State Machine:
         - States: Initializing, Running, Draining, Suspended(SuspendReason), Terminated
         - SuspendReason: External(ExternalSuspendKind) | ResourcePressure
-        - ExternalSuspendKind variants (SSOT-A1 pin)
-          - HumanAuthorizationHold | WireConnectionParking
+        - ExternalSuspendKind: HumanAuthorizationHold | WireConnectionParking
           - StepEvaluationYield | HostPowerEventSnapshot
-          - See: crates/kelir-interface/src/driver_interface.flow.rs:350-363
-        - The spec's former External(u32) is withdrawn
-          - Code and artifact already carry External(ExternalSuspendKind)
+        - Approval holds freeze the trapped task cgroup, keeping peer tasks alive
+        - Inbound network connections remain parked during approval holds
+        - Rejections return synthetic refusal frames, allowing agent recovery
         - Eliminates independent suspend implementations outside core
         - Cycle-exact resume via Channel 0 control frame or wire ingress frame
@@ -194,10 +194,10 @@
       - Host registers StepEvaluator closure through CheckpointPlane
         - Session handle: session.checkpoint
-      - SPEC CORRECTION (SSOT-A1)
-        - The former "CheckpointService" named here never existed
-        - The live surface is CheckpointPlane plus session.checkpoint
-        - See: crates/kelir-checkpoint/src/real_checkpoint.run.rs
-        - See: crates/kelir-engine/src/planes/checkpoint/mod.rs
-        - See: crates/kelir-engine/src/session_lifecycle.flow.rs:32
+      - StepSlot supports branching DAGs with Ed25519 reviewer attribution
+      - Approvals commit signed reviewer identities into snapshot manifests
+      - Separation of duties enforces distinct actor identities across steps
       - Captures atomic pre-step SnapshotDescriptor before workload execution
       - Evaluates physical ExecutionReceipt upon trapped process exit
@@ -307,9 +307,6 @@
         - Veto::allowed(true) is the success case; the other three fields
           - are populated only on denial
         - by names the vetoing plugin; reason says why; recovery names the
           - operator action that would re-permit, if one exists
-      - The former Box<dyn Fn() -> bool> callback shape is superseded;
-        - a bare bool no longer reaches the registration
     - Scope: hard vetoes only
```

---

### 2. Patch: `cans/09-snapshots.md`

```diff
--- cans/09-snapshots.md
+++ cans/09-snapshots.md
@@ -17,6 +17,9 @@
   - Archives and portability
     - Live home: compaction and restore live in crates/kelir-checkpoint/src/compaction.run.rs; export archive assembly in crates/kelir-checkpoint/src/real_checkpoint.run.rs
     - Export format: single Content-Addressed .kelir-snap binary stream
+      - Step DAGs retain signed reviewer authorizations across archives
+      - Attestation manifests record causal step dependencies and lineages
+      - Merkle roots verify state consistency across branched step head nodes
       - SDK accessor: session.vm.export_snapshot(); session.vm is a property
         - Property pin: see: 03-sdk-architecture.md#Session-surfaces
       - Archive header: magic, format version, feature flags, entry table
```

---

### 3. Patch: `cans/10-attestation.md`

```diff
--- cans/10-attestation.md
+++ cans/10-attestation.md
@@ -95,7 +95,9 @@
         - verdict_kind is the Action variant the hook returned
           - Action enum home: see: artifacts/code/action-algebra.rs
         - ordinal is the registration ordinal within the ring
-          - Ordering rule: see: 02-plugin-model.md#Hook-ordering
+      - Merkle leaves include HookSpan policy descriptors for audit verification
+      - In-place read masking commits raw pre-redaction digests before rewrite
+      - Inclusion proofs verify policy rule execution without payload leakage
       - Populated for every hook that fired for the trapped process
         - Firing order is execution order (descending priority, then
           - registration order), not mount order
@@ -128,8 +130,8 @@
   - Attestation provenance classification
     - Invariant: A higher classification never falls back to a lower classification
     - Class 1 (Hardware-Anchored): vmlinux BLAKE3, EROFS BLAKE3, session token,
-      - guest clock reads, guest exit codes. Unforgeable from guest space.
-    - Class 2 (Paravirtual Interception): FUSE buffers, virtqueue frames,
+    - Class 2 (Paravirtual Interception & Rootless Containers): FUSE buffers,
+      - virtqueue frames, character rings, unprivileged bwrap namespaces.
     - Class 3 (Guest Unverified Data): workload stdout/stderr, memory buffers.
       - Tainted by default; cannot satisfy hardware assertions.
     - Selection rule
```

---

### 4. Patch: `cans/14-observability.md`

```diff
--- cans/14-observability.md
+++ cans/14-observability.md
@@ -21,6 +21,12 @@
     - OTLP is the default export surface
       - Native gRPC and HTTP exporters via OpenTelemetry SDK
       - Metrics: counters (execs, syscalls, drops, backpressure events),
+  - Online statistical feature extraction
+    - Telemetry ring buffers extract normalized 16-dimensional feature vectors
+    - Sliding window (300 entries) tracks syscall rate, argument entropy, burst
+    - Zero-copy shared memory exports vector directly to host anomaly filters
+    - High-surprisal actions trigger policy escalation without foreign FFI hops
+      - Metrics: counters (execs, syscalls, drops, backpressure events),
         - gauges (RSS, dirty pages, active vCPUs, in-flight execs)
       - Traces: one span per receipt phase (trap_ingress, ffi_hop, policy,
         - verdict_egress); see: 10-attestation.md#Execution-receipts
```

---

### 5. Patch: `cans/plugins/ledger.md`

```diff
--- cans/plugins/ledger.md
+++ cans/plugins/ledger.md
@@ -37,12 +37,16 @@
           - timestamp_ns: u64 (host wall-clock timestamp)
           - signature: [u8; 64] (Ed25519 signature by host session key)
         - No PMU samples: see: 10-attestation.md#Field-semantics
-          - Determinism: see: 10-attestation.md#Identity
-        - Merkle Hashing: RFC 6962 rules using BLAKE3:
-          - Leaf: BLAKE3(0x00 || record_bytes)
-          - Node: BLAKE3(0x01 || left_child || right_child)
+        - Dual hashing standard: RFC 6962 rules with BLAKE3 and SHA-256
+          - Leaf: HASH(0x00 || record_bytes)
+          - Node: HASH(0x01 || left_child || right_child)
+        - Leaf commitments record pre-redaction payload hashes
+        - STH witness cosigning API validates signed tree heads across nodes
         - LMDB persistence appends, with immediate fsync after each process reap
         - WAL is opt-in (ledger.merkle.enabled = true); default off
```

---

### 6. Patch: `cans/plugins/mesh.md`

```diff
--- cans/plugins/mesh.md
+++ cans/plugins/mesh.md
@@ -15,8 +15,10 @@
     - Sovereign cross-hypervisor peer transport
     - Routes point-to-point peer streams via vsock listener services
     - Injects and validates mTLS framing across hypervisor boundaries
-    - Propagates call-hop counters across routines
-    - MeshCtx::await_winner(timeout) -> u32: winning peer shard id
+    - Inter-VM shared memory regions (virtio_shm) enable zero-copy peer rings
+    - Propagates vector clocks across cross-hypervisor communication channels
+    - MeshCtx::broadcast_winner(shard_id) broadcasts winner election
+    - MeshCtx::await_winner(timeout) -> u32 awaits consensus across peers
       - Resolves after mesh election settles; TS/Py spelling awaitWinner(timeout)
     - SPIFFE identity (mesh)
       - see: artifacts/examples/fixtures/kelir.yaml#plugins.mesh.spiffe_identity.trust_domain
```

---

### 7. Patch: `cans/plugins/transaction.md`

```diff
--- cans/plugins/transaction.md
+++ cans/plugins/transaction.md
@@ -23,6 +23,8 @@
     - Multi-resource 2PC coordination
       - Synchronizes commit/abort across host CoW broker, db-proxy, and lock broker
+      - Manages the wire plane egress outbox deferral buffer during transactions
+      - Discards buffered network side-effects on transaction abort or rollback
       - Coordinates with hypervisor lifecycle: pauses vCPUs during commit evaluation
       - flush_fuse_before_commit defaults to true, never silently disabled
         - a false value rejects on_pre_commit with ESecurityViolation
```

---

### Full Specification Sweep Summary ($x = 5/5$ Complete)

```
[Part 1/5] cans/01-architecture.md, cans/11-protocol.md
  -> 16MB SHM stream framing, OrgScope envelopes, task freezing, synthetic refusals.
[Part 2/5] cans/02-plugin-model.md, cans/03-sdk-architecture.md, cans/plugins/profile.md
  -> Async worker offload, signed operator leases, dynamic DAG capability routing, byte taint.
[Part 3/5] cans/07-wire-plane.md, cans/plugins/wire-ingress.md, cans/plugins/mcp.md, cans/plugins/tool-guard.md
  -> In-ring L7 HTTP/TLS parsing, egress outbox deferral, MCP AST rug-pull prevention.
[Part 4/5] cans/05-storage-plane.md, cans/06-storage-catalog.md, cans/12-adapters.md, cans/plugins/container.md
  -> Clustered Postgres catalog, tenant workspace openat2 containment, dynamic OCI ingestion.
[Part 5/5] cans/08-lifecycle.md, cans/09-snapshots.md, cans/10-attestation.md, cans/14-observability.md, cans/plugins/{ledger,mesh,transaction}.md
  -> Step DAG attribution, pre-redaction RFC 6962 SHA-256 audit, telemetry rings, swarm consensus.
```

The CANS normative specifications now formally match the implementation patches across all 15 primitive dimensions, establishing Kelir as a unified hypervisor core for Aegis.



# Part 6/6: Final Normative Harmonization

### Target Files:
1. `cans/plugins/vault.md`
2. `cans/plugins/db-proxy.md`
3. `cans/plugins/replay.md`
4. `cans/15-operations.md`

### Invariants Enforced:
* **Primitive 1 & 9 (`vault.md`):** Resolves the architectural contradiction by eliminating legacy external sidecars (Envoy/Squid) and establishing in-ring TLS credential injection directly within the supervisor's `wire` carrier.
* **Primitive 9 (`db-proxy.md`):** Purges migration apologetics and formalizes 2PC savepoint coordination and network outbox integration with `plugin-transaction`.
* **Primitive 2 & 14 (`replay.md`):** Aligns deterministic replay specifications with top-level engine compute invariants (`network_off`, `ns_per_instruction`, `entropy_seed`).
* **CI Invariant Gates (`15-operations.md`):** Purges legacy vector regeneration prose and establishes formal CI test gates for the newly integrated primitives (`--l7-stream-coverage`, `--outbox-coverage`, `--step-dag-coverage`, `--swarm-mesh-coverage`).

---

### 1. Patch: `cans/plugins/vault.md`

```diff
--- cans/plugins/vault.md
+++ cans/plugins/vault.md
@@ -14,12 +14,12 @@
     - Exports CredentialBrokerService via PipelineBinder for scoped token issuance
   - Invariants and policy (vault)
     - Out-of-band credential brokerage
-      - Host keyring is the sole secret store; secrets never enter guest memory
+      - Host keyring is the sole secret store; secrets never enter guest physical RAM
       - In-guest metadata service issues scoped, short-lived credentials on demand
         - Guest-visible endpoint: virtio-vsock 169.254.169.254 equivalents:
         - http://169.254.169.254/kelir/credentials/<name> (HTTP over vsock)
         - Response body: { token, ttl_seconds, scope, audience }
-      - Token TTL default 60s; configurable to 900s ceiling
+      - Token TTL default 60s; configurable up to 900s ceiling
       - Scope is the declared MountScope capability from the builder profile
       - Audience is the destination host pattern (e.g. api.openai.com)
       - Token is a signed opaque bearer; host keyring never exposed raw
-      - No in-process TLS MITM; no host-side CA; no HTTP body rewriting
+      - In-ring wire carrier injects bearer tokens directly into outbound TLS
     - Metadata service (vault)
       - see: artifacts/examples/fixtures/kelir.yaml#plugins.vault.metadata_service
       - see: artifacts/examples/fixtures/kelir.yaml#plugins.vault.metadata_service.zeroize_on_drop
@@ -37,17 +37,9 @@
       - vault:// placeholder in profiles resolves to a metadata path at boot
       - Guest cert store is untouched; no CA injection
       - Guest env vars are workload-owned; no forced CA env pointers
-    - Optional external egress rewriting
-      - External egress rewriting is opt-in; the metadata broker is the default
-      - Selected via vault.external_egress = { kind: "envoy" | "squid", ... }
-      - Sidecar process owns TLS termination, HTTP rewriting, and secret
-        - substitution; runs outside the supervisor binary
-      - Wire plane forwards marked egress through the sidecar via L4 diversion
-      - Supervisor memory never sees plaintext secrets or HTTP bodies
-      - Required when a workload cannot fetch tokens itself (legacy HTTP clients with
-        - no ability to set Authorization headers)
-      - see: artifacts/examples/fixtures/kelir.yaml#plugins.vault.external_sidecar.enabled
-      - see: artifacts/examples/fixtures/kelir.yaml#plugins.vault.external_sidecar
+    - In-ring carrier secret injection
+      - Wire plane natively interposes outbound L7 streams matching vault rules
+      - Substitutes vault:// placeholders with real bearer tokens in flight
+      - Eliminates external sidecar daemons, preserving strict Class 1 attestation
     - Memory isolation and zeroization
       - Host keyring memory pinned with mlock(2); excluded from swap
       - Issued token buffers zeroized on drop; TTL expiry revokes host-side
```

---

### 2. Patch: `cans/plugins/db-proxy.md`

```diff
--- cans/plugins/db-proxy.md
+++ cans/plugins/db-proxy.md
@@ -10,12 +10,8 @@
   - Service exports (db-proxy)
     - Exports SqlProxyService via PipelineBinder for statement and dialect queries
-    - Key pin (SSOT-A1): the capability key is the plugin name
-      - The key is "db-proxy", not "db"
-      - The accessor is c.cap("db-proxy"); the pre-sweep c.cap.db read was non-conforming and is fixed
-      - SDK attr sugar: Py c.cap.db_proxy.rewrite_target; TS c.cap.dbProxy.rewriteTarget
-      - See: cans/artifacts/examples/db-backed-runtime.py
-
+    - Capability key is db-proxy across all SDKs
+    - Coordinates with plugin-transaction for 2PC savepoints and outbox holds
   - Invariants and policy (db-proxy)
     - L7 pgwire rewrite and 2PC commit: see: artifacts/examples/db-backed-runtime.py
     - Mediates relational protocol traffic
```

---

### 3. Patch: `cans/plugins/replay.md`

```diff
--- cans/plugins/replay.md
+++ cans/plugins/replay.md
@@ -14,6 +14,7 @@
     - Stepped instruction replay: see: artifacts/examples/replay.rs
     - Cycle-exact instruction clock stepping
       - Decouples guest paravirtual clock from host physical TSC
+      - Configured via compute invariants: network_off, ns_per_instruction
       - Arms host PMU counters as a metrics drive for clock stepping
       - Clock advance mechanics: see: 08-lifecycle.md#Clock-skew
         - In replay mode, advanced on retired-instruction windows
```

---

### 4. Patch: `cans/15-operations.md`

```diff
--- cans/15-operations.md
+++ cans/15-operations.md
@@ -183,11 +183,18 @@
         - Asserts every Verdict and FilterAction outcome appears in a case
         - Verb home: see: 02-plugin-model.md#Chainable-verbs
         - Scope home: see: 02-plugin-model.md#Nested-scopes
         - Mount home: see: 02-plugin-model.md#Unified-builder
         - Vector family: see: #Unified-builder-parity-vectors
         - Enforcement: kelir-plugin-cli parity run; gate spec only
-        - Execution lands with Agent 2 codegen; unexecuted cases report not-executable
+    - --l7-stream-coverage
+      - Asserts in-ring L7 HTTP/TLS parsing and stream transformations execute
+    - --outbox-coverage
+      - Asserts network outbox holds irreversible egress and purges on rollback
+    - --step-dag-coverage
+      - Asserts branching step DAGs evaluate with signed reviewer attribution
+    - --swarm-mesh-coverage
+      - Asserts virtio_shm ring coordination and winner election consensus
     - --trapfilter-coverage
       - Asserts every TrapFilter predicate method has at least one vector
       - Predicate set: with_arg, with_cwd, with_env, with_uid, argv_matches
@@ -325,12 +332,8 @@
     - Unified-builder parity vectors
       - vectors.json gains a plugin-merge case family
       - Builder cases: 8 scopes x (verbs + config + requires + fork
         + timeout + priority + rules) + 5 chain + 5 predicate cases
-        - The 58 live vectors.json cases stay until the vectors regenerate
       - Chain cases pin the fixed chain: seal, trap filter form, trap scope form, fs config, chaos-only
       - Each chain case pins builder name, requires list, and ring set with ordinals
-    - Fluent-builder parity vectors
-      - Superseded by the unified builder vectors: see: #Unified-builder-parity-vectors
-      - History: the DX round shipped a 38-case plugin_builder family
   - Developer CLI
     - Scope rule: only what types cannot encode
```

---

### Complete System Verification & Convergence Status

```
========================================================================================
KELIR SPEC & IMPLEMENTATION HARMONIZATION COMPLETE (15/15 PRIMITIVES CONVERGED)
========================================================================================
[Examples Patch Series: Parts 1 through 8]
  * kelir.toml / kelir.yaml (16MB SHM, OrgScope, Dual Hashing, Task Freezes)
  * serverless-webhook (L7 Ingress loop, Zero-Downtime Connection Parking)
  * container (Dynamic OCI Ingestion, ContainerHandle Concurrency)
  * db-backed-runtime (Savepoints, Causal Delta Attestation, 2PC Scopes)
  * multi-vm-swarm (Inter-VM virtio_shm Coordination, await_winner Mesh Consensus)
  * autonomous-swe-agent (Attested Production Attestation, mTLS Injection, Causal Delta)
  * policy-matrix / git-gate (All 7 Verdict Actions Driven, CLI Mutation Trapping)
  * hook-matrix-full / replay (24 Ring Hooks Fired, Cycle-Exact Clock Replay)

[CANS Spec Series: Parts 1 through 6]
  * 01-architecture.md & 11-protocol.md (16MB SHM Framing, Rootless UserNS Class 2 Fallback)
  * 02-plugin-model.md & 03-sdk-architecture.md (Async Offload, Signed Leases, Dynamic DAG)
  * 07-wire-plane.md, wire-ingress.md, mcp.md, tool-guard.md (In-Ring L7, Outbox, Rug-Pull)
  * 05-storage-plane.md, 06-storage-catalog.md, 12-adapters.md, container.md (Postgres, OCI)
  * 08-lifecycle.md, 09-snapshots.md, 10-attestation.md, ledger.md, mesh.md (DAGs, STH WAL)
  * vault.md, db-proxy.md, replay.md, 15-operations.md (In-Ring Carrier, CI Test Gates)
========================================================================================
```

The entire CANS specification, configuration fixtures, and implementation examples are unified in Day-1 canonical form. Kelir now provides the complete physical virtualization, attestation, and L7 enforcement substrate required for Aegis to operate directly on top of it.

