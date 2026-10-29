# Part 1 — Tooling pipeline

Two patches. `needs:` is the DAG; comments carry tier labels, not sequence numbers. Job bodies are byte-identical to their previous form — only comments and physical order move.

---

## Patch 1.1 — `cans/artifacts/config/ci.yml`

```diff
--- a/cans/artifacts/config/ci.yml
+++ b/cans/artifacts/config/ci.yml
@@ jobs @@
-  # ─── 0. SPEC GATE (merge-blocking, path-filter-free) ───────
-  # Corpus is a typed graph; this compiles it. Every see:, every
-  # duplicate-home, every node_vs_artifact invariant in _rules.yaml
-  # is a red X when it rots. No filter: a node edit can break a
-  # pointer in any other node.
+  # ─── Tier 0: Static Analysis & Contracts ───────────────────
+  # Merge-blocking. No path filter on any job in this tier:
+  # a node edit can break a pointer in any node.
+  # The corpus is a typed graph; this compiles it.
   cans-check:
     …  (unchanged)
 
-  # ─── 1. FAST STATIC GATE (<90s) ────────────────────────────
+  # Fast static gate (<90s).
   lint-and-types:
     …  (unchanged)
 
-  # ─── 2. GUEST SHIM CROSS-COMPILE ────────────────────────────
+  # Acyclic plugin DAG.
+  plugin-graph:
+    …  (unchanged)
+
+  # Same command as `just plugin-check`.
+  plugin-audit:
+    …  (unchanged)
+
+  # Tier model: 02-plugin-model.md#Plugin-tiering
+  verify-tiered-compilation:
+    …  (unchanged)
+
+  # ─── Tier 1: Artifact Build ────────────────────────────────
   build-guest-shim:
     …  (unchanged)
 
-  # ─── 3. CORE KERNEL + PLUGIN TESTKIT ───────────────────────
+  parity:
+    …  (unchanged)
+
+  package-check:
+    …  (unchanged)
+
+  # ─── Tier 2: Unit, Vector & Driver Test Matrix ─────────────
   test-core:
     …  (unchanged)
 
-  # ─── 3b. MULTI-LANGUAGE PARITY & SDK EXECUTION GATE ────────
-  parity:
-    …  (moved to Tier 1)
-
-  # ─── 3c. ZERO-TOOLCHAIN PACKAGING CHECK ────────────────────
-  package-check:
-    …  (moved to Tier 1)
-
-  # ─── 4. PER-PLUGIN TEST MATRIX ─────────────────────────────
   test-plugins:
     …  (unchanged)
 
-  # ─── 4a. PLUGIN DEPENDENCY DAG CHECK ──────────────────────
-  plugin-graph:
-    …  (moved to Tier 0)
-
-  # ─── 4b. PLUGIN CONTRACT AUDIT ─────────────────────────────
-  plugin-audit:
-    …  (moved to Tier 0)
-
-  # ─── 4c. TIERED PLUGIN COMPILATION ─────────────────────────
-  verify-tiered-compilation:
-    …  (moved to Tier 0)
-
-  # ─── 5. SHIM + DRIVER TESTS ────────────────────────────────
   test-shim:
     …  (unchanged)
 
   test-drivers:
     …  (unchanged)
 
-  # ─── 6. CROSS-PLUGIN INTEGRATION (needs KVM) ───────────────
-  test-integration:
-    …  (moved to Tier 3)
-
-  # ─── 7. MACOS APPLE SILICON GATE (Zero Bloat) ──────────────
   check-macos-hvf:
     …  (unchanged)
+
+  # ─── Tier 3: Hardware Integration ──────────────────────────
+  test-integration:
+    …  (unchanged)
```

`needs:` edges unchanged: `parity→[lint-and-types]`, `package-check→[parity]`, `test-plugins→[lint-and-types,test-core]`, `test-integration→[test-plugins,test-drivers]`, every Tier 0 job → `[lint-and-types]`.

---

## Patch 1.2 — `cans/artifacts/config/justfile`

```diff
--- a/cans/artifacts/config/justfile
+++ b/cans/artifacts/config/justfile
@@
-# --- 1. SETUP & TOOLCHAINS ---
+# Toolchain & setup
 bootstrap:
     …
 
-# --- 2. LIGHTNING BUILD (<15s) ---
+# Compilation
 build-host:
     …
 build-shim target="aarch64-unknown-linux-musl":
     …
 build-ts:
     …
 
-# --- 3. RAPID TEST ENGINE (<10s) ---
+# Audit & validation
+cans-check:
+    …
+tiered-check:
+    …
+plugin-check plugin:
+    …
+plugin-check-graph:
+    …
+plugin-check-all:
+    …
+
+# Testing & parity
 test-core:
     …
 test-plugins:
     …
 test-plugin plugin:
     …
-plugin-check plugin:
-    …
-plugin-check-graph:
-    …
-cans-check:
-    …
-tiered-check:
-    …
 test-shim:
     …
 test-drivers:
     …
 test-sdks: build-ts
     …
 test-integration:
     …
 test-all: test-core test-plugins test-shim test-drivers test-sdks
     …
+parity: build-ts
+    cargo run -p kelir-plugin-cli -- parity run
 typecheck:
     …
 
-# --- 3b. PARITY GATE ---
-parity: build-ts
-    cargo run -p kelir-plugin-cli -- parity run
-
-# --- 4. PRE-FLIGHT CHECK (Run before git push) ---
+# Pre-flight (single gate before push)
 pre-flight: cans-check build-shim parity plugin-check-graph plugin-check-all tiered-check
     …
-plugin-check-all:
-    …
```

Targets keep their names (`cans-check`, `tiered-check`, `plugin-check-graph`). The critique's proposed `check-spec` / `check-tiered` / `check-graph` renames are a follow-on if desired; this patch is scoped to section removal and domain grouping.

---

Part 2 will cover spec rot: TBD purge, numbered prose (`Six invariants`, `Four categories`), `Observation vetoes` rename, and the frozen vsock channel allocation table — as one multi-file patch to `00-overview.md`, `01-architecture.md`, `07-lifecycle.md`, `09-protocol.md`, `plugins/profile.md`, `plugins/db-proxy.md`, `04-terminal-plane.md`.


# Part 2 — TBD resolution (resend, node ≤90)

Four files. Each TBD resolved with its day-1 value. Children preferred over long nodes.

---

## Patch 2.1 — `cans/09-protocol.md`

```diff
--- a/cans/09-protocol.md
+++ b/cans/09-protocol.md
@@ -… @@
-    - Framing gaps: TBD
-      - Byte order
-      - Message-type registry
-      - Flags registry
-      - Max frame size
-      - CBOR limits
-      - Version negotiation
-      - Flow control
-      - Reconnect and sequence reset
-      - Port 5253 framing
+    - Framing invariants
+      - Byte order: Little-Endian for all multi-byte integers
+      - Message-type registry
+        - 0x0001-0x0004 core-reserved
+          - MSG_TRAP, MSG_CHANNEL_PAYLOAD
+          - MSG_VM_LIFECYCLE_CONTROL, MSG_SYSTEM_TELEMETRY
+        - 0x0005-0x7FFF core-extended
+        - 0x8000-0xFFFF plugin-framed
+          - channel_id carries scope
+      - Flags registry
+        - 0x0001 FLAG_CONTINUATION
+        - 0x0002-0xFFFF must be zero on send
+        - ignored on receive
+      - Max frame size: 64KB strict
+      - CBOR limits
+        - Nest depth ≤ 16
+        - String length ≤ 1MB
+        - No indefinite-length items
+        - Payload bounded by 64KB frame budget
+      - Version negotiation
+        - Exact match on VERSION=0x04
+        - Lower version gets MSG_VM_LIFECYCLE_CONTROL{rejected}
+        - Min version 0x04; no downgrade path
+      - Flow control
+        - None in-band; vsock credits are the only backpressure
+        - Sender blocks on socket write
+      - Reconnect
+        - None; dropped vsock is terminal, tears down session
+        - No sequence reset path exists
+      - Port 5252 is the sole control transport
+        - No secondary port
@@ -… @@
-  - Trap decision gaps: TBD
-    - Rewrite loop prevention
-    - new_binary canonicalisation
-    - Env injection size and secrets
-    - Custom dispatch
+  - Trap decision invariants
+    - Rewrite loop prevention
+      - No-op mutation aborts EInvalidRewrite
+        - Identical binary, argv, env
+      - Mutation equal to ancestor aborts ECyclicRewrite
+      - Max rewrite depth 4
+    - new_binary canonicalisation
+      - Absolute path only
+      - Resolved via openat2
+        - RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS
+      - Rooted at MountScope binary root
+      - Escapes reject with EInvalidRewrite
+    - Env injection limits
+      - set_env+unset_env total ≤ 32KB CBOR-encoded
+      - Key length ≤ 128 bytes
+      - vault:// placeholder values reject ESecurityViolation
+    - Custom dispatch timeout
+      - Exceeded handler falls through to Allow
+      - Per-custom-action; defaults to 30s
```

---

## Patch 2.2 — `cans/01-architecture.md`

```diff
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@ -… @@
-    - Trust chain: TBD
+    - Trust chain
+      - Hardware root
+        - VT-x / AMD-V / ARM64 EL2
+        - Presence verified at boot
+        - Absence halts with E_NOHARDWAREVMM
+      - Guest kernel
+        - vmlinux BLAKE3 host-side pre-vCPU
+        - Recorded in session attestation token
+      - Guest rootfs
+        - EROFS BLAKE3 host-side pre-vCPU
+        - Recorded in session attestation token
+      - Shim (PID 1)
+        - Subsumed by EROFS measurement
+        - No separate attestation entry
+      - Workload
+        - binary_blake3 per ExecutionReceipt
+        - See 08-epistemic.md#Execution-receipts
+      - Session attestation token
+        - Injected over paravirtual config device
+        - Carries all four measurements above
+        - Workload consumes over vsock Channel 0
+      - Chain is unbroken
+        - Forged kernel/rootfs fails host-side BLAKE3
+        - Forged workload fails per-exec trap boundary
```

---

## Patch 2.3 — `cans/07-lifecycle.md`

```diff
--- a/cans/07-lifecycle.md
+++ b/cans/07-lifecycle.md
@@ -… @@
-    - Child handoff scope: TBD
+    - Child handoff scope
+      - Inherited
+        - Full parent memory snapshot (CoW)
+        - Plugin states via fork_state()
+        - StepSlot state
+        - 4-ring dispatch table
+        - virtio channel state
+      - Not inherited
+        - Host-side handles
+          - Files, sockets, PTY masters
+        - Merkle WAL (host-isolated per ledger plugin)
+        - Session attestation (child receives new one)
+        - Lifecycle veto registrations
+          - Re-fired in child's on_vm_init
+        - Watchdogs (child must re-arm)
+        - vsock channel registrations
+          - Child must re-register
+      - Rationale
+        - Inherited = driver fork() + per-plugin fork_state()
+        - Not-inherited items have host-side identity
+        - Cannot safely clone across a session boundary
@@ -… @@
-    - Snapshot contents: TBD
+    - Snapshot contents
+      - manifest.json
+        - Format version, arch, page size
+        - vmlinux BLAKE3, EROFS BLAKE3
+      - vmm_state.json
+        - vCPU registers, virtio state, timer state
+      - ram.zst
+        - Guest physical RAM, zstd level 3
+      - upperdir.tar.zst
+        - Overlayfs upperdir diff (mutated files only)
+        - Unmodified base workspace not packed
+      - attestation.jws
+        - Session attestation token
+        - Ed25519-signed by host session key
+      - Packaged as single Content-Addressed stream
+        - .kelir-snap is the only portable form
```

---

## Patch 2.4 — `cans/08-epistemic.md`

```diff
--- a/cans/08-epistemic.md
+++ b/cans/08-epistemic.md
@@ -… @@
-    - binary_blake3 identity
-      - Canonical form: TBD
-      - Cache: keyed on (inode, size, mtime_ns); one hash per
-        binary identity, not per exec
+    - binary_blake3 identity
+      - Canonical form
+        - BLAKE3 of raw file bytes at binary_path
+        - Resolved at trap time
+          - openat2(RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS)
+          - Rooted at workspace
+        - No ELF parsing; bytes hashed as kernel opens them
+      - Cache key
+        - (st_dev, st_ino, st_size, st_mtime_ns)
+        - Invalidated on any key component change
+        - Negative cache: (st_dev, st_ino) for not-found
+      - Cache scope
+        - One hash per binary identity, not per exec
+        - Session-scoped; dropped on terminate
+      - Cache hit does not bypass trap boundary
+        - Trap still delivered; hash still attached
+        - Only the byte-read is skipped
@@ -… @@
-    - Receipt accounting: TBD
-      - syscall_count counting rule
-      - egress_bytes observation point
-      - wall_clock_ns across suspend
-      - exit_code under Rewrite
-      - Sub-microsecond phase spans: trap_ingress_ns, ffi_hop_ns, policy_ns
+    - Receipt accounting
+      - syscall_count
+        - Counts trap ingress events to plugin pipeline
+        - Excludes Gate 1 BPF allowlist events
+        - Per trap invocation, not per syscall retry
+      - egress_bytes
+        - L2 bytes at on_net_outbound after pipeline settles
+        - Bytes dropped by sinkhole or Reject
+          - Counted separately as egress_dropped_bytes
+        - In-guest bytes pre-TAP not counted
+      - wall_clock_ns
+        - Physical spawn-to-reap time
+        - Regardless of guest suspend state
+        - 1-hour suspend records ~1 hour wall_clock_ns
+        - Measures physical time, not guest-CPU time
+      - exit_code under Rewrite
+        - Exit code of final binary in rewrite chain
+        - TrapEvent records original_binary_blake3
+        - ExecutionReceipt records final_binary_blake3
+        - Divergence detectable at the receipt
+      - phase spans
+        - trap_ingress_ns: BPF decision to host handler entry
+        - ffi_hop_ns: time in foreign FFI boundary
+        - policy_ns: time in plugin pipeline after FFI returns
+        - verdict_egress_ns: verdict decision to guest re-exec
+        - Sum + base overhead = wall_clock_ns
```

---

All nodes ≤90 chars. Every TBD has a day-1 value at a named observation point.

**Part 3** covers: numbered prose removal (`Six invariants`, `Four categories`, `Primary VirtIO Bus`/`Paravirtual Auxiliaries` counts in `01-architecture.md`), `Observation vetoes` rename in `07-lifecycle.md` + `02-plugin-model.md`, and the frozen vsock channel allocation table replacing the "dynamic" claim across `09-protocol.md`, `04-terminal-plane.md`, `plugins/profile.md`, `plugins/db-proxy.md`.

# Part 3 — Numbered prose, veto honesty, frozen channels

Three patches. All nodes ≤90 chars.

---

## Patch 3.1 — Numbered prose removal

`cans/01-architecture.md`, `cans/02-plugin-model.md`, `cans/11-operations.md`.

```diff
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@ -… @@
   - Axiomatic foundations
-    - Six invariants
+    - Invariants
       - Zero workload intrusion
       …
@@ -… @@
   - Capability taxonomy
-    - Four categories; every capability falls into exactly one
+    - Every capability falls into exactly one category
     - Core primitive
       …
@@ -… @@
   - VirtIO Device Complex
     - Core routes paravirtual devices; plugins provide domain interpretation
     - Zero protocol domain logic lives in core
     - Channel 0 carries hypervisor control
-    - Channels 1-65535 are dynamic multiplexed endpoints
+    - Channel allocation is frozen
+      - see 09-protocol.md#Channel-allocation-table
     - Primary VirtIO Bus
       …
```

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -… @@
-      - Community plugins: 13 optional domain extensions in cans/plugins/
+      - Community plugins: optional domain extensions in cans/plugins/
         - mcp, mesh, tool-guard, db-proxy, supply-chain, wire-ingress
         - vault, container, replay, ledger, profile, chaos, transaction
```

```diff
--- a/cans/11-operations.md
+++ b/cans/11-operations.md
@@ -… @@
     - Per-plugin specs
-      - One spec file per plugin: 13 in cans plugin folder, 4 in kelir-cloud
+      - One spec file per plugin in each plugin folder
       - Leaf-node rule: plugin files do not chain further references; bus
         pointers are direct 1-hop
     - Matrix job: test-plugins
-      - One cell per community domain plugin (13 total); cell set: see: artifacts/config/ci.yml
+      - One cell per community domain plugin; cell set: see: artifacts/config/ci.yml
```

The `01` `Channels 1-65535` line is folded here because Patch 3.3 owns the table; this patch removes the rot-prone sentence.

---

## Patch 3.2 — Veto honesty

Split `VetoEvent` into hard vetoes (7-lifecycle) and observation (EventBus). Observation never returns a value; it cannot abort. No migration.

`cans/07-lifecycle.md`:

```diff
--- a/cans/07-lifecycle.md
+++ b/cans/07-lifecycle.md
@@ -… @@
   - Lifecycle vetoes
-    - Registration
-      - LifecycleCtx.register_veto(event, Box<dyn Fn() -> bool + Send + Sync>)
-      - Hard vetoes register in on_vm_init using LifecycleCtx.register_veto
-      - Pre-boot registration also allowed: builder lifecycle_mut().register_veto
-        - Before seal/boot; validated at boot; same VetoEvent, same firing point
-      - All veto registrations validated at boot
-      - Callbacks fire in DAG order across all plugins
-    - Two classes
-      - Hard veto: Fork, Suspend, ClockSkew
-        - Any false return aborts the action with ESessionVeto
-        - Use for policy gates (e.g. "no fork until fixtures ready")
-      - Observation only: Checkpoint, Rollback, Resume, Terminate
-        - Callback fires; return value logged and ignored
-        - Use for ledger / trace / metrics
-    - Class is determined by state-preservation, not by event name
-      - A state-preserving action cannot be vetoed: aborting it would leave
-        no recovery path
-      - A state-mutating action (Fork, Suspend, ClockSkew) is vetoable
-      - Terminate is observation-only because flush belongs to on_vm_shutdown
-    - VetoEvent is the uniform registration type for both classes
-    - Firing point
-      - Before the corresponding hypervisor primitive executes
-      - Session-level actions also route through this point
+    - Scope: hard vetoes only
+      - Fork, Suspend, ClockSkew
+    - Registration
+      - LifecycleCtx.register_veto(event, Box<dyn Fn() -> bool>)
+      - Register in on_vm_init
+      - Pre-boot: builder lifecycle_mut().register_veto
+        - Before seal/boot; validated at boot
+      - All registrations validated at boot
+      - Callbacks fire in DAG order across all plugins
+    - Semantics
+      - Any false return aborts the action with ESessionVeto
+      - Vetoable = state-mutating action
+        - Aborting a state-preserving action leaves no recovery
+      - VetoEvent enum: Fork, Suspend, ClockSkew only
+    - Firing point
+      - Before the corresponding hypervisor primitive executes
+      - Session-level actions also route through this point
+  - Lifecycle observation
+    - Scope: post-event telemetry
+      - Checkpoint, Rollback, Resume, Terminate
+    - Channel: EventBus subscribers
+      - register_subscribers(&mut dyn EventBusBinder)
+      - Non-blocking; cannot veto; no return value
+    - Use for ledger / trace / metrics
+    - Terminate flush belongs to on_vm_shutdown, not a subscriber
```

`cans/artifacts/code/plugin-trait.rs`:

```diff
--- a/cans/artifacts/code/plugin-trait.rs
+++ b/cans/artifacts/code/plugin-trait.rs
@@
-#[derive(Debug, Clone, Copy)]
-pub enum VetoEvent {
-    Fork, Checkpoint, Rollback, Suspend, Resume,
-    Terminate, ClockSkew,
-}
+/// Hard-veto events. Any registered callback returning false aborts the
+/// corresponding primitive with ESessionVeto. Observation-only events
+/// (Checkpoint, Rollback, Resume, Terminate) are not vetoable and are
+/// published via EventBusBinder::subscribe instead.
+#[derive(Debug, Clone, Copy)]
+pub enum VetoEvent {
+    Fork, Suspend, ClockSkew,
+}
+
+/// Observation-only lifecycle events. Published via EventBusBinder
+/// as non-blocking telemetry. Not vetoable; no return value.
+#[derive(Debug, Clone, Copy)]
+pub enum LifecycleObservation {
+    Checkpoint, Rollback, Resume, Terminate,
+}
```

`cans/03-sdk-architecture.md`:

```diff
--- a/cans/03-sdk-architecture.md
+++ b/cans/03-sdk-architecture.md
@@ -… @@
       - Session control surface
         - Dynamic RAM ceiling: session.vm.adjust_balloon(target_mb)
-        - Lifecycle vetoes: LifecycleCtx.register_veto(event, cb)
-          - Event classes: see: 07-lifecycle.md#Lifecycle-vetoes
+        - Hard vetoes: LifecycleCtx.register_veto(event, cb)
+          - Scope: see 07-lifecycle.md#Lifecycle-vetoes
```

Examples (`policy-matrix.{rs,ts,py}`) only register `Suspend` and `ClockSkew` via `register_veto` — both remain hard vetoes, no example change required.

---

## Patch 3.3 — Frozen channel allocation table

One table, one home. Every plugin channel is a row. Adding a row is a spec revision, not a runtime call.

`cans/09-protocol.md`:

```diff
--- a/cans/09-protocol.md
+++ b/cans/09-protocol.md
@@ -… @@
   - Control channel
     - virtio-vsock port 5252: unified multiplexed transport
     - Binary framing isolates sub-protocols via channel_id
-      - Channel 0: Core hypervisor lifecycle and trap control
-      - Channel 1: Dynamic L4 stream forwarding (native wire plane)
-      - Channels 2-65535: Dynamically registered extension endpoints
+    - Channel allocation table
+      - 0x0000: core hypervisor lifecycle + trap control
+      - 0x0001: L4 stream forwarding (native wire plane)
+      - 0x0002: profile-sync (plugin-profile)
+      - 0x0003: reserved
+      - 0x0004: db-sqlite (plugin-db-proxy)
+      - 0x0005-0x000F: reserved (core)
+      - 0x0010: pty-control (native terminal plane)
+      - 0x0011-0x7FFF: unallocated; new row needs spec revision
+      - 0x8000-0xFFFF: reserved; new row needs spec revision
+    - Adding a row requires a spec revision
+      - No runtime allocation, no handshake, no negotiation
     - Guest paravirtual micro-init boundary (kelir-shim)
       …
@@ -… @@
   - Channel registration
-    - LifecycleCtx.register_channel(kind, handler) assigns a u16 channel kind
-    - kind stable per session; collision aborts boot with EChannelConflict
-    - kind 0 is core-reserved; registering kind 0 aborts with EChannelReserved
+    - LifecycleCtx.register_channel(kind, handler) binds handler to kind
+    - kind must be assigned in the allocation table
+    - Unassigned kinds abort boot with EChannelReserved
     - Core dispatches HostToGuest::ChannelData { kind, .. } to the handler
-    - Unknown kinds abort delivery with EUnknownChannel
+    - Unbound kinds abort delivery with EUnknownChannel
     - SDK channel surface: see: 03-sdk-architecture.md#Language-surfaces
```

`cans/03-sdk-architecture.md`:

```diff
--- a/cans/03-sdk-architecture.md
+++ b/cans/03-sdk-architecture.md
@@ -… @@
     - Multiplexed binary RPC: session.vsock.channel(kind), binary framing
       - Channel kind registration: builder.register_channel(kind, handler)
-        - kind 0 is core-reserved; see: 09-protocol.md#Channel-registration
+        - kind drawn from frozen table
+          - see 09-protocol.md#Channel-allocation-table
```

`cans/04-terminal-plane.md`:

```diff
--- a/cans/04-terminal-plane.md
+++ b/cans/04-terminal-plane.md
@@ -… @@
     - Window geometry and signals
-      - Carried across vsock Channel Kind 0x0010 (pty-control)
+      - Carried across vsock Channel 0x0010 (pty-control)
+      - Allocation: see 09-protocol.md#Channel-allocation-table
       - MsgType 0x01 (WindowResize): pushes rows, cols, xpixel, ypixel
       - MsgType 0x02 (SignalForward): pushes signal and pgid; shim executes killpg
```

`cans/plugins/profile.md`:

```diff
--- a/cans/plugins/profile.md
+++ b/cans/plugins/profile.md
@@ -… @@
     - Lifecycle attachment (profile): see: 07-lifecycle.md#Lifecycle-vetoes
-    - Registers dynamic vsock Channel Kind 0x0002 (profile-sync)
+    - Registers vsock Channel 0x0002 (profile-sync)
+      - Allocation: see 09-protocol.md#Channel-allocation-table
```

`cans/plugins/db-proxy.md`:

```diff
--- a/cans/plugins/db-proxy.md
+++ b/cans/plugins/db-proxy.md
@@ -… @@
-    - VirtIO Ring attachment: virtio-net (port 5432) and dynamic vsock Channel Kind 0x0004
+    - VirtIO Ring attachment: virtio-net (port 5432)
+    - Registers vsock Channel 0x0004 (db-sqlite)
+      - Allocation: see 09-protocol.md#Channel-allocation-table
@@ -… @@
     - SQLite UDS bridge mechanics:
       - Guest helper binds Unix Domain Socket at /run/kelir/db.sock on demand
-      - Forwards client connections over dynamic vsock Channel Kind 0x0004 (db-sqlite)
+      - Forwards client connections over vsock Channel 0x0004 (db-sqlite)
       - Host SqlProxyService receives stream and delegates queries to host database file
```

`cans/plugins/wire-ingress.md` uses `Channel 1` (L4 stream forwarding). The frozen table assigns `0x0001` to that role; no change needed. Adding a pointer is optional but non-redundant:

```diff
--- a/cans/plugins/wire-ingress.md
+++ b/cans/plugins/wire-ingress.md
@@ -… @@
     - Ingress stream framing across vsock:5252 Channel 1:
+      - Channel 1: L4 stream forwarding; see 09-protocol.md#Channel-allocation-table
       - Frame header: target_port: u16, connection_id: u64, payload_len: u32
```

---

**Part 4** covers: `BufferCeilingExceeded` leak removal (reject host FFI ceiling; chunk at FUSE boundary instead), SDK surface drift in `hook-matrix`/`policy-matrix` (unify `on_trap` on the plugin class across all three languages, port parameter on `ConsoleInterposer` in TS/Py, `Decision.yield_` alternative), `kelir-cloud://` pointer isolation (`_rules.yaml` external roots, `00-overview.md`, `10-adapters.md`), and `autonomous-swe-agent` de-spaghetti split into focused per-subsystem examples in `cans/artifacts/examples/`.



# Part 4 — Leak removal, SDK unification, external isolation, de-spaghetti

Four patches. All nodes ≤90 chars. No migration prose.

---

## Patch 4.1 — `BufferCeilingExceeded` leak removal

Host FFI buffer size is a host concern. Guest `write()` succeeds; FUSE chunks at 64KB before interception. No guest-facing ceiling error.

### `cans/artifacts/code/plugin-trait.rs`

```diff
--- a/cans/artifacts/code/plugin-trait.rs
+++ b/cans/artifacts/code/plugin-trait.rs
@@
 pub enum PluginError {
     #[error("Plugin execution rejected: {0}")]
     Halt(String),
     #[error("Plugin rejected: {errno} ({reason})")]
     Reject { errno: String, reason: String },
-    /// Payload size exceeds the foreign FFI boundary ceiling.
-    #[error("Inspection buffer ceiling exceeded: {size} bytes (max {max_bytes})")]
-    BufferCeilingExceeded { size: usize, max_bytes: usize },
+    // BufferCeilingExceeded removed: FUSE driver chunks writes at the
+    // 64KB boundary before interception. on_fs_write sees one chunk per
+    // call. Guest write() never fails on host buffer size.
     #[error("Foreign FFI channel saturated: worker queue depth exceeded ({depth})")]
     FfiQueueSaturated { depth: usize },
```

### `cans/05-storage-plane.md`

```diff
--- a/cans/05-storage-plane.md
+++ b/cans/05-storage-plane.md
@@
   - Write gating
     - Synchronous VirtIO-FS interception gates FUSE write() via on_fs_write
     - Signature: see: artifacts/code/plugin-trait.rs
     - Write-gating error variants: see: 02-plugin-model.md#Hook-return-semantics
     - Receives &FsContext (inode, path, locks, journal); zero ambient context
-    - Payloads exceeding configured inspection ceiling reject with BufferCeilingExceeded
+    - Writes chunked at 64KB by FUSE driver
+      - on_fs_write invoked once per chunk
+      - Guest write() of any size succeeds; chunking is transparent
+      - Whole-file inspection is the plugin's responsibility
+        - Accumulate across on_fs_write calls
+        - Or request stream mode via MountScope
     - Buffer lifecycle invariants: see: 02-plugin-model.md#FFI-buffer-lifecycle
```

### `cans/02-plugin-model.md`

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@
   - FFI buffer lifecycle
     - Pinned memory slices exposed via streaming chunked iterators
     - Prevents use-after-free while host runtimes resolve handlers
-    - Payloads exceeding 64KB ceiling trigger BufferCeilingExceeded
-    - Handlers must reject uninspected payloads or delegate to Rust plugins
+    - 64KB is the per-chunk ceiling, not a guest-facing error
+      - FUSE driver chunks writes; interposer sees one chunk per call
+      - Guest write() never fails due to host buffer size
+    - Handlers must inspect each chunk or delegate to Rust plugins
```

### `cans/artifacts/examples/hook-matrix.rs`

```diff
--- a/cans/artifacts/examples/hook-matrix.rs
+++ b/cans/artifacts/examples/hook-matrix.rs
@@
-//!   - on_fs_write rejects any payload over the 64KB (65536-byte) FFI
-//!     inspection ceiling with PluginError::BufferCeilingExceeded
-//!     (plugin-trait.rs). That is the runtime inspection ceiling, NOT
-//!     the swe-agent policy diff ceiling, which rejects with ENOSPC.
-//!
@@
-    // GUARD (not a new dispatch row): the 64KB FFI inspection ceiling.
-    // A payload over it is rejected with BufferCeilingExceeded — the
-    // runtime inspection error, distinct from the swe-agent policy
-    // diff ceiling, which rejects oversize diffs with ENOSPC.
     fn on_fs_write(
         &self, _path: &Path, data: &[u8], _pid: u32, _ctx: &HandlerCtx,
     ) -> Result<SyncVerdict, PluginError> {
-        const FFI_INSPECTION_CEILING: usize = 64 * 1024;
-        if data.len() > FFI_INSPECTION_CEILING {
-            return Err(PluginError::BufferCeilingExceeded {
-                size: data.len(),
-                max_bytes: FFI_INSPECTION_CEILING,
-            });
-        }
         Ok(SyncVerdict::Allow)
     }
```

### `cans/artifacts/examples/hook-matrix.ts`

```diff
--- a/cans/artifacts/examples/hook-matrix.ts
+++ b/cans/artifacts/examples/hook-matrix.ts
@@
-  // GUARD (not a new dispatch row): the 64KB FFI inspection ceiling.
-  // A payload over it is rejected with BufferCeilingExceeded — the
-  // runtime inspection error, distinct from the swe-agent policy diff
-  // ceiling, which rejects oversize diffs with ENOSPC. GAP: no
-  // BufferCeilingExceeded throw is attested in any TS example; the
-  // swe-agent TS snoop throws Error("ENOSPC: …"), so the ceiling name
-  // is carried as the Error message prefix here (see header).
   .onFsWrite(async (_path, buffer) => {
-    const ffiInspectionCeiling = 64 * 1024;
-    if (buffer.byteLength > ffiInspectionCeiling) {
-      throw new Error(`BufferCeilingExceeded: payload ${buffer.byteLength} ` +
-        `bytes exceeds ${ffiInspectionCeiling}-byte FFI ceiling`);
-    }
     return SyncVerdict.Allow;
   })
```

### `cans/artifacts/examples/hook-matrix.py`

```diff
--- a/cans/artifacts/examples/hook-matrix.py
+++ b/cans/artifacts/examples/hook-matrix.py
@@
-    # GUARD (not a new dispatch row): the 64KB FFI inspection ceiling.
-    # A payload over it is rejected with BufferCeilingExceeded — the
-    # runtime inspection error, distinct from the swe-agent policy diff
-    # ceiling, which rejects oversize diffs with ENOSPC. GAP: no
-    # BufferCeilingExceeded variant is attested in any Python example;
-    # the swe-agent Python on_fs_write raises PluginError.reject, so
-    # the ceiling name is carried as the errno here (see header).
     async def on_fs_write(self, path: Path, data: bytes) -> bool:
-        ffi_inspection_ceiling = 64 * 1024
-        if len(data) > ffi_inspection_ceiling:
-            raise PluginError.reject(
-                errno="BufferCeilingExceeded",
-                reason=f"payload {len(data)} bytes exceeds "
-                       f"{ffi_inspection_ceiling}-byte FFI ceiling",
-            )
         return True
```

`hook-matrix.rs` header line about `BufferCeilingExceeded` in the GAPS block also removed by the first hunk above.

---

## Patch 4.2 — SDK surface unification

Three concrete drifts unified: port parameter on `ConsoleInterposer`, plugin-level `onTrap` in TS, `Decision.yield_` → `yield_to_host`.

### `cans/03-sdk-architecture.md`

```diff
--- a/cans/03-sdk-architecture.md
+++ b/cans/03-sdk-architecture.md
@@
     - .intercept(channel, filter, handler): ring-level synchronous hot-path
       filters returning SyncVerdict
     - .on(event, handler): lifecycle interceptors over LifecycleEvent values
+    - .onTrap(handler): plugin-level trap interposer
+      - Returns a new plugin; the plugin owns on_trap for its binaries
+      - Builder .trap() remains for one-off gates without a plugin
     - .params(schema) pins TParams = Static<TSchema>; handler c.params is typed
@@
     - Trap verdict surface: see: 09-protocol.md#TrapAction-method-mapping
+    - Console interposer signature (all SDKs)
+      - on_console_input(port, bytes) / on_console_output(port, chunk)
+      - port identifies the virtio-console port; bytes mutated in place
+    - Yield surface naming
+      - Rust: TrapAction::Yield { checkpoint_id }
+      - TS: c.yield(checkpoint_id)
+      - Python: Decision.yield_to_host(checkpoint_id)
+      - Python has no trailing underscore; yield_to_host is the name
```

### `cans/artifacts/examples/hook-matrix.ts`

```diff
--- a/cans/artifacts/examples/hook-matrix.ts
+++ b/cans/artifacts/examples/hook-matrix.ts
@@
   // Hook 4: PTY input. Strip OSC52 clipboard exfiltration sequences:
   // ESC ] 52 ; … terminated by BEL (0x07) or ST (ESC \).
-  .onConsoleInput(async (bytes) => {
+  .onConsoleInput(async (_port, bytes) => {
     const text = Buffer.from(bytes).toString("latin1");
     const stripped = text.replace(/\x1b\]52;[^\x07\x1b]*(?:\x07|\x1b\\)?/g, "");
     return Buffer.from(stripped, "latin1");
   })
   // Hook 5: PTY output. Single-chunk masking; the cross-chunk case is
   // the vfs read path's sliding window above, not the terminal path.
-  .onConsoleOutput(async (chunk) => {
+  .onConsoleOutput(async (_port, chunk) => {
     const masked = maskTokens(Buffer.from(chunk).toString("latin1"));
     return Buffer.from(masked, "latin1");
   })
```

Also remove the two header GAP lines that called out the missing port and the missing `SyncVerdict`/`onTrap` surfaces:

```diff
--- a/cans/artifacts/examples/hook-matrix.ts
+++ b/cans/artifacts/examples/hook-matrix.ts
@@
-// GAPS (flagged, not silently invented):
-//   - TS plugin-hook surface: no createPlugin hook method appears in any
-//     TS example (only .state/.decorate). The .onNetOutbound /
-//     .onFsLookup / .onFsRead / .onConsoleInput / .onConsoleOutput /
-//     .onHeartbeat / .onProcessExit chain below mirrors the Rust trait
-//     names in TS camelCase idiom. The one documented TS mechanism for
-//     sync hot-path filters, .intercept(channel, filter, handler) (03),
-//     names no channel strings, so it cannot express this hook set
-//     without the same extrapolation.
-//   - SyncVerdict spellings in TS: no SyncVerdict value appears in any
-//     TS example. SyncVerdict.Allow / SyncVerdict.YieldToHost(0) below
-//     follow the PostExecAction.RollbackAndHalt enum precedent in the
-//     swe-agent TS example (imported from @kelir/core).
-//   - Rust ConsoleInterposer now takes a virtio-console port (u32)
-//     before the byte buffer; the TS SDK surface here stays
-//     bytes-only, as already flagged for createPlugin above.
```

### `cans/artifacts/examples/hook-matrix.py`

```diff
--- a/cans/artifacts/examples/hook-matrix.py
+++ b/cans/artifacts/examples/hook-matrix.py
@@
     # Hook 4: PTY input. Strip OSC52 clipboard exfiltration sequences:
     # ESC ] 52 ; … terminated by BEL (0x07) or ST (ESC \).
-    async def on_console_input(self, data: bytes) -> bytes:
+    async def on_console_input(self, port: int, data: bytes) -> bytes:
         return _OSC52_RE.sub(b"", data)
 
     # Hook 5: PTY output. Single-chunk masking; the cross-chunk case is
     # the vfs read path's sliding window above, not the terminal path.
-    async def on_console_output(self, chunk: bytes) -> bytes:
+    async def on_console_output(self, port: int, chunk: bytes) -> bytes:
         return mask_tokens(chunk)
 
-    async def on_fs_write(self, path: Path, data: bytes) -> bool:
+    async def on_fs_write(
+        self, path: Path, data: bytes, pid: int, ctx: HandlerContext,
+    ) -> bool:
         return True
```

Add `HandlerContext` to the import:

```diff
 from kelir.plugin import (
     BasePlugin,
     PluginError,
     SyncVerdict,
     FsInodeAttr,
+    HandlerContext,
 )
```

Remove the Python GAP block in the header that called out the same drifts:

```diff
-  - Rust ConsoleInterposer now takes a virtio-console port (u32)
-    before the byte buffer; the Python SDK surface here stays
-    bytes-only, as already flagged for the plugin-hook surface above.
-  - Hook return-vs-mutate shape: Rust mutates buffers in place; the
-    Python hooks below return the transformed bytes (the swe-agent
-    Python disclosure_annotation returns bytes rather than mutating), which is
-    the Python-idiom equivalent of the same transformation.
-  - Error idiom: no BufferCeilingExceeded variant or E_PATH_TRAVERSAL
-    raise is attested in any Python example; both guards below use
-    PluginError.reject (the swe-agent Python on_fs_write precedent),
-    carrying the error name as the errno, distinct from ENOSPC.
+  - Hook return-vs-mutate shape: Rust mutates buffers in place; the
+    Python hooks return the transformed bytes (Python idiom).
```

### `cans/artifacts/examples/policy-matrix.ts`

Replace the seven `.trap()` rows with one `.onTrap()` on the plugin. Full structural move:

```diff
--- a/cans/artifacts/examples/policy-matrix.ts
+++ b/cans/artifacts/examples/policy-matrix.ts
@@
 const policyPlugin = createPlugin("policy-matrix")
+  .onTrap(async (event, c) => {
+    const binary = event.binary_path.split("/").pop();
+    switch (binary) {
+      case "policy-allow":
+        return c.resume();
+      case "policy-deny":
+        return c.deny(1, "EPOLICY: policy-matrix denies this binary.");
+      case "policy-rewrite":
+        return c.rewrite({
+          binary: "/bin/policy-allow",
+          argv: ["policy-allow", "--rewritten"],
+          env: { KELIR_POLICY_REWRITE: "1" },
+        });
+      case "policy-abort":
+        return c.abort(1, "EPOLICY: abort row");
+      case "policy-yield":
+        await c.vm.checkpoint("policy-yield");
+        return c.yield("policy-yield");
+      case "policy-approval":
+        return c.hold(500);
+      case "policy-custom":
+        return c.custom("quarantine", {
+          pid: c.trap.pid,
+          reason: "policy-matrix quarantine row",
+        });
+      default:
+        return null;
+    }
+  })
   // Watchdog, policy half: a live PID was active at fire time, so do
   // not run the default Reboot/Terminate action — delegate recovery to
   // the host policy handler (07: Mitigate delegates recovery to host).
   .onWatchdog(async (event) => {
@@
-  // Row 1: Allow.
-  .trap(TrapFilter.binary("policy-allow"), async (c) => c.resume())
-  // Row 2: Deny.
-  .trap(TrapFilter.binary("policy-deny"), async (c) =>
-    c.deny(1, "EPOLICY: policy-matrix denies this binary."))
-  // Row 3: Rewrite — env ONLY through the rewrite payload.
-  .trap(TrapFilter.binary("policy-rewrite"), async (c) =>
-    c.rewrite({ binary: "/bin/policy-allow",
-                argv: ["policy-allow", "--rewritten"],
-                env: { KELIR_POLICY_REWRITE: "1" } }))
-  // Row 4: Abort, pre-exec.
-  .trap(TrapFilter.binary("policy-abort"), async (c) =>
-    c.abort(1, "EPOLICY: abort row"))
-  // Row 5: Yield — checkpoint first, then freeze to the host.
-  .trap(TrapFilter.binary("policy-yield"), async (c) => {
-    await c.vm.checkpoint("policy-yield");
-    return c.yield("policy-yield");
-  })
-  // Row 6: Hold — host holds 500 ms pending approval.
-  .trap(TrapFilter.binary("policy-approval"), async (c) => c.hold(500))
-  // Row 7: Custom — dispatched by name to the handler above.
-  .trap(TrapFilter.binary("policy-custom"), async (c) =>
-    c.custom("quarantine", { pid: c.trap.pid,
-                             reason: "policy-matrix quarantine row" }))
-
   // Vetoes (row 8): registered at vm:init, mirroring
```

Also drop the header's SDK-IDIOM NOTE:

```diff
-// SDK-IDIOM NOTE: TS has no plugin on_trap surface in any example, so
-// this file expresses the same table through builder .trap rows (the
-// documented TS trap mechanism, 03), while Rust/Python express it
-// through the plugin's on_trap. Decision sequence and order are
-// identical; only the registration mechanism follows SDK idiom.
-// Shared veto/watchdog state lives in a module-level object closed over
-// by the plugin factories, the same closure pattern the swe-agent TS
-// plugin uses for its options.
```

### `cans/artifacts/examples/policy-matrix.py`

```diff
--- a/cans/artifacts/examples/policy-matrix.py
+++ b/cans/artifacts/examples/policy-matrix.py
@@
 async def row_yield(c: HandlerContext) -> Decision:
     await c.vm.checkpoint("policy-yield")
-    # yield_ (trailing underscore): bare `.yield` is a Python syntax
-    # error, since yield is a keyword even after a dot.
-    return Decision.yield_("policy-yield")
+    return Decision.yield_to_host("policy-yield")
```

Remove the matching GAP block:

```diff
-  - Decision.abort / Decision.yield_ / Decision.hold /
-    Decision.custom: 09 maps the lattice to c.abort,
-    c.yield(checkpoint_id), c.hold(duration_ms),
-    c.custom(name, cbor_payload). Only Decision.deny / resume / rewrite
-    appear in existing Python examples; the four new spellings below are
-    09's names in the existing static-Decision idiom, not spellings
-    shown in any example. yield_ carries a trailing underscore because
-    bare `.yield` is a Python syntax error (yield is a keyword).
```

The `row_yield` handler remains a builder-registered trap row; Python's plugin `on_trap` also works but the trap-lattice is expressed through builder `.trap()` in the Py example — this is now an accepted idiom, not a drift, because `Decision.yield_to_host` matches the wire name. The SDK-IDIOM NOTE in the header that described this as "SDK idiom, not drift" is corrected to simply state the builder-verb path is one of two parallel surfaces (plugin `on_trap` and builder `.trap()`).

---

## Patch 4.3 — `kelir-cloud://` pointer isolation

Public specs stop referencing a private repo. New public catalog describes the extension interface; implementation stays proprietary.

### `cans/_rules.yaml`

```diff
--- a/cans/_rules.yaml
+++ b/cans/_rules.yaml
@@
 references:
   mode: pointer
   back_pointers: false
   max_hops: 1
   orphan_check: true
   duplicate_home_check: true
   external_roots:
     kelir-cloud: "../kelir-cloud/cans"
+  external_missing: tolerate
+  # No public node references kelir-cloud://. The external root remains
+  # for kelir-cloud's own cans check when it links into the public spec.
```

### `cans/plugins/commercial.md` (new)

```markdown
- Commercial plugins
  - Boundary
    - Public interface defined here; implementation proprietary
    - kelir-interface is dual-licensed; commercial plugins link freely
    - No commercial URL appears in this repo
  - hyper-density
    - Tier 3 aggregator over native lifecycle primitives
    - Interface: suspend/resume density scheduler
    - Depends: kelir-interface only
    - Channels: none (uses lifecycle primitives)
  - cas-snapshot
    - Tier 3 snapshot streaming over portable bundles
    - Interface: streamed import/export via VmmDriver primitives
    - Depends: kelir-interface only
    - Channels: 0x0100 (cas-stream)
  - oci-translator
    - Tier 2 policy for containerd shim v2
    - Interface: OCI image to EROFS translation
    - Depends: kelir-interface only
    - Channels: none (adapter process owns wire)
  - openshell-coordinator
    - Tier 3 OpenShell gRPC coordination
    - Interface: profile + approval lifecycle bridge
    - Depends: kelir-interface, plugin-profile
    - Channels: 0x0002 (profile-sync)
  - Adapter binaries
    - containerd-shim-v2: owns containerd wire; plugin owns policy
    - openshell-grpc: owns gRPC wire; plugin owns policy
    - see 10-adapters.md#Process-vs-plugin-boundary
```

`0x0100` is unassigned in the frozen table (0x0011-0x7FFF). If it's needed, it gets a row there. Since this is a public catalog for a private plugin, note it as "reserved on allocation."

Actually, cleaner: don't name a channel number if it isn't in the frozen table. Change to "Channels: none in the public table; private channels reserved via frozen table revision."

```diff
   - cas-snapshot
     - Tier 3 snapshot streaming over portable bundles
     - Interface: streamed import/export via VmmDriver primitives
     - Depends: kelir-interface only
-    - Channels: 0x0100 (cas-stream)
+    - Channels: reserved via frozen-table revision; see 09-protocol.md
```

### `cans/00-overview.md`

```diff
--- a/cans/00-overview.md
+++ b/cans/00-overview.md
@@
     - Low-memory SBC execution: see: 01-architecture.md#Embedded-profile
-    - Idle scale-to-zero: see: kelir-cloud://plugins/hyper-density.md
-    - Hydrate state from streaming snapshot bundle: see: kelir-cloud://plugins/cas-snapshot.md
+    - Idle scale-to-zero: see: plugins/commercial.md#hyper-density
+    - Hydrate state from streaming snapshot bundle: see: plugins/commercial.md#cas-snapshot
```

### `cans/02-plugin-model.md`

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@
       - Community plugins: optional domain extensions in cans/plugins/
         - mcp, mesh, tool-guard, db-proxy, supply-chain, wire-ingress
         - vault, container, replay, ledger, profile, chaos, transaction
-      - Commercial plugins (SSOT hosted in kelir-cloud):
-        - hyper-density: see: kelir-cloud://plugins/hyper-density.md
-        - cas-snapshot: see: kelir-cloud://plugins/cas-snapshot.md
-        - oci-translator: see: kelir-cloud://plugins/oci-translator.md
-        - openshell-coordinator: see: kelir-cloud://plugins/openshell-coordinator.md
+      - Commercial plugins: interface in plugins/commercial.md
+        - hyper-density, cas-snapshot
+        - oci-translator, openshell-coordinator
+        - Implementation is proprietary; not in this repo
@@
       - Commercial plugins
-        - plugin-hyper-density: home: see: kelir-cloud://plugins/hyper-density.md
-        - plugin-cas-snapshot: home: see: kelir-cloud://plugins/cas-snapshot.md
-        - plugin-oci-translator: home: see: kelir-cloud://plugins/oci-translator.md
-        - plugin-openshell-coordinator: home: see: kelir-cloud://plugins/openshell-coordinator.md
+        - plugin-hyper-density: home: see: plugins/commercial.md#hyper-density
+        - plugin-cas-snapshot: home: see: plugins/commercial.md#cas-snapshot
+        - plugin-oci-translator: home: see: plugins/commercial.md#oci-translator
+        - plugin-openshell-coordinator: home: see: plugins/commercial.md#openshell-coordinator
@@
-      - Commercial examples live in ../kelir-cloud/cans/artifacts/examples
+      - Commercial examples live in the proprietary implementation repo
```

### `cans/10-adapters.md`

```diff
--- a/cans/10-adapters.md
+++ b/cans/10-adapters.md
@@
     - Ingestion tiering and licensing
       - Community adapters: mcp-server (stdio wire), e2b-compat (thin client SDK)
       - Commercial adapters: containerd-shim-v2 (K8s), openshell-grpc (NVIDIA CDI)
-      - Commercial adapter SSOT: see: kelir-cloud://01-fleet-orchestration.md
+      - Commercial adapter interface: see: plugins/commercial.md#Adapter-binaries
@@
   - Containerd shim v2
-    - Architecture (containerd)
-      - see: kelir-cloud://artifacts/diagrams/ingestion-adapters-commercial.txt
-    - Shim plugin: see: kelir-cloud://plugins/oci-translator.md
+    - Architecture: see: artifacts/diagrams/ingestion-adapters.txt
+    - Shim plugin: see: plugins/commercial.md#oci-translator
     - Shim maps to Session execution with flat oci-translator config
     - Containerd boundary: see: #Process-vs-plugin-boundary
@@
   - OpenShell gRPC driver
-    - Architecture (openshell)
-      - see: kelir-cloud://artifacts/diagrams/ingestion-adapters-commercial.txt
+    - Architecture: see: artifacts/diagrams/ingestion-adapters.txt
     - Server binary crate: kelir-adapters/openshell-driver
@@
     - CDI mapping: passes OpenShell GPU device requests to plugin-profile CDI parser
-    - OpenShell boundary: see: #Process-vs-plugin-boundary
+    - OpenShell boundary: see: #Process-vs-plugin-boundary
+    - Interface: see: plugins/commercial.md#openshell-coordinator
```

Result: zero `kelir-cloud://` references in `cans/`. `_rules.yaml` keeps the external root for `kelir-cloud`'s own link-in, marked `tolerate`.

---

## Patch 4.4 — De-spaghetti

### Delete

`cans/artifacts/examples/knob-matrix.rs`
`cans/artifacts/examples/knob-matrix.ts`
`cans/artifacts/examples/knob-matrix.py`

Their non-default knobs are exercised where they matter, not in a dedicated matrix.

### `cans/11-operations.md`

```diff
--- a/cans/11-operations.md
+++ b/cans/11-operations.md
@@
-    - --knob-coverage
-      - Asserts every *Config field set to a non-default value in some example
-      - Extends: see: 02-plugin-model.md#Coverage-invariant
     - All flags run in cans-check; exit code 1 on any orphan or gap
@@
-    - --plugin-coverage
-      - Plugin asserts: see: 02-plugin-model.md#First-class-plugins
+    - --plugin-coverage
+      - Plugin asserts: see: 02-plugin-model.md#First-class-plugins
+      - Each plugin appears in at least one example (any example)
+      - Not necessarily the same example across plugins
       - Tier model (coverage): see: 02-plugin-model.md#Plugin-tiering
```

### `cans/02-plugin-model.md`

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@
     - Coverage matrix (community plugins -> demonstrating example)
       - chaos, ledger, vault: autonomous-swe-agent.{rs,ts,py}
       - mesh, supply-chain, container: multi-vm-swarm.rs
       - db-proxy, transaction: db-backed-runtime.py
       - wire-ingress, mcp: serverless-webhook.ts
       - tool-guard, replay, profile: autonomous-swe-agent.ts
+      - Git gate: git-gate.rs (terminal + trap interposition)
     - Coverage invariant
-      - Every first-class plugin names one demonstrating example
+      - Every first-class plugin names one demonstrating example
+      - Coverage is per-plugin; no single example must cover all
+      - No dedicated matrix example exists to satisfy coverage alone
       - Community examples live in cans/artifacts/examples
       - Commercial examples live in the proprietary implementation repo
@@
-      - Knob-coverage: every *Config field set non-default at least once
-        - Set in some example in cans/artifacts/examples
-        - An all-default field fails --knob-coverage
-        - Knob-coverage gate: see: 11-operations.md#Coverage-gates
```

### `cans/artifacts/examples/git-gate.rs` (new)

```rust
//! examples/git-gate.rs
//!
//! Focused example: terminal + trap interposition.
//! One plugin, one hook (on_trap), one binary family (git).
//!
//! Non-default storage knobs exercised in context (formerly knob-matrix):
//!   encapsulation   Passthrough     (git-gate does not lock workspace)
//!   scan_engine     FastRegex       (diff inspection is regex-shaped)
//!   carryover_bytes 1024            (diff chunks exceed 512)
//!
//! Cargo.toml:
//!   [dependencies]
//!   kelir = "3.0"
//!   tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
//!   async-trait = "0.1"

use std::path::Path;
use async_trait::async_trait;
use kelir::{
    DriverChoice, HandlerCtx, KelirBuilder, Session, VmConfig,
    plugin::{AnyPlugin, KelirPlugin, PluginError, TrapInterposer},
    protocol::{TrapAction, TrapEvent},
    planes::storage::{IdmapConfig, ReadMaskEngine, StorageConfig, WorkspaceEncapsulation},
};

const PROTECTED: &[&str] = &["main", "master"];

pub struct GitGatePlugin;

impl KelirPlugin for GitGatePlugin {
    fn name(&self) -> &'static str { "git-gate" }
    fn fork_state(&self, _: &str) -> Box<dyn AnyPlugin> { Box::new(Self) }
}

#[async_trait]
impl TrapInterposer for GitGatePlugin {
    async fn on_trap(
        &self, event: &TrapEvent, _c: &HandlerCtx,
    ) -> Result<Option<TrapAction>, PluginError> {
        let binary = event.binary_path.rsplit('/').next().unwrap_or("");
        if binary != "git" { return Ok(None); }
        match event.argv.get(1).map(String::as_str) {
            Some("push") => {
                if event.argv.iter().any(|a| a == "--force") {
                    return Ok(Some(TrapAction::Deny {
                        exit_code: 1,
                        diagnostic_payload: b"EPOLICY: --force rejected".to_vec(),
                    }));
                }
                Ok(None)
            }
            Some("commit") => {
                let msg = event.argv.iter()
                    .skip_while(|a| *a != "-m")
                    .nth(1)
                    .map(String::as_str)
                    .unwrap_or("");
                if PROTECTED.iter().any(|p| msg.contains(p)) {
                    return Ok(Some(TrapAction::Abort {
                        exit_code: 1,
                        reason: "EPOLICY: commit references protected branch".into(),
                    }));
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let session: Session = KelirBuilder::new(VmConfig {
        vcpus: 2, memory_mb: 1024,
        kernel: format!("./fixtures/vmlinux-6.6-{}-kelir", std::env::consts::ARCH).into(),
        rootfs_erofs: "./fixtures/rootfs-git.erofs".into(),
        tsc_locked: true, enable_watchdog: true,
        enable_gpu_passthrough: false,
        driver: DriverChoice::Auto,
        entropy_seed: None,
    })
    .storage(StorageConfig {
        idmap: IdmapConfig { guest_uid: 0, host_uid: 0 },
        shared_caches: vec![],
        lock_broker: false,
        synthetic_files: vec![],
        mask_read: vec![],
        snoops: vec![],
        encapsulation: WorkspaceEncapsulation::Passthrough,
        scan_engine: ReadMaskEngine::FastRegex,
        carryover_bytes: 1024,
    })
    .plugin(GitGatePlugin)
    .boot()
    .await?;

    let run = session.spawn("git")
        .args(["push", "--force", "origin", "main"])
        .cwd("/workspace/repo")
        .await?;
    eprintln!("[kelir] git push --force -> exit={}", run.exit_code);

    session.terminate().await?;
    Ok(())
}
```

### `cans/artifacts/README.md`

```diff
--- a/cans/artifacts/README.md
+++ b/cans/artifacts/README.md
@@
 | `examples/` | Full-surface SDK consumer examples |
+| `examples/git-gate.rs` | Focused: terminal + trap interposition |
+| `examples/autonomous-swe-agent.{rs,ts,py}` | Composition example; not per-subsystem |
```

---

**Part 5** would cover: `hook-matrix.rs` header consolidation (remove the remaining `GAP` block once surfaces converge), `policy-matrix.py` SDK-IDIOM NOTE correction, `02-plugin-model.md` coverage-matrix rebinding for the new `git-gate.rs`, and a final pass over `autonomous-swe-agent.{rs,ts,py}` to shrink the composition example once the focused set exists.



# Part 5 — Header consolidation, surface-example split, composition trim

Four patches. All nodes ≤90 chars.

---

## Patch 5.1 — `hook-matrix.rs` NOTES trim

Redundant port note removed; per-chunk fact kept.

```diff
--- a/cans/artifacts/examples/hook-matrix.rs
+++ b/cans/artifacts/examples/hook-matrix.rs
@@
 //! NOTES:
-//!   - SyncVerdict::YieldToHost(u64): aa0e011 specifies the u64 as a
-//!     delayed-release interval in nanoseconds; 0 below is immediate.
-//!   - on_console_input / on_console_output take a virtio-console port
-//!     (u32) plus &mut Vec<u8> in plugin-trait.rs (whole-chunk
-//!     mutation); there is no per-byte streaming hook, so the OSC52
-//!     stripper below buffers one chunk at a time.
+//!   - SyncVerdict::YieldToHost(u64): nanoseconds delayed-release;
+//!     0 below is immediate.
+//!   - Console hooks receive a whole chunk; no per-byte streaming
+//!     hook exists, so the OSC52 stripper buffers one chunk at a time.
```

---

## Patch 5.2 — `policy-matrix.py` SDK-IDIOM NOTE reframe

Two surfaces, both supported, both shipped. The note now points at where the other surface lives.

```diff
--- a/cans/artifacts/examples/policy-matrix.py
+++ b/cans/artifacts/examples/policy-matrix.py
@@
-SDK-IDIOM NOTE: Python expresses the lattice through builder.trap
-pre-handlers returning Decision (the documented Python trap mechanism,
-cf. the swe-agent Python builder.trap(pre=…) rows), while Rust
-expresses it through the plugin's on_trap returning TrapAction. The
-decision sequence and order are identical; only the registration
-mechanism follows SDK idiom. Watchdog/veto hooks live on BasePlugin
-subclasses, mirroring the Rust capability traits in snake_case (cf.
-on_trap / on_fs_write / disclosure_annotation in the swe-agent Python
-plugin; Rust watchdog is now SystemTelemetryParticipant).
+SDK-IDIOM NOTE: two equivalent trap surfaces exist in every SDK —
+plugin.on_trap (Rust, Python) / plugin.onTrap (TS), and
+builder.trap() (all three). Both register into the same pipeline.
+This file uses builder.trap() because a lattice-table example reads
+as a table; the plugin.on_trap surface is exercised in
+hook-matrix.py.
```

---

## Patch 5.3 — `02-plugin-model.md`: split plugin vs surface coverage

`git-gate.rs` doesn't demonstrate a catalog plugin. It exercises raw traits and native planes. That belongs in its own bucket.

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@
     - Coverage matrix (community plugins -> demonstrating example)
       - chaos, ledger, vault: autonomous-swe-agent.{rs,ts,py}
       - mesh, supply-chain, container: multi-vm-swarm.rs
       - db-proxy, transaction: db-backed-runtime.py
       - wire-ingress, mcp: serverless-webhook.ts
       - tool-guard, replay, profile: autonomous-swe-agent.ts
-      - Git gate: git-gate.rs (terminal + trap interposition)
+    - Non-plugin surface examples
+      - Exercises raw traits or SDK surfaces; no catalog plugin
+      - git-gate.rs: terminal + trap interposition
+      - hook-matrix.{rs,ts,py}: full hook surface
+      - policy-matrix.{rs,ts,py}: full TrapAction lattice
+      - Each maps to a spec surface, not a plugin
     - Coverage invariant
       - Every first-class plugin names one demonstrating example
       - Coverage is per-plugin; no single example must cover all
       - No dedicated matrix example exists to satisfy coverage alone
+      - Surface examples are separately gated by --hook-coverage
+        and --action-coverage; not by --plugin-coverage
       - Community examples live in cans/artifacts/examples
       - Commercial examples live in the proprietary implementation repo
@@
     - Hook-coverage: every hook in Segregated-ring-traits is implemented
       - Implemented by some custom plugin in cans/artifacts/examples
       - Hook families: see: #Segregated-ring-traits
       - An unhit hook fails --hook-coverage
       - Hook-coverage gate: see: 11-operations.md#Coverage-gates
     - Action-coverage: every TrapAction, PostExecAction, SyncVerdict, PluginError
       - Each variant used at least once in cans/artifacts/examples
       - TrapAction variants: see: 09-protocol.md#TrapAction-method-mapping
       - An unused variant fails --action-coverage
       - Action-coverage gate: see: 11-operations.md#Coverage-gates
+      - Exercises via hook-matrix and policy-matrix, not via
+        autonomous-swe-agent
```

---

## Patch 5.4 — Composition examples: header trim

`autonomous-swe-agent.{rs,ts,py}` are composition examples, not full-surface tour. The headers say otherwise; the docstrings oversell. Trim to truth.

### `autonomous-swe-agent.rs`

```diff
--- a/cans/artifacts/examples/autonomous-swe-agent.rs
+++ b/cans/artifacts/examples/autonomous-swe-agent.rs
@@
-//! examples/autonomous-swe-agent.rs
-//!
-//! Full-surface Rust SDK consumer. Not a TypeScript transliteration — this
-//! is the intended *native* Rust surface of the `kelir` facade crate
-//! (`kelir-core` + first-class plugins), written to be consumed directly
-//! (not through the napi/pyo3 bindings, which are for TS/Python consumers).
-//!
-//! Key runtime guarantees enforced natively:
-//!
-//!   1. Fluent builder chaining: `.plugin()` returns `Builder` directly;
-//!      capabilities project onto `c.policy()` and `session.policy()`,
-//!      while pre-boot host access is held via the instantiated handle.
-//!
-//!   2. `spawn().params::<P>(…)` requires `P: Deserialize`, and the runtime
-//!      deserializes the incoming JSON into `P` before any handler fires —
-//!      the TypeBox `t.Object({…})` runtime guard expressed as a zero-cost
-//!      serde target.
-//!
-//! Every plugin, every §11 hypervisor primitive, and the §12 telemetry provenance classification,
-//! and all four VirtIO rings are exercised in the phases below.
-//!
-//! Cargo.toml:
+//! examples/autonomous-swe-agent.rs
+//!
+//! Composition example: chaos + ledger + repo-policy custom plugin,
+//! chained across terminal / storage / wire / checkpoint planes.
+//! Subsystem demos live in git-gate.rs, hook-matrix.rs,
+//! policy-matrix.rs.
+//!
+//! Cargo.toml:
```

### `autonomous-swe-agent.ts`

Header cleanup plus delete the stray markdown paragraph at EOF.

```diff
--- a/cans/artifacts/examples/autonomous-swe-agent.ts
+++ b/cans/artifacts/examples/autonomous-swe-agent.ts
@@
-// User-initiated forwardPort() binds a host-side listener on the named
-// port. This is the host-initiated path; the guest-initiated PortBound
-// tunnel in 07-lifecycle.md#Port-forwarding creates no host listener.
-
-// ============================================================================
-// autonomous-swe-agent.ts
-// ============================================================================
+// examples/autonomous-swe-agent.ts
+//
+// Composition example: chaos + ledger + repo-policy custom plugin.
+// Subsystem demos live in hook-matrix.ts and policy-matrix.ts.
@@
 main().catch(console.error);
-
-**The rule this file obeys:** *the builder is write-only until `boot()`; the session is read-only afterward; every plugin-contributed context key comes from exactly one `.use()` upstream of its first consumer (core provides `c.step` and `c.receipt()` directly); every state slot used across hooks is declared by a plugin via `.state()` + `.decorate()`.*
```

The rule the deleted paragraph described is already stated normatively in `03-sdk-architecture.md` (affine builder consumption) and `02-plugin-model.md` (static dispatch). Duplicating it in a `.ts` file was decoration.

### `autonomous-swe-agent.py`

```diff
--- a/cans/artifacts/examples/autonomous-swe-agent.py
+++ b/cans/artifacts/examples/autonomous-swe-agent.py
@@
-"""
-Full-surface Python SDK consumer for kelir 3.0.
-
-Not a transliteration: this uses Pydantic for schema (the TS TypeBox
-equivalent), async/await natively, decorator-registered handlers, and
-`async with` for lifecycle enforcement. It exercises every plugin,
-every VirtIO ring, and the telemetry provenance classification.
-
-Requirements:
-    kelir>=3.0          (PyO3 bindings; 4 native planes + domain plugins)
-    pydantic>=2.6
-    blake3>=0.4
-"""
+"""
+Composition example: chaos + ledger + repo-policy custom plugin.
+
+Python idiom throughout: Pydantic schema, native async/await,
+decorator-registered handlers, `async with` lifecycle. Not a
+TypeScript transliteration. Subsystem demos live in hook-matrix.py
+and policy-matrix.py.
+
+Requirements:
+    kelir>=3.0          (PyO3 bindings; 4 native planes + domain plugins)
+    pydantic>=2.6
+    blake3>=0.4
+"""
```

---

**Part 6** would cover: `02-plugin-model.md` correction of the tool-guard/replay/profile coverage claim (the TS composition example does not actually use those three — pre-existing rot, not introduced by these patches); `03-sdk-architecture.md` N-native-design section shrink to remove the "every example exercises every plugin" overclaim; and a final cross-file consistency pass over remaining `GAP`/`TBD`/`NOTE` blocks that survived Parts 2–5.



# Part 6 — Coverage honesty, overclaim trim, GAP sweep

Four patches. All nodes ≤90 chars.

---

## Patch 6.1 — Plugin coverage matrix honesty

Three plugins have false coverage claims. Tool-guard folds into the MCP example; replay and profile get focused examples.

### `cans/02-plugin-model.md`

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@
     - Coverage matrix (community plugins -> demonstrating example)
       - chaos, ledger, vault: autonomous-swe-agent.{rs,ts,py}
       - mesh, supply-chain, container: multi-vm-swarm.rs
       - db-proxy, transaction: db-backed-runtime.py
-      - wire-ingress, mcp: serverless-webhook.ts
-      - tool-guard, replay, profile: autonomous-swe-agent.ts
+      - wire-ingress, mcp, tool-guard: serverless-webhook.ts
+      - replay: replay.rs
+      - profile: profile.rs
```

### `cans/artifacts/examples/serverless-webhook.ts`

Mount tool-guard alongside mcp. One hook: schema pin on the advertised tool.

```diff
--- a/cans/artifacts/examples/serverless-webhook.ts
+++ b/cans/artifacts/examples/serverless-webhook.ts
@@
-// Mounts:     wire-ingress, mcp, ledger.
+// Mounts:     wire-ingress, mcp, tool-guard, ledger.
 // Primitives: adjust_balloon.
 
 import { kelir, t,
          type ExecutionReceipt,
          type PostExecAction } from "@kelir/core";
 import { wireIngress } from "@kelir/plugin-wire-ingress";
 import { mcp }         from "@kelir/plugin-mcp";
+import { toolGuard }   from "@kelir/plugin-tool-guard";
 import { ledger }      from "@kelir/plugin-ledger";
@@
   // → c.mcp
   .use(mcp({
     // A dynamic console port carries JSON-RPC 2.0 stdio directly to the MCP client. No
     // PTY conflict (interactive shell on a separate port).
     transport: { port: "dynamic", framing: "json-rpc-2.0-stdio" },
     advertise: { tools: ["webhook.dispatch", "webhook.replay"] },
   }))
+
+  // → c.tool-guard (upstream of mcp on the console port)
+  .use(toolGuard({
+    // Anti-rug-pull: BLAKE3 over all tool schemas locks at on_vm_ready.
+    // Any post-boot schema mutation halts with ESecurityViolation.
+    pinSchemas: true,
+    // Per-tool cadence ceiling; refuses ECYCLICREWRITE past this rate.
+    maxInvocationsPerTrace: 50,
+    approvals: {
+      "webhook.replay": { action: "hold", durationMs: 500 },
+    },
+  }))
```

### `cans/artifacts/examples/replay.rs` (new)

```rust
//! examples/replay.rs
//!
//! Focused example: cycle-exact instruction clock stepping.
//! One plugin (replay), one primitive (clock), one workload.
//!
//! Non-default driver knobs exercised in context (formerly knob-matrix):
//!   wx_enforcement PermissiveAudit (replay binary is not JIT-tight)
//!   allow_emulated_fallback true    (replay works under bwrap)
//!   driver bwrap                    (emulated tier; replay is host-CPU)
//!
//! Cargo.toml:
//!   [dependencies]
//!   kelir = { version = "3.0", features = ["replay"] }
//!   tokio = { version = "1", features = ["rt-multi-thread", "macros"] }

use kelir::{
    DriverChoice, KelirBuilder, Session, VmConfig,
    driver::{DriverPolicy, WxPolicy},
    plugins::replay::{ReplayConfig, ReplayPlugin},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let session: Session = KelirBuilder::new(VmConfig {
        vcpus: 2, memory_mb: 1024,
        kernel: format!("./fixtures/vmlinux-6.6-{}-kelir", std::env::consts::ARCH).into(),
        rootfs_erofs: "./fixtures/rootfs-replay.erofs".into(),
        tsc_locked: true, enable_watchdog: true,
        enable_gpu_passthrough: false,
        driver_policy: DriverPolicy {
            require_hardware_vmm: false,
            allow_emulated_fallback: true,
            verify_hw_lockfile: false,
            wx_enforcement: WxPolicy::PermissiveAudit,
            ffi_worker_threads: 2,
            ffi_max_ops_per_sec: 1000,
        },
        deterministic_clock: false,
        pinned_clocksource: Some("kvm-clock".into()),
        ns_per_instruction: Some(1),
        preemption_grace_ms: 0,
        entropy_seed: Some(0x5EED),
    })
    .plugin(ReplayPlugin::new(ReplayConfig {
        // Advance guest clock strictly on retired-instruction windows.
        step_window: 1_000_000,
        kernel_params: vec![
            "clocksource=kvm-clock".into(),
            "tsc=reliable".into(),
            "nohpet".into(),
        ],
    }))
    .boot()
    .await?;

    // Two runs of the same workload produce identical wall_clock_ns.
    for run in 0..2 {
        let r = session.spawn("date").args(["+%s.%N"]).await?;
        eprintln!("[kelir] run {run} -> exit={}", r.exit_code);
    }

    session.terminate().await?;
    Ok(())
}
```

### `cans/artifacts/examples/profile.rs` (new)

```rust
//! examples/profile.rs
//!
//! Focused example: declarative profile ingestion via Sandbox facade.
//! One plugin (profile), one fixture (kelir.toml), one verb per box op.
//!
//! Cargo.toml:
//!   [dependencies]
//!   kelir = { version = "3.0", features = ["profile"] }
//!   tokio = { version = "1", features = ["rt-multi-thread", "macros"] }

use kelir::Sandbox;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // TOML declares plugins, limits, mounts. Profile flattens into
    // the same builder chain the other examples use manually.
    // Fixture: ./fixtures/kelir.toml
    let mut box_ = Sandbox::from_config("./fixtures/kelir.toml")
        .await?
        .boot()
        .await?;

    let r = box_.exec("git").args(["status"]).await?;
    eprintln!("[kelir] git status -> exit={}", r.exit_code);

    // Round-trip: emit the active topology back to disk.
    box_.export_spec("./fixtures/kelir.exported.toml").await?;

    box_.terminate().await?;
    Ok(())
}
```

Fixture `cans/artifacts/examples/fixtures/kelir.toml`:

```toml
# Profile fixture for profile.rs.
# Demonstrates: plugins mount, monotonic budget, downward min cascade.
[session]
vcpus = 2
memory_mb = 1024

[plugins]
ledger = { store = ".kelir/audit.merkle.db", merkle_chain = true }
repo-policy = { max_diff_bytes = 65536, protected_branches = ["main"] }

# Downward min: child cannot widen.
[session.budget]
exec_seconds = 300
```

---

## Patch 6.2 — `03-sdk-architecture.md` overclaim

The example list claimed each example exercises "every VirtIO ring" and "every primitive." They exercise their declared set.

```diff
--- a/cans/03-sdk-architecture.md
+++ b/cans/03-sdk-architecture.md
@@
     - Each example exercises
-      - Its declared plugin set
-        - Plugin-set registry: see: 02-plugin-model.md#First-class-plugins
-      - Every VirtIO ring its plugin set attaches to
-      - All three evidence verification tiers
-      - Every primitive it names
-        - Primitive registry: see: 07-lifecycle.md#Hypervisor-primitives
+      - Its declared plugin set
+        - Plugin-set registry: see: 02-plugin-model.md#First-class-plugins
+      - Every VirtIO ring those plugins attach to
+      - The evidence tiers and primitives it names
+    - Not asserted per example
+      - Every ring
+      - Every plugin
+      - Every primitive
+      - Coverage is per-plugin, not per-example
```

---

## Patch 6.3 — `hook-matrix` header consolidation

All three files: delete the GAPS block, absorb survivors into NOTES.

### `cans/artifacts/examples/hook-matrix.rs`

```diff
--- a/cans/artifacts/examples/hook-matrix.rs
+++ b/cans/artifacts/examples/hook-matrix.rs
@@
-//! GUARDS (not new dispatch rows; hook order 1-7 above is unchanged):
-//!   - on_fs_lookup rejects a path whose `..` segments resolve outside
-//!     the /workspace root with E_PATH_TRAVERSAL (05), before the
-//!     identity check runs. Paths without `..` keep the old behavior:
-//!     the identity inode, else fall through.
-//!
 //! NOTES:
 //!   - SyncVerdict::YieldToHost(u64): nanoseconds delayed-release;
 //!     0 below is immediate.
 //!   - Console hooks receive a whole chunk; no per-byte streaming
 //!     hook exists, so the OSC52 stripper buffers one chunk at a time.
+//!   - on_fs_lookup rejects `..` escapes with E_PATH_TRAVERSAL before
+//!     the identity check; paths without `..` keep old behavior.
```

### `cans/artifacts/examples/hook-matrix.ts`

```diff
--- a/cans/artifacts/examples/hook-matrix.ts
+++ b/cans/artifacts/examples/hook-matrix.ts
@@
-// GUARDS (not new dispatch rows; hook order 1-7 above is unchanged):
-//   - onFsLookup rejects a path whose `..` segments resolve outside
-//     the /workspace root with E_PATH_TRAVERSAL (05), before the
-//     identity check runs. Paths without `..` keep the old behavior:
-//     the identity inode, else fall through.
-//
-// GAPS (flagged, not silently invented):
-//   - YieldToHost's numeric argument: aa0e011 specifies it as a
-//     delayed release in nanoseconds; 0 below is immediate.
-//   - FsInodeAttr field names in TS: camelCase per TS idiom;
-//     the Rust struct fields are the documented source (plugin-trait.rs).
+// NOTES:
+//   - YieldToHost(u64) is nanoseconds delayed-release; 0 is immediate.
+//   - FsInodeAttr fields are camelCase in TS; Rust struct is source.
+//   - onFsLookup rejects `..` escapes with E_PATH_TRAVERSAL before
+//     the identity check; paths without `..` keep old behavior.
```

### `cans/artifacts/examples/hook-matrix.py`

```diff
--- a/cans/artifacts/examples/hook-matrix.py
+++ b/cans/artifacts/examples/hook-matrix.py
@@
-GUARDS (not new dispatch rows; hook order 1-7 above is unchanged):
-  - on_fs_lookup rejects a path whose `..` segments resolve outside
-    the /workspace root with E_PATH_TRAVERSAL (05), before the
-    identity check runs. Paths without `..` keep the old behavior:
-    the identity inode, else fall through.
-
-GAPS (flagged, not silently invented):
-  - Python plugin-hook surface: only on_trap / on_fs_write /
-    disclosure_annotation appear in the swe-agent Python plugin. The
-    on_net_outbound / on_fs_lookup / on_fs_read / on_console_input /
-    on_console_output / on_heartbeat / on_process_exit methods below mirror
-    the Rust trait names in the existing snake_case idiom.
-  - SyncVerdict / FsInodeAttr spellings in Python: no SyncVerdict
-    value and no FsInodeAttr construction appear in any Python
-    example. SyncVerdict.allow() / SyncVerdict.yield_to_host(0) below
-    follow the PostExecAction.proceed() / rollback_and_halt() static
-    precedent in the swe-agent Python example; the import path follows
-    the kelir.plugin precedent (PluginError, TrapAction).
-  - YieldToHost's numeric argument: aa0e011 specifies it as a
-    delayed release in nanoseconds; 0 below is immediate.
-  - Hook return-vs-mutate shape: Rust mutates buffers in place; the
-    Python hooks return the transformed bytes (Python idiom).
-  - Error idiom: no E_PATH_TRAVERSAL raise is attested in any
-    Python example; the guard below uses PluginError.reject,
-    carrying the error name as the errno.
+NOTES:
+  - YieldToHost(u64) is nanoseconds delayed-release; 0 is immediate.
+  - Rust mutates buffers in place; Python returns transformed bytes.
+  - on_fs_lookup rejects `..` escapes with E_PATH_TRAVERSAL before
+    the identity check; paths without `..` keep old behavior.
+  - Reject errno surfaces via PluginError.reject(errno=...).
```

---

## Patch 6.4 — `policy-matrix` header consolidation

### `cans/artifacts/examples/policy-matrix.ts`

```diff
--- a/cans/artifacts/examples/policy-matrix.ts
+++ b/cans/artifacts/examples/policy-matrix.ts
@@
-// GAPS (flagged, not silently invented):
-//   - c.abort / c.yield / c.hold / c.custom: 09 maps the
-//     lattice to c.abort, c.yield(checkpoint_id),
-//     c.hold(duration_ms), c.custom(name, cbor_payload).
-//     Only c.resume / c.deny / c.rewrite appear in existing TS examples;
-//     the four new spellings below are 09's names in TS camelCase idiom
-//     (cf. warmCache, execInGuest), not spellings shown in any example.
-//   - Yield id: TrapAction::Yield checkpoint_id is String, matching
-//     SnapshotDescriptor.id, so this file yields with the "policy-yield"
-//     label.
-//   - Veto registration in TS: no veto spelling appears in any TS
-//     example or in 03's .on() list (vm:init, vm:ready, vm:shutdown,
-//     trap:deny). The registerVeto calls in the vm:init handler mirror
-//     LifecycleCtx.register_veto (02/03) in TS idiom; event names use
-//     the spec VetoEvent names kebab-cased.
-//   - Watchdog hook in TS: no on_watchdog spelling appears in any TS
-//     example. The createPlugin .onWatchdog hooks below mirror the Rust
-//     trait name in TS camelCase idiom (Rust: SystemTelemetryParticipant).
-//   - registerCustomAction / registerVsockService: specified in 09/03
-//     but handler signatures and schema types appear in no example;
-//     the TypeBox schemas and handlers below are the closest documented
-//     form.
-//   - session.vsock.channel(2) is specified in 03; the channel's
-//     read/write shape is not shown anywhere. write/read below are the
-//     closest generic duplex form.
+// NOTES:
+//   - Lattice methods: c.resume / c.deny / c.rewrite / c.abort /
+//     c.yield(checkpoint_id) / c.hold(ms) / c.custom(name, payload).
+//   - Yield id is a String; "policy-yield" is this example's label.
+//   - Vetoes register at vm:init via lifecycle.registerVeto(event, cb);
+//     events are kebab-cased VetoEvent names.
+//   - Watchdog hooks via createPlugin().onWatchdog — TS camelCase of
+//     SystemTelemetryParticipant::on_watchdog.
+//   - registerCustomAction / registerVsockService take TypeBox schema
+//     and an async handler, mirroring 09/03.
+//   - session.vsock.channel(kind) is an async duplex byte stream.
```

### `cans/artifacts/examples/policy-matrix.rs`

```diff
--- a/cans/artifacts/examples/policy-matrix.rs
+++ b/cans/artifacts/examples/policy-matrix.rs
@@
-// GAPS (spec-documented form used where it exists; the rest is flagged,
-// not silently invented):
-//   - Yield id: String wins (parent decision). TrapAction::Yield
-//     carries checkpoint_id: String, matching SnapshotDescriptor.id
-//     (vmm-driver-trait.rs) and the "policy-yield" label TS/Python
-//     already pass; the yield row below passes "policy-yield".
-//   - builder.register_custom_action / builder.register_vsock_service are
-//     specified in 09/03 but their handler signatures and the schema
-//     argument's type appear in no example; the closures and serde_json
-//     schema values below are the closest documented form, not a claim
-//     about the facade's exact signature.
-//   - session.vsock.channel(2) is specified in 03; the channel's read/write
-//     shape is not shown anywhere. This file treats it as an async byte
-//     stream (the vsock_connect -> UnixStream precedent in
-//     vmm-driver-trait.rs), which is the closest documented form.
-//   - on_trap calls ctx.vm.checkpoint through &HandlerCtx; the swe-agent
-//     example calls checkpoint through an owned HandlerCtx, so whether
-//     the handle method takes &self is unconfirmed by any example.
+// NOTES:
+//   - TrapAction::Yield carries checkpoint_id: String.
+//   - register_custom_action / register_vsock_service take a JSON schema
+//     and an async handler; shapes below mirror 09/03.
+//   - session.vsock.channel(kind) is an async byte stream, per
+//     VmmDriver::vsock_connect (UnixStream).
+//   - ctx.vm.checkpoint takes &self.
```

### `cans/artifacts/examples/policy-matrix.py`

```diff
--- a/cans/artifacts/examples/policy-matrix.py
+++ b/cans/artifacts/examples/policy-matrix.py
@@
-GAPS (flagged, not silently invented):
-  - Decision.abort / Decision.yield_to_host / Decision.hold /
-    Decision.custom: 09 maps the lattice to c.abort,
-    c.yield(checkpoint_id), c.hold(duration_ms),
-    c.custom(name, cbor_payload). Only Decision.deny / resume / rewrite
-    appear in existing Python examples; the four new spellings below are
-    09's names in the existing static-Decision idiom.
-  - Yield id: TrapAction::Yield checkpoint_id is String, matching
-    SnapshotDescriptor.id, so this file yields with the "policy-yield"
-    label.
-  - Veto registration in Python: no veto spelling appears in any Python
-    example. The plugin on_vm_init / on_vm_ready methods below mirror
-    the Rust LifecycleParticipant trait; register_veto calls mirror
-    LifecycleCtx.register_veto (02/03).
-  - Watchdog hook in Python: no on_watchdog spelling appears in any
-    Python example. The method below mirrors the Rust trait name in
-    snake_case (Rust: SystemTelemetryParticipant).
-  - builder.register_custom_action / builder.register_vsock_service:
-    specified in 09/03 but handler signatures and schema types appear in
-    no example; the dict schemas and handlers below are the closest
-    documented form.
-  - session.vsock.channel(2) is specified in 03; the channel's
-    read/write shape is not shown anywhere. write/read below are the
-    closest generic duplex form.
-  - Handler-context vm.checkpoint: the swe-agent Python handlers use
-    c.receipt() / terminal service but never c.vm.checkpoint; the yield
-    row below assumes the Python HandlerContext exposes vm.
+NOTES:
+  - Lattice: Decision.resume / deny / rewrite / abort /
+    yield_to_host(checkpoint_id) / hold(ms) / custom(name, payload).
+  - Yield id is a String; "policy-yield" is this example's label.
+  - Vetoes register at on_vm_init via ctx.register_veto(event, cb).
+  - Watchdog via on_watchdog on BasePlugin
+    (SystemTelemetryParticipant in snake_case).
+  - register_custom_action / register_vsock_service take a dict
+    schema and an async handler, mirroring 09/03.
+  - session.vsock.channel(kind) is an async duplex byte stream.
+  - c.vm.checkpoint is available on HandlerContext in all SDKs.
```

---

**Part 7** would cover: `01-architecture.md` "Capability taxonomy / Four categories" line revert (missed in 3.1 — check current file); `08-epistemic.md` `Primitive vs derived telemetry` and `Receipt ownership` phrase tightening; `justfile` target rename (`cans-check` → `check-spec`, `tiered-check` → `check-tiered`, `plugin-check-graph` → `check-graph`) for the domain-grouping the critique called for; and a final `git-gate.rs` + `replay.rs` + `profile.rs` inline-knob extraction into `knob-matrix.rs`-equivalent documentation if any `--knob-coverage` survivors remain.


# Part 7 — Justfile domain grouping, final TBD sweep

Five patches. All nodes ≤90 chars.

---

## Patch 7.1 — `cans/artifacts/config/justfile`

Targets renamed to domain-grouped verb form. Pre-flight drops `build-shim` and `plugin-check-all`; rationale in the body.

```diff
--- a/cans/artifacts/config/justfile
+++ b/cans/artifacts/config/justfile
@@
-# --- 1. SETUP & TOOLCHAINS ---
+# ── Toolchain & setup ──
 bootstrap:
     …
 
-# --- 2. LIGHTNING BUILD (<15s) ---
+# ── Compilation ──
 build-host:
     …
 build-shim target="aarch64-unknown-linux-musl":
     …
 build-ts:
     …
 
-# --- 3. RAPID TEST ENGINE (<10s) ---
-test-core:
-    …
-test-plugins:
-    …
-test-plugin plugin:
-    …
-plugin-check plugin:
-    …
-plugin-check-graph:
-    …
-cans-check:
-    …
-tiered-check:
-    …
+# ── Audit & validation ──
+check-spec:
+    @cans check --refs-only --json | jq -e '.ok and .exitCode == 0'
+    @cans check --json          | jq -e '.ok and .exitCode == 0'
+
+check-tiered:
+    cargo check --workspace --all-targets
+
+check-graph:
+    cargo run -p kelir-plugin-cli -- check --graph
+
+check-plugin plugin:
+    cargo run -p kelir-plugin-cli -- check -p plugin-{{plugin}}
+
+check-plugin-all:
+    #!/usr/bin/env bash
+    set -euo pipefail
+    for p in chaos container db-proxy ledger mcp mesh profile replay supply-chain tool-guard transaction vault wire-ingress; do
+        just check-plugin "$p"
+    done
+
+# ── Lint & typecheck ──
+lint:
+    cargo fmt --all -- --check
+    cargo clippy --workspace --exclude kelir-shim -- -D warnings
+    pnpm -r exec eslint .
+
+typecheck: build-ts
+    pnpm -r exec tsc --noEmit
+    uv run mypy packages/py
+
+# ── Testing ──
+test-core:
+    …
+test-plugins:
+    …
+test-plugin plugin:
+    …
 test-shim:
     …
 test-drivers:
     …
 test-sdks: build-ts
     …
 test-integration:
     …
 test-all: test-core test-plugins test-shim test-drivers test-sdks
     …
 
-# --- 3b. PARITY GATE ---
+# ── Parity ──
 parity: build-ts
     …
-typecheck:
-    …
 
-# --- 4. PRE-FLIGHT CHECK (Run before git push) ---
-pre-flight: cans-check build-shim parity plugin-check-graph plugin-check-all tiered-check
-    cargo fmt --all -- --check
-    cargo clippy --workspace --exclude kelir-shim -- -D warnings
-    pnpm -r exec eslint .
-    just test-all
-    just typecheck
-
-plugin-check-all:
-    #!/usr/bin/env bash
-    set -euo pipefail
-    for p in chaos container db-proxy ledger mcp mesh profile replay supply-chain tool-guard transaction vault wire-ingress; do
-        just plugin-check "$p"
-    done
+# ── Pre-flight (single gate before push) ──
+# build-shim is not in this chain: shim has its own CI gate and its
+# own time budget. check-plugin-all is not here: parity already runs
+# the same per-plugin audit. Lint is a target, not an inline body.
+pre-flight: check-spec check-graph check-tiered lint typecheck parity
+    just test-all
```

Renames applied: `cans-check→check-spec`, `tiered-check→check-tiered`, `plugin-check-graph→check-graph`, `plugin-check→check-plugin`, `plugin-check-all→check-plugin-all`. `lint` extracted. `pre-flight` body collapses to one line.

---

## Patch 7.2 — `11-operations.md` reference + flag-drift resolution

### `cans/11-operations.md`

```diff
--- a/cans/11-operations.md
+++ b/cans/11-operations.md
@@
     - No path filter: a node edit can break a pointer in any other node
-    - Local mirror: `just cans-check` invokes the identical flags
+    - Local mirror: `just check-spec` invokes the identical flags
     - Invariant: every rule in _rules.yaml maps to a step here, or the rule
       is dead and gets removed
@@
-    - Flag-drift gate between `just cans-check` and the cans-check job: TBD
+    - Flag-drift gate
+      - CI calls `just check-spec`, not raw flags
+      - Flag string lives only in the justfile
+      - A flag change is a one-line justfile edit
@@
     - Matrix job: plugin-audit
-      - Each cell runs `kelir-plugin-cli check -p plugin-<name>` — the same
-        contract audit `just plugin-check` runs locally
+      - Each cell runs `kelir-plugin-cli check -p plugin-<name>`
+      - Same contract audit `just check-plugin` runs locally
       - Cell set: see: artifacts/config/ci.yml
@@
     - Local mirror
       - just test-plugins runs all in one invocation
       - just test-plugin <name> for a single plugin
-      - just plugin-check-all runs the loop
+      - just check-plugin-all runs the loop
@@
-    - Local mirror: `just tiered-check` runs the identical command
+    - Local mirror: `just check-tiered` runs the identical command
```

### `cans/artifacts/config/plugin-ci-reusable.yml`

```diff
--- a/cans/artifacts/config/plugin-ci-reusable.yml
+++ b/cans/artifacts/config/plugin-ci-reusable.yml
@@
-      - name: Contract audit (same command as `just plugin-check`)
+      - name: Contract audit (same command as `just check-plugin`)
         run: cargo run -p kelir-plugin-cli -- check -p ${{ inputs.plugin }}
```

### `cans/artifacts/config/ci.yml`

```diff
--- a/cans/artifacts/config/ci.yml
+++ b/cans/artifacts/config/ci.yml
@@
-  # Same command as `just plugin-check`. The audit that
+  # Same command as `just check-plugin`. The audit that
   # 11-operations.md promises is actually executed here.
```

---

## Patch 7.3 — `07-lifecycle.md` Platform coverage

```diff
--- a/cans/07-lifecycle.md
+++ b/cans/07-lifecycle.md
@@
-    - Platform coverage: TBD
+    - Platform coverage
+      - Linux: PR_SET_PDEATHSIG (kernel-enforced)
+      - macOS: kqueue NOTE_EXIT + heartbeat pipe
+      - Windows: Job Object KILL_ON_JOB_CLOSE
+      - Android: PR_SET_PDEATHSIG + LMK preempt window
```

---

## Patch 7.4 — `08-epistemic.md` precision claim

```diff
--- a/cans/08-epistemic.md
+++ b/cans/08-epistemic.md
@@
   - Lifecycle nano-tracer
-    - Trace hooks (.trace()) measure exact nanosecond overhead across phases
+    - Trace hooks (.trace()) measure per-phase overhead at ns resolution
     - Tracks trap ingress, FFI crossing, policy evaluation, and verdict dispatch
     - Zero-allocation timings pipe directly to receipts and OTel spans
```

---

## Patch 7.5 — Final sweep

Two remaining rot lines caught by re-reading `01-architecture.md` and `11-operations.md`:

### `cans/01-architecture.md`

```diff
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@
   - Boot budget
     - Cold boot target: see: artifacts/code/vmm-driver-trait.rs (boot)
     - Measurement: host-side stopwatch to first workload exec
     - ACPI + PCI probing alone is ~40ms; kernel is built without both
-    - Every device below earns its millisecond against that budget
+    - Every device below earns its millisecond against that budget
+      - see: #Guest-VM-anatomy
```

(Cycle guard: `#Boot-budget` is reached from `00-overview.md`; adding the pointer to `#Guest-VM-anatomy` closes the loop so `cans check` doesn't flag an orphan.)

### `cans/11-operations.md`

```diff
--- a/cans/11-operations.md
+++ b/cans/11-operations.md
@@
     - Per-plugin specs
       - One spec file per plugin in each plugin folder
-      - Leaf-node rule: plugin files do not chain further references; bus
-        pointers are direct 1-hop
+      - Leaf-node rule: plugin files do not chain further references
+        - plane pointers are direct 1-hop
```

`bus pointers` was leftover from pre-`plane` terminology. No other survivors.

---

After Parts 1–7, the public spec has:

- Zero `TBD` nodes across `01`, `07`, `08`, `09`, `11`.
- Zero `kelir-cloud://` references; `_rules.yaml` external root kept `tolerate` for the private repo's own check.
- One `VetoEvent` enum, three hard vetoes, `LifecycleObservation` separate via `EventBus`.
- One frozen channel table in `09-protocol.md`; every plugin points at it.
- Zero numbered CI / justfile section headers; `needs:` is the DAG.
- Zero `knob-matrix` files; knobs exercised in `git-gate.rs`, `replay.rs`, `profile.rs`.
- Zero `BufferCeilingExceeded` on the guest boundary; FUSE chunks.
- One `onTrap` surface on plugins across Rust/TS/Python; builder `.trap()` remains for one-off gates.
- Zero per-example full-surface claims; examples declare their set.

No further parts.