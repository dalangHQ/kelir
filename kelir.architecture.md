# `kelir.architecture.md` (Part 1/3)
**A Radical, Plugin-First MicroVM Hypervisor Middleware for Autonomous Systems**

*Version: 3.0.0-normative*  
*Classification: Systems Architecture Blueprint*  
*Status: Normative Specification*

---

## 1. Axiomatic Foundations & The 4-Ring Hardware Bus

`kelir` (Javanese: ꦏꦼꦭꦶꦂ) is the taut white fabric screen in traditional *Wayang Kulit* shadow puppetry. 

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           THE KELIR ARCHITECTURE                            │
│                                                                             │
│   DALANG  (Host Application) ──► Directs orchestrations, injects policy     │
│   PUPPETS (Untrusted Agent)  ──► Compiles, mutates, issues syscalls         │
│   KELIR   (The Screen)       ──► Microkernel Hypervisor & 4-Ring VirtIO Bus │
│                                  through which no puppet can break.         │
└─────────────────────────────────────────────────────────────────────────────┘
```

`kelir` is not a container wrapper, an MCP server, or a background daemon. It is an **embeddable in-process microVM hypervisor microkernel**. It boots stripped Linux microVMs in **sub-60ms** and mediates all interactions through a unified **4-Ring VirtIO Hardware I/O Bus**.

```
                           THE 4-RING VIRTIO BUS
                                     │
      ┌──────────────────┬───────────┴───────────┬──────────────────┐
      ▼                  ▼                       ▼                  ▼
 [virtio-vsock]  [virtio-console]          [virtio-fs]        [virtio-net]
  CONTROL PLANE   TERMINAL PLANE          STORAGE PLANE        WIRE PLANE
  Syscalls, PIDs, Raw TTY, ANSI,          CoW Overlays, Inodes, L4/L7 Traffic,
  Signals, Traps  Token Shield, Resizing  DLP Mask, Ghost Files Secrets, Chaos
```

### The 6 Core Invariants

1. **Zero Harness Awareness:** The guest workload sees standard POSIX interfaces (`/bin/sh`, `/dev/pts/*`, `glibc`/`musl`, TCP/IP). No custom guest SDKs, CLI shims, or guest-side agents are required.
2. **In-Process Microkernel (Zero Daemon Tax):** Virtualization runs inside the host consumer's memory space via direct C-FFI to `libkrun`, `rust-vmm`, or `OpenVMM`. There is no `dockerd`, no `containerd`, no background root socket, and no daemon maintenance.
3. **Pure Epistemic Agnosticism:** `kelir` has zero knowledge of "LLMs", "prompts", or "AI morals". It operates strictly as an operating system physics engine: memory pages, CPU registers, process lineages, file descriptors, and network frames.
4. **Deterministic Hardware Isolation:** The security boundary is hardware-enforced virtualization (Intel VT-x / AMD-V / Apple Silicon ARM64 EL2). An escaped guest root exploit cannot touch host memory or host kernel space.
5. **Radical Plugin-First Microkernel:** The hypervisor core does only two things: drive hardware virtualization and multiplex the 4 VirtIO rings. Every single capability (terminal sanitizing, secret vaulting, filesystem masking, chaos, audit logging) is a completely decoupled, composable plugin.
6. **Transparent, Truthful Steering:** No silent state divergence. Any intercepted syscall, file read, or network packet is either allowed, steered with mandatory terminal disclosure (`tty_notice`), or rejected with machine-parsable receipts.

---

## 2. Audience Target Matrix

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                                   TARGET AUDIENCE MATRIX                                │
├─────────────────────┬──────────────────────────┬────────────────────────┬───────────────┤
│ TARGET AUDIENCE     │ VALUE DELIVERED          │ INTEGRATION METHOD     │ CORE TRIGGER  │
├─────────────────────┼──────────────────────────┼────────────────────────┼───────────────┤
│ 1. INFRA PLATFORMS  │ Sub-60ms cold boot micro-│ `containerd-shim-v2`   │ VM escapes,   │
│    (K8s, Cube,      │ VMs; pluggable KVM/WHP/  │ `openshell-grpc`       │ daemon OOMs,  │
│     OpenShell)      │ HVF bare-metal drivers   │ (Unix Domain Socket)   │ multi-tenancy │
├─────────────────────┼──────────────────────────┼────────────────────────┼───────────────┤
│ 2. HARNESS CREATORS │ Virtio-console hardware  │ Drop-in `spawn()` shim │ Tmux breaks,  │
│    (Claude Code,    │ PTY; ANSI sanitizing;    │ Native MCP Server      │ token bleed,  │
│     Cursor, Devin)  │ automated prompt answers │ (`@kelir/mcp-server`)  │ PII leaks     │
├─────────────────────┼──────────────────────────┼────────────────────────┼───────────────┤
│ 3. ORCHESTRATORS    │ Hardware memory CoW fork;│ Drop-in E2B client SDK │ RAM exhaust,  │
│    (LangGraph,      │ 0MB RAM suspend/resume;  │ Elysia-style typed     │ slow tests,   │
│     CrewAI, E2B)    │ shared VirtIO-FS cache   │ plugin chains          │ cloud VM cost │
├─────────────────────┼──────────────────────────┼────────────────────────┼───────────────┤
│ 4. EPISTEMIC ENGINES│ Unforgeable execution    │ Hardware TSC offsets;  │ LLM hallucina-│
│    (Axiom, RLVR,    │ receipts; 8ms RAM roll-  │ Merkle-chained audit   │ tions, untaxed│
│     Falsifiers)     │ backs; dynamic chaos     │ ledger; CausalDelta    │ side effects  │
└─────────────────────┴──────────────────────────┴────────────────────────┴───────────────┘
```

---

## 3. Physical Architecture & Pluggable `VmmDriver` Substrate

`kelir` executes as a single host binary that directly drives guest virtualization across diverse operating systems and hypervisor platforms.

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                                 HOST PROCESS RUNTIME                                    │
│                                                                                         │
│  ┌───────────────────────────────────────────────────────────────────────────────────┐  │
│  │                    CONSUMER PLUGIN CHAIN (TypeScript / Rust / Python)             │  │
│  │    kelir().use(pty()).use(vfs()).use(wire()).use(chaos()).use(customPlugin())     │  │
│  └──────────────────────────────────────────┬────────────────────────────────────────┘  │
│                                             │ In-Process FFI (napi-rs / PyO3)           │
│                                             ▼                                           │
│  ┌───────────────────────────────────────────────────────────────────────────────────┐  │
│  │                        kelir-core (Stripped Microkernel)                          │  │
│  │   ├── BusDispatcher: Zero-copy multiplexer for the 4 VirtIO Rings                 │  │
│  │   ├── EventBus: Typed compile-time and runtime async hook pipeline                │  │
│  │   └── DriverRouter: Lifecycle controller mediating Box<dyn VmmDriver>             │  │
│  └──────────────────────────────────────────┬────────────────────────────────────────┘  │
│                                             │ Dynamic Driver Trait Interface            │
│                                             ▼                                           │
│  ┌───────────────────────────────────────────────────────────────────────────────────┐  │
│  │                           kelir-drivers (Pluggable Backends)                      │  │
│  │   ├── driver-libkrun (DEFAULT): macOS (HVF + Metal GPU) + Linux KVM               │  │
│  │   ├── driver-rustvmm: Bare-metal Linux KVM hypervisor + CUDA CDI GPU passthrough  │  │
│  │   └── driver-openvmm: Windows WHP + ConPTY + HV_SOCKET + Azure Linux MSHV         │  │
│  └───────┬──────────────┬──────────────┬──────────────┬──────────────┬───────────────┘  │
└──────────┼──────────────┼──────────────┼──────────────┼──────────────┼──────────────────┘
           │ virtio-      │ virtio-      │ virtio-      │ virtio-blk   │ virtio-rng /
           │ console      │ vsock        │ fs           │ (EROFS)      │ virtio-wdt /
           │ [Ring 1]     │ [Ring 2]     │ [Ring 3]     │              │ virtio-balloon
           ▼              ▼              ▼              ▼              ▼
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                             GUEST MICROVM (<60ms Cold Boot)                             │
│                                                                                         │
│   ├── Minimal vmlinux: Stripped Linux kernel (no ACPI, no PCI bloat)                   │
│   │     - macOS Host: Built with CONFIG_ARM64_16K_PAGES=y (Apple Silicon parity)       │
│   │     - Linux Host: Built with CONFIG_ARM64_4K_PAGES=y or x86 4KB standard           │
│   │                                                                                     │
│   ├── Hardware Paravirtualized Devices:                                                 │
│   │     - virtio-rng: Hardware entropy feeding /dev/urandom (no boot hangs)             │
│   │     - virtio-wdt: Hardware watchdog timer preventing guest kernel lockups           │
│   │     - virtio-balloon: Dynamic memory inflation/deflation under host pressure        │
│   │                                                                                     │
│   ├── Rootfs Mounting: Ultra-fast read-only EROFS block device (cold-mounted in <2ms)   │
│   │                                                                                     │
│   ├── /sbin/kelir-shim (Static 50KB Musl Binary):                                       │
│   │     - Installs PR_SET_CHILD_SUBREAPER (reaps zombie child processes cleanly)        │
│   │     - Installs seccomp-unotify filter with in-kernel BPF allowlist                  │
│   │     - Performs Anti-TOCTOU PTRACE_SEIZE execution pauses                            │
│   │     - Inspects Dynamic Linker maps (/proc/$PID/maps) & Shannon Entropy              │
│   │     - Maps guest PTY to /dev/hvc0 (virtio-console) & executes TIOCSWINSZ resize     │
│   │                                                                                     │
│   └── UNTOUCHED GUEST WORKLOAD: Claude Code / Cursor / SWE-bench / Node / Rust / Python │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.1 The Universal `VmmDriver` Trait

`kelir-core` decouples from hypervisor backends through the `VmmDriver` abstraction:

```rust
// crates/kelir-core/src/vmm/driver.rs

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct VmConfig {
    pub vcpus: u8,
    pub memory_mb: u32,
    pub kernel_path: PathBuf,
    pub rootfs_erofs_path: PathBuf,
    pub tsc_locked: bool,
    pub entropy_seed: Option<u64>,
    pub enable_gpu_passthrough: bool,
    pub enable_watchdog: bool,
}

#[derive(Debug, Clone)]
pub struct SnapshotDescriptor {
    pub id: String,
    pub dirty_pages: usize,
    pub created_at_ns: u64,
}

#[async_trait]
pub trait VmmDriver: Send + Sync {
    /// Cold-boots the microVM environment in <60ms via libkrun or active driver
    async fn boot(&mut self, config: VmConfig) -> Result<(), VmmError>;

    /// Branches the running VM into an isolated CoW clone in <15ms
    /// Linux: userfaultfd + KVM memory cloning
    /// macOS: HVF memory-mapped page duplication
    async fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError>;

    /// Creates an atomic in-memory checkpoint of guest RAM and vCPU registers
    async fn checkpoint(&mut self, label: &str) -> Result<SnapshotDescriptor, VmmError>;

    /// Reverts guest memory and registers to a prior checkpoint in <8ms
    async fn rollback(&mut self, snapshot: &SnapshotDescriptor) -> Result<(), VmmError>;

    /// Flushes dirty RAM pages to disk and drops host memory consumption to 0 MB
    async fn suspend(&mut self, output_path: &Path) -> Result<(), VmmError>;

    /// Restores a suspended VM image from storage in <50ms
    async fn resume(&mut self, image_path: &Path) -> Result<(), VmmError>;

    /// Offsets the virtual hardware clock (KVM_SET_CLOCK / CNTVOFF_EL2)
    async fn skew_clock(&mut self, offset: Duration) -> Result<(), VmmError>;

    /// Adjusts guest RAM ceiling dynamically via virtio-balloon
    async fn adjust_balloon(&mut self, target_mb: u32) -> Result<(), VmmError>;

    /// Yields raw duplex channel to virtio-vsock / HV_SOCKET multiplexer
    fn vsock_connect(&self, port: u32) -> Result<tokio::net::UnixStream, VmmError>;

    /// Terminates hypervisor instance and releases memory allocations
    async fn terminate(&mut self) -> Result<(), VmmError>;
}
```

---

## 4. In-Guest Substrate, Fast-Path BPF, Anti-TOCTOU & Wire Protocol

Host-guest coordination bypasses network interfaces and flows over **`virtio-vsock` (Port 5252 for Control/Traps, Port 5253 for Ingress/Port-Forwarding)**. On Windows, `driver-openvmm` translates this to **`HV_SOCKET`**.

### 4.1 In-Kernel BPF Allowlist & Anti-TOCTOU Inspection

Syscall trapping on every command causes fatal slowdowns on workloads like `cargo build` or `npm install`. To prevent this, the in-guest shim combines three defensive layers:

1. **In-Kernel BPF Fast-Path:** Standard compilers and benign utilities (`rustc`, `gcc`, `as`, `ld`, `sed`, `cat`) are evaluated purely inside the guest kernel via BPF filters. They execute with **0µs roundtrip overhead**.
2. **Anti-TOCTOU Execution Freeze:** When a security-sensitive binary (`git`, `curl`, `ssh`, `rm`, `sudo`, `npm`, or binaries from `/tmp`) triggers `seccomp-unotify`, the shim executes `ptrace(PTRACE_SEIZE, target_pid)` to freeze sibling threads. This prevents Time-of-Check to Time-of-Use race conditions where a concurrent thread rewrites memory buffers after inspection.
3. **Entropy & Dynamic Linker Audit:** Before forwarding the trap to the host, `/sbin/kelir-shim`:
   * Calculates Shannon Entropy on `argv`. Values **> 5.2** flag encrypted shell drops or base64 payloads.
   * Reads `/proc/$PID/maps` to verify loaded shared libraries (`.so`), blocking `LD_PRELOAD` rootkits.

```
GUEST KERNEL                               HOST HYPERVISOR (kelir-core)
Agent invokes execve("/bin/git", ["push"])
      │
      ▼
Kernel evaluates BPF filter ──► Is it gcc/rustc? ──► YES ──► In-kernel Fast Path (0µs)
      │ NO (Security-critical binary)
      ▼
Kernel seccomp-unotify triggers
      │
      ├──────────────────────────────┐
      │ Process frozen               │
      ▼                              ▼
/sbin/kelir-shim seizes PID     Shim extracts process metadata:
Freezes threads via PTRACE_SEIZE - BLAKE3 hash of binary
Reads /proc/$PID/mem safely      - Shannon entropy of arguments (>5.2)
Checks /proc/$PID/maps (.so)     - Full ancestor process DAG
      │                          - Working directory & envp
      ▼                              │
Pipes TLV packet to vsock:5252 ─────►│ Host receives TrapEvent
                                     │ Evaluates Plugin Event Bus
                                     │
                                     ├──────────────────────────────┐
                                     ▼                              ▼
                               ALLOW / REWRITE                     DENY
                                     │                              │
Pipes Decision to vsock:5252 ◄───────┘                              │
      │                                                             │
      ├── If Rewrite: Prints `tty_notice`                           │
      │   directly to /dev/pts/0                                    │
      ▼                                                             ▼
kelir-shim releases syscall                               Injects exit code + stderr
Workload resumes on vCPU                                  to agent TTY immediately
```

### 4.2 Wire Framing Specification

All frames over vsock port `5252` utilize binary Type-Length-Value (TLV) framing with CBOR payload encoding:

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|          Magic (0x4B45)       |          Version (0x03)       |  (0x4B45 = 'KE')
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|          Message Type         |          Flags                |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         Sequence Number                       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         Payload Length                        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         Payload Data (CBOR)...                |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

### 4.3 Protocol Wire Messages

```rust
// crates/kelir-core/src/protocol/messages.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub enum GuestToHost {
    /// Syscall Trap Interception
    TrapEvent {
        seq: u64,
        pid: u32,
        ppid: u32,
        binary_path: String,
        binary_blake3: [u8; 32],
        argv: Vec<String>,
        envp: HashMap<String, String>,
        cwd: String,
        ancestor_chain: Vec<String>,
        entropy: f64,
        loaded_libraries: Vec<String>,
    },
    /// Process Exit Notification
    ProcessExit {
        pid: u32,
        exit_code: i32,
        wall_clock_ns: u64,
    },
    /// Memory Pressure Alert from In-Guest Watcher
    MemoryPressureWarning {
        available_mb: u32,
        total_mb: u32,
    },
    /// Background Service Bound Port
    PortBound {
        guest_port: u16,
        protocol: String,
    },
    Heartbeat {
        uptime_ms: u64,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum HostToGuest {
    /// Decision on Intercepted Syscall
    TrapDecision {
        seq: u64,
        action: TrapAction,
    },
    /// Forwarding TTY Terminal Window Resize (SIGWINCH)
    WindowResize {
        rows: u16,
        cols: u16,
        xpixel: u16,
        ypixel: u16,
    },
    /// Signal Propagation (SIGINT, SIGTERM, SIGKILL)
    SendSignal {
        pid: u32,
        signal: u32,
    },
    /// Dynamic Memory Balloon Adjustment
    AdjustBalloon {
        target_mb: u32,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TrapAction {
    Allow,
    Deny {
        exit_code: i32,
        stderr_message: String,
    },
    Rewrite {
        new_binary: String,
        new_argv: Vec<String>,
        injected_env: HashMap<String, String>,
        /// Mandatory terminal disclosure notice printed to /dev/pts/0
        /// Guarantees Invariant #6: Zero silent state divergence
        tty_notice: Option<String>,
    },
    SuspendForApproval {
        poll_interval_ms: u32,
    },
}
```

---

*(Part 1 complete. Awaiting instruction to deliver Part 2: Radical Plugin-First Core Traits, The Elysia-Style Accumulating SDK Engine, Hardware Terminal Plane [virtio-console], and Zero-Leak Storage Plane with DLP Masking [virtio-fs]).*

# `kelir.architecture.md` (Part 2/3)
**A Radical, Plugin-First MicroVM Hypervisor Middleware for Autonomous Systems**

*Version: 3.0.0-normative*  
*Classification: Systems Architecture Blueprint*  
*Status: Normative Specification*

---

## 5. The Radical Plugin-First Rust Trait Architecture

`kelir-core` is a **stripped microkernel**. It contains zero hardcoded logic for terminal filtering, file redaction, credential vaulting, or chaos injection. 

The core binary maintains an internal asynchronous **Plugin Event Bus**. Every subsystem—internal or third-party—implements the unified `KelirPlugin` Rust trait across the 4 VirtIO hardware rings:

```rust
// crates/kelir-core/src/plugin.rs

use async_trait::async_trait;
use std::path::Path;
use crate::protocol::messages::{TrapEvent, TrapAction};
use crate::vmm::driver::VmmDriver;

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Plugin execution rejected: {0}")]
    Halt(String),
    #[error("Internal I/O failure: {0}")]
    Io(#[from] std::io::Error),
}

#[async_trait]
pub trait KelirPlugin: Send + Sync {
    /// Unique identifier for telemetry and ordering
    fn name(&self) -> &'static str;

    // ─── 1. MicroVM Lifecycle Hooks ─────────────────────────
    async fn on_vm_init(&self, _driver: &mut Box<dyn VmmDriver>) -> Result<(), PluginError> { Ok(()) }
    async fn on_vm_ready(&self) -> Result<(), PluginError> { Ok(()) }
    async fn on_vm_shutdown(&self) -> Result<(), PluginError> { Ok(()) }

    // ─── 2. Control Plane Hooks (virtio-vsock) ───────────────
    async fn on_trap(&self, _event: &TrapEvent) -> Result<Option<TrapAction>, PluginError> { Ok(None) }
    async fn on_process_exit(&self, _pid: u32, _exit_code: i32) -> Result<(), PluginError> { Ok(()) }

    // ─── 3. Terminal Plane Hooks (virtio-console) ────────────
    async fn on_pty_input(&self, _bytes: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
    async fn on_pty_output(&self, _chunk: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }

    // ─── 4. Storage Plane Hooks (virtio-fs) ──────────────────
    async fn on_vfs_lookup(&self, _path: &Path) -> Result<Option<Vec<u8>>, PluginError> { Ok(None) }
    async fn on_vfs_read(&self, _path: &Path, _buffer: &mut [u8]) -> Result<(), PluginError> { Ok(()) }
    async fn on_vfs_write(&self, _path: &Path, _data: &[u8]) -> Result<bool, PluginError> { Ok(true) }

    // ─── 5. Wire Plane Hooks (virtio-net) ────────────────────
    async fn on_net_outbound(&self, _frame: &mut Vec<u8>) -> Result<bool, PluginError> { Ok(true) }
}
```

### 5.1 Static Dispatch Table Compilation
To achieve sub-12 microsecond hook execution, `kelir-core` compiles registered plugins into a flat, contiguous dispatch array at boot. Plugins are evaluated in strict priority order. The first plugin returning `Some(TrapAction::Deny { .. })` or `Some(TrapAction::Rewrite { .. })` halts the pipeline and dispatches the packet immediately over `AF_VSOCK`.

---

## 6. The Elysia-Style Accumulating SDK Engine (`@kelir/core`)

The consumer SDK mirrors the microkernel: **`kelir()` by default is an empty execution context.** Every capability is mounted via `.use()`.

Context accumulates through TypeScript generic inference (`Kelir<TContext>`). If an upstream plugin, state variable, or guard is removed, **downstream handlers fail to compile at build time**. Zero `any` casting.

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                              TYPE CONTEXT ACCUMULATION                                 │
│                                                                                        │
│   kelir()                                                                              │
│     │ Context: {}                                                                      │
│     ▼                                                                                  │
│   .use(vfsPlugin)                                                                      │
│     │ Context: { fs: VfsController }                                                   │
│     ▼                                                                                  │
│   .use(stripeSandboxPlugin)                                                            │
│     │ Context: { fs: VfsController, store: { stripeInvoices: number }, getStripe() }   │
│     ▼                                                                                  │
│   .exec((cmd) => cmd.before("npm test", (c) => {                                       │
│       // c is 100% type-inferred: c.fs, c.store.stripeInvoices, c.getStripe()          │
│   }))                                                                                  │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

### 6.1 Plugin Authoring API (`createPlugin`)

Developers author reusable, self-contained plugins that configure the 4 hardware planes, store internal state, and decorate downstream contexts:

```typescript
// packages/core/src/plugin.ts
import { createPlugin } from "@kelir/core";

export interface StripePluginOptions {
  mockMode: boolean;
  apiKey: string;
}

export const stripePlugin = (opts: StripePluginOptions) =>
  createPlugin("stripe-plugin")
    // Declare plugin-scoped state variable
    .state("interceptedStripeCalls", 0)

    // Decorate downstream execution context with typed helper
    .decorate("getStripeTelemetry", (store) => ({
      calls: store.interceptedStripeCalls,
    }))

    // Wire Plane: Zero-Knowledge credential substitution
    .wire((w) =>
      w.vault("STRIPE_API_KEY", {
        placeholder: "vault://stripe_key",
        resolve: () => opts.apiKey,
        scopeUrls: ["https://api.stripe.com/*"],
      })
    )

    // Storage Plane: Mount ephemeral in-memory configuration
    .vfs((fs) =>
      fs.ghost("/root/.stripe/config.json", async () => ({
        content: JSON.stringify({ mock: opts.mockMode }),
        mode: 0o600,
      }))
    )

    // Control Plane: Intercept process execution
    .exec((cmd) =>
      cmd.before("stripe *", async (c) => {
        c.store.interceptedStripeCalls += 1;
        return c.resume();
      })
    );
```

### 6.2 Schema Enforcement on Shell Commands (TypeBox)

Parameters are validated against static and runtime TypeBox schemas:

```typescript
import { kelir, t } from "@kelir/core";
import { pty } from "@kelir/plugin-pty";
import { vfs } from "@kelir/plugin-vfs";

const app = kelir({ vcpus: 2, memoryMb: 1024 })
  .use(pty())
  .use(vfs())
  .exec((cmd) =>
    cmd
      .guard({
        schema: t.Object({
          action: t.Literal("deploy"),
          env: t.Union([t.Literal("staging"), t.Literal("production")]),
        }),
      })
      .before("./deploy.sh *", async (c) => {
        // c.params.env is typed: "staging" | "production"
        if (c.params.env === "production" && c.trap.uid !== 0) {
          return c.deny(1, "EPERM: Production deployments require root privileges.");
        }
        return c.resume();
      })
  );
```

---

## 7. The Terminal Plane: `virtio-console` Ring Architecture

AI harnesses (Claude Code, Cursor, Aider) operate within interactive terminals. `kelir` isolates terminal execution via **hardware `virtio-console` streaming, ANSI sanitization, signal synchronization, and token budget clamping**.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          HARDWARE TERMINAL PIPELINE                         │
│                                                                             │
│   GUEST WORKLOAD (Claude Code / Bash)                                       │
│     │ Writes stdout/stderr bytes                                            │
│     ▼                                                                       │
│   Linux Kernel PTY Driver (/dev/hvc0 -> /dev/pts/0)                         │
│     │ isatty(1) == true. Raw mode active. Zero CLI downgrade.               │
│     ▼                                                                       │
│   virtio-console Ring Buffer (Hardware Shared Memory)                       │
│     │ Direct byte streaming to Host Memory (Zero TCP/UDS overhead)          │
│     ▼                                                                       │
│   HOST SUPERVISOR (@kelir/plugin-pty)                                       │
│     │                                                                       │
│     ├── 1. ANSI Stream Sanitizer ─► Drops OSC 52 (Clipboard Hijack) &       │
│     │                             OSC 0/1/2 (Title spoofing) in-flight.     │
│     │                                                                       │
│     ├── 2. onPrompt Responder ────► Verifies authentic foreground TTY;      │
│     │                             matches prompts; auto-injects keystrokes. │
│     │                             (Kills tmux send-keys forever).           │
│     │                                                                       │
│     ├── 3. WindowResize Bridge ───► Host terminal resize fires SIGWINCH;    │
│     │                             sends WindowResize over vsock:5252;       │
│     │                             shim executes TIOCSWINSZ ioctl on guest.  │
│     │                                                                       │
│     ├── 4. Signal Forwarding ─────► Ctrl+C (SIGINT) / Ctrl+D forwarded to  │
│     │                             guest foreground process group.           │
│     │                                                                       │
│     └── 5. Token Shield Filter ───► Clamps chunks exceeding byte ceiling.  │
│                                   Injects truncation markers into stream.   │
│                                   (Protects LLM Context Window).            │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 7.1 In-Flight ANSI Stream Sanitizer
Untrusted scripts execute escape sequences to compromise host environments:
* **OSC 52 Attack Defense:** Scripts printing `\x1b]52;c;...` attempt to read or overwrite the host system clipboard. The PTY broker drops all OSC 52 bytes at the hardware buffer boundary.
* **Terminal Title Spoofing (OSC 0/1/2):** Stripped to prevent tampering with host terminal emulator titles.

### 7.2 Terminal Window Resizing (`SIGWINCH`) & Signal Forwarding
When an agent renders interactive curses interfaces or spinners, column and row mismatches corrupt output:
1. The host catches window adjustments via `process.stdout.on("resize")`.
2. A `HostToGuest::WindowResize { rows, cols, xpixel, ypixel }` packet is dispatched over `AF_VSOCK:5252`.
3. In-guest `/sbin/kelir-shim` executes `ioctl(pts_fd, TIOCSWINSZ, &ws)` on `/dev/pts/0`.
4. The guest shell automatically redraws layouts without terminal corruption.

### 7.3 Programmatic Prompt Responder (`onPrompt`) with Foreground Verification
Orchestrators avoid scraping terminal screens with regex. `onPrompt` triggers directly on the hardware stream chunk:

```typescript
app.use(
  pty((t) =>
    t.onPrompt({
      match: /Do you want to proceed\? \[y\/N\]/i,
      timeoutMs: 1500,
      action: async (prompt, stream) => {
        // Injects raw keystrokes directly into the virtio-console hardware ring:
        stream.write("y\n");
      },
    })
  )
);
```
* **Anti-Spoof Verification:** The prompt responder executes **only** if the process holding `/dev/pts/0` matches the kernel's active foreground process group (`tcgetpgrp()`). Background processes executing `echo "Do you want to proceed? [y/N]"` cannot trigger automated responses.

### 7.4 Token Shield (Context Window Bleed Protection)
When an agent dumps runaway logs or a 50,000-line stack trace, Token Shield clamps the stream before it consumes orchestrator context tokens:

```typescript
app.use(
  pty((t) =>
    t.tokenShield({
      maxBytesTotal: 64 * 1024,      // 64 KB ceiling per command execution
      maxLinesTotal: 300,            // 300 lines ceiling
      clampStrategy: "head-and-tail",// Keeps first 50 lines + last 100 lines
      onClamp: (stats) =>
        `\n⚡ [KELIR TOKEN SHIELD: Clamped ${stats.droppedBytes} bytes / ${stats.droppedLines} lines]\n`,
    })
  )
);
```

---

## 8. The Storage Plane: `virtio-fs` Ring Architecture

File operations flow over **`virtio-fs`** (directory sharing) and **`virtio-blk`** (rootfs), connecting the guest directly to host storage memory with zero TCP/UDS overhead.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       STORAGE & FILESYSTEM ARCHITECTURE                     │
├─────────────────────────────────────────────────────────────────────────────┤
│ 1. EROFS READ-ONLY ROOTFS (virtio-blk):                                     │
│    - Stripped base system (Python, Node, Rust, Bash) pre-baked into EROFS.  │
│    - Cold-mounted as block device in <2ms. Zero tarball decompression.      │
├─────────────────────────────────────────────────────────────────────────────┤
│ 2. COPY-ON-WRITE RAM SCRATCHPAD (overlayfs):                                │
│    - Base host workspace mounted STRICTLY READ-ONLY.                         │
│    - Guest kernel mounts an in-memory `tmpfs` over repo via `overlayfs`.    │
│    - Instant Revert: Drop overlayfs layer in 1ms. Host SSD is untouched.   │
├─────────────────────────────────────────────────────────────────────────────┤
│ 3. THE SUCCESS PATH RECONCILER (c.commit()):                                │
│    - Merges tmpfs overlay diffs back to physical host disk atomically.      │
│    - Converts overlayfs whiteout devices (c 0 0) into real host unlink().   │
│    - Zero filesystem corruption; clean git state preservation.              │
├─────────────────────────────────────────────────────────────────────────────┤
│ 4. UID / GID ID-MAPPING (idmap):                                            │
│    - Translates guest root (UID 0) and node (UID 1000) to host geteuid().   │
│    - Prevents files written by microVM from requiring host `sudo` to edit.  │
├─────────────────────────────────────────────────────────────────────────────┤
│ 5. SHARED HOST PACKAGE CACHES (virtio-fs read-only mounts):                 │
│    - Mounts host ~/.npm, ~/.cache/pip, ~/.cargo/registry into guest.        │
│    - Package installs drop from 2 minutes to <3 seconds on cold microVMs.   │
├─────────────────────────────────────────────────────────────────────────────┤
│ 6. SYNTHETIC GHOST INODES (RAM-Only Virtual Files):                         │
│    - Injects virtual configurations (.env, .gitconfig) directly into VFS.   │
│    - Served from host memory; physically DOES NOT EXIST on host SSD.        │
├─────────────────────────────────────────────────────────────────────────────┤
│ 7. IN-FLIGHT READ-TIME DLP MASKING:                                         │
│    - FUSE-level byte stream interception on file reads.                     │
│    - Matches patterns (sk_live_*, AKIA*, PEM keys); injects redactions.     │
├─────────────────────────────────────────────────────────────────────────────┤
│ 8. CENTRAL INODE LOCK BROKER (Cross-VM Concurrency):                        │
│    - Intercepts FUSE SETLK / SETLKW (flock / fcntl) across multiple VMs.    │
│    - Prevents .git/index.lock collisions when multiple agents share a repo. │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 8.1 The Success Path Reconciler (`c.commit()`)
When an agent mutates files in its in-memory `tmpfs` scratchpad, changes must merge back to physical disk atomically without corrupting the developer's Git index:
* **The OverlayFS Whiteout Problem:** Deleted files in Linux `overlayfs` are represented as special character devices with major/minor numbers `0/0` (`mknod c 0 0`). A naive file copy back to the host filesystem writes unusable, broken device nodes.
* **The Reconciler Algorithm:**
  1. Traverses the guest `tmpfs` `upperdir` differential tree.
  2. Detects whiteout device nodes (`S_ISCHR(mode) && rdev == 0`) and issues POSIX `unlink()` operations directly against the host workspace file path.
  3. Writes modified and newly created files to host disk using atomic staging dot-files (`.file.tmp.PID`), followed by `rename()` replacement.
  4. Flushes disk buffers with `fsync()`. Host Git state is updated cleanly with zero device node corruption.

### 8.2 FUSE UID/GID Dynamic ID-Mapping (`idmap`)
To resolve file ownership conflicts between guest and host:
* Workloads inside the guest run as `root` (UID `0`) or standard user (UID `1000`).
* On macOS hosts, the developer is UID `501`; on Linux hosts, UID `1000`.
* The `virtio-fs` host driver maps all incoming filesystem write requests:
  $$\text{Guest UID } 0 \longrightarrow \text{Host } \texttt{geteuid()}$$
  $$\text{Guest GID } 0 \longrightarrow \text{Host } \texttt{getegid()}$$
* Result: Any file modified or created by the agent remains immediately editable and removable by the developer on the host machine without invoking `sudo`.

### 8.3 Shared Host Package Caches
Cold-booting an agent that runs `npm install`, `cargo build`, or `pip install` typically spends minutes downloading duplicate packages from public registries. 
* `kelir` auto-mounts host package cache directories as cached `virtio-fs` mounts:
  * Host `~/.npm` $\longrightarrow$ Guest `/root/.npm`
  * Host `~/.cache/pip` $\longrightarrow$ Guest `/root/.cache/pip`
  * Host `~/.cargo/registry` $\longrightarrow$ Guest `/usr/local/cargo/registry`
* Cold dependency installations complete in **<3 seconds** via local host SSD caching.

### 8.4 In-Flight Read-Time DLP Masking (`maskRead`)
When an agent executes `cat /workspace/.env` or inspects configuration files, local secrets must not leak into guest memory:
* `virtio-fs` intercepts `read()` operations at the FUSE buffer boundary.
* Registered regex patterns scrub bytes in host RAM before copying to guest pages:

```typescript
app.use(
  vfs((fs) =>
    fs.maskRead("**/secrets.*", [
      { pattern: /sk_live_[0-9a-zA-Z]{24}/g, replace: "[REDACTED_STRIPE_KEY]" },
      { pattern: /AKIA[0-9A-Z]{16}/g, replace: "[REDACTED_AWS_KEY]" },
      { pattern: /-----BEGIN PRIVATE KEY-----[^-]+-----END PRIVATE KEY-----/gs, replace: "[REDACTED_RSA_KEY]" },
    ])
  )
);
```

### 8.5 Central Cross-VM Inode Lock Broker
When multiple agents in a swarm share the same repository:
* Concurrent invocations of `git status` or `git commit` collide on `.git/index.lock`. Standard `virtio-fs` does not synchronize POSIX locks across multiple independent hypervisor instances.
* `kelir-core` embeds a **Central Inode Lock Broker**:
  1. Intercepts `FUSE_SETLK` and `FUSE_SETLKW` (`fcntl`/`flock`) system calls.
  2. Routes lock acquisitions through a centralized host-side in-memory mutex table indexed by physical file Inode numbers.
  3. Conflicting lock attempts either block cleanly until release or return `EWOULDBLOCK`, preventing Git index corruption.

### 8.6 Synthetic Ghost Inodes API
Orchestrators inject environment files and licenses into the microVM without leaving artifacts on the developer's physical disk:

```typescript
app.use(
  vfs((fs) =>
    fs
      // Static in-memory file:
      .ghost("/root/.gitconfig", `[user]\n  name = Claude Agent\n  email = agent@kelir.dev\n`)
      // Dynamic in-memory file (evaluated freshly per open()):
      .ghostDynamic("/workspace/.env.runtime", async () => ({
        content: `DATABASE_URL=postgres://ephemeral_${crypto.randomUUID()}@localhost/db\n`,
        mode: 0o600,
      }))
  )
);
```

---

*(Part 2 complete. Awaiting instruction to deliver Part 3: The Wire Plane [virtio-net], Hypervisor Chaos Controller, State Superpowers [fork/suspend/clock], Persistent Causal Ledger, and Repository Layout).*

# `kelir.architecture.md` (Part 3/3)
**A Radical, Plugin-First MicroVM Hypervisor Middleware for Autonomous Systems**

*Version: 3.0.0-normative*  
*Classification: Systems Architecture Blueprint*  
*Status: Normative Specification*

---

## 9. The Wire Plane: `virtio-net` & Zero-Knowledge Credential Vault

Guest workloads operate in a **Zero-Knowledge Credential Architecture**. Production API keys, database credentials, and SSH private keys **never enter guest memory, shell variables, or disk files**.

```
                           THE WIRE PLANE (virtio-net)
                                        │
           ┌────────────────────────────┼────────────────────────────┐
           ▼                            ▼                            ▼
  [1. L7 HTTP/TLS PROXY]      [2. NON-HTTP L4 SOCKS5]      [3. GIT-OVER-SSH GATE]
  Replaces dummy placeholders  Terminates DB connections    Bridges guest SSH
  prior to outbound TLS hand-  (Postgres/Redis/MySQL);      to host SSH-Agent via
  shake. Scrubs responses.     injects mTLS & dynamic certs. vsock. Enclave signing.
                                        │
                                        ▼
                              [4. DNS SINKHOLE]
                              Traps all UDP 53; drops non-whitelisted
                              domains with NXDOMAIN. Blocks exfiltration.
```

### 9.1 L7 Transparent Credential Swapper
When the agent executes an outbound HTTP command:
1. Shell environment variables contain dummy URN placeholders:
   ```bash
   curl -H "Authorization: Bearer $STRIPE_KEY" https://api.stripe.com/v1/invoices
   # In-guest: STRIPE_KEY = "vault://stripe_prod_key"
   ```
2. The `virtio-net` TAP interface routes outgoing frames through `kelir-plugin-wire`.
3. The forward proxy intercepts the plaintext request before the outbound TLS handshake:
   * Validates target host against the egress allowlist (`api.stripe.com`).
   * Replaces `vault://stripe_prod_key` with the real key retrieved from the host keyring or AWS Secrets Manager.
4. Performs TLS handshake directly from the host. Responses flowing back to the guest are scrubbed for sensitive headers (`Set-Cookie`, echoed auth headers).

### 9.2 Non-HTTP L4 Gateway & Git-over-SSH
* **Database Proxying:** Direct TCP connections (PostgreSQL on `5432`, Redis on `6379`) route through an in-memory SOCKS5 / SNI gateway. Ephemeral database credentials and client mTLS certificates are injected at the host connection boundary.
* **Git-over-SSH (`git@github.com:...`):** `/sbin/kelir-shim` maps guest SSH connections over `/dev/vhost-vsock` directly to the host's `SSH_AUTH_SOCK`. The host signs cryptographic authentication challenges via Secure Enclave keys. Private keys never touch guest memory.

### 9.3 DNS Sinkholing (Exfiltration Defense)
All UDP port 53 traffic is trapped at the TAP boundary. Unregistered or non-allowlisted domains are sinkholed with an immediate `NXDOMAIN` response. DNS data tunneling exploits (`secret-data.attacker.com`) are neutralized at the hypervisor perimeter.

---

## 10. The Hypervisor Chaos Controller (`@kelir/plugin-chaos`)

To verify that an agent's code is production-resilient, `kelir-plugin-chaos` injects physical hardware faults directly into the `virtio-net` and `virtio-fs` ring buffers:

```typescript
app.use(
  chaos((c) =>
    c
      // 1. Network Plane Chaos (virtio-net ring)
      .network({
        target: "api.stripe.com",
        latency: { baseMs: 500, jitterMs: 150 }, // 500ms ± 150ms lag
        dropRate: 0.05,                          // 5% synthetic packet loss
        injectRst: true,                         // Injects synthetic TCP RST flags
        bandwidthKbps: 128,                      // Clamps link to dialup speeds
      })
      // 2. Storage Plane Chaos (virtio-fs FUSE driver)
      .fs({
        targetPath: "/workspace/cache/*",
        simulateError: "ENOSPC",                 // "No space left on device"
        errorProbability: 0.20,                  // Fails 20% of write() calls
      })
  )
);
```

---

## 11. State, Time, & Hypervisor Lifecycle Superpowers

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          HYPERVISOR SUPERPOWERS                             │
├─────────────────────────────────────────────────────────────────────────────┤
│ 1. vm.fork() ────────► Multi-branch CoW memory cloning in <15ms.            │
│ 2. vm.rollback() ────► Reverts RAM & registers to checkpoint in <8ms.       │
│ 3. vm.suspend() ─────► Flushes dirty RAM to disk; host RAM drops to 0 MB.   │
│ 4. vm.clock.skew() ──► Offsets TSC / CNTVOFF_EL2 hardware clock registers.  │
│ 5. Sleep/Wake Snap ──► Resyncs clock on MacBook lid open; saves TLS handshakes.│
│ 6. Host Crash Reaper ─► PR_SET_PDEATHSIG terminates VM threads on host kill -9.│
│ 7. Docker-in-Kelir ──► Rootless Podman + crun in EROFS; no nested VMX needed.│
│ 8. Break-Glass Attach─► Drops developer directly into live interactive TTY. │
│ 9. Port Forwarding ──► Bridges guest web services over vsock port 5253.     │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 11.1 Tree-Based CoW Memory Forking (`vm.fork()`)
When running parallel agent exploration (e.g., 5 parallel candidate strategies):
* **Linux KVM:** Clones the Guest Physical Address (GPA) memory map via `userfaultfd` and Copy-on-Write in **<15ms**.
* **macOS (Apple Silicon HVF):** Clones memory mappings via memory-mapped page duplication.
* Children share warm parent memory (warmed compilers, loaded dependencies). Host RAM is dirtied only when a child writes to a page.

### 11.2 Zero-RAM Hibernation (`suspend` & `resume`)
When an agent pauses waiting for human approval:
* `vm.suspend("/storage/session.snap")` freezes vCPU registers, compresses dirty memory pages via zstd, flushes to local NVMe in **<50ms**, and deallocates host memory.
* **Host RAM consumption drops to 0 MB.**
* `vm.resume("/storage/session.snap")` restores registers and memory in **<50ms** at the exact cycle boundary.

### 11.3 Virtual Hardware Clock Skewing & MacBook Lid Sleep Listener
Agents testing time-sensitive code (JWT expiry, cron cycles, certificate renewals) shift time without monkeypatching:
* **Linux (KVM):** Injects offsets into Guest TSC via `KVM_SET_CLOCK` and `kvm_clock` MSRs.
* **macOS (Apple Silicon):** Offsets ARM64 `CNTVOFF_EL2` (Virtual Timer Offset Register). Guest `CNTVCT_EL0` reads jump forward instantly.
* **MacBook Lid Sleep/Wake Snapping:** Closing a laptop lid pauses guest monotonic time while host wall-clock advances, normally causing TLS handshakes to fail upon wake. `kelir-core` listens to host power events (`IOPMrootDomain` on macOS, `systemd-logind` on Linux) and dispatches an immediate hardware clock resync over `AF_VSOCK`, snapping guest time to host wall-clock in 0ms.

### 11.4 Host Crash Orphan Reclaimer (`PR_SET_PDEATHSIG`)
If the host Node.js or Python process terminates abruptly (`kill -9`, OOM-killer, segfault):
* Hypervisor child threads could remain orphaned, consuming 100% CPU.
* `kelir-core` links all worker threads to an anonymous `eventfd` and sets `prctl(PR_SET_PDEATHSIG, SIGKILL)`.
* If the host process exits, the host OS kernel terminates the hypervisor backend and unmaps memory immediately.

### 11.5 Nested Containers: "Docker-in-Kelir" via Rootless Podman
Apple Silicon's `Hypervisor.framework` does not support nested hardware virtualization (VMX/SVM passthrough).
* `kelir` embeds **Rootless Podman + `crun`** inside the read-only EROFS rootfs.
* Commands like `docker build` or `docker compose up` run via rootless Podman using Linux **user namespaces** (`CLONE_NEWUSER`).
* OCI containers run cleanly inside the microVM without nested hypervisor requirements or host Docker daemons.

### 11.6 Human Break-Glass Interactive Attach
When an automated agent gets stuck in a loop, the developer seizes interactive control without terminating the session:
```typescript
if (agent.isConfused) {
  // Pauses agent loop; bridges developer's host TTY directly to virtio-console:
  await app.breakGlass();
  // Developer inspects state, runs manual commands, exits subshell; control returns to agent
}
```

### 11.7 Background Daemons & Dynamic Ingress Port Forwarding
1. **Zombie Reaping:** `/sbin/kelir-shim` acts as `PR_SET_CHILD_SUBREAPER`. When a child shell exits, background processes are adopted and reaped, preventing defunct `<zombie>` processes from exhausting the PID table.
2. **Ingress Port Forwarding:** When a background service binds a port (e.g., Next.js dev server on 3000), `kelir-shim` sends `GuestToHost::PortBound`. Traffic bridges over `virtio-vsock` port 5253 to host `localhost:8080`:
   ```typescript
   const tunnel = await app.forwardPort({ host: 8080, guest: 3000 });
   ```

---

## 12. Epistemic Oracles & The Persistent Causal Ledger (`@kelir/plugin-ledger`)

Epistemic orchestrators (Axiom, RLVR runners, Popperian falsifiers) require objective verification of agent claims.

### 12.1 Cryptographic Execution Receipts
Every executed process emits an unforgeable physical signature to the host:

```rust
// crates/kelir-core/src/telemetry/receipt.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    pub pid: u32,
    pub binary_path: String,
    pub binary_blake3: [u8; 32],       // Cryptographic hash of executed binary
    pub dirty_memory_pages: usize,    // True physical memory pages mutated
    pub syscall_count: u64,           // Total system calls evaluated
    pub exit_code: i32,               // Kernel return code
    pub wall_clock_ns: u64,           // True physical elapsed time
    pub egress_bytes: usize,          // Total network egress bytes transmitted
}
```

### 12.2 Mathematical State Diffs (`CausalDelta`)
To compute counterfactual state deltas ($\Delta = S_A - S_B$), `kelir` calculates exact differences between two checkpoint descriptors:

```rust
pub struct CausalDelta {
    pub memory_diff_pages: usize,
    pub modified_inodes: Vec<std::path::PathBuf>,
    pub spawned_pids: Vec<u32>,
    pub egress_requests: Vec<HttpRequestRecord>,
}
```

### 12.3 Cryptographic Merkle WAL & OpenTelemetry
* **Merkle Chaining:** Every execution receipt is hashed into an append-only WAL backed by embedded SQLite or DuckDB:
  $$H_n = \text{BLAKE3}(H_{n-1} \parallel \text{Receipt}_n)$$
  Creates a tamper-proof audit trail for SOC2 and regulatory compliance.
* **Distributed Tracing (OTel):** Automatically injects standard W3C `traceparent` headers into outbound HTTP requests, connecting internal shell execution directly to Jaeger or Datadog.

### 12.4 The 3-Tier Epistemic Truth Hierarchy & Synchronous Inode Snooper

AI agents suffer from **self-reporting bias**: when evaluated via internal harness artifacts (e.g. `~/.claude/sessions/*.json` or local SQLite logs), agents frequently hallucinate success, report falsified test passes, or break orchestrators when minor harness updates mutate private JSON schemas. 

`kelir` establishes a strict **3-Tier Hierarchy of Truth** that decouples orchestration from private harness internals:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      THE 3-TIER HIERARCHY OF TRUTH                          │
├─────────────────────────────────────────────────────────────────────────────┤
│ TIER 1: PHYSICAL GROUND TRUTH (Hypervisor / Kernel Boundary)                │
│   - Unforgeable Receipts: Cryptographic BLAKE3 hashes & dirty memory pages. │
│   - OS Inode Diffs: Physical disk modifications evaluated via CausalDelta.  │
│   - Egress Telemetry: True network frames passing the virtio-net wire proxy.│
│   ──► INVARIANT: The agent cannot fake this, even with root privileges.     │
├─────────────────────────────────────────────────────────────────────────────┤
│ TIER 2: PUBLIC FORMAL PROTOCOLS (The Contract Interface)                   │
│   - Model Context Protocol (MCP) tool events routed over stdio.             │
│   - Deterministic PTY stream outputs sanitized of ANSI injection.           │
│   ──► INVARIANT: Evaluated strictly against public, versioned contracts.    │
├─────────────────────────────────────────────────────────────────────────────┤
│ TIER 3: SYNCHRONOUS INODE SNOOPING (Harness State Interception)             │
│   - Zero Post-Mortem Polling: Intercepts harness write() calls in real-time.│
│   - Host In-Memory Buffer: Captures session updates before disk sync.       │
│   ──► INVARIANT: Eliminates async race conditions and file lock collisions. │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### Synchronous Inode Snooper API
If an orchestrator requires access to harness-internal telemetry, it does not poll files post-execution. It attaches a synchronous tap directly to the `virtio-fs` FUSE write pipeline:

```typescript
app.use(
  vfs((fs) =>
    fs.snoop("~/.claude/sessions/*.json", {
      sync: true, // Blocks the guest write() until host handler completes
      onWrite: async (sessionPath, rawBytes) => {
        const event = JSON.parse(rawBytes.toString());
        // Ingests harness state in host memory with ZERO race conditions
        orchestrator.ingestHarnessEvent(event);
      },
    })
  )
);
```

---

### 12.5 The Hardware Epistemic Lifecycle Bridge

Epistemic evaluation platforms (RLVR loops, Popperian falsifiers, benchmark runners) traditionally control agents by monkeypatching language-level SDKs (`agent._step()`, overriding private runtime prototypes). This breaks on every CLI update and remains blind to OS-level side effects.

`kelir` replaces software monkeypatching with an **In-Process Hardware Lifecycle Bridge**, converting high-level orchestrator controls into bare-metal hypervisor primitives:

```
┌───────────────────────────────┬─────────────────────────────────────────────┐
│ ORCHESTRATOR LIFECYCLE ACTION │ PHYSICAL HYPERVISOR MECHANISM               │
├───────────────────────────────┼─────────────────────────────────────────────┤
│ orchestrator.pause()          │ Halts vCPU registers instantly via KVM/HVF  │
│                               │ ioctl at the current cycle boundary (0% CPU)│
├───────────────────────────────┼─────────────────────────────────────────────┤
│ orchestrator.resume()         │ Restores vCPU execution with 0ms clock skew │
├───────────────────────────────┼─────────────────────────────────────────────┤
│ orchestrator.step()           │ Executes workload until next virtio-vsock   │
│                               │ syscall trap, then pauses vCPUs for eval    │
├───────────────────────────────┼─────────────────────────────────────────────┤
│ orchestrator.revert()         │ Drops dirty RAM pages and purges tmpfs diffs│
│                               │ back to last known good snapshot in <8ms    │
└───────────────────────────────┴─────────────────────────────────────────────┘
```

#### The Atomic Step-Evaluation Loop
Orchestrators execute unmodified agent binaries while maintaining cycle-accurate governance over every operational step:

```typescript
import { kelir } from "@kelir/core";
import { vfs } from "@kelir/plugin-vfs";
import { pty } from "@kelir/plugin-pty";

const orchestrator = new EpistemicEvaluator();
let lastGoodCheckpoint: SnapshotDescriptor;

const app = kelir({ vcpus: 2, memoryMb: 2048 })
  .use(vfs())
  .use(pty())
  // Intercept every action at the hardware boundary
  .exec((cmd) =>
    cmd.before("*", async (c) => {
      // 1. Hardware Snapshot: Record clean state before step executes
      lastGoodCheckpoint = await c.vm.checkpoint("pre-step");

      // 2. Hardware Pause: Hold vCPU registers while verifying intent
      const intentAnalysis = await orchestrator.analyzeIntent({
        binary: c.trap.binary_path,
        argv: c.trap.argv,
        lineage: c.trap.ancestor_chain,
      });

      if (!intentAnalysis.isHypothesisValid) {
        // Reject action immediately: Zero mutations reach the guest OS
        return c.deny(1, `Epistemic Reject: ${intentAnalysis.falsificationReason}`);
      }

      return c.resume(); // Releases vCPU to execute step
    })
    .after("*", async (c) => {
      // 3. Post-Condition Verification via Tier 1 Physical Ground Truth
      const receipt = await c.getReceipt(); // BLAKE3 hash, dirty pages, exit code
      const isConsistent = await orchestrator.verifyPhysicalReceipt(receipt);

      if (!isConsistent) {
        // 4. Instant Hardware Rollback: Memory and disk revert in <8ms
        await c.vm.rollback(lastGoodCheckpoint);
        c.pty.notice("⚡ [EPISTEMIC ENGINE]: Side-effect violation. State reverted.\n");
      }
    })
  );

// Boots stripped microVM and runs unmodified harness
await app.spawn("claude", ["code", "run test verification"]);
```


---

## 13. Ecosystem Ingestion Adapters

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           KELIR INGESTION ADAPTERS                          │
├─────────────────────────────────────────────────────────────────────────────┤
│ 1. STANDARD LIBRARY SHIMS:                                                  │
│    - Node.js: `import { spawn } from "@kelir/core";` (Drop-in child_process)│
│    - Python:  `from kelir import Popen`              (Drop-in subprocess)   │
├─────────────────────────────────────────────────────────────────────────────┤
│ 2. MODEL CONTEXT PROTOCOL (MCP):                                            │
│    - Binary `@kelir/mcp-server` mounts directly into Claude Desktop/Cursor. │
│    - Exposes hypervisor terminal execution without touching host system.    │
├─────────────────────────────────────────────────────────────────────────────┤
│ 3. CONTAINERD SHIM V2 (K8s Parity):                                         │
│    - `containerd-shim-kelir-v2` runs microVMs as Kubernetes Pods.           │
│    - Specified via: `runtimeClassName: kelir`                               │
├─────────────────────────────────────────────────────────────────────────────┤
│ 4. OPENSHELL COMPUTE DRIVER (NVIDIA Ecosystem):                             │
│    - `openshell-driver-kelir` implements `compute_driver.proto` over UDS.   │
├─────────────────────────────────────────────────────────────────────────────┤
│ 5. E2B SDK COMPATIBILITY:                                                   │
│    - `@kelir/e2b-compat` replicates `Sandbox` class for instant migration.  │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 14. Consolidated Repository Layout & Crate Architecture

```
kelir/
├── Cargo.toml                         # Workspace manifest (Rust MSRV 1.85+)
│
├── crates/
│   ├── kelir-core/                    # Radical Microkernel (Driver Router + 4-Ring Bus)
│   │   ├── src/
│   │   │   ├── lib.rs                 # Core library entrypoint
│   │   │   ├── bus/                   # 4-Ring VirtIO event bus & dispatcher
│   │   │   ├── plugin.rs              # KelirPlugin async trait definition
│   │   │   ├── protocol/              # AF_VSOCK TLV & CBOR wire framing
│   │   │   └── vmm/                   # VmmDriver trait & lifecycle management
│   │   └── Cargo.toml
│   │
│   ├── kelir-plugins/                 # Modular First-Class Plugin Crates
│   │   ├── plugin-pty/                # virtio-console broker, ANSI sanitizer, Token Shield
│   │   ├── plugin-vfs/                # virtio-fs daemon, Ghost Inodes, lock broker, reconciler
│   │   ├── plugin-dlp/                # Read-time FUSE regex secret masking
│   │   ├── plugin-wire/               # L7 forward proxy, non-HTTP SOCKS5, DNS sinkhole
│   │   ├── plugin-chaos/              # virtio-net jitter/drop & virtio-fs ENOSPC fault injector
│   │   └── plugin-ledger/             # Persistent Merkle WAL, CausalDelta, OpenTelemetry
│   │
│   ├── kelir-drivers/                 # Pluggable Virtualization Engines
│   │   ├── driver-libkrun/            # macOS (HVF + Metal GPU) + Linux KVM (DEFAULT)
│   │   ├── driver-rustvmm/            # Bare-metal Linux KVM hypervisor + CUDA/CDI
│   │   └── driver-openvmm/            # Windows WHP (ConPTY + HV_SOCKET) + Azure MSHV
│   │
│   ├── kelir-shim/                    # In-guest Musl static binary (/sbin/kelir-shim, 50KB)
│   │   ├── src/
│   │   │   ├── main.rs                # Guest PID 1 init & seccomp-unotify loop
│   │   │   ├── subreaper.rs           # PR_SET_CHILD_SUBREAPER daemon reaper
│   │   │   ├── trap.rs                # PTRACE_SEIZE anti-TOCTOU, .so maps & entropy scan
│   │   │   └── vsock.rs               # /dev/vhost-vsock client transport
│   │   └── Cargo.toml                 # Target: x86_64/aarch64-unknown-linux-musl
│   │
│   ├── kelir-adapters/                # Ecosystem Interoperability Shims
│   │   ├── containerd-shim/           # containerd-shim-v2 implementation for Kubernetes
│   │   ├── openshell-grpc/            # NVIDIA OpenShell compute_driver.proto service
│   │   └── mcp-server/                # Model Context Protocol standard terminal server
│   │
│   └── kelir-bindings/                # Foreign Function Interface Layers
│       ├── napi/                      # Node-API FFI bridging Rust -> TypeScript
│       └── pyo3/                      # PyO3 bindings bridging Rust -> Python
│
├── packages/
│   ├── ts/                            # Node.js / TypeScript Package Distribution
│   │   ├── core/                      # @kelir/core (Microkernel builder & plugin engine)
│   │   ├── plugins/                   # @kelir/plugin-* (Modular TypeScript plugin bindings)
│   │   ├── presets/                   # @kelir/presets (Pre-bundled agent configurations)
│   │   └── e2b-compat/                # @kelir/e2b-compat (Drop-in E2B client SDK)
│   │
│   └── py/                            # Python Package Distribution (PyPI: kelir)
│       ├── kelir/                     # Pythonic async middleware builder
│       └── compat/e2b/                # Drop-in Python Sandbox implementation
│
├── tests/
│   ├── e2e/                           # Full Claude Code, Cursor, and SWE-bench test runs
│   ├── integration/                   # CoW memory fork, clock skew, and vsock stress tests
│   └── fixtures/                      # Minimal vmlinux kernels, EROFS rootfs images
│
└── Makefile                           # Orchestrated cross-compilation pipeline
```

---

## 15. The Normative Architectural Verdict

`kelir` delivers a unified, production-grade substrate for autonomous AI execution:

1. **Replaces Brittle Terminal Hacks (`tmux`, regex screen scraping):** Employs hardware `virtio-console` channels, programmatic prompt responders with foreground process validation, dynamic `SIGWINCH` window resizing, Token Shield context protection, and in-flight ANSI sanitization.
2. **Replaces Unsafe Host Storage Mutations:** Operates on pre-baked EROFS read-only rootfs devices, Copy-on-Write RAM overlays, synthetic in-memory Ghost Inodes, read-time DLP secret scrubbing, dynamic UID/GID ID-mapping, and an atomic Success Path Commit Reconciler (`c.commit()`) that correctly handles Linux whiteout character devices (`c 0 0`).
3. **Replaces Fragmented Virtualization Scripts:** Establishes a modular `VmmDriver` abstraction driving `libkrun` by default on macOS (with Metal GPU) and Linux KVM, with seamless pluggability for bare-metal `rust-vmm` or Windows `OpenVMM`.
4. **Enforces True Plugin-First Architecture:** Strips `kelir-core` down to a microkernel managing the 4 VirtIO hardware rings, delegating all behavioral logic to decoupled, composable plugins with statically accumulating TypeScript types.
5. **Guarantees Operational Survival:** Engineered with host crash reclamation (`PR_SET_PDEATHSIG`), MacBook sleep/wake clock snapping, Docker-in-Kelir rootless Podman execution, hardware chaos fault injection, and an unforgeable Merkle-chained Causal Ledger.

By maintaining strict **Epistemic Agnosticism** at the hardware hypervisor layer while delivering **Unbreakable Type Safety and Complete Operational Governance** at the application layer, `kelir` stands as the normative execution screen for autonomous artificial intelligence.
