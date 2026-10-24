# WORK-ORDER: Universal Taxonomy Refactor & Metaphor Grounding

**Status**: FINAL / EXECUTION-READY  
**Scope**: Entire `cans/` specification corpus + all `cans/artifacts/` subdirectories  
**Execution Model**: 6 Parallel Agent Streams (Independent write-sets; zero lock contention)

---

## 1. CANONICAL IDENTITY & METAPHOR CONTRACT

The Wayang Kulit metaphor is structural lore and brand identity. It remains canon, but is strictly anchored to operating system physics:

| Metaphor | OS / Hardware Reality | Operational Invariant |
| :--- | :--- | :--- |
| **Kelir** (ꦏꦼꦭꦶꦂ - The Screen) | VirtIO mediation boundary (TAP, FUSE, PTY, Vsock) | The sole physical contact plane between host and guest. |
| **Dalang** (The Puppeteer) | Host runtime supervisor (`kelir-core`, NAPI/PyO3) | Owns hypervisor loops, page tables, and verdict pipelines. |
| **Wayang** (The Puppet) | Sandboxed microVM workload | POSIX guest running unmodified binaries; zero in-guest agent. |
| **Blencong** (The Oil Lamp) | Hardware clocksource & PMU counter | Immutable light source: retired instruction counters, host TSC. |
| **Bayang** (The Shadow) | Observable terminal streams & frame telemetry | Projected output evaluated by Dalang. Guest never sees host. |

**Scope Rule**: Metaphor terms belong **exclusively** in `00-overview.md` and high-level architectural summaries. All internal structs, traits, error types, hooks, and interface files must use exact POSIX, Linux kernel ABI, and VirtIO nomenclature.

---

## 2. UNIVERSAL TAXON SPECIFICATION (RFC 8141)

Every plugin and plane hook MUST declare coordinates using this exact 4-tuple:  
`Coordinate = (URN, Rings, Role, Tier)`

### A. URN Canonical Grammar
```
URN            ::= "urn:kelir:" Subsystem ":" Target [ ":" Subtarget ]
Subsystem      ::= "net" | "fs" | "io" | "sys" | "ipc"
```

1. **`urn:kelir:net`** (Network Boundary)
   * `l2`: Raw Ethernet TAP frames (`virtio-net`)
   * `l4`: Host-side TCP/UDP stream sockets, SOCKS5
   * `l7[:<proto>]`: Protocol termination/proxying (`http`, `dns`, `sql`, `ssh`)
2. **`urn:kelir:fs`** (Storage & Filesystem)
   * `vfs[:<role>]`: Inode/directory operations via FUSE ABI (`virtio-fs`, e.g., `vfs:lock`)
   * `block[:<format>]`: Block storage images (`virtio-blk`, e.g., `block:cas`, `block:oci`)
3. **`urn:kelir:io`** (Character Streams & Terminal)
   * `pty`: Interactive TTY stream, ANSI sanitizing, signals (`virtio-console:hvc0`)
   * `channel[:<adapter>]`: Multiplexed paravirtual stream (`virtio-console:hvc1+`, e.g., `channel:mcp`)
4. **`urn:kelir:sys`** (Kernel, Hardware & Supervisor)
   * `trap[:<subsystem>]`: In-kernel eBPF/seccomp syscall intercepts (e.g., `trap:exec`)
   * `proc[:<target>]`: Process tree, cgroup freezer, page swap (e.g., `proc:memory`, `proc:lifecycle`)
   * `time`: Monotonic TSC offsets, paravirtual clock (`kvm_clock`, `CNTVOFF_EL2`)
   * `fault`: Deterministic fault injection engine
   * `audit`: Immutable hardware receipt logging & Merkle WAL
5. **`urn:kelir:ipc`** (Inter-VM & Host Transport)
   * `vsock[:<subtarget>]`: AF_VSOCK packet and stream multiplexing
   * `shm`: Direct physical memory mapping (`virtio-shm` / `memfd`)

### B. Standard Roles (Closed Set)
* **`Observer`**: Read-only intercept. Zero side effects. Cannot block pipeline. (Replaces `fact-producer`).
* **`Gate`**: Admission control. Returns `Allow` or `Reject { errno, reason }`. Cannot mutate payloads.
* **`Mutator`**: In-flight payload rewrite. Modifies byte buffers, headers, or environment vectors.
* **`Proxy`**: Protocol termination and re-origination at host boundary.

### C. Standard Tiers (Closed Set)
* **`Native`**: Rust in-process host microkernel memory (sub-12µs).
* **`Foreign`**: Host runtime event loops (Node.js/Python via FFI trampoline).

---

## 3. MANDATORY SLOP-TO-SYSTEMS LEXICON

| AI Slop / Vague Buzzword | Standard Engineering Replacement |
| :--- | :--- |
| `Pure epistemic agnosticism` | `Strict hardware/kernel abstraction layer` |
| `Three-tier truth hierarchy` | `Three-tier evidence verification model` |
| `Epistemic role` / `fact-producer` | `Telemetry role` / `Observer` |
| `Cohesive state duality` | `Physical RAM and inode differential diffing` |
| `Token-to-bit lineage` | `Instruction-counter and dirty-byte hardware attribution` |
| `Superpowers` | `Hypervisor primitives` |
| `Radical plugin-first microkernel` | `Modular VirtIO interposition microkernel` |
| `Teleport twin microVMs` | `Hydrate state from streaming snapshot bundle` |
| `Autonomous idle eviction` | `Zero-RSS working-set page compaction and swap` |
| `Zero silent state divergence` | `Mandatory TTY host disclosure invariant` |
| `Ghost inodes` / `GhostFile` | `Synthetic FUSE inodes` / `SyntheticFile` |
| `Eden-style typed vsock RPC` | `Multiplexed vsock Channel 2 binary RPC` |
| `Epistemic Reject` | `Verification Reject` |

---

## 4. PARALLEL AGENT STREAM ALLOCATION

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ Stream 1: Core Architecture & Interface Code                                │
│   Specs: 00-overview.md, 01-architecture.md, 02-plugin-model.md             │
│   Artifacts: code/plugin-trait.rs, code/vmm-driver-trait.rs,                │
│              diagrams/host-runtime.txt, diagrams/guest-vm.txt               │
├─────────────────────────────────────────────────────────────────────────────┤
│ Stream 2: Hardware Planes & Intercepts                                      │
│   Specs: 04-terminal-plane.md, 05-storage-plane.md, 06-wire-plane.md         │
│   Artifacts: code/reconciler.rs, code/wire-protocol.rs,                     │
│              diagrams/terminal-pipeline.txt, diagrams/storage-architecture.txt│
│              diagrams/wire-plane.txt, diagrams/four-ring-bus.txt            │
├─────────────────────────────────────────────────────────────────────────────┤
│ Stream 3: Primitives, Evidence & Telemetry                                  │
│   Specs: 07-lifecycle.md, 08-epistemic.md                                   │
│   Artifacts: code/execution-receipt.rs, code/causal-delta.rs,               │
│              code/messages.rs, code/step-evaluation-loop.ts,                │
│              diagrams/hypervisor-primitives.txt,                            │
│              diagrams/truth-hierarchy.txt, diagrams/lifecycle-bridge.txt,   │
│              diagrams/trap-flow.txt                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│ Stream 4: SDKs, Ingestion, Protocol & Ops                                   │
│   Specs: 03-sdk-architecture.md, 09-protocol.md, 10-adapters.md, 11-ops.md │
│   Artifacts: code/rust-handle-injection.rs, code/typescript-accumulation.ts │
│              code/python-raii-pydantic.py, diagrams/sdks-invariants.txt,    │
│              diagrams/ingestion-adapters.txt, diagrams/target-audience.txt, │
│              diagrams/ecosystem-ci.txt, config/ci.yml, config/justfile,     │
│              config/plugin-ci-reusable.yml, config/project-structure.txt    │
├─────────────────────────────────────────────────────────────────────────────┤
│ Stream 5: Plugin Catalog (15 Files)                                         │
│   Specs: cans/plugins/*.md                                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│ Stream 6: Polyglot Examples & Spec Metadata                                │
│   Artifacts: examples/autonomous-swe-agent.rs,                              │
│              examples/autonomous-swe-agent.ts,                              │
│              examples/autonomous-swe-agent.py,                              │
│              artifacts/README.md                                            │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. DETAILED STREAM INSTRUCTIONS

### STREAM 1: Core Architecture & Interface Code
* **Target Files**:
  * `cans/00-overview.md`
  * `cans/01-architecture.md`
  * `cans/02-plugin-model.md`
  * `cans/artifacts/code/plugin-trait.rs`
  * `cans/artifacts/code/vmm-driver-trait.rs`
  * `cans/artifacts/diagrams/host-runtime.txt`
  * `cans/artifacts/diagrams/guest-vm.txt`
* **Directives**:
  1. `00-overview.md`: Retain the Wayang Kulit / Dalang / Kelir metaphor. Document it as host supervisor vs guest workload. Strip out marketing and product buzzwords.
  2. `01-architecture.md`: Rewrite the 6 foundational invariants into concrete hypervisor boundaries. Rename all mentions of "Ghost inodes" to "Synthetic FUSE inodes".
  3. `02-plugin-model.md`: Rewrite the taxonomy model around the 5 subsystems (`net`, `fs`, `io`, `sys`, `ipc`). Replace `fact-producer` with `Observer`. Replace tiers with `Native` and `Foreign`.
  4. `code/plugin-trait.rs`:
     * Replace `EpistemicRole` with `TelemetryRole { Observer, Gate, Mutator, Proxy }`.
     * Replace `ExecutionTier` with `ExecutionTier { Native, Foreign }`.
     * Refactor `PluginTaxonomy`:
       ```rust
       pub struct PluginTaxonomy {
           pub boundary: &'static str, // urn:kelir:<subsystem>:<target>[:<subtarget>]
           pub rings: &'static [VirtioDevice],
           pub role: TelemetryRole,
           pub tier: ExecutionTier,
           pub mutation_targets: &'static [&'static str],
       }
       ```
     * Rename `host_stable_hash` to `occ_metadata_hash`.
  5. `code/vmm-driver-trait.rs`: Purge slop comments ("CAS Teleport", "superpowers"). Frame methods around raw KVM/HVF/WHP primitives.
  6. `diagrams/host-runtime.txt` & `diagrams/guest-vm.txt`: Update block labels to match native vs foreign tiers and synthetic FUSE inodes.

---

### STREAM 2: Hardware Planes & Intercepts
* **Target Files**:
  * `cans/04-terminal-plane.md`
  * `cans/05-storage-plane.md`
  * `cans/06-wire-plane.md`
  * `cans/artifacts/code/reconciler.rs`
  * `cans/artifacts/code/wire-protocol.rs`
  * `cans/artifacts/diagrams/terminal-pipeline.txt`
  * `cans/artifacts/diagrams/storage-architecture.txt`
  * `cans/artifacts/diagrams/wire-plane.txt`
  * `cans/artifacts/diagrams/four-ring-bus.txt`
* **Directives**:
  1. `04-terminal-plane.md`: Rephrase Tier-1 threat model references to use the **Three-tier evidence verification model**. Keep PTY/hvc0 technical sanitization invariants intact.
  2. `05-storage-plane.md`: Replace "Ghost inodes" with "Synthetic FUSE inodes". Replace "in-flight masking" slop with "sliding-window replacement over FUSE read buffers".
  3. `06-wire-plane.md`: Replace `net.fabric` terms with `TAP L2 bridge` and `L7 forward proxy`. Ground connection parking in TAP buffers and raw virtqueues.
  4. `code/reconciler.rs`: Ensure comments define atomic workspace exchange without using "teleport" or "magical sync".
  5. `code/wire-protocol.rs`: Verify strictly 24-byte FrameHeader, magic byte `0x4B45`, monotonic 64-bit sequence numbers.
  6. Diagrams (`terminal-pipeline.txt`, `storage-architecture.txt`, `wire-plane.txt`, `four-ring-bus.txt`): Replace all occurrences of "Ghost Inodes" with "Synthetic FUSE Inodes". Standardize `hvc0` to PTY Master and `hvc1` to binary transport.

---

### STREAM 3: Primitives, Evidence & Telemetry
* **Target Files**:
  * `cans/07-lifecycle.md`
  * `cans/08-epistemic.md`
  * `cans/artifacts/code/execution-receipt.rs`
  * `cans/artifacts/code/causal-delta.rs`
  * `cans/artifacts/code/messages.rs`
  * `cans/artifacts/code/step-evaluation-loop.ts`
  * `cans/artifacts/diagrams/hypervisor-primitives.txt`
  * `cans/artifacts/diagrams/truth-hierarchy.txt`
  * `cans/artifacts/diagrams/lifecycle-bridge.txt`
  * `cans/artifacts/diagrams/trap-flow.txt`
* **Directives**:
  1. `07-lifecycle.md`: Rename node `Superpowers` to `Hypervisor primitives`. Describe Fork, Rollback, Suspend, and ClockSkew strictly as driver-level operations (`KVM_SET_CLOCK`, `userfaultfd`, `lz4_flex`). Replace "CAS Teleport" with "Content-addressed snapshot sync".
  2. `08-epistemic.md`: Retain filename for slug stability. Rename top-level node and contents to **Verification & Hardware Telemetry**. Refactor **Three-tier evidence verification model**:
     * **Tier 1 (Hardware/Hypervisor Sampling)**: PMU retired instructions, host TSC deltas, dirty-page bitmaps, kernel register state.
     * **Tier 2 (Boundary Paravirtual Intercepts)**: FUSE write journals, TAP L2/L4 frame counters, sanitized PTY stream digests.
     * **Tier 3 (Guest Userland Self-Reporting)**: Unverified harness stdout/stderr, guest status JSON.
     Replace "Cohesive state duality" with "State diff aggregation".
  3. `code/execution-receipt.rs`: Rename `causal_token_id` to `correlation_token`. Explicitly document PMU retired instructions and hypervisor dirty-byte counters.
  4. `code/causal-delta.rs`: Purge "Counterfactual state delta... epistemic orchestration". Define as differential delta (Δ = S_A - S_B) tracking memory, inodes, and wire egress.
  5. `code/messages.rs`: Standardize `TrapAction::Yield` as hypervisor cgroup freeze. Clean up comments on permission lattice.
  6. `code/step-evaluation-loop.ts`: Replace "Epistemic Reject" with "Verification Reject". Replace "Physical Ground Truth" with "Tier 1 Hardware Evidence".
  7. `diagrams/hypervisor-primitives.txt`: Change header to `HYPERVISOR HARDWARE PRIMITIVES`. Replace slop entries:
     * `15. Zero-RSS Evict` ──► `Zero-RSS working-set swap via lz4_flex & io_uring.`
     * `16. CAS Teleport` ──► `Content-addressed block hydration (FastCDC + object_store).`
  8. `diagrams/truth-hierarchy.txt`: Change header to `THE 3-TIER EVIDENCE VERIFICATION MODEL`. Reorganize into Tier 1 (Hardware), Tier 2 (Boundary), Tier 3 (Guest). Replace "The agent cannot fake this" with "Unforgeable outside guest VM memory boundary".
  9. `diagrams/lifecycle-bridge.txt` & `diagrams/trap-flow.txt`: Align terminology with hardware primitives and Gate 1-3 pipelines.

---

### STREAM 4: SDKs, Ingestion, Protocol & Operations
* **Target Files**:
  * `cans/03-sdk-architecture.md`
  * `cans/09-protocol.md`
  * `cans/10-adapters.md`
  * `cans/11-operations.md`
  * `cans/artifacts/code/rust-handle-injection.rs`
  * `cans/artifacts/code/typescript-accumulation.ts`
  * `cans/artifacts/code/python-raii-pydantic.py`
  * `cans/artifacts/diagrams/sdks-invariants.txt`
  * `cans/artifacts/diagrams/ingestion-adapters.txt`
  * `cans/artifacts/diagrams/target-audience.txt`
  * `cans/artifacts/diagrams/ecosystem-ci.txt`
  * `cans/artifacts/config/ci.yml`
  * `cans/artifacts/config/justfile`
  * `cans/artifacts/config/plugin-ci-reusable.yml`
  * `cans/artifacts/config/project-structure.txt`
* **Directives**:
  1. `03-sdk-architecture.md`: Purge "iron rules" and lecture tone. Define explicit type-safety invariants across Rust, TS, and Python. Explain NAPI/PyO3 AOT hook fusion.
  2. `09-protocol.md`: Change "Channel 2: Strongly-typed Eden RPC" to "Channel 2: Multiplexed binary RPC". Standardize wire framing to 24 bytes.
  3. `10-adapters.md`: Clarify process vs plugin boundaries for containerd, MCP, OpenShell, and E2B wrappers. Apply Universal Taxonomy URNs.
  4. `11-operations.md`: Replace "Zero CPU burn" rhetoric with explicit resource constraints (<50KB shim binary, <60ms cold boot, <15s host build).
  5. Code snippets (`rust-handle-injection.rs`, `typescript-accumulation.ts`, `python-raii-pydantic.py`): Update comments and types to match `SyntheticFile` and `Verification Reject`.
  6. `diagrams/target-audience.txt`: Update row 4 to `4. VERIFIERS & AUDIT SYSTEMS (Axiom, RLVR, Formal Falsifiers)`.
  7. `config/ci.yml` & `config/justfile`: Add the `audit-slop` verification target.
  8. `config/project-structure.txt`: Align all inline tree comments with new taxonomy and names.

---

### STREAM 5: Plugin Catalog Canonicalization
* **Target Files**: All 15 files in `cans/plugins/*.md`
* **Directives**:
  Rewrite the `Taxonomy (<name>)` block of every plugin using the exact schema below. Purge all internal descriptions of "teleport", "epistemic", "ghost inodes", and "fact-producer".

| Plugin File | New Canonical Taxonomy Block |
| :--- | :--- |
| `cas-snapshot.md` | `Taxonomy: urn:kelir:fs:block:cas`<br>`coordinate: rings=[virtio-blk, virtio-shm]; role=Observer; tier=Native` |
| `chaos.md` | `Taxonomy: urn:kelir:sys:fault`<br>`coordinate: rings=[virtio-net, virtio-fs]; role=Mutator; tier=Native` |
| `db-proxy.md` | `Taxonomy: urn:kelir:net:l7:sql`<br>`coordinate: rings=[virtio-net, virtio-vsock]; role=Proxy; tier=Native` |
| `hyper-density.md` | `Taxonomy: urn:kelir:sys:proc:memory`<br>`coordinate: rings=[virtio-balloon, virtio-shm]; role=Mutator; tier=Native` |
| `ledger.md` | `Taxonomy: urn:kelir:sys:audit`<br>`coordinate: rings=[virtio-vsock:ch0]; role=Observer; tier=Native` |
| `lock-broker.md` | `Taxonomy: urn:kelir:fs:vfs:lock`<br>`coordinate: rings=[virtio-fs]; role=Gate; tier=Native` |
| `mcp.md` | `Taxonomy: urn:kelir:io:channel:mcp`<br>`coordinate: rings=[virtio-console:hvc1]; role=Mutator; tier=Native` |
| `mesh.md` | `Taxonomy: urn:kelir:ipc:vsock:mesh`<br>`coordinate: rings=[virtio-vsock]; role=Mutator; tier=Native` |
| `oci-translator.md` | `Taxonomy: urn:kelir:fs:block:oci`<br>`coordinate: rings=[virtio-blk]; role=Mutator; tier=Native` |
| `openshell-coordinator.md`| `Taxonomy: urn:kelir:sys:proc:lifecycle`<br>`coordinate: rings=[virtio-vsock]; role=Gate; tier=Native` |
| `pty.md` | `Taxonomy: urn:kelir:io:pty`<br>`coordinate: rings=[virtio-console:hvc0]; role=Mutator; tier=Native` |
| `supply-chain.md` | `Taxonomy: urn:kelir:sys:trap:exec`<br>`coordinate: rings=[virtio-vsock:ch0]; role=Gate; tier=Native` |
| `vfs.md` | `Taxonomy: urn:kelir:fs:vfs`<br>`coordinate: rings=[virtio-fs]; role=Mutator; tier=Native` |
| `wire-ingress.md` | `Taxonomy: urn:kelir:net:l4:ingress`<br>`coordinate: rings=[virtio-vsock:ch1]; role=Proxy; tier=Native` |
| `wire.md` | `Taxonomy: urn:kelir:net:l7:proxy`<br>`coordinate: rings=[virtio-net]; role=Mutator; tier=Native` |

---

### STREAM 6: Polyglot Examples & Spec Metadata
* **Target Files**:
  * `cans/artifacts/examples/autonomous-swe-agent.rs`
  * `cans/artifacts/examples/autonomous-swe-agent.ts`
  * `cans/artifacts/examples/autonomous-swe-agent.py`
  * `cans/artifacts/README.md`
* **Directives**:
  1. Across all 3 examples (`.rs`, `.ts`, `.py`):
     * Change rejection messages from `Epistemic Reject: ...` to `Verification Reject: ...`.
     * Rename ghost file calls to synthetic file primitives (e.g., `SyntheticFile.static_`, `.syntheticFile(...)`).
     * Change comments describing the `Oracle` from "epistemic evaluator" to "post-condition verification engine".
     * In `RepoPolicyPlugin`, declare coordinates as `urn:kelir:sys:trap:exec` with rings `virtio-vsock` and `virtio-fs`.
  2. `cans/artifacts/README.md`:
     * Preserve the SSOT contract (Artifacts own content, Nodes own normativity).
     * Replace "epistemic orchestration" references with "deterministic hypervisor verification".

---

## 6. AUTOMATED TEST & VERIFICATION GATES

Execute this automated suite after all streams complete. Every check must pass green:

```bash
#!/usr/bin/env bash
set -euo pipefail

echo "=== GATE 1: BANNED BUZZWORD SCAN ==="
BANNED_REGEX="pure epistemic|truth tier|fact-producer|ghost inode|GhostFile|cohesive state duality|token-to-bit|superpowers|teleport"
MATCHES=$(git grep -i -E "$BANNED_REGEX" cans/ || true)
if [ -n "$MATCHES" ]; then
  echo "FAIL: Found banned terms:"
  echo "$MATCHES"
  exit 1
fi
echo "PASS: Zero banned terms found."

echo "=== GATE 2: UNIVERSAL URN TAXONOMY SCAN ==="
BAD_URNS=$(git grep "Taxonomy (" cans/plugins/ | grep -v -E "urn:kelir:(net|fs|io|sys|ipc):[a-z0-9]+(:[a-z0-9]+)?" || true)
if [ -n "$BAD_URNS" ]; then
  echo "FAIL: Non-standard URN format detected:"
  echo "$BAD_URNS"
  exit 1
fi
echo "PASS: All 15 plugins comply with RFC 8141 URN schema."

echo "=== GATE 3: DIAGRAM TITLE INTEGRITY ==="
grep -q "THE 3-TIER EVIDENCE VERIFICATION MODEL" cans/artifacts/diagrams/truth-hierarchy.txt
grep -q "HYPERVISOR HARDWARE PRIMITIVES" cans/artifacts/diagrams/hypervisor-primitives.txt
echo "PASS: Diagram headers correctly refactored."

echo "=== GATE 4: CANS SPEC COMPILATION ==="
cans check --refs-only --json | jq -e '.ok and .exitCode == 0'
cans check --json | jq -e '.ok and .exitCode == 0'
echo "PASS: CANS graph compiled cleanly with 0 errors."
```

---

## 7. AGENT EXECUTION PROMPT

When dispatching parallel sub-agents, inject this prompt verbatim:

```text
You are assigned to STREAM <N> of the Kelir Specification Refactor.
Your assigned files are: <TARGET_FILES>.

STRICT OPERATIONAL DIRECTIVES:
1. Obey the Universal Taxonomy Grammar: urn:kelir:<net|fs|io|sys|ipc>:<target>[:<subtarget>]
2. Roles must be one of: Observer, Gate, Mutator, Proxy. Tiers must be: Native, Foreign.
3. Ground the Wayang Kulit metaphor in OS physics (Kelir=Boundary, Dalang=Supervisor, Wayang=Workload, Blencong=PMU/Clock, Bayang=Telemetry). Do not delete the metaphor, but purge all pseudo-philosophical slop.
4. Replace all buzzwords according to Section 3 (Slop-to-Systems Lexicon).
5. Rename "Ghost Inodes" to "Synthetic FUSE Inodes". Rename "Superpowers" to "Hypervisor primitives".
6. Do NOT edit any files outside your assigned stream.
7. Node text must strictly remain within 3-90 characters per cans/_rules.yaml.
```
