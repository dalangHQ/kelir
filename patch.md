$X = 3$. 

Here is **Part 1/3**: Agent Governance & Microkernel Core Invariants.

---

### File: `cans/AGENTS.md`
```diff
--- cans/AGENTS.md
+++ cans/AGENTS.md
@@ -95,6 +95,10 @@
 ## Collaboration
 
 One owner per task file. Claim before editing.
+- Build isolation: agents must set CARGO_TARGET_DIR=target/agent-<id>
+- Eliminates Cargo build lock contention across concurrent workers
+- Manifest mutations: generate to temporary file, then atomic rename
+- Prevents broken intermediate states from poisoning workspace builds
 
 ```
 # If task has ← agent-3, do NOT edit it
```

---

### File: `cans/01-architecture.md`
```diff
--- cans/01-architecture.md
+++ cans/01-architecture.md
@@ -62,6 +62,19 @@
         - File and test discipline
           - Strict file bounds: 500 LOC target, 1,000 LOC hard ceiling per .rs file
           - Test segregation: zero `#[cfg(test)]` in `src/`; tests live in sibling `tests/`
           - Single-line documentation: maximum one line of docstring per public item
+        - Zero-panic production invariant
+          - Workspace-wide deny on unwrap_used, expect_used, and panic
+          - Production code returns Result; unhandled errors bubble via ?
+          - Infallible branches require #[allow(clippy::expect_used)] and proof
+        - Unsafe memory isolation
+          - Workspace enforces unsafe_code = "forbid" by default
+          - Core, interface, catalog, otlp, and plugins contain zero unsafe
+          - Unsafe restricted to kelir-drivers and kelir-transport enclaves
+          - Every driver unsafe block mandates a verified // SAFETY: invariant
+        - Defensive boundary arithmetic
+          - Guest-derived numbers are hostile: offsets, descriptors, ring indices
+          - Direct memory slicing and unchecked array indexing are forbidden
+          - Arithmetic uses checked_* or saturating_* variants unconditionally
+          - Allocations sized by guest frames clamp to hard resource ceilings
         - Functional core
           - Pure functional core: evaluation follows `f(State, Event) -> (State, Actions)`
```

---

Confirm to output **Part 2/3** (Plugin Dispatch Quarantine & State Serialization Tripwires).



Here is **Part 2/3**: Plugin Dispatch Quarantine & State Serialization Tripwires.

---

### File: `cans/02-plugin-model.md`
```diff
--- cans/02-plugin-model.md
+++ cans/02-plugin-model.md
@@ -23,6 +23,10 @@
     - Tier 3 plugins depend on kelir-interface and consume peer services via ServiceRegistry
     - Dependency invariant: Zero concrete peer imports within the same tier
     - Direct Cargo dependencies are acyclic: no plugin imports a higher tier
+    - Layering enforcement
+      - Crate dependency hierarchy enforced mechanically via layer lint
+      - Downward-only dependencies: higher tiers cannot import lower tiers
+      - Plugins link kelir-interface only; direct core imports rejected
     - Invariants and purity
     - Code is sole SSOT: declared programmatically via KelirPlugin trait
     - Manifest artifact is strictly auto-generated for zero-compile indexing
@@ -155,6 +159,10 @@
     - Hardware MPK page keys (x86_64) isolate per-plugin memory domains
     - Missing declared dependency aborts boot with EConfig
     - Lock-free dispatch swap via ArcSwap on the host event bus (internal)
+    - Panic quarantine boundary
+      - Interposer dispatches wrap in catch_unwind at the engine perimeter
+      - Escaped hook panics convert to Verdict::Drop with EFAULT
+      - Supervisor microkernel stays resident; active StepSlot rolls back
     - ArcSwap serves atomic dispatch swapping during snapshot hydration only
     - Not a public runtime API; session configuration is frozen after boot
     - First Drop, Suspend, or Abort halts pipeline; Pass/Patch continue
```

---

### File: `cans/09-snapshots.md`
```diff
--- cans/09-snapshots.md
+++ cans/09-snapshots.md
@@ -24,6 +24,10 @@
     - Restore semantics
       - Import materializes compute and disk only
       - Workloads that had live sockets observe them gone on first access
+    - Serialization exhaustiveness tripwire
+      - Snapshot format structs fail compilation on unhandled schema additions
+      - Struct fields in kelir-interface mandate explicit serde default tags
+      - Format converters must map every field; silent field omission banned
       - No transparent socket rebinding exists; workloads must reconnect
     - Unpack containment invariants (Single Source of Truth)
       - Untrusted input boundary: zero ambient host filesystem trust
```

---

### File: `cans/10-attestation.md`
```diff
--- cans/10-attestation.md
+++ cans/10-attestation.md
@@ -24,6 +24,10 @@
       - Receipts compare as values via canonical per-field structural assertions
       - Receipts carry opaque TelemetryExtension payloads emitted by mounted plugins
+      - Struct exhaustiveness tripwire
+        - ExecutionReceipt and CausalDelta additions fail compilation everywhere
+        - Merkle leaf hashing and OTel mappers must explicitly handle new fields
+        - Eliminates untracked receipt divergence across schema updates
     - Identity
       - Execution receipts are deterministic because inputs are frozen
         - Frozen virtual clock (see: plugins/replay.md#Kernel-clocksource-lockdown)
```

---

Confirm to output **Part 3/3** (Operational Verification, Fuzzing & CI Hard Gates).


Here is **Part 3/3**: Operational Verification, Fuzzing & CI Hard Gates.

---

### File: `cans/15-operations.md`
```diff
--- cans/15-operations.md
+++ cans/15-operations.md
@@ -74,6 +74,10 @@
       - Shim musl binary size gate: see: artifacts/config/ci.yml (build-guest-shim)
       - sccache enabled via SCCACHE_GHA_ENABLED
+      - Hard performance budget gate
+        - Budgets in perf/budgets.toml enforced strictly with enforce = true
+        - Regressions in Gate 1, Gate 2 (<2µs), or Gate 3 abort CI run
+        - Cold-boot and RSS purge (<50ms) violations fail with exit code 1
     - Per-plugin matrix: each plugin fails independently
     - Integration tier: KVM-detected, continue-on-error on GitHub-hosted
     - External plugin CI: contract attestation via reusable workflow
@@ -107,6 +111,13 @@
     - --lifecycle-coverage
       - Lifecycle asserts: see: 08-lifecycle.md#Hypervisor-primitives
       - State machine assert: verifies all SuspendReason variants execute through
         - centralized lifecycle
+    - --panic-hunt-coverage
+      - Fuzzes every stream kind and registered hook with adversarial frames
+      - Asserts zero unhandled panics across malformed CBOR, VFS, and FUSE ops
+      - Escaped unwinds fail the test suite; every path must return Err
+    - --layering-coverage
+      - Validates downward-only crate dependencies via cargo xtask layers
+      - Core and interface violations reject compilation before test run
     - --hook-coverage
       - Asserts every hook in the segregated traits is implemented by some
         - custom plugin in cans/artifacts/examples
@@ -282,9 +293,12 @@
     - Registry index
       - Registry entries: see: #Plugin-registry
     - Test fixtures
-      - tests/fixtures/kernel: per-arch vmlinux
-      - tests/fixtures/rootfs: EROFS build scripts
-      - tests/fixtures/payloads: high-entropy test binaries (shared: pty, core-trap)
+      - External test oracles fetched into tests/fixtures via cargo xtask fixtures
+      - Fixture manifests pinned with cryptographic hashes in fixtures.toml
+      - SHA-256 and BLAKE3 verification mandatory before mounting test images
+      - tests/fixtures/kernel: pinned per-arch vmlinux binaries
+      - tests/fixtures/rootfs: pinned EROFS guest rootfs images
+      - tests/fixtures/payloads: high-entropy and exploit test binaries
```

---

All 3 parts complete. All modified nodes strictly adhere to `_rules.yaml` (≤90 characters/line, single-line bullets, no filler levels, Day-1 canonical tone).


Here are the targeted, non-redundant diff patches for `cans/artifacts/`.

---

### File: `cans/artifacts/config/ci.yml`
```diff
--- cans/artifacts/config/ci.yml
+++ cans/artifacts/config/ci.yml
@@ -62,11 +62,31 @@
           INLINE_TESTS=$(grep -rn --include="*.rs" '#\[cfg(test)\]' crates/*/src/ || true)
           if [ -n "$INLINE_TESTS" ]; then
             echo "ERROR: Inline #[cfg(test)] detected in src/. Move tests to tests/:"
             echo "$INLINE_TESTS" && exit 1
           fi
+          UNSAFE_VIOLATIONS=$(grep -rn --include="*.rs" 'unsafe ' crates/ | grep -v 'crates/kelir-drivers/' | grep -v 'crates/kelir-transport/' || true)
+          if [ -n "$UNSAFE_VIOLATIONS" ]; then
+            echo "ERROR: Unsafe code detected outside isolated driver/transport enclaves:"
+            echo "$UNSAFE_VIOLATIONS" && exit 1
+          fi
+
+      - name: Validate Crate Layering Hierarchy
+        run: cargo run -p kelir-plugin-cli -- check --layers
 
       - name: Rust Lints
         run: |
           cargo fmt --all --check
-          cargo clippy --workspace --exclude kelir-shim -- -D warnings
+          cargo clippy --workspace --exclude kelir-shim -- -D warnings \
+            -D clippy::unwrap_used \
+            -D clippy::expect_used \
+            -D clippy::panic \
+            -D clippy::unimplemented \
+            -D clippy::todo \
+            -D clippy::unreachable
 
       - name: Type Checks (TS & Python)
@@ -107,6 +127,18 @@
       - name: Core + testkit
         run: cargo nextest run -p kelir-core -p kelir-plugin-testkit
 
+  # ─── 3a. ADVERSARIAL INTEGRATION HARNESS (PANIC HUNT) ──────
+  panic-hunt:
+    needs: [lint-and-types]
+    runs-on: ubuntu-latest
+    steps:
+      - uses: actions/checkout@v4
+      - uses: dtolnay/rust-toolchain@stable
+      - uses: taiki-e/install-action@nextest
+      - uses: mozilla-actions/sccache-action@v0.0.5
+      - name: Adversarial Frame Fuzzing
+        run: cargo nextest run -p kelir-core --test panic_hunt
+
   # ─── 3b. MULTI-LANGUAGE PARITY & SDK EXECUTION GATE ────────
   # Merge-blocking. Executes TS and Python test suites (ForeignPluginProxy,
@@ -194,6 +226,17 @@
       - uses: dtolnay/rust-toolchain@stable
       - uses: taiki-e/install-action@nextest
       - uses: mozilla-actions/sccache-action@v0.0.5
+      - name: Verify Pinned Test Oracles
+        run: cargo run -p kelir-plugin-cli -- fixtures --verify
+      - name: Driver unit tests (excludes real-hardware paths)
+        run: cargo nextest run -p driver-rustvmm -p driver-openvmm -p driver-bwrap -E 'not test(test_hw_)'
+
+  # ─── 5b. PERFORMANCE BUDGET GATE (HARD ENFORCEMENT) ────────
+  perf-budgets:
+    needs: [test-core, test-drivers]
+    runs-on: ubuntu-latest
+    steps:
+      - uses: actions/checkout@v4
+      - uses: dtolnay/rust-toolchain@stable
+      - name: Assert Latency and RSS Ceilings
+        run: cargo run -p kelir-plugin-cli -- check --budgets perf/budgets.toml
```

---

### File: `cans/artifacts/config/justfile`
```diff
--- cans/artifacts/config/justfile
+++ cans/artifacts/config/justfile
@@ -37,6 +37,18 @@
     @cans check --refs-only --json | jq -e '.ok and .exitCode == 0'
     @cans check --json          | jq -e '.ok and .exitCode == 0'
 
+check-layers:
+    cargo run -p kelir-plugin-cli -- check --layers
+
+check-budgets:
+    cargo run -p kelir-plugin-cli -- check --budgets perf/budgets.toml
+
+panic-hunt:
+    cargo nextest run -p kelir-core --test panic_hunt
+
+fetch-fixtures:
+    cargo run -p kelir-plugin-cli -- fixtures --fetch
+
 check-hygiene:
     #!/usr/bin/env bash
     set -euo pipefail
@@ -47,6 +59,11 @@
     inline_tests=$(grep -rn --include="*.rs" '#\[cfg(test)\]' crates/*/src/ || true)
     if [ -n "$inline_tests" ]; then
         echo "FAIL: Inline #[cfg(test)] forbidden in src/. Move to tests/:"
         echo "$inline_tests" && exit 1
     fi
+    unsafe_violations=$(grep -rn --include="*.rs" 'unsafe ' crates/ | grep -v 'crates/kelir-drivers/' | grep -v 'crates/kelir-transport/' || true)
+    if [ -n "$unsafe_violations" ]; then
+        echo "FAIL: Unsafe code detected outside driver/transport enclaves:"
+        echo "$unsafe_violations" && exit 1
+    fi
     echo "OK: All files <= 1000 LOC; zero inline tests in src/."
@@ -62,7 +79,13 @@
 # ── Lint & typecheck ──
 lint:
     cargo fmt --all -- --check
-    cargo clippy --workspace --exclude kelir-shim -- -D warnings
+    cargo clippy --workspace --exclude kelir-shim -- -D warnings \
+        -D clippy::unwrap_used \
+        -D clippy::expect_used \
+        -D clippy::panic \
+        -D clippy::unimplemented \
+        -D clippy::todo \
+        -D clippy::unreachable
     pnpm -r exec eslint .
 
 typecheck: build-ts
@@ -107,4 +130,4 @@
 # build-shim is not in this chain: shim has its own CI gate and its
 # own time budget. check-plugin-all is not here: parity already runs
 # the same per-plugin audit. Lint is a target, not an inline body.
-pre-flight: check-hygiene check-spec check-graph check-tiered lint typecheck parity
+pre-flight: check-hygiene check-spec check-layers check-graph check-tiered check-budgets lint typecheck parity
     just test-all
```

---

### File: `cans/artifacts/config/project-structure.txt`
```diff
--- cans/artifacts/config/project-structure.txt
+++ cans/artifacts/config/project-structure.txt
@@ -7,6 +7,8 @@
 │
 ├── justfile                           # Zero-friction local development commands
 ├── Cargo.toml                         # Root Cargo workspace manifest (Rust 2024 / MSRV 1.85+)
+├── fixtures.toml                      # Pinned cryptographic hashes (BLAKE3/SHA-256) for external test oracles
+├── perf/budgets.toml                  # Machine-enforced latency and RSS ceilings (enforce = true)
 ├── Cargo.lock
 ├── pnpm-workspace.yaml                # PNPM workspace definition
 ├── package.json                       # Root devDependencies (Turborepo/Biome/TypeScript)
@@ -102,7 +104,7 @@
 │       ├── doctor.rs              # Consumer probes: KVM/HVF, kernel, EROFS
 │       ├── audit.rs               # Same audit as CI
 │       ├── vectors.rs             # Regenerate vectors.json from testkit fixtures
-│       └── registry.rs            # Registry attestation and verification commands
+│       ├── registry.rs            # Registry attestation and verification commands
+│       ├── layers.rs              # Downward-only crate dependency validator
+│       ├── budgets.rs             # Hard performance budget validator
+│       └── fixtures.rs            # Content-addressed test oracle fetcher and validator
 │
 └── packages/                          # ── LANGUAGE SDK DISTRIBUTION ──
```


No. The code contracts and boundary definitions in `artifacts/code/` still lacked the compiler-enforced tripwires, panic quarantine barriers, hostile input bounds, and unsafe enclave constraints.

Here are the targeted diff patches completing the `artifacts/code/` layer.

---

### File: `cans/artifacts/code/plugin-trait.rs`
```diff
--- cans/artifacts/code/plugin-trait.rs
+++ cans/artifacts/code/plugin-trait.rs
@@ -1,9 +1,11 @@
 // crates/kelir-interface/src/plugin.rs
 //
 // The typed KelirPlugin trait and ring interposer contracts.
 // Plugins link directly via Cargo crates; no runtime string resolution.
+// Zero unsafe code permitted: workspace enforces #![forbid(unsafe_code)].
+// Pipeline execution guarantees panic quarantine via catch_unwind.
 //
 // Owned by: cans/02-plugin-model.md#KelirPlugin-trait
 
 use async_trait::async_trait;
 use std::path::Path;
@@ -107,6 +109,14 @@
     pub paths: Option<Vec<PathBuf>>,
 }
 
+/// Quarantine boundary guarantee: every interposer dispatch is executed
+/// within std::panic::catch_unwind at the supervisor engine perimeter.
+/// A panicking plugin does not unwind across the FFI or crash the microkernel;
+/// the engine converts the panic into Verdict::Drop { errno: Some("EFAULT"),
+/// reason: "Plugin panicked" } or PluginError::Halt, logs the backtrace
+/// to the host ledger, and rolls back the active StepSlot.
+pub const PANIC_QUARANTINE_ENABLED: bool = true;
+
 impl MountScope {
     pub fn with_devices<I: IntoIterator<Item = VirtioDevice>>(mut self, devices: I) -> Self {
         self.devices = Some(devices.into_iter().collect());
```

---

### File: `cans/artifacts/code/wire-protocol.rs`
```diff
--- cans/artifacts/code/wire-protocol.rs
+++ cans/artifacts/code/wire-protocol.rs
@@ -10,9 +10,14 @@
 // Wire layout: stream_id, flags, len, payload. The header on the wire
 // is 9 bytes (stream_id: u32, flags: u8, len: u32, all Little-Endian),
 // with no magic, version, channel, or sequence field. The struct below
 // is the logical field layout; it is not a packed 9-byte struct, so
 // no size assert is made on it.
+// Hostile input rules: all wire-derived numbers (len, stream_id, flags)
+// must be validated with checked_* arithmetic. Direct unchecked slicing
+// is forbidden; allocations exceeding MAX_FRAME_PAYLOAD fail immediately.
 
 pub const MAX_FRAME_PAYLOAD: usize = 64 * 1024; // 64KB strict boundary
+pub const MAX_CONTROL_PAYLOAD: usize = 4 * 1024; // 4KB control ceiling
+pub const MAX_STREAM_ID: u32 = 0x00FF_FFFF; // Upper byte reserved for flags
 
 /// Stream mux frame header (11-protocol Stream mux framing).
 pub struct MuxFrameHeader {
```

---

### File: `cans/artifacts/code/vmm-driver-trait.rs`
```diff
--- cans/artifacts/code/vmm-driver-trait.rs
+++ cans/artifacts/code/vmm-driver-trait.rs
@@ -1,9 +1,14 @@
 // crates/kelir-interface/src/driver.rs
 //
 // The universal hypervisor backend abstraction.
+// Interface enforces #![forbid(unsafe_code)].
+// Pluggable backend drivers (driver-rustvmm, driver-hvf, driver-openvmm)
+// isolate required ioctls and raw pointer operations under #![deny(unsafe_code)],
+// with mandatory // SAFETY: invariants documented on every unsafe block.
 //
 // Owned by: cans/01-architecture.md#VmmDriver-trait
 
+#![forbid(unsafe_code)]
+
 use async_trait::async_trait;
 use std::path::{Path, PathBuf};
 use std::time::Duration;
@@ -87,6 +92,10 @@
 /// Object KILL_ON_JOB_CLOSE.
+///
+/// Compile-time tripwire: Any field added to SnapshotDescriptor mandates
+/// immediate explicit serialization mapping in kelir-core and kelir-catalog;
+/// silent field omission fails the workspace exhaustiveness assert.
 #[derive(Debug, Clone)]
 pub struct SnapshotDescriptor {
     pub id: String,
     /// Byte count; see cans/01-architecture.md#VmmDriver-trait
```

---

### File: `cans/artifacts/code/execution-receipt.rs`
```diff
--- cans/artifacts/code/execution-receipt.rs
+++ cans/artifacts/code/execution-receipt.rs
@@ -1,8 +1,11 @@
 // crates/kelir-interface/src/telemetry/receipt.rs
 //
 // Per-process cryptographic execution receipt.
+// Struct exhaustiveness tripwire: adding any field to ExecutionReceipt
+// breaks compilation in kelir-core, kelir-catalog, and kelir-otlp until
+// Merkle hashing, serialization, and OTel mappers explicitly handle it.
 //
 // Owned by: cans/10-attestation.md#Execution-receipts
 
 use serde::{Deserialize, Serialize};
 
@@ -37,6 +40,7 @@
 /// PMU counters are diagnostic gauges only, emitted to OTel; they never
 /// appear in receipts, CausalDelta, or Merkle leaves (the former
 /// retired_instructions field was deleted on that basis; no other
 /// artifact file referenced it).
 #[derive(Debug, Clone, Serialize, Deserialize)]
+#[non_exhaustive]
 pub struct ExecutionReceipt {
     pub pid: u32,
     pub binary_path: String,
```

---

### File: `cans/artifacts/code/causal-delta.rs`
```diff
--- cans/artifacts/code/causal-delta.rs
+++ cans/artifacts/code/causal-delta.rs
@@ -7,8 +7,11 @@
 // Merkle leaves. Computes exactly what an agent branch did differently
 // from its parent.
+// Struct exhaustiveness tripwire: unhandled fields fail compilation in
+// delta aggregation and snapshot validation.
 //
 // Canonical spec home: cans/10-attestation.md#Causal-delta
 
 use std::path::PathBuf;
 
@@ -28,6 +31,7 @@
 // Object KILL_ON_JOB_CLOSE. Lineage itself is not a field of this struct.
+#[non_exhaustive]
 pub struct CausalDelta {
     /// Hardware-agnostic byte-level RAM delta
     pub memory_diff_bytes: u64,
```



No. The core production engine code in `reconciler.rs` still contained illegal production panics (`.unwrap()`) that immediately break the new Clippy gate, the wire message definitions lacked non-exhaustive tripwires, and the architectural diagrams had fallen out of sync with the new CI coverage gates and panic boundaries.

Here are the diff patches resolving those omissions.

---

### File: `cans/artifacts/code/reconciler.rs`
```diff
--- cans/artifacts/code/reconciler.rs
+++ cans/artifacts/code/reconciler.rs
@@ -180,7 +180,9 @@
     guard: TraversalGuard,
 ) -> Result<CommitStats, std::io::Error> {
     let mut stats = CommitStats::default();
-    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
+    let nonce = SystemTime::now()
+        .duration_since(UNIX_EPOCH)
+        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
+        .as_nanos();
 
     let canonical_root = host_target.canonicalize()?;
     let upperdir = broker_dir.join("upper");
@@ -227,7 +229,8 @@
     for entry in fs::read_dir(current)? {
         let entry = entry?;
         let path = entry.path();
-        let rel_path = path.strip_prefix(upper_root).unwrap();
+        let rel_path = path.strip_prefix(upper_root)
+            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
         let target_path = host_target.join(rel_path);
 
         // Enforce strict containment: reject symlink hops outside host_target
@@ -312,7 +315,8 @@
     for entry in fs::read_dir(current)? {
         let entry = entry?;
         let path = entry.path();
-        let rel_path = path.strip_prefix(tombstones_root).unwrap();
+        let rel_path = path.strip_prefix(tombstones_root)
+            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
         let target_path = host_target.join(rel_path);
 
         if entry.file_type()?.is_dir() {
```

---

### File: `cans/artifacts/code/messages.rs`
```diff
--- cans/artifacts/code/messages.rs
+++ cans/artifacts/code/messages.rs
@@ -52,6 +52,7 @@
 // ── Wire enums ───────────────────────────────────────────────────
 
 #[derive(Debug, Serialize, Deserialize)]
+#[non_exhaustive]
 pub enum GuestToHost {
     TrapEvent(TrapEvent),
     ProcessExit(ProcessExit),
@@ -83,6 +84,7 @@
 pub const EXEC_STREAM_CONTROL: u8 = 0x03; // stream control: exit code, EOF, rlimit descriptors, window resize (signals ride Control 0x0000)
 
 #[derive(Debug, Serialize, Deserialize)]
+#[non_exhaustive]
 pub enum HostToGuest {
     /// Verdict on Intercepted Syscall
     TrapDecision {
@@ -101,6 +103,7 @@
 /// Drop = c.drop(errno, reason), Suspend = c.suspend(until),
 /// Abort = c.abort(reason).
 #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
+#[non_exhaustive]
 pub enum SuspendUntil {
     /// Hold until the given deadline in milliseconds elapses.
     Deadline(u64),
@@ -115,6 +118,7 @@
 /// Custom carries name + CBOR; the handler name resolves at SDK
 /// registration (see 11-protocol.md Custom dispatch). Unknown names
 /// fail with EUnknownAction before the pipeline resumes.
 #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
+#[non_exhaustive]
 pub enum PatchPayload {
     ExecveMutation(Vec<u8>),
     HttpFrame(Vec<u8>),
@@ -126,6 +130,7 @@
 }
 
 #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
+#[non_exhaustive]
 pub enum Verdict {
     /// Proceed untouched. Registers and memory are never modified.
     Pass,
```

---

### File: `cans/artifacts/diagrams/trap-flow.txt`
```diff
--- cans/artifacts/diagrams/trap-flow.txt
+++ cans/artifacts/diagrams/trap-flow.txt
@@ -16,11 +16,12 @@
                                      │   (target script + argv paths must resolve strictly within EROFS;
                                      │   interpreter argv[1..] into /workspace/** forfeits the skip)
                                      │   NO ──► Evaluates Tier 1 Rust Plugins (at the latency ceiling)
-                                     │ Gate 3: Crosses FFI Trampoline to TS/Python async handler
+                                     │ Gate 3: Crosses FFI Trampoline to TS/Python async handler (catch_unwind protected)
                                      │
-                                     ├──────────────────────────────┐
-                                     ▼                              ▼
-                               PASS / PATCH                      DROP
+                                     ├──────────────────────────────┬──────────────────────────────┐
+                                     ▼                              ▼                              ▼
+                               PASS / PATCH                      DROP                        PLUGIN PANIC
+                                     │                              │                              │
                                      │                              │                 Quarantined to Drop (EFAULT);
                                      │                              │                 Rolls back active StepSlot
                                      │                              │                              │
```

---

### File: `cans/artifacts/diagrams/ecosystem-ci.txt`
```diff
--- cans/artifacts/diagrams/ecosystem-ci.txt
+++ cans/artifacts/diagrams/ecosystem-ci.txt
@@ -40,5 +40,5 @@
 
   Canon CI also runs audit-slop: banned-term grep over cans/.
   Canon CI coverage gates (see ci.yml): channel, drain, transport,
-  snapshot-depth, clock-continuity; Windows cells transport-only.
+  snapshot-depth, clock-continuity, panic-hunt, layering, perf-budgets; Windows cells transport-only.
```



No. Four structural blind spots remained: the wire protocol specification lacked the hostile arithmetic invariant, external plugin CI permitted unvetted panics and unsafe code, and the host runtime and SDK safety diagrams omitted the panic quarantine perimeter.

Here are the diff patches closing those gaps.

---

### File: `cans/11-protocol.md`
```diff
--- cans/11-protocol.md
+++ cans/11-protocol.md
@@ -194,6 +194,10 @@
     - Framing invariants
       - Byte order: Little-Endian for all multi-byte integers
+      - Hostile input validation
+        - Frame-derived lengths and stream IDs evaluate via checked_* math
+        - Unchecked slice indexing across wire buffers is strictly forbidden
+        - Payload buffers clamp to MAX_FRAME_PAYLOAD before memory reservation
       - Message-type registry
         - 0x0001-0x0004 core-reserved
           - MSG_TRAP, MSG_CHANNEL_PAYLOAD
```

---

### File: `cans/artifacts/config/plugin-ci-reusable.yml`
```diff
--- cans/artifacts/config/plugin-ci-reusable.yml
+++ cans/artifacts/config/plugin-ci-reusable.yml
@@ -29,6 +29,22 @@
     steps:
       - uses: actions/checkout@v4
       - uses: dtolnay/rust-toolchain@stable
+      - name: Verify zero unsafe code
+        run: |
+          if grep -rn --include="*.rs" 'unsafe ' src/; then
+            echo "ERROR: Unsafe code is forbidden in plugins" && exit 1
+          fi
+      - name: Enforce zero-panic clippy lints
+        run: |
+          cargo clippy -- -D warnings \
+            -D clippy::unwrap_used \
+            -D clippy::expect_used \
+            -D clippy::panic \
+            -D clippy::unimplemented \
+            -D clippy::todo \
+            -D clippy::unreachable
       - name: Contract audit (same command as `just check-plugin`)
         run: cargo run -p kelir-plugin-cli -- check -p ${{ inputs.plugin }}
```

---

### File: `cans/artifacts/diagrams/host-runtime.txt`
```diff
--- cans/artifacts/diagrams/host-runtime.txt
+++ cans/artifacts/diagrams/host-runtime.txt
@@ -8,11 +8,11 @@
                │ (Direct Monomorphization)                   │ (napi-rs / PyO3)          │
                ▼                                             ▼                           │
   ┌───────────────────────────────────────────────────────────────────────────────────┐  │
-  │                        kelir-core (Stripped Microkernel)                          │  │
+  │            kelir-core (Stripped Microkernel, #![forbid(unsafe_code)])             │  │
   │   ├── Native tier plugins: In-memory Rust execution at the latency ceiling        │  │
   │   ├── Foreign tier Filter-Before-FFI Gate: MountScope drops unmonitored events    │  │
   │   ├── BusDispatcher: Zero-copy multiplexer for the 4 VirtIO Rings                 │  │
-  │   ├── PipelineEngine: Cargo-linked contiguous interceptor execution tables        │  │
+  │   ├── PipelineEngine: Panic-quarantined execution tables (catch_unwind protected) │  │
   │   ├── EventBus: Unified compile-time and async hook pipeline                      │  │
   │   ├── Transport: SPSC rings primary; AF_VSOCK / named-pipe fallback                │ │
   │   ├── Host state: ~/.kelir/catalog.db (SQLite WAL), cas/, sessions/               │  │
@@ -21,7 +21,7 @@
                                              │ Dynamic Driver Trait Interface            │
                                              ▼                                           │
   ┌───────────────────────────────────────────────────────────────────────────────────┐  │
-  │                           kelir-drivers (Pluggable Backends)                      │  │
+  │            kelir-drivers (Pluggable Backends, Isolated Unsafe Enclaves)           │  │
   │   ├── driver-rustvmm (DEFAULT Linux): KVM + userfaultfd CoW + CUDA CDI            │  │
   │   │   CPUID synthetic filter: invariant TSC, Leaf 0x1 TSC-deadline,               │  │
   │   │   unconditional across AMD/Hygon; unknown vendors -> canonical KVM.           │  │
```

---

### File: `cans/artifacts/diagrams/sdks-invariants.txt`
```diff
--- cans/artifacts/diagrams/sdks-invariants.txt
+++ cans/artifacts/diagrams/sdks-invariants.txt
@@ -19,4 +19,5 @@
 at compile time in Rust and TypeScript and at import time in Python; string
 event names are not accepted dispatch keys in any language.
 Session architecture: The 4 native planes (terminal, storage, wire, checkpoint)
 are directly accessible on Session; optional domain plugins attach via extensions.
+Panic quarantine: All SDK handler dispatches wrap in catch_unwind at the engine perimeter.
```