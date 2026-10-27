# WORK ORDER: ARCHITECTURAL PURIFICATION & DIRECT PLUGIN LINKAGE

**Document:** `work-order.md`  
**System:** `kelir` Hypervisor Microkernel & Plugin Ecosystem  
**Target State:** Canonical Day-1 Architecture (Zero migration narrative, zero historical diffs)  
**Execution:** 6 Parallel Non-Conflicting Autonomous Agents  

---

## 1. System Context & Architectural Mandates

`kelir` is an in-process microVM hypervisor microkernel. It mediates guest Linux workloads via the VirtIO Device Complex (`virtio-vsock`, `virtio-console`, `virtio-fs`, `virtio-net`). It provides hardware-isolated execution for untrusted processes with zero guest-side agents.

Every agent executing this work order must enforce three non-negotiable architectural mandates:

### Mandate A: Pure Microkernel Boundary
Core handles vCPUs, MMU boundaries, physical memory regions, and raw VirtIO queue descriptors. Core contains **zero domain logic, zero protocol parsers, zero agent schemas, and zero hardcoded plugin identities**. Any reference to PTY control, MCP tools, database protocols, or specific plugin names inside core crates or wire definitions is a defect.

### Mandate B: 3-Tier Direct-Import Plugin Hierarchy
Runtime string-based dependency resolution (`Capability("...")`, `OrderConstraint`, Kahn's DAG sorting) is dead. Plugins are compiled Rust crates that link directly via Cargo (and package managers for TS/Python). Inter-plugin dependencies must follow an acyclic 3-tier hierarchy:

```
┌────────────────────────────────────────────────────────┐
│  Tier 3: Aggregators & Orchestrators                   │
│  (plugin-ledger, plugin-profile, plugin-chaos)         │
│  Imports: Tier 2 and Tier 1 directly                   │
└───────────────────────────┬────────────────────────────┘
                            │ imports
┌───────────────────────────▼────────────────────────────┐
│  Tier 2: Governance, Security & Interposition          │
│  (plugin-tool-guard, plugin-lock-broker, db-proxy,     │
│   plugin-supply-chain, plugin-wire-ingress)            │
│  Imports: Tier 1 directly                              │
└───────────────────────────┬────────────────────────────┘
                            │ imports
┌───────────────────────────▼────────────────────────────┐
│  Tier 1: Base VirtIO Ring Interposers                  │
│  (plugin-pty, plugin-vfs, plugin-wire, plugin-mcp)     │
│  Imports: kelir-interface only                         │
└────────────────────────────────────────────────────────┘
```

### Mandate C: Decoupled Session Extension Trait Pattern
The core `Session` object provides only hypervisor primitives (`boot`, `fork`, `suspend`, `resume`, `rollback`, `terminate`) and raw ring channel access. Domain APIs (`session.forward_port()`, `session.term.*`, `session.fs.*`) must not exist on the core struct. They attach via language-level extension traits/interfaces implemented in owning plugin packages:
* **Rust:** `session.extension::<PtyHandle>()?.write_input(...)`
* **TypeScript:** `session.use(ptyExtension).writeInput(...)`
* **Python:** `session.ext(PtyExtension).write_input(...)`

---

## 2. File Ownership Matrix

To prevent git conflicts, agents operate strictly on disjoint file boundaries:

| Agent | Functional Domain | Exclusive File Ownership |
|---|---|---|
| **Agent 1** | Core Interface & Wire Framing | `cans/artifacts/code/wire-protocol.rs`<br>`cans/artifacts/code/messages.rs`<br>`cans/01-architecture.md`<br>`cans/09-protocol.md` |
| **Agent 2** | Plugin Trait & Tiered Linkage | `cans/artifacts/code/plugin-trait.rs`<br>`cans/02-plugin-model.md`<br>`cans/plugins/*.md` (all 13 files) |
| **Agent 3** | VirtIO Ring Interposition Planes | `cans/04-terminal-plane.md`<br>`cans/05-storage-plane.md`<br>`cans/06-wire-plane.md`<br>`cans/artifacts/diagrams/four-ring-bus.txt`<br>`cans/artifacts/diagrams/terminal-pipeline.txt`<br>`cans/artifacts/diagrams/wire-plane.txt`<br>`cans/artifacts/diagrams/storage-architecture.txt` |
| **Agent 4** | Lifecycle, Telemetry & Receipts | `cans/07-lifecycle.md`<br>`cans/08-epistemic.md`<br>`cans/artifacts/code/vmm-driver-trait.rs`<br>`cans/artifacts/code/execution-receipt.rs`<br>`cans/artifacts/code/causal-delta.rs` |
| **Agent 5** | CI, Build & Tooling Realignment | `cans/11-operations.md`<br>`cans/artifacts/config/ci.yml`<br>`cans/artifacts/config/justfile`<br>`cans/artifacts/config/project-structure.txt` |
| **Agent 6** | SDK Surface & Typestate Facades | `cans/03-sdk-architecture.md`<br>`cans/artifacts/code/rust-handle-injection.rs`<br>`cans/artifacts/code/typescript-accumulation.ts`<br>`cans/artifacts/code/python-raii-pydantic.py`<br>`cans/artifacts/diagrams/sdks-invariants.txt` |

---

## 3. Detailed Work Packages

### Agent 1: Core Interface & Wire Framing

#### 1. Target Invariants
* The core multiplexed protocol over `vsock:5252` is agnostic of process lineage, terminals, and plugins.
* `HostToGuest` and `GuestToHost` represent generic kernel traps, raw channel streams, memory pressure, and watchdog signals.

#### 2. Exact Specification Changes

* **`cans/artifacts/code/wire-protocol.rs`:**
  * Preserve the packed 24-byte header: `magic (0x4B45)`, `version (0x04)`, `channel_id (u16)`, `message_type (u16)`, `flags (u16)`, `reserved (u16)`, `sequence (u64)`, `payload_len (u32)`.
  * Define canonical message types strictly as integers:
    * `0x0001` = Trap Notification / Decision
    * `0x0002` = Channel Payload (`ChannelData`)
    * `0x0003` = VM Lifecycle Control
    * `0x0004` = System Telemetry (Heartbeat, Pressure)

* **`cans/artifacts/code/messages.rs`:**
  * **Delete** `HostToGuest::WindowResize` and `HostToGuest::SendSignal`. Terminal control frames must be transported inside `HostToGuest::ChannelData` destined for a console channel endpoint.
  * **Cleanse `TrapEvent`:**
    ```rust
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TrapEvent {
        pub seq: u64,
        pub pid: u32,
        pub ppid: u32,
        pub task_id: u64,
        pub binary_path: String,
        pub binary_dev: u64,
        pub binary_ino: u64,
        pub binary_blake3: [u8; 32],
        pub argv: Vec<String>,
        pub envp: HashMap<String, String>,
        pub cwd: String,
    }
    ```
    *Remove:* `rewrite_count`, `namespace_depth`, `ancestor_chain`, `loaded_libraries`. These are plugin-derived properties, not raw kernel trap facts.
  * **Simplify `TrapAction`:**
    ```rust
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub enum TrapAction {
        Allow,
        Deny { exit_code: i32, stderr_message: String },
        Rewrite { new_binary: String, new_argv: Vec<String>, injected_env: HashMap<String, String> },
        Yield { checkpoint_id: String },
        Abort { exit_code: i32, reason: String },
        Hold { duration_ms: u32 },
        Custom(ciborium::value::Value),
    }
    ```
    *Remove:* `HostHold { poll_interval_ms }` (generalized to `Hold`).

* **`cans/01-architecture.md` & `cans/09-protocol.md`:**
  * Purge all text claiming core evaluates process ancestor chains or terminal signals on the wire.
  * Document Channel 0 as hypervisor control, Channels 1–65535 as dynamic multiplexed endpoints.

---

### Agent 2: Plugin Trait & Tiered Linkage

#### 1. Target Invariants
* Zero runtime string capability resolution. Delete `Capability`, `OrderConstraint`, and runtime Kahn's DAG sorting.
* Direct Cargo crate linkage between dependent plugins.
* Services are strongly typed Rust traits exported directly by the defining plugin crate.

#### 2. Exact Specification Changes

* **`cans/artifacts/code/plugin-trait.rs`:**
  * Remove `Capability`, `OrderConstraint`, `PluginTaxonomy`.
  * Replace runtime registration with typed lifecycle and interposer registration:
    ```rust
    pub trait KelirPlugin: Send + Sync {
        fn name(&self) -> &'static str;
        fn default_scope(&self) -> MountScope { MountScope::default() }
        fn is_enabled(&self) -> bool { true }
        fn fork_state(&self, child_session_id: &str) -> Box<dyn AnyPlugin>;
    }
    ```
  * Retain segregated ring interposer traits (`FsInterposer`, `NetInterposer`, `TrapInterposer`, `ConsoleInterposer`, `LifecycleParticipant`).
  * Remove `occ_metadata_hash` from `FsInodeAttr`. Keep attributes strictly to standard POSIX: `mode`, `uid`, `gid`, `size`, `content`.

* **`cans/02-plugin-model.md`:**
  * Replace runtime DAG sorting documentation with the **3-Tier Compile-Time Dependency Model**:
    * **Tier 1:** Base ring adapters (`plugin-pty`, `plugin-vfs`, `plugin-wire`, `plugin-mcp`). Depend strictly on `kelir-interface`.
    * **Tier 2:** Security, governance, protocol proxies (`plugin-tool-guard`, `plugin-lock-broker`, `plugin-db-proxy`, `plugin-supply-chain`, `plugin-wire-ingress`). Declare direct Cargo dependencies on Tier 1 plugins.
    * **Tier 3:** Cross-cutting aggregators (`plugin-ledger`, `plugin-profile`, `plugin-chaos`). Declare direct Cargo dependencies on Tier 1 and Tier 2 plugins.
  * Define typed cross-plugin communication:
    * `plugin-tool-guard` directly imports `plugin-mcp` AST/call definitions.
    * `plugin-lock-broker` directly imports `plugin-vfs` inode abstractions.
    * `plugin-db-proxy` directly imports `plugin-wire` L4 proxy primitives.

* **`cans/plugins/*.md` (All 13 files):**
  * Rewrite headers to replace string taxonomy (`urn:kelir:...`, `provides = [...]`, `requires = [...]`) with explicit Rust crate dependencies and exposed traits.

---

### Agent 3: VirtIO Ring Interposition Planes

#### 1. Target Invariants
* VirtIO devices are transport pipes. No core device ring is reserved for a specific plugin.
* The wire, storage, and console planes process raw payloads without hardcoded pipeline stage sequences.

#### 2. Exact Specification Changes

* **`cans/04-terminal-plane.md`:**
  * Delete hardcoded port mappings (`hvc0` = PTY, `hvc1` = MCP/Tool-Guard).
  * Specify `virtio-console` as a multi-port paravirtual character bus. Channel 0 defaults to interactive console master; subsequent ports are allocated dynamically at session configuration.
  * Delete all references to `plugin-tool-guard` in the terminal plane.

* **`cans/05-storage-plane.md`:**
  * Purge all mentions of "OCC Stable Inode Hashes". Optimistic concurrency control is an application/plugin concern, not a microkernel VirtIO-FS property.
  * Document `virtio-fs` as an unadulterated FUSE pass-through bridge with read/write interposition hooks.

* **`cans/06-wire-plane.md`:**
  * Purge hardcoded pipeline sequences (`chaos -> ingress -> db-proxy -> wire proxy`).
  * Document the wire plane as an L2 TAP bridge with hook points `on_net_outbound` and `on_net_inbound`.
  * Remove database proxying and credential swapping from the core plane specification; assign them strictly to `plugins/wire.md` and `plugins/db-proxy.md`.

* **`cans/artifacts/diagrams/`:**
  * `four-ring-bus.txt`: Remove plugin names (`Token Shield`, `TAP Framing`, `Credentials`, `Chaos`). Label purely by transport:
    * `virtio-vsock`: Control & Multiplexed IPC
    * `virtio-console`: Multiplexed Character Streams
    * `virtio-fs`: FUSE Inode & File Data
    * `virtio-net`: Raw L2 TAP Ethernet Frames
  * `terminal-pipeline.txt`, `wire-plane.txt`, `storage-architecture.txt`: Strip references to hardcoded stage ordering.

---

### Agent 4: Lifecycle, Telemetry & Receipts

#### 1. Target Invariants
* Hypervisor lifecycle states must not leak plugin identities.
* Hardware telemetry is strictly empirical (PMU, registers, dirty bytes). Core does not generate policy judgments.

#### 2. Exact Specification Changes

* **`cans/artifacts/code/vmm-driver-trait.rs` & `cans/07-lifecycle.md`:**
  * Purge plugin-specific suspend reasons. Generalize `SuspendReason`:
    ```rust
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SuspendReason {
        External(u32),
        ResourcePressure,
    }
    ```
    *Delete:* `IngressWait`, `HostHold(TOOL_GUARD_HOLD)`, `AdminHold`.
  * Establish the Single Source of Truth for zero-RSS swap in `cans/07-lifecycle.md#Suspend-and-resume`:
    * Page-dump engine: `lz4_flex` compression over `io_uring` direct I/O.
    * Memory ceiling drops to 0 MB host RSS.
  * Purge redundant copies of the zero-RSS swap description from `cans/01-architecture.md`, `cans/plugins/tool-guard.md`, and `cans/plugins/wire-ingress.md`.

* **`cans/08-epistemic.md` & `cans/artifacts/code/execution-receipt.rs`:**
  * Remove specific plugin references (`plugin-wire`, `plugin-ledger`, `plugin-db-proxy`) from `cans/08-epistemic.md`.
  * Keep `ExecutionReceipt` purely hardware-focused:
    * Hardware primitives: `retired_instructions`, `dirty_bytes`, `wall_clock_ns`, `hardware_attestation`, `exit_code`.
    * Extensibility: `extensions: Vec<TelemetryExtension>` with opaque `namespace: String` and `payload: Vec<u8>`.

---

### Agent 5: CI, Build & Tooling Realignment

#### 1. Target Invariants
* CI must enforce compile-time crate dependencies, not artificial module isolation.
* Build configurations must reflect the 3-tier hierarchy.

#### 2. Exact Specification Changes

* **`cans/artifacts/config/ci.yml` & `cans/artifacts/config/justfile`:**
  * **Delete** the `plugin-isolation` job and `test-isolation` script (which deleted plugin directories to assert decoupling).
  * **Delete** the `plugin-audit` restrictions that banned inter-plugin dependencies in `Cargo.toml`.
  * Add static workspace verification:
    ```yaml
    - name: Verify Tiered Plugin Compilation
      run: cargo check --workspace --all-targets
    ```
  * Retain `cans-check`, `audit-slop`, guest shim compile checks (<50KB), and nextest matrices.

* **`cans/11-operations.md`:**
  * Rewrite the CI and testing sections. Document that plugins compile against their explicit upstream dependencies.
  * Remove all references to isolation proof by deletion.
  * Consolidate coverage gates: verify that each plugin registers with the builder without runtime string tokens.

* **`cans/artifacts/config/project-structure.txt`:**
  * Annotate crate dependency rules explicitly:
    * Tier 1 plugins depend only on `kelir-interface`.
    * Tier 2 plugins depend on Tier 1 plugins.
    * Tier 3 plugins depend on Tier 1 and Tier 2 plugins.

---

### Agent 6: SDK Surface & Typestate Facades

#### 1. Target Invariants
* Dynamic language SDKs (TypeScript, Python) and Rust builder surfaces must use typed enums and constants instead of stringly-typed dispatch.
* The core `Session` object provides raw hypervisor control; domain capabilities attach via typed extensions.

#### 2. Exact Specification Changes

* **`cans/03-sdk-architecture.md`:**
  * **Eradicate String Event Hooks:**
    * Replace `.on("vm:ready", ...)` with `.on(LifecycleEvent.VmReady, ...)`.
    * Replace `.intercept("net:outbound", ...)` with `.intercept(Ring.NetOutbound, ...)`.
    * Replace `.trap("git commit *", ...)` with structured matchers: `TrapFilter.binary("git").withArg("commit")`.
  * **Document the Extension Pattern for `Session`:**
    * In Rust:
      ```rust
      let session = builder.boot().await?;
      let pty = session.extension::<PtyExtension>()?;
      pty.resize(120, 40).await?;
      ```
    * In TypeScript:
      ```typescript
      const session = await builder.boot();
      const pty = session.use(ptyExtension);
      await pty.resize(120, 40);
      ```
    * In Python:
      ```python
      async with builder as session:
          pty = session.ext(PtyExtension)
          await pty.resize(120, 40)
      ```

* **`cans/artifacts/code/rust-handle-injection.rs`, `typescript-accumulation.ts`, `python-raii-pydantic.py`:**
  * Update all sample code to use the extension trait pattern and typed event constants.
  * Strip `c.forward_port()`, `c.term`, `c.fs` ambient properties. Replace with `c.service::<PtyService>()`.

* **`cans/artifacts/diagrams/sdks-invariants.txt`:**
  * Update runtime safety invariants to reflect typed enum compilation across all three languages.

---

## 4. Single Source of Truth (SSOT) Map

To guarantee zero redundancy, enforce the following authoritative locations:

| Fact / Concept | Canonical Single Source of Truth | Forbidden Everywhere Else |
|---|---|---|
| **Gate Latency Ceilings** (`0µs / <2µs / <12µs`) | `cans/02-plugin-model.md#Latency-ceiling` | Do not restate numbers in diagrams or code docs. |
| **VM Boot & Rollback Constants** (`<60ms`, `<8ms`) | `cans/artifacts/code/vmm-driver-trait.rs` | Do not restate in markdown spec nodes. |
| **Wire Header Byte Layout** | `cans/artifacts/code/wire-protocol.rs` | Do not draw header bit-tables in markdown files. |
| **Zero-RSS Swap Mechanics** (`lz4_flex + io_uring`) | `cans/07-lifecycle.md#Suspend-and-resume` | Reference via `see: 07-lifecycle.md#Suspend-and-resume`. |
| **Permission Verdict Lattice** (`Allow < Rewrite < Deny < Abort`) | `cans/artifacts/code/messages.rs#TrapAction` | Reference via `see: artifacts/code/messages.rs`. |
| **3-Tier Plugin Hierarchy** | `cans/02-plugin-model.md#Plugin-tiering` | Do not duplicate tier classifications in plane files. |

---

## 5. Execution & Verification Instructions

### Phase 1: Clean Core & Interfaces (Agents 1, 4)
* Execute Package 1 (`wire-protocol.rs`, `messages.rs`, `01-architecture.md`, `09-protocol.md`).
* Execute Package 4 (`vmm-driver-trait.rs`, `execution-receipt.rs`, `causal-delta.rs`, `07-lifecycle.md`, `08-epistemic.md`).
* *Checkpoint Gate:* Ensure wire definitions compile with zero plugin and terminal symbols.

### Phase 2: Restructure Plugin & Plane Layers (Agents 2, 3)
* Execute Package 2 (`plugin-trait.rs`, `02-plugin-model.md`, `cans/plugins/*.md`).
* Execute Package 3 (`04-terminal-plane.md`, `05-storage-plane.md`, `06-wire-plane.md`, diagrams).
* *Checkpoint Gate:* Verify that all plugin specifications express dependencies via explicit crate imports.

### Phase 3: Update SDKs, Tooling & Verification (Agents 5, 6)
* Execute Package 5 (`ci.yml`, `justfile`, `11-operations.md`, `project-structure.txt`).
* Execute Package 6 (`03-sdk-architecture.md`, SDK sample code, diagrams).
* *Final Gate:* Run verification test suite.

---

## 6. Final Acceptance Gate Commands

The agents' collective work is complete only when the following suite succeeds with zero warnings:

```bash
# 1. Spec Integrity & Reference Check (No dangling anchors, no rule breaks)
cans check --refs-only --json | jq -e '.ok and .exitCode == 0'
cans check --json | jq -e '.ok and .exitCode == 0'

# 2. Banned Term & Concept Leak Grep (Must return 0 lines)
git grep -E "Capability\(|IngressWait|TOOL_GUARD_HOLD|occ_metadata_hash" -- cans/

# 3. Microkernel Wire Cleanliness Audit (Must return 0 lines)
git grep -i -E "pty|mcp|token_shield|tool_guard|db_proxy" -- cans/artifacts/code/messages.rs cans/artifacts/code/wire-protocol.rs

# 4. Redundancy Audit (Ensure swap engine is mentioned exactly once)
test $(git grep -c "lz4_flex" -- cans/ | wc -l) -eq 1
```