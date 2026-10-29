- Hardware Telemetry & Cryptographic Verification
  - Scope
    - Telemetry ownership: Hypervisor core measures physical hardware events exclusively
    - Fact emission: TrapEvent, ExecutionReceipt, CausalDelta
    - Hardware counters: CPU instructions, dirty pages, MMU faults, wire byte volume
    - Boundary opacity: Internal interception policies surface to the guest strictly
      as physical bus errors, standard POSIX errnos, or silent packet drops
  - Primitive vs derived telemetry
    - Telemetry decision test
      - Single read API, hardware boundary, no policy → primitive, core-owned
      - Computed from primitives, policy-bearing → derived, plugin-owned
    - Default posture: per-exec, per-trap, per-receipt primitives enabled
  - Execution receipts
    - see: artifacts/code/execution-receipt.rs
    - Deterministic hardware counter attribution emitted per executed process
    - Emitted per executed process, at the trap/exit boundary
    - Receipts compare as values via canonical per-field structural assertions
    - Receipts carry opaque TelemetryExtension payloads emitted by mounted plugins
    - Hardware attestation token: BLAKE3 cryptographic proof over execution tuple
    - Byte-not-page reason: see: 01-architecture.md#SnapshotDescriptor.dirty-bytes
    - Population: per ReceiptConfig; opt-in fields not enabled are absent
      (Option), never 0
      - retired_instructions: sampled from host vCPU thread via perf_event_open
      - Apple Silicon: queried via native Mach thread instruction counters
      - Eliminates cross-architecture dirty-page and wall-clock variance
      - Opt-in: syscall_count
      - dirty_bytes is always populated: zero is a real measurement, not an
        absent value
    - binary_blake3 identity
      - Canonical form: TBD
      - Cache: keyed on (inode, size, mtime_ns); one hash per
        binary identity, not per exec
    - Receipt accounting: TBD
      - syscall_count counting rule
      - egress_bytes observation point
      - wall_clock_ns across suspend
      - exit_code under Rewrite
      - Sub-microsecond phase spans: trap_ingress_ns, ffi_hop_ns, policy_ns
    - Receipt ownership
      - kelir-core generates and holds at trap/exit boundary
      - c.receipt() is the single read API on HandlerCtx
  - Causal delta
    - see: artifacts/code/causal-delta.rs
    - Definition: Δ = S_A − S_B between two checkpoint descriptors
    - Delta fields: see: artifacts/code/causal-delta.rs
    - State diff aggregation: CausalDelta aggregates net physical diffs
    - Merkle root chaining links granular receipts to state diffs
    - Tracks physical page deltas, modified inodes, and process lineage
    - L7 protocol telemetry lives outside core structs, in extensions
    - Type ownership
      - Struct is a shared context type; it lives in kelir-interface next to
        ExecutionReceipt
      - Core aggregates domain deltas into CausalDelta on checkpoint boundaries
  - Telemetry provenance classification
    - Invariant: A higher classification never falls back to a lower classification
    - Class 1 (Hardware Sampling): PMU instruction counters, dirty byte counts,
      physical exit codes, hardware TSC timestamps. Unforgeable from guest space.
    - Class 2 (Paravirtual Interception): FUSE buffers, virtqueue frames, character rings.
      Observed at the hypervisor boundary.
    - Class 3 (Guest Unverified Data): Workload stdout/stderr payloads, memory buffers.
      Tainted by default; cannot satisfy hardware assertions.
    - Selection rule
      - Claim must resist forgery by guest code → Tier 1
      - Claim about a public, versioned contract → Tier 2
      - Claim about harness-internal state with no wire surface → Tier 3
    - Degradation rule
      - A higher-tier claim never falls back to a lower-tier signal
      - Missing Tier 1 evidence is a denial, not a Tier 3 downgrade
      - Rationale: silent downgrade is exactly the harness self-report failure
        this tier scheme exists to prevent
    - Consumer of Tier 3: Oracle.ingest_harness_event; see: #Oracle
  - Lifecycle bridge
    - Orchestrator verbs: pause, resume, step, revert, fork, suspend
    - Each verb aliases exactly one mechanism in
      07-lifecycle.md#Hypervisor-primitives
    - pause: vCPU-only halt at cycle boundary; guest RAM stays resident
    - step: run to next virtio-vsock syscall trap, then pause for eval
    - suspend is the only verb that flushes RAM; every other verb is
      cycle-exact
    - Veto points: see: 07-lifecycle.md#Lifecycle-vetoes
  - Step evaluation loop
    - Orchestrators execute unmodified agent binaries
    - Checkpoint descriptors are retained in userland orchestrator state
    - Post-step verification compares physical receipts against pre-step states
    - StepSlot semantics
      - Single slot per active PID lineage; sub-steps push to stacked savepoints
      - StepSlot managed by plugin-checkpoint: see: plugins/checkpoint.md
      - Fork operations clone active StepSlot state into child sessions
      - Post-hook failure triggers immediate driver rollback to slot base
    - Typical wiring
      - Trap match triggers pre-hook: checkpoint creation & intent analysis
      - Guest executes process
      - Process exit triggers post-hook: physical ExecutionReceipt assertion
      - Mismatch triggers hypervisor rollback
        (timing: see: artifacts/code/vmm-driver-trait.rs)
  - Lifecycle nano-tracer
    - Trace hooks (.trace()) measure exact nanosecond overhead across phases
    - Tracks trap ingress, FFI crossing, policy evaluation, and verdict dispatch
    - Zero-allocation timings pipe directly to receipts and OTel spans
