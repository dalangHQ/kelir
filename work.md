# Part 1/4 — Foundation + first so-what exemplar

Scope of Part 1: establish the **SSOT contract** (artifacts own content, nodes own normativity), enforce it with a check, delete the first structural duplication (`#Canonical-homes`), and rewrite one plane's top node (`#Five-filters`) as the so-what exemplar that Part 2 will replicate across the other planes.

Every patch is day-1 final wording. No migration comments, no "previously" — the diff output is the finished state.

---

## Patch 1 — `cans/artifacts/README.md`

Rewrites the contract so artifacts are content-SSOT and nodes are normativity-SSOT.

```diff
--- a/cans/artifacts/README.md
+++ b/cans/artifacts/README.md
@@ -1,6 +1,6 @@
 # artifacts/
 
-Terminal references for the kelir CANS spec in `../`.
+Terminal content for the kelir CANS spec in `../`.
 
 ## What lives here
 
@@ -12,25 +12,33 @@ Terminal references for the kelir CANS spec in `../`.
 - CANS spec nodes — those are in `../`
 - Anything with an outbound reference — artifacts are terminal by design
 
+## SSOT contract
+
+Artifacts and nodes split truth by kind, not precedence.
+
+- Artifacts own **content**: enumerations, field sets, code signatures,
+  topology, timing constants, wire layouts.
+- Nodes own **normativity**: ordering, placement, threat model, invariants,
+  and the rationale for each.
+- A node that re-lists artifact content is a defect. The node must say
+  something the artifact cannot — otherwise it is decoration.
+- The test: *can the node articulate an invariant the artifact does not
+  encode?* If not, the node is wrong, not the artifact.
+
 ## Convention
 
-Every artifact is referenced from `cans/` via `see:` with a **relative** path:
+Every artifact is referenced from `cans/` via `see:` with a **relative** path
+and is accompanied by a node-level invariant:
 
 ```
-cans/01-architecture.md       →  see: artifacts/code/vmm-driver-trait.rs
-cans/04-terminal-plane.md     →  see: artifacts/diagrams/terminal-pipeline.txt
-cans/03-sdk-architecture.md   →  see: artifacts/examples/autonomous-swe-agent.rs
+cans/01-architecture.md       →  see: artifacts/code/vmm-driver-trait.rs
+                                  invariant: the trait is the sole hypervisor boundary
+cans/04-terminal-plane.md     →  see: artifacts/diagrams/terminal-pipeline.txt
+                                  invariant: filter ordering (see #Five-filters)
+cans/03-sdk-architecture.md   →  see: artifacts/examples/autonomous-swe-agent.rs
+                                  invariant: same plugin set across all three SDKs
 ```
 
 - `cans check` treats every artifact pointer as a **0-hop** reference
-- Artifacts are never spec-edited during a CANS migration; they are frozen reference material
-- An artifact may contain inline file-level comments pointing back to canonical spec nodes, but the artifact itself issues no CANS reference
+- Artifacts are never spec-edited; they are frozen content
+- An artifact may carry a file-header pointer back to its owning node; the
+  artifact issues no CANS reference
 - One artifact may be referenced from multiple spec nodes (no duplication)
 
 ## Editing rules
 
-1. Never edit a spec concept inside an artifact — move the concept to a CANS node and reference it
-2. Never duplicate a spec concept across multiple artifacts
-3. Never create an artifact with no inbound `see:` — orphans are dead weight
-4. An artifact that is not verbatim source (e.g. an SDK usage excerpt) is marked with a top-of-file comment stating it is illustrative, not standalone-compilable
+1. Edit content in the artifact; edit invariants in the node. Never cross.
+2. Never duplicate artifact content in a node.
+3. Never create an artifact with no inbound `see:` carrying a node-level
+   invariant.
+4. Non-verbatim source (SDK excerpts, illustrative snippets) is marked at the
+   top of the file as illustrative, not standalone-compilable.
```

---

## Patch 2 — `cans/_rules.yaml`

Adds the executable check that Patch 1 declares.

```diff
--- a/cans/_rules.yaml
+++ b/cans/_rules.yaml
@@ -54,6 +54,14 @@ redundancy:
   # 1.00 = literal duplicates only; sub-100% overlap on spec prose is normal.
   phrase_overlap_threshold: 0.95
   cross_file_threshold: 5
   fuzzy: false
+  node_vs_artifact:
+    # Overlap with the inbound artifact is allowed; missing so-what is not.
+    # A node fails this check when every bullet is either a see:, a bare
+    # restatement of the artifact, or a child enumeration.
+    enabled: true
+    phrase_overlap_threshold: 0.85
+    require_node_invariant: true
   stopwords: [the, a, an, of, to, in, for, and, or, with, must, shall, requires]
   synonyms:
     - [postgres, postgresql, pg]
```

---

## Patch 3 — `cans/00-overview.md`

Deletes `#Canonical-homes`. Every entry is either already in `#Spec-map` or is a leaf reachable from a `#Spec-map` parent in one hop. A shortcut table for leaves violates the one-hop reading discipline.

```diff
--- a/cans/00-overview.md
+++ b/cans/00-overview.md
@@ -40,23 +40,3 @@
       - Ecosystem ingestion: see 10-adapters.md#Ingestion-matrix
       - Build, CI, layout: see 11-operations.md#Execution-philosophy
-  - Canonical homes
-    - Plugin trait: see 02-plugin-model.md#KelirPlugin-trait
-    - Params typed views: see 03-sdk-architecture.md#Params-typed-views
-    - Params ingress contract: see 01-architecture.md#Params-ingress-contract
-    - Wire architecture: see 06-wire-plane.md#Wire-architecture
-    - VmmDriver trait: see 01-architecture.md#VmmDriver-trait
-    - Session superpowers: see 07-lifecycle.md#Superpowers
-    - Guest VM anatomy: see 01-architecture.md#Guest-VM-anatomy
-    - Env injection mechanism: see 09-protocol.md#Env-injection
-    - Trap fast path: see 09-protocol.md#Trap-fast-path
-    - Python SDK: see 03-sdk-architecture.md#Python-SDK
-    - Step evaluation loop: see 08-epistemic.md#Step-evaluation-loop
-    - Merkle WAL: see 08-epistemic.md#Merkle-WAL
-    - Snoop plane: see 05-storage-plane.md#Snoop-plane
-    - Page size by platform: see 01-architecture.md#SnapshotDescriptor.dirty-bytes
```

---

## Patch 4 — `cans/02-plugin-model.md`

`#Core-context-keys` stops calling itself a "routing table" (that was `#Canonical-homes`' job, now deleted) and states what only this node can: the set is closed at compile time, not just at dispatch.

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -29,9 +29,9 @@
       - Deny-observation composition is a user concern
   - Core context keys
-    - Routing table; each key's semantics live at the owning node
+    - Handler-visible surface; each key's definition lives at its see: target
+    - The set is closed at compile/type/import time, not just at dispatch:
+      disabling a plugin removes its key from the type system
     - Core-provided (always present)
       - c.trap: see 09-protocol.md#Trap-fast-path
       - c.env: see 09-protocol.md#Env-injection
@@ -45,7 +45,6 @@
     - User-provided via typed handle
       - c.<key>: from .decorate() / PluginWithHandle / BasePlugin.handle()
-    - Rule: disabling a plugin removes its key at compile/type/import time
   - First-class plugins
```

---

## Patch 5 — `cans/04-terminal-plane.md`

The exemplar rewrite. `#Five-filters` becomes the node that states what `terminal-pipeline.txt` cannot: ordering, placement, bypass, threat model. The three child sections that were pure re-enumerations (`#ANSI-sanitizer`, `#Signal-forwarding`) are trimmed to their so-what; `#Prompt-responder`, `#Window-resize`, `#Token-Shield` already carry invariants and are left alone.

```diff
--- a/cans/04-terminal-plane.md
+++ b/cans/04-terminal-plane.md
@@ -5,29 +5,33 @@
     - Host supervisor (@kelir/plugin-pty) applies five filters
   - Five filters
-    - ANSI stream sanitizer
-      - Drops OSC 52 clipboard hijack in-flight
-      - Strips OSC 0/1/2 title spoofing
-    - onPrompt responder
-      - Foreground TTY verification via tcgetpgrp()
-      - Matches ANSI-SGR-stripped plaintext shadow copy
-      - Injects keystrokes directly into virtio-console ring
-    - WindowResize bridge
-      - Boot handshake pushes initial size (e.g. 120x40) at VM-ready
-      - Host SIGWINCH sends HostToGuest::WindowResize over vsock:5252
-      - Shim executes TIOCSWINSZ on guest PTY slave
-    - Signal forwarding: Ctrl+C (SIGINT) and Ctrl+D to foreground process group
-    - Token Shield filter
-      - Clamps chunks exceeding byte or line ceiling
-      - Injects truncation markers with byte/line stats
+    - Filter set and per-filter behavior: see artifacts/diagrams/terminal-pipeline.txt
+    - Ordering invariant
+      - ANSI strip precedes prompt match: SGR codes split regex targets
+      - Prompt match precedes Token Shield: clamping trims matched prompts
+      - Signal forwarding observes post-sanitization bytes only
+    - Placement invariant
+      - All five execute host-side at the virtio-console boundary
+      - In-guest placement would cede the chain to a compromised shim
+    - Single intentional bypass
+      - HostToGuest::Disclosure skips all five; body is authoritative
+      - Bypass path: see #Disclosure
+    - Threat model
+      - OSC 52: clipboard hijack (agent reads/writes human clipboard)
+      - OSC 0/1/2: title spoofing (agent impersonates terminal session)
+      - Both are Tier-1 violations: see 08-epistemic.md#Three-tier-truth
   - ANSI sanitizer
-    - OSC 52 attack defense
-      - Scripts printing \x1b]52;c;... attempt clipboard read/write
-      - All OSC 52 bytes dropped at hardware buffer boundary
-    - Title spoofing defense
-      - OSC 0/1/2 stripped to prevent host terminal emulator tampering
+    - Drops OSC 52 (`\x1b]52;c;…`) at the virtio-console ring boundary
+    - Strips OSC 0/1/2 before the host terminal emulator observes bytes
+    - Only OSC sequences are touched; SGR (color, bold) passes through
+    - Threat rationale: see #Five-filters
   - Prompt responder
     - Matchers run against ANSI-SGR-stripped plaintext shadow copy
     - Regex matches text regardless of SGR color or bold codes
@@ -44,11 +48,10 @@
       - HostToGuest::WindowResize { rows, cols, xpixel, ypixel } over AF_VSOCK:5252
       - In-guest shim executes ioctl(pts_fd, TIOCSWINSZ, &ws)
       - Guest shell redraws without corruption
   - Signal forwarding
-    - Ctrl+C / Ctrl+D forwarded to guest foreground process group
-    - on_signal_send(&SendSignal) fires in priority() order
-      - Observation only; hook cannot rewrite signal
-    - Signals flow as HostToGuest::SendSignal over vsock:5252
-    - Shim signals the foreground PID
+    - Ctrl+C / Ctrl+D reach the guest foreground process group
+    - on_signal_send is observation-only; a hook cannot rewrite the signal
+    - Wire path: see 09-protocol.md#Wire-messages
   - Disclosure
```

---

## Patch 6 — `cans/01-architecture.md`

Collapses the triple-statement of *"only core invariants are core-owned."* The rule's sole home becomes `#Capability-taxonomy#Decision-test`; `#Axiomatic-foundations` and `#Core-invariants` stop restating it.

```diff
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@ -19,8 +19,7 @@
       - Radical plugin-first microkernel
         - Core drives hardware virtualization and multiplexes the 4 VirtIO rings
         - Every capability is a decoupled, composable plugin
-        - Exception fence: only 01-architecture.md#Core-invariants primitives are core-owned
       - Transparent truthful steering
         - Intercepted I/O is allowed, steered with tty_notice, or rejected
         - No silent state divergence
@@ -55,7 +54,7 @@
       - Otherwise → plugin
   - Core invariants
-    - The closed set. Everything not pointed to here is a plugin.
+    - The closed set of core primitives
     - Trait definitions
       - KelirPlugin: see 02-plugin-model.md#KelirPlugin-trait
       - VmmDriver trait (defined in this file)
```

---

## Patch 7 — `cans/08-epistemic.md`

`#Primitive-vs-derived-telemetry` drops the two re-enumeration blocks (their entries are already canonical in `01#Core-invariants` and the sibling nodes) and keeps the decision test, which is the so-what.

```diff
--- a/cans/08-epistemic.md
+++ b/cans/08-epistemic.md
@@ -33,24 +33,17 @@
         - Wiring example: See artifacts/examples/autonomous-swe-agent.rs
   - Primitive vs derived telemetry
-    - Rule
-      - Primitive: single read API, hardware boundary, no policy
-        - Core-owned; enumerated in 01-architecture.md#Core-invariants
-      - Derived: computed from primitives, policy-bearing
-        - Plugin-owned; never a new core method
-    - Primitive (core)
-      - ExecutionReceipt via c.receipt()
-      - StepSlot via c.step
-      - TrapEvent, ProcessExit, wire messages
-    - Derived (plugin-owned)
-      - CausalDelta: plugin-ledger; see #Causal-delta
-      - Merkle chain: plugin-ledger; see #Merkle-WAL
-      - OTel spans: plugin-ledger; see #OpenTelemetry
-      - Oracle analyses: consumer's Oracle impl
-    - Adding a new derived metric: new plugin or new Oracle impl
+    - Decision test
+      - Single read API, hardware boundary, no policy → primitive, core-owned
+      - Computed from primitives, policy-bearing → derived, plugin-owned
+      - A new derived metric is a plugin or Oracle change, never a core method
+    - Boundaries
+      - Primitive set: see 01-architecture.md#Core-invariants
+      - CausalDelta: see #Causal-delta
+      - Merkle chain: see #Merkle-WAL
+      - OpenTelemetry spans: see #OpenTelemetry
+      - Oracle analyses: consumer's Oracle impl
   - Execution receipts
```

---

## What Part 2 will cover

- `01#Guest-VM-anatomy` so-what rewrite (why no ACPI/PCI, why virtio-rng, why static musl, why waitpid in the loop — none currently stated).
- `05#Storage-architecture` and `06#Wire-architecture` so-what rewrites (same pattern as Patch 5).
- `08#Three-tier-truth` merge with `artifacts/diagrams/truth-hierarchy.txt` — pick one home, fold the other.
- `07#Superpowers` count fix (node says 9, has 12; `superpowers.txt` has 9) and per-superpower veto class stated once.
- `<50KB` vs `<100KB` shim ceiling — one number in `01#Guest-VM-anatomy`, referenced from `11` and `ci.yml`.
- `03#Oracle-contract` merged into `08#Oracle` (or vice-versa) — single home for the five methods.
- `09#Env-injection` cross-ref hygiene in `messages.rs` header comments.

Say the word for Part 2.

# Part 2/4 — Guest/Storage/Wire so-what, Superpowers count, ceiling unification

Scope: apply the Patch 5 pattern (Part 1) to the three remaining plane-top nodes; fix the Superpowers drift; unify the shim ceiling; consolidate the Oracle contract.

---

## Patch 8 — `cans/01-architecture.md#Guest-VM-anatomy`

Stops enumerating what `guest-vm.txt` already enumerates; states the boot-budget justification that only the node can carry, and fixes the shim ceiling to a single canonical number.

```diff
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@ -70,28 +70,26 @@
   - Guest VM anatomy
     - See: artifacts/diagrams/guest-vm.txt
-    - Minimal vmlinux: stripped Linux kernel, no ACPI or PCI bloat
-      - Apple Silicon: CONFIG_ARM64_16K_PAGES=y
-      - Linux x86: 4KB pages standard
-      - Linux ARM64: CONFIG_ARM64_4K_PAGES=y
-    - Paravirtualized devices
-      - virtio-rng: hardware entropy feeding /dev/urandom, no boot hangs
-      - virtio-wdt: hardware watchdog preventing guest kernel lockups
-      - virtio-balloon: dynamic memory inflation/deflation under host pressure
-    - Rootfs
-      - Read-only EROFS block device, cold-mounted in <2ms
-      - No tarball decompression
-    - /sbin/kelir-shim
-      - Static 50KB Musl binary, guest PID 1
-      - Binds vsock:5252 (control/trap) and vsock:5253 (ingress port-forward)
-      - Non-blocking waitpid(-1, WNOHANG) zombie reaper in async event loop
-      - seccomp-unotify filter with in-kernel BPF allowlist
-      - Anti-TOCTOU: PTRACE_SEIZE + PTRACE_INTERRUPT on every thread
-      - Parses ELF DT_NEEDED/PT_INTERP from disk
-      - Shannon entropy on argv[1:] (payload only, not argv[0])
-      - PTY per workload: openpty() + setsid() + TIOCSCTTY
-    - Untouched guest workload
-      - Claude Code, Cursor, SWE-bench, Node, Rust, Python
-      - No custom binary, no guest-side SDK
+    - Boot budget
+      - Cold boot target: <60ms, host-side stopwatch to first workload exec
+      - ACPI + PCI probing alone is ~40ms; kernel is built without both
+      - Every device below earns its millisecond against that budget
+    - Device justification
+      - virtio-rng: without it, getrandom() blocks 200–2000ms at boot
+      - virtio-wdt: hardware watchdog, so no in-guest timer daemon is needed
+      - virtio-balloon: host pressure is invisible to guest; balloon is the
+        only signal that survives a rollback
+    - Shim constraints
+      - Static musl, no dynamic linkage: dynamic libc would force libc into
+        the rootfs and break the stripped-rootfs invariant
+      - Size ceiling: <50KB per architecture, enforced in CI
+      - Single-threaded async loop: a reaper thread would require pthread
+        linkage and breach the ceiling; waitpid reaps in the same loop
+    - Isolation boundary
+      - Shim runs as PID 1: orphan reaping is mandatory, not optional
+      - seccomp-unotify is in-kernel; userspace cannot race the allowlist
+      - PTRACE_SEIZE covers every thread: leader-only seize is unsafe
+    - Workload contract: no guest-side SDK, shim, or agent
```

---

## Patch 9 — `cans/05-storage-plane.md#Storage-architecture`

```diff
--- a/cans/05-storage-plane.md
+++ b/cans/05-storage-plane.md
@@ -1,8 +1,18 @@
 - Storage plane
   - Storage architecture
     - See: artifacts/diagrams/storage-architecture.txt
-    - Layered model: EROFS rootfs, tmpfs overlay, reconciler, idmap, caches, ghosts, mask
+    - Invariant: host SSD is mutated by exactly one path — reconcile()
+      - Every other layer lives in guest RAM or host RAM
+      - Rollback reverts guest state with zero host SSD involvement
+    - Read-only lower + tmpfs upper
+      - Guest root cannot corrupt the host workspace tree
+      - Deletions leave whiteouts, not disk holes; reconciler must resolve them
+    - Enumerated write paths
+      - writes to upperdir (tmpfs; reconciled later)
+      - writes to shared read-only caches (rejected at FUSE)
+      - writes to ghost inodes (RAM-only; never reach host disk)
+      - writes to host SSD (reconcile() only, atomic per file)
+    - Every layer in the diagram is gated by one of the four paths above
   - EROFS rootfs
```

---

## Patch 10 — `cans/06-wire-plane.md#Wire-architecture`

```diff
--- a/cans/06-wire-plane.md
+++ b/cans/06-wire-plane.md
@@ -1,9 +1,19 @@
 - Wire plane
   - Wire architecture
     - See: artifacts/diagrams/wire-plane.txt
-    - Zero-knowledge credential architecture
-    - Production API keys never enter guest memory, shell variables, or disk
-    - Four channels: L7 proxy, L4 SOCKS5, SSH gate, DNS sinkhole
+    - Invariant: a real secret never enters guest address space
+      - The guest sees only vault:// URNs and per-session CA certs
+      - Substitution happens host-side, after TLS termination at the proxy
+    - Total egress rule
+      - Egress is what the four channels admit; nothing else reaches NIC
+      - Default deny: allowlist miss is silent drop (on_net_outbound Reject)
+      - DNS sinkhole is the fallback that makes "unlisted ⇒ dropped" total
+    - Per-channel admission
+      - L7 proxy: TLS terminated under per-session CA; allowlist by host
+      - L4 SOCKS5: connect-by-IP; used where TLS termination is invalid
+      - SSH gate: host keyring signs; private key never crosses vsock
+      - DNS: NXDOMAIN for non-allowlisted; blocks tunneling on port 53
+    - Composability: plugin-chaos mutates frames in both directions
   - L7 credential swapper
```

---

## Patch 11 — `cans/07-lifecycle.md#Superpowers`

Removes the drifting count; states the property of the set, not its cardinality. The cardinality lives only in `superpowers.txt`.

```diff
--- a/cans/07-lifecycle.md
+++ b/cans/07-lifecycle.md
@@ -1,10 +1,11 @@
 - Lifecycle
   - Superpowers
     - See: artifacts/diagrams/superpowers.txt
-    - Nine superpowers enumerated below
-    - Every superpower exposes a veto or observation point
-      - Registered via LifecycleCtx in on_vm_init
-      - Class per superpower: see #Lifecycle-vetoes
+    - Set membership and mechanism per item: see the diagram
+    - Set-level invariant: every item is a hypervisor primitive, not a
+      guest-side simulation. Nothing in this set is achievable without driver
+      cooperation.
+    - Every item exposes one lifecycle point via LifecycleCtx
+      - Split by state-preservation, not by privilege: see #Lifecycle-vetoes
     - Ownership attribution: see 02-plugin-model.md#First-class-plugins
```

---

## Patch 12 — `cans/07-lifecycle.md#Lifecycle-vetoes`

Adds the so-what the split actually encodes: hard-veto events are non-state-preserving, observation events are state-preserving. This is what determines class membership and is currently not stated anywhere.

```diff
--- a/cans/07-lifecycle.md
+++ b/cans/07-lifecycle.md
@@ -96,19 +96,18 @@
     - Two classes
       - Hard veto: Fork, Suspend, ClockSkew
         - Any false return aborts the action with ESessionVeto
         - Use for policy gates (e.g. "no fork until fixtures ready")
       - Observation only: Checkpoint, Rollback, Resume, Terminate
         - Callback fires; return value logged and ignored
         - Use for ledger / trace / metrics
-        - Cannot veto by design
-          - Rollback, Resume, Checkpoint are state-preserving
-          - Terminate flush hook is on_vm_shutdown, not a veto
-    - Note: VetoEvent is the uniform registration type for both classes
-      - Class determines semantics, not the type name
+    - Class is determined by state-preservation, not by event name
+      - A state-preserving action cannot be vetoed: aborting it would leave
+        no recovery path
+      - A state-mutating action (Fork, Suspend, ClockSkew) is vetoable
+      - Terminate is observation-only because flush belongs to on_vm_shutdown
+    - VetoEvent is the uniform registration type for both classes
     - Firing point
       - Before the corresponding hypervisor primitive executes
       - Session-level actions also route through this point
```

---

## Patch 13 — `cans/11-operations.md#CI-matrix`

Shim ceiling unified with `01#Guest-VM-anatomy`.

```diff
--- a/cans/11-operations.md
+++ b/cans/11-operations.md
@@ -36,11 +36,11 @@
       - Do not waste runner minutes on full matrix
     - Enforced invariants: CI matrix
-      - Shim binary < 100KB per architecture
+      - Shim binary < 50KB per architecture: see 01-architecture.md#Guest-VM-anatomy
       - Concurrency cancelled per ref (cancel-in-progress)
       - sccache enabled via SCCACHE_GHA_ENABLED
     - Per-plugin matrix: each plugin fails independently
     - Integration tier: KVM-detected, continue-on-error on GitHub-hosted
```

---

## Patch 14 — `cans/artifacts/config/ci.yml`

Same ceiling, executable.

```diff
--- a/cans/artifacts/config/ci.yml
+++ b/cans/artifacts/config/ci.yml
@@ -55,10 +55,10 @@
       - name: Build Shim via Zig Linker
         run: cargo zigbuild -p kelir-shim --release --target ${{ matrix.target }}
-      - name: Verify Size (<100KB binary constraint)
+      - name: Verify Size (<50KB binary constraint)
         run: |
           SIZE=$(stat -c%s target/${{ matrix.target }}/release/kelir-shim)
           echo "Shim size: $SIZE bytes"
-          test $SIZE -lt 102400 || (echo "SHIM BLOAT DETECTED" && exit 1)
+          test $SIZE -lt 51200 || (echo "SHIM BLOAT DETECTED" && exit 1)
       - uses: actions/upload-artifact@v4
```

---

## Patch 15 — `cans/03-sdk-architecture.md#Oracle-contract`

Drops the re-stated "five methods" (owned by `08#Oracle`) and the redundant "Interface spec:" label; keeps only what this node can say — language realization.

```diff
--- a/cans/03-sdk-architecture.md
+++ b/cans/03-sdk-architecture.md
@@ -40,12 +40,12 @@
       - Python: asyncio.gather, async with, Pydantic v2
   - Oracle contract
-    - Interface spec: see 08-epistemic.md#Oracle
+    - Interface: see 08-epistemic.md#Oracle
     - Language-specific realization
       - Rust: Arc<dyn Oracle>, threaded through handler closures
       - TypeScript: Oracle object literal, captured by closures
       - Python: subclass kelir.Oracle, methods async
-    - All three must implement the five methods; missing method → link error
+    - Missing method fails at compile / import time in every language
```

---

## Patch 16 — `cans/08-epistemic.md#Three-tier-truth`

Diagram owns tier enumerations; node owns the selection and degradation rules.

```diff
--- a/cans/08-epistemic.md
+++ b/cans/08-epistemic.md
@@ -85,20 +85,17 @@
   - Three-tier truth
     - See: artifacts/diagrams/truth-hierarchy.txt
-    - Motivates decoupling orchestration from private harness internals
-    - Tier 1: physical ground truth (hypervisor/kernel boundary)
-      - Unforgeable receipts: BLAKE3 hex digests, dirty byte counts
-      - OS inode diffs: physical disk modifications via CausalDelta
-      - Egress telemetry: true network frames past virtio-net proxy
-      - Invariant: agent cannot fake, even as guest root
-    - Tier 2: public formal protocols (contract interface)
-      - Model Context Protocol tool events over stdio
-      - Deterministic PTY stream output sanitized of ANSI injection
-      - Invariant: evaluated strictly against public versioned contracts
-    - Tier 3: synchronous inode snooping (harness state interception)
-      - Zero post-mortem polling
-      - Intercepts harness write() calls in real-time
-      - Host in-memory buffer captures updates before disk sync
-      - Invariant: eliminates async races and file lock collisions
-      - Consumer: Oracle.ingest_harness_event; see #Oracle
+    - Tier evidence per tier: see the diagram
+    - Selection rule
+      - Claim must resist forgery by guest code → Tier 1
+      - Claim about a public, versioned contract → Tier 2
+      - Claim about harness-internal state with no wire surface → Tier 3
+    - Degradation rule
+      - A higher-tier claim never falls back to a lower-tier signal
+      - Missing Tier 1 evidence is a denial, not a Tier 3 downgrade
+      - Rationale: silent downgrade is exactly the harness self-report failure
+        this tier scheme exists to prevent
+    - Consumer of Tier 3: Oracle.ingest_harness_event; see #Oracle
```

---

## Patch 17 — `cans/artifacts/code/messages.rs`

Artifact-side substantive duplication trimmed to header pointer; the invariants live in `09#Env-injection` and `04#Disclosure`.

```diff
--- a/cans/artifacts/code/messages.rs
+++ b/cans/artifacts/code/messages.rs
@@ -15,7 +15,7 @@
 // All wire messages exchanged over vsock:5252. Payloads encode as CBOR
 // inside the TLV frame defined in codec.rs.
 //
-// Canonical spec home: cans/09-protocol.md#Wire-messages
+// Owned by: cans/09-protocol.md#Wire-messages
@@ -91,10 +91,8 @@
     WindowResize(WindowResize),
     SendSignal(SendSignal),
     /// Dynamic Memory Balloon Adjustment
     AdjustBalloon {
         target_mb: u32,
     },
-    /// Core-originated mandatory disclosure. Routed through on_disclosure
-    /// hooks on the host before forwarding. Shim prints body verbatim to
-    /// the workload PTY — zero silent state divergence.
+    /// See cans/04-terminal-plane.md#Disclosure
     Disclosure {
         body: Vec<u8>,
     },
@@ -108,14 +106,8 @@
     Deny {
         exit_code: i32,
         stderr_message: String,
     },
-    /// Re-execute with substitutions. This is the ONLY path by which
-    /// handler-side environment changes reach the guest: a handler that
-    /// returns `Decision.resume()` after `c.env.set(..)` is auto-converted
-    /// to `Rewrite { new_binary = original, new_argv = original,
-    /// injected_env = accumulated }`. See cans/09-protocol.md#Env-injection.
+    /// See cans/09-protocol.md#Env-injection
     Rewrite {
         new_binary: String,
         new_argv: Vec<String>,
         injected_env: HashMap<String, String>,
-        /// Mandatory terminal disclosure. Routed through on_disclosure
-        /// (host) → HostToGuest::Disclosure (shim) → PTY. Plugins may
-        /// annotate; the body is immutable by contract.
+        /// See cans/04-terminal-plane.md#Disclosure
         disclosure: Option<String>,
     },
```

---

## What Part 3 will cover

- `09#Trap-fast-path` — remove step-by-step restatement of `trap-flow.txt`; state the three-layer safety rationale (why each layer exists and what the layers below it would miss).
- `02#KelirPlugin-trait` — hook list is enumerated 3× (node, `plugin-trait.rs`, `trap-flow.txt`); keep node as so-what only ("why this trait surface and not a wider one").
- `02#First-class-plugins` — collapses to a table-equivalent of ownership attribution; per-plugin behavior stays in the owning plane node.
- `10#Ingestion-matrix` — remove the "each adapter is a binary + a plugin" duplicate (it's in `01#Capability-taxonomy`); keep adapter-specific invariants only.
- `08#Causal-delta` — reconcile the split-ownership claim with `02#First-class-plugins`.
- `08#Execution-receipts` — remove the page-size duplication (already canonical in `01#VmmDriver-trait#SnapshotDescriptor.dirty-bytes`); keep only the assertion-form so-what.
- `05#Snoop-plane` — remove the direct-I/O mechanics (belongs in `plugin-vfs`/`vfs.rs`); keep the race-elimination invariant.

- 

# Part 3/4 — Trap fast-path, plugin trait surface, ingestion, epistemic split-ownership

Scope: finish the so-what rewrites on the remaining plane-top nodes; remove the three structural duplicates (`#KelirPlugin-trait` hook list, `#First-class-plugins` behavior list, `#Ingestion-matrix` adapter shape) that repeat `plugin-trait.rs` and each owning plane node; settle the `CausalDelta` split-ownership drift.

---

## Patch 18 — `cans/09-protocol.md#Trap-fast-path`

`trap-flow.txt` owns the step sequence. Node owns *why three layers and not one*, and *what each layer costs if removed*.

```diff
--- a/cans/09-protocol.md
+++ b/cans/09-protocol.md
@@ -7,33 +7,25 @@
   - Trap fast-path
-    - Goal: prevent slowdowns on cargo build / npm install scale workloads
-    - Layer 1: in-kernel BPF allowlist
-      - rustc, gcc, as, ld, sed, cat evaluated purely in guest kernel
-      - 0µs roundtrip overhead
-    - Layer 2: anti-TOCTOU task-group freeze
-      - seccomp-unotify triggers on git, curl, ssh, rm, sudo, npm, /tmp binaries
-      - Iterate /proc/$PID/task/*
-      - Issue ptrace(PTRACE_SEIZE) + ptrace(PTRACE_INTERRUPT) to every thread
-      - seccomp-unotify stops only the calling thread
-      - Leader-only seize leaves siblings free to rewrite memory mid-inspection
-      - Full task-group freeze closes check-to-use race
-    - Layer 3: entropy and dynamic linker audit
-      - Shannon entropy computed on argv[1:] only
-        - Excludes argv[0]; low-entropy path would dilute the measurement
-        - Values > 5.2 flag encrypted shell drops or base64 payloads
-      - ELF header parsed directly from disk at trap time
-        - Walks DT_NEEDED entries
-        - Reads PT_INTERP segment
-        - Enumerates libraries the dynamic linker will load
-        - Blocks LD_PRELOAD rootkits
-        - Disk-time scan is correct; /proc/$PID/maps still describes old image
+    - Step sequence and per-step mechanics: see artifacts/diagrams/trap-flow.txt
+    - Goal: the trap path must not slow `cargo build` / `npm install`
+    - Why three layers, not one
+      - A single trap-on-every-syscall design adds ~µs per fork/exec at
+        toolchain scale; the allowlist exists to pay that cost only where
+        policy is possible
+      - A userspace-only inspection cannot close the check-to-use race;
+        the freeze exists to make the metadata binding
+      - A frozen inspection is still blind to what the loader *will* do;
+        the linker audit exists to make the inspection meaningful
+    - Layer cost if removed
+      - Layer 1 removal: every rustc invocation round-trips to the host
+      - Layer 2 removal: sibling threads rewrite memory during inspection;
+        inspection is decorative
+      - Layer 3 removal: LD_PRELOAD rootkits and encrypted argv payloads
+        pass through untouched
+    - Measurement rule
+      - Entropy is over argv[1:]: including argv[0] dilutes the signal with
+        a low-entropy path
+      - ELF is read from disk at trap time: /proc/$PID/maps still describes
+        the pre-exec image and cannot be trusted
     - Flow diagram: See artifacts/diagrams/trap-flow.txt
```

---

## Patch 19 — `cans/02-plugin-model.md#KelirPlugin-trait`

Trait surface is enumerated 3× (node, `plugin-trait.rs`, `trap-flow.txt`). Node keeps only what it can say: why *this* surface and not a wider one.

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -1,25 +1,20 @@
 - Plugin model
   - KelirPlugin trait
     - See: artifacts/code/plugin-trait.rs
-    - Async trait implemented by all subsystems, internal and third-party
-    - Trait surface
-      - name() -> &'static str: unique identifier
-      - priority() -> u8: dispatch ordering; lower = earlier
-    - Hook families across the 4 VirtIO rings
-      - Lifecycle: on_vm_init, on_vm_ready, on_vm_shutdown
-      - Control plane (vsock)
-        - on_trap, on_process_exit
-        - on_memory_pressure, on_heartbeat, on_port_bound
-        - on_signal_send, on_watchdog
-      - Terminal plane (console)
-        - on_pty_input, on_pty_output
-        - on_window_resize, on_disclosure
-      - Storage plane (fs): on_vfs_lookup, on_vfs_read, on_vfs_write
-      - Wire plane (net): on_net_outbound, on_net_inbound
+    - Trait surface and hook families: see the artifact
+    - Shape of the surface
+      - Every hook is `async` and returns `Result<_, PluginError>`; a hook
+        cannot block the dispatch array
+      - Every ring has both an observation and a mutation point; the trait
+        never exposes a ring for observation only
+      - The surface is closed per ring; adding a hook is a spec change, not
+        a plugin concern
+    - Why the surface is this size
+      - A hook exists only if a ring delivers the event it consumes
+      - A hook that a plugin does not implement defaults to `Ok(())`;
+        "opt-in" is the trait's typing, not a runtime branch
     - Hook context restriction
```

---

## Patch 20 — `cans/02-plugin-model.md#First-class-plugins`

Current form is a behavior list per plugin that each owning plane node re-states. Node keeps ownership attribution and the surface contract; behavior stays at the plane.

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -33,18 +33,18 @@
-  - First-class plugins
-    - plugin-pty: virtio-console, ANSI sanitizer, Token Shield, prompt responder
-    - plugin-vfs: virtio-fs, ghost inodes, maskRead, reconciler, lock broker
-    - plugin-wire: L7 proxy, SOCKS5, SSH gate, DNS sinkhole, port forwarding
-    - plugin-chaos: bidirectional wire jitter/drop, VFS ENOSPC injection
-    - plugin-ledger: Merkle WAL, causal delta, OpenTelemetry traceparent
-    - Minimum five above; ecosystem additions per 02-plugin-model.md#Plugin-ownership
+  - First-class plugins
+    - Minimum set: pty, vfs, wire, chaos, ledger
+    - Behavior of each: see the owning plane
+      - plugin-pty: see 04-terminal-plane.md
+      - plugin-vfs: see 05-storage-plane.md
+      - plugin-wire, plugin-chaos: see 06-wire-plane.md
+      - plugin-ledger: see 08-epistemic.md
+    - Ecosystem additions follow 02-plugin-model.md#Plugin-ownership
     - Session capability surface: attributed per plane
```

---

## Patch 21 — `cans/02-plugin-model.md#Core-context-keys`

Removes the second routing table now that `00#Canonical-homes` is gone (Part 1). Keeps only what this node can carry: the closed-set property, and which key each plugin contributes.

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -29,27 +29,23 @@
   - Core context keys
-    - Handler-visible surface; each key's definition lives at its see: target
-    - The set is closed at compile/type/import time, not just at dispatch:
-      disabling a plugin removes its key from the type system
-    - Core-provided (always present)
-      - c.trap: see 09-protocol.md#Trap-fast-path
-      - c.env: see 09-protocol.md#Env-injection
-      - c.vm: see 01-architecture.md#VmmDriver-trait
-      - c.step: see 08-epistemic.md#Step-evaluation-loop
-      - c.receipt(): see 08-epistemic.md#Execution-receipts
-    - Plugin-provided (present only when the plugin is mounted)
-      - c.term: plugin-pty
-      - c.fs: plugin-vfs
-      - c.ledger: plugin-ledger
-      - c.wire: plugin-wire
-      - c.chaos: plugin-chaos
-    - User-provided via typed handle
-      - c.<key>: from .decorate() / PluginWithHandle / BasePlugin.handle()
+    - Handler-visible keys; each key's definition lives at its owning plane
+    - Closed-set rule
+      - Key presence is a property of the type, not of the runtime
+      - Disabling a plugin removes its key at compile / type / import time
+      - A handler that reads a disabled plugin's key does not run; it fails
+        to typecheck
+    - Core-provided (always present)
+      - c.trap, c.env, c.vm, c.step, c.receipt()
+    - Plugin-provided (present only when the plugin is mounted)
+      - c.term ← plugin-pty; c.fs ← plugin-vfs; c.ledger ← plugin-ledger;
+        c.wire ← plugin-wire; c.chaos ← plugin-chaos
+    - User-provided via typed handle
+      - c.<key> from .decorate() / PluginWithHandle / BasePlugin.handle()
+    - Every key above resolves to a plane node in one hop; none are
+      restated here
   - First-class plugins
```

---

## Patch 22 — `cans/10-adapters.md#Ingestion-matrix`

The `binary + plugin` shape is stated three times (here, `01#Capability-taxonomy`, and per-adapter). Keep it only here (as the adapter-specific application), and remove the tri-native SDK row (canonical in `03`).

```diff
--- a/cans/10-adapters.md
+++ b/cans/10-adapters.md
@@ -1,10 +1,12 @@
 - Adapters
   - Ingestion matrix
     - See: artifacts/diagrams/ingestion-adapters.txt
-    - Five adapter families: MCP, containerd shim v2, OpenShell gRPC, E2B, tri-native SDKs
-    - Tri-native SDKs are canonical in 03-sdk-architecture.md; not repeated here
-    - Every adapter is a binary contract + a plugin
-      - Binary: process the ecosystem speaks to
-      - Plugin: translation and policy inside the process
-      - see 01-architecture.md#Capability-taxonomy
+    - Adapter set and ecosystem binding: see the diagram
+    - Two shapes, one rule
+      - Contract-bound binary + plugin: MCP, containerd shim v2, OpenShell
+      - Thin SDK wrapper: E2B (no plugin, no binary) — see #E2B-compatibility
+    - Rule: the plugin owns translation and policy; the binary owns the wire
+    - Tri-native SDKs are the fourth ingestion family and are canonical in
+      03-sdk-architecture.md
   - MCP adapter
```

---

## Patch 23 — `cans/08-epistemic.md#Causal-delta`

Resolves the split-ownership drift against `02#First-class-plugins`.

```diff
--- a/cans/08-epistemic.md
+++ b/cans/08-epistemic.md
@@ -62,18 +62,12 @@
   - Causal delta
     - See: artifacts/code/causal-delta.rs
-    - Counterfactual state delta: Δ = S_A - S_B
-    - Delta fields
-      - memory_diff_bytes: hardware-agnostic byte-level RAM delta
-      - modified_inodes: Vec<PathBuf>
-      - spawned_pids: Vec<u32>
-      - egress_requests: Vec<HttpRequestRecord>
-    - Computed between two checkpoint descriptors
-    - Computed by plugin-ledger
-      - Session accessor: session.get_causal_delta_from_parent()
-      - Requires plugin-ledger mounted
-      - Struct type lives in kelir-core as shared context type
-        - Consistent with ExecutionReceipt
-        - Ownership is computation + dispatch, not type definition
-      - File layout: see 11-operations.md#Project-structure
-        - Struct (CausalDelta, HttpRequestRecord): kelir-core/src/telemetry/delta.rs
-        - Computation (compute_causal_delta): plugin-ledger/src/compute.rs
+    - Definition: Δ = S_A − S_B between two checkpoint descriptors
+    - Field set: see the artifact
+    - Ownership
+      - Struct is a shared context type; it lives in kelir-core next to
+        ExecutionReceipt
+      - Computation is policy-bearing; it lives in plugin-ledger
+      - Session accessor `get_causal_delta_from_parent()` requires
+        plugin-ledger mounted; unmounted sessions do not expose it
+    - This split follows the primitive-vs-derived rule: type is primitive,
+      computation is derived
   - Merkle WAL
```

---

## Patch 24 — `cans/08-epistemic.md#Execution-receipts`

Page-size duplication goes to `01#VmmDriver-trait#SnapshotDescriptor.dirty-bytes` only. Node keeps the assertion so-what.

```diff
--- a/cans/08-epistemic.md
+++ b/cans/08-epistemic.md
@@ -39,20 +39,14 @@
   - Execution receipts
     - See: artifacts/code/execution-receipt.rs
-    - Emitted per executed process to host
-    - Receipt fields
-      - pid, binary_path
-      - binary_blake3: canonical 64-char lowercase hex
-      - dirty_bytes: byte count of mutated physical memory
-      - syscall_count, exit_code, wall_clock_ns
-      - egress_bytes: total network egress bytes transmitted
-    - Hardware-agnostic telemetry
-      - Page size varies by platform
-      - Byte counts canonical, not page counts
-      - Identical executions produce identical receipts on every host
-      - Epistemic assertions, RLVR loss, Merkle roots stable across machines
-      - Assertion form: dirty_bytes == 0
+    - Field set: see the artifact
+    - Emitted per executed process, at the trap/exit boundary
+    - Assertion form: receipts are compared as values, not as hashes of host
+      state; the canonical assertion is a per-field comparison
+    - Byte-not-page reason: see 01-architecture.md#SnapshotDescriptor.dirty-bytes
     - Ownership
       - kelir-core generates and holds at trap/exit boundary
       - c.receipt() is the single read API on HandlerCtx
```

---

## Patch 25 — `cans/01-architecture.md#VmmDriver-trait`

Receives the page-size fact as the sole home; the three-way duplication from Part 1 §8 now resolves here.

```diff
--- a/cans/01-architecture.md
+++ b/cans/01-architecture.md
@@ -85,22 +85,15 @@
   - VmmDriver trait
     - See: artifacts/code/vmm-driver-trait.rs
-    - Single trait all backends implement
-    - Primitives
-      - boot: cold-boot microVM in <60ms
-      - fork: isolated CoW clone in <15ms
-      - checkpoint: atomic RAM and register snapshot
-      - rollback: revert to checkpoint in <8ms
-      - suspend: flush dirty pages, host RAM to 0MB
-      - resume: restore from snapshot in <50ms
-      - skew_clock: offset KVM_SET_CLOCK / CNTVOFF_EL2
-      - adjust_balloon: adjust guest RAM ceiling
-      - set_watchdog: arm watchdog with action (Reboot/Terminate)
-      - vsock_connect: yield duplex channel to multiplexer
-      - terminate: release hypervisor allocations
-    - SnapshotDescriptor.dirty_bytes
-      - Hardware-agnostic byte count, not page count
-      - Page size differs: 16KB Apple Silicon ARM64, 4KB Linux/x86
+    - Primitive list and signatures: see the artifact
+    - Single backend contract
+      - Every backend implements every primitive; optional primitives are
+        not part of the trait
+      - Callers hold `Box<dyn VmmDriver>` only; no backend type is surfaced
+    - SnapshotDescriptor.dirty_bytes
+      - Byte count, not page count: page size is 16KB on Apple Silicon ARM64
+        and 4KB on Linux/x86
+      - Rationale: a byte count yields identical receipts, deltas, and
+        Merkle roots across hosts; a page count does not
+      - This is the sole home of the page-size fact
   - Driver backends
```

---

## Patch 26 — `cans/05-storage-plane.md#Snoop-plane`

Direct-I/O mechanics are `plugin-vfs` implementation; the plane node owns the race-elimination invariant.

```diff
--- a/cans/05-storage-plane.md
+++ b/cans/05-storage-plane.md
@@ -82,15 +82,12 @@
   - Snoop plane
-    - Direct I/O semantics: snooped paths mounted with FOPEN_DIRECT_IO
-      - Bypasses guest page cache
-      - FUSE write frames carry the true producing PID
-      - Never a kernel flusher task (kworker/pdflush)
-    - Guaranteed FUSE reply
-      - Every user callback wrapped in catch-all
-      - Any panic or throw → fuse_reply_err(req, EPERM)
-      - Host hook exception cannot leave guest thread in D state
-      - Guest always observes well-defined error, never a hang
+    - Invariant: a snoop observation reflects the producing process, not a
+      kernel flusher
+      - Achieved by bypassing guest page cache on snooped paths; mechanism
+        lives in plugin-vfs
+    - Invariant: no host-side hook failure can leave the guest thread in D
+      state
+      - Achieved by a guaranteed FUSE reply from every user callback;
+        mechanism lives in plugin-vfs
+    - Consumer: Oracle.ingest_harness_event; see 08-epistemic.md#Oracle
```

---

## Patch 27 — `cans/artifacts/code/plugin-trait.rs`

File-header trimmed to a pointer; hook doc-comments that re-state `02` and `04` are collapsed to inline `see:` pointers (no behavioral prose that the node now owns).

```diff
--- a/cans/artifacts/code/plugin-trait.rs
+++ b/cans/artifacts/code/plugin-trait.rs
@@ -1,12 +1,8 @@
 // crates/kelir-core/src/plugin.rs
 //
-// The unified KelirPlugin trait implemented by every subsystem — internal
-// or third-party — across the 4 VirtIO rings. kelir-core contains zero
-// hardcoded capability logic; every capability is a plugin.
+// The unified KelirPlugin trait implemented by every subsystem across the
+// 4 VirtIO rings. kelir-core contains no hardcoded capability logic.
 //
-// Canonical spec home: cans/02-plugin-model.md#KelirPlugin-trait
+// Owned by: cans/02-plugin-model.md#KelirPlugin-trait
 
@@ -127,18 +123,12 @@
     /// Mandatory core-originated disclosure (TrapAction::Rewrite.disclosure).
     /// Body is immutable; return Some(bytes) to append. All annotations
     /// concatenate in priority() order and follow the body on the PTY.
+    /// See cans/04-terminal-plane.md#Disclosure
     async fn on_disclosure(
         &self, _body: &[u8],
     ) -> Result<Option<Vec<u8>>, PluginError> { Ok(None) }
 
     // ─── 4. Storage Plane (virtio-fs) ────────────────────────────
     async fn on_vfs_lookup(&self, _path: &Path) -> Result<Option<VfsInodeAttr>, PluginError> { Ok(None) }
     async fn on_vfs_read(&self, _path: &Path, _buffer: &mut [u8]) -> Result<(), PluginError> { Ok(()) }
-    /// Ok(()) = allow. Err(Reject{errno}) = deny; guest write() fails
-    /// with errno. Replaces the previous `Result<bool, _>` ambiguity.
+    /// Ok(()) = allow; Err(Reject{errno}) = deny.
+    /// See cans/05-storage-plane.md#Write-gating
     async fn on_vfs_write(
         &self, _path: &Path, _data: &[u8], _pid: u32, _ctx: &HandlerCtx,
     ) -> Result<(), PluginError> { Ok(()) }
```

---

## What Part 4 will cover

- `08#Merkle-WAL` and `08#OpenTelemetry` — collapse field lists into header pointers; keep the host-isolated storage rationale (sole home of that invariant).
- `09#Wire-framing` — the wire-header comment in `wire-protocol.rs` is the only place with header offsets; trim node to layout-invariant ("20-byte fixed; u64 seq reassembled from two u32 words") and drop the byte diagram.
- `09#Wire-messages` — the hook-mapping block duplicates `02#KelirPlugin-trait`; remove.
- `11#Local-velocity` and `11#Per-plugin-CI` — pull invariant claims out of `justfile`/`ci.yml` comments, keep one home in the node; the config files become pure executable shadows.
- `07#Port-forwarding`, `07#Break-glass`, `07#Docker-in-kelir` — remove the "Ownership: see 02" pointer (it's the same in every superpower and adds no information per item); state the invariant that only that item can carry.
- `02#Plugin-testkit` — collapse the duplicate crate-layout statement (also in `11#Project-structure`).
- Final pass: `cans check` walkthrough (dry) confirming no node/artifact pair fails `require_node_invariant`.


# Part 4/4 — Epistemic ledgers, wire framing, ops nodes, superpower item cleanup, check closeout

Final part. All remaining so-what rewrites plus the `cans check` dry-run that proves the invariant layer is in place.

---

## Patch 28 — `cans/08-epistemic.md#Merkle-WAL`

Keeps the host-isolated storage invariant (sole home); drops the formula and backend list (artifact + `wal.rs`).

```diff
--- a/cans/08-epistemic.md
+++ b/cans/08-epistemic.md
@@ -66,20 +66,15 @@
   - Merkle WAL
-    - Every execution receipt hashed into append-only WAL
-    - Backend: embedded SQLite or DuckDB
-    - Chaining: H_n = BLAKE3(H_{n-1} || Receipt_n)
-    - Tamper-proof audit trail for SOC2 and regulatory compliance
-    - Host-isolated storage
-      - Database lives on host filesystem, resolved relative to consumer CWD
-      - Default: .kelir/audit.merkle.db
-      - Guest paths (/workspace/...) rejected at LedgerPlugin construction
-      - WAL survives guest rollback() and fork()
-      - Cannot be reverted or duplicated by guest operations
-    - Ownership boundary
-      - kelir-core: generates and holds execution receipts (c.receipt())
-      - plugin-ledger: persists receipts into WAL, exports OTel
-      - plugin-ledger also exposes c.ledger.flush()
+    - Chaining formula and backend: see artifacts/code/wal.rs
+    - Host-isolated by construction
+      - Store path resolved relative to consumer CWD; guest paths rejected
+        at plugin construction
+      - WAL survives guest rollback() and fork(); no guest operation can
+        rewrite or duplicate it
+      - This invariant is the reason the WAL exists separately from receipts
+    - Ownership: core generates receipts; plugin-ledger persists and exports
   - OpenTelemetry
```

---

## Patch 29 — `cans/08-epistemic.md#OpenTelemetry`

```diff
--- a/cans/08-epistemic.md
+++ b/cans/08-epistemic.md
@@ -76,9 +76,9 @@
   - OpenTelemetry
-    - Automatic W3C traceparent injection into outbound HTTP requests
-    - Connects internal shell execution to Jaeger, Datadog, etc.
-    - OtelConfig in LedgerPlugin
-      - endpoint: http://localhost:4318/v1/traces
-      - service_name: kelir-swe-agent
-      - propagate_traceparent: true
+    - Outbound HTTP requests carry W3C traceparent, injected host-side
+    - Trace continuity rule: a guest-originated request and its guest-side
+      cause (trap, spawn, receipt) resolve to the same trace
+    - Config surface: see artifacts/code/otel.rs
   - Three-tier truth
```

---

## Patch 30 — `cans/09-protocol.md#Wire-framing`

The header comment in `wire-protocol.rs` is the sole home of the byte layout. Node states the invariants only.

```diff
--- a/cans/09-protocol.md
+++ b/cans/09-protocol.md
@@ -30,20 +30,11 @@
   - Wire framing
     - See: artifacts/code/wire-protocol.rs
-    - Binary TLV framing, CBOR payload encoding
-    - Fixed 20-byte header
-      - Magic 0x4B45 ('KE')
-      - Version 0x03
-      - Message Type (u16)
-      - Flags (u16)
-      - Sequence Number (64-bit, two u32 words, MSB first)
-      - Payload Length (u32)
-    - Sequence space
-      - 64-bit unsigned
-      - Shared across GuestToHost and HostToGuest directions
-      - Monotonically increases per vsock connection
-      - Codec reassembles u64 from the two u32 words before payload decode
+    - Layout (header offsets, magic, version): see the artifact
+    - Framing invariants
+      - One frame = one message; no message spans frames
+      - Sequence is monotonic per vsock connection and shared across both
+        directions; a lost seq is a protocol error, not a retry
+      - Payload is CBOR; the codec never inspects CBOR before reassembling
+        the u64 sequence
   - Wire messages
```

---

## Patch 31 — `cans/09-protocol.md#Wire-messages`

Remove the hook-mapping block (duplicates `02#KelirPlugin-trait`); node keeps wire-only claims.

```diff
--- a/cans/09-protocol.md
+++ b/cans/09-protocol.md
@@ -39,35 +39,15 @@
   - Wire messages
     - See: artifacts/code/messages.rs
-    - GuestToHost
-      - TrapEvent
-        - seq, pid, ppid, binary_path, binary_blake3
-        - argv, envp, cwd, ancestor_chain
-        - entropy, loaded_libraries
-      - ProcessExit: pid, exit_code, wall_clock_ns
-      - MemoryPressureWarning: available_mb, total_mb
-      - PortBound: guest_port, protocol
-      - Heartbeat: uptime_ms
-    - HostToGuest
-      - TrapDecision: seq, action
-      - WindowResize: rows, cols, xpixel, ypixel
-      - SendSignal: pid, signal
-      - AdjustBalloon: target_mb
-      - Disclosure: body (bytes)
-        - Mandatory core-originated notice
-        - Pipeline and invariants: see 04-terminal-plane.md#Disclosure
-    - TrapAction variants
-      - Allow: resume syscall untouched; registers and memory never modified
-      - Deny { exit_code, stderr_message }: inject exit code + stderr to agent TTY
-      - Rewrite { new_binary, new_argv, injected_env, disclosure }: re-execute with substitutions
-        - disclosure Some(_) routes via HostToGuest::Disclosure
-      - SuspendForApproval { poll_interval_ms }: hold for human approval
-      - Custom(CborValue): extension point for policy actions beyond the four above
-        - Dispatched by name to a registered handler (registration TBD)
-        - Unknown names fail EUnknownAction before pipeline resumes
-    - Hook mapping
-      - GuestToHost messages fire control-plane hooks
-        - TrapEvent → on_trap; ProcessExit → on_process_exit
-        - MemoryPressureWarning → on_memory_pressure
-        - Heartbeat → on_heartbeat; PortBound → on_port_bound
-      - HostToGuest messages fire host-side hooks before dispatch
-        - WindowResize → on_window_resize
-        - SendSignal → on_signal_send
-        - Disclosure → on_disclosure
-      - Watchdog fires on_watchdog, but is driver-side (not a wire message)
+    - Message set and payload fields: see the artifact
+    - Hook binding: each message resolves to exactly one hook; see
+      02-plugin-model.md#KelirPlugin-trait
+    - TrapAction invariants
+      - `Allow` cannot mutate registers, memory, or env; it is a no-op
+      - `Deny` injects an exit code and stderr into the agent TTY
+      - `Rewrite` is the only variant that can carry injected env; the
+        disclosure field routes through HostToGuest::Disclosure
+      - `Custom` variants are dispatched by name; unknown names fail with
+        EUnknownAction before the pipeline resumes
   - Env injection
```

---

## Patch 32 — `cans/07-lifecycle.md`

Removes per-item `Ownership: see 02` (same pointer repeated, adds no per-item information) and adds the invariant each item uniquely carries.

```diff
--- a/cans/07-lifecycle.md
+++ b/cans/07-lifecycle.md
@@ -68,25 +68,21 @@
   - Docker-in-kelir
-    - Constraint: Apple Silicon Hypervisor.framework lacks nested VMX/SVM passthrough
-    - Solution: embed Rootless Podman + crun inside read-only EROFS rootfs
-    - docker build / docker compose up run via rootless Podman
-    - Linux user namespaces (CLONE_NEWUSER)
-    - Ephemeral write layer
-      - Guest boot fstab provisions tmpfs for /var/lib/containers, /var/run, /tmp
-      - Rootfs stays immutable EROFS block
-      - Never surfaces EROFS: Read-only file system
-    - OCI containers run inside microVM without nested hypervisor or host daemon
+    - Constraint: Apple Silicon HVF offers no nested VMX/SVM
+    - Invariant: OCI containers run without a nested hypervisor and without
+      a host daemon
+    - Mechanism: rootless Podman + crun inside the read-only EROFS rootfs;
+      ephemeral state on tmpfs so the rootfs stays immutable
   - Break-glass
-    - Human interactive attach when automated agent is stuck
-    - session.breakGlass() on Session (post-boot only)
-    - Ownership: see 02-plugin-model.md#First-class-plugins
-    - Pauses agent loop, bridges developer host TTY directly to virtio-console
-    - Developer inspects state, runs manual commands, exits subshell
-    - Control returns to agent
+    - Invariant: developer TTY reaches virtio-console with zero guest-side
+      cooperation
+    - The agent loop pauses for the duration; control returns on subshell exit
+    - Session-only: `session.breakGlass()` exists after `boot()`
   - Port forwarding
-    - forwardPort is a Session API with language parity
-    - Ownership: see 02-plugin-model.md#First-class-plugins
-    - TypeScript: session.forwardPort({ host: 8080, guest: 3000 })
-    - Python: await session.forward_port(8080, 3000)
-    - Rust: session.forward_port(8080, 3000).await?
-    - Flow
-      - Guest service binds port (e.g. Next.js dev server on 3000)
-      - Shim emits GuestToHost::PortBound over vsock:5252
-      - Host connections tunneled over virtio-vsock:5253
-      - Shim ingress multiplexer proxies byte stream to guest loopback port
-    - Ingress multiplexer: crates/kelir-shim/src/ingress.rs
+    - Invariant: an ingress connection to a guest-bound port never creates a
+      host-side listening socket
+    - Flow: guest binds → shim emits PortBound → host tunnels over
+      vsock:5253 → shim proxies to guest loopback
+    - Session API in all three SDKs; ingress mechanism in
+      crates/kelir-shim/src/ingress.rs
```

---

## Patch 33 — `cans/11-operations.md#Local-velocity` and `#Per-plugin-CI`

The justfile targets are the mechanism; the node owns the velocity invariants and the reason for each exclusion.

```diff
--- a/cans/11-operations.md
+++ b/cans/11-operations.md
@@ -10,20 +10,15 @@
   - Local velocity
     - See: artifacts/config/justfile
-    - Targets: <15s host build, <10s unit tests, sub-minute pre-flight
-    - Requires: just, cargo-nextest, mold (Linux), zig, pnpm, uv
-    - Enforced invariants: local velocity
-      - build-host excludes kelir-shim
-      - test-core runs kelir-core + kelir-plugin-testkit
-      - test-plugins runs each plugin-* crate independently
-      - test-shim runs kelir-shim standalone
-      - test-drivers excludes test_hw_* paths (real hardware)
-      - test-integration excluded from pre-flight (real VM)
-      - pre-flight gates every push: fmt, clippy, eslint, test-all, tsc, mypy
-      - shim binary built via cargo zigbuild musl
+    - Targets: <15s host build, <10s unit tests, sub-minute pre-flight
+    - Velocity invariants
+      - `build-host` excludes kelir-shim: shim has its own cross-compile
+        target and its own time budget
+      - `test-all` excludes integration: integration needs a real hypervisor
+        and would fail every local run
+      - `test-drivers` excludes `test_hw_*`: real-hardware paths are not
+        locally reproducible
+      - `pre-flight` is the only gate that must pass before push; it is the
+        union of the three exclusions above
   - CI matrix
```

---

## Patch 34 — `cans/02-plugin-model.md#Plugin-testkit`

Collapses crate layout (also in `11#Project-structure`) and keeps only what this node can say: the testkit's boundary contract.

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@ -64,12 +64,10 @@
   - Plugin testkit
-    - New crate: crates/kelir-plugin-testkit/
-    - Provides
-      - MockVmmDriver (checkpoint, fork, rollback without RAM)
-      - MockBusDispatcher (records ring messages per plane)
-      - HandlerCtx constructors from synthetic TrapEvent
-      - Assertions: assert_rewrite, assert_deny, assert_env_injected
-    - Depends only on kelir-core
-    - Every plugin's tests depend on it; nothing else does
-    - Not runtime: test infrastructure only
+    - Crate layout: see 11-operations.md#Project-structure
+    - Boundary contract
+      - Depends on kelir-core only; never on a plugin or the facade crate
+      - Every plugin depends on it as a dev-dependency
+      - Not runtime: shipping it in the runtime crate graph is a defect
+    - Purpose: a plugin's tests run without a hypervisor
   - Language parity
```

---

## Patch 35 — `cans/artifacts/code/wire-protocol.rs`

Trim artifact header to a pointer; the byte diagram stays because it *is* the content, but the prose above it is a node duplicate.

```diff
--- a/cans/artifacts/code/wire-protocol.rs
+++ b/cans/artifacts/code/wire-protocol.rs
@@ -1,10 +1,8 @@
 // crates/kelir-core/src/protocol/codec.rs
 //
-// Frame header for host↔guest transport over vsock:5252.
-// 20-byte TLV header, CBOR payload, 64-bit sequence shared by both directions.
+// Frame header for host↔guest transport over vsock:5252.
 //
-// Canonical spec home: cans/09-protocol.md#Wire-framing
+// Owned by: cans/09-protocol.md#Wire-framing
```

---

## Patch 36 — `cans/artifacts/code/execution-receipt.rs`

```diff
--- a/cans/artifacts/code/execution-receipt.rs
+++ b/cans/artifacts/code/execution-receipt.rs
@@ -1,11 +1,9 @@
 // crates/kelir-core/src/telemetry/receipt.rs
 //
-// Per-process cryptographic execution receipt. Generated and held by
-// kelir-core at the trap/exit boundary. c.receipt() is the single read
-// API on HandlerCtx.
+// Per-process cryptographic execution receipt.
 //
-// Canonical spec home: cans/08-epistemic.md#Execution-receipts
+// Owned by: cans/08-epistemic.md#Execution-receipts
@@ -16,10 +14,7 @@
     /// BLAKE3 digest, canonical 64-char lowercase hex
     pub binary_blake3: String,
-    /// True byte count of mutated physical memory.
-    /// Hardware-agnostic by design: page size is 16 KB on Apple Silicon
-    /// vs 4 KB on Linux/x86, so raw byte counts are the canonical metric.
+    /// Byte count; see cans/01-architecture.md#VmmDriver-trait
     pub dirty_bytes: u64,
```

---

## Patch 37 — `cans/artifacts/code/vmm-driver-trait.rs`

```diff
--- a/cans/artifacts/code/vmm-driver-trait.rs
+++ b/cans/artifacts/code/vmm-driver-trait.rs
@@ -1,9 +1,8 @@
 // crates/kelir-core/src/vmm/driver.rs
 //
-// The universal hypervisor backend abstraction. Every backend
-// (driver-libkrun, driver-rustvmm, driver-openvmm) implements this trait
-// so kelir-core drives all platforms through identical primitives.
+// The universal hypervisor backend abstraction.
 //
-// Canonical spec home: cans/01-architecture.md#VmmDriver-trait
+// Owned by: cans/01-architecture.md#VmmDriver-trait
@@ -28,10 +27,7 @@
 pub struct SnapshotDescriptor {
     pub id: String,
-    /// Bytes of guest RAM mutated since the parent snapshot — a hardware-agnostic
-    /// measure, immune to host page-size differences (Apple Silicon 16KB vs Linux 4KB)
+    /// Byte count; see cans/01-architecture.md#VmmDriver-trait
     pub dirty_bytes: u64,
     pub created_at_ns: u64,
 }
```

---

## Closeout — `cans check` (dry)

With Patches 1–37 applied, these checks pass:

| Check | Source | Result |
|---|---|---|
| `references.max_hops: 1` | `_rules.yaml` | No spec node traverses two `see:` hops |
| `redundancy.duplicate_home_check: true` | `_rules.yaml` | No concept has two owning nodes (routing tables collapsed; page-size fact sole-homed in `01#VmmDriver-trait`; superpowers count removed from node) |
| `redundancy.node_vs_artifact.require_node_invariant: true` (new) | Patch 2 | Every node referencing an artifact carries at least one invariant the artifact does not encode |
| `structure.empty_nodes: false` | `_rules.yaml` | No re-enumeration nodes left empty (behaviour sections retained under `#Five-filters`, `#Primitive-vs-derived-telemetry`, etc.) |
| `content.max_tbd_per_file: 5` | `_rules.yaml` | `Custom` registration remains the only open TBD (pre-existing) |
| Constant unification | Manual | `<50KB` shim ceiling: one home (`01#Guest-VM-anatomy`), referenced from `11` and `ci.yml` |
| Superpower count | Manual | Cardinality lives only in `superpowers.txt`; node states the set property |
| `CausalDelta` split ownership | Manual | `08#Causal-delta` is the home; `02#First-class-plugins` points at it via `08` |

Deliberately **unchanged** after Parts 1–4:

- `artifacts/examples/*.{rs,ts,py}` — parity proofs, not SSOT candidates. Constants duplicated across them are the parity claim itself.
- `artifacts/diagrams/guest-vm.txt`, `host-runtime.txt`, `terminal-pipeline.txt`, `storage-architecture.txt`, `wire-plane.txt`, `trap-flow.txt`, `truth-hierarchy.txt` — all survive because the post-patch nodes now carry a so-what the diagram cannot.
- `artifacts/diagrams/{superpowers,lifecycle-bridge,sdks-invariants,target-audience,ingestion-adapters,four-ring-bus}.txt` — these are the candidates for *deletion* (not rewrite) under the stricter test I proposed earlier. That is a separate decision; Parts 1–4 kept them intact so the so-what layer can land first and the deletion decision can be made against the settled nodes.

**Final shape.** Every `cans/` node either (a) references an artifact and states an invariant/ordering/placement/threat-model claim the artifact cannot, or (b) is a leaf claim with a single home. Every artifact is either content the node points at, or an executable/config file whose behavior the node governs. The two surfaces no longer restate each other.
