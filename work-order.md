# WORK-ORDER: KELIR 3.0 COMPLETE SURFACE ALIGNMENT

**Status:** Canonical Execution Blueprint  
**Target:** Total elimination of web-router metaphors; unification into an **Event-Driven Hardware Interception Bus**; zero-closure flat plugin configuration; day-1 non-archaeological canonical text across all 33 spec and artifact files.

---

## 1. Canonical API Specification (Day-1 SSOT)

Every SDK and spec file must reflect these exact signatures. No variations.

### 1.1 TypeScript SDK (`@kelir/core`)
```ts
// 1. Blueprint Phase (Write-Only Builder)
const runtime = kelir(vmConfig: VmConfig)
  // Plugins take strictly FLAT options (no callback builders)
  .use(plugin: KelirPluginInstance)
  
  // Per-session schema push-down (AOT-compiled to native Rust validator)
  .params<T>(schema: TSchema)

  // Trap Engine (Control Plane: Syscalls & Binaries)
  // Overload A: Simple Pre-Exec Gate
  .trap(pattern: string, handler: (c: HandlerCtx<T>) => Promise<Decision>)
  // Overload B: Dual Pre/Post Gate
  .trap(pattern: string, hooks: {
    pre?: (c: HandlerCtx<T>) => Promise<Decision>;
    post?: (c: HandlerCtx<T>) => Promise<PostExecAction>;
  })

  // Intercept Engine (Ring-level synchronous hot-path filters)
  .intercept(channel: "vfs:write", pathGlob: string, handler: (e: VfsWriteEvent, c: HandlerCtx<T>) => SyncVerdict | Promise<SyncVerdict>)
  .intercept(channel: "net:outbound" | "net:inbound", filter: string, handler: (frame: NetFrame, c: HandlerCtx<T>) => SyncVerdict)

  // Lifecycle Interceptors (Hypervisor Transitions)
  .on(event: "vm:init", handler: (driver: VmmDriver) => Promise<void>)
  .on(event: "vm:ready" | "vm:shutdown", handler: (session: Session) => Promise<void>)
  .on(event: "trap:deny", handler: (e: TrapDenialEvent, session: Session) => Promise<void>);

// 2. Sealed Execution Phase
const session: Session = await runtime.boot();
```

### 1.2 Rust SDK (`kelir`)
```rust
// Typestate Transition: KelirBuilder<Unset> -> KelirBuilder<Sealed> -> Session
let session: Session = KelirBuilder::new(vm_config)
    .plugin(pty_plugin)
    .plugin(vfs_plugin)
    .params::<SpawnAction>()
    .trap("git push *", handler(|c: HandlerCtx| async move {
        Ok(Decision::deny(1, "EPERM"))
    }))
    .trap("*", dual_handler(pre_hook, post_hook))
    .intercept_vfs("**/secrets/**", vfs_handler)
    .on_vm_ready(|session: &Session| async move { Ok(()) })
    .boot()
    .await?;
```

### 1.3 Python SDK (`kelir`)
```python
builder = (
    kelir.builder(vm_config)
    .plugin(PtyPlugin(cols=120, rows=40))
    .plugin(VfsPlugin(idmap=IdmapConfig(...)))
    .params(FixStripeWebhook)
)

builder.trap("git push *", git_gate)
builder.trap("*", pre=universal_step, post=verify_receipt)
builder.intercept("vfs:write", "**/secrets/**", vfs_gate)

@builder.on("vm:ready")
async def _ready(session: Session) -> None:
    pass

async with builder as session:
    await session.spawn("npm", ["test"])
```

---

## 2. All 15 First-Class Plugins: Flat Options Contracts

Any file instantiating or referencing these plugins must use these **flat config signatures**. Nested builder callbacks are completely illegal.

| Plugin | Package | Flat Config Interface / Struct Fields | Contributed Context Key |
|---|---|---|---|
| `pty` | `@kelir/plugin-pty` | `{ cols: number; rows: number; dropOsc52: boolean; stripTitle: boolean; tokenShield?: TokenShieldConfig; prompts?: PromptRule[] }` | `c.term` |
| `vfs` | `@kelir/plugin-vfs` | `{ idmap: IdmapConfig; sharedCaches?: string[]; lockBroker?: boolean; syntheticFiles?: SyntheticFile[]; maskRead?: MaskRule[]; snoops?: SnoopRule[] }` | `c.fs` |
| `wire` | `@kelir/plugin-wire` | `{ sinkhole?: SinkholeConfig; sshAgentBridge?: boolean; vaults?: VaultEntry[]; l4Proxies?: L4Proxy[] }` | `c.wire` |
| `chaos` | `@kelir/plugin-chaos` | `{ network?: NetworkChaosConfig[]; fs?: FsChaosConfig[] }` | `c.chaos` |
| `ledger` | `@kelir/plugin-ledger`| `{ store: string; merkleChain: boolean; otel?: OtelConfig; receipts?: ReceiptConfig }` | `c.ledger` |
| `db-proxy` | `@kelir/plugin-db-proxy` | `{ postgres?: PgWireRewrite; credentials?: EphemeralCredentials; sqlite?: SqliteOverVsock }` | `c.db` |
| `mesh` | `@kelir/plugin-mesh` | `{ peerIdentity: PeerIdentity; mtls: boolean; traceparent: TraceParentMode; listeners: PeerListener[] }` | `c.mesh` |
| `lock-broker` | `@kelir/plugin-lock-broker` | `{ scope: LockScope; defaultMode: LockMode; nonBlocking: boolean }` | `c.locks` |
| `supply-chain` | `@kelir/plugin-supply-chain` | `{ managers: PackageManager[]; policy: LifecyclePolicy; trapOnAncestorChain: boolean }` | *None (Pure Gate)* |
| `hyper-density` | `@kelir/plugin-hyper-density` | `{ idleDetect: IdleDetectConfig; compaction: CompactionConfig; resumeBudgetMs: number }` | `c.density` |
| `cas-snapshot` | `@kelir/plugin-cas-snapshot` | `{ chunking: FastCDCConfig; tier: S3TierConfig }` | `c.cas` |
| `wire-ingress` | `@kelir/plugin-wire-ingress` | `{ listen: ListenConfig; parking: ParkingConfig; wakeOnPacket: WakeConfig; backlog: BacklogConfig }` | `c.ingress` |
| `mcp` | `@kelir/plugin-mcp` | `{ transport: TransportConfig; advertise: { tools: string[] } }` | `c.mcp` |
| `oci-translator` | `@kelir/plugin-oci-translator` | `{ image: string; layerPolicy: LayerPolicyConfig; inFlight: boolean }` | *None (Daemon)* |
| `openshell-coordinator` | `@kelir/plugin-openshell-coordinator` | `{ tenantId: string; checkpointPolicy: CheckpointPolicyConfig; gpu?: GpuBinding }` | *None (Daemon)* |

---

## 3. Comprehensive File-by-File Mutation Matrix

### Group A: Core Architecture Spec (`cans/`)
* **`cans/00-overview.md`**:
  * Line 42: Replace `.before("*")` and prompt responder regex references with `trap("*")` and `pty({ prompts })`.
* **`cans/01-architecture.md`**:
  * Node `Params ingress contract`: Replace any `.guard()` reference with `session.params(schema)` validated out-of-band via paravirtual configuration device before guest instruction execution.
  * Node `In-process microkernel`: Remove references to userland "web-like middleware". State: "Interposition occurs directly at VirtIO device complex via synchronous native gates and asynchronous trap handlers."
* **`cans/02-plugin-model.md`**:
  * Node `Static dispatch`: Define the dispatch array explicitly: VirtIO Rings $\to$ MountScope pre-filter $\to$ Monotonic Lattice Evaluation (`Allow < Rewrite < Deny < Abort`). Remove `.before()` and `.after()`.
  * Node `Declarative plugin macros`: Remove "macro expansion into HTTP-like routing". State: "Macros expand to static MountScope masks and binary trap registrations."
* **`cans/03-sdk-architecture.md`**:
  * Replace the entire subsection `TypeScript SDK`: Document the fluent `.use().params().trap().intercept()` model.
  * Replace `Rust SDK`: Document typestate transitions without `.exec(|cmd| ...)`.
  * Define **Idiomatic Surfaces Law**: TS, Python, and Rust do not copy each other's method names; they enforce 1:1 behavioral and wire parity against `vectors.json`.
* **`cans/07-lifecycle.md`**:
  * Node `Hypervisor primitives`: Anchor the typestate boundary. Primitives (`fork`, `suspend`, `rollback`, `skew_clock`) exist exclusively on `Session`, never on `KelirBuilder`.
  * Node `Lifecycle vetoes`: Enforce hard vetoes (`Fork`, `Suspend`, `ClockSkew`) via `LifecycleCtx.register_veto` in `on_vm_init`.
* **`cans/08-epistemic.md`**:
  * Node `Step evaluation loop`: Completely scrub `.before("*")` and `.after("*")`. Reframe as:
    ```
    1. Trap match triggers pre-hook: checkpoint creation & intent analysis.
    2. Guest executes process.
    3. Process exit triggers post-hook: physical ExecutionReceipt assertion.
    4. Mismatch triggers instant <8ms hypervisor rollback.
    ```
* **`cans/09-protocol.md`**:
  * Node `Trap fast-path`: Rewrite the 3 gates: Gate 1 (in-kernel eBPF allowlist) $\to$ Gate 2 (native MountScope/schema eval) $\to$ Gate 3 (trap handler dispatch).
  * Node `Wire messages`: Explicitly map `TrapAction` to context methods:
    * `Allow` $\to$ `c.resume()`
    * `Deny` $\to$ `c.deny(errno, msg)`
    * `Rewrite` $\to$ `c.rewrite({ binary, argv, env })`
    * `Yield` $\to$ `c.yield(checkpoint_id)`
    * `Abort` $\to$ `c.abort(exit_code, reason)`

### Group B: Peripheral Plane Specs (`cans/`)
* **`cans/04-terminal-plane.md`**:
  * Update terminal pipeline specs: All 5 filters configured strictly via `PtyConfig` flat struct.
* **`cans/05-storage-plane.md`**:
  * Node `Write gating`: Remove middleware terminology. Replace with synchronous VirtIO-FS interception: `on_vfs_write` yielding `SyncVerdict::Allow` or `PluginError::Reject`.
* **`cans/06-wire-plane.md`**:
  * Frame TAP bridge as L2/L4 interception points dispatched via `.intercept("net:outbound", ...)`.
* **`cans/10-adapters.md`**:
  * Validate container-shim, MCP stdio, and OpenShell bindings map strictly to `Session` execution and flat plugin configurations.
* **`cans/11-operations.md`**:
  * Node `Multi-language parity gate`: Rewrite invariant: "Parity is evaluated on identical wire frame inputs and outputs using `vectors.json`. Syntax is idiomatic per language runtime."

### Group C: Plugin Catalog (`cans/plugins/`)
Audit all 15 files (`cans/plugins/*.md`):
* [ ] Verify that every code example uses flat config initialization.
* [ ] Ensure all references point to `c.<key>` according to the Table in Section 2.
* [ ] Assert that every line adheres to the 90-character node length limit (`_rules.yaml`).

### Group D: Artifacts & Code Examples (`cans/artifacts/`)

* **`cans/artifacts/code/typescript-accumulation.ts`**:
  * Purge `.configure((c) => ...)`, `.guard()`, `.before()`.
  * Rewrite to:
    ```ts
    const runtime = kelir({ vcpus: 4, memoryMb: 4096 })
      .use(secureWorkspace)
      .use(ledger({ enabled: false }))
      .use(repoPolicyPlugin({ maxDiffBytes: 64 * 1024 }))
      .params(t.Object({ action: t.Literal("fix-webhook"), issueId: t.String() }))
      .trace((t) => { if (t.ffi_hop_ns > 10_000) console.warn("FFI boundary spike:", t); })
      .trap("git commit *", async (c) => {
        if (c.policy.violations > 0) {
          return c.deny(1, `EPOLICY: ${c.policy.violations} violation(s)`);
        }
        return c.resume();
      });
    ```

* **`cans/artifacts/code/step-evaluation-loop.ts`**:
  * Purge `.exec((cmd) => cmd.before(...).after(...))`.
  * Rewrite to dual-trap pattern:
    ```ts
    const session = await kelir({ vcpus: 2, memoryMb: 2048 })
      .use(vfs())
      .use(pty())
      .trap("*", {
        pre: async (c) => {
          activeCheckpoint = await c.vm.checkpoint("pre-step");
          const intent = await orchestrator.analyzeIntent({
            binary: c.trap.binary_path, argv: c.trap.argv, lineage: c.trap.ancestor_chain,
          });
          return intent.isHypothesisValid ? c.resume() : c.deny(1, intent.falsificationReason);
        },
        post: async (c) => {
          const receipt = await c.receipt();
          if (!await orchestrator.verifyPhysicalReceipt(receipt)) {
            if (activeCheckpoint) await c.vm.rollback(activeCheckpoint);
            c.term.notice("⚡ [VERIFICATION ENGINE]: Reverted.\n");
          }
        }
      })
      .boot();
    ```

* **`cans/artifacts/code/rust-handle-injection.rs`**:
  * Remove `.exec(|exec| exec.before(...))`.
  * Replace with:
    ```rust
    let builder = builder
        .params::<Action>()
        .trap("npm test", handler(|c: HandlerCtx| async move {
            let params = c.params::<Action>()?;
            Ok(Decision::Resume)
        }))
        .trap("git commit *", handler(|c: HandlerCtx| async move {
            if c.policy().violations().await > 0 {
                return Ok(Decision::deny(1, "Policy violations recorded"));
            }
            Ok(Decision::Resume)
        }));
    ```

* **`cans/artifacts/code/python-raii-pydantic.py`**:
  * Replace `builder.exec(lambda cmd: cmd.before(...))` with:
    ```python
    builder.trap("git commit *", commit_gate)
    ```

* **`cans/artifacts/examples/autonomous-swe-agent.ts`**:
  * Complete overhaul of the single linear chain:
    * Flat `pty({ ... })` configuration.
    * Flat `vfs({ ... })` configuration.
    * Flat `wire({ ... })` configuration.
    * Direct `.params(t.Object({ ... }))`.
    * Direct `.trap("*", { pre: ..., post: ... })`.
    * Direct `.trap("npm test -- --runInBand", ...)`.
    * Direct `.trap("git push *", ...)`.
    * Direct `.trap("git commit *", { pre: ..., post: ... })`.
    * Lifecycle hooks registered via `.on("vm:init", ...)`, `.on("vm:ready", ...)`, etc.

* **`cans/artifacts/examples/serverless-webhook.ts`**:
  * Flatten `wireIngress`, `hyperDensity`, `mcp`, and `ledger` to pure objects.
  * Replace `.exec((cmd) => cmd.guard(...).before(...).after(...))` with:
    ```ts
    .params(t.Object({ action: t.Literal("dispatch-webhook"), webhookId: t.String() }))
    .trap("dispatch-webhook", {
      pre: async (c) => {
        await c.vm.adjust_balloon(512);
        return c.resume();
      },
      post: async (c) => {
        const receipt = await c.receipt();
        return receipt.dirtyBytes === 0 ? PostExecAction.RollbackAndHalt : PostExecAction.Proceed;
      }
    })
    ```

* **`cans/artifacts/examples/autonomous-swe-agent.rs`**:
  * Purge `.exec(|exec| ...)` graph block.
  * Flatten `.trap()` registrations into root `KelirBuilder`.

* **`cans/artifacts/examples/autonomous-swe-agent.py`**:
  * Purge `_register_handlers(cmd: kelir.ExecChain)`.
  * Register traps directly: `builder.trap("*", pre=..., post=...)`.

* **`cans/artifacts/examples/db-backed-runtime.py`**:
  * Purge `_register_exec_graph(cmd: kelir.ExecChain)`.
  * Use `builder.trap("migrate *", pre=pre_migrate, post=post_migrate)`.

---

## 4. Parallel Agent Swarm Allocation (Collision-Free)

| Agent Thread | Target Scope | Output Deliverables |
|---|---|---|
| **Agent Alpha** | Spec Core: `cans/01-architecture.md`, `cans/02-plugin-model.md`, `cans/03-sdk-architecture.md`, `cans/07-lifecycle.md`, `cans/09-protocol.md` | Complete, non-commented, updated markdown files adhering to `<90` char node length. |
| **Agent Beta** | Peripheral Planes & Ops: `cans/00-overview.md`, `cans/04-terminal-plane.md`, `cans/05-storage-plane.md`, `cans/06-wire-plane.md`, `cans/08-epistemic.md`, `cans/10-adapters.md`, `cans/11-operations.md` | Purged of all web-middleware terminology. CANS anchors fully validated. |
| **Agent Gamma** | Plugin Specs: All 15 files in `cans/plugins/*.md` | Flat configuration structures enforced; legacy callback references eliminated. |
| **Agent Delta** | TypeScript Artifacts: `typescript-accumulation.ts`, `step-evaluation-loop.ts`, `autonomous-swe-agent.ts`, `serverless-webhook.ts` | Fully compilable, production-ready TS code excerpts; zero lambda configs. |
| **Agent Epsilon** | Rust & Python Artifacts: `rust-handle-injection.rs`, `python-raii-pydantic.py`, `autonomous-swe-agent.rs`, `autonomous-swe-agent.py`, `db-backed-runtime.py`, `multi-vm-swarm.rs` | Idiomatic typestate and Pythonic trap registrations across all examples. |

---

## 5. Automated Gate Checklists

Every agent must run and pass these exact verification checks before finalizing their partition:

```bash
# 1. Zero Tolerance for HTTP Router Metaphors
! rg -q "\.exec\(" cans/
! rg -q "cmd\.before" cans/
! rg -q "cmd\.after" cans/
! rg -q "ExecChain" cans/
! rg -q "\.guard\(" cans/

# 2. Zero Tolerance for Nested Lambda Builders
! rg -q "\.use\(pty\(\(" cans/
! rg -q "\.use\(vfs\(\(" cans/
! rg -q "\.use\(wire\(\(" cans/
! rg -q "\.use\(hyperDensity\(\(" cans/

# 3. Zero Tolerance for Archaeological Notes
! rg -qi "formerly" cans/
! rg -qi "migrated" cans/
! rg -qi "deprecated" cans/
! rg -qi "replaces the old" cans/

# 4. Canonical Specification Verification
cans check --refs-only
cans check
```

**Execution Order:** Proceed immediately with full-file rewrites. Do not summarize. Do not truncate. Emit complete, final-state documents.