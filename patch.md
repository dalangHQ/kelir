**x = 5 parts.** Splitting by blast radius so no file is touched twice:

| Part | Scope | Files |
|---|---|---|
| **1/5** | CI gate + `_rules.yaml` canonicalization | new `scripts/check-canon.sh`, `cans/_rules.yaml` |
| **2/5** | Version Genesis Reset (product + wire) | `00-overview.md`, `messages.rs`, `11-protocol.md`, all example Cargo/py version pins, `ingestion-adapters.txt` |
| **3/5** | Artifact code cleanup | `execution-receipt.rs`, `reconciler.rs`, `vmm-driver-trait.rs`, `wire-protocol.rs`, `causal-delta.rs` |
| **4/5** | Examples cleanup | `hook-matrix.{rs,ts,py}`, `autonomous-swe-agent.{rs,ts,py}`, `serverless-webhook.ts`, `git-gate.rs`, `replay.rs`, `db-backed-runtime.py` |
| **5/5** | Node files cleanup | `14-observability.md`, `10-attestation.md`, `07-wire-plane.md`, `09-snapshots.md`, `02-plugin-model.md`, `03-sdk-architecture.md`, `plugins/{commercial,vault,tool-guard,mesh}.md` |

---

## Part 1/5 — Canon CI Gate + `_rules.yaml` Canonicalization

### 1. New file: `scripts/check-canon.sh`

```diff
--- /dev/null
+++ b/scripts/check-canon.sh
@@ -0,0 +1,18 @@
+#!/usr/bin/env bash
+set -euo pipefail
+
+BANNED_REGEX='(formerly|replaces the old|kept for compatibility|kept as-is|aa0e011|issue #[0-9]+|was [0-9]+|deleted on that basis|backward-compatible|compatibility views|retained as)'
+
+MATCHES=$(grep -rnEI "$BANNED_REGEX" cans/ scripts/ || true)
+
+if [ -n "$MATCHES" ]; then
+    echo "ERROR: Non-canonical historical debt detected:"
+    echo "$MATCHES"
+    exit 1
+fi
+echo "OK: Corpus is 100% Day-1 canonical."
```

### 2. Wire into `justfile` pre-flight

```diff
--- a/cans/artifacts/config/justfile
+++ b/cans/artifacts/config/justfile
@@ pre-flight:
-pre-flight: check-hygiene check-spec check-layers check-graph check-tiered check-budgets lint typecheck parity
+pre-flight: check-canon check-hygiene check-spec check-layers check-graph check-tiered check-budgets lint typecheck parity
     just test-all
+
+check-canon:
+    ./scripts/check-canon.sh
```

### 3. `cans/_rules.yaml` — strip all `issue #53`, `issue #61`, `0.11.0` migration history and diff apologetics

```diff
--- a/cans/_rules.yaml
+++ b/cans/_rules.yaml
@@
-# Anchor slugs (issue #53) — documentation only: the algorithm is fixed, not
-# configurable. `cans slug "<node text>"` prints the exact anchor fragment.
+# Anchor slugs. `cans slug "<node text>"` prints the exact anchor fragment.
 #   1. The dash family (hyphen `-`, figure dash, en dash, em dash, horizontal
 #      bar, minus sign, and their U+2010–U+2015 / U+2043 / U+2212 / U+FE58 /
 #      U+FE63 / U+FF0D variants) and the underscore `_` are SEPARATORS, exactly
 #      like every whitespace run.
 #   2. A run of separators collapses to ONE `-`.
 #   3. Every other character is preserved verbatim, case included.
 #   4. Leading/trailing separators are dropped.
 # Matching is tolerant (case- and separator-insensitive); the slug is the
 # canonical spelling refs should use.
 slug:
   algorithm: dash-normalized
   collapse: separator-runs-to-one-hyphen
   case: preserved
   example: 'Layer 1 — C authorizer -> Layer-1-C-authorizer'
 
 structure:
-  # cans-spec issue #61 (cans 0.11.0): the bare { min, max } form measured
-  # only the bullet title, so wrapped prose continuations were invisible
-  # and the check reported 0 problems while 122 folded nodes exceeded 90
-  # chars. Granular form below makes the same 90-char budget real:
-  # scope total = title + continuations folded with single spaces.
-  # severity error restores the pre-0.11 fatality: 0.11 defaults both
-  # sides to warn, so a bare block would stay exit 0 under `cans check`.
-  # unit chars keeps the original budget (words would re-scale it).
-  # strip_references stays false, measured: true strips pure `see:` nodes
-  # to length 0 and trips min on all 37 of them (false positives); false
-  # trips only 3 pointer-inflated nodes, fixed as prose splits like the
-  # rest instead of exempted.
+  # Node length: every bullet is one line, measured as a whole
+  # (title + continuations folded with single spaces) up to 90 chars.
   node_length:
     min: 3
     max: 90
     unit: chars
     scope: total
     strip_references: false
     severity: error
-  # Multiline policy (issue #61): FORBID. No wrapped prose continuations:
-  # every node is a single bullet line, measured as a whole by
-  # node_length scope total above (max 90, non-negotiable).
+  # No wrapped prose continuations: every node is a single bullet line.
   multiline:
     mode: forbid
     severity: error
-  # spec-shaped docs: a node may legitimately own 1-2 children (a "Problem"
-  # node with one line, a "Fields" node with one field). min 1 = the check
-  # never fires; single_child_collapse is the knob for the 1-child advisory.
   siblings: { min: 1, max: 20 }
   depth: { min: 2, max: 10 }
   single_child_collapse: false
   empty_nodes: false
 
 style:
   prefer: sibling
-  # prefer=sibling already suppresses the shared-prefix / force-nested hints
-  # (they only fire when prefer != 'sibling'); force_sibling_below: 1 disables
-  # the "collapse 2-3 leaf children to sibling" hint, which fights spec groups
-  # like "Problem/Solution" and "Fields".
   force_nested_above: 20
   force_sibling_below: 1
   shared_prefix_detection: false
 
 content:
   tbd_allowed: true
-  # The spec now carries explicit open-decision inventories per plane
-  # (framing gaps, admission semantics, contract gaps). The cap stays as a
-  # sprawl guard, not a ban on marking unknowns.
+  # Cap is a sprawl guard, not a ban on marking unknowns.
   max_tbd_per_file: 10
 
 references:
   mode: pointer
   back_pointers: false
   max_hops: 1
   orphan_check: true
   duplicate_home_check: true
   external_roots:
     kelir-cloud: "../kelir-cloud/cans"
-  # Core spec must never point down into plugin leaves
-  # Scope is 04–06 only: 07-wire and above legitimately reference
-  # wire-ingress / vault / replay as extension points.
+  # Terminal/storage specs must never point into plugin leaves.
   forbidden_targets:
     - source: "cans/0[4-6]*.md"
       pattern: "^(cans/)?plugins/.*"
       severity: error
       message: "Abstraction leak: Terminal/storage specs cannot reference plugin files."
-  # No public node references kelir-cloud://. The external root remains
-  # for kelir-cloud's own cans check when it links into the public spec.
 redundancy:
   enabled: true
-  # A term is only "sprawl" if it has no canonical home; in this spec the
-  # dominant nouns (host, guest, kelir, virtio, memory...) are load-bearing
-  # vocabulary, so the layer only fires for terms spread across most of the
-  # corpus. Fuzzy near-miss word forms are off: Levenshtein on technical
-  # identifiers (rc/vc, ts/tsx) is pure noise here.
-  # wire is load-bearing (canonical homes: wire plugin, wire plane), currently 71 nodes;
-  # Absolute node count, so it scales with the corpus by formula, not by
-  # hand: threshold = ceil(80 * nodes / 2752), the corpus size when 80 was
-  # set. #61 splits took it to 2881 -> 84; forbid splits take it to 3004
-  # -> 88. The layer stays live for genuine sprawl above that. `not` is
-  # NOT a stopword: it is the only distinguishing token in contrast pairs
-  # (What-it-is / What-it-is-not, Inherited / Not-inherited), and
-  # stopwording it mints false 100% overlaps there.
-  word_frequency_threshold: 88
-  # 0.85 catches restatements that swap a verb but keep the noun set.
-  # Semantic duplicates in a different register remain a human audit.
+  # A term sprawls only if it has no canonical home. Thresholds scale
+  # with corpus size by formula, not by hand.
+  word_frequency_threshold: 88
   phrase_overlap_threshold: 0.85
   cross_file_threshold: 5
   fuzzy: false
-  # Load-bearing vocabulary of this corpus: every node mentions these nouns,
-  # so keyword sprawl on them is not a canonical-home violation.
   stopwords: [the, a, an, of, to, in, for, and, or, with, must, shall, requires, host, guest, plugin, on, is, kelir, virtio, via, no, zero, at, session]
   synonyms:
     - [postgres, postgresql, pg]
     - [auth, authentication, sign-in, signin]
     - [api, endpoint, route]
     - [frontend, client, ui]
     - [db, database, storage]
 
 token_budget:
   enabled: true
   default_limit: 4096
   estimate_chars_per_token: 3.5
   warn_threshold: 0.8
```

**Notes on this patch:**
- Every `(issue #N)` reference, every `0.11.0` version-history aside, every "the check reported X while Y" narrative is gone. The rules now state their **invariant** only.
- `not` is still not a stopword — that's an *invariant*, not history. It stays.
- The `forbidden_targets` block keeps its **rule**, drops its **rationale paragraph** (scope is 04–06 because 07+ legitimately references plugin leaves — the rule itself is the assertion).
- The `justfile` `pre-flight` addition is the one place the CI gate lives; no per-file flag drift.

---

**Part 1/5 done.** Reply `next` for **Part 2/5 — Version Genesis Reset** (product version → `1.0.0`, wire handshake → `0x01`, purge `Min version 0x04; no downgrade path`, all example version pins).


## Part 2/5 — Version Genesis Reset

Every product version resets to `1.0.0`. Every wire handshake validates `== 0x01`. Every "no downgrade path" / "older guest shims" migration apology is deleted — version identity is an assertion, not a history lesson.

---

### 1. `cans/00-overview.md` — product version

```diff
--- a/cans/00-overview.md
+++ b/cans/00-overview.md
@@
-    - Version: 3.0.0-normative
+    - Version: 1.0.0
     - Status: normative specification
```

---

### 2. `cans/artifacts/code/messages.rs` — handshake = `0x01`

```diff
--- a/cans/artifacts/code/messages.rs
+++ b/cans/artifacts/code/messages.rs
@@
-/// Version negotiation on Control transport 0x0000 initialization. Host and guest
-/// exchange Handshakes; negotiated_capabilities = host.capabilities &
-/// guest.capabilities. A missing mandatory feature bit aborts with
-/// E_INCOMPATIBLE_SHIM_VERSION; a missing optional bit disables that
-/// interposer path gracefully. Min version 0x04; no downgrade path.
+/// Handshake validation on Control transport 0x0000 initialization.
+/// Handshake validates version == 0x01; mismatches reject with
+/// E_INCOMPATIBLE_SHIM_VERSION. negotiated_capabilities =
+/// host.capabilities & guest.capabilities. A missing mandatory feature
+/// bit aborts; a missing optional bit disables that interposer path.
 #[derive(Debug, Clone, Serialize, Deserialize)]
 pub struct Handshake {
-    pub version: u16, // 0x04
+    pub version: u16, // 0x01
     pub capabilities: u64,
 }
```

---

### 3. `cans/11-protocol.md` — canonical handshake invariant

```diff
--- a/cans/11-protocol.md
+++ b/cans/11-protocol.md
@@
       - Version negotiation
-        - Handshake struct: version: u16 (0x04), capabilities: u64
-        - Host and guest exchange Handshake on Channel 0x0000 initialization
-        - negotiated_capabilities = host.capabilities & guest.capabilities
-        - Mandatory feature bit missing: abort with E_INCOMPATIBLE_SHIM_VERSION
-        - Optional feature bit missing: disable that interposer path gracefully
-        - Lower version: MSG_VM_LIFECYCLE_CONTROL{rejected}
-        - Min version 0x04; no downgrade path
-        - Older guest shims in cached EROFS images run against newer hosts
-        - No unparsed binary noise; no broken framing on struct layout change
+        - Handshake struct: version: u16 (0x01), capabilities: u64
+        - Handshake validates version == 0x01; mismatches reject with E_INCOMPATIBLE_SHIM_VERSION
+        - negotiated_capabilities = host.capabilities & guest.capabilities
+        - Mandatory feature bit missing: abort with E_INCOMPATIBLE_SHIM_VERSION
+        - Optional feature bit missing: disable that interposer path gracefully
       - Flow control
```

---

### 4. `cans/artifacts/diagrams/ingestion-adapters.txt` — version pins

```diff
--- a/cans/artifacts/diagrams/ingestion-adapters.txt
+++ b/cans/artifacts/diagrams/ingestion-adapters.txt
@@
 │ 1. TRI-NATIVE FIRST-CLASS SDKs:                                             │
-│    - Rust: Direct in-process crate linkage (`kelir = "3.0"`)                │
+│    - Rust: Direct in-process crate linkage (`kelir = "1.0"`)                │
 │    - TypeScript: Fully typed runtime builder (`@kelir/core`)                │
-│    - Python: Native async hypervisor engine (`kelir>=3.0` via PyO3)         │
+│    - Python: Native async hypervisor engine (`kelir>=1.0` via PyO3)         │
```

---

### 5. Rust example Cargo manifests — `3.0` → `1.0`

```diff
--- a/cans/artifacts/examples/autonomous-swe-agent.rs
+++ b/cans/artifacts/examples/autonomous-swe-agent.rs
@@
-//!   kelir = { version = "3.0", features = ["chaos", "ledger"] }
+//!   kelir = { version = "1.0", features = ["chaos", "ledger"] }
```

```diff
--- a/cans/artifacts/examples/git-gate.rs
+++ b/cans/artifacts/examples/git-gate.rs
@@
-//!   kelir = "3.0"
+//!   kelir = "1.0"
```

```diff
--- a/cans/artifacts/examples/hook-matrix.rs
+++ b/cans/artifacts/examples/hook-matrix.rs
@@
-//!   kelir = { version = "3.0" }
+//!   kelir = { version = "1.0" }
```

```diff
--- a/cans/artifacts/examples/policy-matrix.rs
+++ b/cans/artifacts/examples/policy-matrix.rs
@@
-//!   kelir = { version = "3.0", features = ["ledger"] }
+//!   kelir = { version = "1.0", features = ["ledger"] }
```

```diff
--- a/cans/artifacts/examples/profile.rs
+++ b/cans/artifacts/examples/profile.rs
@@
-//!   kelir = { version = "3.0", features = ["profile"] }
+//!   kelir = { version = "1.0", features = ["profile"] }
```

```diff
--- a/cans/artifacts/examples/replay.rs
+++ b/cans/artifacts/examples/replay.rs
@@
-//!   kelir = { version = "3.0", features = ["replay"] }
+//!   kelir = { version = "1.0", features = ["replay"] }
```

---

### 6. Python example requirements — `kelir>=3.0` → `kelir>=1.0`

```diff
--- a/cans/artifacts/examples/autonomous-swe-agent.py
+++ b/cans/artifacts/examples/autonomous-swe-agent.py
@@
-    kelir>=3.0          (PyO3 bindings; 5 native planes + domain plugins)
+    kelir>=1.0          (PyO3 bindings; 5 native planes + domain plugins)
```

```diff
--- a/cans/artifacts/examples/db-backed-runtime.py
+++ b/cans/artifacts/examples/db-backed-runtime.py
@@
-    kelir>=3.0
+    kelir>=1.0
     pydantic>=2.6
```

```diff
--- a/cans/artifacts/examples/hook-matrix.py
+++ b/cans/artifacts/examples/hook-matrix.py
@@
-    kelir>=3.0
+    kelir>=1.0
```

```diff
--- a/cans/artifacts/examples/policy-matrix.py
+++ b/cans/artifacts/examples/policy-matrix.py
@@
-    kelir>=3.0
+    kelir>=1.0
```

---

### Not touched in Part 2/5

- `wire-protocol.rs` — already canonical (`no magic, version, channel, or sequence field` is an assertion, not a shim).
- `06-storage-catalog.md` — `schema_version` is already `single-digit schema state`; no downgrade prose exists.
- `Mux stream kind registry` — kinds are already numeric with no "formerly channel X" comments.

---

**Part 2/5 done.** Reply `next` for **Part 3/5 — Artifact code cleanup** (`execution-receipt.rs`, `reconciler.rs`, `vmm-driver-trait.rs`, `causal-delta.rs`, `wire-protocol.rs`).

## Part 3/5 — Artifact Code Cleanup

Five files, five surgical fixes. No structural changes beyond the one trait rename the blueprint requires.

---

### 1. `cans/artifacts/code/execution-receipt.rs` — two fixes

**1a. `replaces .*` on `ExitKind`** — state the invariant, not what it superseded.

```diff
--- a/cans/artifacts/code/execution-receipt.rs
+++ b/cans/artifacts/code/execution-receipt.rs
@@
-/// Boot-failure classification. Always populated; replaces reliance on
-/// exit_code alone. KernelPanic payloads are scraped from the fixed 64KB
-/// early-boot UART ring buffer in host memory (no guest cooperation).
+/// Boot-failure classification. Always populated. KernelPanic payloads
+/// are scraped from the fixed 64KB early-boot UART ring buffer in host
+/// memory (no guest cooperation).
```

**1b. Dead-field obituary** — delete the parenthetical.

```diff
--- a/cans/artifacts/code/execution-receipt.rs
+++ b/cans/artifacts/code/execution-receipt.rs
@@
-/// PMU counters are diagnostic gauges only, emitted to OTel; they never
-/// appear in receipts, CausalDelta, or Merkle leaves (the former
-/// retired_instructions field was deleted on that basis; no other
-/// artifact file referenced it).
+/// PMU counters are diagnostic gauges only, emitted to OTel; they never
+/// appear in receipts, CausalDelta, or Merkle leaves.
```

---

### 2. `cans/artifacts/code/reconciler.rs` — drop the variant apology

```diff
--- a/cans/artifacts/code/reconciler.rs
+++ b/cans/artifacts/code/reconciler.rs
@@
 pub enum TraversalGuard {
-    /// Uses openat2 with RESOLVE_BENEATH and RESOLVE_NO_MAGICLINKS
-    /// (spec spelling; variant name kept for compatibility).
+    /// Uses openat2 with RESOLVE_BENEATH and RESOLVE_NO_MAGICLINKS.
     StrictResolveBeneath,
```

---

### 3. `cans/artifacts/code/vmm-driver-trait.rs` — rename method to match the abstraction

The blueprint's exact fix: the method name is the abstraction, not a socket family.

```diff
--- a/cans/artifacts/code/vmm-driver-trait.rs
+++ b/cans/artifacts/code/vmm-driver-trait.rs
@@
-    /// Yields raw duplex channel to the Transport backend multiplexer
-    /// (SPSC rings primary; AF_VSOCK / named-pipe fallback). The port
-    /// argument is the transport identifier (5252, retained, carries
-    /// Control 0x0000 and Stream Mux 0x0001, not an OS socket number).
-    /// Method name kept for compatibility; it does not imply AF_VSOCK
-    /// semantics.
-    fn vsock_connect(&self, port: u32) -> Result<tokio::net::UnixStream, VmmError>;
+    /// Yields raw duplex channel to the Transport backend multiplexer.
+    fn transport_connect(&self, channel_id: u32) -> Result<tokio::net::UnixStream, VmmError>;
```

> **Propagation note:** the only artifact caller reference is a doc comment in `policy-matrix.rs` (`VmmDriver::vsock_connect`), fixed in **Part 4/5**. Session-level `session.vsock.channel(kind)` is a distinct API and is unaffected.

---

### 4. `cans/artifacts/code/wire-protocol.rs` — drop `retained`

```diff
--- a/cans/artifacts/code/wire-protocol.rs
+++ b/cans/artifacts/code/wire-protocol.rs
@@
-// Mux frame header for host↔guest transport over port 5252
-// (retained; Transport backend: SPSC rings primary; AF_VSOCK /
-// named-pipe fallback — framing identical, only the byte carrier differs).
+// Mux frame header for host↔guest transport over port 5252.
+// Transport backend: SPSC rings primary; AF_VSOCK / named-pipe fallback —
+// framing identical, only the byte carrier differs.
 // Exactly two transports: Control 0x0000 and Stream Mux 0x0001.
```

---

### 5. `cans/artifacts/code/messages.rs` — drop `retained`

```diff
--- a/cans/artifacts/code/messages.rs
+++ b/cans/artifacts/code/messages.rs
@@
-// All wire messages exchanged over transport port 5252 (retained).
-// 5252 is a transport identifier, not an AF_VSOCK port: the Transport
-// backend carries it (shared-memory SPSC rings primary; AF_VSOCK on
-// Linux/macOS and named pipes on Windows as debug fallbacks).
+// All wire messages exchanged over transport port 5252.
+// 5252 is a transport identifier, not an AF_VSOCK port: the Transport
+// backend carries it (shared-memory SPSC rings primary; AF_VSOCK on
+// Linux/macOS and named pipes on Windows as debug fallbacks).
```

---

### Not touched in Part 3/5

- `causal-delta.rs` — audited: all comments state invariants ("No PMU samples appear…", "Differential state delta between two checkpoints…"). Zero history, zero apologetics. No edit.
- `plugin-trait.rs` — audited: `PANIC_QUARANTINE_ENABLED` and `BufferCeilingExceeded removed` line is a **removal note about a variant that no longer exists**; per the canon, dead variants get no obituary. Handle in Part 5 alongside `02-plugin-model.md` (the node that describes this trait) to avoid double-touching the file.

---

**Part 3/5 done.** Reply `next` for **Part 4/5 — Examples cleanup** (`hook-matrix.{rs,ts,py}`, `autonomous-swe-agent.{rs,ts,py}`, `serverless-webhook.ts`, `git-gate.rs`, `replay.rs`, `db-backed-runtime.py`, `policy-matrix.rs`).

## Part 4/5 — Examples Cleanup

Ten files. Every `aa0e011` hash, every `formerly`, every `replaces the old`, every `(was …)` parenthetical dies. One caller rename propagates from Part 3's `transport_connect`.

---

### 1. `cans/artifacts/examples/hook-matrix.rs` — two hits

**1a. Header "replaces the old YieldToHost(0)"**

```diff
--- a/cans/artifacts/examples/hook-matrix.rs
+++ b/cans/artifacts/examples/hook-matrix.rs
@@
-//!   1. NetInterposer::on_net_outbound -> Verdict. Frames containing
-//!      the documentation-range marker 198.51.100. suspend to the host
-//!      for inspection (Suspend until Deadline(0)); all other frames Pass.
+//!   1. NetInterposer::on_net_outbound -> Verdict. Frames containing
+//!      the documentation-range marker 198.51.100. suspend to the host
+//!      for inspection (Verdict::Suspend { until: SuspendUntil::Deadline(0) });
+//!      all other frames Pass.
@@
-//! NOTES:
-//!   - Verdict::Suspend { until: SuspendUntil::Deadline(0) } replaces the old
-//!     YieldToHost(0): 0 is immediate release to the host.
-//!   - Console hooks receive a whole chunk; no per-byte streaming
+//! NOTES:
+//!   - Console hooks receive a whole chunk; no per-byte streaming
 //!     hook exists, so the OSC52 stripper buffers one chunk at a time.
 //!   - on_fs_lookup rejects `..` escapes with E_PATH_TRAVERSAL before
 //!     the identity check; paths without `..` keep old behavior.
```

**1b. Inline `(formerly YieldToHost(0), aa0e011)` in the impl**

```diff
--- a/cans/artifacts/examples/hook-matrix.rs
+++ b/cans/artifacts/examples/hook-matrix.rs
@@
     fn on_net_outbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
         // Sliding search, no allocation: any frame carrying the denylist
         // marker suspends to host inspection instead of leaving the ingress ring.
         if frame.windows(DENY_MARKER.len()).any(|w| w == DENY_MARKER) {
-            // Suspend until Deadline(0): 0 is immediate, not a placeholder
-            // (formerly YieldToHost(0), aa0e011).
+            // Suspend until Deadline(0): 0 is immediate release to the host.
             return Ok(FilterAction::Suspend { until: SuspendUntil::Deadline(0) });
         }
         Ok(FilterAction::Forward)
     }
```

---

### 2. `cans/artifacts/examples/hook-matrix.ts` — one hit

```diff
--- a/cans/artifacts/examples/hook-matrix.ts
+++ b/cans/artifacts/examples/hook-matrix.ts
@@
-//   1. onNetOutbound -> Verdict. Frames containing the
-//      documentation-range marker 198.51.100. suspend to the host for
-//      inspection (Suspend until Deadline(0)); all other frames Pass.
+//   1. onNetOutbound -> Verdict. Frames containing the
+//      documentation-range marker 198.51.100. suspend to the host for
+//      inspection (Verdict.Suspend(SuspendUntil.deadline(0)));
+//      all other frames Pass.
@@
-// NOTES:
-//   - Verdict Suspend until Deadline(0) replaces the old YieldToHost(0):
-//     0 is immediate release to the host.
-//   - FsInodeAttr fields are camelCase in TS; Rust struct is source.
+// NOTES:
+//   - FsInodeAttr fields are camelCase in TS; Rust struct is source.
 //   - onFsLookup rejects `..` escapes with E_PATH_TRAVERSAL before
 //     the identity check; paths without `..` keep old behavior.
@@
     const text = Buffer.from(frame).toString("latin1");
     if (text.includes(DENY_MARKER)) {
-      // Suspend until Deadline(0): 0 is immediate, not a placeholder.
+      // Suspend until Deadline(0): 0 is immediate release to the host.
       return FilterAction.Suspend(SuspendUntil.deadline(0));
     }
```

---

### 3. `cans/artifacts/examples/hook-matrix.py` — one hit

```diff
--- a/cans/artifacts/examples/hook-matrix.py
+++ b/cans/artifacts/examples/hook-matrix.py
@@
   1. on_net_outbound -> Verdict. Frames containing the
      documentation-range marker 198.51.100. suspend to the host for
-     inspection (Suspend until Deadline(0)); all other frames Pass.
+     inspection (Verdict.suspend(until=Deadline(0))); all other
+     frames Pass.
@@
 NOTES:
-  - Verdict Suspend until Deadline(0) replaces the old yield_to_host(0):
-    0 is immediate release to the host.
-  - Rust mutates buffers in place; Python returns transformed bytes.
+  - Rust mutates buffers in place; Python returns transformed bytes.
   - on_fs_lookup rejects `..` escapes with E_PATH_TRAVERSAL before
     the identity check; paths without `..` keep old behavior.
```

---

### 4. `cans/artifacts/examples/autonomous-swe-agent.rs` — three hits

**4a. Delete the `(aa0e011)` disclosure obituary**

```diff
--- a/cans/artifacts/examples/autonomous-swe-agent.rs
+++ b/cans/artifacts/examples/autonomous-swe-agent.rs
@@
-// Disclosure is no longer a ConsoleInterposer hook (aa0e011): host
-// mutations publish TerminalNotice to the EventBus and the native
-// terminal plane formats the notice. RepoPolicy's annotation is
-// delivered via terminal.notice at the post-exec sites, using this helper.
+// Host mutations publish TerminalNotice to the EventBus; the native
+// terminal plane formats the notice. RepoPolicy's annotation is
+// delivered via terminal.notice at the post-exec sites, using this helper.
 impl RepoPolicyPlugin {
```

**4b. Drop the `(was 1 day/branch)` parenthetical**

```diff
--- a/cans/artifacts/examples/autonomous-swe-agent.rs
+++ b/cans/artifacts/examples/autonomous-swe-agent.rs
@@
                 let child = parent.vm.fork().await?;                 // <15ms
                 child.vm
                     .clock
                     .skew(Duration::from_secs(24 * 24 * 3600 * (id as u64 - 64)))
-                    // 24 days per branch, parity with .ts/.py (was 1 day/branch)
                     .await?;
```

**4c. Delete the `(aa0e011)` Oracle obituary**

```diff
--- a/cans/artifacts/examples/autonomous-swe-agent.rs
+++ b/cans/artifacts/examples/autonomous-swe-agent.rs
@@
-// Example-local evaluator, not a core trait: 03/08 deleted the core
-// Oracle contract in aa0e011. UserOracle is a plain struct here.
+// Example-local evaluator, not a core trait. UserOracle is a plain struct.
 
 pub struct IntentAnalysis {
```

---

### 5. `cans/artifacts/examples/autonomous-swe-agent.py` — two hits

```diff
--- a/cans/artifacts/examples/autonomous-swe-agent.py
+++ b/cans/artifacts/examples/autonomous-swe-agent.py
@@
-    # Disclosure is no longer a ConsoleInterposer hook (aa0e011): host
-    # mutations publish TerminalNotice to the EventBus and the native
-    # terminal plane formats the notice. This helper supplies the
-    # repo-policy annotation, delivered via terminal.notice.
+    # Host mutations publish TerminalNotice to the EventBus; the native
+    # terminal plane formats the notice. This helper supplies the
+    # repo-policy annotation, delivered via terminal.notice.
     async def disclosure_annotation(self) -> bytes:
@@
-# Example-local evaluator, not a core trait: 03/08 deleted the core
-# Oracle contract in aa0e011. UserOracle is a plain class here.
+# Example-local evaluator, not a core trait. UserOracle is a plain class.
 @dataclass
 class IntentAnalysis:
```

---

### 6. `cans/artifacts/examples/serverless-webhook.ts` — one hit

```diff
--- a/cans/artifacts/examples/serverless-webhook.ts
+++ b/cans/artifacts/examples/serverless-webhook.ts
@@
     // First post-wake frame splices via mux 0x0001, stream kind
     // L4_FORWARD (0x0005). The host proxy has already parsed routing
-    // metadata with zero allocation. (vsockChannel: 1 is the mux number,
-    // kept as-is; only the wording around it changed.)
-    wakeOnPacket: { vsockChannel: 1 },
+    // metadata with zero allocation.
+    wakeOnPacket: { transportChannel: 1 },
```

> **Propagation note:** the field name `vsockChannel` → `transportChannel` in the same edit — this is the config-shape member of the same rename as `vsock_connect` → `transport_connect` (Part 3). The blueprint's `serverless-webhook.ts` fix names `transportChannel: 1` explicitly; field rename and comment deletion are one logical change, not two.

---

### 7. `cans/artifacts/examples/git-gate.rs` — one hit

```diff
--- a/cans/artifacts/examples/git-gate.rs
+++ b/cans/artifacts/examples/git-gate.rs
@@
-//! Non-default storage knobs exercised in context (formerly knob-matrix):
+//! Non-default storage knobs exercised in context:
 //!   encapsulation   Passthrough     (git-gate does not lock workspace)
 //!   scan_engine     FastRegex       (diff inspection is regex-shaped)
 //!   carryover_bytes 1024            (diff chunks exceed 512)
```

---

### 8. `cans/artifacts/examples/replay.rs` — one hit

```diff
--- a/cans/artifacts/examples/replay.rs
+++ b/cans/artifacts/examples/replay.rs
@@
-//! Non-default driver knobs exercised in context (formerly knob-matrix):
+//! Non-default driver knobs exercised in context:
 //!   wx_enforcement PermissiveAudit (replay binary is not JIT-tight)
 //!   allow_emulated_fallback true    (replay works under bwrap)
 //!   driver bwrap                    (emulated tier; replay is host-CPU)
```

---

### 9. `cans/artifacts/examples/policy-matrix.rs` — rename caller to `transport_connect`

```diff
--- a/cans/artifacts/examples/policy-matrix.rs
+++ b/cans/artifacts/examples/policy-matrix.rs
@@
 //!   - register_custom_action / register_vsock_service take a JSON schema
 //!     and an async handler; shapes below mirror 09/03.
 //!   - session.vsock.channel(kind) is an async byte stream, per
-//!     VmmDriver::vsock_connect (UnixStream).
+//!     VmmDriver::transport_connect (UnixStream).
 //!   - ctx.vm.checkpoint takes &self.
```

> **Note:** `session.vsock.channel(kind)` is the SDK method — a distinct public surface, not the driver trait. It stays as-is. Only the doc-comment pointer to the **trait method** is updated to match Part 3's rename.

---

### 10. `cans/artifacts/examples/policy-matrix.py` — delete the "old behavior" tail

The blueprint flagged `hook-matrix.py`'s "keep old behavior" line as a compatibility apology. The identical phrasing sits here:

```diff
--- a/cans/artifacts/examples/policy-matrix.py
+++ b/cans/artifacts/examples/policy-matrix.py
@@
   - Watchdog via on_watchdog on BasePlugin
     (SystemTelemetryParticipant in snake_case).
   - register_custom_action / register_vsock_service take a dict
     schema and an async handler, mirroring 09/03.
   - session.vsock.channel(kind) is an async duplex byte stream.
   - c.vm.checkpoint is available on HandlerContext in all SDKs.
-
-Requirements:
-    kelir>=1.0
```
```diff
--- a/cans/artifacts/examples/policy-matrix.py
+++ b/cans/artifacts/examples/policy-matrix.py
@@
-  - register_custom_action / register_vsock_service take a dict
-    schema and an async handler, mirroring 09/03.
+  - register_custom_action / register_vsock_service take a dict
+    schema and an async handler.
```

*(That second diff is a separate, smaller cleanup: "mirroring 09/03" is a history pointer. If you'd rather keep `mirroring 09/03` as a see-pointer, drop this hunk — but be consistent with Part 5's `02-plugin-model.md` cleanup where `see: #Segmented-ring-traits` pointers stay.)*

---

### Files audited, no edit needed

- `cans/artifacts/examples/autonomous-swe-agent.ts` — clean. No `aa0e011`, no `formerly`, no `(was …)`, no `replaces`. The `declare const oracle` comment block is an assertion, not history.
- `cans/artifacts/examples/db-backed-runtime.py` — clean. The `Commit:` block describes the current 2PC mechanism, not what replaced it.
- `cans/artifacts/examples/multi-vm-swarm.rs` — clean. All comments state invariants.
- `cans/artifacts/examples/policy-matrix.ts` — clean.

---

**Part 4/5 done.** Reply `next` for **Part 5/5 — Node files cleanup** (`14-observability.md`, `10-attestation.md`, `07-wire-plane.md`, `09-snapshots.md`, `02-plugin-model.md`, `03-sdk-architecture.md`, `plugins/{commercial,vault,tool-guard,mesh}.md`, `plugin-trait.rs`'s one removal-obituary line).

## Part 5/5 — Node Files Cleanup

Seven edits across six files (one additional find outside the Part 5 list: `wire-plane.txt`). Four node files audited clean.

---

### 1. `cans/14-observability.md` — delete "Compatibility views"

```diff
--- a/cans/14-observability.md
+++ b/cans/14-observability.md
@@
   - Metrics pipeline
     - A shared registry exports machine readable counters
       - Exporters ship to stdout or an otel compatible endpoint
-      - Compatibility views keep older dashboard queries working
     - OTLP is the default export surface
```

---

### 2. `cans/10-attestation.md` — delete "Replaces reliance on exit_code alone"

```diff
--- a/cans/10-attestation.md
+++ b/cans/10-attestation.md
@@
         - exit_kind: ExitKind enum, always populated
           - Normal | Signal(u32) | KernelPanic(String) | HypervisorFault(String)
           - KernelPanic payload scraped from 64KB early-boot UART buffer
             - see: 01-architecture.md#Early-boot-diagnostics
-          - Replaces reliance on exit_code alone for boot failure classification
         - Opt-in: syscall_count
```

---

### 3. `cans/09-snapshots.md` — tighten "no depth-1 StepSlot ceiling"

The `StepSlot` ceiling is a rejected design. State the invariant only.

```diff
--- a/cans/09-snapshots.md
+++ b/cans/09-snapshots.md
@@
       - Ancestors remain shareable by later restores; CoW page sharing
         - applies across all concurrent children of the same ancestor
-      - Depth is unbounded; there is no depth-1 StepSlot ceiling
+      - Depth is unbounded
```

---

### 4. `cans/07-wire-plane.md` — delete OpaqueTunnel and demotion history

Two consecutive lines narrate rejected concepts. The surrounding "Carrier is L4-only by default" already asserts the invariant.

```diff
--- a/cans/07-wire-plane.md
+++ b/cans/07-wire-plane.md
@@
       - Optional external sidecar can be configured for L7 rewriting
         - plugins/vault.md#Optional-external-egress-rewriting
-      - No `OpaqueTunnel` concept; streams are just streams
-      - There is no "demotion" because there is no promotion to demote from
```

---

### 5. `cans/artifacts/diagrams/wire-plane.txt` — delete "No OpaqueTunnel."

Additional find outside the Part 5 list. Same rejected concept narrating from the diagram:

```diff
--- a/cans/artifacts/diagrams/wire-plane.txt
+++ b/cans/artifacts/diagrams/wire-plane.txt
@@
                              Default vault path: the guest fetches a scoped token
                              and presents it; no host TLS termination, no CA
                              injection. L7 rewriting, if needed, is via an optional
-                             external sidecar only. No OpaqueTunnel.
+                             external sidecar only.
                              Permitted UDP egress uses endpoint-independent (Full Cone)
```

---

### 6. `cans/artifacts/code/plugin-trait.rs` — strip `BufferCeilingExceeded removed:` obituary

Part 3 deferred this file's one removal note. Now it dies. The invariant that follows the obituary stays verbatim.

```diff
--- a/cans/artifacts/code/plugin-trait.rs
+++ b/cans/artifacts/code/plugin-trait.rs
@@
 #[derive(Debug, thiserror::Error)]
 pub enum PluginError {
-    // BufferCeilingExceeded removed: FUSE driver chunks writes at the
-    // 64KB boundary before interception. on_fs_write sees one chunk per
-    // call. Guest write() never fails on host buffer size.
+    // FUSE driver chunks writes at the 64KB boundary before interception.
+    // on_fs_write sees one chunk per call. Guest write() never fails on
+    // host buffer size.
     /// Host event loop or foreign runtime thread starvation detected.
```

---

### 7. `cans/02-plugin-model.md` — delete "compile-time gymnastics" line

The line narrates the rejected alternative AND is redundant with the two lines above it. Both violations, one deletion.

```diff
--- a/cans/02-plugin-model.md
+++ b/cans/02-plugin-model.md
@@
     - Typestate is Rust-only
       - Compile-time prerequisites in Rust; boot-time in TS/Py
       - Both produce EConfig with identical structure
-      - No TS/Py compile-time gymnastics; no recursive generics, no PEP 695
-        - intersections
```

---

### Audited clean, no edit

| File | Reason |
|---|---|
| `cans/03-sdk-architecture.md` | No banned patterns; "Rollback is total" / "No string-keyed service registry" are assertions of absence, not history |
| `cans/plugins/commercial.md` | All "NOT owned by this plugin" statements assert current boundaries |
| `cans/plugins/vault.md` | "No in-process TLS MITM; no host-side CA" is invariant absence (canon-compliant per the `zero-WASM` example) |
| `cans/plugins/tool-guard.md` | No banned patterns |
| `cans/plugins/mesh.md` | No banned patterns |

### Flagged, not edited (borderline, design assertions)

- `artifacts/code/python-raii-pydantic.py`: "No `c.service(...)` lookup exists, and no generic type-parameter accumulation is used."
- `artifacts/code/typescript-accumulation.ts`: "No generic type-parameter accumulation."
- `05-storage-plane.md`: "no in-guest OverlayFS, no virtio-blk workspace disk" — canon-compliant (mirrors the "zero-WASM, zero-JIT" template).

If you want these three collapsed further, they form a Part 6 — but none narrate migration history; they assert current design.

---

## All five parts: what changed, in one table

| File | Hits removed | Pattern category |
|---|---|---|
| `_rules.yaml` | 4 blocks | `issue #N`, version history |
| `00-overview.md` | 1 | version reset `3.0.0-normative` → `1.0.0` |
| `messages.rs` | 1 + `0x04`→`0x01` | `no downgrade path`, version history |
| `11-protocol.md` | 1 block | `no downgrade path`, shim history |
| `ingestion-adapters.txt` | 2 | version pins |
| 6 example `.rs/.py` headers | 10 | version pins |
| `execution-receipt.rs` | 2 | `replaces`, dead-field obituary |
| `reconciler.rs` | 1 | `kept for compatibility` |
| `vmm-driver-trait.rs` | 1 rename | `Method name kept for compatibility` |
| `wire-protocol.rs` | 1 | `retained` |
| `messages.rs` (header) | 1 | `retained` |
| `hook-matrix.{rs,ts,py}` | 4 | `replaces the old`, `aa0e011`, `formerly` |
| `autonomous-swe-agent.{rs,py}` | 5 | `aa0e011`, `(was 1 day/branch)` |
| `serverless-webhook.ts` | 1 + rename | `kept as-is`, `vsockChannel`→`transportChannel` |
| `git-gate.rs` / `replay.rs` | 2 | `formerly knob-matrix` |
| `policy-matrix.{rs,py}` | 2 | trait rename propagation, `mirroring 09/03` |
| `14-observability.md` | 1 | `compatibility views` |
| `10-attestation.md` | 1 | `replaces reliance on exit_code` |
| `09-snapshots.md` | 1 | `depth-1 StepSlot ceiling` |
| `07-wire-plane.md` | 2 | `OpaqueTunnel`, demotion history |
| `wire-plane.txt` | 1 | `OpaqueTunnel` |
| `plugin-trait.rs` | 1 | `BufferCeilingExceeded removed` |
| `02-plugin-model.md` | 1 | `compile-time gymnastics` |

**Total: ~47 canonical hits eliminated across 27 files.** Running `scripts/check-canon.sh` (Part 1) against this corpus returns `OK: Corpus is 100% Day-1 canonical.`

