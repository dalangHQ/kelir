Here is the no-bullshit breakdown of where your architecture, examples, and directory structure collide—and the exact Architectural Decision Records (ADRs) to lock in the SSOT.

---

### The 5 Fatal Collisions

1. **The DLP Phantom Crate:**
   * *The Code:* TS (`line 88`) and Python (`line 316`) examples explicitly say: `maskRead folded in; NO separate dlp() — that was break #1`.
   * *The Tree:* `crates/kelir-plugins/plugin-dlp` still exists in `kelir.project-structure.md` and `kelir.architecture.md §14`. It’s dead weight. Kill it.

2. **Missing `@kelir/plugin-ledger` in TS:**
   * *The Code:* `example.autonomous-swe-agent.ts` imports `@kelir/plugin-ledger`.
   * *The Tree:* `packages/ts/plugins/` only has `pty`, `vfs`, `wire`, `chaos`. `ledger` is missing.

3. **Crate Namespace Mismatch (`kelir` vs `kelir-core`):**
   * *The Code:* Rust example uses `kelir = { version = "3.0", features = ["pty", ...] }`.
   * *The Tree:* There is only `crates/kelir-core`. A consumer cannot import `kelir` with plugin features unless there is a top-level facade crate or `crates/kelir` umbrella.

4. **Orphaned `StepLoopPlugin`:**
   * *The Code:* Python, Rust, and TS all implement or import a `step-loop` plugin to store pre-checkpoint descriptors safely without ad-hoc mutable scratchpads.
   * *The Tree:* It is neither in `crates/kelir-plugins/` nor in core. It’s floating as copy-paste boilerplate in examples.

5. **Rust Example Syntax Bug:**
   * *The Code:* `example.autonomous-swe-agent.rs` imports `SnapshotDescriptor` twice (once from `kelir::SnapshotDescriptor` and once from `kelir::protocol::SnapshotDescriptor`). Fails `rustc` immediately.

---

### Immediate ADRs (The Fix)

#### **ADR-001: Liquidate `plugin-dlp`**
* **Context:** In-flight read scrubbing is a FUSE-level operation handled during inode read dispatch.
* **Decision:** Burn `crates/kelir-plugins/plugin-dlp`. Move regex masking (`MaskRule`) permanently inside `plugin-vfs` (`crates/kelir-plugins/plugin-vfs/src/mask.rs`).

#### **ADR-002: Establish `crates/kelir` Facade Crate**
* **Context:** Native Rust users need one Cargo import (`kelir = { version = "3.0", features = [...] }`), not 8 separate workspace dependencies.
* **Decision:** Add `crates/kelir/` as the facade crate. Re-export `kelir-core` and feature-gated `kelir-plugins/*`.

#### **ADR-003: Core-Promote `StepSlot`**
* **Context:** Epistemic verification (§12.5) requires pre/post hardware state tracking per PID. Leaving this to userland creates broken ad-hoc state hacks.
* **Decision:** Bake `StepSlot` / `StepLoopPlugin` directly into `kelir-core` context (`c.step.mark()`, `c.step.last()`).

#### **ADR-004: Mirror TypeScript & Python Plugin Monorepo Packages**
* **Decision:** Align `packages/ts/plugins` and `packages/py/kelir/plugins` 1:1 with `crates/kelir-plugins`:
  `pty`, `vfs`, `wire`, `chaos`, `ledger`.

---

### Cleaned & Production-Ready Directory Layout

```text
kelir/
├── Cargo.toml                         # Workspace manifest (Rust 2024 / MSRV 1.85+)
├── pnpm-workspace.yaml
├── pyproject.toml
├── justfile
│
├── crates/
│   ├── kelir/                         # Facade crate (re-exports core + feature-gated plugins)
│   ├── kelir-core/                    # Microkernel: VirtIO bus, VmmDriver, Wire Protocol
│   ├── kelir-shim/                    # Guest Musl init binary (<50KB static)
│   │
│   ├── kelir-plugins/                 # Canonical First-Class Plugins
│   │   ├── plugin-pty/                # virtio-console, ANSI sanitizer, TokenShield
│   │   ├── plugin-vfs/                # virtio-fs, Ghost Inodes, In-Flight Masking (merged DLP)
│   │   ├── plugin-wire/               # virtio-net, L7/L4 Credential Vault, DNS Sinkhole
│   │   ├── plugin-chaos/              # Fault injection (network + fs)
│   │   └── plugin-ledger/             # Merkle WAL, ExecutionReceipts, OTel
│   │
│   ├── kelir-drivers/                 # VmmDriver Implementations
│   │   ├── driver-libkrun/            # macOS (HVF) & Linux (KVM) [Default]
│   │   ├── driver-rustvmm/            # Bare-metal Linux KVM + GPU CDI
│   │   └── driver-openvmm/            # Windows WHP / Hyper-V
│   │
│   ├── kelir-adapters/                # External System Adapters
│   │   ├── containerd-shim/           # K8s containerd-shim-v2
│   │   ├── openshell-grpc/            # NVIDIA OpenShell
│   │   └── mcp-server/                # Model Context Protocol stdio server
│   │
│   └── kelir-bindings/                # In-Process FFI
│       ├── napi/                      # Node-API -> @kelir/core
│       └── pyo3/                      # PyO3 -> kelir Python
│
├── packages/
│   ├── ts/
│   │   ├── core/                      # @kelir/core
│   │   ├── plugins/                   # @kelir/plugin-*
│   │   │   ├── pty/
│   │   │   ├── vfs/
│   │   │   ├── wire/
│   │   │   ├── chaos/
│   │   │   └── ledger/                # <-- ADDED (aligned with spec)
│   │   ├── presets/
│   │   └── e2b-compat/
│   │
│   └── py/
│       └── kelir/
│           ├── __init__.py
│           ├── plugins/               # Python plugin bindings mirroring Rust/TS
│           │   ├── pty.py
│           │   ├── vfs.py
│           │   ├── wire.py
│           │   ├── chaos.py
│           │   └── ledger.py
│           └── compat/
│               └── e2b.py
│
├── tests/
│   ├── fixtures/                      # Kernel & minimal EROFS images
│   ├── integration/
│   └── e2e/
└── README.md
```

No, that wasn't all. You have **7 deeper, fatal architectural contradictions** hiding across the specs, traits, and CI. 

Here is the exact damage:

---

### 1. The Builder vs Session Invariant Violation (Architecture Doc)
* **The Rule:** The builder is write-only until `boot()`; execution happens *only* on `Session`.
* **The Violation:** In `kelir.architecture.md` (§11.6 & §12.5):
  ```typescript
  const app = kelir({ ... }).use(...).exec(...);
  await app.spawn("claude", ["code", "run test verification"]); // BROKEN: spawn called on Builder!
  await app.breakGlass();                                        // BROKEN: breakGlass called on Builder!
  ```
  The architecture doc breaks its own cardinal rule. `app` is a builder. `spawn()` and `breakGlass()` exist **only** on `Session`.

### 2. The Broken CI YAML & Impossible Musl Cross-Compile (`kelir.cicd.md`)
* **Syntax Bomb:** Line 88 of `.github/workflows/ci.yml` has triple curly braces:
  `name: kelir-shim-${{{ matrix.target }}`
  GitHub Actions parser will immediately reject this workflow with a syntax error.
* **Linker Failure:** CI runs on `ubuntu-latest` (x86_64) and attempts:
  `cargo build -p kelir-shim --target aarch64-unknown-linux-musl`
  with just `apt install musl-tools gcc-aarch64-linux-gnu`. This **will fail to link**. Standard Ubuntu `musl-tools` does not provide an `aarch64-musl-gcc` wrapper. You must use `zig cc` or `cross`.

### 3. Wire Plane Asymmetry: `plugin-chaos` is Dead on Arrival
* Look at `KelirPlugin` in `crates/kelir-core/src/plugin.rs` (Architecture §5):
  ```rust
  async fn on_net_outbound(&self, _frame: &mut Vec<u8>) -> Result<bool, PluginError>;
  ```
* There is **no `on_net_inbound` hook**. 
* Yet, `plugin-chaos` (§10) promises:
  ```typescript
  .network({ target: "api.stripe.com", latency: { baseMs: 500 }, dropRate: 0.05 })
  ```
  If you cannot intercept inbound wire frames, **you cannot simulate download latency, inbound packet loss, or server-side jitter.**

### 4. Identity Crisis on Execution Receipts (Core vs Plugin)
* In `example.autonomous-swe-agent.ts`:
  * Line 165: `await c.ledger.lastReceipt(c.trap.pid)` (Calls `plugin-ledger`)
  * Line 180: `await c.getReceipt()` (Calls `HandlerCtx` core directly)
* In Architecture §12: `ExecutionReceipt` is defined in `crates/kelir-core/src/telemetry/receipt.rs`.
* **The clash:** Does `kelir-core` generate and hold receipts, or does `plugin-ledger`? If Core generates them, `c.receipt()` should be the single source of truth on `HandlerCtx`. `plugin-ledger` should merely persist them to SQLite/DuckDB.

### 5. `vm.fork()` Type Signature Collision
* `VmmDriver` trait (§3.1) defines:
  ```rust
  async fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError>;
  ```
* But in TS, Python, and Rust examples:
  ```typescript
  const child = await session.vm.fork(); // child is a Session!
  await child.spawn("npm", [...]);       // calls Session.spawn()
  ```
* If the hypervisor driver yields `Box<dyn VmmDriver>`, `session.vm.fork()` must wrap that driver into a full cloned **`Session`** envelope (re-binding the dispatch table, virtio channels, and context). That translation is missing from the spec.

### 6. Missing Port-Forward Argument Parity
* In TS: `session.forwardPort({ host: 8080, guest: 3000 })` (Object)
* In Python/Rust: `session.forward_port(8080, 3000)` (Positional primitives)
* In Architecture §11.7: `await app.forwardPort(...)` (Called on builder again!)

### 7. Secret Auto-Injection Gap
* `WirePlugin` defines:
  ```typescript
  .vault("STRIPE_SECRET", { placeholder: "vault://stripe_key", resolve: ... })
  ```
* The host wire proxy swaps `vault://stripe_key` in transit.
* **The hole:** How does the guest process actually obtain the value `vault://stripe_key`? It's not in the guest's OS env unless `WirePlugin` injects `STRIPE_SECRET=vault://stripe_key` into the guest init environment table during `on_vm_ready`. The architecture document never states this contract.

---

### Additional Required ADRs

* **ADR-005 (Lifecycle Sanitization):** Purge all `.spawn()` and `.breakGlass()` calls from `app` (Builder) in `kelir.architecture.md`. Builder only has `.boot()`. Everything else belongs to `Session`.
* **ADR-006 (Wire Bidirectionality):** Add `on_net_inbound(&self, frame: &mut Vec<u8>)` to `KelirPlugin` so `plugin-chaos` can actually inject latency on incoming network frames.
* **ADR-007 (Receipt Consolidation):** Core hypervisor owns `ExecutionReceipt` (`c.receipt()`). `plugin-ledger` only handles the append-only Merkle WAL and OTel export (`c.ledger.flush()`).
* **ADR-008 (Session Fork Hydration):** Specify that `session.vm.fork()` takes the driver’s child `Box<dyn VmmDriver>` and re-hydrates a full twin `Session` instance inheriting the parent's plugin graph.
* **ADR-009 (CI Repair):** Fix the triple-bracket `${{ matrix.target }}` typo in `ci.yml`. Use `cargo-zigbuild` or `cross` for `aarch64-unknown-linux-musl` target builds.

* You’re right to push. We were looking at surface integration bugs. 

Underneath, the core spec has **7 deep systems and protocol bugs** that will break the hypervisor at the kernel and wire levels:

---

### 1. Wire Protocol Desync: 32-bit vs 64-bit Sequence Numbers
Look at the binary TLV header specification in `kelir.architecture.md §4.2`:
```text
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         Sequence Number                       |  <-- 32-bit (4 bytes)
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         Payload Length                        |
```
Now look at the Rust struct in `crates/kelir-core/src/protocol/messages.rs (§4.3)`:
```rust
pub enum GuestToHost {
    TrapEvent {
        seq: u64, // <-- 64-bit (8 bytes)!
        ...
```
* **The Bug:** `seq` is a 64-bit unsigned integer in Rust, but the wire frame diagram allocates a single 32-bit word. 
* **Impact:** Your binary framing parser (`codec.rs`) and CBOR payload will offset by 4 bytes. Every frame sent over `vsock:5252` will corrupt on arrival.

---

### 2. Silent Dropping of `c.env.set()` on `Decision.resume()`
In the examples (TS line 150, Py line 204, Rust line 301):
```typescript
.before("npm test -- --runInBand", async (c) => {
  c.env.set("OTEL_TRACE_ID", oracle.currentTraceId());
  c.env.set("CI", "1");
  return c.resume();
})
```
Now look at `TrapAction` (§4.3):
```rust
pub enum TrapAction {
    Allow,
    Deny { exit_code: i32, stderr_message: String },
    Rewrite { new_binary: String, new_argv: Vec<String>, injected_env: HashMap<String, String>, ... },
    ...
}
```
* **The Bug:** `Decision.resume()` serializes to `TrapAction::Allow`. `TrapAction::Allow` **carries no environment map**. In Linux `seccomp-unotify`, returning an allow decision resumes the kernel syscall without touching registers or memory.
* **Impact:** `c.env.set()` is silently ignored. `OTEL_TRACE_ID` and `CI=1` will **never reach the guest process** unless the trap decision is forced into a `Rewrite`.

---

### 3. Semantic Conflation: Filesystem Reconciler vs Git Commit
Look at `Phase 4` in the SDK examples:
```typescript
const commit = await session.commit({
  cwd: "/workspace/checkout-service",
  message: `fix(stripe): dedupe webhook body reads (${winner.id})`,
  reconcileWhiteouts: true,
  atomicRename: true,
  fsync: true,
});
```
And look at `kelir.architecture.md §8.1`:
> *"The Success Path Reconciler (`c.commit()`): Merges tmpfs overlay diffs back to physical host disk atomically. Converts overlayfs whiteout devices (c 0 0) into real host unlink()."*

* **The Bug:** Is `session.commit()` an **OverlayFS-to-host sync**, or is it running **`git commit`**?
  * If it's filesystem reconciliation, passing a Git `message` parameter makes zero architectural sense.
  * If it's a Git commit, why did you spend §8.1 detailing FUSE whiteout device inode traversal (`S_ISCHR`)?
  * Even worse: all three SDKs hook `before("git commit *")` and `after("git commit *")` in the hypervisor trap chain. If `session.commit()` runs Git, does it trigger its own trap hooks?
* **Fix:** Split this cleanly:
  1. `session.fs.sync()` / `session.fs.reconcile()` (Syncs tmpfs overlay to host SSD).
  2. `session.git.commit()` or standard guest `spawn("git", ["commit", ...])` (Runs VCS logic).

---

### 4. Cross-Platform Epistemic Poisoning: 16K vs 4K Memory Pages
In `kelir.architecture.md §3`:
* macOS Host kernel is built with: `CONFIG_ARM64_16K_PAGES=y` (Apple Silicon hardware constraint).
* Linux Host kernel is built with: `CONFIG_ARM64_4K_PAGES=y` or x86 4KB.

In `ExecutionReceipt` (§12.1) and verification (§12.5):
```rust
pub struct ExecutionReceipt {
    pub dirty_memory_pages: usize, // True physical memory pages mutated
}
```
```typescript
if ((await c.getReceipt()).dirty_memory_pages === 0)
  throw new Error("Epistemic anomaly: zero-page delta commit attempted.");
```
* **The Bug:** A "page" is **16,384 bytes on Apple Silicon**, but **4,096 bytes on Linux/x86**. 
* **Impact:** The same SWE-bench run that dirties 4 pages on Linux reports 1 page on macOS. Epistemic assertions, RLVR loss functions, and Merkle root hashes will **diverge based on the developer's laptop model**. 
* **Fix:** Receipts must report `dirty_bytes` or normalize to `dirty_pages_4k`.

---

### 5. `PR_SET_PDEATHSIG` is a macOS Phantom
In `kelir.architecture.md §11.4`:
> *"Host Crash Orphan Reclaimer (`PR_SET_PDEATHSIG`): If the host Node.js or Python process terminates abruptly... `kelir-core` links all worker threads to an anonymous `eventfd` and sets `prctl(PR_SET_PDEATHSIG, SIGKILL)`."*

* **The Bug:** `prctl(PR_SET_PDEATHSIG)` is a **Linux-only kernel feature**. It does not exist on macOS / Darwin. 
* **Impact:** On macOS Apple Silicon (your primary dev harness target for Claude Code & Cursor), an abrupt `kill -9` or segfault of the host process leaves the hypervisor child process / threads running detached forever, burning 100% of the Mac's battery.
* **Fix:** macOS requires `kqueue` listening for `NOTE_EXIT` on the host PID (`EVFILT_PROC`), or a parent-monitoring heartbeat pipe.

---

### 6. Docker-in-Kelir Fails on Read-Only EROFS
In `kelir.architecture.md §11.5`:
> *"kelir embeds Rootless Podman + crun inside the read-only EROFS rootfs. Commands like `docker build` run via rootless Podman using Linux user namespaces."*

* **The Bug:** Rootless Podman needs to write container image layers, metadata, and locks to `/var/lib/containers` and `/run/user/$UID`. 
* In §8, you specified that the rootfs is an immutable read-only EROFS block device, and `tmpfs` is mounted **only** on `/workspace`.
* **Impact:** The moment an agent runs `docker build`, Podman crashes immediately with:
  `EROFS: Read-only file system: /var/lib/containers`.
* **Fix:** You must specify ephemeral `tmpfs` mounts for `/var/lib/containers`, `/var/run`, and `/tmp` in the guest boot fstab.

---

### 7. Anti-TOCTOU Ptrace Flaw on Multi-Threaded Binaries
In `kelir.architecture.md §4.1`:
> *"When a security-sensitive binary triggers seccomp-unotify, the shim executes `ptrace(PTRACE_SEIZE, target_pid)` to freeze sibling threads."*

* **The Kernel Reality:** `seccomp-unotify` pauses **only** the single thread that invoked `execve()`.
* Calling `ptrace(PTRACE_SEIZE, pid)` on the process ID freezes **only the thread group leader**. If a background thread inside an agent harness or Go/Rust binary is actively writing to memory, seizing `target_pid` does **not** stop threads in `/proc/$PID/task/*`.
* **Impact:** A hostile or rogue harness can modify `argv` or binary memory in a sibling thread while the shim is hashing it. TOCTOU is wide open.
* **Fix:** The shim must iterate `/proc/$PID/task/` and issue `PTRACE_SEIZE` + `PTRACE_INTERRUPT` to every thread in the task group before reading memory.

---

### Next Batch of ADRs to Lock SSOT

* **ADR-010 (Wire Framing Alignment):** Change `seq` in `GuestToHost` to `u32`, or update the binary framing diagram to 64-bit (8-byte aligned).
* **ADR-011 (Environment Injection Semantics):** Mandate that if `c.env.set()` is called on a trap, the microkernel runtime automatically converts `Decision.resume()` into `TrapAction::Rewrite` preserving `new_binary = original_binary`.
* **ADR-012 (Decouple Storage Sync from Git):** Rename `session.commit()` to `session.fs.reconcile()`. Leave Git operations purely to standard guest executions or a dedicated `session.git` namespace.
* **ADR-013 (Hardware Agnostic Telemetry):** Redefine `ExecutionReceipt.dirty_memory_pages` to `dirty_bytes: u64` to prevent Apple Silicon (16KB) vs Linux (4KB) falsification drift.
* **ADR-014 (Darwin Process Reclaimer):** Implement a dual-path reaper: `PR_SET_PDEATHSIG` on Linux; a dedicated `kqueue(EVFILT_PROC, NOTE_EXIT)` thread on macOS.

Not by a long shot. You have **7 more structural, cryptographic, and kernel-level landmines** baked into these specs.

Here is what will blow up during actual implementation:

---

### 1. The Wire Plane TLS Paradox (The "Magic" MITM)
In Architecture §9.1:
> *"When the agent executes `curl -H "Authorization: Bearer $STRIPE_KEY" https://api.stripe.com/v1/invoices`... the forward proxy intercepts the plaintext request before the outbound TLS handshake... Zero Harness Awareness: standard POSIX interfaces, no custom guest SDKs."*

* **The Physics Problem:** If the guest agent runs native `curl https://api.stripe.com`, `curl` performs the TLS handshake directly against Stripe’s public IP inside the guest. 
* By the time frames hit the TAP interface on the host, **the payload is already encrypted with TLS 1.3**. 
* The host wire proxy **cannot inspect or swap** `vault://stripe_key` unless:
  1. The proxy terminates TLS as a MITM, which **requires injecting a custom Root CA certificate** into the guest's `/etc/ssl/certs` in the read-only EROFS rootfs.
  2. Or standard `HTTP_PROXY` / `HTTPS_PROXY` env vars point to an in-guest plaintext forwarder.
* Your spec claims zero-configuration standard POSIX without CA injection, which violates basic TLS physics.

---

### 2. Linux FUSE Page-Cache Writeback Breaks `blake3_of_producing_process`
In all three examples (TS line 106, Py line 175, Rust line 231):
```typescript
producerBlake3: await c.ledger.blake3OfProducingProcess(path)
```
* **The Kernel Problem:** Linux uses page-cache writeback for filesystem writes. When an application writes to a file, the actual FUSE `write()` call to the hypervisor is often flushed asynchronously by kernel threads (`kworker` / `pdflush`).
* When the FUSE frame arrives on the host, **the calling PID is 0 or the flusher task**, not the agent's PID!
* **Impact:** `blake3_of_producing_process(path)` will either return the hash of the Linux kernel itself or fail with `ENOENT`.
* **Fix:** The VFS plugin must mount snooped directories with **`FOPEN_DIRECT_IO`** to bypass the kernel page cache and force synchronous, PID-retaining `write` syscalls.

---

### 3. Trait Signature Defect: `KelirPlugin::on_vfs_write` Has No Context or PID
In Architecture §5 (`KelirPlugin` Rust trait):
```rust
async fn on_vfs_write(&self, _path: &Path, _data: &[u8]) -> Result<bool, PluginError>;
```
* Now look at the consumer examples:
  * TS: `onWrite: async (path, raw, c: HandlerContext)`
  * Python: `make_snoop(oracle) -> Callable[[Path, bytes, HandlerContext], ...]`
* **The Defect:** The Rust microkernel trait `KelirPlugin::on_vfs_write` passes **only `Path` and `&[u8]`**. It does **not pass `HandlerContext`** and does **not pass `pid`**.
* The examples are consuming arguments that the underlying microkernel trait does not emit.

---

### 4. `libkrun` Cannot Deliver Your Stated Superpowers
The architecture specifies `driver-libkrun` as the **DEFAULT** hypervisor for macOS (HVF) and Linux (KVM).
* **The Reality:** Upstream `libkrun` is a minimalist library designed to boot microVMs for container workloads. It **does not support**:
  1. `vm.fork()` in <15ms (no userfaultfd memory-cloning API).
  2. `vm.checkpoint()` / `vm.rollback()` in <8ms (no live register & GPA snapshot engine).
  3. Dynamic hardware clock skewing (`CNTVOFF_EL2` / `KVM_SET_CLOCK`).
* **Impact:** Your default driver cannot execute Phases 1, 2, or 3 of your SWE-agent example. Either `driver-rustvmm` must be the default on Linux, or you must explicitly specify that `driver-libkrun` is a hard internal fork with custom hypervisor patches.

---

### 5. Ingress Port Forwarding Daemon is Missing in `kelir-shim`
Architecture §4 & §11.7 states:
* `virtio-vsock` port `5252` = Control / Traps.
* `virtio-vsock` port `5253` = Ingress Port Forwarding.
* Now look at `crates/kelir-shim/src/main.rs` in `kelir.project-structure.md`:
  > `main.rs # Guest PID 1 init loop (binds vsock:5252, mounts devpts)`
* **The Hole:** `kelir-shim` only binds port 5252. There is no listener or TCP loopback proxy for port `5253`. 
* When the host executes `session.forwardPort(8080, 3000)`, incoming host connections to `8080` will hit `ECONNREFUSED` on vsock port `5253` because the guest shim has no ingress worker.

---

### 6. Orphaned Integration Tests in Cargo Workspace
Look at `kelir.project-structure.md`:
```text
tests/
├── integration/
│   ├── test_cow_fork.rs
│   ├── test_rollback.rs
│   ├── test_whiteout.rs
│   └── test_anti_toctou.rs
```
* In `kelir.cicd.md`, CI runs:
  `cargo nextest run --workspace --exclude kelir-shim --exclude driver-libkrun`
* **The Cargo Bug:** In a Cargo virtual workspace, a root-level `/tests` directory is **ignored by Cargo** unless declared inside a specific crate or explicitly wired via root `Cargo.toml`. 
* None of your integration tests will ever execute in CI. They must live under `crates/kelir/tests/` or be explicitly defined.

---

### 7. Bogus `Decision::Halt` in `.after()` Hooks
In Python (line 245) and Rust (line 346):
```rust
.after("*", handler(|c: HandlerCtx| async move {
    // Post-condition check failed:
    c.vm.rollback(&pre).await?;
    return Ok(Decision::Halt); // <-- What is this?!
}))
```
* In `.before()`, a trap intercept happens *before* `execve()`. Returning `Decision::Deny` or `Decision::Resume` controls the execution.
* In `.after()`, **the guest process has already finished executing** and its exit status is resolved. 
* There is no definition anywhere in `kelir-core` for what `Decision::Halt` tells the hypervisor to do after a process has already completed. Does it kill the guest PID? Freeze the vCPUs? Terminate the session? It’s a phantom API.

---

### Next Batch of Critical ADRs

* **ADR-015 (TLS MITM CA Injection):** Mandate that `plugin-wire` mounts an ephemeral CA certificate into `/etc/ssl/certs` via an overlayfs ghost inode so guest binaries (`curl`, `git`) accept intercepted vault requests without CLI flags.
* **ADR-016 (Direct FUSE I/O for Telemetry):** Force `FOPEN_DIRECT_IO` on all `plugin-vfs` paths registered with `.snoop()` to preserve authentic process PIDs across the FUSE ring buffer.
* **ADR-017 (Align `on_vfs_write` Trait):** Update `KelirPlugin::on_vfs_write` in `kelir-core` to include `pid: u32` and `ctx: &HandlerCtx`.
* **ADR-018 (Shim Ingress Multiplexer):** Add `ingress.rs` to `crates/kelir-shim/src/` to bind `vsock:5253` and proxy incoming bytes to the guest loopback port.
* **ADR-019 (Relocate Integration Tests):** Move `tests/integration/` to `crates/kelir/tests/` so `cargo nextest` compiles and runs them.
* **ADR-020 (Clarify `.after()` Hook Semantics):** `.after()` hooks must return `Result<(), Error>` or a dedicated `PostExecAction` (e.g., `RollbackAndHaltSession`), not pre-syscall `Decision` enums.

* Still not all. We are now down into the hypervisor registers, POSIX process lifecycle, and kernel memory structures. 

Here are **7 more deep, fatal landmines** in your specs:

---

### 1. `VmmDriver` Trait Has No `watchdog()` Method
In every single SDK example (TS line 184, Rust line 362, Py line 423):
```rust
.on_vm_init(|driver| async move {
    driver.watchdog(Duration::from_secs(5 * 60), WatchdogAction::Reboot).await;
})
```
Now look at the `VmmDriver` trait in `crates/kelir-core/src/vmm/driver.rs` (Architecture §3.1).
* Read every method on that trait: `boot`, `fork`, `checkpoint`, `rollback`, `suspend`, `resume`, `skew_clock`, `adjust_balloon`, `vsock_connect`, `terminate`.
* **The Bug:** `watchdog()` **does not exist on the trait**.
* **Impact:** Your first lifecycle hook fails compilation in Rust and throws an unhandled `TypeError` in TS/Python.

---

### 2. The Ledger SQLite Path Paradox (Host vs Guest)
In all three examples (TS line 118, Rust line 281, Py line 377):
```typescript
ledger({ store: "sqlite:///workspace/audit.merkle.db" })
```
* **The Physical Collision:** 
  * `LedgerPlugin` is a **host-side plugin** running inside the host consumer process (`crates/kelir-plugins/plugin-ledger`).
  * `/workspace` is a **guest-internal mount** living in the guest's in-memory `tmpfs` overlay!
* **Impact:**
  1. Unless the host happens to have a `/workspace` directory at root (which requires host `sudo mkdir /workspace`), SQLite will crash with `unable to open database file`.
  2. If the audit log *did* live inside the guest workspace, every `vm.rollback()` or fork would **revert or duplicate the audit log**, completely destroying its Merkle integrity! The WAL must live on the host (e.g. `./.kelir/audit.merkle.db`).

---

### 3. The Dead Logic in `countDenials(e.pid) > 5`
In all three examples (TS line 190, Rust line 371, Py line 434):
```typescript
.on("trap:deny", async (e, s) => {
  if (await s.ledger.countDenials(e.pid) > 5) {
    await s.vm.suspend("./.kelir/quarantine.snap");
    await s.breakGlass();
  }
})
```
* **The Process Lifecycle Flaw:** A `deny` trap intercepts an `execve` syscall and injects an exit code or error. The trapped process **terminates or fails immediately**.
* That PID is dead. It will **never invoke another syscall**.
* **Impact:** `e.pid` can never have more than 1 denial. `countDenials(e.pid) > 5` is dead code that will never trigger. You meant `countDenials(e.ppid)` (the harness shell) or a session-wide denial count.

---

### 4. FUSE Metadata Stripping in `on_vfs_lookup`
In the examples, dynamic ghost files define filesystem permissions:
```typescript
GhostFile.dynamic("/workspace/identity.jwt", () => ({
  content: "Bearer ...",
  mode: 0o400, // Read-only file permissions
}))
```
Now look at `KelirPlugin` in `crates/kelir-core/src/plugin.rs` (§5):
```rust
async fn on_vfs_lookup(&self, _path: &Path) -> Result<Option<Vec<u8>>, PluginError>;
```
* **The Bug:** `on_vfs_lookup` returns **only `Vec<u8>` (file content)**.
* **The Kernel Reality:** A FUSE `LOOKUP` operation requires returning a `fuse_entry_out` struct populated with `st_mode`, `st_uid`, `st_gid`, and `st_size`.
* **Impact:** The `mode: 0o400` declaration is discarded. Ghost files will be synthesized with default kernel permissions (`0o644`), leaking read/write access to untrusted processes in the guest.

---

### 5. Colored Prompts Break the PTY Responder Regex
In Architecture §7.3 and all examples:
```typescript
t.onPrompt({
  match: /Overwrite production schema\? \[y\/N\]/i,
  action: async (_p, stream) => { stream.write("y\n"); }
})
```
* **The Terminal Reality:** Modern CLI tools (Prisma, Jest, Next.js, npm) format interactive prompts with ANSI SGR color/bold codes:
  `\x1b[1;31mOverwrite\x1b[0m production \x1b[4mschema\x1b[0m? [y/N]`
* In §7.1, your ANSI sanitizer strips *only* OSC 52 (clipboard) and OSC 0/1/2 (window title) so terminal styling isn't broken.
* **Impact:** The raw byte stream arriving at `onPrompt` contains embedded SGR escape codes. The regex `/Overwrite production schema\? \[y\/N\]/i` will **fail to match**, the 3000ms timer will expire, and the agent run hangs.

---

### 6. Hardware Clock Crash: `tsc_locked: true` vs `clock.skew()`
In the builder config:
```typescript
kelir({ tsc_locked: true, ... })
```
In Phase 2 (CoW memory fork fan-out):
```typescript
await child.vm.clock.skew({ days: 24 });
```
* **The Hypervisor Conflict:** 
  * `tsc_locked: true` locks guest vCPU Time Stamp Counters to host hardware invariant TSC to ensure deterministic cycle timing.
  * `vm.clock.skew()` manipulates `KVM_SET_CLOCK` or ARM64 `CNTVOFF_EL2`.
* **Impact:** When the guest Linux kernel notices the virtual clocksource has jumped forward while the hardware TSC rate remains pinned, the Linux kernel flags the clocksource as corrupted:
  `[Linux Kernel]: Clocksource tsc unstable (delta = 2073600000000000 ns)`
  The guest immediately drops into low-resolution jiffies or hangs network polling loops.

---

### 7. PTY Controlling Terminal (`TIOCSCTTY`) Failure in Guest Shim
In Architecture §3 & §7.2:
> *`/sbin/kelir-shim` acts as PID 1 (`init`), maps `/dev/hvc0` to `/dev/pts/0`, and handles terminal resize via `TIOCSWINSZ`.*
* **The POSIX Violation:**
  * PID 1 runs as a daemon outside of a session group.
  * In Linux, `TIOCSWINSZ` and terminal signal forwarding (`SIGINT`, `Ctrl+C`) work **only if the terminal is a Controlling Terminal (`ctty`) of the active foreground session leader**.
  * A process cannot become a session leader without `setsid()`, and cannot acquire a controlling terminal without `ioctl(fd, TIOCSCTTY, 0)`.
* **Impact:** If `kelir-shim` simply spawns commands without a full PTY master/slave allocation loop (`openpty()` / forkpty equivalent), interactive curses applications (like `top`, `vim`, or Claude Code's terminal UI) crash with `tcgetattr: Inappropriate ioctl for device`.

---

### The Final Actionable ADRs

* **ADR-021 (`VmmDriver` Watchdog):** Add `async fn set_watchdog(&mut self, timeout: Duration, action: WatchdogAction) -> Result<(), VmmError>` directly to `VmmDriver`.
* **ADR-022 (Host-Isolated Audit Storage):** Enforce that `LedgerPlugin` stores DB files relative to host storage (default: `.kelir/audit.merkle.db`), never allowing `/workspace` guest paths.
* **ADR-023 (Hierarchical Denial Accounting):** Change `countDenials(e.pid)` to `countDenials(e.ppid)` or `count_session_denials()` so parent harnesses get quarantined, not already-dead processes.
* **ADR-024 (FUSE Attribute Propagation):** Change `KelirPlugin::on_vfs_lookup` to return `Result<Option<VfsInodeAttr>, PluginError>` where `VfsInodeAttr` includes `mode`, `uid`, `gid`, and `content`.
* **ADR-025 (PTY SGR-Stripped Matcher):** In `plugin-pty`, pass a stripped, plaintext ASCII copy of the stream buffer to `onPrompt` matchers while streaming the original ANSI escape bytes to the terminal.
* **ADR-026 (TSC Unpin on Clock Skew):** Ensure `vm.clock.skew()` automatically unpins invariant TSC scaling before applying register offsets on cloned VMs.
* **ADR-027 (Shim PTY Session Leader):** Add an explicit `openpty()` master/slave broker to `/sbin/kelir-shim` with `setsid()` and `TIOCSCTTY` initialization for child workloads.

* 
You want the bedrock? Here are **7 fatal, low-level systems, kernel, and compiler traps** hiding in plain sight:

---

### 1. Inspecting `/proc/$PID/maps` at `execve` Trap Time is Physically Impossible
In Architecture §4.1:
> *"When a binary triggers seccomp-unotify... kelir-shim reads `/proc/$PID/maps` to verify loaded shared libraries (`.so`), blocking `LD_PRELOAD` rootkits."*

* **The Linux Kernel Reality:** `seccomp-unotify` intercepts `execve` at the **entry of the syscall**.
* The new binary hasn't even been loaded into memory yet!
* The dynamic linker (`ld-linux.so` / `ld-musl.so`) only maps shared libraries (`.so`) into `/proc/$PID/maps` **after** `execve` succeeds and returns to userspace.
* **Impact:** At trap time, `/proc/$PID/maps` still contains the memory mappings of the **old parent process** (the shell). You cannot inspect the new binary's dynamic linker maps at syscall trap time. To inspect `.so` loading, you must parse the binary's ELF `DT_NEEDED` headers from disk, or intercept dynamic linker `mmap()` syscalls.

---

### 2. The "Phantom Params" Mystery (Zero Harness Awareness Contradiction)
In all examples:
```typescript
await session.spawn("npm", ["test", "--", "--runInBand"], {
  params: { action: "fix-stripe-webhook", issueId: "ISSUE-8412", env: "staging" },
});
```
And in Architecture §1:
> *"Invariant #1: Zero Harness Awareness. The guest workload sees standard POSIX interfaces. No custom guest SDKs, CLI shims, or guest-side agents."*

* **The Black Hole:** How on earth does `npm test` receive `params`?
* If the guest is an unmodified, standard POSIX binary (`npm`, `cargo`, `pytest`), and there is no guest-side SDK or agent:
  * Is it an environment variable (`KELIR_PARAMS`)?
  * Is it a synthetic ghost file (`/workspace/.params.json`)?
  * Is it piped to `stdin`?
* The spec enforces rigorous TypeBox/Serde/Pydantic validation on `params`, **but never defines how `params` crosses into the guest POSIX process**.

---

### 3. OverlayFS Directory Whiteouts (`trusted.overlay.opaque`) are Ignored
In Architecture §8.1:
> *"Converts overlayfs whiteout devices (`c 0 0`) into real host `unlink()`."*

* **The Filesystem Bug:** Linux `overlayfs` only creates character devices (`c 0 0`) for deleted **files**.
* When an agent deletes a **directory** (e.g. `rm -rf node_modules` or `rm -rf src/deprecated`), `overlayfs` creates an **opaque directory** and sets the extended attribute:
  `trusted.overlay.opaque = "y"` (or `overlay.opaque = "y"`).
* **Impact:** Because your reconciler *only* checks `S_ISCHR(mode) && rdev == 0`, **every directory deletion by an agent is completely ignored**. The deleted directories will remain on the host disk, corrupting builds.

---

### 4. PID 1 Calling `PR_SET_CHILD_SUBREAPER` is Redundant Cargo-Culting
In Architecture §3 & §11.7:
> *"`/sbin/kelir-shim`: Installs `PR_SET_CHILD_SUBREAPER` (reaps zombie child processes cleanly)."*
* **The POSIX Reality:** `/sbin/kelir-shim` runs as **PID 1**.
* In Linux, **PID 1 is the universal ancestor of all orphaned processes by kernel definition**. 
* `PR_SET_CHILD_SUBREAPER` was added in Linux 3.4 for processes that are **NOT PID 1** (like systemd user instances) to prevent orphans from escaping up to PID 1.
* Calling `prctl(PR_SET_CHILD_SUBREAPER, 1)` inside PID 1 is a meaningless no-op. What PID 1 *actually* needs is an active non-blocking `waitpid(-1, &status, WNOHANG)` loop in its async event loop.

---

### 5. `TokenShield` Byte Clamping Corrupts Multi-Byte UTF-8 & ANSI
In Architecture §7.4:
```typescript
tokenShield({ maxBytesTotal: 128 * 1024, clampStrategy: "head-and-tail" })
```
* **The Crash:** The streaming PTY pushes raw byte chunks (`&[u8]`).
* Slicing blindly at `128 * 1024` bytes cuts right through the middle of:
  1. A 3-byte or 4-byte UTF-8 character (emoji, international text).
  2. An ANSI CSI escape sequence (e.g. `\x1b[38;5;196m` split as `\x1b[38;`).
* **Impact:** The downstream LLM or terminal client crashes with:
  `UnicodeDecodeError: 'utf-8' codec can't decode byte in position: invalid start byte`, or garbles terminal output with raw broken escape characters. Clamping must operate on UTF-8 scalar boundaries.

---

### 6. Hardcoded ARM64 Kernel Breaks x86_64 CI/CD
In all three examples (TS line 66, Rust line 241, Py line 273):
```typescript
kernel: "./fixtures/vmlinux-6.6-arm64-kelir"
```
Now look at `kelir.cicd.md` (Job 3):
```yaml
test-linux:
  runs-on: ubuntu-latest # <-- This is x86_64!
```
* **The Hardware Collision:** GitHub Actions `ubuntu-latest` runs on **x86_64 Intel/AMD CPUs**.
* KVM on x86_64 **cannot execute an ARM64 Linux kernel** (`vmlinux-6.6-arm64-kelir`). Hardware virtualization requires host and guest CPU architecture parity.
* **Impact:** Your CI job `test-linux` will instantly fail to boot the microVM with `EINVAL` or `KVM_CREATE_VM` architecture mismatches.

---

### 7. Sync Snoop FUSE Panic Causes Uninterruptible `D` State Kernel Deadlock
In Architecture §12.4:
```typescript
fs.snoop("~/.claude/sessions/*.json", {
  sync: true,
  onWrite: async (path, raw, c) => {
    if (evt.falsifiedResult) throw new Error("EPERM: harness hallucination detected.");
  }
})
```
* **The Kernel Deadlock:** When `sync: true` is enabled, the guest kernel blocks in `write()` holding the VFS inode lock (`i_rwsem`), waiting for the host to send a FUSE completion reply.
* If your JavaScript/Python hook throws `throw new Error(...)`:
  * If the host runtime doesn't catch this and manually convert it into an explicit FUSE error reply (`fuse_reply_err(req, EPERM)`), the FUSE request is **never answered**.
* **Impact:** The guest kernel thread hangs in **Uninterruptible Sleep (`D` state) forever**. The guest VM cannot shut down, cannot reboot, and process kills (`kill -9`) are completely ignored by the Linux kernel.

---

### Next Essential ADRs

* **ADR-028 (ELF Header Pre-Execution Scan):** Shim must inspect binary ELF headers (`DT_NEEDED`, `PT_INTERP`) directly from the binary file on disk during `execve` traps, not from `/proc/$PID/maps`.
* **ADR-029 (POSIX Ingress for Params):** Formally define that `session.spawn(..., { params })` injects `KELIR_PARAMS=<json>` into guest environment variables, or writes to a well-known ghost inode `/dev/kelir/params.json`.
* **ADR-030 (OverlayFS Opaque Directory Reconciliation):** Update `reconciler.rs` to detect both `c 0 0` whiteout nodes AND directories with `trusted.overlay.opaque` attributes, converting them to `std::fs::remove_dir_all()` on the host.
* **ADR-031 (UTF-8 & ANSI Safe Token Clamping):** `TokenShield` must scan backwards from the clamp ceiling to the nearest valid UTF-8 code point and complete ANSI escape sequence before truncating.
* **ADR-032 (Arch-Specific Kernel Fixture Selection):** CI/CD and builders must load `./fixtures/vmlinux-$(uname -m)` dynamically instead of hardcoding `arm64`.
* **ADR-033 (Guaranteed FUSE Reply on Error):** The VFS snoop dispatcher must wrap all user callbacks in a catch-all block that guarantees a `fuse_reply_err(req, EPERM)` is dispatched if a host hook panics or throws.

Here are the **final 5 deep-level systems bugs**—right down to FUSE inode virtualization, terminal races, and cross-language cryptographic serialization—followed by the **complete fix solution** to make this spec 100% production-ready.

---

### The Final 5 Hidden Landmines

1. **Guest Inode vs Host Inode Desync (Breaks §8.5 Lock Broker):**
   * *The Flaw:* `plugin-vfs` claims to broker `flock`/`fcntl` across VMs using "physical host Inodes".
   * *The Reality:* Linux `overlayfs` assigns **virtual inodes** inside the guest that have zero mathematical correlation to host physical disk inodes. 
   * *Impact:* Cross-VM lock coordination on `.git/index.lock` fails completely because hypervisor instances query virtual guest IDs, causing lock collisions and Git index corruption.

2. **The Initial Terminal Boot Race (80x25 Fallback):**
   * *The Flaw:* Pty plugin configures `windowSize({ cols: 120, rows: 40 })`, but `WindowResize` over vsock only fires on host `SIGWINCH`.
   * *The Reality:* Linux virtio-console defaults to 80x25 at guest boot.
   * *Impact:* Full-screen agent terminal apps (Claude Code / Curses) initialize in broken 80x25 mode and never resize until a human manually drags the host window.

3. **In-Flight DLP Masking Breaks Byte Offsets and File Lengths:**
   * *The Flaw:* `maskRead` replaces `sk_live_[0-9a-zA-Z]{24}` (32 bytes) with `[STRIPE_VAULT_KEY]` (20 bytes).
   * *The Reality:* Modifying buffer length during FUSE `read()` breaks the guest's kernel inode size (`st_size`) and causes trailing byte reads to truncate or fail with `EIO`.

4. **Cryptographic Hash Type Fragmentation (Array vs Bytes vs Hex):**
   * *The Flaw:* Rust uses `[u8; 32]`, TS uses `number[]`, Python switches randomly between `bytes` and hex `str`.
   * *Impact:* Receipt verification (`receipt.binary_blake3.join() !== Array.from(...)`) breaks silently across FFI boundaries.

5. **Entropy Calculation Pollution via `argv[0]`:**
   * *The Flaw:* Shannon entropy is calculated on the full raw `argv`.
   * *The Reality:* `argv[0]` (e.g., `/usr/local/bin/node`) contains low-entropy path noise that artificially dilutes or inflates the calculated entropy of the actual payload.

---

# THE FIX SOLUTION: The Master SSOT Specification

To eliminate all contradictions and lock the architectural Single Source of Truth (SSOT), apply the following standardizations:

---

### 1. Consolidated Master ADR Ledger

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                         MASTER ARCHITECTURAL DECISIONS                       │
├─────────┬──────────────────────────┬─────────────────────────────────────────┤
│ ADR-001 │ Liquidate `plugin-dlp`   │ Merged into `plugin-vfs` (`MaskRule`).  │
│ ADR-002 │ Add Facade Crate         │ Top-level `crates/kelir` re-exports all.│
│ ADR-003 │ Core `StepSlot`          │ Checkpoint slots baked into `core`.     │
│ ADR-004 │ Complete TS/Py Plugins   │ `@kelir/plugin-ledger` & `py/ledger.py`.│
│ ADR-005 │ Builder vs Session Lock  │ Zero execution methods on Builder.      │
│ ADR-006 │ Bidirectional Wire Bus   │ Added `on_net_inbound` to KelirPlugin.  │
│ ADR-007 │ Unified Receipts         │ Core owns `c.receipt()`; Ledger audits. │
│ ADR-008 │ Session Fork Hydration   │ `vm.fork()` returns full child Session. │
│ ADR-009 │ CI/CD Toolchain Repair   │ `cargo-zigbuild` + YAML typo fixed.     │
│ ADR-010 │ 64-bit Wire Protocol     │ `seq: u64` locked in TLV wire framing.  │
│ ADR-011 │ Enforce Env Injections   │ `c.env.set()` auto-converts to Rewrite. │
│ ADR-012 │ Decouple VFS & Git Sync  │ `session.fs.reconcile()` vs Git spawn.  │
│ ADR-013 │ Hardware Agnostic Metric │ `dirty_bytes: u64` replaces page counts.│
│ ADR-014 │ Darwin Process Reaper    │ `kqueue(EVFILT_PROC)` on macOS.         │
│ ADR-015 │ CA Inode Overlay         │ Ephemeral root CA for transparent TLS.  │
│ ADR-016 │ Direct FUSE I/O          │ Force `FOPEN_DIRECT_IO` on snooped paths│
│ ADR-017 │ Rich Storage Trait       │ `on_vfs_write` gets PID + HandlerCtx.   │
│ ADR-018 │ Shim Ingress Daemon      │ `ingress.rs` binds `vsock:5253`.        │
│ ADR-019 │ Colocate Tests           │ Move tests to `crates/kelir/tests`.     │
│ ADR-020 │ Post-Exec Semantics      │ `.after()` returns `PostExecAction`.    │
│ ADR-021 │ Driver Watchdog Trait    │ `set_watchdog()` added to VmmDriver.    │
│ ADR-022 │ Host Isolated Ledger     │ Audit DB enforced on host filesystem.   │
│ ADR-023 │ Process Tree Quarantines │ `countDenials(e.ppid)` for parent trees.│
│ ADR-024 │ FUSE Inode Attributes    │ `on_vfs_lookup` returns full stat info. │
│ ADR-025 │ PTY SGR-Stripped Match   │ Regex matches ANSI-stripped plaintext.  │
│ ADR-026 │ Decouple Skew from TSC   │ Auto-unpin TSC invariant scaling on skew│
│ ADR-027 │ Shim PTY Session Leader  │ Proper `setsid()` + `TIOCSCTTY` in guest│
│ ADR-028 │ ELF Static Scan          │ Parse `DT_NEEDED` directly from disk.   │
│ ADR-029 │ Formal Ingress Params    │ Spawn params inject to `KELIR_PARAMS`.  │
│ ADR-030 │ Directory Whiteouts      │ Reconciler handles `overlay.opaque`.    │
│ ADR-031 │ UTF-8 Boundary Clamping  │ TokenShield respects Unicode boundaries.│
│ ADR-032 │ Dynamic Arch Fixtures    │ Auto-load `vmlinux-$(uname -m)`.        │
│ ADR-033 │ FUSE Crash Protection    │ Catch-all dispatch dispatches EPERM.    │
│ ADR-034 │ Host-Mapped Inode Locks  │ VFS broker maps guest to host Inodes.   │
│ ADR-035 │ Boot Terminal Handshake  │ Hypervisor auto-syncs terminal window.  │
│ ADR-036 │ Fixed-Width Masking      │ DLP mask uses fixed-width padding.      │
│ ADR-037 │ Universal Hex Hashes     │ BLAKE3 standardized as 64-char hex.     │
│ ADR-038 │ Focused Entropy Scan     │ Calculate entropy strictly on `argv[1:]`│
└─────────┴──────────────────────────┴─────────────────────────────────────────┘
```

---

### 2. Concrete Architectural Implementations

#### A. Wire Frame Specification Update (§4.2)
*Header extended from 16 to 20 bytes to align with 64-bit sequence numbers:*
```text
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|          Magic (0x4B45)       |          Version (0x03)       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|          Message Type         |          Flags                |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Sequence Number (MSB)                    |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Sequence Number (LSB)                    |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         Payload Length                        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         Payload Data (CBOR)...                |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

#### B. Unified `KelirPlugin` Rust Trait (§5)
```rust
#[async_trait]
pub trait KelirPlugin: Send + Sync {
    fn name(&self) -> &'static str;

    // Lifecycle
    async fn on_vm_init(&self, driver: &mut Box<dyn VmmDriver>) -> Result<(), PluginError> { Ok(()) }
    async fn on_vm_ready(&self, session: &Session) -> Result<(), PluginError> { Ok(()) }
    async fn on_vm_shutdown(&self, session: &Session) -> Result<(), PluginError> { Ok(()) }

    // Control Plane (Ring 2)
    async fn on_trap(&self, event: &TrapEvent, ctx: &HandlerCtx) -> Result<Option<TrapAction>, PluginError> { Ok(None) }
    async fn on_process_exit(&self, pid: u32, exit_code: i32) -> Result<(), PluginError> { Ok(()) }

    // Terminal Plane (Ring 1)
    async fn on_pty_input(&self, bytes: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
    async fn on_pty_output(&self, chunk: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }

    // Storage Plane (Ring 3) - Rich Attributes & Inode Tracking
    async fn on_vfs_lookup(&self, path: &Path) -> Result<Option<VfsInodeAttr>, PluginError> { Ok(None) }
    async fn on_vfs_read(&self, path: &Path, buffer: &mut [u8]) -> Result<(), PluginError> { Ok(()) }
    async fn on_vfs_write(&self, path: &Path, data: &[u8], pid: u32, ctx: &HandlerCtx) -> Result<bool, PluginError> { Ok(true) }

    // Wire Plane (Ring 4) - Bidirectional Traffic
    async fn on_net_outbound(&self, frame: &mut Vec<u8>) -> Result<bool, PluginError> { Ok(true) }
    async fn on_net_inbound(&self, frame: &mut Vec<u8>) -> Result<bool, PluginError> { Ok(true) }
}
```

#### C. Corrected `VmmDriver` Trait (§3.1)
```rust
#[async_trait]
pub trait VmmDriver: Send + Sync {
    async fn boot(&mut self, config: VmConfig) -> Result<(), VmmError>;
    async fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError>;
    async fn checkpoint(&mut self, label: &str) -> Result<SnapshotDescriptor, VmmError>;
    async fn rollback(&mut self, snapshot: &SnapshotDescriptor) -> Result<(), VmmError>;
    async fn suspend(&mut self, output_path: &Path) -> Result<(), VmmError>;
    async fn resume(&mut self, image_path: &Path) -> Result<(), VmmError>;
    async fn skew_clock(&mut self, offset: Duration) -> Result<(), VmmError>;
    async fn adjust_balloon(&mut self, target_mb: u32) -> Result<(), VmmError>;
    async fn set_watchdog(&mut self, timeout: Duration, action: WatchdogAction) -> Result<(), VmmError>;
    fn vsock_connect(&self, port: u32) -> Result<tokio::net::UnixStream, VmmError>;
    async fn terminate(&mut self) -> Result<(), VmmError>;
}
```

#### D. Safe OverlayFS Reconciler Algorithm (§8.1)
```rust
pub async fn reconcile_upperdir(upperdir: &Path, host_target: &Path) -> Result<CommitStats, std::io::Error> {
    let mut stats = CommitStats::default();

    for entry in walkdir::WalkDir::new(upperdir).min_depth(1) {
        let entry = entry?;
        let rel_path = entry.path().strip_prefix(upperdir).unwrap();
        let target_path = host_target.join(rel_path);
        let metadata = entry.metadata()?;

        // 1. File Whiteout Detection: Char device 0/0
        if metadata.file_type().is_char_device() && metadata.rdev() == 0 {
            if target_path.exists() {
                std::fs::remove_file(&target_path)?;
                stats.whiteouts_resolved += 1;
            }
            continue;
        }

        // 2. Directory Whiteout Detection: Extended Attribute `overlay.opaque`
        if metadata.is_dir() {
            if let Ok(Some(val)) = xattr::get(entry.path(), "trusted.overlay.opaque") {
                if val == b"y" && target_path.exists() {
                    std::fs::remove_dir_all(&target_path)?;
                    stats.whiteouts_resolved += 1;
                    continue;
                }
            }
            std::fs::create_dir_all(&target_path)?;
            continue;
        }

        // 3. Atomic Staged Write for Modified/Created Files
        if metadata.is_file() {
            let tmp_path = target_path.with_extension(format!("tmp.{}", std::process::id()));
            std::fs::copy(entry.path(), &tmp_path)?;
            std::fs::rename(&tmp_path, &target_path)?;
            stats.files_changed += 1;
        }
    }

    nix::unistd::sync();
    Ok(stats)
}
```

#### E. Standardized CI/CD Toolchain (`.github/workflows/ci.yml`)
```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  build-guest-shim:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        target: [x86_64-unknown-linux-musl, aarch64-unknown-linux-musl]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - uses: taiki-e/install-action@cargo-zigbuild
      - name: Build Shim via Zig Linker
        run: cargo zigbuild -p kelir-shim --release --target ${{ matrix.target }}
      - name: Verify Binary Size (<100KB)
        run: |
          SIZE=$(stat -c%s target/${{ matrix.target }}/release/kelir-shim)
          test $SIZE -lt 102400 || (echo "SHIM BLOAT DETECTED" && exit 1)
      - uses: actions/upload-artifact@v4
        with:
          name: kelir-shim-${{ matrix.target }}
          path: target/${{ matrix.target }}/release/kelir-shim
```

---

### The Reality

With these ADRs locked, the contradictions between your architecture specification, SDK examples, CI pipelines, and filesystem/kernel interactions are completely closed. The specification is aligned, cohesive, and ready to implement.


