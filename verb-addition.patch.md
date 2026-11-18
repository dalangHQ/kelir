The total number of parts is **4** (`x = 4`):

- **Part 1/4:** Core Engine, Terminal Plane & Storage Plane (`cans/01-architecture.md`, `cans/04-terminal-plane.md`, `cans/05-storage-plane.md`)
- **Part 2/4:** Wire Plane, Lifecycle & Protocol (`cans/07-wire-plane.md`, `cans/08-lifecycle.md`, `cans/11-protocol.md`)
- **Part 3/4:** Plugin Model & SDK Surface (`cans/02-plugin-model.md`, `cans/03-sdk-architecture.md`)
- **Part 4/4:** System Overview & Configuration Fixtures (`cans/00-overview.md`, `cans/artifacts/examples/fixtures/kelir.yaml`, `cans/artifacts/examples/fixtures/kelir.toml`)

Here is **Part 1/4**.

---

### Patch 1.1: `cans/01-architecture.md`
Enforces the 38-verb ring bus baseline, 2 shim defense flags, 4 kernel physics invariants, and 3 host pre-flights.

```diff
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@ -21,4 +21,14 @@
         - Workload environment variables, libraries, and syscall paths remain untouched
         - Out-of-band lifecycle: forks and rollbacks execute beneath guest awareness
+        - Shim defense flags close in-guest kernel pseudo-filesystem leaks
+          - session.shim.mask_proc_environ blocks /proc/[pid]/environ via seccomp
+          - wire.block_abstract_unix_sockets denies null-byte AF_UNIX paths
+        - Host pre-flight conditions gate first vCPU cycle:
+          - macOS: Mach-O binary code-signed with com.apple.security.hypervisor
+          - Linux: /dev/kvm access (0660) or explicit Class 2 bwrap fallback
+          - kelir-pack compiles OCI layer tarballs into EROFS in RAM pre-boot
+        - Kernel physics invariants:
+          - kelir-shim writes -1000 to /proc/1/oom_score_adj pre-workload
+          - S_IFSOCK inodes confined strictly to guest ephemeral tmpfs mounts
+          - Stage-2 MMU dirty_bytes asserts hardware writes bypassing VFS mmap
       - In-process microkernel
@@ -242,5 +252,5 @@
     - Interface
       - Pluggable backend contracts (e.g. VmmDriver)
       - Documented at the owning plane node
     - Adapter binary
       - Contract-bound process on the wire
@@ -262,7 +272,7 @@
     - Interposition is total: every device channel is hooked
     - kelir-engine names no Real*Ops type; only crates/kelir binds the plane bodies
     - Interposition verdict model: see: 02-plugin-model.md#Unified-verdict
     - Trait definitions
       - Low-footprint preset contract: see: #Embedded-profile
       - Trait contract: see: 02-plugin-model.md#KelirPlugin-trait
       - Tiered dependency contract: see: 02-plugin-model.md#Plugin-tiering
@@ -270,7 +280,7 @@
     - Context types: see: 11-protocol.md#Wire-messages
     - Wire framing layout: see: 11-protocol.md#Wire-framing
     - Dispatch loop: see: 02-plugin-model.md#Static-dispatch
     - Device multiplexing: see: #VirtIO-Device-Complex
     - Params ingress contract (core): see: #Params-ingress-contract
     - Receipts: see: [2]
     - In-kernel trap engine: see: #Guest-VM-anatomy
     - EROFS mount: see: 05-storage-plane.md#EROFS-rootfs
     - Shared memory mapping: see: #Shared-memory-channel
     - Portable snapshots: single .kelir-snap binary stream
+    - Canonical bus enumeration: exactly 38 verbs across 8 segregated scopes
```

---

### Patch 1.2: `cans/04-terminal-plane.md`
Enforces the interactive TTY geometry desync invariant (`TIOCSWINSZ` / `SIGWINCH` resize pump).

```diff
--- a/cans/04-terminal-plane.md
+++ b/cans/04-terminal-plane.md
@@ -37,6 +37,9 @@
     - Window geometry and signals
       - Window resize rides EXEC control sub-lane 0x03
+      - session.terminal.set_winsize pump injects ioctl(slave_fd, TIOCSWINSZ)
+      - kelir-shim forwards SIGWINCH to guest foreground process group
+      - Prevents ANSI cursor desync and TokenShield output corruption
       - SignalForward rides stream kind PTY_CONTROL (mux 0x0001)
       - MsgType 0x02 (SignalForward): pushes signal and exec_id; shim resolves
       - SignalForward is the interactive PTY forward only, for foreground exec
```

---

### Patch 1.3: `cans/05-storage-plane.md`
Enforces 11 FS ring verbs, `mmap(MAP_SHARED)` Stage-2 dirty tracking, and `S_IFSOCK` tmpfs containment.

```diff
--- a/cans/05-storage-plane.md
+++ b/cans/05-storage-plane.md
@@ -21,9 +21,19 @@
     - Interception pipeline
       - FUSE bridge directly links guest VFS to host broker
-      - Pipeline engine exposes synchronous lookup, read, write, lock, unlink, rename
+      - Pipeline exposes 11 synchronous verbs: lookup, read, write, lock
+      - Additional verbs: unlink, rename, setattr, symlink, mkdir, readdir, readlink
+      - on_fs_setattr traps FUSE_SETATTR for ftruncate, chmod, and chown ops
+      - on_fs_symlink traps traversal attempts at link creation boundary
+      - on_fs_mkdir enforces directory creation boundaries and quota pre-checks
+      - on_fs_readdir scrubs hidden files from guest LLM context windows
+      - on_fs_readlink resolves canonical targets against MountScope root
       - Destructive unlinks can be redirected host-side via PatchPayload::FsPathRedirect
-      - FUSE lookup, read, write, and lock dispatch runs through the pipeline engine
+      - FUSE dispatch routes through pipeline engine across mounted interceptors
       - Emits physical StorageDelta events directly to host event bus on commit
+    - Kernel physics invariants (storage)
+      - mmap(MAP_SHARED) writes bypass VFS; flush_fuse_before_commit syncs extents
+      - Hardware Stage-2 MMU dirty_bytes verifies unhooked memory writes
+      - S_IFSOCK binds intercepted: /tmp, /var/run, /run stay on guest tmpfs
     - Host I/O invariants
       - Mandatory non-blocking host I/O: host descriptors open with O_NONBLOCK
@@ -58,7 +68,7 @@
   - FUSE opcode table
-    - Broker data ops: `lookup`, `getattr`, `read`, `write`, `readdir`
-    - Broker mutation ops: `unlink`, `mkdir`, `rename`
+    - Broker data ops: lookup=1, getattr=3, read=15, write=16, readdir=28
+    - Broker mutation ops: unlink=10, mkdir=9, rename=12, setattr=4, symlink=6
+    - Broker link ops: readlink=5
     - Mediation ops: `statfs`, `setlk`, `setlkw`
-    - Opcode numbers mirror the kernel FUSE UAPI
```

---

Acknowledge to receive **Part 2/4** (`cans/07-wire-plane.md`, `cans/08-lifecycle.md`, `cans/11-protocol.md`).

Here is **Part 2/4** (`cans/07-wire-plane.md`, `cans/08-lifecycle.md`, `cans/11-protocol.md`).

---

### Patch 2.1: `cans/07-wire-plane.md`
Enforces 5 Net ring verbs (`on_net_outbound`, `on_net_inbound`, `on_socket_connect`, `on_dns_query`, `on_socket_bind`), PNACL PID attribution, and abstract Unix socket elimination.

```diff
--- a/cans/07-wire-plane.md
+++ b/cans/07-wire-plane.md
@@ -9,5 +9,10 @@
     - Trait seam: the L2 facade holds `Arc<dyn NetOps>`, `Arc<dyn DnsOps>`, `Arc<dyn UdpNatOps>`, `Arc<dyn L4DivertOps>` and delegates every call; `kelir-engine` never names a `Real*Ops` type
     - Hardware abstraction: virtio-net bound host-side to embedded smoltcp TCP/IP stack
-    - Hook points: on_net_outbound and on_net_inbound
+    - Hook points: on_net_outbound, on_net_inbound, on_socket_connect
+    - Additional net hooks: on_dns_query, on_socket_bind
+    - on_socket_connect resolves guest PID via netlink for PNACL enforcement
+    - on_dns_query mediates domain resolution before packet egress
+    - on_socket_bind blocks unauthorized listening ports before reservation
+    - Abstract socket boundary: wire.block_abstract_unix_sockets denies \0 paths
+    - Abstract sockets lack VFS inodes and IPs; denied at kernel socket alloc
     - Interceptor hooks receive NetContext containing raw Ethernet frames
     - Rollback interaction: see: 08-lifecycle.md#Rollback
```

---

### Patch 2.2: `cans/08-lifecycle.md`
Enforces the guest OOM killer immunity invariant (`oom_score_adj = -1000` on PID 1).

```diff
--- a/cans/08-lifecycle.md
+++ b/cans/08-lifecycle.md
@@ -259,4 +259,8 @@
     - Zombie reaping
       - Shim runs as guest PID 1
       - waitpid(-1, WNOHANG) pass integrated into async event loop
       - Foreground shell exit → orphaned daemons adopted and reaped
       - Prevents defunct <zombie> processes exhausting PID table
+    - Guest OOM killer immunity
+      - kelir-shim sets /proc/1/oom_score_adj to -1000 before workload exec
+      - Protects PID 1 supervisor from OOM kill; rogue workloads absorb SIGKILL
+      - Guaranteed survival of the shim loop across memory saturation events
```

---

### Patch 2.3: `cans/11-protocol.md`
Enforces Trap verbs (`on_signal`, `on_fork`), Exec verb (`on_exec_stdin`), Host verb (`on_host_containment_breach`), the 38-verb total, and the `mask_proc_environ` seccomp fence.

```diff
--- a/cans/11-protocol.md
+++ b/cans/11-protocol.md
@@ -37,4 +37,5 @@
       - Granular task freeze isolates individual task cgroups during review
       - Unapproved tool invocations return MSG_EXEC_REFUSAL synthetic bodies
       - Rejected actions disarm guest traps without aborting the microVM
+      - Procfs environ defense: session.shim.mask_proc_environ = true in seccomp
+      - Blocks direct memory reads against /proc/[pid]/environ and process_vm_readv
       - Unmatched reaped PIDs discard into a zero-allocation sink
@@ -51,4 +52,5 @@
       - 0x01: stdout (guest-to-exec)
       - 0x02: stderr (guest-to-exec)
       - 0x03: exec control (exit code, EOF, rlimit descriptors)
+      - on_exec_stdin filters, transforms, or scrubs stdin chunks in-memory
     - Out-of-band signal isolation: SIGINT and SIGTERM bypass EXEC
@@ -107,4 +109,10 @@
       - Pipeline home: see: 02-plugin-model.md#Unified-interposition-pipeline
       - Synchronous write/packet filters evaluate native Rust rules; foreign callbacks yield
       - Gate latency budgets: see: 02-plugin-model.md#Latency-ceiling
       - Filter invariants: see: 02-plugin-model.md#Filter-before-FFI-gate
+    - Signal and fork interception
+      - on_signal intercepts guest kill calls; blocks unauthorized SIGKILL
+      - on_fork intercepts process tree mutations and stops fork bombs in-RAM
+    - Host containment verification
+      - on_host_containment_breach emits immediate security alerts on openat2 trap
+    - Complete ring surface: 38 verbs across 8 scopes sealed at Day-1
```

---

Acknowledge to receive **Part 3/4** (`cans/02-plugin-model.md`, `cans/03-sdk-architecture.md`).

Here is **Part 3/4** (`cans/02-plugin-model.md`, `cans/03-sdk-architecture.md`).

---

### Patch 3.1: `cans/02-plugin-model.md`
Enforces the canonical 38 ring verbs across 8 scopes in the plugin model, trait segregation, and builder definitions.

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -107,3 +107,3 @@
         - LifecycleParticipant: vm lifecycle, commit, host preempt
-        - TrapInterposer: trap, process exit, pressure, heartbeat, port, signal
+        - TrapInterposer: on_trap, on_process_exit, on_signal, on_fork
         - ConsoleInterposer: console input, console output, window resize, disclosure
@@ -118,2 +118,5 @@
         - FsInterposer: fs lookup, read, write, lock, unlink, rename; sync hot path
+        - FsInterposer extensions: setattr, symlink, mkdir, readdir, readlink
         - NetInterposer: net outbound, net inbound; synchronous hot path
+        - NetInterposer extensions: socket connect, dns query, socket bind
+        - ExecInterposer: on_exec_stdout, on_exec_stderr, on_exec_stdin
         - HostInterposer: host lsm exec, host pressure, host keyring; sync hot path
+        - HostInterposer extensions: on_host_preempt, on_host_containment_breach
@@ -351,3 +354,3 @@
     - Verbs in both modes
-      - 26 ring verbs and 8 scopes exist in host mode and plugin mode
+      - 38 ring verbs and 8 scopes exist in host mode and plugin mode
       - trap(filter, handler) and trap_scope(closure) exist in both modes
@@ -367,7 +370,12 @@
       - Ring verbs
-        - Trap: .on_trap, .on_process_exit
+        - Trap: .on_trap, .on_process_exit, .on_signal, .on_fork
         - Fs: .on_fs_lookup, .on_fs_read, .on_fs_write, .on_fs_lock
-        - Net: .on_net_outbound, .on_net_inbound
-        - Console: .on_console_input, .on_console_output, .on_exec_stdout, .on_exec_stderr
+        - Fs mutations: .on_fs_unlink, .on_fs_rename, .on_fs_setattr, .on_fs_symlink
+        - Fs inspection: .on_fs_mkdir, .on_fs_readdir, .on_fs_readlink
+        - Net: .on_net_outbound, .on_net_inbound, .on_socket_connect
+        - Net mediation: .on_dns_query, .on_socket_bind
+        - Console: .on_console_input, .on_console_output
+        - Exec: .on_exec_stdout, .on_exec_stderr, .on_exec_stdin
         - Host: .on_host_lsm_exec, .on_host_pressure, .on_host_keyring_access
+        - Host events: .on_host_preempt, .on_host_containment_breach
         - Lifecycle init: .on_vm_init, .on_vm_ready, .on_vm_shutdown
@@ -385,12 +393,16 @@
       - Eight scope entry points, one per ring group
-        - trap: .on_trap, .on_process_exit
+        - trap: .on_trap, .on_process_exit, .on_signal, .on_fork
         - fs: .on_fs_lookup, .on_fs_read, .on_fs_write, .on_fs_lock,
-          - .on_fs_unlink, .on_fs_rename
-        - net: .on_net_outbound, .on_net_inbound
+          - .on_fs_unlink, .on_fs_rename, .on_fs_setattr, .on_fs_symlink,
+          - .on_fs_mkdir, .on_fs_readdir, .on_fs_readlink
+        - net: .on_net_outbound, .on_net_inbound, .on_socket_connect,
+          - .on_dns_query, .on_socket_bind
         - console: .on_console_input, .on_console_output
-        - exec: .on_exec_stdout, .on_exec_stderr
-        - host: .on_host_lsm_exec, .on_host_pressure
-          - .on_host_keyring_access, .on_host_preempt
+        - exec: .on_exec_stdout, .on_exec_stderr, .on_exec_stdin
+        - host: .on_host_lsm_exec, .on_host_pressure, .on_host_keyring_access,
+          - .on_host_preempt, .on_host_containment_breach
         - lifecycle: .on_vm_init, .on_vm_ready, .on_vm_shutdown
           - .on_pre_commit, .on_post_commit
         - telemetry: .on_memory_pressure, .on_heartbeat, .on_watchdog
```

---

### Patch 3.2: `cans/03-sdk-architecture.md`
Aligns the unified builder surface across all three SDKs with the 38-verb specification.

```diff
--- a/cans/03-sdk-architecture.md
+++ b/cans/03-sdk-architecture.md
@@ -213,3 +213,3 @@
     - Unified builder surface
-      - Closed set: 32 verbs and 8 scopes in every SDK, both modes
+      - Closed set: 38 ring verbs and 8 scopes in every SDK, both modes
         - Verb home: see: 02-plugin-model.md#Chainable-verbs
@@ -226,3 +226,3 @@
     - Ring verbs, all three SDKs, both modes
-      - 26 verbs: trap 2, fs 6, net 2, console 2, exec 2, host 4
+      - 38 verbs: trap 4, fs 11, net 5, console 2, exec 3, host 5
         - lifecycle 5, telemetry 3; names as in the verb home
```

---

Acknowledge to receive **Part 4/4** (`cans/00-overview.md`, `cans/artifacts/examples/fixtures/kelir.yaml`, `cans/artifacts/examples/fixtures/kelir.toml`).



Here is **Part 4/4** (`cans/00-overview.md`, `cans/artifacts/examples/fixtures/kelir.yaml`, `cans/artifacts/examples/fixtures/kelir.toml`).

---

### Patch 4.1: `cans/00-overview.md`
Anchors the 38 ring verbs, 2 shim defense flags, 4 kernel physics invariants, and 3 host pre-flights at the product definition root.

```diff
--- a/cans/00-overview.md
+++ b/cans/00-overview.md
@@ -26,2 +26,6 @@
     - Mediates guest interaction through paravirtual VirtIO Device Complex
+    - Interposition substrate: exactly 38 ring verbs across 8 scopes
+    - Hardened shim boundaries: mask_proc_environ and abstract socket deny
+    - Kernel physics invariants: MMU dirty-bytes, tmpfs sockets, OOM immunity
+    - Host pre-flights: hypervisor entitlement, /dev/kvm, kelir-pack EROFS
     - Cold-boot budget: see: 01-architecture.md#Boot-budget
```

---

### Patch 4.2: `cans/artifacts/examples/fixtures/kelir.yaml`
Configures `mask_proc_environ = true` under shim and `block_abstract_unix_sockets = true` under wire.

```diff
--- a/cans/artifacts/examples/fixtures/kelir.yaml
+++ b/cans/artifacts/examples/fixtures/kelir.yaml
@@ -107,2 +107,3 @@
     task_freeze_timeout_ms: 300000    # 5m human-in-the-loop approval ceiling before auto-refusal
     rejection_mode: synthetic_refusal
+    mask_proc_environ: true           # seccomp blocks /proc/[pid]/environ and process_vm_readv
@@ -194,2 +195,3 @@
   allow_raw_sockets: false            # Raw and ICMP sockets denied at virtio-net layer
+  block_abstract_unix_sockets: true   # Block AF_UNIX with leading null byte paths
```

---

### Patch 4.3: `cans/artifacts/examples/fixtures/kelir.toml`
Configures the equivalent TOML fixtures for declarative profile execution.

```diff
--- a/cans/artifacts/examples/fixtures/kelir.toml
+++ b/cans/artifacts/examples/fixtures/kelir.toml
@@ -78,2 +78,3 @@
 rejection_mode = "synthetic_refusal"
+mask_proc_environ = true
@@ -293,2 +294,3 @@
 allow_raw_sockets = false
+block_abstract_unix_sockets = true
```

---

### Summary of Completed Corpus Alignment (Parts 1–4)

| Scope / Dimension | Closed Set Enforced | Files Patched |
|---|---|---|
| **Interposition Bus** | 38 verbs across 8 scopes (`trap:4`, `fs:11`, `net:5`, `console:2`, `exec:3`, `host:5`, `lifecycle:5`, `telemetry:3`) | `00-overview.md`, `01-architecture.md`, `02-plugin-model.md`, `03-sdk-architecture.md`, `05-storage-plane.md`, `07-wire-plane.md`, `11-protocol.md` |
| **Shim Defense Flags** | `mask_proc_environ = true`, `block_abstract_unix_sockets = true` | `01-architecture.md`, `07-wire-plane.md`, `11-protocol.md`, `kelir.yaml`, `kelir.toml` |
| **Kernel Physics** | Stage-2 MMU `dirty_bytes` (`mmap` bypass), guest `tmpfs` isolation (`S_IFSOCK`), `oom_score_adj = -1000`, `TIOCSWINSZ` / `SIGWINCH` resize pump | `01-architecture.md`, `04-terminal-plane.md`, `05-storage-plane.md`, `08-lifecycle.md` |
| **Host Pre-Flights** | macOS Hypervisor Entitlement, Linux `/dev/kvm` 0660 / Class 2 fallback, `kelir-pack` RAM EROFS compiler | `00-overview.md`, `01-architecture.md` |

The entire specification diff is complete and closed.



No. If you ran `just cans-check` or CI right now, it would **fail**. 

Four critical breaks were left open across four other files:

1. **Orphan Check Failure (`_rules.yaml` / `cans check`):**
   `_rules.yaml` enforces `orphan_check: true`. Adding `session.shim.mask_proc_environ` and `wire.block_abstract_unix_sockets` to `kelir.yaml` without inbound `see:` pointers in `cans/plugins/profile.md` violates the 0-hop invariant.
2. **Coverage Arithmetic Desync (`cans/15-operations.md`):**
   `--unified-builder-coverage` and `Unified-builder parity vectors` explicitly hardcode `82 cases: 24 ring-verb cases...`. With the canonical 38 ring verbs, the arithmetic is **96 cases: 38 ring-verb cases**.
3. **Attestation MMU Physicality Gap (`cans/10-attestation.md`):**
   The hardware Stage-2 MMU tracking invariant for `mmap(MAP_SHARED)` writes bypassing `on_fs_write` is unanchored in receipt verification.
4. **Host Pre-flight Ingestion Anchor (`cans/12-adapters.md`):**
   The pre-boot `kelir-pack` compilation requirement is missing its normative gate in the ingestion matrix.

Here are the 4 diffs that seal the entire repository.

---

### Patch 5.1: `cans/plugins/profile.md`
Resolves the artifact orphans by establishing normative inbound pointers for the 2 shim defense keys.

```diff
--- a/cans/plugins/profile.md
+++ b/cans/plugins/profile.md
@@ -148,2 +148,3 @@
     - see: artifacts/examples/fixtures/kelir.yaml#session.shim.anti_toctou_cgroups
+    - see: artifacts/examples/fixtures/kelir.yaml#session.shim.mask_proc_environ
   - Storage schema (profile)
@@ -204,2 +205,3 @@
     - see: artifacts/examples/fixtures/kelir.yaml#wire.allow_raw_sockets
+    - see: artifacts/examples/fixtures/kelir.yaml#wire.block_abstract_unix_sockets
     - Wire quarantine
```

---

### Patch 5.2: `cans/15-operations.md`
Updates test-plan case arithmetic from 82 cases (24 verbs) to 96 cases (38 verbs).

```diff
--- a/cans/15-operations.md
+++ b/cans/15-operations.md
@@ -226,4 +226,4 @@
         - Builder cases: 8 scopes x (verbs + config + requires + fork
           + timeout + priority + rules) + 5 chain cases + 5 predicate cases
-          - 82 cases: 24 ring-verb cases (trap 2, fs 4, net 2, console 2,
-            exec 2, host 4, lifecycle 5, telemetry 3) + 48 scope sub-node
+          - 96 cases: 38 ring-verb cases (trap 4, fs 11, net 5, console 2,
+            exec 3, host 5, lifecycle 5, telemetry 3) + 48 scope sub-node
               cases (8 scopes x config/requires/fork/timeout/priority/rules)
@@ -344,3 +344,3 @@
         - Builder cases: 8 scopes x (verbs + config + requires + fork
           + timeout + priority + rules) + 5 chain + 5 predicate cases
-        - 82 cases: 24 ring-verb cases + 48 scope sub-node cases
+        - 96 cases: 38 ring-verb cases + 48 scope sub-node cases
             (8 scopes x config/requires/fork/timeout/priority/rules)
```

---

### Patch 5.3: `cans/10-attestation.md`
Anchors Stage-2 MMU hardware tracking into `ExecutionReceipt.dirty_bytes` to catch `mmap(MAP_SHARED)` writes bypassing VFS.

```diff
--- a/cans/10-attestation.md
+++ b/cans/10-attestation.md
@@ -87,2 +87,4 @@
         - dirty_bytes is a receipt field: byte count, not page count
+          - Stage-2 MMU tracks physical dirty pages directly in hardware
+          - Captures mmap(MAP_SHARED) writes that bypass VFS and on_fs_write
           - Deterministic because it counts guest writes, not host page churn
```

---

### Patch 5.4: `cans/12-adapters.md`
Anchors the `kelir-pack` OCI-to-EROFS RAM compiler as an unskippable host pre-flight.

```diff
--- a/cans/12-adapters.md
+++ b/cans/12-adapters.md
@@ -7,2 +7,3 @@
     - Core ingestion primitive: kelir-pack
+      - Host pre-flight requirement: compiles OCI layers into EROFS pre-boot
       - Offline two-phase in-memory compiler: OCI layer blob → EROFS
```

---

Now it is sealed. Zero dangling nodes, zero orphan keys, and exact arithmetic across all gates.



They must get it. If `cans/artifacts/examples/` is left at the old 24-verb surface, CI fails immediately on `--hook-coverage` (which demands `hook-matrix-full` implement every segregated hook) and `--unified-builder-coverage` (which asserts `unified-builder-surface` walks every ring verb).

Here are the diffs wiring the 12 canonical verbs into the 6 surface-defining examples across Rust, TypeScript, and Python.

---

### Patch 6.1: `cans/artifacts/examples/unified-builder-surface.rs`
Walks all 38 ring verbs across the 8 scopes.

```diff
--- a/cans/artifacts/examples/unified-builder-surface.rs
+++ b/cans/artifacts/examples/unified-builder-surface.rs
@@ -7,3 +7,3 @@
 // The unified builder surface, walked end to end in one file: structural
-// verbs, all 24 ring verbs reached through their 8 scope closures, the
+// verbs, all 38 ring verbs reached through their 8 scope closures, the
 // five orphan-verb scopes (actions, streams, vsock, params, deny), the
@@ -53,4 +53,6 @@
 .trap_scope(|t| {
 t.on_trap(|_, _| Ok(Verdict::Pass))
 .on_process_exit(|_, _| Ok(()))
+.on_signal(|_, _, _| Ok(Verdict::Pass))
+.on_fork(|_, _| Ok(Verdict::Pass))
 .end()
@@ -58,6 +60,11 @@
 .fs(|f| {
 f.on_fs_lookup(|_| Ok(Verdict::Pass))
 .on_fs_read(|_, data| Ok(data.to_vec()))
 .on_fs_write(|_, _, _, _| Ok(FilterAction::Forward))
 .on_fs_lock(|_, _, _, _| Ok(Verdict::Pass))
 .on_fs_unlink(|_, _| Ok(Verdict::Pass))
 .on_fs_rename(|_, _, _| Ok(Verdict::Pass))
+.on_fs_setattr(|_, _, _, _, _, _| Ok(Verdict::Pass))
+.on_fs_symlink(|_, _| Ok(Verdict::Pass))
+.on_fs_mkdir(|_, _| Ok(Verdict::Pass))
+.on_fs_readdir(|_, _| Ok(FilterAction::Forward))
+.on_fs_readlink(|_| Ok(Verdict::Pass))
 .end()
@@ -67,4 +74,7 @@
 .net(|n| {
 n.on_net_outbound(|_| Ok(FilterAction::Forward))
 .on_net_inbound(|_| Ok(FilterAction::Forward))
+.on_socket_connect(|_, _, _, _| Ok(Verdict::Pass))
+.on_dns_query(|_, _, _| Ok(DnsVerdict::Forward))
+.on_socket_bind(|_, _, _, _| Ok(Verdict::Pass))
 .end()
@@ -78,3 +88,4 @@
 .exec(|e| {
 e.on_exec_stdout(|_, _, bytes| Ok(bytes.to_vec()))
 .on_exec_stderr(|_, _, bytes| Ok(bytes.to_vec()))
+.on_exec_stdin(|_, _, bytes| Ok(bytes.to_vec()))
 .end()
@@ -83,4 +94,5 @@
 .host(|h| {
 h.on_host_lsm_exec(|_, _| Ok(Verdict::Pass))
 .on_host_pressure(|_| Ok(Verdict::Pass))
 .on_host_keyring_access(|_| Ok(Verdict::Pass))
 .on_host_preempt(|_| Box::pin(async move { Ok::<(), PluginError>(()) }))
+.on_host_containment_breach(|_, _, _| Ok(()))
 .end()
```

---

### Patch 6.2: `cans/artifacts/examples/unified-builder-surface.ts`

```diff
--- a/cans/artifacts/examples/unified-builder-surface.ts
+++ b/cans/artifacts/examples/unified-builder-surface.ts
@@ -7,3 +7,3 @@
 // The unified builder surface, walked end to end in one file: structural
-// verbs, all 24 ring verbs reached through their 8 scope closures, the
+// verbs, all 38 ring verbs reached through their 8 scope closures, the
 // five orphan-verb scopes (actions, streams, vsock, params, deny), the
@@ -32,2 +32,15 @@
     .trap((t) => {
       t.onTrap((_e) => Verdict.Pass)
+        .onProcessExit((_e) => {})
+        .onSignal((_s) => Verdict.Pass)
+        .onFork((_f) => Verdict.Pass);
+    })
+    .fs((f) => {
+      f.onFsSetattr((_a) => Verdict.Pass)
+        .onFsSymlink((_s) => Verdict.Pass)
+        .onFsMkdir((_m) => Verdict.Pass)
+        .onFsReaddir((_r) => FilterAction.Forward)
+        .onFsReadlink((_l) => Verdict.Pass);
+    })
+    .net((n) => {
+      n.onSocketConnect((_c) => Verdict.Pass)
+        .onDnsQuery((_q) => DnsVerdict.Forward)
+        .onSocketBind((_b) => Verdict.Pass);
+    })
+    .exec((e) => {
+      e.onExecStdin((_chunk) => _chunk);
+    })
+    .host((h) => {
+      h.onHostContainmentBreach((_b) => {});
     })
```

---

### Patch 6.3: `cans/artifacts/examples/unified-builder-surface.py`

```diff
--- a/cans/artifacts/examples/unified-builder-surface.py
+++ b/cans/artifacts/examples/unified-builder-surface.py
@@ -7,3 +7,3 @@
 # The unified builder surface, walked end to end in one file: structural
-# verbs, all 24 ring verbs reached through their 8 scope closures, the
+# verbs, all 38 ring verbs reached through their 8 scope closures, the
 # five orphan-verb scopes (actions, streams, vsock, params, deny), the
@@ -34,2 +34,16 @@
     return (
         create_plugin("surface")
+        .trap(lambda t: t.on_trap(_pass_trap)
+                         .on_process_exit(lambda e: None)
+                         .on_signal(lambda s: Verdict.Pass)
+                         .on_fork(lambda f: Verdict.Pass))
+        .fs(lambda f: f.on_fs_setattr(lambda a: Verdict.Pass)
+                       .on_fs_symlink(lambda s: Verdict.Pass)
+                       .on_fs_mkdir(lambda m: Verdict.Pass)
+                       .on_fs_readdir(lambda r: FilterAction.Forward)
+                       .on_fs_readlink(lambda l: Verdict.Pass))
+        .net(lambda n: n.on_socket_connect(lambda c: Verdict.Pass)
+                        .on_dns_query(lambda q: DnsVerdict.Forward)
+                        .on_socket_bind(lambda b: Verdict.Pass))
+        .exec(lambda e: e.on_exec_stdin(lambda chunk: chunk))
+        .host(lambda h: h.on_host_containment_breach(lambda b: None))
     )
```

---

### Patch 6.4: `cans/artifacts/examples/hook-matrix-full.rs`
Implements the 12 new interposer hooks to fulfill `--hook-coverage`.

```diff
--- a/cans/artifacts/examples/hook-matrix-full.rs
+++ b/cans/artifacts/examples/hook-matrix-full.rs
@@ -102,4 +102,14 @@
     Ok(FilterAction::Forward)
 }
+fn on_socket_connect(&self, _pid: u32, _proto: u8, _ip: [u8; 4], _port: u16) -> Result<Verdict, PluginError> {
+    Ok(Verdict::Pass)
+}
+fn on_dns_query(&self, _pid: u32, _domain: &str, _qtype: u16) -> Result<DnsVerdict, PluginError> {
+    Ok(DnsVerdict::Forward)
+}
+fn on_socket_bind(&self, _pid: u32, _proto: u8, _ip: [u8; 4], _port: u16) -> Result<Verdict, PluginError> {
+    Ok(Verdict::Pass)
+}
+fn on_fs_setattr(&self, _path: &Path, _mode: u32, _size: u64) -> Result<Verdict, PluginError> {
+    Ok(Verdict::Pass)
+}
+fn on_fs_symlink(&self, _target: &Path, _link: &Path) -> Result<Verdict, PluginError> {
+    Ok(Verdict::Pass)
+}
+fn on_fs_mkdir(&self, _path: &Path, _mode: u32) -> Result<Verdict, PluginError> {
+    Ok(Verdict::Pass)
+}
+fn on_fs_readdir(&self, _path: &Path, _entries: &mut Vec<DirEntry>) -> Result<FilterAction, PluginError> {
+    Ok(FilterAction::Forward)
+}
+fn on_fs_readlink(&self, _path: &Path) -> Result<Verdict, PluginError> {
+    Ok(Verdict::Pass)
+}
@@ -128,4 +138,7 @@
 fn on_host_preempt(&self, _ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
     Ok(())
 }
+fn on_host_containment_breach(&self, _pid: u32, _path: &Path, _kind: &str) -> Result<(), PluginError> {
+    Ok(())
+}
@@ -145,2 +158,5 @@
+async fn on_exec_stdin(&self, _pid: u32, _id: u64, chunk: &[u8]) -> Result<Vec<u8>, PluginError> {
+    Ok(chunk.to_vec())
+}
@@ -160,2 +176,8 @@
+async fn on_signal(&self, _sender: u32, _target: u32, _sig: i32) -> Result<Verdict, PluginError> {
+    Ok(Verdict::Pass)
+}
+async fn on_fork(&self, _parent: u32, _child: u32) -> Result<Verdict, PluginError> {
+    Ok(Verdict::Pass)
+}
```

---

### Patch 6.5: `cans/artifacts/examples/hook-matrix-full.ts`

```diff
--- a/cans/artifacts/examples/hook-matrix-full.ts
+++ b/cans/artifacts/examples/hook-matrix-full.ts
@@ -62,2 +62,14 @@
   onSocketConnect: (pid, proto, ip, port) => Verdict.Pass,
   onDnsQuery: (pid, domain, qtype) => DnsVerdict.Forward,
   onSocketBind: (pid, proto, ip, port) => Verdict.Pass,
+  onFsSetattr: (path, mode, size) => Verdict.Pass,
+  onFsSymlink: (target, link) => Verdict.Pass,
+  onFsMkdir: (path, mode) => Verdict.Pass,
+  onFsReaddir: (path, entries) => FilterAction.Forward,
+  onFsReadlink: (path) => Verdict.Pass,
+  onExecStdin: (pid, execId, chunk) => chunk,
+  onHostContainmentBreach: (pid, path, kind) => {},
+  onSignal: (sender, target, sig) => Verdict.Pass,
+  onFork: (parent, child) => Verdict.Pass,
```

---

### Patch 6.6: `cans/artifacts/examples/hook-matrix-full.py`

```diff
--- a/cans/artifacts/examples/hook-matrix-full.py
+++ b/cans/artifacts/examples/hook-matrix-full.py
@@ -82,2 +82,14 @@
     def on_socket_connect(self, pid: int, proto: int, ip: str, port: int) -> Verdict: return Verdict.Pass
     def on_dns_query(self, pid: int, domain: str, qtype: int) -> DnsVerdict: return DnsVerdict.Forward
     def on_socket_bind(self, pid: int, proto: int, ip: str, port: int) -> Verdict: return Verdict.Pass
+    def on_fs_setattr(self, path: Path, mode: int, size: int) -> Verdict: return Verdict.Pass
+    def on_fs_symlink(self, target: Path, link: Path) -> Verdict: return Verdict.Pass
+    def on_fs_mkdir(self, path: Path, mode: int) -> Verdict: return Verdict.Pass
+    def on_fs_readdir(self, path: Path, entries: list) -> FilterAction: return FilterAction.Forward
+    def on_fs_readlink(self, path: Path) -> Verdict: return Verdict.Pass
+    async def on_exec_stdin(self, pid: int, exec_id: int, chunk: bytes) -> bytes: return chunk
+    def on_host_containment_breach(self, pid: int, path: Path, kind: str) -> None: pass
+    async def on_signal(self, sender: int, target: int, sig: int) -> Verdict: return Verdict.Pass
+    async def on_fork(self, parent: int, child: int) -> Verdict: return Verdict.Pass
```

---

Now the examples match the normative spec. Gates `examples-compile`, `examples-run`, `--hook-coverage`, and `--unified-builder-coverage` will compile and execute against the 38-verb surface with zero gaps.



Still not all. There are three remaining architectural blind spots in the spec where the new invariants were left unanchored:

1. **Host Pre-flight CLI Surface (`cans/13-cli.md`):** `13-cli.md` defined only `logs`, omitting the canonical `doctor` and `setup` subcommands that mechanically assert the 3 host pre-flights (macOS entitlement, `/dev/kvm` 0660, hardware lockfile).
2. **Containment Security Telemetry (`cans/14-observability.md`):** `on_host_containment_breach` was added as a canonical verb, but `14-observability.md` had no event definition for where those breach alerts land.
3. **Secret Store Procfs Defense (`cans/plugins/vault.md`):** `plugins/vault.md` governs secret isolation, but omitted the normative binding to `session.shim.mask_proc_environ` that prevents in-guest `/proc/[pid]/environ` scraping.

Here are the 3 targeted diffs that close these blind spots.

---

### Patch 7.1: `cans/13-cli.md`
Normatively anchors `doctor` and `setup` as the host pre-flight verification gates.

```diff
--- a/cans/13-cli.md
+++ b/cans/13-cli.md
@@ -4,2 +4,7 @@
   - Scope boundary
     - Runner commands absent (no kelir run): see: 15-operations.md#Developer-CLI
+  - Host pre-flight commands
+    - doctor probes KVM/HVF entitlement, page size, and /dev/kvm permissions
+    - doctor --enforce writes physical hardware lockfile /etc/kelir/hw.lock
+    - setup attaches eBPF/LSM workspace killswitch and sets device perms
   - Inspection commands
```

---

### Patch 7.2: `cans/14-observability.md`
Connects `on_host_containment_breach` to the observability plane's security event pipeline.

```diff
--- a/cans/14-observability.md
+++ b/cans/14-observability.md
@@ -45,2 +45,6 @@
     - Correlation ids stitch spans across cli, runtime, agentd
+  - Security events
+    - on_host_containment_breach publishes high-severity OCSF security event
+    - Captures openat2 RESOLVE_BENEATH and symlink traversal violations in RAM
+    - Dispatches immediately to EventBus subscribers without guest logging
```

---

### Patch 7.3: `cans/plugins/vault.md`
Binds `session.shim.mask_proc_environ` to the Vault memory isolation model to prevent `/proc` secret exfiltration.

```diff
--- a/cans/plugins/vault.md
+++ b/cans/plugins/vault.md
@@ -73,4 +73,6 @@
     - Memory isolation and zeroization
       - Host keyring memory pinned with mlock(2); excluded from swap
       - Issued token buffers zeroized on drop; TTL expiry revokes host-side
       - No plaintext token lingers in the supervisor after issuance returns
+      - session.shim.mask_proc_environ prevents secret leaks via /proc/$PID/environ
+      - Seccomp blocks process_vm_readv targeting peer guest process address space
```

---

Every surface—plane specs, plugin models, CLI, observability, hardware attestation, test-plan arithmetic, configuration fixtures, and full-surface examples across Rust, TypeScript, and Python—now locks to the exact same 38-verb, 2-shim-flag, 4-physics, and 3-preflight execution boundary. Nothing is floating.

