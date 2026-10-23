# WORK ORDER: Radical CANS Spec Restructuring (Plugin-First Decoupling)

**Target:** Decouple `cans/` from monolithic planes into a stripped microkernel bus specification + 15 isolated first-class plugin specifications in `cans/plugins/*.md`.  
**Execution:** Zero merge conflicts. Two-wave parallel dispatch. Rigid verification gate.

---

## File Topology Blueprint

```
CORE SUBSTRATE (Mechanism Only)        FIRST-CLASS PLUGINS (Policy Only)
cans/                                   cans/plugins/
├── 00-overview.md                      ├── pty.md
├── 01-architecture.md                  ├── vfs.md
├── 02-plugin-model.md (Master Matrix)  ├── wire.md
├── 03-sdk-architecture.md              ├── chaos.md
├── 04-terminal-plane.md                ├── ledger.md
├── 05-storage-plane.md                 ├── db-proxy.md
├── 06-wire-plane.md                    ├── mesh.md
├── 07-lifecycle.md                     ├── lock-broker.md
├── 08-epistemic.md                     ├── supply-chain.md
├── 10-adapters.md                      ├── hyper-density.md
└── 11-operations.md                    ├── cas-snapshot.md
                                        ├── wire-ingress.md
                                        ├── mcp.md
                                        ├── oci-translator.md
                                        └── openshell-coordinator.md
```

---

## Wave 1: Parallel Domain Carve-Out (5 Agents)

All 5 agents run concurrently on isolated, non-overlapping file sets.

### Agent 1: Terminal Plane & PTY
* **Can Edit:** `cans/04-terminal-plane.md`, `cans/plugins/pty.md` (create).
* **Must Not Edit:** Any other file.
* **Tasks:**
  1. Create `cans/plugins/pty.md`.
  2. **CUT** from `04-terminal-plane.md` and **MOVE** to `cans/plugins/pty.md`:
     - ANSI Sanitizer (OSC 52 clipboard, OSC 0/1/2 title stripping).
     - Prompt Responder (`onPrompt`, foreground verification, auto-keystroke rules).
     - Token Shield (clamping ceilings, UTF-8 code point boundary safety, CSI reset).
     - Active Leak Detection (Aho-Corasick scanning on terminal byte streams).
     - Screen Matrix Export (Headless VT100 parser grid state machine).
     - Context key definition: `c.term`.
  3. Strip `04-terminal-plane.md` down to **virtio-console bus mechanics only**:
     - Raw PTY driver `/dev/hvc0 -> /dev/pts/N` initialization.
     - Byte streaming over VirtIO shared memory ring buffers.
     - Live resize handshake (`TIOCSWINSZ` ioctl over `vsock:5252`).
     - Raw signal propagation (Ctrl+C / Ctrl+D).
     - Core-originated disclosure wire transport (`HostToGuest::Disclosure`).
  4. Point `cans/plugins/pty.md` to `04-terminal-plane.md#Terminal-pipeline` via 1-hop `see:`.

---

### Agent 2: Storage Plane, VFS, & Locks
* **Can Edit:** `cans/05-storage-plane.md`, `cans/plugins/vfs.md` (create), `cans/plugins/lock-broker.md` (create).
* **Must Not Edit:** Any other file.
* **Tasks:**
  1. Create `cans/plugins/vfs.md` and `cans/plugins/lock-broker.md`.
  2. **CUT** from `05-storage-plane.md` and **MOVE** to `cans/plugins/vfs.md`:
     - Reconciler (`session.fs.reconcile()`, whiteout translation: `c 0 0` -> unlink, `trusted.overlay.opaque` -> rmdir).
     - Ghost Inodes (static configurations, dynamic JWT generators, `0o400` RAM-only).
     - In-Flight Read Masking (`maskRead`, fixed-width padding, format-preserving generators).
     - UID/GID ID Mapping (`geteuid` host mapping).
     - Shared host cache mounts (`~/.npm`, `~/.cargo/registry`).
     - Snoop Plane harness write intercepts.
     - Context key: `c.fs`.
  3. **CUT** from `05-storage-plane.md` and **MOVE** to `cans/plugins/lock-broker.md`:
     - Entity lock broker (FUSE `flock`/`fcntl` translation to host advisory locks).
     - Context key: `c.locks`.
  4. Strip `05-storage-plane.md` down to **virtio-fs bus mechanics only**:
     - Read-only EROFS block device cold-mount (<2ms).
     - Overlayfs lower/upper RAM tmpfs mount topology.
     - Raw FUSE write hook signature and errno rejection return contract.
     - Secondary image mount mechanics.
     - OCC 64-bit metadata inode hashing algorithm.

---

### Agent 3: Wire Plane, Network, & Proxies
* **Can Edit:** 
  - `cans/06-wire-plane.md`
  - `cans/plugins/wire.md` (create)
  - `cans/plugins/chaos.md` (create)
  - `cans/plugins/db-proxy.md` (create)
  - `cans/plugins/mesh.md` (create)
  - `cans/plugins/wire-ingress.md` (create)
* **Must Not Edit:** Any other file.
* **Tasks:**
  1. Create all 5 plugin files in `cans/plugins/`.
  2. **CUT** from `06-wire-plane.md` and **MOVE**:
     - To `wire.md`: L7 credential swapper (`vault://` URNs), per-session CA generator, response header scrubber, DNS Sinkhole (NXDOMAIN on UDP 53), SSH Gate (SSH-Agent bridge). Context: `c.wire`.
     - To `chaos.md`: Network delay/jitter/drop rules, VFS `ENOSPC` fault injector. Context: `c.chaos`.
     - To `db-proxy.md`: pgwire port 5432 intercept, SQLite UDS proxy, `DbAction` (Allow, Deny, Rewrite). Context: `c.db`.
     - To `mesh.md`: Cross-hypervisor peer vsock streams, mTLS framing, traceparent hop propagation. Context: `c.mesh`.
     - To `wire-ingress.md`: TCP connection parking, `socket2` buffer clamping, `httparse` header sniffer, wake-on-traffic. Context: `c.ingress`.
  3. Strip `06-wire-plane.md` down to **virtio-net bus mechanics only**:
     - TAP interface bridge and L2 frame intercept.
     - Raw packet interception points (`on_net_outbound`, `on_net_inbound`).
     - Frame drop/forward semantics.

---

### Agent 4: Lifecycle Extensions & Telemetry
* **Can Edit:**
  - `cans/07-lifecycle.md`
  - `cans/08-epistemic.md`
  - `cans/plugins/hyper-density.md` (create)
  - `cans/plugins/cas-snapshot.md` (create)
  - `cans/plugins/ledger.md` (create)
* **Must Not Edit:** Any other file.
* **Tasks:**
  1. Create all 3 plugin files in `cans/plugins/`.
  2. **CUT** from `07-lifecycle.md` and **MOVE**:
     - To `hyper-density.md`: Line-speed idle detection, `lz4_flex` memory compression, direct NVMe dump via `io-uring`. Context: `c.density`.
     - To `cas-snapshot.md`: FastCDC chunking, BLAKE3 SIMD chunk hashing, `object_store` S3 sync. Context: `c.cas`.
  3. Strip `07-lifecycle.md` down to **raw hypervisor superpowers only**:
     - KVM/HVF CoW `fork()` (<15ms).
     - Register/memory `rollback()` (<8ms).
     - Standard `suspend()` / `resume()`.
     - Hardware `clock.skew()` (TSC unpinning, `CNTVOFF_EL2`).
     - Sleep/wake hardware snapping (IOPM / systemd-logind).
     - Host crash reapers (`PDEATHSIG` / kqueue `NOTE_EXIT`).
     - Two-phase commit (2PC) synchronization protocol.
     - Break-glass interactive TTY attach.
     - Lifecycle veto registration and hardware watchdog timer.
  4. **CUT** from `08-epistemic.md` and **MOVE** to `ledger.md`:
     - Merkle-tree Write-Ahead Log (WAL) and SQLite sink.
     - OpenTelemetry W3C traceparent injection and OTel exporter.
     - `compute_causal_delta()` engine.
     - Context key: `c.ledger`.
  5. Retain in `08-epistemic.md`:
     - Three-Tier Truth hierarchy.
     - Raw `ExecutionReceipt` generation and PMU retired instructions accounting.
     - Epistemic Oracle interface (`analyze_intent`, `verify_physical_receipt`).
     - Core `StepSlot` checkpoint loop (`c.step.mark`, `c.step.last`).

---

### Agent 5: Adapters & Supply Chain
* **Can Edit:**
  - `cans/10-adapters.md`
  - `cans/02-plugin-model.md`
  - `cans/plugins/mcp.md` (create)
  - `cans/plugins/oci-translator.md` (create)
  - `cans/plugins/openshell-coordinator.md` (create)
  - `cans/plugins/supply-chain.md` (create)
* **Must Not Edit:** Any other file.
* **Tasks:**
  1. Create all 4 plugin files in `cans/plugins/`.
  2. **CUT** from `10-adapters.md` and `02-plugin-model.md` and **MOVE**:
     - To `mcp.md`: MCP stdio JSON-RPC tool router, capability advertising, ring mappings. Context: `c.mcp`.
     - To `oci-translator.md`: OCI layer flattening, EROFS block generation, container volume resolver.
     - To `openshell-coordinator.md`: Checkpoint/branch policies, NVIDIA CDI GPU binding, lifecycle vetoes.
     - To `supply-chain.md`: Package manager process tree scanner, lifecycle script vetoes (npm/pip).
  3. Keep in `10-adapters.md`:
     - Ingestion taxonomy (Contract-bound binary vs thin SDK wrapper).
     - Wire daemon architectures: `containerd-shim-v2`, `openshell-grpc`, `mcp-server`.
     - E2B SDK drop-in compatibility shim.

---

## Wave 2: Synchronization & Hardened Verification (Agent 6)

* **Prerequisite:** Agents 1 through 5 merged cleanly into the integration branch.
* **Can Edit:**
  - `cans/00-overview.md`
  - `cans/01-architecture.md`
  - `cans/02-plugin-model.md`
  - `cans/03-sdk-architecture.md`
  - `cans/11-operations.md`

### 1. Reconcile Master Topology Matrix
In `cans/02-plugin-model.md#First-class-plugins`, construct the canonical binding table. Every plugin crate in `crates/kelir-plugins/` must map 1:1 to a `cans/plugins/*.md` file:

```markdown
- First-class plugins
  - Topology matrix: see artifacts/diagrams/four-ring-bus.txt
  - plugin-pty: see plugins/pty.md
  - plugin-vfs: see plugins/vfs.md
  - plugin-wire: see plugins/wire.md
  - plugin-chaos: see plugins/chaos.md
  - plugin-ledger: see plugins/ledger.md
  - plugin-db-proxy: see plugins/db-proxy.md
  - plugin-mesh: see plugins/mesh.md
  - plugin-lock-broker: see plugins/lock-broker.md
  - plugin-supply-chain: see plugins/supply-chain.md
  - plugin-hyper-density: see plugins/hyper-density.md
  - plugin-cas-snapshot: see plugins/cas-snapshot.md
  - plugin-wire-ingress: see plugins/wire-ingress.md
  - plugin-mcp: see plugins/mcp.md
  - plugin-oci-translator: see plugins/oci-translator.md
  - plugin-openshell-coordinator: see plugins/openshell-coordinator.md
```

### 2. Update Spec Maps
* Update `cans/00-overview.md#Spec-map` to include the `plugins/` catalog.
* Update `cans/11-operations.md#Per-plugin-CI` and `#Project-structure` to reflect the clean decoupled spec structure.

### 3. Hardened Verification Pipeline (Merge Gate)

The integration agent must run and pass all 5 verification phases:

#### Phase A: 1:1 Parity Audit (Bash Gate)
Assert that every plugin in Cargo matches an exact spec file:
```bash
for p in $(ls crates/kelir-plugins); do
  NAME=${p#plugin-}
  test -f "cans/plugins/${NAME}.md" || (echo "MISSING SPEC: cans/plugins/${NAME}.md" && exit 1)
done
echo "PARITY AUDIT: 15/15 PLUGINS VERIFIED"
```

#### Phase B: Pointer Graph Audit (0 Broken Links, Max 1 Hop)
```bash
out=$(cans check --refs-only --json)
echo "$out" | jq -e '.ok == true and .exitCode == 0' >/dev/null || (echo "POINTER AUDIT FAILED:" && echo "$out" && exit 1)
```

#### Phase C: Full Node Invariant & Duplicate Home Audit
Enforces `_rules.yaml` rules (node length 3–90, max 20 siblings, depth 2–10, zero duplicate homes, redundancy thresholds):
```bash
out=$(cans check --json)
echo "$out" | jq -e '.ok == true and .exitCode == 0' >/dev/null || (echo "CANS INVARIANT AUDIT FAILED:" && echo "$out" && exit 1)
```

#### Phase D: Write Envelope Boundary Audit
```bash
git diff --name-only origin/main...HEAD > /tmp/touched-files
cans check --touched-files /tmp/touched-files --json | jq -e '.ok == true and .exitCode == 0' >/dev/null || (echo "BUDGET OVERFLOW DETECTED" && exit 1)
```

#### Phase E: Local Justfile Pre-Flight Execution
```bash
just cans-check
```

---

## Standard Plugin File Skeleton (`cans/plugins/*.md`)

Every file created in `cans/plugins/` MUST match this structure:

```markdown
- Plugin <name>
  - Capability definition
    - Plugin identifier: plugin-<name>
    - Owner crate: crates/kelir-plugins/plugin-<name>
    - VirtIO Ring attachment: see 01-architecture.md#Four-Ring-Bus
    - Lifecycle attachment: see 07-lifecycle.md#Lifecycle-vetoes
  - Context keys
    - Context key contributed: c.<key>
    - Closed-set binding rule: see 02-plugin-model.md#Core-context-keys
  - Invariants and policy
    - [Specific policy rules, regex rules, thresholds, or algorithms]
  - Error and return semantics
    - Hook signature: see artifacts/code/plugin-trait.rs
    - Return errno and rejection behavior
```

---

## Conflict Resolution Rules

1. **Cut, Never Copy:** Any concept moved to `cans/plugins/*.md` MUST be deleted from its origin plane file. Duplicate text fails Phase C immediately.
2. **Mechanism vs. Policy:** Wire mechanics, register handling, and FUSE hooks stay in `cans/0X-*.md`. All parsing, business logic, regexes, credentials, and error policies go to `cans/plugins/*.md`.
3. **Strict 1-Hop:** No chained references (`A -> B -> C`). Plugins point directly to the owning bus in one jump.
4. **No Guesses:** Unknown behaviors remain marked as `TBD`. Never invent configuration fields.