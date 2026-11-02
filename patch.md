**x = 9.** Parts:

1. **Vocabulary purge + scope amputation**
2. Unified interposition pipeline (verdict lattice)
3. Transport mux + channel table
4. State/memory reconciliation (ephemeral/catalog, 64-slot)
5. Storage plane single abstraction (kill FUSE/overlay hybrid)
6. PMU demotion + `10-epistemic.md` rename
7. SDK typestate simplification
8. Vault out-of-band credential broker
9. Lifecycle rollback network truth

---

## Part 1 / 9 — Vocabulary purge + scope amputation


```diff
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@ -Strict-hardware-kernel-abstraction-layer
         - Zero knowledge of prompts, evaluation loops, or orchestration logic
-        - Operates strictly as OS physics: pages, registers, descriptors, frames
+        - Operates strictly on physical primitives: pages, registers, descriptors, frames
         - Hardware memory execution policy: strict W^X enforced at MMU boundary
@@ -Deterministic-hardware-isolation
-      - Transparent truthful steering
-        - Any intercepted syscall, file read, or network packet is allowed,
-          - steered with a host disclosure, or rejected with a machine-parsable receipt
-        - Mandatory TTY host disclosure invariant
```

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -Tier-2-governance-and-protocol-proxies
       - plugin-tool-guard: tool-call governance proxy
-      - plugin-db-proxy: database protocol proxy
-      - plugin-supply-chain: execution governance proxy
       - plugin-wire-ingress: inbound wire proxy
@@ -Composite-plugin-hierarchy
-    - Composite expansion enforces downward min cascade across budget fields
+    - Composite expansion enforces monotonic limits across budget fields
@@ -Catalog-tiering
-      - Community plugins: optional domain extensions in cans/plugins/
-        - mcp, mesh, tool-guard, db-proxy, supply-chain, wire-ingress
-        - vault, container, replay, ledger, profile, chaos, transaction
+      - Community plugins (in-process engine): cans/plugins/
+        - mcp, mesh, tool-guard, wire-ingress, vault, container
+        - replay, ledger, profile, chaos, transaction
+      - Userland SDK policies (host consumer code, not engine plugins):
+        - db-proxy, supply-chain
@@ -Coverage-matrix
       - chaos, ledger, vault: autonomous-swe-agent.{rs,ts,py}
-      - mesh, supply-chain, container: multi-vm-swarm.rs
-      - db-proxy, transaction: db-backed-runtime.py
+      - mesh, container: multi-vm-swarm.rs
+      - transaction: db-backed-runtime.py
       - wire-ingress, mcp, tool-guard: serverless-webhook.ts
       - replay: replay.rs
       - profile: profile.rs
+    - Userland SDK policy demonstrations
+      - db-proxy: db-backed-runtime.py
+      - supply-chain: multi-vm-swarm.rs
```

```diff
--- a/cans/03-sdk-architecture.md
+++ b/cans/03-sdk-architecture.md
@@ -Session-control-surface
           - Graceful drain: session.drain(ms); see: 08-lifecycle.md#Hypervisor-primitives
-          - Break-glass SSH/SFTP: session.ssh() -> SshHandle; see: 08-lifecycle.md#Break-glass
```

```diff
--- a/cans/08-lifecycle.md
+++ b/cans/08-lifecycle.md
@@ -Break-glass
   - Break-glass
-    - Human operators need real tooling: VS Code Remote, JetBrains Gateway,
-      - rsync, sftp, multi-session shells. A raw PTY cannot satisfy these.
-    - Ephemeral SSH/SFTP subsystem
-      - In-process SSH server hosted inside the runtime (russh + russh-sftp)
-      - Listener bound to 127.0.0.1:<ephemeral> on the host
-      - Public key auth: Ed25519 keypair minted per `session.ssh()` call,
-        - public key injected to the guest over the paravirtual control channel
-      - Default user `root`; configurable via connect options
-      - SFTP subsystem on by default
-      - Port forwarded from host to guest via the vsock transport
-    - Standard tools attach transparently
-      - ssh, sftp, rsync, scp, VS Code Remote SSH, JetBrains Gateway
-      - Multiple concurrent sessions; no head-of-line blocking
-    - Endpoint lifecycle
-      - `session.ssh()` opens the endpoint; caller receives host, port,
-        - and fingerprint
-      - `close()` tears down the endpoint and invalidates the keypair
-      - Dropping the handle closes connections and unregisters the listener
-    - Interaction with draining and paused states
-      - SSH is available in Running and Draining
-      - Paused and Stopped refuse with ESSHUnavailable
-    - Coexistence with PTY break-glass
-      - Raw PTY console master remains for kernel-level recovery only
-      - Human debugging uses SSH by default
-    - Spelling: `session.ssh()` (all SDKs);
-      - `session.breakGlass()` (TS) is a deprecated alias for `session.ssh()`
+    - PTY console master (Port 0) is the sole in-process break-glass surface
+    - Exec channel (0x0003) carries programmatic break-glass commands
+    - No in-process SSH/SFTP daemon; zero russh, russh-sftp in runtime
+    - Guest workloads that need sshd ship it inside their EROFS rootfs
+    - Host operators run ssh as a subprocess against the guest sshd
+    - Interaction with lifecycle states
+      - Running and Draining: available
+      - Paused and Stopped: refuse with ESSHUnavailable
```

```diff
--- a/cans/12-adapters.md
+++ b/cans/12-adapters.md
@@ -Core-ingestion-primitive-kelir-pack
     - Core ingestion primitive: kelir-pack
-      - Host-side two-phase in-memory compiler: OCI layer blob → EROFS
+      - Offline two-phase in-memory compiler: OCI layer blob → EROFS
@@ -Distributable
-      - Distributable as compile-time CLI or host-side library call
+      - Distributed as external CLI binary only
+      - Zero in-process OCI client, layer unpacker, or EROFS builder in supervisor
```

```diff
--- a/cans/15-operations.md
+++ b/cans/15-operations.md
@@ -Supporting-crates
         - kelir-transport: Stream trait + SPSC ring, AF_VSOCK, named-pipe backends
         - kelir-catalog: SQLite schema, pool, migration helpers (used by core)
-        - kelir-ssh: in-process SSH/SFTP server (russh), used by break-glass
         - kelir-otlp: zero-allocation metric registry and OTLP exporter
```

```diff
--- a/cans/plugins/db-proxy.md
+++ b/cans/plugins/db-proxy.md
@@ -Crate-dependencies
     - name() = "db-proxy"
-    - Tier 2 governance and protocol proxy (db-proxy)
+    - Userland SDK policy (db-proxy): host-consumer code, not engine plugin
     - Depends on (db-proxy): kelir-interface only (binds to native Wire Plane)
```

```diff
--- a/cans/plugins/supply-chain.md
+++ b/cans/plugins/supply-chain.md
@@ -Crate-dependencies
     - name() = "supply-chain"
-    - Tier 2 governance and protocol proxy (supply-chain)
+    - Userland SDK policy (supply-chain): host-consumer code, not engine plugin
     - Depends on (supply-chain): kelir-interface and Tier 1 crates
```

```diff
--- a/cans/plugins/profile.md
+++ b/cans/plugins/profile.md
@@ -Invariants-and-policy
       - Routes unparsed configuration fragments to registered namespace owners
       - Profile contains zero hardcoded child domain logic or expansion rules
-      - Downward min cascade enforced uniformly across scalar budget fields
-    - Downward min cascade
+      - Monotonic limits enforced uniformly across scalar budget fields
+    - Monotonic limits
       - Effective budget and resource limits evaluate as min(base, child)
       - Overlays cannot widen execution duration, memory, or instruction ceilings
-      - Violations of downward min cascade fail validation with ESecurityViolation
+      - Violations of monotonic limits fail validation with ESecurityViolation
```

```diff
--- a/cans/plugins/tool-guard.md
+++ b/cans/plugins/tool-guard.md
@@ -Anti-Rug-Pull-schema-pinning
-    - Anti-Rug-Pull schema pinning
+    - Immutable schema locking
       - Intercepts initial "tools/list" advertisement at on_vm_ready
       - Computes BLAKE3 hash over all tool schemas, signatures, and descriptions
       - Locks the catalog: dynamic schema mutation or description injection
         post-boot halts with ESecurityViolation
@@ -Denial-of-Wallet-recursion-limiter
-    - "Denial of Wallet" recursion limiter
+    - Call quota
       - Per-tool token-bucket governor clamps call cadence
       - Call depth ceiling: max_invocations_per_trace (default: 50)
       - Infinite recursion or retries immediately reject with ECYCLICREWRITE / ETOOLQUOTA
```

End Part 1/9. Say `continue` for Part 2.

## Part 2 / 9 — Unified interposition pipeline + verdict lattice

**Touched files:** `02-plugin-model.md`, `01-architecture.md`, `11-protocol.md`, `05-storage-plane.md`, `07-wire-plane.md`, `plugins/tool-guard.md`, `plugins/db-proxy.md`, `plugins/chaos.md`, `03-sdk-architecture.md`.

```diff
--- a/02-plugin-model.md
+++ b/02-plugin-model.md
@@ -Dual-plane-execution-architecture
     - Interceptor pipeline plane
       - Synchronous or bounded latency inline processors
-      - Evaluates monotonic verdict lattice: Allow < Rewrite < Deny < Abort
+      - Evaluates unified verdict lattice: Pass < Patch < Drop < Suspend < Abort
       - Single mutator owns target byte slice at any stage
@@ -Error-variants
-    - Error variants
-      - Interceptor traits return typed verdicts: SyncVerdict (FS/Net), Option<TrapAction> (Trap)
-      - Idempotency and density: see: 01-architecture.md#Codebase-invariants
-      - Ok(SyncVerdict::Allow) = allow hot-path operation
-      - Ok(SyncVerdict::YieldToHost(ns)) = delay or yield operation to host event loop
-      - Err(PluginError::Reject { errno, reason }) = reject with POSIX errno surfaced to guest
-      - Err(PluginError::Halt(msg)) = unrecoverable failure; freezes VM pipeline
+    - Unified verdict
+      - Every interposer hook returns the same engine-level `Verdict`
+      - One enum across Trap, Fs, Net, Console, Exec, MCP, and DB rings
+      - Variants
+        - Verdict::Pass = proceed untouched
+        - Verdict::Patch(payload) = in-place mutation; payload schema per ring
+        - Verdict::Drop { errno, reason } = deny; errno surfaces to guest
+        - Verdict::Suspend { until } = yield or hold to host
+        - Verdict::Abort { reason } = fatal; pipeline freezes
+      - Drop { errno: None } = silent blackhole (chaos-loss only)
+      - Patch payload is registered per ring: ExecveMutation, HttpFrame,
+        - FsChunk, JsonRpcFrame, PgFrame, Custom(schema)
+      - Idempotency and density: see: 01-architecture.md#Codebase-invariants
@@ -Hook-return-semantics
     - Hook return semantics
-      - on_fs_write: see: 05-storage-plane.md#Write-gating (storage owns it)
-      - on_fs_lookup → Ok(None) = fall through; Ok(Some(attr)) = serve
-      - on_net_outbound / on_net_inbound → see: artifacts/code/plugin-trait.rs
+      - Every hook returns Verdict; no per-ring aliases
+      - on_fs_lookup → Verdict::Pass = fall through; Patch(attr) = serve
+      - on_fs_write / on_net_outbound / on_net_inbound → Verdict
+      - Trap hooks → Verdict; ring-specific defaults documented per plane
@@ -Static-dispatch
     - Static dispatch
       - Dispatch compilation: direct Cargo linkage resolves plugin order at build time
       - Pipeline tables index linear interceptor chains per VirtIO ring
       - Core verifies plugin crate graph: cycles and missing deps halt boot
-      - Monotonic permission lattice: Allow -> Rewrite -> Deny -> Abort
+      - Monotonic verdict lattice: Pass -> Patch -> Drop -> Suspend -> Abort
       - Restrictive monotonicity: downstream hooks cannot widen upstream denial
       - Fail-closed mutation collisions: overlapping writes raise EAmbiguousMutation
@@
     - First Deny or Rewrite halts pipeline, dispatches over control transport
+    - First Drop, Suspend, or Abort halts pipeline; Pass/Patch continue
     - on_trap_deny is builder-level, not a plugin hook
       - Deny-observation composition is a user concern
+  - Unified interposition pipeline
+    - Single engine for every guest-to-host interaction
+    - Guest Ring Event (Syscall | NetFrame | FsOp | Msg) enters one path
+    - Stage 1: Fast Filter (native BPF, inode mask, CIDR/port table)
+    - Stage 2: Dispatcher evaluates MountScope; FFI gated here
+    - Stage 3: Execution commits Verdict to hardware or VirtIO ring
+    - Zero bespoke pipelines; zero per-ring verdict enums
+    - Ring, Fast Filter, and Dispatcher are native Rust; only Stage 3
+      - crosses FFI on scope match
+    - Fast Filter budget: <2µs; Dispatcher budget: <1µs
+    - See: #Latency-ceiling for FFI cross budget
```

```diff
--- a/01-architecture.md
+++ b/01-architecture.md
@@ -Modular-VirtIO-interposition-microkernel
       - Modular VirtIO interposition microkernel
         - Core controls hypervisor execution and dispatches VirtIO events
+        - Single unified interposition pipeline; no per-ring bespoke paths
+          - see: 02-plugin-model.md#Unified-interposition-pipeline
         - Core defines zero domain policies, protocols, or agent schemas
@@ -Core-invariants
     - The closed set of core primitives
     - Interposition is total: every device channel is hooked
+    - Unified verdict lattice: Pass | Patch | Drop | Suspend | Abort
+      - see: 02-plugin-model.md#Unified-verdict
     - Trait definitions
```

```diff
--- a/11-protocol.md
+++ b/11-protocol.md
@@ -Trap-fast-path
     - Three-tier gate pipeline
       - Gate 1: guest in-kernel BPF allowlist returns for harmless tools
       - Gate 2: native MountScope/schema eval in host RAM; 0 FFI
       - Gate 3: trap handler dispatch through foreign FFI trampoline
+      - Pipeline home: see: 02-plugin-model.md#Unified-interposition-pipeline
       - Synchronous write/packet filters evaluate native Rust rules; foreign callbacks yield
@@ -TrapAction-method-mapping
-    - TrapAction-method-mapping
-      - TrapAction::Allow: c.resume()
-      - TrapAction::Deny: c.deny(exit_code, diagnostic_payload)
-      - TrapAction::Rewrite: c.rewrite(target_mutation)
-      - TrapAction::Yield: c.yield(checkpoint_id)
-      - TrapAction::Abort: c.abort(exit_code, reason)
-      - TrapAction::Hold: c.hold(duration_ms)
-      - TrapAction::Custom: c.custom(name, cbor_payload)
+    - Verdict-method-mapping
+      - Verdict::Pass: c.pass()
+      - Verdict::Patch(ExecveMutation): c.patch(mutation)
+      - Verdict::Drop { errno, reason }: c.drop(errno, reason)
+      - Verdict::Suspend { until }: c.suspend(until)
+      - Verdict::Abort { reason }: c.abort(reason)
+      - SuspendUntil: Deadline(ms) | Approval | Handshake
+      - Custom payload: c.patch(Custom { name, cbor })
     - Hold wire contract
       - Host halts vCPU execution and drops RAM footprint via suspend primitive
       - Host releases suspension when the hold duration elapses
     - Custom dispatch contract
       - Registration: builder.register_custom_action(name, schema, handler)
-      - Wire Custom carries the CBOR payload only; the name resolves at SDK registration
+      - Wire Custom carries the CBOR payload only; the name resolves at SDK
+        - registration
       - Unknown name at trap time fails with EUnknownAction
       - Registration after boot() fails with EConfig
-    - Action invariants: see: artifacts/code/messages.rs
+    - Verdict invariants: see: artifacts/code/messages.rs
   - Channel registration
@@ -Trap-decision-invariants
     - Trap decision invariants
       - Rewrite loop prevention
         - No-op mutation aborts EInvalidRewrite
           - Identical binary, argv, env
         - Mutation equal to ancestor aborts ECyclicRewrite
         - Max rewrite depth 4
       - new_binary canonicalisation
         - Absolute path only
         - Resolved via openat2
           - RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS
         - Rooted at MountScope binary root
         - Escapes reject with EInvalidRewrite
       - Env injection limits
         - set_env+unset_env total ≤ 32KB CBOR-encoded
         - Key length ≤ 128 bytes
         - vault:// placeholder values reject ESecurityViolation
-      - Custom dispatch timeout
-        - Exceeded handler falls through to Allow
+      - Custom dispatch timeout
+        - Exceeded handler falls through to Verdict::Pass
         - Per-custom-action; defaults to 30s
```

```diff
--- a/05-storage-plane.md
+++ b/cans/05-storage-plane.md
@@ -Write-gating
   - Write gating
     - Synchronous VirtIO-FS interception gates FUSE write() via on_fs_write
     - Signature: see: artifacts/code/plugin-trait.rs
-    - Write-gating error variants: see: 02-plugin-model.md#Hook-return-semantics
+    - Verdict surface: see: 02-plugin-model.md#Unified-verdict
     - Receives &FsContext (inode, path, locks, journal); zero ambient context
@@
     - Buffer lifecycle invariants: see: 02-plugin-model.md#FFI-buffer-lifecycle
-    - SyncVerdict::Allow = allow; kernel proceeds with write to upperdir
-    - Err(PluginError::Reject { errno, reason }) = deny
-    - Err(PluginError::Halt) = deny + halt pipeline
+    - Verdict::Pass = allow; kernel proceeds with write to upperdir
+    - Verdict::Drop { errno, reason } = deny
+    - Verdict::Patch(FsChunk) = in-place mutate; then write
+    - Verdict::Abort { reason } = deny + halt pipeline
```

```diff
--- a/07-wire-plane.md
+++ b/07-wire-plane.md
@@ -Frame-drop-and-forward-semantics
   - Frame drop and forward semantics
-    - Ok(Allow) on outbound = forward; Ok(Allow) on inbound = deliver
-    - Ok(YieldToHost) = yield frame to host for async policy
-    - Err(Reject) on L4 TCP = inject TCP RST or ICMP Port Unreachable into guest ingress ring
-    - Err(Reject) on L7 HTTP = synthesize 403 with machine-readable JSON receipt
+    - Verdict::Pass on outbound = forward; Pass on inbound = deliver
+    - Verdict::Suspend = yield frame to host for async policy
+    - Verdict::Drop { errno: ECONNREFUSED } on L4 TCP = inject TCP RST
+      - or ICMP Port Unreachable into guest ingress ring
+    - Verdict::Drop { errno: EACCES } on L7 HTTP = synthesize 403 JSON receipt
+    - Verdict::Drop { errno: None } = silent blackhole (chaos-only)
     - Guest observes ECONNREFUSED in <1ms
     - Agent corrects action without orchestration timeout
```

```diff
--- a/plugins/tool-guard.md
+++ b/plugins/tool-guard.md
@@ -Decision-lattice
-    - Decision lattice (tool-guard)
-      - ToolAction::Allow: forwards frame untouched
-      - ToolAction::Rewrite(mutation): serializes canonical ExecveMutation ABI:
-        - new_binary: Option<String> (overrides target ELF path)
-        - new_argv: Option<Vec<String>> (replaces execution arguments)
-        - set_env: HashMap<String, String> (injected environment variables)
-        - unset_env: Vec<String> (stripped environment variables)
-        Packed into target_mutation bytes; guest shim decodes and applies
-        atomically before releasing trapped execve syscall
-      - ToolAction::Deny(errno, reason): drops call, synthesizes standard JSON-RPC error response
-      - ToolAction::Hold: requests hypervisor
-        - LifecycleRequest::Suspend(External)
-        - Swap mechanics: see: 08-lifecycle.md#Suspend-and-resume
-        - Host waits for human out-of-band authorization via vsock Channel 0
+    - Unified verdict surface (tool-guard)
+      - Returns engine-level Verdict from 02-plugin-model.md#Unified-verdict
+      - Verdict::Pass: forwards frame untouched
+      - Verdict::Patch(ExecveMutation): serializes canonical mutation ABI:
+        - new_binary: Option<String> (overrides target ELF path)
+        - new_argv: Option<Vec<String>> (replaces execution arguments)
+        - set_env: HashMap<String, String> (injected environment variables)
+        - unset_env: Vec<String> (stripped environment variables)
+        - Guest shim decodes and applies atomically before trapped execve releases
+      - Verdict::Drop { errno, reason }: JSON-RPC error response synthesized
+      - Verdict::Suspend { until: Approval }: hypervisor external suspend
+        - Host waits for human out-of-band authorization via vsock Channel 0
+        - Swap mechanics: see: 08-lifecycle.md#Suspend-and-resume
@@ -Error-and-return-semantics
     - Return errno and rejection behavior (tool-guard)
-      - ToolAction::Deny returns JSON-RPC error object:
+      - Verdict::Drop returns JSON-RPC error object:
         - code = -32001 (EPERM) or mapped POSIX errno
         - message = user-defined reason or "Policy violation: tool execution rejected"
```

```diff
--- a/plugins/db-proxy.md
+++ b/plugins/db-proxy.md
@@ -Invariants-and-policy
-    - Verdict execution: DbAction::Allow, DbAction::Deny, DbAction::Rewrite
+    - Verdict execution: engine Verdict (Pass | Patch | Drop)
+      - see: 02-plugin-model.md#Unified-verdict
@@ -Error-and-return-semantics
     - Return errno and rejection behavior (db-proxy)
-      - DbAction::Deny
+      - Verdict::Drop
         - pgwire ErrorResponse frame precedes connection close
         - SQLSTATE derived from errno (EPERM→42501, ENOENT→42P01)
         - Unmapped errno → SQLSTATE XX000 (internal_error)
-      - DbAction::Rewrite
+      - Verdict::Patch(PgFrame)
         - Q/P frame payload replaced before upstream forward
         - Server sees rewritten frame as if client sent it
         - Per-connection rewrites tallied; >3 aborts ECYCLICREWRITE
```

```diff
--- a/plugins/chaos.md
+++ b/plugins/chaos.md
@@ -Error-and-return-semantics
     - Return errno and rejection behavior (chaos)
-      - Fault injection adheres to monotonic lattice via PluginError::Reject
-      - Synthetic loss uses SyncVerdict::Drop, a chaos-only variant
+      - Fault injection adheres to monotonic lattice via Verdict
+      - Synthetic loss uses Verdict::Drop { errno: None }, chaos-only
         - Silently discards frame, injects no RST, surfaces no errno
         - see: 07-wire-plane.md#Frame-drop-and-forward-semantics
-      - PluginError::Reject retains policy semantics (fast-fail RST)
-        - Chaos never returns Reject for loss; Reject is used only for RST injection
+      - Verdict::Drop { errno: ECONNREFUSED } retains policy semantics
+        - Chaos uses ECONNREFUSED only for RST injection
```

```diff
--- a/03-sdk-architecture.md
+++ b/cans/03-sdk-architecture.md
@@ -Trap-verdict-surface
-      - Trap verdict surface: see: 11-protocol.md#TrapAction-method-mapping
+      - Unified verdict surface: see: 02-plugin-model.md#Unified-verdict
+        - Rust: c.pass() / c.patch(m) / c.drop(e,r) / c.suspend(u) / c.abort(r)
+        - TS: c.pass / c.patch / c.drop / c.suspend / c.abort
+        - Python: Verdict.pass_() / Verdict.patch / Verdict.drop
+          - Verdict.suspend / Verdict.abort
         - Console interposer signature (all SDKs)
           - on_console_input(port, bytes) / on_console_output(port, chunk)
           - port identifies the virtio-console port; bytes mutated in place
-        - Yield surface naming
-          - Rust: TrapAction::Yield { checkpoint_id }
-          - TS: c.yield(checkpoint_id)
-          - Python: Decision.yield_to_host(checkpoint_id)
-          - Python has no trailing underscore; yield_to_host is the name
+        - Suspend surface naming
+          - Rust: Verdict::Suspend { until: SuspendUntil }
+          - TS: c.suspend(SuspendUntil.deadline | .approval | .handshake)
+          - Python: Verdict.suspend(until=...)
+          - No per-SDK aliases; one name across languages
```

**Cross-refs to verify in CI:** `--action-coverage` must now assert every `Verdict` variant is exercised; `--veto-coverage` unchanged. `TrapAction`, `PostExecAction`, `SyncVerdict`, `PluginError::Reject`, `DbAction`, `ToolAction` no longer appear as identifiers.

End Part 2/9. Say `continue` for Part 3 (Transport mux + channel table).

## Part 3 / 9 — Transport mux + channel table

**Touched files:** `11-protocol.md`, `01-architecture.md`, `04-terminal-plane.md`, `07-wire-plane.md`, `08-lifecycle.md`, `02-plugin-model.md`, `03-sdk-architecture.md`, `15-operations.md`, `plugins/profile.md`, `plugins/db-proxy.md`, `plugins/mcp.md`, `plugins/wire-ingress.md`.

Canonical shape: **two transport channels only.** `0x0000` = Hypervisor Control. `0x0001` = Generic Stream Mux. Every other "channel" is a **stream kind** inside the mux. Core stream kinds frozen; plugin stream kinds register at boot.

```diff
--- a/11-protocol.md
+++ b/11-protocol.md
@@ -Control-channel
   - Control channel
     - virtio-vsock port 5252: unified multiplexed transport
-    - Binary framing isolates sub-protocols via channel_id
-    - Channel allocation table
-      - 0x0000: core hypervisor lifecycle + trap control
-      - 0x0000 also dispatches out-of-band process signals and window resize
-      - 0x0001: L4 stream forwarding (native wire plane)
-      - 0x0002: profile-sync (plugin-profile)
-      - 0x0003: binary-exec (native terminal plane, programmatic exec streams)
-      - 0x0004: db-sqlite (plugin-db-proxy)
-      - 0x0005: control-plane VFS (paginated posix-like filesystem API)
-      - 0x0006: bulk transfer (raw chunked byte streams)
-      - 0x0007-0x000F: reserved (core)
-      - 0x0010: pty-control (native terminal plane)
-      - 0x0011-0x7FFF: unallocated; new row needs spec revision
-      - 0x8000-0xFFFF: reserved; new row needs spec revision
-    - Adding a row requires a spec revision
-      - No runtime allocation, no handshake, no negotiation
+    - Exactly two transport channels
+      - 0x0000: Hypervisor Control (lifecycle, trap control, signals)
+      - 0x0001: Generic Stream Mux (all request/response and data flows)
+    - Every other capability is a stream kind multiplexed on 0x0001
+    - Stream kind registry
+      - Core kinds (frozen)
+        - 0x0001: EXEC (programmatic exec + stdout/stderr/control)
+        - 0x0002: PTY_CONTROL (window resize, signal-forward per exec)
+        - 0x0003: VFS (paginated POSIX-like filesystem API)
+        - 0x0004: BULK (raw chunked byte streams)
+        - 0x0005: L4_FORWARD (native wire plane stream diversion)
+        - 0x0006: TERMINAL_CONSOLE (Port 0 human PTY bytes)
+        - 0x0007-0x000F: reserved (core)
+      - Plugin kinds (registered at boot, no spec revision)
+        - 0x0010: PROFILE_SYNC (plugin-profile)
+        - 0x0011: DB_SQLITE (plugin-db-proxy)
+        - 0x0012: MCP_CONSOLE (plugin-mcp)
+        - 0x0013-0x7FFF: plugin-registered; collision aborts EKindConflict
+      - 0x8000-0xFFFF: reserved
+    - Kind registration
+      - builder.register_stream_kind(kind, handler) binds a handler
+      - Core kinds abort re-registration with EKindReserved
+      - Unregistered kind on OPEN aborts with EUnknownKind
+      - Kinds are numbers; no per-kind spec edits required
@@ -Binary-exec-channel
   - Binary exec channel
-    - Transport: vsock port 5252, channel_id 0x0003
-    - Frame layout: 4-byte channel, 1-byte stream_id, 4-byte len, payload
-    - stream_id values
-      - 0x00: stdin
-      - 0x01: stdout
-      - 0x02: stderr
-      - 0x03: stream control (exit code, EOF, rlimit descriptors)
+    - Transport: stream mux 0x0001, stream kind EXEC
+    - Each exec is one mux stream; per-exec stream_id disambiguates
+    - Exec sub-lane codes (open-payload metadata)
+      - 0x00: stdin (exec-to-guest)
+      - 0x01: stdout (guest-to-exec)
+      - 0x02: stderr (guest-to-exec)
+      - 0x03: exec control (exit code, EOF, rlimit descriptors)
     - Out-of-band signal isolation: SIGINT, SIGTERM, and resize bypass 0x0003
-    - High-priority process signals route exclusively over Channel 0x0000
+    - High-priority process signals route exclusively over Channel 0x0000
     - Eliminates signal delivery stalls behind saturated stdout/stderr rings
@@
-    - Control-plane VFS channel (0x0005)
-    - Transport: vsock channel 0x0005, framed CBOR
+  - Control-plane VFS stream
+    - Transport: mux 0x0001, stream kind VFS, framed CBOR
@@
   - Bulk transfer channel (0x0006)
+  - Bulk transfer stream
+    - Transport: mux 0x0001, stream kind BULK
     - Purpose: large payload ingestion without CBOR, FUSE, or syscall interposition
@@
-    - Distinction from VFS 0x0005
+    - Distinction from VFS stream kind
       - VFS is paginated inspection/streaming with metadata
       - Bulk is contiguous high-bandwidth byte movement
@@
-  - Channel registration
-    - LifecycleCtx.register_channel(kind, handler) binds handler to kind
-    - kind must be assigned in the allocation table
-    - Unassigned kinds abort boot with EChannelReserved
-    - Core dispatches HostToGuest::ChannelData { kind, .. } to the handler
-    - Unbound kinds abort delivery with EUnknownChannel
-    - SDK channel surface: see: 03-sdk-architecture.md#Language-surfaces
+  - Stream mux framing
+    - Frame layout: stream_id: u32, flags: u8, len: u32, payload
+    - flags
+      - 0x01 OPEN: payload carries kind_id: u16 + tags
+      - 0x02 DATA
+      - 0x04 CLOSE
+      - 0x08 RST (peer aborted)
+      - 0x10 WINDOW_UPDATE (backpressure)
+    - Per-stream credit windows; credit exhausted parks sender
+    - Backpressure inherits ring/transport semantics, not new framing
+    - Zero-copy splice between mux and ring backends on supported platforms
@@
   - Env injection
-    - Workload environment variables, registers, and execution paths remain
-      - immutable under Allow verdicts
+    - Workload environment variables, registers, and execution paths remain
+      - immutable under Pass verdicts
```

```diff
--- a/01-architecture.md
+++ b/01-architecture.md
@@ -Transport-engine-abstraction
     - Host hypervisor backend terminates virtqueue rings directly in kelir-core memory
-    - Channel multiplexing (0x0000–0x0010) routes to in-memory lock-free SPSC rings
+    - Two transport channels: 0x0000 Control, 0x0001 Stream Mux
+      - see: 11-protocol.md#Control-channel
+    - Stream Mux routes all request/data flows as stream kinds
+    - Both channels terminate in in-memory lock-free SPSC rings
     - Bypasses host OS socket subsystems (AF_VSOCK, HV_SOCKET, named pipes)
```

```diff
--- a/04-terminal-plane.md
+++ b/04-terminal-plane.md
@@ -Window-geometry-and-signals
     - Window geometry and signals
-      - Carried across vsock Channel 0x0010 (pty-control)
+      - Carried on stream kind PTY_CONTROL (mux 0x0001)
       - MsgType 0x01 (WindowResize): pushes rows, cols, xpixel, ypixel
       - MsgType 0x02 (SignalForward): pushes signal and pgid; shim executes killpg
@@ -Binary-exec-channel-terminal-plane
   - Binary exec channel (terminal plane)
-    - Carries programmatic execution streams over Channel 0x0003
-    - Byte framing and stream_id values defined on Channel 0x0003
+    - Carries programmatic execution streams on stream kind EXEC (mux 0x0001)
+    - Sub-lane framing defined on EXEC stream kind
@@
     - Per-exec PTY opt-in
       - FLAG_ALLOC_PTY on EXEC_IO_SPEC allocates a private slave PTY per ExecSlot
       - isatty(3) semantics satisfied for language REPLs, git, and build runners
       - Concurrent interactive execs run without Port 0 serialization
-      - Port 0 remains the sole human break-glass channel
+      - Port 0 remains the sole human break-glass console
@@
-  - Stream replay
+  - Stream replay
     - Each console port emits an append-only log
@@
-    - Exec channel (0x0003) streams share the cursor model per exec_id
+    - EXEC stream kind shares the cursor model per exec_id
       - exec_log persists per-exec-id for post-run inspection
```

```diff
--- a/07-wire-plane.md
+++ b/cans/07-wire-plane.md
@@ -Port-forwarding
   - Port forwarding
-    - Guest-initiated: forward tunnels over vsock:5252 Channel 0x0001 on PortBound
+    - Guest-initiated: forward tunnels on stream kind L4_FORWARD on PortBound
     - Host-initiated: session.wire.forward_port(guest_port, host_port) -> Result<u16, WireError>
@@
-    - Guest-initiated tunnels carry L4_FORWARD streams
+    - Guest-initiated tunnels carry L4_FORWARD mux streams
       - Frame header: target_port: u16, connection_id: u64, payload_len: u32
```

```diff
--- a/08-lifecycle.md
+++ b/cans/08-lifecycle.md
@@ -Pre-teardown-flush
     - Graceful path
-      - Host issues MSG_SHUTDOWN_REQUEST{timeout_ms} on Channel 0x0000
+      - Host issues MSG_SHUTDOWN_REQUEST{timeout_ms} on Control 0x0000
         - Default timeout: 10s; caller-tunable
         - timeout_ms=0 skips graceful and jumps to abrupt
@@
     - Resume executes cycle-exact upon external release signal
+      - Release signal arrives on Control 0x0000
```

```diff
--- a/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -Scoping-is-transport-agnostic
     - Scoping is transport-agnostic
       - Filters evaluate above the Transport backend (SPSC ring, AF_VSOCK,
         - or named pipe); plugin authors never observe the transport
       - Windows hosts apply identical scoping semantics as Linux/macOS
       - No plugin may key behavior on the transport identity
+    - Stream kinds are registered, not hardcoded
+      - Core kinds frozen; plugin kinds declared via register_stream_kind
+      - see: 11-protocol.md#Control-channel
```

```diff
--- a/03-sdk-architecture.md
+++ b/cans/03-sdk-architecture.md
@@ -Multiplexed-binary-RPC-for-SDK-consumers
       - Multiplexed binary RPC for SDK consumers: session.vsock.channel(kind), binary framing
-        - Channel kind registration: builder.register_channel(kind, handler)
-          - kind drawn from frozen table; see: 11-protocol.md#Channel-allocation-table
+        - Stream kind registration: builder.register_stream_kind(kind, handler)
+          - core kinds frozen; plugin kinds registered at boot
+          - see: 11-protocol.md#Control-channel
         - Programmatic exec surface: session.exec(cmd, limits?)
-          - Writes on Channel 0x0003; framing see: 11-protocol.md#Binary-exec-channel
+          - Writes on stream kind EXEC; framing see: 11-protocol.md#Binary-exec-channel
```

```diff
--- a/15-operations.md
+++ b/cans/15-operations.md
@@ -Channel-coverage
     - --channel-coverage
-      - Asserts every channel in 11-protocol.md#Channel-allocation-table is
-        - exercised by at least one integration test
-      - Specifically asserts 0x0005 (VFS) and 0x0006 (bulk) have live round-trip
-      - A channel with no test fails the gate
+      - Asserts every core stream kind in 11-protocol.md#Control-channel has a
+        - live round-trip integration test
+      - Asserts VFS and BULK stream kinds have live round-trip tests
+      - Asserts at least one plugin-registered stream kind is exercised
+      - A core stream kind with no test fails the gate
```

```diff
--- a/plugins/profile.md
+++ b/cans/plugins/profile.md
@@ -Crate-dependencies
-    - Registers vsock Channel 0x0002 (profile-sync)
-      - Allocation: see 11-protocol.md#Channel-allocation-table for profile-sync
+    - Registers stream kind PROFILE_SYNC on the mux
+      - Kind 0x0010; see: 11-protocol.md#Control-channel
@@ -RFC-6902-dynamic-policy-patching
     - RFC 6902 dynamic policy patching
-      - Ingests JSON/YAML patches via ChannelData on Channel Kind 0x0002 (profile-sync)
+      - Ingests JSON/YAML patches via PROFILE_SYNC stream
```

```diff
--- a/plugins/db-proxy.md
+++ b/cans/plugins/db-proxy.md
@@ -Crate-dependencies
-    - Registers vsock Channel 0x0004 (db-sqlite)
-      - Allocation: see 11-protocol.md#Channel-allocation-table
+    - Registers stream kind DB_SQLITE on the mux
+      - Kind 0x0011; see: 11-protocol.md#Control-channel
     - Lifecycle attachment (db-proxy): see: 08-lifecycle.md#Lifecycle-vetoes
@@ -SQLite-UDS-bridge-mechanics
     - SQLite UDS bridge mechanics:
       - Guest helper binds Unix Domain Socket at /run/kelir/db.sock on demand
-      - Forwards client connections over vsock Channel 0x0004 (db-sqlite)
+      - Forwards client connections over DB_SQLITE mux stream
       - Host SqlProxyService receives stream and delegates queries to host database file
```

```diff
--- a/plugins/mcp.md
+++ b/cans/plugins/mcp.md
@@ -Crate-dependencies
-    - Binds to virtio-console port named "kelir-mcp" (/dev/virtio-ports/kelir-mcp)
+    - Registers stream kind MCP_CONSOLE on the mux
+      - Kind 0x0012; see: 11-protocol.md#Control-channel
+      - Guest shim exposes /run/kelir/mcp.sock to the guest MCP server
@@ -Mapping-external-tool-ring-op
     - Mapping (external tool → ring op)
-      - execute_command → routed through Channel 0x0003 (binary-exec)
+      - execute_command → routed through stream kind EXEC
         - Streams stdout/stderr via separate stream_ids
@@
-    - Port multiplexing routes its console port directly to MCP client without PTY conflict
+    - Stream kind MCP_CONSOLE routes directly to MCP client without PTY conflict
     - MCP contract
-      - Transport: JSON-RPC 2.0 over its virtio-console port
+      - Transport: JSON-RPC 2.0 over MCP_CONSOLE mux stream
       - Framing: JSON Lines; one object per \n, no Content-Length
       - Max frame: 64 KB; larger split via FLAG_CONTINUATION
```

```diff
--- a/plugins/wire-ingress.md
+++ b/cans/plugins/wire-ingress.md
@@ -Invariants-and-policy
-    - Ingress stream framing across vsock:5252 Channel 1:
-      - Channel 1: L4 stream forwarding; see 11-protocol.md#Channel-allocation-table
+    - Ingress stream framing on stream kind L4_FORWARD (mux 0x0001):
+      - Kind 0x0005; see: 11-protocol.md#Control-channel
       - Frame header: target_port: u16, connection_id: u64, payload_len: u32
       - Guest shim ingress forwarder connects to 127.0.0.1:target_port and splices duplex bytes
```

**Frozen-table grief eliminated.** "Adding a row requires spec revision" is gone. Core stream kinds are frozen; plugin kinds register at boot with collision detection. `--channel-coverage` now walks stream kinds. All `Channel 0x000N` / `channel_id` identifiers are dead.

End Part 3/9. Say `continue` for Part 4 (State/memory reconciliation).

## Part 4 / 9 — State & memory reconciliation

**Touched files:** `06-storage-catalog.md`, `01-architecture.md`, `08-lifecycle.md`, `14-observability.md`, `11-protocol.md`, `plugins/db-proxy.md`, `plugins/profile.md`.

Two contradictions resolved:
1. **Ephemeral vs. catalog.** Truth: *all* sessions register a row with a lease. Ephemeral sessions simply use `expires_at = NULL` and rely on OS-native liveness (`pidfd`/`kqueue`/Job Object). SQLite is the SSOT; PID primitives are proof-of-life only.
2. **Fixed 64-slot ExecSlot table.** Replaced with a dynamic slab capped only by the in-VM `RLIMIT_NPROC` and host supervisor's `max_exec_slots` (default 4096).

```diff
--- a/06-storage-catalog.md
+++ b/06-storage-catalog.md
@@ -Entity-model
   - Entity model
     - Minimal 3-table ledger: sessions, cas_leases, schema_version
     - Single Source of Truth for runtime persistence schema
-      - sessions table tracks detached sessions exclusively; ephemeral runs bypass it
-      - Ephemeral session state stays resident in host process memory only
+      - sessions table tracks every session: ephemeral and detached
+      - Ephemeral rows carry expires_at_unix = NULL and lease_kind = 'ephemeral'
+      - Detached rows carry expires_at_unix and lease_kind = 'lease'
+      - Both row kinds are the sole SSOT for session identity and liveness
       - sessions row carries incarnation_id: 128-bit UUIDv7 minted per spawn
       - sessions row carries boot_id, start_time_ns, pid for OS-native proof
       - cas_leases tracks blob_hash, holder_pid, expires_at_unix with 30s TTL
       - schema_version tracks single-digit schema state
       - Lineage parent_digest edges are stored inside immutable snapshot manifests
       - CAS blobs with zero active unexpired leases are safely pruned without locks
-      - Schema evolution drops ephemeral tables; immutable CAS files stay intact
+      - Ephemeral rows are reaped on pidfd/kqueue/Job-Object death without lease wait
+      - Detached rows are reaped on lease TTL expiry
+      - janitor sweeps both kinds on runtime child startup
```

```diff
--- a/01-architecture.md
+++ b/cans/01-architecture.md
@@ -Host-state-root
   - Host state root
     - Content-addressed root: ~/.kelir/
       - catalog.db: embedded SQLite (WAL mode)
-      - Lockless ephemeral path: detached=false sessions bypass catalog.db
-      - Ephemeral sessions allocate zero SQLite transactions or rows
-      - Ephemeral state anchors strictly to parent process lifetime
-      - catalog.db tracks detached sessions and shared CAS blob leases only
+      - Every session registers one catalog.db row: ephemeral or detached
+      - ephemeral rows: expires_at_unix = NULL; lease_kind = 'ephemeral'
+      - detached rows: expires_at_unix set; lease_kind = 'lease'
+      - OS-native liveness primitives are proof-of-life, not SSOT
+        - Linux: pidfd_open + pidfd_send_signal
+        - macOS: kqueue(EVFILT_PROC, NOTE_EXIT)
+        - Windows: Job Object KILL_ON_JOB_CLOSE
+      - parent process death reaps ephemeral rows without lease wait
+      - catalog.db is SSOT for every session; no bypass path
       - cas/blobs/blake3_<hash>: immutable layer storage
       - sessions/<session_id>/: ephemeral runtime state
@@ -Session-ownership-and-leases
     - Session ownership and leases
       - sessions/<id>/owner.pid records the runtime process
       - catalog sessions row carries the liveness lease proof with explicit expiry
       - catalog sessions row carries (incarnation_id, boot_id, start_time_ns, pid)
+      - ephemeral rows use lease_kind='ephemeral' and expires_at_unix=NULL
+      - detached rows use lease_kind='lease' and update expires_at every 30s
       - Incarnation proof semantics: see: 08-lifecycle.md#Session-incarnation
       - Orphaned relay.sock, sparse backing file, staging trees reclaimed by runtime child janitor
```

```diff
--- a/08-lifecycle.md
+++ b/cans/08-lifecycle.md
@@ -Detached-lifecycle
   - Detached lifecycle
     - Affine builder flag .detached(true) spawns a dedicated runtime child
-    - The CLI never manages VM memory directly in detached mode
-    - Runner ownership is explicit
-      - The spawning process inserts an active row into SQLite sessions table
-      - Row carries session_id, runner child pid, and expires_at lease TTL
+    - The CLI never manages VM memory directly in detached mode
+    - Runner ownership is explicit
+      - The spawning process inserts a row with lease_kind='lease'
+      - Row carries session_id, runner child pid, and expires_at lease TTL
       - Linux signaling uses pidfd_open and pidfd_send_signal exclusively
       - Signal delivery verifies host_boot_id and proc start_time before dispatch
       - Stale catalog rows with recycled PIDs abort signal delivery safely
       - Runner updates expires_at every 30s; expired lease triggers cleanup
@@
-  - No resident daemon
-      - Default sessions retain PR_SET_PDEATHSIG / kqueue / Job Object reaping
-      - Detached sessions have no PDEATHSIG; the SQLite lease replaces it
+  - Liveness
+    - Every session row registers lease_kind = 'ephemeral' | 'lease'
+    - Ephemeral: OS-native reap on parent death; no lease TTL
+      - PR_SET_PDEATHSIG, kqueue NOTE_EXIT, Job Object on parent exit
+    - Detached: SQLite lease TTL; no OS-native reap of parent
+    - Both kinds share one row schema; the kind column selects the reaper
@@ -Zombie-reaping
@@ -Session-incarnation
```

```diff
--- a/14-observability.md
+++ b/cans/14-observability.md
@@ -Health-signals
   - Health signals
     - Heartbeat loss marks a guest as stalled or wedged
-    - Parent watchdogs reap orphaned runtime processes
-      - Detached sessions: watchdog in runtime child polls SQLite lease TTL
-      - Ephemeral sessions: Linux pidfd and OS death signals drive zero-delay reaping
-      - Lease heartbeat from the runtime proves liveness; TTL expiry
-        - triggers the janitor
-      - OS kernel death signals terminate backends on host parent exit
-      - Runtime child janitor sweeps expired session directories on startup
+    - Parent watchdogs reap orphaned runtime processes
+      - Every session registers a catalog row with lease_kind
+      - Ephemeral rows: pidfd / kqueue / Job Object drive zero-delay reaping
+      - Detached rows: lease heartbeat updates expires_at; TTL expiry drives reap
+      - Lease heartbeat from the runtime proves liveness for detached rows only
+      - Runtime child janitor sweeps both kinds on startup
+      - Ephemeral rows: row unlinked immediately on parent-process death
+      - Detached rows: row marked Crashed on (boot_id, start_time_ns, pid) mismatch
     - Correlation ids stitch spans across cli, runtime, agentd
```

```diff
--- a/11-protocol.md
+++ b/cans/11-protocol.md
@@ -Guest-paravirtual-micro-init-boundary
     - Guest paravirtual micro-init boundary (kelir-shim)
       - Shim runs as uninstrumented musl PID 1 (<50KB binary)
-      - Fixed 64-slot ExecSlot table maps guest pid to exec_id
-      - ExecSlot saturation: shim returns MSG_EXEC_REJECT{EBUSY} in-band
+      - Dynamic ExecSlot slab maps guest pid to exec_id
+        - Backed by a chunked allocator; frees return chunks to the pool
+        - Sized by min(guest RLIMIT_NPROC, host max_exec_slots)
+        - Host max_exec_slots default 4096; configurable per session
+      - Slab saturation: shim returns MSG_EXEC_REJECT{ENOMEM} in-band
       - waitpid(-1, WNOHANG) updates slot exit status without blocking
       - Unmatched reaped PIDs discard into a zero-allocation sink
       - Exit frames on Channel 0x0003 defer until stdout/stderr EOF flush
       - Multiplexes raw VirtIO ring bytes; carries zero database, proxy, or domain logic
       - Domain Unix sockets are provisioned dynamically over vsock, never hardcoded in PID 1
@@ -Slot-saturation-backpressure
     - Slot saturation backpressure
-      - All 64 ExecSlots occupied: MSG_EXEC_REJECT{reason: EBUSY} returns in-band
-      - No queue growth, no PID 1 termination on burst, no silent drop
+      - Slab allocator returns MSG_EXEC_REJECT{reason: ENOMEM} when pool exhausted
+      - No queue growth, no PID 1 termination on burst, no silent drop
       - Orchestrator retries after slot release or escalates to drain
```

**Invariant outcome:**
- **One truth:** `catalog.db` is the SSOT for *every* session. There is no bypass path.
- **Two reapers:** `lease_kind` selects the mechanism. Ephemeral is OS-native death; detached is lease TTL. No table is bypassed; no live session is invisible to the watchdog.
- **ExecSlot ceiling removed:** dynamic slab bounded by `RLIMIT_NPROC` and `max_exec_slots`. The 64-slot number is gone from the spec.

End Part 4/9. Say `continue` for Part 5 (Storage plane: kill the FUSE/overlay hybrid).



## Part 5 / 9 — Storage plane: single abstraction

**Decision: Path A (host-driven passthrough).** In-guest OverlayFS is deleted. The host storage broker owns CoW layering, deletion tracking, quota, and commit. Guest sees one VirtIO-FS mount. Whiteout parsing and `renameat2` staging are removed.

**Touched files:** `05-storage-plane.md`, `06-storage-catalog.md`, `09-snapshots.md`, `12-adapters.md`, `plugins/transaction.md`, `plugins/container.md`.

```diff
--- a/05-storage-plane.md
+++ b/cans/05-storage-plane.md
@@ -Storage-architecture
   - Storage architecture
     - Storage plane attributes
       - see: artifacts/diagrams/storage-architecture.txt
       - Owner: kelir-core native storage subsystem (kelir-core::storage)
-    - Interception pipeline
-      - Low-level FUSE pass-through bridge directly linking guest VFS to host workspace
-      - Pipeline engine exposes synchronous lookup, read, and write interception
-      - Emits physical StorageDelta events directly to host event bus on commit
+    - Single storage abstraction
+      - VirtIO-FS only; no in-guest OverlayFS, no virtio-blk workspace disk
+      - Host broker implements CoW layering, deletion tracking, quota, commit
+      - Guest sees one POSIX filesystem; zero union-driver behavior in guest
+    - Interception pipeline
+      - FUSE bridge directly links guest VFS to host broker
+      - Pipeline engine exposes synchronous lookup, read, and write interception
+      - Emits physical StorageDelta events directly to host event bus on commit
     - Host I/O invariants
       - Mandatory non-blocking host I/O: host descriptors open with O_NONBLOCK
       - Host FIFO and socket rejection: S_IFIFO and S_IFSOCK fail lookup with ENXIO
       - Host blocking prevention: zero guest syscalls may sleep host ring workers
@@ -EROFS-rootfs
   - EROFS rootfs
     - Stripped base system pre-baked into read-only EROFS
     - Cold-mounted as virtio-blk block device in <2ms
     - Zero tarball decompression
-  - CoW overlay scratchpad
-    - Base workspace mounted strictly read-only as lower layer
-    - overlayfs upperdir maps a sparse host-backed file via virtio-blk
-    - Rollback page purge: FALLOC_FL_PUNCH_HOLE follows with posix_fadvise
-    - posix_fadvise(POSIX_FADV_DONTNEED) purges host kernel dirty page cache
-    - Writes hit sparse disk blocks, never guest RAM
-    - Instant revert: truncate sparse backing file or punch holes in <1ms
-    - No guest kernel OOM on large writes (npm install, pip install torch, git clone)
-    - Host memory footprint independent of guest write volume
+  - Host CoW broker
+    - Base workspace mounted strictly read-only as host broker lower layer
+    - Writes land in per-session sparse upper file owned by the broker
+    - Deletions recorded as explicit tombstones in broker state (no whiteouts)
+    - Guest observes a normal writable mount; no union driver inside microVM
+    - Broker exposes lookup/read/write/getattr/readdir/unlink/mkdir/rename
+    - Dir merges (base + upper + tombstones) computed host-side per readdir
+    - Rollback: broker drops upper file and tombstones; base untouched
+    - Rollback purge: FALLOC_FL_PUNCH_HOLE + posix_fadvise(DONTNEED) on upper
+    - Instant revert: <1ms; guest kernel never sees a filesystem restart
+    - No guest kernel OOM on large writes (npm install, pip install torch)
+    - Host memory footprint independent of guest write volume
+    - Sparse upper file bytes counted toward the session quota
   - Ephemeral scratch device
     - Dedicated virtio-blk sparse scratch disk mounted at /tmp and /var/cache
     - Isolates high-churn dependencies from workspace git diffs
     - Automatically discarded on session teardown without reconciliation
     - Prevents cache pollution and unneeded whiteout tracking on host root
-  - Atomic workspace reconciler
-    - Detect
-      - Scans sparse upperdir diffs; unmodified base files remain untouched
-    - Classify whiteouts
-      - Deleted files: char devices with st_rdev == 0 trigger unlink on host target
-      - Deleted or opaque dirs: trusted.overlay.opaque="y" triggers host remove_dir_all
-    - Stage
-      - Writes mutated files to sibling temporary files (.tmp.<nonce>)
-      - StagingGuard struct owns all .tmp.<nonce> files
-      - Staging cleanup: uncommitted staging files unlink unconditionally on Drop
-      - Disarm on commit: guard disarms only after leaf-level atomic commit succeeds
-    - Commit
-      - Reconcile operates on leaf and intermediate directory level exclusively
-      - Host workspace root and intermediate directory inodes are never swapped
-      - Mutated files replace via platform-native single-file atomic rename
-        - Linux: renameat2; macOS: renameatx_np(RENAME_SWAP); Windows:
-        - SetFileInformationByHandle(FileRenameInfoEx)
-      - Deleted files unlink in place; new subdirectories create in place
-      - Host Inode Broker pre-checks active host flock descriptors
-      - Fallback on EBUSY or EPERM: in-place write with 2000ms backoff
-    - Contain
-      - Strict containment: rejects symlink hops and traversals outside host root
-      - Host containment engine resolves via platform-native root handle
-        - Linux: openat2(RESOLVE_BENEATH|RESOLVE_NO_MAGICLINKS)
-        - macOS: iterative openat(O_NOFOLLOW) with st_dev/st_ino root verification
-        - Windows: FILE_FLAG_OPEN_REPARSE_POINT against declared root handle
-      - Symlink traversals escaping root fail with EXDEV at host VFS layer
-      - S_ISUID and S_ISGID stripped on every write, create, chmod (mode & !0o6000)
-      - In-guest root cannot mint setuid host binaries
+  - Workspace commit
+    - Trigger: session.storage.reconcile() or session.stop() graceful flush
+    - Broker replays upper file + tombstones against the base workspace
+    - Per-entry atomic commit at leaf level: temp file + platform rename
+      - Linux: renameat2; macOS: renameatx_np(RENAME_SWAP); Windows:
+      - SetFileInformationByHandle(FileRenameInfoEx)
+    - Deletes unlink in place; new dirs create in place
+    - Host Inode Broker pre-checks active host flock descriptors
+    - Fallback on EBUSY or EPERM: in-place write with 2000ms backoff
+    - Whole commit is idempotent: replaying after crash converges
+  - Containment
+    - Strict containment: rejects symlink hops and traversals outside host root
+    - Host containment engine resolves via platform-native root handle
+      - Linux: openat2(RESOLVE_BENEATH|RESOLVE_NO_MAGICLINKS)
+      - macOS: iterative openat(O_NOFOLLOW) with st_dev/st_ino root verification
+      - Windows: FILE_FLAG_OPEN_REPARSE_POINT against declared root handle
+    - Symlink traversals escaping root fail with EXDEV at host VFS layer
+    - S_ISUID and S_ISGID stripped on every write, create, chmod (mode & !0o6000)
+    - In-guest root cannot mint setuid host binaries
```

```diff
--- a/05-storage-plane.md
+++ b/cans/05-storage-plane.md
@@ -Synthetic-FUSE-inodes
   - Synthetic FUSE inodes
     - Injects RAM-only configuration files (.env, .gitconfig) into guest VFS
     - Lookups return synthetic FsInodeAttr (mode: 0o400 honored strictly)
     - Served directly from host supervisor memory; physically absent on host SSD
+    - Synthetic inodes overlay the host CoW broker namespace at lookup
@@ -Storage-quotas
   - Storage quotas
     - Tracked state
-      - kelir-core::storage maintains allocated_bytes and allocated_inodes atomics
+      - Host broker maintains allocated_bytes and allocated_inodes atomics
+      - Counters track sparse upper file bytes + tombstone count + new inodes
     - Enforcement
       - on_fs_write and FUSE mknod/create/mkdir gate against MountScope budget
-      - statfs reporting: returns remaining upperdir quota in O(1) time
+      - statfs reporting: returns remaining broker quota in O(1) time
     - Isolation semantics
-      - Quota isolation: quotas evaluate exclusively against sparse upperdir delta
-      - Zero-crawl baseline: lowerdir host workspace incurs zero statfs traversal
+      - Quota isolation: quotas evaluate exclusively against broker delta state
+      - Zero-crawl baseline: base workspace incurs zero statfs traversal
     - Failure behavior
       - Exceeding max_bytes fails synchronously with ENOSPC
       - Exceeding max_inodes fails synchronously with ENOSPC
       - Zero disk writes occur on quota rejection
     - Blast radius
       - Host filesystem and neighboring microVMs remain protected
   - Secondary image mounts
     - Grafts host-opened squashfs/EROFS images into guest VFS namespace
     - Zero PCI hot-plug overhead; serves pack lookups directly via FUSE
-    - Prevents tmpfs upperdir memory exhaustion when loading large packages
+    - Prevents broker upper exhaustion when loading large packages
@@ -Virtual-ID-remapping
   - Virtual ID remapping
     - FUSE driver dynamically maps guest UID 0/1000 to caller process UID
```

```diff
--- a/06-storage-catalog.md
+++ b/cans/06-storage-catalog.md
@@ -Entity-model
   - Entity model
     - Minimal 3-table ledger: sessions, cas_leases, schema_version
     - Single Source of Truth for runtime persistence schema
@@
       - Lineage parent_digest edges are stored inside immutable snapshot manifests
       - CAS blobs with zero active unexpired leases are safely pruned without locks
       - Ephemeral rows are reaped on pidfd/kqueue/Job-Object death without lease wait
       - Detached rows are reaped on lease TTL expiry
       - janitor sweeps both kinds on runtime child startup
+    - No whiteout, overlay, or union-driver state is ever persisted
+      - Host CoW broker state lives under sessions/<id>/upper and /tombstones
+      - The broker's state is ephemeral by construction; catalog stores no rows
+      - Commit reads broker state, writes to workspace, then unlinks broker state
```

```diff
--- a/09-snapshots.md
+++ b/cans/09-snapshots.md
@@ -Archives-and-portability
   - Archives and portability
     - Export format: single Content-Addressed .kelir-snap binary stream
       - Archive header: magic, format version, feature flags, entry table
       - Stream contents: manifest.json, vmm_state.json, ram.zst, upperdir.tar.zst,
         - guest_clock.json, attestation.jws, tls_ca.jws
+      - upperdir.tar.zst is broker state: sparse upper file + tombstones
+      - No whiteouts, no overlay metadata, no union driver artifacts
       - tls_ca.jws carries vault CA cert + key under session key
@@ -Unpack-containment-invariants
     - Unpack containment invariants (Single Source of Truth)
       - Untrusted input boundary: zero ambient host filesystem trust
-        - Containment engine: 05-storage-plane.md#Atomic-workspace-reconciler
+        - Containment engine: 05-storage-plane.md#Containment
         - Traversal, absolute paths, and symlink escapes reject with EXDEV there
         - Sparse entries pre-sized; zstd frames bounded by declared size
         - Codec errors discard staging atomically; no partial roots leak
```

```diff
--- a/12-adapters.md
+++ b/cans/12-adapters.md
@@ -Phase-2
     - Phase 2: layer payload bodies stream into CAS blobs
       - Finalized EROFS data and directory blocks emitted from Phase 1 map
+    - Emits a session-ready EROFS image; workspace CoW is host-broker state
+      - No overlay or union driver is materialized by kelir-pack
```

```diff
--- a/plugins/transaction.md
+++ b/cans/plugins/transaction.md
@@ -Invariants-and-policy
     - Multi-resource 2PC coordination
-      - Synchronizes commit/abort across native Storage Plane, db-proxy, and lock broker
+      - Synchronizes commit/abort across host CoW broker, db-proxy, and lock broker
       - Coordinates with hypervisor lifecycle: pauses vCPUs during commit evaluation
-      - on_pre_commit pauses guest vCPUs, flushes VFS buffers, and prepares staging files
-      - on_post_commit(true) executes atomic directory swap into host workspace
+      - on_pre_commit pauses guest vCPUs and drains FUSE in-flight ops
+      - on_post_commit(true) invokes broker commit: replay upper + tombstones
         - and issues SQL COMMIT across active proxy connections
-      - on_post_commit(false) discards the sparse overlay delta, drops staging directories,
+      - on_post_commit(false) discards the broker upper file and tombstones,
         - issues SQL ROLLBACK, and releases advisory locks
       - Prevents desynchronization between host database state and guest workspace diffs
@@ -Temporal-savepoints
     - Temporal savepoints
       - Injects transparent SQL SAVEPOINT markers into active pgwire connections
-      - Marks VFS upperdir whiteout journals to isolate sub-command mutations
+      - Snapshots broker upper and tombstone state as opaque savepoint blobs
       - Rollback to savepoint restores intermediate state without aborting parent transaction
```

```diff
--- a/plugins/container.md
+++ b/cans/plugins/container.md
@@ -Invariants-and-policy
     - Non-nested OCI container execution
       - Configures rootless Podman and crun runtime environments inside microVM
-      - Mounts ephemeral tmpfs scratchpad for /var/lib/containers to isolate storage diffs
+      - Mounts ephemeral scratch device for /var/lib/containers to isolate storage diffs
       - Constructs synthetic cgroup v2 delegation hierarchy at on_vm_ready
```

**Outcomes:**
- **One storage abstraction.** Guest is a POSIX filesystem. Host broker does CoW.
- **No OverlayFS in guest.** No whiteout parsing, no union driver, no `trusted.overlay.opaque` string anywhere.
- **No `renameat2` staging Rube Goldberg.** Broker writes directly to workspace at commit time via leaf-level atomic replace.
- **Snapshots carry broker state, not overlay state.** `upperdir.tar.zst` = broker upper + tombstones.
- **Containment gets its own node.** No hidden coupling to the deleted reconciler node.
- **2PC still works.** It now coordinates `broker.commit()` instead of a non-existent staging directory.

**`05-storage-plane.md` node deletions:** `CoW overlay scratchpad`, `Atomic workspace reconciler` (Detect/Classify/Stage/Commit/Contain). **New nodes:** `Host CoW broker`, `Workspace commit`, `Containment`.

End Part 5/9. Say `continue` for Part 6 (PMU demotion + `10-epistemic.md`).

## Part 6 / 9 — PMU demotion + `10-epistemic.md` rename

**Decision:** File renamed to `10-attestation.md`. PMU counters become diagnostic gauges only. Execution receipts derive determinism from logical event traces (syscall log, I/O streams, seeded RNG, frozen clock). No "cross-host deterministic counter equivalence" claim anywhere.

**Touched files:** `10-epistemic.md` → `10-attestation.md`, `00-overview.md`, `01-architecture.md`, `14-observability.md`, `15-operations.md`, `_rules.yaml`, `plugins/replay.md`, `plugins/ledger.md`, `plugins/chaos.md`, `plugins/mesh.md`, `plugins/tool-guard.md`, `plugins/vault.md`, `plugins/profile.md`, `plugins/db-proxy.md`, `plugins/supply-chain.md`, `plugins/container.md`, `plugins/mcp.md`, `plugins/wire-ingress.md`, `plugins/commercial.md`, `12-adapters.md`, `03-sdk-architecture.md`.

```diff
--- a/cans/10-epistemic.md
+++ b/cans/10-attestation.md
@@
-- Hardware Telemetry & Cryptographic Verification
+- Hardware Attestation & Verification
   - Scope
-    - Telemetry ownership: Hypervisor core measures physical hardware events exclusively
-    - Fact emission: TrapEvent, ExecutionReceipt, CausalDelta
-    - Hardware counters: CPU instructions, dirty pages, MMU faults, wire byte volume
+    - Attestation ownership: hypervisor core measures physical hardware events
+    - Fact emission: TrapEvent, ExecutionReceipt, CausalDelta, AttestationToken
+    - Attestation primitives: vmlinux BLAKE3, EROFS BLAKE3, session token
+    - Diagnostic metrics: PMU counters, dirty bytes, wire volume (gauges only)
     - Boundary opacity: Internal interception policies surface to the guest strictly
       - as physical bus errors, standard POSIX errnos, or silent packet drops
-  - Primitive vs derived telemetry
-    - Telemetry decision test
-      - Single read API, hardware boundary, no policy → primitive, core-owned
-      - Computed from primitives, policy-bearing → derived, plugin-owned
-    - Default posture: per-exec, per-trap, per-receipt primitives enabled
+  - Primitive vs derived metrics
+    - Metric decision test
+      - Single read API, hardware boundary, no policy → primitive, core-owned
+      - Computed from primitives, policy-bearing → derived, plugin-owned
+    - Default posture: per-exec, per-trap, per-receipt primitives enabled
   - Execution receipts
     - Receipt foundation
       - see: artifacts/code/execution-receipt.rs
-      - Deterministic hardware counter attribution emitted per executed process
+      - Deterministic logical-trace attribution emitted per executed process
       - Emitted per executed process, at the trap/exit boundary
       - Receipts compare as values via canonical per-field structural assertions
       - Receipts carry opaque TelemetryExtension payloads emitted by mounted plugins
     - Identity
-      - Hardware attestation token: BLAKE3 cryptographic proof over execution tuple
+      - Execution receipts are deterministic because inputs are frozen
+        - Frozen virtual clock (see: plugins/replay.md#Kernel-clocksource-lockdown)
+        - Seeded ChaCha20 RNG (see: plugins/replay.md#Deterministic-RNG)
+        - Ordered syscall trace (intercepted argv, retval, errno)
+        - Ordered I/O streams (stdin/stdout/stderr bytes)
+      - No receipt field claims cross-host counter equivalence
       - binary_blake3 identity
         - Canonical form
           - BLAKE3 of raw file bytes at binary_path
@@
     - Field semantics
       - Population
         - Per ReceiptConfig; opt-in fields not enabled are absent (Option), never 0
-        - retired_instructions: sampled from host vCPU thread via perf_event_open
-          - Linux: perf_event_open with exclude_host=1 and exclude_hypervisor=1
-          - Sampled strictly during active KVM_RUN state
-          - macOS ARM64: read via hv_vcpu_get_sys_reg from PMCCNTR_EL0 / PMEVCNTR<n>_EL0
-          - Mach thread counters banned; host hypervisor overhead excluded
-          - Guarantees cross-host deterministic counter equivalence
+        - retired_instructions: diagnostic metric, not a receipt identity field
+          - Linux: perf_event_open with exclude_host=1, exclude_hypervisor=1
+          - macOS ARM64: hv_vcpu_get_sys_reg from PMCCNTR_EL0 / PMEVCNTR<n>_EL0
+          - Sampled strictly during active KVM_RUN state
+          - Emitted to OTel as a gauge; excluded from receipts and Merkle roots
+          - Never used to compare execution across hosts
         - exit_kind: ExitKind enum, always populated
           - Normal | Signal(u32) | KernelPanic(String) | HypervisorFault(String)
           - KernelPanic payload scraped from 64KB early-boot UART buffer
             - see: 01-architecture.md#Early-boot-diagnostics
           - Replaces reliance on exit_code alone for boot failure classification
         - Opt-in: syscall_count
-        - dirty_bytes is always populated: zero is a real measurement, not absent
+        - dirty_bytes is a receipt field: byte count, not page count
+          - Deterministic because it counts guest writes, not host page churn
         - Byte-not-page reason: see: 01-architecture.md#SnapshotDescriptor.dirty-bytes
@@
         - phase spans
           - trap_ingress_ns: BPF decision to host handler entry
           - ffi_hop_ns: time in foreign FFI boundary
           - policy_ns: time in plugin pipeline after FFI returns
           - verdict_egress_ns: verdict decision to guest re-exec
-          - Sum + base overhead = wall_clock_ns
+          - Diagnostic only: spans emit to OTel; excluded from Merkle roots
+          - Sum + base overhead = wall_clock_ns (wall_clock only)
@@
   - Causal delta
     - see: artifacts/code/causal-delta.rs
     - Definition: Δ = S_A − S_B between two checkpoint descriptors
     - Delta fields: see: artifacts/code/causal-delta.rs
-    - State diff aggregation: CausalDelta aggregates net physical diffs
+    - State diff aggregation: CausalDelta aggregates net logical diffs
+      - Filesystem: broker upper file + tombstones (see: 05-storage-plane.md)
+      - Memory: dirty bytes recorded by the checkpoint descriptor
+      - Process: process tree edges and exit statuses
     - Merkle root chaining links granular receipts to state diffs
-    - Tracks physical page deltas, modified inodes, and process lineage
+    - No PMU samples appear in CausalDelta
     - L7 protocol telemetry lives outside core structs, in extensions
@@
-  - Telemetry provenance classification
+  - Attestation provenance classification
     - Invariant: A higher classification never falls back to a lower classification
-    - Class 1 (Hardware Sampling): PMU instruction counters, dirty byte counts,
-      - physical exit codes, hardware TSC timestamps. Unforgeable from guest space.
-    - Class 2 (Paravirtual Interception): FUSE buffers, virtqueue frames, character rings.
-      - Observed at the hypervisor boundary.
-    - Class 3 (Guest Unverified Data): Workload stdout/stderr payloads, memory buffers.
-      - Tainted by default; cannot satisfy hardware assertions.
+    - Class 1 (Hardware-Anchored): vmlinux BLAKE3, EROFS BLAKE3, session token,
+      - guest clock reads, guest exit codes. Unforgeable from guest space.
+    - Class 2 (Paravirtual Interception): FUSE buffers, virtqueue frames,
+      - character rings. Observed at the hypervisor boundary.
+    - Class 3 (Guest Unverified Data): workload stdout/stderr, memory buffers.
+      - Tainted by default; cannot satisfy hardware assertions.
     - Selection rule
       - Claim must resist forgery by guest code → Tier 1
       - Claim about a public, versioned contract → Tier 2
@@
   - Lifecycle bridge
@@
-  - Step evaluation loop
+  - Step evaluation loop
     - Orchestrators execute unmodified agent binaries
     - Checkpoint descriptors are retained in userland orchestrator state
-    - Post-step verification compares physical receipts against pre-step states
+    - Post-step verification compares logical receipts against pre-step states
+      - syscall trace, I/O streams, seeded RNG output, frozen clock
     - StepSlot semantics
@@
       - Process exit triggers post-hook: physical ExecutionReceipt assertion
+        - "Physical" here means host-observed, not PMU-derived
     - Mismatch triggers hypervisor rollback
         see: artifacts/code/vmm-driver-trait.rs
```

```diff
--- a/cans/00-overview.md
+++ b/cans/00-overview.md
@@ -Spec-map
-    - Verification and hardware telemetry: see: 10-epistemic.md
+    - Hardware attestation & verification: see: 10-attestation.md
```

```diff
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@ -Strict-hardware-kernel-abstraction-layer
-        - Factoring: see: 10-epistemic.md#Scope
+        - Factoring: see: 10-attestation.md#Scope
@@ -Core-invariants
-    - Receipts: see: 10-epistemic.md#Execution-receipts
+    - Receipts: see: 10-attestation.md#Execution-receipts
@@ -Trust-chain
       - Workload
-        - binary_blake3 per ExecutionReceipt; see: 10-epistemic.md#Execution-receipts
+        - binary_blake3 per ExecutionReceipt; see: 10-attestation.md#Execution-receipts
```

```diff
--- a/cans/14-observability.md
+++ b/cans/14-observability.md
@@ -Metrics-pipeline
       - Traces: one span per receipt phase (trap_ingress, policy, ffi_hop,
-        - verdict_egress); see: 10-epistemic.md#Execution-receipts
+        - verdict_egress); see: 10-attestation.md#Execution-receipts
       - Trace continuity across guest-originated requests via W3C traceparent
       - No local Merkle WAL is written by default
       - Local disk audit requires an explicit EventBus subscriber
+    - PMU counters are gauges only
+      - retired_instructions, cycle counts, cache misses emit as OTel gauges
+      - Never enter receipts, CausalDelta, or Merkle leaves
+      - Never compared across hosts; microarchitectural variance is expected
```

```diff
--- a/cans/15-operations.md
+++ b/cans/15-operations.md
@@ -Default-posture
     - Default posture
       - Primitive telemetry on; derived telemetry off unless configured
       - Ecosystem apparatus gated on registry size; not shipped ahead of it
-      - see: 10-epistemic.md#Primitive-vs-derived-telemetry
+      - see: 10-attestation.md#Primitive-vs-derived-metrics
```

```diff
--- a/cans/_rules.yaml
+++ b/cans/_rules.yaml
@@
 stopwords: [the, a, an, of, to, in, for, and, or, with, must, shall, requires, host, guest, plugin, on, is, kelir, virtio, via, no, zero, at, session]
```
```diff
--- a/cans/plugins/replay.md
+++ b/cans/plugins/replay.md
@@ -Invariants-and-policy
     - Cycle-exact instruction clock stepping
       - Decouples guest paravirtual clock from host physical TSC
-      - Arms host PMU hardware counters to sample instructions retired per vCPU
-      - Advances guest KVM_SET_CLOCK / CNTVOFF_EL2 strictly on retired-instruction windows
+      - Arms host PMU counters as a metric-only drive for clock stepping
+      - Advances guest KVM_SET_CLOCK / CNTVOFF_EL2 on retired-instruction windows
+      - PMU values drive local stepping only; never appear in receipts
       - Eliminates host scheduling jitter and timer interrupt variance across runs
```

```diff
--- a/cans/plugins/ledger.md
+++ b/cans/plugins/ledger.md
@@ -Invariants-and-policy
         - Record Schema (Packed Little-Endian):
           - seq: u64 (monotonic index)
           - prev_hash: [u8; 32] (BLAKE3 hash of previous leaf)
           - receipt_blake3: [u8; 32] (ExecutionReceipt digest)
           - delta_root: [u8; 32] (StorageDelta modified inode tree root)
           - timestamp_ns: u64 (host wall-clock timestamp)
           - signature: [u8; 64] (Ed25519 signature by host session key)
+        - No PMU samples appear in any leaf
+          - Determinism comes from logical trace, seeded RNG, frozen clock
```

```diff
--- a/cans/plugins/commercial.md
+++ b/cans/plugins/commercial.md
@@ -hyper-density
     - hyper-density
       - Tier 3 aggregator over native lifecycle primitives
@@
   - cas-snapshot
@@
   - oci-translator
@@
   - openshell-coordinator
@@
-  - Adapter binaries
-    - containerd-shim-v2, openshell-grpc
-    - Boundary and ownership: see: 12-adapters.md#Process-vs-plugin-boundary
+  - Adapter binaries
+    - containerd-shim-v2, openshell-grpc
+    - Boundary and ownership: see: 12-adapters.md#Process-vs-plugin-boundary
```

```diff
--- a/cans/12-adapters.md
+++ b/cans/12-adapters.md
@@ -E2B-compatibility
     - Process vs plugin boundary: thin client wrapper, no in-guest shims or daemons
```

```diff
--- a/cans/03-sdk-architecture.md
+++ b/cans/03-sdk-architecture.md
@@ -Cross-SDK-guarantees
         - ReceiptConfig: Py/RS include_syscall_count
           - TS: includeDirtyBytes, includeBinaryBlake3, includeSyscallCount
+        - Receipts are deterministic across hosts when replay is active
+          - Frozen clock, seeded RNG, ordered syscall + I/O traces
+          - No PMU-derived field is compared cross-host
         - Decision static constructors (Py/RS) vs c.deny/c.resume/c.rewrite (TS)
```

```diff
--- a/cans/plugins/mesh.md
+++ b/cans/plugins/mesh.md
@@ -Invariants-and-policy
     - Propagates call-hop counters across routines
-    - Traceparent continuity: 14-observability.md#Metrics-pipeline
+    - Traceparent continuity: 14-observability.md#Metrics-pipeline
```

```diff
--- a/cans/plugins/tool-guard.md
+++ b/cans/plugins/tool-guard.md
@@ -Invariants-and-policy
@@
     - Output scrubber: scans tool execution responses on return;
         strips host secrets before model context ingests bytes
```

**Rename mechanics:**

1. `git mv cans/10-epistemic.md cans/10-attestation.md` — content patched as above.
2. Every `see: 10-epistemic.md` → `see: 10-attestation.md` across the corpus (7 files, ~10 references).
3. `cans/_rules.yaml` `references.external_roots` and `00-overview.md` spec map updated.
4. **Deleted claims:** "cross-host deterministic counter equivalence", "bitwise identical receipts across hosts", "hardware counter attribution emitted per executed process".
5. **New receipt determinism contract:** frozen clock + seeded RNG + ordered syscall trace + ordered I/O streams. PMU is a gauge, full stop.

**Verification gate for CI:** `cans check` must fail if any file in `cans/` still mentions `epistemic` or `perf_event_open` as a receipt field.

End Part 6/9. Say `continue` for Part 7 (SDK typestate simplification).

## Part 7 / 9 — SDK typestate simplification

**Decision:** Typestates stay in Rust only. TypeScript and Python get idiomatic flat builders with runtime dependency-graph validation at `boot()`. No TypeBox recursive generics, no PEP 695 protocol intersections, no `Has<T>` trait-bound gymnastics in userland code. `builder.boot()` is the atomic validation gate for all three SDKs.

**Touched files:** `03-sdk-architecture.md`, `15-operations.md`, `02-plugin-model.md`.

```diff
--- a/cans/03-sdk-architecture.md
+++ b/cans/03-sdk-architecture.md
@@ -N-native-design
   - N-native design
     - Foundation
       - No dynamic any-typing, no second-class shims
-      - All SDKs enforce identical runtime guarantees through native type systems
+      - All SDKs enforce identical runtime guarantees at builder.boot()
       - Bootstrap: Rust, TypeScript, Python
       - Adding a language: bindings crate + vector consumer + surface manifest
       - Runtime safety matrix: see: artifacts/diagrams/sdks-invariants.txt
       - Failure occurs at the earliest lifecycle phase native to each runtime
-      - Affine builder consumption: every fluent verb consumes the previous
-        - builder by value; no builder instance is reused, branched, or mutated
-      - Compile-time prerequisite gating: a plugin declaring a prerequisite
-        - without its dependency installed fails at type-check, not boot
-      - Zero dynamic service lookups: handler context c statically intersects
-        - each installed plugin's surface; no string keys, no runtime downcast
+    - Builder consumption model (per language)
+      - Rust: affine typestate; .plugin() consumes self by value
+      - TypeScript: fluent flat builder returning this; registry is an
+        - ordered list; no recursive generic accumulation
+      - Python: fluent flat builder returning self; registry is an
+        - ordered list; no Protocol intersection
+    - Prerequisite gating (per language)
+      - Rust: compile-time, via Has<T> trait bounds and typestate
+      - TypeScript: runtime check at builder.boot() before any vCPU start
+      - Python: runtime check at builder.boot() before any vCPU start
+      - All three produce the same structured error code: EConfig
+      - Runtime check cost: O(plugins) at boot; <2µs; validated once
+    - Service lookup (per language)
+      - Rust: c.ctx::<T>() infallible, statically resolved
+      - TypeScript: c.<pluginKey> typed by .use(plugin) overloads;
+        - resolves the plugin's declared context shape; no string keys
+      - Python: c.cap.<plugin_key> typed by .use(plugin) overloads;
+        - resolves the plugin's declared context shape; no string keys
+      - No string-keyed service registry on any SDK
     - Type-safety
       - Type-safety invariants
         - Rust borrow checker and Serde perimeter validation: see: #Rust-SDK
-        - TypeScript tsc and TypeBox AOT native validators: see: #TypeScript-SDK
-        - Python Pydantic v2 frozen models, import-time checks: see: #Python-SDK
+        - TypeScript tsc + TypeBox runtime validators at builder.boot()
+        - Python Pydantic v2 frozen models, validated at builder.boot()
         - AOT hook fusion and FFI crossing: see: 02-plugin-model.md#Foreign-plugin-proxy
       - Filter-before-FFI declaration
         - Declared filters register during builder plugin mount
         - Evaluation semantics: see: 02-plugin-model.md#Filter-before-FFI-gate
     - Runtime surface
-      - Language surfaces
-        - Rust: extension traits provide zero-cost fluent chaining on builder
-        - TypeScript: generic context accumulation decorates fluent plugin verbs
-        - Python: dynamic protocol mixins inject fluent verbs on mount
-        - Chained verbs appear on builder instance only when plugin is mounted
+      - Language surfaces
+        - Rust: extension traits provide zero-cost fluent chaining on builder
+        - TypeScript: fluent builder; plugin verbs are overloads of .use()
+        - Python: fluent builder; plugin verbs are overloads of .use()
+        - Verbs are available after .use() in all languages (typed in Rust,
+          - returned by overload in TS/Py)
@@
-      - Plugin extension pattern
-          - Optional domain plugins attach via typed extensions
-          - Rust: session.ctx::<T>() infallible via `Session<Plugins, Params>`
-            - where `Plugins: Has<T>`; no Result, no `ext`/`extension` surface
-          - TypeScript: session.<pluginKey> is a static member of
-            - Session<TContext, TParams>; no session.use(ext)
-          - Python: session.cap.<plugin_key> is statically projected by the
-            - PEP 695 Protocol intersection; no session.ext(T)
-          - Inter-plugin capabilities resolve via c.ctx::<T>() (Rust),
-            - c.<pluginKey> (TS), c.cap.<plugin_key> (Python)
+      - Plugin extension pattern
+        - Optional domain plugins attach via typed extensions
+        - Rust: session.ctx::<T>() infallible via `Session<Plugins, Params>`
+          - where `Plugins: Has<T>`; no Result, no `ext`/`extension` surface
+        - TypeScript: session.<pluginKey> is typed by the .use() overload
+          - for that plugin; no session.use(ext), no session.ext(T)
+        - Python: session.cap.<plugin_key> is typed by the .use() overload
+          - for that plugin; no session.ext(T)
+        - Inter-plugin capabilities resolve via c.ctx::<T>() (Rust),
+          - c.<pluginKey> (TS), c.cap.<plugin_key> (Python)
```

```diff
--- a/cans/03-sdk-architecture.md
+++ b/cans/03-sdk-architecture.md
@@ -Rust-SDK
   - Rust SDK
     - see: artifacts/code/rust-handle-injection.rs
     - Typestate transitions: KelirBuilder<Unsealed, Plugins, Params> to
       - KelirBuilder<Sealed, Plugins, Params> to Session<Plugins, Params>
     - Every verb consumes self by value: .plugin(), .params(), .trap(), .boot()
@@
     - Scope constraint via MountOptions: rings, binaries, paths
   - TypeScript SDK
     - see: artifacts/code/typescript-accumulation.ts
-    - KelirBuilder<TContext, TParams, TInstalledPlugins> carries the accumulated
-      - surface in type parameters; every verb returns a new builder type
-    - .use(plugin) intersects TPluginCtx into TContext and unions TPluginId
-      - into TInstalledPlugins; .use() takes flat plugin instances, no callbacks
-    - Prerequisite gate: a missing dependency yields
-      - CompileError<"Missing ...">, so no method (including .params and .trap)
-      - is callable on the returned type
-    - .trap(matcher, handler): simple pre-exec gate returning a Decision
-    - .trap(matcher, { pre, post }): dual pre/post gate; post returns a
-      - PostExecAction
-    - .intercept(channel, filter, handler): ring-level synchronous hot-path
-      - filters returning SyncVerdict
-    - .on(event, handler): lifecycle interceptors over LifecycleEvent values
-    - .onTrap(handler): plugin-level trap interposer
-      - Returns a new plugin; the plugin owns on_trap for its binaries
-      - Builder .trap() remains for one-off gates without a plugin
-    - .params(schema) pins TParams = Static<TSchema>; handler c.params is typed
-      - from that parameter, not from a runtime schema lookup
-    - Handlers receive HandlerContext<TContext, TParams>; plugin surfaces are
-      - static members (c.policy, c.ledger), never c.service(Token)
-    - AOT hook fusion compiles all TS pipeline handlers into a single flat closure
-    - Invariant: exactly 1 NAPI trampoline jump per event regardless of depth
-    - Fully unified TContext passes to callbacks; cross-plugin reads are static
-      - member accesses, not service-token resolution
-    - Wraps async TS handlers in ForeignPluginProxy at NAPI boundary
-    - Null-object pattern: use(plugin({ enabled: false })) preserves static
-      - member shape with a no-op implementation
+    - Flat fluent builder: KelirBuilder carries an ordered plugin registry
+      - and an optional Params schema; no recursive generic accumulation
+    - .use(plugin) appends to the registry; returns this (typed overload)
+      - The overload is generic in P extends KelirPlugin; the returned type
+      - exposes session.<pluginKey> for that plugin
+    - Prerequisite validation happens at builder.boot()
+      - Missing dependency throws EConfig with a structured error envelope
+      - The error names the missing plugin and the plugin that declared it
+      - No compile-time generic gymnastics; TS compiler stays fast
+    - .trap(matcher, handler): pre-exec gate returning a Verdict
+    - .trap(matcher, { pre, post }): dual pre/post gate
+    - .intercept(ring, filter, handler): ring-level synchronous hot-path
+    - .on(event, handler): lifecycle interceptors over LifecycleEvent values
+    - .onTrap(handler): plugin-level trap interposer; returns a new plugin
+    - .params(schema) pins Params from the schema; handler c.params is typed
+      - from TypeBox's Static<> type, resolved once at build time
+    - Handlers receive HandlerContext<Params>; plugin surfaces are typed
+      - members (c.policy, c.ledger), resolved via .use() overloads
+    - AOT hook fusion compiles all TS pipeline handlers into a single flat
+      - closure; exactly 1 NAPI trampoline per event regardless of depth
+    - Wraps async TS handlers in ForeignPluginProxy at NAPI boundary
+    - Null-object pattern: use(plugin({ enabled: false })) yields a no-op
+      - plugin; .boot() still validates but skips registration
   - Python SDK
     - see: artifacts/code/python-raii-pydantic.py
     - async with builder as session
       - __aenter__ boots microVM
       - __aexit__ guarantees session.terminate() on any exception
-    - KelirBuilder[TCapabilities, TParams] carries the accumulated surface in
-      - PEP 695 type parameters; every verb returns a new builder type
-    - .use(plugin) intersects TNewCap into TCapabilities via Protocol
-      - intersection; the c.cap projection is statically typed by pyright/mypy
-    - .params(schema: type[TNewParams]) pins TParams; handler c.params is typed
-      - from the Pydantic model class
-    - Composite plugins unpack nested plugins atomically before boot
-    - Capabilities are static members of c.cap (c.cap.policy), never
-      - c.service(ServiceType)
-    - Mount scoping via kwargs: rings=..., binaries=..., paths=...
+    - Flat fluent builder: KelirBuilder carries an ordered plugin registry
+      - and an optional Params model class; no Protocol intersection
+    - .use(plugin) appends to the registry; returns self
+      - .use() is overloaded per plugin so c.cap.<plugin_key> is typed
+    - Prerequisite validation happens at builder.boot()
+      - Missing dependency raises EConfig with a structured error envelope
+      - The error names the missing plugin and the plugin that declared it
+      - No runtime cost beyond a single O(plugins) walk at boot
+    - .params(model: type[P]) pins Params; handler c.params is typed
+      - from the Pydantic v2 model class
+    - Composite plugins unpack nested plugins atomically before boot
+    - Capabilities are static members of c.cap (c.cap.policy), never
+      - c.service(ServiceType)
+    - Mount scoping via kwargs: rings=..., binaries=..., paths=...
```

```diff
--- a/cans/15-operations.md
+++ b/cans/15-operations.md
@@ -Multi-language-parity-gate
   - Multi-language parity gate
     - Rust macros generate typed FFI stubs: napi-rs for TS, pyo3 for Python
     - Parity evaluated on identical wire frame inputs and outputs using vectors.json
     - Syntax is idiomatic per language runtime, not copied across SDKs
+    - Gating is behavioral, not type-level
+      - Rust: compile-time Has<T> bounds (typestate)
+      - TS/Py: builder.boot() EConfig on missing dependency
+      - Identical vectors.json outcomes; runtime EConfig is a parity vector
     - Testkit ground truth
       - kelir-plugin-testkit serializes canonical ring events and verdicts
       - Emits raw binary frames, TrapEvent JSON, and Decision expectations
@@
-    - AST surface manifest diff
+    - AST surface manifest diff
       - Automated AST scraper extracts public exported API signatures
       - Asserts 1:1 match across Rust, TypeScript, and Python SDK surfaces
-      - Fails CI on divergent method names, parameter arity, or return types
+      - Fails CI on divergent method names, parameter arity, or return types
+      - Divergence in typestate encoding is allowed: Rust may express a
+        - constraint at compile time that TS/Py expresses at boot(); the
+        - behavioral outcome in vectors.json is what the gate asserts
```

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -Language-parity
   - Language parity
     - Invariant: Same first-class plugins across Rust, TypeScript, Python (1:1)
     - Enforcement gate: see: 15-operations.md#Multi-language-parity-gate
     - Projection implementations: see: 03-sdk-architecture.md#Language-surfaces
+    - Typestate is Rust-only
+      - Compile-time prerequisites in Rust; boot-time in TS/Py
+      - Both produce EConfig with identical structure
+      - No TS/Py compile-time gymnastics; no recursive generics, no PEP 695
+        - intersections
```

**What stays:**
- Rust affine typestate (unchanged).
- `vectors.json` parity gate (unchanged).
- `builder.boot()` as atomic validation (strengthened: now the *only* gate for TS/Py).

**What dies:**
- `KelirBuilder<TContext, TParams, TInstalledPlugins>` recursive generic accumulation.
- `CompileError<"Missing ...">` phantom types.
- PEP 695 `Protocol` intersections for `TCapabilities`.
- Any claim that TS/Py enforce compile-time prerequisite gating.

End Part 7/9. Say `continue` for Part 8 (Vault out-of-band credential broker).

## Part 8 / 9 — Vault out-of-band credential broker

**Decision:** Kill in-process TLS MITM. Vault becomes an in-guest metadata service over vsock that dispenses scoped, short-lived credentials. No CA mint, no HTTP/2→HTTP/1.1 downgrade, no Aho-Corasick body rewriting, no chunked re-encode. Optional network-level rewriting moves to an external sidecar process (Envoy/Squid) selected by explicit config, and it lives outside the supervisor.

**Touched files:** `plugins/vault.md`, `01-architecture.md`, `07-wire-plane.md`, `08-lifecycle.md`, `09-snapshots.md`, `plugins/tool-guard.md`, `plugins/commercial.md`.

```diff
--- a/cans/plugins/vault.md
+++ b/cans/plugins/vault.md
@@ -Crate-dependencies
   - Crate dependencies (vault)
     - Plugin identifier: plugin-vault
     - Owner crate: crates/kelir-plugins/plugin-vault
     - name() = "vault"
-    - Tier 2 governance and protocol proxy (vault)
+    - Tier 2 governance (vault)
     - Depends on (vault): kelir-interface only
-    - Exposes: VaultService
+    - Exposes: CredentialBrokerService
     - VirtIO device attachment (vault): see: 01-architecture.md#VirtIO-Device-Complex
     - Lifecycle attachment (vault): see: 08-lifecycle.md#Lifecycle-vetoes
   - Service exports (vault)
-    - Exports VaultService through PipelineBinder for secret resolution and redaction
+    - Exports CredentialBrokerService via PipelineBinder for scoped token issuance
   - Invariants and policy (vault)
-    - Single source of truth for secret handling
-      - Compiles unified Aho-Corasick automaton across all registered host secrets
-      - Shared across native Storage, Wire, and Terminal planes
-      - Enforces vault:// placeholder URN generation: real secrets never enter guest physical RAM
-    - Cross-plane interposer attachments
-      - Implements FsInterposer::on_fs_read for stream secret redaction
-      - Maintains carryover window of MAX_PLACEHOLDER_LEN - 1 bytes across chunks
-      - Implements ConsoleInterposer::on_console_output for terminal stream scrubbing
-      - Implements ConsoleInterposer::on_exec_stdout and on_exec_stderr
-        - Scrubs Channel 0x0003 stdout/stderr byte streams
-        - Uses same Aho-Corasick automaton and carryover window
-      - Operates explicit L7 proxy stage on wire plane transparent L4 diversion hooks
-      - Injects ephemeral session root CA into guest trust store at on_vm_ready
-      - Forces runtime CA environment variables across all spawned task contexts:
-        - SSL_CERT_FILE, NODE_EXTRA_CA_CERTS, REQUESTS_CA_BUNDLE, CURL_CA_BUNDLE
-      - Mounts synthetic FUSE inodes over site-packages certifi bundles
-    - L7 proxy stage
-      - Host terminates outbound TLS for secret-bound hosts
-      - Ephemeral per-session CA minted by host runtime at on_vm_ready
-      - CA injected via trust store, env pointers, and synthetic FUSE overrides
-      - Plaintext HTTP request/response is the substitution surface
-      - Mandatory auth header lockdown on all secret-bound destination hosts
-      - Non-placeholder authentication headers reject instantly with ESecurityViolation
-      - Prevents model context exfiltration using attacker-supplied personal API keys
-      - Header substitution: by binding field name (Authorization, X-API-Key, ...)
-      - Body substitution: form-encoded, JSON, and text bodies up to 16 MiB
-        - Ceiling is fail-closed: bodies exceeding 16 MiB to secret-bound hosts
-        - reject immediately with ESecurityViolation(E_PAYLOAD_TOO_LARGE)
-      - Query string decoded and scanned by the same Aho-Corasick automaton
-        - before the HTTP request line forwards upstream
-      - Streaming bodies reassembled with MAX_PLACEHOLDER_LEN - 1 carryover window
-      - Eliminates pattern fragmentation across TCP frames and chunked transfers
-      - Content-Length recomputed; chunked bodies re-encoded with fresh sizes
-      - Normalizes guest HTTP/2 into semantic HTTP/1.1 request structures in host RAM
-      - Substitutes headers and bodies before re-encoding for upstream TLS/H2 session
-      - Eliminates in-flight HPACK dynamic table surgery and stream frame desync
-      - No byte-slice substitution on raw L4 streams
-      - Host proxy opens a fresh upstream TLS session and forwards the mutated request
-      - Plain HTTP (port 80) uses the same substitution path without TLS
-      - Secret bytes never exist in guest physical RAM, guest VFS, or VirtIO rings
-    - CA lifecycle
-      - Ephemeral session CA minted at on_vm_ready for fresh sessions
-      - Snapshot restore hydrates recorded ca_cert/ca_key from .kelir-snap
-      - Missing vault in lineage or fingerprint mismatch aborts boot
-      - Abort code E_RESTORE_TLS_MISMATCH
-    - Opaque tunnel bypass
-      - Protocol demotion: Upgrade: websocket or completed CONNECT handshake
-        - transitions the stream to OpaqueTunnel state
-      - OpaqueTunnel skips Aho-Corasick and substitution entirely
-      - Socket pair delegates to kernel zero-copy splice
-        - splice on Linux, direct ring copy on macOS
-    - Memory isolation and zeroization
-      - Host secret memory pinned with mlock(2) to prevent swapping to disk
-      - Memory zeroed using explicit volatile writes upon session termination or drop
-      - Plaintext buffers at the L7 proxy stage are zeroized after each request
-        - or response is forwarded; no plaintext lingers across connections
-    - Out-of-band secret resolution
-      - Host-side forward proxy resolves vault:// placeholders against secure host keyring
-      - Ephemeral per-session CA certificates injected into guest trust stores transparently
+    - Out-of-band credential brokerage
+      - Host keyring is the sole secret store; secrets never enter guest memory
+      - In-guest metadata service issues scoped, short-lived credentials on demand
+        - Guest-visible endpoint: virtio-vsock 169.254.169.254 equivalents:
+        - http://169.254.169.254/kelir/credentials/<name> (HTTP over vsock)
+        - Response body: { token, ttl_seconds, scope, audience }
+      - Token TTL default 60s; configurable to 900s ceiling
+      - Scope is the declared MountScope capability from the builder profile
+      - Audience is the destination host pattern (e.g. api.openai.com)
+      - Token is a signed opaque bearer; host keyring never exposed raw
+      - No in-process TLS MITM; no host-side CA; no HTTP body rewriting
+    - Credential types
+      - Bearer token (Authorization: Bearer <token>)
+      - Signed request V4 (AWS SigV4, GCP SA JWT, Azure AAD)
+      - mTLS client cert (short-lived cert + key pair, PEM bundle)
+      - Each type has a declared issuer backend (host keyring entry kind)
+      - Issuance is fail-closed: unregistered name → ENODATA; expired scope → EPERM
+    - Guest integration
+      - Zero SDK required; workload fetches over plain HTTP from metadata service
+      - Common language runtime helpers ship as userland libraries, not engine
+      - vault:// placeholder in profiles resolves to a metadata path at boot
+      - Guest cert store is untouched; no CA injection
+      - Guest env vars are workload-owned; no forced CA env pointers
+    - Optional external egress rewriting (opt-in, out-of-process)
+      - Not the default; the default is metadata-broker-only
+      - Selected via vault.external_egress = { kind: "envoy" | "squid", ... }
+      - Sidecar process owns TLS termination, HTTP rewriting, and secret
+        - substitution; runs outside the supervisor binary
+      - Wire plane forwards marked egress through the sidecar via L4 diversion
+      - Supervisor memory never sees plaintext secrets or HTTP bodies
+      - Required only when a workload cannot fetch tokens itself (legacy HTTP-only,
+        - no ability to set Authorization headers)
+    - Memory isolation and zeroization
+      - Host keyring memory pinned with mlock(2); excluded from swap
+      - Issued token buffers zeroized on drop; TTL expiry revokes host-side
+      - No plaintext token lingers in the supervisor after issuance returns
+    - Revocation
+      - Session terminate revokes all outstanding tokens for that session
+      - Token validation is host-side; guest cannot forge or replay post-revoke
   - Error and return semantics (vault)
     - Hook signature (vault): see: artifacts/code/plugin-trait.rs
     - Return errno and rejection behavior (vault)
-      - Plaintext secret detected in guest-to-host trap event rejects with ESecurityViolation
-      - Unresolvable placeholder URN returns ENODATA
+      - Unregistered credential name → ENODATA
+      - Credential name outside declared scope → EPERM
+      - Audience mismatch → EACCES
+      - TTL request above ceiling → EINVAL
+      - Metadata service request from non-session vsock peer → ECONNREFUSED
```

```diff
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@ -Trust-boundaries
   - Trust boundaries
-    - TLS terminates on the host via L7 proxy stage before bytes reach the guest
-    - Carrier is transparent L4 diversion; see: 07-wire-plane.md#Transparent-L4-stream-diversion
-    - Inspection, normalization, and substitution run on the native L7 stage
-      - see: 07-wire-plane.md#L7-interposer-stage
-    - Injects runtime CA env pointers and synthetic FUSE bundles across all tasks
-    - Streaming substitution preserves sliding carryover window across frames
-    - Upstream certificates validate host-side; guest never runs HPACK dynamic table surgery
+    - Guest TLS sessions are end-to-end between workload and peer
+      - Host does not terminate TLS by default
+      - Credentials are issued as scoped short-lived tokens by plugin-vault
+      - Workload presents tokens itself; no in-flight substitution
+    - Optional external sidecar can terminate TLS if the workload cannot
+      - see: plugins/vault.md#Optional-external-egress-rewriting-opt-in-out-of-process
+    - Host never sees plaintext request bodies for credential-bound traffic
+    - Upstream certificate validation is the workload's responsibility
+      - System CA bundles ship inside the EROFS rootfs as normal
```

```diff
--- a/cans/07-wire-plane.md
+++ b/cans/07-wire-plane.md
@@ -Transparent-L4-stream-diversion
     - Directs guest TCP streams across VirtIO ring to designated host socket
     - Exposes raw duplex stream to registered NetInterposer proxies
-    - Protocol sniff gate: carrier inspects initial 16 bytes of diverted stream
-    - Valid HTTP verb or TLS ClientHello promotes stream to L7 proxy stage
-    - Non-HTTP byte streams bypass L7 stages with zero-copy L4 socket splicing
-    - Prevents stream stalls on Redis, NATS, and custom binary protocols
-    - Tunnel demotion
-      - Mid-stream upgrade (Upgrade: websocket) demotes to OpaqueTunnel
-      - Completed HTTP CONNECT handshake demotes to OpaqueTunnel
-      - OpaqueTunnel bypasses Aho-Corasick and substitution; splice(2) on Linux
-      - Demotion is one-way per stream; no re-promotion to L7 stages
-      - Vault-side bypass semantics: see: plugins/vault.md#Opaque-tunnel-bypass
-    - L7 interposer stage
-      - Registers as L7Interposer hooks on the L4 diversion carrier
-      - Receives normalized HTTP/1.1 frames for mutation
-      - Multiple interposers mount at distinct host:port scopes
-    - Guest TLS and plaintext payloads traverse proxy without guest stack changes
-    - Diverted endpoints configured via MountScope port and CIDR allowlists
-    - The carrier does not perform byte-slice substitution on L4 streams;
-      - L7 stages terminate TLS host-side and operate on plaintext HTTP
-    - Streaming payloads maintain MAX_PLACEHOLDER_LEN - 1 carryover window
-    - Frame reassembly eliminates token fragmentation across TCP boundaries
-    - Normalizes HTTP/2 streams into framed request buffers before scan
+    - Carrier is L4-only by default
+      - Bytes are relayed to the upstream host through the dial transport
+      - No protocol sniff, no L7 parsing, no HTTP rewriting in the supervisor
+      - TLS between guest and peer passes through byte-for-byte
+    - L7 interposition is off by default
+      - Enabled only when the builder mounts a NetInterposer with an L7 scope
+      - L7 scopes are declared at mount; no implicit sniff-and-promote
+      - Interposer receives reassembled stream bytes; it never terminates TLS
+    - Vault and credential flow
+      - Default path: guest fetches scoped token from plugin-vault, presents it
+      - No host-side TLS termination; no CA injection
+      - Optional external sidecar can be configured for L7 rewriting
+        - see: plugins/vault.md#Optional-external-egress-rewriting-opt-in-out-of-process
+    - No `OpaqueTunnel` concept; streams are just streams
+      - There is no "demotion" because there is no promotion to demote from
     - Diverted endpoints configured via MountScope port and CIDR allowlists
```

```diff
--- a/cans/08-lifecycle.md
+++ b/cans/08-lifecycle.md
@@ -Suspend-and-resume
-    - Snapshot CA hydration
-      - .kelir-snap carries encrypted ca_cert + ca_key under session key
-      - vault restores exact recorded CA on resume; fingerprint mismatch aborts
-      - E_RESTORE_TLS_MISMATCH fires before any vCPU cycle
-      - Fresh boot without snapshot mints a new CA at on_vm_ready as usual
     - Typical use: waiting for human approval with zero host CPU cost
```

```diff
--- a/cans/09-snapshots.md
+++ b/cans/09-snapshots.md
@@ -Archives-and-portability
     - Export format: single Content-Addressed .kelir-snap binary stream
       - Archive header: magic, format version, feature flags, entry table
       - Stream contents: manifest.json, vmm_state.json, ram.zst, upperdir.tar.zst,
-        - guest_clock.json, attestation.jws, tls_ca.jws
+        - guest_clock.json, attestation.jws
+      - No TLS CA material is snapshotted; no host CA exists
       - upperdir.tar.zst is broker state: sparse upper file + tombstones
@@
-      - tls_ca.jws carries vault CA cert + key under session key
-        - Only present when plugin-vault is mounted in the snapshot lineage
       - Verification: trailer carries manifest digest for end-to-end integrity
```

```diff
--- a/cans/plugins/tool-guard.md
+++ b/cans/plugins/tool-guard.md
@@ -Parameter-payload-sanitization
     - Parameter & payload sanitization
       - Zero-copy regex & Aho-Corasick scanning over string arguments
-      - Injects vault:// placeholder dereferencing: raw secrets in arguments reject immediately
+      - Raw secret patterns in arguments reject immediately with ESecurityViolation
+      - vault:// placeholders are resolved by the workload, not by tool-guard
       - Output scrubber: scans tool execution responses on return;
         - strips host secrets before model context ingests bytes
```

```diff
--- a/cans/plugins/commercial.md
+++ b/cans/plugins/commercial.md
@@ -openshell-coordinator
     - Dynamic approval lifecycle bridge
       - Denial triggers external suspension: see: 08-lifecycle.md#Suspend-and-resume
       - Gateway admin approvals push RFC 6902 patches to plugin-profile before cycle-exact resume
     - CDI mapping: passes OpenShell GPU device requests to plugin-profile CDI parser
     - OpenShell boundary: see: 12-adapters.md#Process-vs-plugin-boundary
     - OpenShell interface: OpenShellCoordinationPolicy trait
+  - egress-sidecar (commercial)
+    - Tier 3 external egress rewriting daemon (Envoy/Squid wrapper)
+    - Out-of-process; runs outside the supervisor binary
+    - Implements optional L7 rewriting for legacy workloads
+      - see: plugins/vault.md#Optional-external-egress-rewriting-opt-in-out-of-process
+    - Interface: EgressSidecarConfig (kind, socket, upstream policy)
+    - Depends: kelir-interface; runs as a separate OS process
+    - Channels: none; wire plane diverts marked egress via L4
```

**Outcomes:**
- **Default credential path is out-of-band.** Metadata service over vsock, short-lived scoped tokens. No MITM, no CA, no body rewriting in supervisor.
- **Attack surface removed.** No TLS terminator, no HTTP/2 downgrade, no HPACK surgery, no Aho-Corasick body scanner in the hypervisor.
- **Opaque-tunnel gap eliminated.** There is no MITM to bypass, so there is no tunnel-demotion hole.
- **Snapshot format cleaned.** `tls_ca.jws` gone; no `E_RESTORE_TLS_MISMATCH`; no vault CA to hydrate.
- **Legacy workloads still served.** Optional external sidecar process (commercial) provides L7 rewriting without touching the supervisor.
- **tool-guard stops pretending to dereference.** It rejects raw secrets; it does not substitute them.

**Deleted nodes across vault:** `L7-proxy-stage`, `CA-lifecycle`, `Opaque-tunnel-bypass`, `Cross-plane-interposer-attachments`, `Single-source-of-truth-for-secret-handling`.

**New nodes:** `Out-of-band credential brokerage`, `Credential types`, `Guest integration`, `Optional external egress rewriting`, `Revocation`.

End Part 8/9. Say `continue` for Part 9 (Lifecycle rollback network truth).

## Part 9 / 9 — Lifecycle rollback network truth

**Decision:** Rollback is a compute+disk reset, not a distributed-state time machine. Every `session.rollback()` **immediately terminates all L4 host sockets**. Guest observes `ECONNRESET`/`EPIPE`. No claim of preserving live remote connections. Suspend/resume still preserves *host-side* session state; rollback does not.

**Touched files:** `08-lifecycle.md`, `07-wire-plane.md`, `09-snapshots.md`, `01-architecture.md`, `plugins/replay.md`, `plugins/wire-ingress.md`, `03-sdk-architecture.md`.

```diff
--- a/cans/08-lifecycle.md
+++ b/cans/08-lifecycle.md
@@ -Rollback
   - Rollback
     - Pre-condition invariant: Rollback is prohibited while vCPUs hold unreleased syscall traps
     - Attempting rollback on a frozen syscall trap aborts with E_INVALID_LIFECYCLE_STATE
-    - Two-phase execution boundary
-      - 1. Storage reset: Session::rollback() truncates the sparse host-backed
-        - upperdir file and punches holes (FALLOC_FL_PUNCH_HOLE)
-      - 2. Hardware reset: driver.rollback(&snapshot) resets physical RAM
-        - and vCPU registers in <8ms
-      - HVF path: resets Stage-2 translation table roots to snapshot base
-      - KVM path: restores userfaultfd memory tracking to checkpoint slot
-    - Host SSD workspace and host-side Merkle WAL remain completely untouched
+    - State boundary (rollback covers compute + disk, not remote state)
+      - Rollback resets: vCPU registers, guest RAM, broker upper file,
+        - tombstones, session-local RNG position, virtual clock position
+      - Rollback does NOT reset: host workspace, host Merkle WAL,
+        - external network peer state, issued credential tokens (revoked),
+        - host keyring, in-flight upstream sockets
+    - Network truth
+      - All L4 host-side sockets for the session are closed immediately
+      - TCP peers receive a clean FIN or RST depending on shutdown path
+      - Guest observes ECONNRESET on next read or EPIPE on next write
+      - WebSocket, HTTP/2, and gRPC connections are terminated the same way
+      - No proxy-level socket reconnection protocol exists in core
+      - Workloads that need transparent reconnection must implement it above L4
+    - Credential truth
+      - plugin-vault revokes all outstanding tokens on rollback
+      - Workload must re-fetch credentials from the metadata service
+    - Three-phase execution boundary
+      - 1. Network quiesce: cancel in-flight L4 streams, emit RST to peers,
+        - drain wire-plane splicing buffers, drop UDP NAT entries for session
+      - 2. Storage reset: broker drops upper file and tombstones;
+        - FALLOC_FL_PUNCH_HOLE + posix_fadvise(DONTNEED) on the sparse upper
+      - 3. Hardware reset: driver.rollback(&snapshot) resets physical RAM
+        - and vCPU registers in <8ms
+      - HVF path: resets Stage-2 translation table roots to snapshot base
+      - KVM path: restores userfaultfd memory tracking to checkpoint slot
+    - Host SSD workspace and host-side Merkle WAL remain completely untouched
+    - Sockets are not part of the snapshot; they are never restored
     - Veto: Rollback (observation)
       - Cannot be vetoed; rollback is the recovery action
       - Use for ledger/trace of recovery events
+    - Anti-claims
+      - Rollback does not preserve open TCP connections
+      - Rollback does not preserve TLS sessions or certificate validity
+      - Rollback does not rewind external peer state
+      - Rollback does not "resume at exact cycle boundary" for network I/O
```

```diff
--- a/cans/07-wire-plane.md
+++ b/cans/07-wire-plane.md
@@ -Wire-architecture
   - Wire architecture
@@
     - Interceptor hooks receive NetContext containing raw Ethernet frames
+    - Rollback interaction
+      - session.rollback() closes every host-side socket for the session
+      - Guest TCP stack observes RST/FIN; subsequent sends fail ECONNRESET
+      - UDP NAT slab entries for the session are dropped; no port leak
+      - Sockets are not restored from snapshots; there is no socket state to restore
+      - see: 08-lifecycle.md#Rollback
```

```diff
--- a/cans/09-snapshots.md
+++ b/cans/09-snapshots.md
@@ -Archives-and-portability
     - Export format: single Content-Addressed .kelir-snap binary stream
       - Archive header: magic, format version, feature flags, entry table
       - Stream contents: manifest.json, vmm_state.json, ram.zst, upperdir.tar.zst,
         - guest_clock.json, attestation.jws
       - No TLS CA material is snapshotted; no host CA exists
+      - No socket state, TCP sequence numbers, or connection metadata
+        - is snapshotted; snapshots are compute + disk only
       - upperdir.tar.zst is broker state: sparse upper file + tombstones
@@
     - Verification: trailer carries manifest digest for end-to-end integrity
+    - Restore semantics
+      - Import materializes compute and disk only
+      - Workloads that had live sockets observe them gone on first access
+      - No transparent socket rebinding exists; workloads must reconnect
```

```diff
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@ -In-process-microkernel
         - Fallback policy
@@
         - Foreign runtime embedding
@@
         - FFI boundary economics
@@
         - Isolation mechanics
@@
+      - Snapshot and rollback scope
+        - Snapshots capture compute + disk, not distributed network state
+        - Rollback closes session sockets; guests observe ECONNRESET
+        - See: 08-lifecycle.md#Rollback
       - Strict hardware/kernel abstraction layer
```

```diff
--- a/cans/plugins/wire-ingress.md
+++ b/cans/plugins/wire-ingress.md
@@ -Invariants-and-policy
     - Holds inbound client TCP connections without dropped frames during suspend
+      - Suspend preserves inbound parked connections; rollback does not
+      - On rollback: parked connections get TCP RST (post-handshake: 503 close)
+      - Post-rollback inbound clients must reconnect
     - Host proxy terminates TCP handshake and buffers up to 64KB initial request data
@@
     - Zero-downtime webhook and interactive terminal connection continuity
+      - Continuity applies to suspend/resume only
+      - Rollback terminates parked connections; retry is client-side
```

```diff
--- a/cans/plugins/replay.md
+++ b/cans/plugins/replay.md
@@ -Invariants-and-policy
     - Cycle-exact instruction clock stepping
@@
       - Advances guest KVM_SET_CLOCK / CNTVOFF_EL2 on retired-instruction windows
       - PMU values drive local stepping only; never appear in receipts
       - Eliminates host scheduling jitter and timer interrupt variance across runs
+      - Replay mode requires network isolation for determinism
+        - Deterministic replay cannot include live external network peers
+        - Replay profiles declare network: off; wire plane drops all egress
+        - I/O determinism draws from seeded inputs, not from live sockets
```

```diff
--- a/cans/03-sdk-architecture.md
+++ b/cans/03-sdk-architecture.md
@@ -Session-control-surface
         - Session control surface
@@
           - Graceful drain: session.drain(ms); see: 08-lifecycle.md#Hypervisor-primitives
+          - Rollback semantics
+            - session.rollback() resets compute + disk
+            - Closes all session sockets; guest sees ECONNRESET
+            - No opt-in flag preserves sockets; rollback is total
+            - See: 08-lifecycle.md#Rollback
```

**Outcomes:**
- **Rollback is honest.** Compute+disk reset. Network dies. Guest sees the truth.
- **No "zero broken sockets" claim anywhere.** Deleted from the spec.
- **Suspend/resume unchanged.** It preserves host RSS, not sockets either, but the claim there was RSS, and parked connections were via `wire-ingress` (explicit parking plugin, not transport).
- **`wire-ingress` parking still works for suspend.** It stops working for rollback — and says so.
- **Replay mode is network-off.** Determinism and live sockets are mutually exclusive; spec says so.
- **Snapshot format carries no socket state.** Because there is no socket state to carry.

**Deleted:** "External suspend/resume across multi-day offsets with zero broken sockets", "Zero CERT_HAS_EXPIRED", "rollback preserving live remote network sessions".

**New gate:** `cans check` rejects any node that claims rollback preserves network sockets.

---

**All 9 parts delivered.** Final verification checklist:

| Gate | Assertion |
|---|---|
| `--action-coverage` | Every `Verdict` variant exercised in an example |
| `--channel-coverage` | Every core stream kind exercised; ≥1 plugin kind |
| `--plugin-coverage` | db-proxy + supply-chain no longer counted as engine plugins |
| `--hook-coverage` | Unchanged; hooks now return `Verdict` |
| `cans check` | No `epistemic`, `perf_event_open` receipts, `TrapAction`, `SyncVerdict`, `PluginError::Reject`, `DbAction`, `ToolAction`, `overlay`, `whiteout`, `russh`, `Channel 0x000N` identifiers |
| `git mv` | `10-epistemic.md` → `10-attestation.md` |
| `_rules.yaml` | `10-attestation.md` path live; no dangling refs |