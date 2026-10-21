- Epistemic
  - Scope
    - Boundary specification; not a runtime subsystem
    - Core owns facts, never judgments
      - Facts: TrapEvent, ExecutionReceipt, CausalDelta (struct), StepSlot
    - Judgment has exactly two homes, both removable
      - Oracle: user interface, one per session, not a plugin, no dispatch slot
      - plugin-ledger: derived computation over core primitives
    - Removing either leaves core intact; see #Primitive-vs-derived-telemetry
    - Enforcement: see 01-architecture.md#Axiomatic-foundations
  - Oracle
    - User-supplied interface; NOT a plugin
    - Location: kelir-core::oracle::Oracle
    - Injection: builder.oracle(impl Oracle); one per session
    - Not in dispatch array; no priority(); no requires()
    - Rationale: single policy layer for the epistemic engine
      - Epistemic policy is user-owned, not composable
      - Oracles chain by composition inside their impl, not the plugin graph
    - Methods
      - analyze_intent(&TrapEvent) -> IntentAnalysis
        - Called from handlers, typically before(*)
        - IntentAnalysis: is_hypothesis_valid, falsification_reason, allow_high_entropy
      - verify_physical_receipt(&ExecutionReceipt) -> bool
        - Called from after(*) post-condition verification
      - ingest_harness_event(&Path, &Value, &str)
        - Called from snoop callbacks; see 05-storage-plane.md#Snoop-plane
      - await_human_approval()
        - Called between suspend/resume for human-in-the-loop gates
      - current_trace_id() -> String
        - Called from handlers setting OTEL_TRACE_ID
    - Wiring example: See artifacts/examples/autonomous-swe-agent.rs
  - Primitive vs derived telemetry
    - Telemetry decision test
      - Single read API, hardware boundary, no policy → primitive, core-owned
      - Computed from primitives, policy-bearing → derived, plugin-owned
      - A new derived metric is a plugin or Oracle change, never a core method
    - Default posture: per-exec, per-trap, per-receipt signals on; per-syscall,
      per-request, per-page signals off until enabled per config
    - Boundaries
      - Primitive set: see 01-architecture.md#Core-invariants
      - CausalDelta: see #Causal-delta
      - Merkle chain: see #Merkle-WAL
      - OpenTelemetry spans: see #OpenTelemetry
      - Oracle analyses: consumer's Oracle impl
  - Execution receipts
    - See: artifacts/code/execution-receipt.rs
    - Field set: See artifacts/code/execution-receipt.rs
    - Emitted per executed process, at the trap/exit boundary
    - Assertion form: receipts are compared as values, not as hashes of host
      state; the canonical assertion is a per-field comparison
    - Byte-not-page reason: see 01-architecture.md#SnapshotDescriptor.dirty-bytes
    - Population: per ReceiptConfig; opt-in fields not enabled are absent
      (Option), never 0
      - Always: pid, binary_path, binary_blake3, dirty_bytes, exit_code,
        wall_clock_ns, egress_bytes
      - Opt-in: syscall_count
      - dirty_bytes is always populated: zero is a real measurement, not an
        absent value
    - binary_blake3 cache: keyed on (inode, size, mtime_ns); one hash per
      binary identity, not per exec
    - Ownership
      - kelir-core generates and holds at trap/exit boundary
      - c.receipt() is the single read API on HandlerCtx
  - Causal delta
    - See: artifacts/code/causal-delta.rs
    - Definition: Δ = S_A − S_B between two checkpoint descriptors
    - Delta fields: See artifacts/code/causal-delta.rs
    - egress_requests: sampled or host-allowlisted; full per-request capture
      is never the default. Use receipt.egress_bytes unless per-request
      attribution is the question.
    - Type ownership
      - Struct is a shared context type; it lives in kelir-core next to
        ExecutionReceipt
      - Computation is policy-bearing; it lives in plugin-ledger
      - Session accessor `get_causal_delta_from_parent()` requires
        plugin-ledger mounted; unmounted sessions do not expose it
    - This split follows the primitive-vs-derived rule: type is primitive,
      computation is derived
  - Merkle WAL
    - Chaining formula and backend live in plugin-ledger; not restated here
    - Default: plain append-only WAL. Merkle chaining is opt-in via
      LedgerConfig::merkle_chain; plain WAL already resists guest rollback
      and fork. Enable Merkle only under a host-operator trust threat.
    - Host-isolated by construction
      - Store path resolved relative to consumer CWD; guest paths rejected
        at plugin construction
      - Default path: .kelir/audit.merkle.db relative to consumer CWD
      - WAL survives guest rollback() and fork(); no guest operation can
        rewrite or duplicate it
      - This invariant is the reason the WAL exists separately from receipts
    - Ownership: core generates receipts; plugin-ledger persists and exports
    - Ledger handler API
      - c.ledger.flush() forces WAL flush and OTel export
  - OpenTelemetry
    - Outbound HTTP requests carry W3C traceparent, injected host-side
    - Trace continuity rule: a guest-originated request and its guest-side
      cause (trap, spawn, receipt) resolve to the same trace
    - Config surface lives in plugin-ledger (OtelConfig); not restated here
  - Three-tier truth
    - See: artifacts/diagrams/truth-hierarchy.txt
    - Tier evidence per tier: See artifacts/diagrams/truth-hierarchy.txt
    - Selection rule
      - Claim must resist forgery by guest code → Tier 1
      - Claim about a public, versioned contract → Tier 2
      - Claim about harness-internal state with no wire surface → Tier 3
    - Degradation rule
      - A higher-tier claim never falls back to a lower-tier signal
      - Missing Tier 1 evidence is a denial, not a Tier 3 downgrade
      - Rationale: silent downgrade is exactly the harness self-report failure
        this tier scheme exists to prevent
    - Consumer of Tier 3: Oracle.ingest_harness_event; see #Oracle
  - Lifecycle bridge
    - See: artifacts/diagrams/lifecycle-bridge.txt
    - Replaces language-level monkeypatching
      - Old form: agent._step(), private runtime prototype overrides
      - Breaks on every CLI update, blind to OS-level side effects
    - In-process hardware lifecycle bridge
    - Mapping from orchestrator actions to hypervisor primitives
      - pause() → halt vCPU registers instantly, 0% CPU
      - resume() → restore vCPU with 0ms clock skew
      - step() → execute until next virtio-vsock syscall trap, then pause
      - revert() → drop dirty RAM, purge tmpfs diffs to last good snapshot in <8ms
    - Underlying primitives: see 07-lifecycle.md#Superpowers
    - Veto points: see 07-lifecycle.md#Lifecycle-vetoes
    - This node specifies the orchestrator-facing mapping only
  - Step evaluation loop
    - See: artifacts/code/step-evaluation-loop.ts
    - Orchestrators execute unmodified agent binaries
    - Cycle-accurate governance over every operational step
    - Pre/post tracking is a core context service
    - StepSlot on HandlerCtx
      - c.step.mark(descriptor): stores typed checkpoint slot for trapping PID
      - c.step.last(): retrieves it, raises if none
      - No userland scratchpads, no plugin boilerplate
    - Typical wiring
      - before("*"): checkpoint + analyzeIntent; deny or resume
      - after("*"): read receipt; rollback on post-condition violation
