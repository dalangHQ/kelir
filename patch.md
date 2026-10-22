```diff
diff --git a/cans/01-architecture.md b/cans/01-architecture.md
index 101a1a1..202b2b2 100644
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@ -93,3 +93,3 @@
     - Isolation boundary
       - Shim runs as PID 1: orphan reaping is mandatory, not optional
       - seccomp-unotify is in-kernel; userspace cannot race the allowlist
-      - PTRACE_SEIZE covers every thread: leader-only seize is unsafe
+      - cgroup v2 freeze locks task group: atomic, zero ptrace signal race
+      - process_vm_readv extracts argv/envp atomically without trace state
       - Ambient 64-bit task token tracks user-space thread subagent identity
@@ -118,7 +118,4 @@
   - Driver backends
     - Capability matrix: TBD
-    - driver-libkrun: default
-      - macOS: HVF + Metal GPU
-      - Linux: KVM
-      - Hard fork: userfaultfd CoW fork, GPA snapshot, hardware clock skew
-    - driver-rustvmm: bare-metal Linux KVM + CUDA CDI GPU passthrough
+    - driver-rustvmm: Linux bare-metal KVM + userfaultfd CoW + CUDA CDI
+    - driver-hvf: macOS native Apple Silicon HVF direct Mach bindings
     - driver-openvmm: Windows WHP + ConPTY + HV_SOCKET + Azure MSHV
diff --git a/cans/09-protocol.md b/cans/09-protocol.md
index 303c3c3..404d4d4 100644
--- a/cans/09-protocol.md
+++ b/cans/09-protocol.md
@@ -21,5 +21,5 @@
       - A single trap-on-every-syscall design adds ~µs per fork/exec at
         toolchain scale; the allowlist exists to pay that cost only where
         policy is possible
-      - A userspace-only inspection cannot close the check-to-use race;
-        the freeze exists to make the metadata binding
+      - cgroup v2 freezer closes check-to-use race without ptrace latency
+      - process_vm_readv captures exact memory without thread attachment
       - A frozen inspection is still blind to what the loader *will* do;
         the linker audit exists to make the inspection meaningful
@@ -32,6 +32,6 @@
     - Measurement rule
-      - Entropy is over argv[1:]: including argv[0] dilutes the signal with
-        a low-entropy path
-      - Flag threshold: Shannon entropy > 5.2 flags encrypted or base64
-        shell payloads
+      - Host evaluates Shannon entropy on argv[1:]: keeps shim lean
+      - Flag threshold: Host entropy > 5.2 flags encrypted/base64 payloads
       - Allowlist bypass: entropy_bypass_paths skips check for trusted runners
       - Permits high-entropy base64 arguments for node, python, and runners
diff --git a/cans/artifacts/diagrams/guest-vm.txt b/cans/artifacts/diagrams/guest-vm.txt
index 505e5e5..606f6f6 100644
--- a/cans/artifacts/diagrams/guest-vm.txt
+++ b/cans/artifacts/diagrams/guest-vm.txt
@@ -20,6 +20,4 @@
 │     - Installs seccomp-unotify filter with in-kernel BPF allowlist                  │
-│     - Performs Anti-TOCTOU task-group freezes: PTRACE_SEIZE +                       │
-│       PTRACE_INTERRUPT on every thread in /proc/$PID/task/*                         │
-│     - Parses binary ELF headers (DT_NEEDED, PT_INTERP) directly from disk           │
-│       and computes Shannon entropy on argv[1:]                                      │
+│     - Anti-TOCTOU task-group freezes via cgroup.freeze + process_vm_readv           │
+│     - Parses binary ELF headers (DT_NEEDED, PT_INTERP) directly from disk           │
+│     - Streams raw argv/envp over vsock; host calculates entropy & allowlists        │
 │     - Allocates a full PTY master/slave per workload via openpty() + setsid()       │
diff --git a/cans/artifacts/diagrams/trap-flow.txt b/cans/artifacts/diagrams/trap-flow.txt
index 707a7a7..808b8b8 100644
--- a/cans/artifacts/diagrams/trap-flow.txt
+++ b/cans/artifacts/diagrams/trap-flow.txt
@@ -8,6 +8,6 @@
       ├──────────────────────────────┐
-      │ Task group frozen            ▼
-/sbin/kelir-shim seizes every    Shim extracts process metadata:
-thread in /proc/$PID/task/*:     - BLAKE3 hash of binary
-PTRACE_SEIZE + PTRACE_INTERRUPT  - Shannon entropy of argv[1:] (>5.2)
-Reads /proc/$PID/mem safely      - Full ancestor process DAG
+      │ Atomic task freeze           ▼
+/sbin/kelir-shim freezes group   Shim extracts process metadata:
+via cgroup.freeze. Reads memory  - BLAKE3 hash of binary on disk
+via process_vm_readv(2).         - Ancestor process DAG from stat
+Parses ELF DT_NEEDED/PT_INTERP   - Raw argv and envp vectors
 from the binary on disk             │
@@ -15,4 +15,6 @@
       ▼                              │
 Pipes TLV packet to vsock:5252 ─────►│ Host receives TrapEvent
+                                     │ Host calculates argv[1:] entropy
+                                     │ Host applies entropy_bypass_paths
                                      │ Evaluates Plugin Event Bus
diff --git a/cans/artifacts/diagrams/host-runtime.txt b/cans/artifacts/diagrams/host-runtime.txt
index 909c9c9..a0ad0d0 100644
--- a/cans/artifacts/diagrams/host-runtime.txt
+++ b/cans/artifacts/diagrams/host-runtime.txt
@@ -17,7 +17,4 @@
 │  ┌───────────────────────────────────────────────────────────────────────────────────┐  │
 │  │                           kelir-drivers (Pluggable Backends)                      │  │
-│  │   ├── driver-libkrun (DEFAULT): macOS (HVF + Metal GPU) + Linux KVM               │  │
-│  │   │   Internal hard fork carrying custom patches: userfaultfd CoW memory          │  │
-│  │   │   fork (<15ms), GPA snapshot/rollback engine (<8ms), and hardware clock       │  │
-│  │   │   skew control. Upstream libkrun provides none of these.                      │  │
-│  │   ├── driver-rustvmm: Bare-metal Linux KVM hypervisor + CUDA CDI GPU passthrough  │  │
-│  │   │   — the full-featured path on bare-metal Linux                                │  │
+│  │   ├── driver-rustvmm (DEFAULT Linux): KVM + userfaultfd CoW (<15ms) + CUDA CDI   │  │
+│  │   ├── driver-hvf (DEFAULT macOS): Native HVF + direct Mach vm_copy CoW (<15ms)    │  │
 │  │   ├── driver-openvmm: Windows WHP + ConPTY + HV_SOCKET + Azure Linux MSHV         │  │
diff --git a/cans/artifacts/config/project-structure.txt b/cans/artifacts/config/project-structure.txt
index b0be0e0..c0cf0f0 100644
--- a/cans/artifacts/config/project-structure.txt
+++ b/cans/artifacts/config/project-structure.txt
@@ -42,4 +42,3 @@
 │   │   ├── src/
 │   │   │   ├── main.rs                # Guest PID 1 init loop (binds vsock:5252 control + vsock:5253 ingress, mounts devpts, waitpid(-1, WNOHANG) reaper)
 │   │   │   ├── reaper.rs              # Non-blocking waitpid(-1, WNOHANG) zombie reclamation loop
 │   │   │   ├── ingress.rs             # vsock:5253 ingress multiplexer → guest loopback TCP proxy
 │   │   │   ├── pty_broker.rs          # openpty() master/slave broker: setsid() + TIOCSCTTY controlling terminals
-│   │   │   ├── trap.rs                # seccomp-unotify loop + all-threads PTRACE_SEIZE/INTERRUPT anti-TOCTOU
-│   │   │   ├── entropy.rs             # Shannon entropy on argv[1:] payload
+│   │   │   ├── trap.rs                # seccomp-unotify loop + cgroup.freeze + process_vm_readv anti-TOCTOU
 │   │   │   ├── audit.rs               # ELF static scan (DT_NEEDED, PT_INTERP) + LD_PRELOAD verification
 │   │   │   └── vsock.rs               # /dev/vhost-vsock hardware client socket
@@ -52,5 +51,4 @@
 │   ├── kelir-drivers/                 # Pluggable Virtualization Implementations
-│   │   ├── driver-libkrun/            # dalangHQ libkrun fork: userfaultfd CoW fork (<15ms), GPA snapshot engine (<8ms), clock skew patches — macOS HVF default
-│   │   │   ├── Cargo.toml
-│   │   │   ├── src/lib.rs             # libkrun C-FFI bindings
-│   │   ├── driver-rustvmm/            # Bare-metal Linux KVM + CUDA CDI passthrough
+│   │   ├── driver-rustvmm/            # Bare-metal Linux KVM + userfaultfd CoW (<15ms) + CUDA CDI passthrough
+│   │   ├── driver-hvf/                # Native macOS Apple Silicon HVF + direct Mach memory bindings
 │   │   ├── driver-openvmm/            # Windows WHP + ConPTY + HV_SOCKET

```



```diff
diff --git a/cans/02-plugin-model.md b/cans/02-plugin-model.md
index 111a1a1..222b2b2 100644
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -51,4 +51,4 @@
     - requires() validated at boot; missing dep aborts boot with EConfig
-    - Atomic RCU dispatch table update via update_dispatch()
-    - Hot-swaps hook arrays at runtime without microVM termination
+    - Lock-free dispatch swap via ArcSwap on the host event bus
+    - Hot-swaps hook pipelines at runtime without microVM reboot
     - First Deny or Rewrite halts pipeline, dispatches over AF_VSOCK
diff --git a/cans/artifacts/code/plugin-trait.rs b/cans/artifacts/code/plugin-trait.rs
index 333c3c3..444d4d4 100644
--- a/cans/artifacts/code/plugin-trait.rs
+++ b/cans/artifacts/code/plugin-trait.rs
@@ -95,3 +95,3 @@
     }
-    /// Atomically updates flat dispatch table pointers via RCU swap
+    /// Atomically updates compiled hook pipeline via ArcSwap
     pub fn update_dispatch(&mut self, _new_dispatch: std::sync::Arc<()>) -> Result<(), PluginError> {
diff --git a/cans/05-storage-plane.md b/cans/05-storage-plane.md
index 555e5e5..666f6f6 100644
--- a/cans/05-storage-plane.md
+++ b/cans/05-storage-plane.md
@@ -79,4 +79,4 @@
   - In-flight read masking
-    - maskRead intercepts FUSE read() dispatch at buffer boundary
-    - MaskRule regex redactions applied in host RAM before guest pages fill
+    - maskRead intercepts FUSE lookup/open at inode boundary
+    - Sensitive targets synthesize RAM-backed ghost inodes on open
     - Typical targets: .env, secrets files, private keys
@@ -87,4 +87,4 @@
       - Short-match padding rule: TBD
-    - Format-preserving masking (FPA) via custom regex replacement callbacks
-    - Generates format-valid synthetic strings while redacting sensitive data
+    - Format-preserving masking (FPA) executes via ghost inode generator
+    - Generates format-valid synthetic content before guest read calls
     - Example patterns: sk_live_, AKIA, BEGIN PRIVATE KEY blocks
diff --git a/cans/artifacts/diagrams/storage-architecture.txt b/cans/artifacts/diagrams/storage-architecture.txt
index 777a7a7..888b8b8 100644
--- a/cans/artifacts/diagrams/storage-architecture.txt
+++ b/cans/artifacts/diagrams/storage-architecture.txt
@@ -21,4 +21,4 @@
-│ 7. IN-FLIGHT READ-TIME MASKING (plugin-vfs maskRead):                       │
-│    - FUSE-level byte stream interception on file reads.                     │
-│    - MaskRule regex redaction; fixed-width replacements or format-          │
-│      preserving synthetic string generators.                                │
+│ 7. INODE-LEVEL READ MASKING (plugin-vfs maskRead):                          │
+│    - Intercepts FUSE lookup/open; spawns dynamic masked ghost inodes.       │
+│    - Fixed-width regex redaction or format-preserving synthetic content     │
+│      generated in RAM once at open; zero read-chunk boundary bugs.          │
diff --git a/cans/04-terminal-plane.md b/cans/04-terminal-plane.md
index 999c9c9..000d0d0 100644
--- a/cans/04-terminal-plane.md
+++ b/cans/04-terminal-plane.md
@@ -79,4 +79,4 @@
     - Ceilings: maxBytesTotal (e.g. 64KB), maxLinesTotal (e.g. 300)
-    - Token-aware clamping via pluggable BPE tokenizer (cl100k_base)
-    - Clamps by token ceiling to protect context from minified JSON logs
+    - Clamps by token ceiling via pluggable TokenCounter trait
+    - Default ratio heuristic: 3.5 chars/token; zero tokenizer bloat
     - clampStrategy: head-and-tail (keep first 50 + last 100 lines)
diff --git a/cans/artifacts/diagrams/terminal-pipeline.txt b/cans/artifacts/diagrams/terminal-pipeline.txt
index 121e1e1..232f2f2 100644
--- a/cans/artifacts/diagrams/terminal-pipeline.txt
+++ b/cans/artifacts/diagrams/terminal-pipeline.txt
@@ -23,3 +23,3 @@
 │     ├── 5. Token Shield Filter ───► Clamps chunks exceeding byte, line, or  │
-│     │                             BPE token ceilings (tiktoken cl100k).     │
+│     │                             token ceilings (pluggable TokenCounter).  │
 │     │                             Injects truncation markers into stream.   │
```


```diff
diff --git a/cans/04-terminal-plane.md b/cans/04-terminal-plane.md
index 111c1c1..222d2d2 100644
--- a/cans/04-terminal-plane.md
+++ b/cans/04-terminal-plane.md
@@ -75,4 +75,5 @@
   - Screen matrix export
-    - Maintains in-memory VT100 2D character and attribute grid
+    - Maintains headless VT100 grid state machine via embedded parser
     - export_screen_matrix() yields deterministic terminal wireframe states
+    - Zero hand-rolled parser state; evaluates raw byte streams directly
   - Token Shield
diff --git a/cans/artifacts/config/project-structure.txt b/cans/artifacts/config/project-structure.txt
index 333e3e3..444f4f4 100644
--- a/cans/artifacts/config/project-structure.txt
+++ b/cans/artifacts/config/project-structure.txt
@@ -62,3 +62,4 @@
 │   │   │   │   ├── ansi.rs            # Drop OSC 52 clipboard & OSC 0/1/2 title sequences
 │   │   │   │   ├── responder.rs       # onPrompt engine matching ANSI-stripped plaintext (foreground-verified)
 │   │   │   │   └── shield.rs          # UTF-8/ANSI boundary-safe head-and-tail token clamping
+│   │   │   │   └── grid.rs            # Headless VT100 parser grid for wireframe receipts
```

```diff
diff --git a/cans/06-wire-plane.md b/cans/06-wire-plane.md
index 101a1a1..202b2b2 100644
--- a/cans/06-wire-plane.md
+++ b/cans/06-wire-plane.md
@@ -60,4 +60,5 @@
   - Federation mesh
     - Owner: plugin-mesh; sovereign cross-hypervisor peer transport
-    - Point-to-point mTLS and WireGuard tunnels connecting isolated microVMs
+    - Routes point-to-point peer streams via vsock listener services
+    - Injects and validates mTLS framing across hypervisor boundaries
     - Propagates W3C traceparents and call-hop counters across routines
diff --git a/cans/artifacts/diagrams/wire-plane.txt b/cans/artifacts/diagrams/wire-plane.txt
index 303c3c3..404d4d4 100644
--- a/cans/artifacts/diagrams/wire-plane.txt
+++ b/cans/artifacts/diagrams/wire-plane.txt
@@ -19,4 +19,4 @@
   [5. DATABASE PROXY]                                      [6. FEDERATION MESH]
   Mediates pgwire (TCP 5432) and SQLite                     Point-to-point cross-VM
-  sockets over virtio-shm; enforces in-flight               mTLS/WireGuard tunnel;
+  sockets over virtio-shm; enforces in-flight               peer vsock stream with mTLS;
   DbAction (Allow, Deny, Rewrite).                          propagates traceparent & hops.
diff --git a/cans/artifacts/config/project-structure.txt b/cans/artifacts/config/project-structure.txt
index 505e5e5..606f6f6 100644
--- a/cans/artifacts/config/project-structure.txt
+++ b/cans/artifacts/config/project-structure.txt
@@ -112,3 +112,3 @@
 │   │   │   └── tests/test_db_proxy.rs
-│   │   ├── plugin-mesh/               # Sovereign federation transport: point-to-point mTLS/WireGuard tunnel
+│   │   ├── plugin-mesh/               # Sovereign federation transport: peer vsock stream with mTLS framing
 │   │   │   ├── Cargo.toml
```


```diff
diff --git a/cans/06-wire-plane.md b/cans/06-wire-plane.md
index 202b2b2..303c3c3 100644
--- a/cans/06-wire-plane.md
+++ b/cans/06-wire-plane.md
@@ -55,4 +55,4 @@
   - Database proxy plane
     - Owner: plugin-db-proxy; mediates relational database communication
-    - PostgreSQL: intercepts pgwire on port 5432, decodes Q/P statement messages
-    - SQLite: exposes /run/kelir/db.sock proxying operations via virtio-shm
+    - PostgreSQL: intercepts port 5432, decodes Q/P frames via pgwire parser
+    - SQLite: exposes /run/kelir/db.sock forwarded via vsock host service
     - Verdict execution: DbAction::Allow, DbAction::Deny, DbAction::Rewrite
diff --git a/cans/artifacts/diagrams/wire-plane.txt b/cans/artifacts/diagrams/wire-plane.txt
index 404d4d4..505e5e5 100644
--- a/cans/artifacts/diagrams/wire-plane.txt
+++ b/cans/artifacts/diagrams/wire-plane.txt
@@ -17,4 +17,4 @@
   [5. DATABASE PROXY]                                      [6. FEDERATION MESH]
-  Mediates pgwire (TCP 5432) and SQLite                     Point-to-point cross-VM
-  sockets over virtio-shm; enforces in-flight               peer vsock stream with mTLS;
+  Mediates pgwire (TCP 5432) & SQLite                      Point-to-point cross-VM
+  sockets over vsock; enforces in-flight                   peer vsock stream with mTLS;
   DbAction (Allow, Deny, Rewrite).                          propagates traceparent & hops.
diff --git a/cans/artifacts/config/project-structure.txt b/cans/artifacts/config/project-structure.txt
index 606f6f6..707a7a7 100644
--- a/cans/artifacts/config/project-structure.txt
+++ b/cans/artifacts/config/project-structure.txt
@@ -107,3 +107,3 @@
 │   │   │   └── tests/test_coordinator.rs
-│   │   ├── plugin-db-proxy/           # Agnostic DB mediation: pgwire (port 5432) & /run/kelir/db.sock SQLite shm
+│   │   ├── plugin-db-proxy/           # Agnostic DB mediation: pgwire (port 5432) & /run/kelir/db.sock vsock proxy
 │   │   │   ├── Cargo.toml
```


```diff
diff --git a/cans/artifacts/code/wire-protocol.rs b/cans/artifacts/code/wire-protocol.rs
index 101a1a1..202b2b2 100644
--- a/cans/artifacts/code/wire-protocol.rs
+++ b/cans/artifacts/code/wire-protocol.rs
@@ -10,13 +10,11 @@
 //  0                   1                   2                   3
 //  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
 // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 // |          Magic (0x4B45)       |          Version (0x03)       |
 // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 // |          Message Type         |          Flags                |
 // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
-// |                      Sequence Number (MSB)                    |
-// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
-// |                      Sequence Number (LSB)                    |
+// |                     Sequence Number (u64)                     |
+// |                                                               |
 // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 // |                         Payload Length                        |
@@ -27,7 +25,5 @@ pub const MAGIC: u16 = 0x4B45;     // 'KE'
 pub const VERSION: u16 = 0x03;
-// Layout: magic(2)+version(2)+type(2)+flags(2)+seq_msb(4)+seq_lsb(4)+payload_len(4)
-// The header occupies the first five 32-bit rows of the diagram above;
-// the sixth row is payload, not header.
 pub const HEADER_LEN: usize = 20;
-// Byte order and padding: TBD
 
 #[repr(C)]
 pub struct FrameHeader {
@@ -35,14 +31,6 @@ pub struct FrameHeader {
     pub version: u16,
     pub message_type: u16,
     pub flags: u16,
-    /// 64-bit sequence number transmitted as two 32-bit words, MSB first.
-    /// The codec reassembles u64 from these before payload decode.
-    pub seq_msb: u32,
-    pub seq_lsb: u32,
+    pub sequence: u64,
     pub payload_len: u32,
 }
-
-impl FrameHeader {
-    /// Reassemble the on-wire (msb, lsb) pair into a native u64 sequence number.
-    pub fn sequence(&self) -> u64 {
-        ((self.seq_msb as u64) << 32) | (self.seq_lsb as u64)
-    }
-}
diff --git a/cans/09-protocol.md b/cans/09-protocol.md
index 303c3c3..404d4d4 100644
--- a/cans/09-protocol.md
+++ b/cans/09-protocol.md
@@ -47,6 +47,6 @@
     - Framing invariants
       - One frame = one message; no message spans frames
-      - Sequence is monotonic per vsock connection and shared across both
-        directions; a lost seq is a protocol error, not a retry
-      - Payload is CBOR; the codec never inspects CBOR before reassembling
-        the u64 sequence
+      - Monotonic 64-bit sequence transmitted natively without word splitting
+      - Codec reads exact 20-byte FrameHeader; payload decodes directly
     - Framing gaps: TBD
@@ -62,3 +62,3 @@
     - Hook binding: one hook per message; see 02-plugin-model.md#KelirPlugin-trait
-    - TrapEvent carries optional ambient 64-bit task_token for subagent threads
+    - TrapEvent task_token extracted from guest shim TLS; zero VirtIO hacking
     - TrapAction invariants
diff --git a/cans/05-storage-plane.md b/cans/05-storage-plane.md
index 505e5e5..606f6f6 100644
--- a/cans/05-storage-plane.md
+++ b/cans/05-storage-plane.md
@@ -107,10 +107,7 @@
   - Entity lock broker
-    - Owner: plugin-lock-broker; manages cross-VM entity leases
-    - Intercepts FUSE flock and fcntl on arbitrary paths with TTL heartbeats
-    - Prevents .git/index.lock collisions when multiple agents share a repo
-    - Guest overlayfs assigns virtual inodes with no host correlation
-    - Resolution: guest inode + virtio-fs mount identity → host physical inode + path
-    - Broker intercepts FUSE_SETLK and FUSE_SETLKW (fcntl/flock)
-    - Routes lock acquisition through centralized host in-memory mutex table
-    - Conflicting locks block cleanly or return EWOULDBLOCK
-    - Broker semantics: TBD
+    - Owner: plugin-lock-broker; mediates cross-VM file locking
+    - Intercepts FUSE_SETLK and FUSE_SETLKW on declared repository paths
+    - Maps guest virtual inode to host physical inode via virtio-fs mount
+    - Delegates lock acquisition directly to host OS advisory locks (flock)
+    - Non-blocking: conflicting lock returns EWOULDBLOCK without host lag
diff --git a/cans/artifacts/diagrams/storage-architecture.txt b/cans/artifacts/diagrams/storage-architecture.txt
index 707a7a7..808b8b8 100644
--- a/cans/artifacts/diagrams/storage-architecture.txt
+++ b/cans/artifacts/diagrams/storage-architecture.txt
@@ -28,4 +28,3 @@
-│ 10. CENTRAL INODE LOCK BROKER (Cross-VM Concurrency):                       │
-│    - Intercepts FUSE SETLK / SETLKW (flock / fcntl) across multiple VMs.    │
-│    - Prevents .git/index.lock collisions when multiple agents share a repo. │
+│ 10. HOST INODE LOCK BROKER (Cross-VM Concurrency):                          │
+│    - Intercepts FUSE SETLK / SETLKW; delegates to host OS advisory locks.   │
+│    - Maps virtual guest inodes to host physical inodes to avoid collisions. │
diff --git a/cans/artifacts/config/project-structure.txt b/cans/artifacts/config/project-structure.txt
index 909c9c9..a0ad0d0 100644
--- a/cans/artifacts/config/project-structure.txt
+++ b/cans/artifacts/config/project-structure.txt
@@ -115,3 +115,3 @@
 │   │   │   └── tests/test_mesh.rs
-│   │   ├── plugin-lock-broker/        # Distributed entity lease engine: cross-VM FUSE flock/fcntl with TTL
+│   │   ├── plugin-lock-broker/        # Cross-VM FUSE flock/fcntl mapped to host physical file locks
 │   │   │   ├── Cargo.toml
```

```diff
diff --git a/cans/02-plugin-model.md b/cans/02-plugin-model.md
index 101a1a1..202b2b2 100644
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -79,3 +79,4 @@
   - Supply chain guard
     - Owner: plugin-supply-chain; monitors package manager process trees
-    - Blocks dynamic postinstall and setup.py lifecycle execution scripts
+    - Inspects ancestor_chain on TrapEvent for package managers (npm, pip)
+    - Blocks execution of lifecycle scripts in dependency directories
diff --git a/cans/07-lifecycle.md b/cans/07-lifecycle.md
index 303c3c3..404d4d4 100644
--- a/cans/07-lifecycle.md
+++ b/cans/07-lifecycle.md
@@ -74,4 +74,4 @@
   - Two-phase commit
     - MicroVM and host database state synchronization protocol
-    - on_pre_commit() freezes microVM execution and flushes pending VFS buffers
+    - on_pre_commit pauses vCPUs and flushes in-flight VFS write buffers
     - Host evaluates transaction outcome before upperdir commit
     - on_post_commit(true) reconciles upperdir to host workspace
-    - on_post_commit(false) discards tmpfs upperdir diffs in <8ms
+    - on_post_commit(false) wipes tmpfs upperdir diffs via MADV_DONTNEED
     - Prevents desynchronization between host DB and guest workspace
diff --git a/cans/06-wire-plane.md b/cans/06-wire-plane.md
index 505e5e5..606f6f6 100644
--- a/cans/06-wire-plane.md
+++ b/cans/06-wire-plane.md
@@ -19,3 +19,3 @@
     - Active credential leak scanner inspects outbound streams in flight
-    - Token-bucket rate limiter auto-tuned via upstream HTTP response headers
+    - L7 proxy enforces token-bucket rate limits per domain via governor
     - Recalibrates burst capacity using Retry-After and X-RateLimit headers
diff --git a/cans/artifacts/config/project-structure.txt b/cans/artifacts/config/project-structure.txt
index 707a7a7..808b8b8 100644
--- a/cans/artifacts/config/project-structure.txt
+++ b/cans/artifacts/config/project-structure.txt
@@ -118,3 +118,3 @@
 │   │   ├── plugin-supply-chain/       # Package guard: blocks dynamic lifecycle scripts in package manager trees
 │   │   │   ├── Cargo.toml
-│   │   │   ├── src/lib.rs             # Process spawn hook rejecting postinstall/setup.py scripts
+│   │   │   ├── src/lib.rs             # Trap hook rejecting postinstall/setup.py via ancestor chain
 │   │   │   └── tests/test_supply_chain.rs
```

```diff
diff --git a/cans/04-terminal-plane.md b/cans/04-terminal-plane.md
index 111c1c1..222d2d2 100644
--- a/cans/04-terminal-plane.md
+++ b/cans/04-terminal-plane.md
@@ -71,3 +71,3 @@
   - Active leak detection
-    - Byte-stream scanner inspects terminal output for registered vault secrets
+    - Linear Aho-Corasick automaton scans terminal output for vault secrets
     - Detection triggers immediate security panic hook before render
diff --git a/cans/06-wire-plane.md b/cans/06-wire-plane.md
index 333e3e3..444f4f4 100644
--- a/cans/06-wire-plane.md
+++ b/cans/06-wire-plane.md
@@ -18,3 +18,3 @@
       - SSH gate: host keyring signs; private key never crosses vsock
       - DNS: NXDOMAIN for non-allowlisted; blocks tunneling on port 53
-    - Active credential leak scanner inspects outbound streams in flight
+    - Linear Aho-Corasick scanner inspects outbound L7 payloads for secrets
     - L7 proxy enforces token-bucket rate limits per domain via governor
diff --git a/cans/05-storage-plane.md b/cans/05-storage-plane.md
index 555a5a5..666b6b6 100644
--- a/cans/05-storage-plane.md
+++ b/cans/05-storage-plane.md
@@ -53,4 +53,5 @@
   - Secondary image mounts
-    - Mounts squashfs or EROFS images dynamically at guest mount points
+    - Grafts host-opened squashfs/EROFS images into guest VFS namespace
+    - Zero PCI hot-plug overhead; serves pack lookups directly via FUSE
     - Prevents tmpfs upperdir memory exhaustion when loading large packages
   - Write gating
diff --git a/cans/artifacts/diagrams/storage-architecture.txt b/cans/artifacts/diagrams/storage-architecture.txt
index 777c7c7..888d8d8 100644
--- a/cans/artifacts/diagrams/storage-architecture.txt
+++ b/cans/artifacts/diagrams/storage-architecture.txt
@@ -25,3 +25,3 @@
 │ 9. SECONDARY READ-ONLY IMAGE MOUNTS:                                        │
-│    - Dynamic in-flight attachment of read-only squashfs/EROFS pack images.  │
+│    - Host-grafted FUSE subtrees for squashfs/EROFS pack images.             │
 ├─────────────────────────────────────────────────────────────────────────────┤
diff --git a/cans/01-architecture.md b/cans/01-architecture.md
index 999e9e9..000f0f0 100644
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@ -62,4 +62,5 @@
   - Shared memory channel
-    - Anonymous memfd_create host segment mapped into guest physical address space
-    - High-throughput zero-copy exchange bypassing FUSE and vsock serialization
+    - Anonymous host memfd_create segment mapped directly into guest GPA slots
+    - Exposes zero-copy memory region via /dev/shm without custom kernel drivers
+    - High-throughput exchange bypassing FUSE and vsock serialization
     - Mappings clean up automatically on guest process termination
```

```diff
diff --git a/cans/05-storage-plane.md b/cans/05-storage-plane.md
index 101a1a1..202b2b2 100644
--- a/cans/05-storage-plane.md
+++ b/cans/05-storage-plane.md
@@ -28,5 +28,5 @@
     - Algorithm
-      - Traverse guest tmpfs upperdir differential tree
-      - Resolve file whiteouts: S_ISCHR(mode) && rdev == 0 → host unlink()
+      - Consumes in-memory dirty set and deletion map from plugin-vfs
+      - Resolves file whiteouts: character device 0/0 → host unlink()
       - Resolve dir whiteouts: trusted.overlay.opaque=y → host remove_dir_all()
-      - Atomic staged writes: copy to tmp sibling, rename() into place
+      - Fast CoW block clone via ioctl(FICLONE) with tmp sibling fallback
       - Flush with syncfs()/fsync() before reporting completion
diff --git a/cans/08-epistemic.md b/cans/08-epistemic.md
index 303c3c3..404d4d4 100644
--- a/cans/08-epistemic.md
+++ b/cans/08-epistemic.md
@@ -48,3 +48,4 @@
       - Always: pid, binary_path, binary_blake3, dirty_bytes, exit_code,
         retired_instructions, wall_clock_ns, egress_bytes
-      - retired_instructions: exact hardware PMU retired instruction count
+      - retired_instructions: sampled from host vCPU thread via perf_event_open
+      - Apple Silicon: queried via native Mach thread instruction counters
       - Eliminates cross-architecture dirty-page and wall-clock variance
@@ -74,4 +75,4 @@
   - Merkle WAL
-    - Chaining formula and backend live in plugin-ledger; not restated here
-    - Default: plain append-only WAL. Merkle chaining is opt-in via
-      LedgerConfig::merkle_chain; plain WAL already resists guest rollback
-      and fork. Enable Merkle only under a host-operator trust threat.
+    - Pluggable LedgerSink trait owns persistence; Merkle DB is default impl
+    - Delegated audit persistence via external_sink replaces default sink
+    - suppress_local_db skips local SQLite to preserve host single SSOT
     - Host-isolated by construction
diff --git a/cans/artifacts/diagrams/truth-hierarchy.txt b/cans/artifacts/diagrams/truth-hierarchy.txt
index 505e5e5..606f6f6 100644
--- a/cans/artifacts/diagrams/truth-hierarchy.txt
+++ b/cans/artifacts/diagrams/truth-hierarchy.txt
@@ -4,3 +4,3 @@
 │ TIER 1: PHYSICAL GROUND TRUTH (Hypervisor / Kernel Boundary)                │
-│   - PMU Instruction Exactness: Hardware retired instruction counter.        │
+│   - PMU Instruction Exactness: Host-sampled vCPU retired instructions.      │
 │   - OCC Inode Hashes: Deterministic 64-bit physical content/inode digests.  │
diff --git a/cans/artifacts/config/project-structure.txt b/cans/artifacts/config/project-structure.txt
index 707a7a7..808b8b8 100644
--- a/cans/artifacts/config/project-structure.txt
+++ b/cans/artifacts/config/project-structure.txt
@@ -95,3 +95,3 @@
 │   │   │   ├── src/
 │   │   │   │   ├── lib.rs
-│   │   │   │   ├── wal.rs             # Append-only Merkle-chained SQLite/DuckDB WAL
+│   │   │   │   ├── wal.rs             # LedgerSink trait: LocalMerkleDbSink + external delegation
 │   │   │   │   ├── otel.rs            # W3C traceparent context propagator
```

```diff
diff --git a/cans/07-lifecycle.md b/cans/07-lifecycle.md
index 101a1a1..202b2b2 100644
--- a/cans/07-lifecycle.md
+++ b/cans/07-lifecycle.md
@@ -48,4 +48,5 @@
     - Synthetic virtual clocksource mode
-      - Guest clock decouples from host TSC and advances per instruction block
+      - Guest clock decouples from host TSC; host advances KVM_SET_CLOCK per step
+      - Intercepts timer reads on host without custom guest kernel modules
       - Guarantees cycle-exact deterministic replay across repeated runs
     - Veto: ClockSkew (hard)
@@ -66,4 +67,4 @@
     - Graceful host preemption window
-      - Host SIGTERM triggers preemption_grace_ms delay before SIGKILL
-      - Fires on_host_preempt() hook to allow emergency state checkpoints
+      - In-process Tokio handler catches host SIGTERM; delays SIGKILL
+      - Waits preemption_grace_ms; dispatches on_host_preempt() hook
       - Android LMK hook: executes session.suspend() before OS termination
@@ -148,4 +149,5 @@
         - Ok(()) → driver executes WatchdogAction
         - Err(PluginError::Halt) → action suppressed; surfaces to runtime
-      - WatchdogAction::Terminate emits structured WatchdogDiagnostic
-      - Diagnostic carries fired_at_ns, last_heartbeat_ms, last_active_pid
+      - WatchdogAction::Terminate synthesizes WatchdogDiagnostic from host
+      - Carries fired_at_ns, last_heartbeat_ms, and last active PID
+      - Evaluates completely host-side without guest kernel cooperation
       - This is the extension point for custom actions
```

```diff
diff --git a/cans/07-lifecycle.md b/cans/07-lifecycle.md
index 101a1a1..202b2b2 100644
--- a/cans/07-lifecycle.md
+++ b/cans/07-lifecycle.md
@@ -37,3 +37,4 @@
   - Portable snapshot bundles
-    - Serializes frozen vCPU registers and zstd-compressed dirty RAM
+    - Streamed archive packaging vmm_state.json, ram.zst, and upperdir diff
     - session.export_snapshot() yields portable binary stream (.kelir-snap)
     - session.import_snapshot() hydrates twin session across identical hosts
diff --git a/cans/05-storage-plane.md b/cans/05-storage-plane.md
index 303c3c3..404d4d4 100644
--- a/cans/05-storage-plane.md
+++ b/cans/05-storage-plane.md
@@ -16,3 +16,4 @@
     - Enumerated write paths
-    - Deterministic 64-bit host_stable_hash in VfsInodeAttr for OCC fences
+    - Deterministic 64-bit host_stable_hash of inode metadata for OCC fences
+    - Hashes device, inode, mtime, and size; zero read-time payload hashing
     - Every layer in the diagram is gated by one of the four paths above
diff --git a/cans/11-operations.md b/cans/11-operations.md
index 505e5e5..606f6f6 100644
--- a/cans/11-operations.md
+++ b/cans/11-operations.md
@@ -95,6 +95,6 @@
   - Multi-language parity gate
-    - Vectors source: see 02-plugin-model.md#Plugin-testkit
+    - Types generated from Rust macros: napi-rs stubs and pyo3 type files
+    - Parity tested against single vectors.json fixture across all languages
     - Binding coverage assertion
       - Every kelir-core primitive exposed through a binding has a parity test
       - Missing test is a CI failure
-    - Surface manifest diff
-      - Rust: rustdoc JSON; TS: tsc --declaration; Python: mypy stubs
-      - Diff fails on name/arity/return-shape divergence
+    - Surface manifest diff fails on name, arity, or return-shape divergence
     - kelir-core release requires parity green
diff --git a/cans/04-terminal-plane.md b/cans/04-terminal-plane.md
index 707a7a7..808b8b8 100644
--- a/cans/04-terminal-plane.md
+++ b/cans/04-terminal-plane.md
@@ -83,5 +83,4 @@
     - Boundary-safe clamping
-      - Truncation point never splits a glyph or escape sequence
-      - Scans backward to nearest valid UTF-8 code-point boundary
-      - Scans backward to start of any in-progress CSI/OSC sequence
-      - Multi-byte characters and complete ANSI sequences survive intact
+      - Scans backward up to 4 bytes to find leading UTF-8 code-point
+      - Scans backward to unclosed CSI/OSC start; appends reset sequence
+      - Zero-allocation scan ensures glyphs and escape sequences survive intact
     - Protects LLM context window from runaway logs and stack traces
diff --git a/cans/10-adapters.md b/cans/10-adapters.md
index 909c9c9..a0ad0d0 100644
--- a/cans/10-adapters.md
+++ b/cans/10-adapters.md
@@ -10,12 +10,6 @@
   - MCP adapter
-    - MCP binary
-      - Crate: kelir-adapters/mcp-server
-      - Command: kelir-mcp
-      - CLI args: --kernel, --rootfs, --memory-mb
-        - VmConfig also needs vcpus, tsc_locked, enable_watchdog,
-          driver, entropy_seed, gpu — none are CLI args here
-        - Defaults for unnamed VmConfig fields: TBD
-      - Transport: stdio JSON-RPC to client (Claude Desktop, Cursor, Devin)
-      - Loads plugin-mcp and nothing else
-      - Bypasses host shell execution entirely
+    - Thin in-process adapter routing JSON-RPC stdio directly to VirtIO rings
+    - MCP binary: kelir-mcp boots microVM with plugin-mcp and stdio transport
+    - MCP plugin: maps tools directly to virtio-console, virtio-fs, and traps
+    - Eliminates external bridge daemons, proxy sockets, and shell layers
     - MCP plugin
```

```diff
diff --git a/cans/00-overview.md b/cans/00-overview.md
index 101a1a1..202b2b2 100644
--- a/cans/00-overview.md
+++ b/cans/00-overview.md
@@ -16,3 +16,3 @@
     - In-process microkernel
-      - Backends: libkrun, rustvmm, openvmm, avf, bwrap, proot
+      - Backends: rustvmm, hvf, openvmm, avf, bwrap, proot
       - No dockerd, containerd, background root socket, daemon maintenance
diff --git a/cans/01-architecture.md b/cans/01-architecture.md
index 303c3c3..404d4d4 100644
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@ -58,3 +58,3 @@
     - virtio-net wire plane: L4/L7 traffic, secrets, chaos
-    - virtio-shm shared memory: zero-copy tabular data and host DB sockets
+    - virtio-shm shared memory: zero-copy tabular data buffers
   - Shared memory channel
diff --git a/cans/11-operations.md b/cans/11-operations.md
index 505e5e5..606f6f6 100644
--- a/cans/11-operations.md
+++ b/cans/11-operations.md
@@ -35,3 +35,3 @@
     - Strategy
       - 90% of checks on Linux x86
-      - macOS Apple Silicon jobs limited to compile gate for driver-libkrun
+      - macOS Apple Silicon jobs limited to compile gate for driver-hvf
       - Do not waste runner minutes on full matrix
@@ -156,3 +156,3 @@
       - kelir-plugins: first-class plugins
         - pty, vfs, wire, chaos, ledger, mcp, oci-translator
         - openshell-coordinator, db-proxy, mesh, lock-broker, supply-chain
-      - kelir-drivers: libkrun, rustvmm, openvmm, avf, bwrap, proot
+      - kelir-drivers: rustvmm, hvf, openvmm, avf, bwrap, proot
       - kelir-shim: in-guest musl PID 1 (<50KB)
diff --git a/cans/artifacts/config/ci.yml b/cans/artifacts/config/ci.yml
index 707a7a7..808b8b8 100644
--- a/cans/artifacts/config/ci.yml
+++ b/cans/artifacts/config/ci.yml
@@ -217,5 +217,5 @@ jobs:
     steps:
       - uses: actions/checkout@v4
       - uses: dtolnay/rust-toolchain@stable
       - uses: mozilla-actions/sccache-action@v0.0.5
-      # Only verify compilation of Apple Silicon HVF driver; do not waste runner minutes on full matrix
-      - name: Check HVF libkrun driver
-        run: cargo check -p driver-libkrun
+      # Verify compilation of native Apple Silicon HVF driver
+      - name: Check native HVF driver
+        run: cargo check -p driver-hvf
diff --git a/cans/artifacts/config/justfile b/cans/artifacts/config/justfile
index 909c9c9..a0ad0d0 100644
--- a/cans/artifacts/config/justfile
+++ b/cans/artifacts/config/justfile
@@ -65,3 +65,3 @@ test-isolation plugin:
 
-# Runs anti-TOCTOU, PTRACE_SEIZE, and zombie reaper tests in isolated container/namespaces
+# Runs anti-TOCTOU, cgroup.freeze, and zombie reaper tests in isolated container/namespaces
 test-shim:
diff --git a/cans/artifacts/code/execution-receipt.rs b/cans/artifacts/code/execution-receipt.rs
index b0be0e0..c0cf0f0 100644
--- a/cans/artifacts/code/execution-receipt.rs
+++ b/cans/artifacts/code/execution-receipt.rs
@@ -17,3 +17,3 @@ pub struct ExecutionReceipt {
     pub dirty_bytes: u64,
-    /// Exact retired instruction count queried directly from hardware PMU
+    /// Retired instruction count sampled from host vCPU thread perf counter
     pub retired_instructions: u64,
diff --git a/cans/artifacts/diagrams/ingestion-adapters.txt b/cans/artifacts/diagrams/ingestion-adapters.txt
index d0df0f0..e0ea0a0 100644
--- a/cans/artifacts/diagrams/ingestion-adapters.txt
+++ b/cans/artifacts/diagrams/ingestion-adapters.txt
@@ -8,3 +8,3 @@
 │ 2. MODEL CONTEXT PROTOCOL (MCP):                                            │
-│    - Binary `@kelir/mcp-server` mounts directly into Claude Desktop/Cursor. │
+│    - In-process `plugin-mcp` connects Claude Desktop/Cursor to VirtIO rings.│
 │    - Mediates hardware-isolated execution without exposing host boundaries. │
```

```diff
diff --git a/cans/01-architecture.md b/cans/01-architecture.md
index 101a1a1..202b2b2 100644
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@ -75,3 +75,3 @@
     - kelir-core: stripped microkernel (BusDispatcher, EventBus, DriverRouter)
-    - kelir-drivers: pluggable backends (libkrun, rustvmm, openvmm, avf, bwrap, proot)
+    - kelir-drivers: pluggable backends (rustvmm, hvf, openvmm, avf, bwrap, proot)
   - Guest VM anatomy
diff --git a/cans/11-operations.md b/cans/11-operations.md
index 303c3c3..404d4d4 100644
--- a/cans/11-operations.md
+++ b/cans/11-operations.md
@@ -167,3 +167,3 @@
       - tests/fixtures/kernel: per-arch vmlinux
       - tests/fixtures/rootfs: EROFS build scripts
-      - tests/fixtures/payloads: high-entropy trap test binaries (shared: pty, shim)
+      - tests/fixtures/payloads: high-entropy test binaries (shared: pty, core-trap)
diff --git a/cans/artifacts/config/project-structure.txt b/cans/artifacts/config/project-structure.txt
index 505e5e5..606f6f6 100644
--- a/cans/artifacts/config/project-structure.txt
+++ b/cans/artifacts/config/project-structure.txt
@@ -156,3 +156,3 @@
 │   │   ├── rootfs/                    # Build scripts generating minimal EROFS base images
 │   │   │   └── build-erofs.sh
-│   │   └── payloads/                  # Binaries with Shannon entropy >5.2 (shared: plugin-pty + kelir-shim tests)
+│   │   └── payloads/                  # Binaries with Shannon entropy >5.2 (shared: plugin-pty + core trap tests)
 │   └── e2e/
diff --git a/cans/artifacts/code/messages.rs b/cans/artifacts/code/messages.rs
index 707a7a7..808b8b8 100644
--- a/cans/artifacts/code/messages.rs
+++ b/cans/artifacts/code/messages.rs
@@ -23,3 +23,4 @@ pub struct TrapEvent {
     pub cwd: String,
     pub ancestor_chain: Vec<String>,
+    /// Evaluated host-side on argv[1:] to keep shim under 50KB
     pub entropy: f64,
     pub loaded_libraries: Vec<String>,
diff --git a/cans/artifacts/code/reconciler.rs b/cans/artifacts/code/reconciler.rs
index 909c9c9..a0ad0d0 100644
--- a/cans/artifacts/code/reconciler.rs
+++ b/cans/artifacts/code/reconciler.rs
@@ -10,2 +10,3 @@
 //   - dir whiteout:   xattr trusted.overlay.opaque=y → host remove_dir_all()
+//   - data sync:      ioctl(FICLONE) CoW block clone with atomic tmp write fallback
 //
@@ -56,3 +57,3 @@ pub async fn reconcile_upperdir(
 
-        // 3. Atomic staged write for modified/created files: tmp + rename
+        // 3. CoW reflink block clone with atomic staged write fallback
         if metadata.is_file() {
```

```diff
diff --git a/cans/01-architecture.md b/cans/01-architecture.md
index 101a1a1..202b2b2 100644
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@ -16,3 +16,3 @@
     - In-process microkernel
       - Virtualization runs in host consumer memory via direct C-FFI
-      - Backends: libkrun, rustvmm, openvmm, avf, bwrap, proot
+      - Backends: rustvmm, hvf, openvmm, avf, bwrap, proot
       - No dockerd, containerd, background root socket, daemon maintenance
diff --git a/cans/artifacts/code/plugin-trait.rs b/cans/artifacts/code/plugin-trait.rs
index 303c3c3..404d4d4 100644
--- a/cans/artifacts/code/plugin-trait.rs
+++ b/cans/artifacts/code/plugin-trait.rs
@@ -82,2 +82,3 @@ pub struct WatchdogEvent {
     pub fired_at_ns: u64,
     pub last_heartbeat_ms: u64,
+    pub last_active_pid: Option<u32>,
 }
diff --git a/cans/artifacts/diagrams/truth-hierarchy.txt b/cans/artifacts/diagrams/truth-hierarchy.txt
index 505e5e5..606f6f6 100644
--- a/cans/artifacts/diagrams/truth-hierarchy.txt
+++ b/cans/artifacts/diagrams/truth-hierarchy.txt
@@ -5,3 +5,3 @@
 │   - PMU Instruction Exactness: Host-sampled vCPU retired instructions.      │
-│   - OCC Inode Hashes: Deterministic 64-bit physical content/inode digests.  │
+│   - OCC Inode Hashes: Deterministic 64-bit physical metadata/inode digests. │
 │   - OS Inode Diffs: Physical disk modifications evaluated via CausalDelta.  │
```










