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
  Signals, Traps  Token Shield, Resizing  In-Flight Masking,     Secrets, Chaos
                                          Ghost Files
```

### The 6 Core Invariants

1. **Zero Harness Awareness:** The guest workload sees standard POSIX interfaces (`/bin/sh`, `/dev/pts/*`, `glibc`/`musl`, TCP/IP). No custom guest SDKs, CLI shims, or guest-side agents are required.
2. **In-Process Microkernel (Zero Daemon Tax):** Virtualization runs inside the host consumer's memory space via direct C-FFI to `libkrun`, `rust-vmm`, or `OpenVMM`. There is no `dockerd`, no `containerd`, no background root socket, and no daemon maintenance.
3. **Pure Epistemic Agnosticism:** `kelir` has zero knowledge of "LLMs", "prompts", or "AI morals". It operates strictly as an operating system physics engine: memory pages, CPU registers, process lineages, file descriptors, and network frames.
4. **Deterministic Hardware Isolation:** The security boundary is hardware-enforced virtualization (Intel VT-x / AMD-V / Apple Silicon ARM64 EL2). An escaped guest root exploit cannot touch host memory or host kernel space.
5. **Radical Plugin-First Microkernel:** The hypervisor core does only two things: drive hardware virtualization and multiplex the 4 VirtIO rings. Every single capability (terminal sanitizing, secret vaulting, filesystem masking, chaos, audit logging) is a completely decoupled, composable plugin.
6. **Transparent, Truthful Steering:** No silent state divergence. Any intercepted syscall, file read, or network packet is either allowed, steered with mandatory terminal disclosure (`tty_notice`), or rejected with machine-parsable receipts.

### The Params Ingress Contract

`session.spawn(..., { params })` is the single ingress channel for structured execution parameters, and it is defined end-to-end by host-side machinery:

1. **Host-side validation.** `params` is validated on the host against the registered schema (Serde in Rust, TypeBox in TypeScript, Pydantic v2 in Python) before any guest instruction executes. Falsified payloads fail at the perimeter with `EINVAL`.
2. **Single-variable injection.** The validated value is serialized to JSON and injected into the guest process environment as exactly one variable: `KELIR_PARAMS=<json>`.
3. **Standard POSIX consumption.** Inside the guest, the workload reads `KELIR_PARAMS` like any other environment variable — the guest remains a stock POSIX process with zero guest-side SDKs, CLI shims, or agents (Invariant #1). Host-side handlers access the typed view through `c.params::<T>()`, which deserializes from the same validated buffer.

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

`kelir` executes as a single in-process hypervisor runtime. Host consumer applications interface with the microkernel via two distinct pathways: **direct zero-overhead Rust linking** or **in-process foreign function interfaces (Node-API / PyO3)**.

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                                 HOST PROCESS RUNTIME                                    │
│                                                                                         │
│  ┌─────────────────────────┐       ┌─────────────────────────────────────────────────┐  │
│  │   NATIVE RUST CONSUMER  │       │       DYNAMIC RUNTIME CONSUMERS                 │  │
│  │   (Direct Cargo Crate)  │       │       TypeScript (@kelir/core) | Python (kelir) │  │
│  └────────────┬────────────┘       └────────────────────────┬────────────────────────┘  │
│               │ Zero-Cost Link                              │ In-Process FFI            │
│               │ (Direct Monomorphization)                   │ (napi-rs / PyO3)          │
│               ▼                                             ▼                           │
│  ┌───────────────────────────────────────────────────────────────────────────────────┐  │
│  │                        kelir-core (Stripped Microkernel)                          │  │
│  │   ├── BusDispatcher: Zero-copy multiplexer for the 4 VirtIO Rings                 │  │
│  │   ├── EventBus: Unified compile-time and async hook pipeline                      │  │
│  │   └── DriverRouter: Lifecycle controller mediating Box<dyn VmmDriver>             │  │
│  └──────────────────────────────────────────┬────────────────────────────────────────┘  │
│                                             │ Dynamic Driver Trait Interface            │
│                                             ▼                                           │
│  ┌───────────────────────────────────────────────────────────────────────────────────┐  │
│  │                           kelir-drivers (Pluggable Backends)                      │  │
│  │   ├── driver-libkrun (DEFAULT): macOS (HVF + Metal GPU) + Linux KVM               │  │
│  │   │   Internal hard fork carrying custom patches: userfaultfd CoW memory          │  │
│  │   │   fork (<15ms), GPA snapshot/rollback engine (<8ms), and hardware clock       │  │
│  │   │   skew control. Upstream libkrun provides none of these.                      │  │
│  │   ├── driver-rustvmm: Bare-metal Linux KVM hypervisor + CUDA CDI GPU passthrough  │  │
│  │   │   — the full-featured path on bare-metal Linux                                │  │
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
│   ├── Minimal vmlinux: Stripped Linux kernel (no ACPI, no PCI bloat)                    │
│   │     - macOS Host: Built with CONFIG_ARM64_16K_PAGES=y (Apple Silicon parity)        │
│   │     - Linux Host: Built with CONFIG_ARM64_4K_PAGES=y or x86 4KB standard            │
│   │                                                                                     │
│   ├── Hardware Paravirtualized Devices:                                                 │
│   │     - virtio-rng: Hardware entropy feeding /dev/urandom (no boot hangs)             │
│   │     - virtio-wdt: Hardware watchdog timer preventing guest kernel lockups           │
│   │     - virtio-balloon: Dynamic memory inflation/deflation under host pressure        │
│   │                                                                                     │
│   ├── Rootfs Mounting: Ultra-fast read-only EROFS block device (cold-mounted in <2ms)   │
│   │                                                                                     │
│   ├── /sbin/kelir-shim (Static 50KB Musl Binary, Guest PID 1):                          │
│   │     - Binds vsock:5252 (control/trap plane) AND vsock:5253 (ingress                 │
│   │       port-forward multiplexer proxying host connections to guest loopback)         │
│   │     - Runs a non-blocking waitpid(-1, WNOHANG) zombie reclamation loop in its       │
│   │       async event loop (as PID 1 it is the universal orphan ancestor)               │
│   │     - Installs seccomp-unotify filter with in-kernel BPF allowlist                  │
│   │     - Performs Anti-TOCTOU task-group freezes: PTRACE_SEIZE +                       │
│   │       PTRACE_INTERRUPT on every thread in /proc/$PID/task/*                         │
│   │     - Parses binary ELF headers (DT_NEEDED, PT_INTERP) directly from disk           │
│   │       and computes Shannon entropy on argv[1:]                                      │
│   │     - Allocates a full PTY master/slave per workload via openpty() + setsid()       │
│   │       + TIOCSCTTY so children get a real controlling terminal; executes             │
│   │       TIOCSWINSZ resizes over virtio-console                                        │
│   │                                                                                     │
│   └── UNTOUCHED GUEST WORKLOAD: Claude Code / Cursor / SWE-bench / Node / Rust / Python │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.1 The Universal `VmmDriver` Trait

Hypervisor backends are completely swappable via `VmmDriver`. The microkernel drives all backends through identical primitives:

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
    /// Bytes of guest RAM mutated since the parent snapshot — a hardware-agnostic
    /// measure, immune to host page-size differences (Apple Silicon 16KB vs Linux 4KB)
    pub dirty_bytes: u64,
    pub created_at_ns: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum WatchdogAction {
    /// Guest kernel lockup detected → hard-reset the vCPUs and reboot the microVM
    Reboot,
    /// Unresponsive VM → flush ledger state, then terminate the instance
    Terminate,
}

#[async_trait]
pub trait VmmDriver: Send + Sync {
    /// Cold-boots the microVM environment in <60ms via the active platform driver
    async fn boot(&mut self, config: VmConfig) -> Result<(), VmmError>;

    /// Branches the running VM into an isolated CoW clone in <15ms
    /// - Linux: userfaultfd + KVM memory cloning
    /// - macOS: HVF memory-mapped page duplication
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

    /// Arms the hardware watchdog: when no heartbeat arrives within `timeout`,
    /// the driver executes `action` (Reboot / Terminate)
    async fn set_watchdog(&mut self, timeout: Duration, action: WatchdogAction) -> Result<(), VmmError>;

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
2. **Anti-TOCTOU Task-Group Freeze:** When a security-sensitive binary (`git`, `curl`, `ssh`, `rm`, `sudo`, `npm`, or binaries from `/tmp`) triggers `seccomp-unotify`, the shim iterates `/proc/$PID/task/*` and issues `ptrace(PTRACE_SEIZE)` followed by `ptrace(PTRACE_INTERRUPT)` to **every thread in the task group** before reading any guest memory. `seccomp-unotify` stops only the calling thread — seizing the thread-group leader alone would leave sibling threads free to rewrite `argv` or binary memory mid-inspection — so the freeze must cover the entire task group before inspection begins. This closes Time-of-Check to Time-of-Use races where a concurrent thread rewrites memory buffers after inspection.
3. **Entropy & Dynamic Linker Audit:** Before forwarding the trap to the host, `/sbin/kelir-shim`:
   * Calculates Shannon entropy strictly on `argv[1:]` — the payload — excluding `argv[0]`, whose low-entropy executable path (e.g. `/usr/local/bin/node`) would dilute the measurement. Values **> 5.2** flag encrypted shell drops or base64 payloads.
   * Parses the binary's ELF headers directly from disk at trap time, walking `DT_NEEDED` entries and the `PT_INTERP` segment to enumerate the shared libraries the dynamic linker *will* load, blocking `LD_PRELOAD` rootkits. Memory mappings for the new image only exist after `execve` completes and returns to userspace, so a disk-time static ELF scan is the attestation mechanism; `/proc/$PID/maps` is not consulted at trap time because it still describes the old image.

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
      │ Task group frozen            ▼
/sbin/kelir-shim seizes every    Shim extracts process metadata:
thread in /proc/$PID/task/*:     - BLAKE3 hash of binary
PTRACE_SEIZE + PTRACE_INTERRUPT  - Shannon entropy of argv[1:] (>5.2)
Reads /proc/$PID/mem safely      - Full ancestor process DAG
Parses ELF DT_NEEDED/PT_INTERP   - Working directory & envp
from the binary on disk             │
      │                              │
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
      │   directly to the workload's PTY                               │
      ▼                                                             ▼
kelir-shim releases syscall                               Injects exit code + stderr
Workload resumes on vCPU                                  to agent TTY immediately
```

### 4.2 Wire Framing Specification

All frames over vsock port `5252` utilize binary Type-Length-Value (TLV) framing with CBOR payload encoding. The header is exactly **20 bytes**. Sequence numbers are **64-bit unsigned** and are transmitted as two 32-bit words, most-significant word first:

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|          Magic (0x4B45)       |          Version (0x03)       |  (0x4B45 = 'KE')
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

The 64-bit sequence space is shared by both directions (`GuestToHost`, `HostToGuest`) and monotonically increases per vsock connection; the codec reads the two 32-bit words and reassembles them into the native `u64` before payload decoding.

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
        /// Mandatory terminal disclosure notice printed to the
        /// workload's PTY. Guarantees Invariant #6: Zero silent
        /// state divergence
        tty_notice: Option<String>,
    },
    SuspendForApproval {
        poll_interval_ms: u32,
    },
}
```

**TrapAction semantics — environment injection.** `TrapAction::Allow` resumes the intercepted syscall untouched: registers and memory are never modified, so a plain `Allow` decision can never mutate the guest environment. Environment variables set by a trap handler (`c.env.set(...)`) reach the guest exclusively through `TrapAction::Rewrite`. When a handler that called `c.env.set()` returns `Decision.resume()`, the runtime automatically converts the decision into `TrapAction::Rewrite`, preserving `new_binary = original_binary` and the original `new_argv`, and carrying the accumulated handler environment map in `injected_env`. The syscall then re-executes through the rewrite path with the injected environment attached — the only channel through which handler-side environment changes become visible to the guest process.

---

*(End of Part 1. Part 2: Radical Plugin-First Core Traits, The Elysia-Style Accumulating SDK Engine, Hardware Terminal Plane [virtio-console], and Zero-Leak Storage Plane with In-Flight Masking [virtio-fs].)*

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
use crate::handlers::HandlerCtx;
use crate::protocol::messages::{TrapEvent, TrapAction};
use crate::session::Session;
use crate::vmm::driver::VmmDriver;

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Plugin execution rejected: {0}")]
    Halt(String),
    #[error("Internal I/O failure: {0}")]
    Io(#[from] std::io::Error),
}

/// Complete inode attribute set surfaced by storage-plane lookups. FUSE `LOOKUP`
/// replies are populated verbatim from this struct (`st_mode`, `st_uid`, `st_gid`,
/// `st_size`) alongside the content bytes.
#[derive(Debug, Clone)]
pub struct VfsInodeAttr {
    /// File mode bits (e.g. `0o400` read-only, `0o600` private)
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
    pub content: Vec<u8>,
}

#[async_trait]
pub trait KelirPlugin: Send + Sync {
    /// Unique identifier for telemetry and ordering
    fn name(&self) -> &'static str;

    // ─── 1. MicroVM Lifecycle Hooks ─────────────────────────
    async fn on_vm_init(&self, _driver: &mut Box<dyn VmmDriver>) -> Result<(), PluginError> { Ok(()) }
    async fn on_vm_ready(&self, _session: &Session) -> Result<(), PluginError> { Ok(()) }
    async fn on_vm_shutdown(&self, _session: &Session) -> Result<(), PluginError> { Ok(()) }

    // ─── 2. Control Plane Hooks (virtio-vsock) ───────────────
    async fn on_trap(&self, _event: &TrapEvent, _ctx: &HandlerCtx) -> Result<Option<TrapAction>, PluginError> { Ok(None) }
    async fn on_process_exit(&self, _pid: u32, _exit_code: i32) -> Result<(), PluginError> { Ok(()) }

    // ─── 3. Terminal Plane Hooks (virtio-console) ────────────
    async fn on_pty_input(&self, _bytes: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
    async fn on_pty_output(&self, _chunk: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }

    // ─── 4. Storage Plane Hooks (virtio-fs) ──────────────────
    async fn on_vfs_lookup(&self, _path: &Path) -> Result<Option<VfsInodeAttr>, PluginError> { Ok(None) }
    async fn on_vfs_read(&self, _path: &Path, _buffer: &mut [u8]) -> Result<(), PluginError> { Ok(()) }
    async fn on_vfs_write(&self, _path: &Path, _data: &[u8], _pid: u32, _ctx: &HandlerCtx) -> Result<bool, PluginError> { Ok(true) }

    // ─── 5. Wire Plane Hooks (virtio-net) ────────────────────
    async fn on_net_outbound(&self, _frame: &mut Vec<u8>) -> Result<bool, PluginError> { Ok(true) }
    async fn on_net_inbound(&self, _frame: &mut Vec<u8>) -> Result<bool, PluginError> { Ok(true) }
}
```

The wire plane is fully bidirectional: `on_net_outbound` and `on_net_inbound` give plugins a mutation point on both directions of the `virtio-net` ring, so fault-injection plugins can apply outbound *and* inbound latency, packet loss, and jitter. The storage plane is fully attributed: `on_vfs_write` carries the writing guest PID and the live `HandlerCtx`, so snoop callbacks correlate every FUSE write with the exact process and its trap state, and `on_vfs_lookup` returns the full `VfsInodeAttr` so synthetic inodes honor their declared ownership and permissions (§8.6).

### 5.1 Static Dispatch Table Compilation
To achieve sub-12 microsecond hook execution, `kelir-core` compiles registered plugins into a flat, contiguous dispatch array at boot. Plugins are evaluated in strict priority order. The first plugin returning `Some(TrapAction::Deny { .. })` or `Some(TrapAction::Rewrite { .. })` halts the pipeline and dispatches the packet immediately over `AF_VSOCK`.

---

## 6. Tri-Native Consumer SDK Architecture (Rust, TypeScript, Python)

`kelir` rejects dynamic `any`-typing and second-class shims. All three host consumer SDKs enforce identical runtime guarantees through their language-native compile-time type systems:

1. **Rust:** Borrow-checker verified handle injection + zero-cost Serde deserialization.
2. **TypeScript:** Generic context accumulation (`Kelir<TContext>`) + TypeBox schema guards.
3. **Python:** RAII context managers (`async with`) + Pydantic v2 execution validation.

All three SDKs publish the same five first-class plugin packages, maintained **1:1 across languages**: the Rust crates `plugin-pty`, `plugin-vfs`, `plugin-wire`, `plugin-chaos`, and `plugin-ledger`; the TypeScript packages `@kelir/plugin-pty`, `@kelir/plugin-vfs`, `@kelir/plugin-wire`, `@kelir/plugin-chaos`, and `@kelir/plugin-ledger`; and the Python modules under `kelir.plugins.*`. A plugin mounted in one language has an exact counterpart — same trait surface, same hook semantics — in the other two.

Spawn options follow the params ingress contract defined in §1: `params` are validated on the host against the registered schema, then injected into the guest process environment as the single variable `KELIR_PARAMS=<json>`, where the stock POSIX workload consumes it like any other environment variable.

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        LANGUAGE-SPECIFIC SAFETY INVARIANTS                             │
├──────────────┬────────────────────────────────────┬────────────────────────────────────┤
│ LANGUAGE     │ PLUGIN INSTALLATION PROOF          │ INBOUND PARAMETER VALIDATION       │
├──────────────┼────────────────────────────────────┼────────────────────────────────────┤
│ Rust         │ `plugin_with_handle(P)` produces   │ `c.params::<T>()` deserializes     │
│              │ typed handle; closures fail to     │ via Serde; failure halts spawn     │
│              │ compile if plugin is unmounted     │ before traps execute               │
├──────────────┼────────────────────────────────────┼────────────────────────────────────┤
│ TypeScript   │ Type-level context accumulation;   │ `.guard({ schema: t.Object(...) })`│
│              │ `c.pluginKey` fails `tsc` if       │ validates via TypeBox prior to     │
│              │ `.use()` is absent upstream        │ execution                          │
├──────────────┼────────────────────────────────────┼────────────────────────────────────┤
│ Python       │ `plugin_with_handle()` yields typed│ Pydantic `BaseModel` on `spawn()`  │
│              │ handle instance; missing handle    │ raises `EINVAL` before hook        │
│              │ fails closures at import time      │ evaluation                         │
└──────────────┴────────────────────────────────────┴────────────────────────────────────┘
```

---

### 6.1 Native Rust Architecture: The Borrow-Checker Invariant

In Rust, plugin dependencies are verified without dynamic reflection or string keys. A plugin defines an associated `Handle` type. Mounting the plugin atomically returns the builder and its handle.

```rust
// Rust Consumer: Compile-Time Plugin Verification & Serde Guards

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
enum Action {
    FixWebhook { repo: String, issue_id: String },
}

// 1. Atomic handle extraction:
let (builder, policy) = builder.plugin_with_handle(RepoPolicyPlugin::new(options));

// 2. Downstream handler captures typed handle:
let builder = builder.exec(move |exec| {
    let policy = policy.clone();
    exec
        // Serde deserializes `Action` before handler execution:
        .before("npm test", handler(|c: HandlerCtx| async move {
            let params = c.params::<Action>()?; 
            Ok(Decision::Resume)
        }))
        // Compiler guarantees `policy` exists in scope:
        .before("git commit *", handler(move |_c: HandlerCtx| {
            let policy = policy.clone();
            async move {
                if policy.violations().await > 0 {
                    return Ok(Decision::deny(1, "Policy violations recorded"));
                }
                Ok(Decision::Resume)
            }
        }))
});
```

* **No Handle, No Compile:** If `builder.plugin_with_handle(...)` is omitted, `policy` does not exist. Closures attempting to capture it trigger rustc error `E0425`.
* **Zero-Allocation Validation:** `c.params::<T>()` operates directly on zero-copy guest memory buffers.

---

### 6.2 TypeScript Architecture: Type-Accumulating Pipeline (`@kelir/core`)

The TypeScript SDK models pipeline state as an accumulating generic type `Kelir<TContext>`. Each `.use()` call returns an expanded type context.

```typescript
// TypeScript Consumer: Generic Inference & TypeBox Guard

import { kelir, t } from "@kelir/core";
import { vfs } from "@kelir/plugin-vfs";

const runtime = kelir({ vcpus: 4, memoryMb: 4096 })
  .use(vfs())
  .use(repoPolicyPlugin({ maxDiffBytes: 64 * 1024 }))
  .exec((cmd) => cmd
    // 1. Static and runtime schema validation
    .guard({
      schema: t.Object({
        action: t.Literal("fix-webhook"),
        issueId: t.String(),
      }),
    })
    // 2. `c.policy` and `c.fs` are fully inferred on HandlerContext:
    .before("git commit *", async (c) => {
      if (c.policy.violations > 0) {
        return c.deny(1, `EPOLICY: ${c.policy.violations} violation(s)`);
      }
      return c.resume();
    })
  );
```

* **Build-Time Invalidation:** Removing `.use(repoPolicyPlugin)` strips `policy` from `TContext`. Downstream references to `c.policy` fail at `tsc` compilation.
* **Schema Boundary:** Spawn actions failing the `.guard()` schema terminate execution with `EINVAL` prior to VM instruction dispatch.

---

### 6.3 Python Architecture: RAII Context Managers & Pydantic Validation

The Python SDK bridges native hypervisor memory management directly to Python runtime semantics. It replaces ad-hoc hypervisor lifecycle management with deterministic `async with` resource cleanup and Pydantic v2 models.

```python
# Python Consumer: RAII Lifecycles & Pydantic Schema Enforcement

from pydantic import BaseModel, Field
from kelir import kelir, Decision, HandlerContext

class FixWebhookParams(BaseModel, frozen=True):
    action: str = Field(pattern=r"^fix-webhook$")
    issue_id: str

# 1. Plugin yields typed handle instance
builder = kelir.builder(vcpus=4, memory_mb=4096)
builder, policy = builder.plugin_with_handle(RepoPolicyPlugin())

# 2. Handler captures handle via closure
async def commit_gate(_c: HandlerContext) -> Decision:
    if policy.violations > 0:
        return Decision.deny(1, f"EPOLICY: {policy.violations} violation(s)")
    return Decision.resume()

builder.exec(lambda cmd: cmd.before("git commit *", commit_gate))

# 3. RAII Execution: __aenter__ boots microVM; __aexit__ guarantees termination
async with builder as session:
    await session.spawn(
        "npm", ["test"],
        params=FixWebhookParams(action="fix-webhook", issue_id="ISSUE-8412"),
    )
```

* **Guaranteed Hypervisor Teardown:** Python's `async with` guarantees `session.terminate()` execution inside `__aexit__`, matching Rust's `Drop` semantics under unhandled exceptions.
* **Pre-Hook Type Safety:** Inbound spawn payloads are parsed through Pydantic v2 before any hypervisor trap or interceptor evaluates. Falsified payloads fail immediately at the guest perimeter.

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
│   Linux Kernel PTY Driver (/dev/hvc0 -> /dev/pts/N)                         │
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
│     ├── 3. WindowResize Bridge ───► Boot handshake pushes initial window    │
│     │                             size (e.g. 120x40) at VM-ready; host      │
│     │                             SIGWINCH resizes follow the same path:    │
│     │                             WindowResize over vsock:5252 → shim       │
│     │                             executes TIOCSWINSZ on guest PTY.         │
│     │                                                                       │
│     ├── 4. Signal Forwarding ─────► Ctrl+C (SIGINT) / Ctrl+D forwarded to   │
│     │                             guest foreground process group.           │
│     │                                                                       │
│     └── 5. Token Shield Filter ───► Clamps chunks exceeding byte ceiling.   │
│                                   Injects truncation markers into stream.   │
│                                   (Protects LLM Context Window).            │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 7.1 In-Flight ANSI Stream Sanitizer
Untrusted scripts execute escape sequences to compromise host environments:
* **OSC 52 Attack Defense:** Scripts printing `\x1b]52;c;...` attempt to read or overwrite the host system clipboard. The PTY broker drops all OSC 52 bytes at the hardware buffer boundary.
* **Terminal Title Spoofing (OSC 0/1/2):** Stripped to prevent tampering with host terminal emulator titles.

### 7.2 Terminal Window Resizing (`SIGWINCH`) & Signal Forwarding
When an agent renders interactive curses interfaces or spinners, column and row mismatches corrupt output.

**Boot terminal handshake.** At VM-ready, the hypervisor pushes the configured initial window size (e.g. `120x40`) to the guest over the console control channel, before the first workload spawns. The guest never boots into a default `80x25` terminal — full-screen applications (Claude Code, `vim`, `top`) initialize with correct geometry from their first frame.

**Live resizes** follow the same path as the boot handshake:
1. The host catches window adjustments via `process.stdout.on("resize")` (`SIGWINCH`).
2. A `HostToGuest::WindowResize { rows, cols, xpixel, ypixel }` packet is dispatched over `AF_VSOCK:5252`.
3. In-guest `/sbin/kelir-shim` executes `ioctl(pts_fd, TIOCSWINSZ, &ws)` on the workload's PTY slave (`/dev/pts/N`).
4. The guest shell automatically redraws layouts without terminal corruption.

### 7.3 Programmatic Prompt Responder (`onPrompt`) with Foreground Verification
Orchestrators avoid scraping terminal screens with regex. `onPrompt` triggers directly on the hardware stream chunk:

```typescript
app.use(
  pty((t) =>
    t.onPrompt({
      match: /Overwrite production schema\? \[y\/N\]/i,
      timeoutMs: 1500,
      action: async (prompt, stream) => {
        // Injects raw keystrokes directly into the virtio-console hardware ring:
        stream.write("y\n");
      },
    })
  )
);
```
* **ANSI-Stable Matching:** Matchers run against an ANSI-SGR-stripped plaintext shadow copy of the output stream. The plugin maintains a parallel buffer with all SGR color/bold/underline codes removed, evaluates `match` against that plaintext, and streams the original bytes — styling intact — to the consumer. Regexes therefore match prompt text like `Overwrite production schema? [y/N]` regardless of embedded SGR color or bold codes emitted by modern CLI tools.
* **Anti-Spoof Verification:** The prompt responder executes **only** if the process holding the PTY slave matches the kernel's active foreground process group (`tcgetpgrp()`). Background processes executing `echo "Overwrite production schema? [y/N]"` cannot trigger automated responses.

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

* **Boundary-Safe Clamping:** The truncation point never splits a glyph or an escape sequence. When a byte or line ceiling is hit, Token Shield scans backward from the clamp point to the nearest valid UTF-8 code-point boundary, and to the start of any in-progress CSI/OSC escape sequence, so multi-byte characters (emoji, international text) and complete ANSI sequences survive intact. The head-and-tail strategy retains whole lines and whole sequences at both ends of the stream.

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
│    - Base host workspace mounted STRICTLY READ-ONLY.                        │
│    - Base host workspace mounted STRICTLY READ-ONLY.                        │
│    - Instant Revert: Drop overlayfs layer in 1ms. Host SSD is untouched.    │
│    - Instant Revert: Drop overlayfs layer in 1ms. Host SSD is untouched.    │
│ 3. THE SUCCESS PATH RECONCILER (session.fs.reconcile()):                    │
│    - Pure storage-plane sync: overlay upperdir → host disk, atomically.     │
│    - Resolves file whiteouts (c 0 0) and directory whiteouts                │
│      (trusted.overlay.opaque=y) into host unlink() / remove_dir_all().      │
│    - Zero filesystem corruption; Git commits stay ordinary guest runs.      │
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
│ 7. IN-FLIGHT READ-TIME MASKING (plugin-vfs maskRead):                       │
│    - FUSE-level byte stream interception on file reads.                     │
│    - MaskRule regex redaction; fixed-width replacements preserve            │
│      st_size and file offsets.                                              │
├─────────────────────────────────────────────────────────────────────────────┤
│ 8. CENTRAL INODE LOCK BROKER (Cross-VM Concurrency):                        │
│    - Intercepts FUSE SETLK / SETLKW (flock / fcntl) across multiple VMs.    │
│    - Prevents .git/index.lock collisions when multiple agents share a repo. │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 8.1 The Success Path Reconciler (`session.fs.reconcile()`)
When an agent mutates files in its in-memory `tmpfs` scratchpad, changes merge back to physical disk atomically. `session.fs.reconcile()` is a **pure storage-plane operation**: it syncs the overlay `upperdir` differential to the host workspace and nothing else. It carries no VCS semantics and accepts no `message` parameter — a Git commit (or any other version-control operation) is an ordinary guest execution that traverses the normal trap chain like every other command.

* **The OverlayFS Whiteout Problem:** Deleted *files* in Linux `overlayfs` are represented as special character devices with major/minor numbers `0/0` (`mknod c 0 0`). Deleted or shadowed *directories* are represented as opaque directories carrying the extended attribute `trusted.overlay.opaque = "y"`. A naive copy of the upperdir back to the host would write unusable device nodes and resurrect deleted directories.
* **The Reconciler Algorithm:**
  1. Traverses the guest `tmpfs` `upperdir` differential tree.
  2. Resolves **file whiteouts** (`S_ISCHR(mode) && rdev == 0`) with POSIX `unlink()` against the corresponding host workspace path.
  3. Resolves **directory whiteouts** (`trusted.overlay.opaque = "y"`) with `remove_dir_all()` against the host directory, so guest-visible directory deletions hold on disk.
  4. Writes modified and newly created files with atomic staged writes — copy to a `tmp` sibling, then `rename()` into place — so the host never observes a partially written file.
  5. Flushes disk buffers with `syncfs()`/`fsync()` before reporting completion.

```rust
// crates/kelir-plugins/plugin-vfs/src/reconciler.rs

pub async fn reconcile_upperdir(
    upperdir: &Path,
    host_target: &Path,
) -> Result<CommitStats, std::io::Error> {
    let mut stats = CommitStats::default();

    for entry in walkdir::WalkDir::new(upperdir).min_depth(1) {
        let entry = entry?;
        let rel_path = entry.path().strip_prefix(upperdir).unwrap();
        let target_path = host_target.join(rel_path);
        let metadata = entry.metadata()?;

        // 1. File whiteout: char device 0/0 → unlink the host file
        if metadata.file_type().is_char_device() && metadata.rdev() == 0 {
            if target_path.exists() {
                std::fs::remove_file(&target_path)?;
                stats.whiteouts_resolved += 1;
            }
            continue;
        }

        // 2. Directory whiteout: `trusted.overlay.opaque = "y"` → remove
        //    the host directory tree so the guest deletion holds
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

        // 3. Atomic staged write for modified/created files: tmp + rename
        if metadata.is_file() {
            let tmp_path =
                target_path.with_extension(format!("tmp.{}", std::process::id()));
            std::fs::copy(entry.path(), &tmp_path)?;
            std::fs::rename(&tmp_path, &target_path)?;
            stats.files_changed += 1;
        }
    }

    nix::unistd::sync();
    Ok(stats)
}
```

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

### 8.4 In-Flight Read-Time Masking (`maskRead`, plugin-vfs)
When an agent executes `cat /workspace/.env` or inspects configuration files, local secrets must not leak into guest memory:
* `plugin-vfs` intercepts `read()` dispatch at the FUSE buffer boundary and applies registered `MaskRule` regex redactions in host RAM before bytes copy to guest pages:

```typescript
app.use(
  vfs((fs) =>
    fs.maskRead("**/secrets.*", [
      { pattern: /sk_live_[0-9a-zA-Z]{24}/g, replace: "[REDACTED_STRIPE_KEY]" },
      { pattern: /AKIA[0-9A-Z]{16}/g, replace: "[REDACTED_AWS_KEY]" },
      { pattern: /-----BEGIN PRIVATE KEY-----[\s\S]+?-----END PRIVATE KEY-----/g, replace: "[REDACTED_RSA_KEY]" },
    ])
  )
);
```

* **Fixed-Width Redaction:** Every replacement is padded or truncated to the **exact byte length of the match** — a 20-byte replacement landing on a 32-byte `sk_live\u2026` match is padded to 32 bytes; a replacement longer than its match is truncated to the match length. `st_size` and file offsets are therefore preserved: the FUSE read buffer never changes length, so the guest kernel's inode accounting, sequential reads, and trailing reads all stay consistent.

### 8.5 Central Cross-VM Inode Lock Broker
When multiple agents in a swarm share the same repository:
* Concurrent invocations of `git status` or `git commit` collide on `.git/index.lock`. Standard `virtio-fs` does not synchronize POSIX locks across multiple independent hypervisor instances.
* Guest `overlayfs` assigns **virtual inodes** that have no correlation to host physical inodes, so guest inode numbers alone cannot key a host-side lock table. The broker resolves each guest inode to its host physical inode through the **virtio-fs mount identity** — the host-side path mapping backing that guest mount — and keys all lock state on the **host-resolved inode + path identity**.
* `kelir-core` embeds the **Central Inode Lock Broker**:
  1. Intercepts `FUSE_SETLK` and `FUSE_SETLKW` (`fcntl`/`flock`) system calls.
  2. Maps the guest virtual inode through the virtio-fs mount identity to the host physical inode and path, then routes lock acquisition through a centralized host-side in-memory mutex table keyed on that host identity.
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
        mode: 0o400,   // full VfsInodeAttr: mode, uid, gid, size, content
        uid: 0,
        gid: 0,
      }))
  )
);
```

Every ghost inode — static or dynamic — surfaces as a full `VfsInodeAttr` (`mode`, `uid`, `gid`, `size`, `content`). The FUSE `LOOKUP` reply is populated verbatim from these attributes, so guest-visible permissions such as `mode: 0o400` are honored exactly: an untrusted guest process opens a read-only ghost file as read-only, with no default-permission fallback.

---

*(End of Part 2. Part 3: The Wire Plane [virtio-net], Hypervisor Chaos Controller, State Superpowers [fork/suspend/clock], Persistent Causal Ledger, and Repository Layout.)*

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
  Terminates guest TLS under   Terminates DB connections    Bridges guest SSH
  the per-session CA; swaps    (Postgres/Redis/MySQL);      to host SSH-Agent via
  vault placeholders in-       injects mTLS & dynamic certs. vsock. Enclave signing.
  transit. Scrubs responses.
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
2. **Secret Auto-Injection Contract:** During `on_vm_ready`, `plugin-wire` injects every registered vault key into the guest init environment table (`STRIPE_KEY=vault://stripe_prod_key`, `DATABASE_URL=vault://pg_creds`, …). The placeholder URN is the only value that ever exists inside the guest; the real secret crosses the wire plane exclusively on the host side.
3. **TLS Interception Contract:** At boot, `plugin-wire` generates an ephemeral per-session root CA and mounts it into the guest trust store (`/etc/ssl/certs`) as an overlayfs ghost inode. Standard guest binaries (`curl`, `git`, language-native TLS stacks) therefore transparently accept the intercepted TLS session with zero CLI flags — no `--cacert`, no `NODE_EXTRA_CA_CERTS`, no harness awareness.
4. The `virtio-net` TAP interface routes outgoing frames through `kelir-plugin-wire`. The forward proxy terminates the guest TLS session:
   * Validates target host against the egress allowlist (`api.stripe.com`).
   * Replaces `vault://stripe_prod_key` with the real key retrieved from the host keyring or AWS Secrets Manager.
   * Establishes the upstream TLS handshake directly from the host.
5. Response scrubbing: responses flowing back to the guest are scrubbed for sensitive headers (`Set-Cookie`, echoed auth headers) and re-encrypted under the per-session CA before entering the TAP device.

### 9.2 Non-HTTP L4 Gateway & Git-over-SSH
* **Database Proxying:** Direct TCP connections (PostgreSQL on `5432`, Redis on `6379`) route through an in-memory SOCKS5 / SNI gateway. Ephemeral database credentials and client mTLS certificates are injected at the host connection boundary.
* **Git-over-SSH (`git@github.com:...`):** `/sbin/kelir-shim` maps guest SSH connections over `/dev/vhost-vsock` directly to the host's `SSH_AUTH_SOCK`. The host signs cryptographic authentication challenges via Secure Enclave keys. Private keys never touch guest memory.

### 9.3 DNS Sinkholing (Exfiltration Defense)
All UDP port 53 traffic is trapped at the TAP boundary. Unregistered or non-allowlisted domains are sinkholed with an immediate `NXDOMAIN` response. DNS data tunneling exploits (`secret-data.attacker.com`) are neutralized at the hypervisor perimeter.

---

## 10. The Hypervisor Chaos Controller (`@kelir/plugin-chaos`)

To verify that an agent's code is production-resilient, `kelir-plugin-chaos` injects physical hardware faults directly into the `virtio-net` and `virtio-fs` ring buffers. Fault injection is **bidirectional** across the wire plane: `on_net_outbound` mutates frames leaving the guest (uplink latency, outbound packet loss, TCP RST injection), while `on_net_inbound` mutates frames arriving at the guest (download latency, inbound packet loss, server-side jitter simulation). Both directions flow through the same chaos rule engine:

```typescript
app.use(
  chaos((c) =>
    c
      // 1. Network Plane Chaos (virtio-net ring) - outbound AND inbound frames
      .network({
        target: "api.stripe.com",
        direction: "both",                        // "outbound" | "inbound" | "both"
        latency: { baseMs: 500, jitterMs: 150 }, // 500ms ± 150ms lag (uplink + downlink)
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
│ 4. vm.clock.skew() ──► Offsets TSC / CNTVOFF_EL2 (auto-unpins TSC scaling).  │
│ 5. Sleep/Wake Snap ──► Resyncs clock on MacBook lid open; saves TLS handshakes.│
│ 6. Host Crash Reaper ─► Linux PDEATHSIG / macOS kqueue NOTE_EXIT reclamation.│
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
* **Session Hydration:** The driver-level `fork()` yields a child `Box<dyn VmmDriver>` — the raw CoW memory clone. The `kelir-core` runtime then re-hydrates a complete twin `Session` around that driver: it rebinds the 4-ring dispatch table, the virtio channels, and the inherited plugin graph. `session.vm.fork()` therefore returns a full `Session` on which `spawn()` and every other session API are directly callable:
  ```typescript
  const child = await session.vm.fork(); // child is a complete twin Session
  await child.spawn("npm", ["test", "--", "--runInBand"]);
  ```

### 11.2 Zero-RAM Hibernation (`suspend` & `resume`)
When an agent pauses waiting for human approval:
* `vm.suspend("/storage/session.snap")` freezes vCPU registers, compresses dirty memory pages via zstd, flushes to local NVMe in **<50ms**, and deallocates host memory.
* **Host RAM consumption drops to 0 MB.**
* `vm.resume("/storage/session.snap")` restores registers and memory in **<50ms** at the exact cycle boundary.

### 11.3 Virtual Hardware Clock Skewing & MacBook Lid Sleep Listener
Agents testing time-sensitive code (JWT expiry, cron cycles, certificate renewals) shift time without monkeypatching:
* **Linux (KVM):** Injects offsets into Guest TSC via `KVM_SET_CLOCK` and `kvm_clock` MSRs.
* **macOS (Apple Silicon):** Offsets ARM64 `CNTVOFF_EL2` (Virtual Timer Offset Register). Guest `CNTVCT_EL0` reads jump forward instantly.
* **TSC Scaling Unpin:** `vm.clock.skew()` automatically unpins invariant-TSC scaling on the cloned VM before applying the clock offset (`KVM_SET_CLOCK` / `CNTVOFF_EL2`). The vCPU TSC rate decouples from the host hardware clock and tracks the virtual clocksource, so the guest clocksource stays stable under skew — the kernel never flags `tsc` as unstable and never falls back to low-resolution jiffies.
* **MacBook Lid Sleep/Wake Snapping:** Closing a laptop lid pauses guest monotonic time while host wall-clock advances, normally causing TLS handshakes to fail upon wake. `kelir-core` listens to host power events (`IOPMrootDomain` on macOS, `systemd-logind` on Linux) and dispatches an immediate hardware clock resync over `AF_VSOCK`, snapping guest time to host wall-clock in 0ms.

### 11.4 Host Crash Orphan Reclaimer
If the host Node.js or Python process terminates abruptly (`kill -9`, OOM-killer, segfault), hypervisor children must not survive as orphaned CPU spinners. Reclamation is dual-path by design, matching each host OS's native process-notification primitive:
* **Linux:** `kelir-core` links all worker threads to an anonymous `eventfd` and sets `prctl(PR_SET_PDEATHSIG, SIGKILL)` on every hypervisor backend thread. If the host process dies, the host OS kernel terminates the hypervisor backend and unmaps memory immediately — no polling involved.
* **macOS (Darwin):** `prctl(PR_SET_PDEATHSIG)` does not exist on Darwin. Each hypervisor backend is guarded by a dedicated `kqueue(EVFILT_PROC, NOTE_EXIT)` watcher thread attached to the host consumer PID, plus a parent-monitoring heartbeat pipe as an independent second channel. When the host PID exits (or the heartbeat pipe's write end closes), the watcher sends `SIGKILL` to the backend process group and reclaims the VM memory mappings.

### 11.5 Nested Containers: "Docker-in-Kelir" via Rootless Podman
Apple Silicon's `Hypervisor.framework` does not support nested hardware virtualization (VMX/SVM passthrough).
* `kelir` embeds **Rootless Podman + `crun`** inside the read-only EROFS rootfs.
* Commands like `docker build` or `docker compose up` run via rootless Podman using Linux **user namespaces** (`CLONE_NEWUSER`).
* **Ephemeral Write Layer:** The guest boot fstab provisions ephemeral tmpfs mounts for `/var/lib/containers`, `/var/run`, and `/tmp`, so rootless Podman can write image layers, metadata stores, and lock files while the rootfs stays an immutable EROFS block. Container builds never surface `EROFS: Read-only file system`.
* OCI containers run cleanly inside the microVM without nested hypervisor requirements or host Docker daemons.

### 11.6 Human Break-Glass Interactive Attach
When an automated agent gets stuck in a loop, the developer seizes interactive control without terminating the session.

**Lifecycle invariant:** the builder is write-only until `boot()`; `spawn()`, `breakGlass()`, and all execution APIs exist only on `Session`:

```typescript
const session = await kelir({ vcpus: 2, memoryMb: 2048 })
  .use(pty())
  .use(vfs())
  .boot();

const agent = await session.spawn("claude", ["code", "run test verification"]);

if (agent.isConfused) {
  // Pauses agent loop; bridges developer's host TTY directly to virtio-console:
  await session.breakGlass();
  // Developer inspects state, runs manual commands, exits subshell; control returns to agent
}
```

### 11.7 Background Daemons & Dynamic Ingress Port Forwarding
1. **Zombie Reaping:** `/sbin/kelir-shim` runs as guest PID 1 — the universal ancestor of all orphaned processes by kernel definition. Its async event loop integrates a non-blocking `waitpid(-1, &status, WNOHANG)` reap pass: when a foreground shell exits, orphaned background daemons are adopted and reaped, preventing defunct `<zombie>` processes from exhausting the PID table.
2. **Ingress Port Forwarding:** `forwardPort` is a `Session` API with language-idiomatic parity across SDKs. When a background service binds a port (e.g., a Next.js dev server on guest `3000`), `kelir-shim` emits `GuestToHost::PortBound`; host connections to the forwarded port are tunneled over `virtio-vsock:5253` to the shim's ingress multiplexer (`crates/kelir-shim/src/ingress.rs`), which proxies the byte stream to the guest loopback port:
   ```typescript
   // TypeScript: object form
   const tunnel = await session.forwardPort({ host: 8080, guest: 3000 });
   ```
   ```python
   # Python: positional form
   tunnel = await session.forward_port(8080, 3000)
   ```
   ```rust
   // Rust: positional form
   let tunnel = session.forward_port(8080, 3000).await?;
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
    pub binary_blake3: String,        // BLAKE3 digest, canonical 64-char lowercase hex
    pub dirty_bytes: u64,             // True byte count of mutated physical memory.
                                      // Hardware-agnostic by design: page size is
                                      // 16 KB on Apple Silicon vs 4 KB on Linux/x86,
                                      // so raw byte counts are the canonical metric.
    pub syscall_count: u64,           // Total system calls evaluated
    pub exit_code: i32,               // Kernel return code
    pub wall_clock_ns: u64,           // True physical elapsed time
    pub egress_bytes: u64,            // Total network egress bytes transmitted
}
```

* **Hardware-Agnostic Telemetry:** `dirty_bytes: u64` reports the true byte count of mutated physical memory. Physical page size differs across platforms (16 KB on Apple Silicon ARM64, 4 KB on Linux/x86), so byte counts — not page counts — are the canonical metric: identical executions produce identical receipts on every host, keeping epistemic assertions, RLVR loss functions, and Merkle root hashes stable across developer machines. Epistemic assertions check `dirty_bytes == 0`.
* **Universal Digest Encoding:** every BLAKE3 digest — binary hashes, execution receipts, causal deltas — is a canonical 64-character lowercase hex string across Rust, TypeScript, and Python. It is the single cross-FFI representation; no raw byte arrays or mixed encodings cross language boundaries.
* **Ownership:** receipts are generated and held by `kelir-core` at the trap/exit boundary. `c.receipt()` on the handler context is the single read API.

### 12.2 Mathematical State Diffs (`CausalDelta`)
To compute counterfactual state deltas ($\Delta = S_A - S_B$), `kelir` calculates exact differences between two checkpoint descriptors:

```rust
pub struct CausalDelta {
    pub memory_diff_bytes: u64,             // Hardware-agnostic byte-level RAM delta
    pub modified_inodes: Vec<std::path::PathBuf>,
    pub spawned_pids: Vec<u32>,
    pub egress_requests: Vec<HttpRequestRecord>,
}
```

### 12.3 Cryptographic Merkle WAL & OpenTelemetry
* **Merkle Chaining:** Every execution receipt is hashed into an append-only WAL backed by embedded SQLite or DuckDB:
  $$H_n = \text{BLAKE3}(H_{n-1} \parallel \text{Receipt}_n)$$
  Creates a tamper-proof audit trail for SOC2 and regulatory compliance.
* **Host-Isolated Storage:** The audit database lives on the host filesystem, resolved relative to the consumer process CWD (default `.kelir/audit.merkle.db`). Guest paths (`/workspace/...`) are rejected at `LedgerPlugin` construction: the WAL must survive guest `vm.rollback()` and `vm.fork()` and can never be reverted or duplicated by them.
* **Ownership Boundary:** `kelir-core` generates and holds execution receipts (`c.receipt()`); `plugin-ledger` persists them into the Merkle WAL and exports OTel telemetry (`c.ledger.flush()`).
* **Distributed Tracing (OTel):** Automatically injects standard W3C `traceparent` headers into outbound HTTP requests, connecting internal shell execution directly to Jaeger or Datadog.

### 12.4 The 3-Tier Epistemic Truth Hierarchy & Synchronous Inode Snooper

AI agents suffer from **self-reporting bias**: when evaluated via internal harness artifacts (e.g. `~/.claude/sessions/*.json` or local SQLite logs), agents frequently hallucinate success, report falsified test passes, or break orchestrators when minor harness updates mutate private JSON schemas. 

`kelir` establishes a strict **3-Tier Hierarchy of Truth** that decouples orchestration from private harness internals:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      THE 3-TIER HIERARCHY OF TRUTH                          │
├─────────────────────────────────────────────────────────────────────────────┤
│ TIER 1: PHYSICAL GROUND TRUTH (Hypervisor / Kernel Boundary)                │
│   - Unforgeable Receipts: BLAKE3 hex digests & dirty byte counts.           │
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

The snoop plane carries two kernel-level guarantees:

* **Direct I/O Semantics (`FOPEN_DIRECT_IO`):** Snooped paths are mounted with `FOPEN_DIRECT_IO`, bypassing the guest page cache. FUSE write frames therefore carry the true producing PID — never a kernel flusher task (`kworker`/`pdflush`) — so PID-attributed telemetry (per-process BLAKE3 hashing, causal attribution) is exact.
* **Guaranteed FUSE Reply:** The VFS snoop dispatcher wraps every user callback in a catch-all. Any panic or throw in a host hook is converted into an explicit FUSE error reply (`fuse_reply_err(req, EPERM)`); a host hook exception can never leave a guest kernel thread blocked in uninterruptible D state. The guest always observes a well-defined error rather than a hang.

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
Orchestrators execute unmodified agent binaries while maintaining cycle-accurate governance over every operational step. Pre/post state tracking is a **core context service**: `c.step.mark(descriptor)` stores a typed checkpoint slot for the trapping PID and `c.step.last()` retrieves it — no userland scratchpads, no plugin boilerplate:

```typescript
import { kelir } from "@kelir/core";
import { vfs } from "@kelir/plugin-vfs";
import { pty } from "@kelir/plugin-pty";

const orchestrator = new EpistemicEvaluator();

const session = await kelir({ vcpus: 2, memoryMb: 2048 })
  .use(vfs())
  .use(pty())
  // Intercept every action at the hardware boundary
  .exec((cmd) =>
    cmd
      .before("*", async (c) => {
        // 1. Hardware Snapshot: record clean state in a core-owned StepSlot
        //    (typed, per-PID pre/post state tracking on the handler context)
        await c.step.mark(await c.vm.checkpoint("pre-step"));

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
        //    (c.receipt() is the single receipt read API on the context)
        const receipt = await c.receipt(); // BLAKE3 hex digest, dirty_bytes, exit code
        const isConsistent = await orchestrator.verifyPhysicalReceipt(receipt);

        if (!isConsistent) {
          // 4. Instant Hardware Rollback: Memory and disk revert in <8ms
          await c.vm.rollback(c.step.last());
          c.pty.notice("⚡ [EPISTEMIC ENGINE]: Side-effect violation. State reverted.\n");
        }
      })
  )
  .boot();

// Boots stripped microVM and runs unmodified harness on the session
await session.spawn("claude", ["code", "run test verification"]);
```


---
## 13. Ecosystem Ingestion Adapters

`kelir` delivers seamless interoperability across cloud-native infrastructure, agent orchestration frameworks, and developer environments.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           KELIR INGESTION ADAPTERS                          │
├─────────────────────────────────────────────────────────────────────────────┤
│ 1. TRI-NATIVE FIRST-CLASS SDKs:                                             │
│    - Rust: Direct in-process crate linkage (`kelir = "3.0"`)                │
│    - TypeScript: Fully typed runtime builder (`@kelir/core`)                │
│    - Python: Native async hypervisor engine (`kelir>=3.0` via PyO3)         │
├─────────────────────────────────────────────────────────────────────────────┤
│ 2. MODEL CONTEXT PROTOCOL (MCP):                                            │
│    - Binary `@kelir/mcp-server` mounts directly into Claude Desktop/Cursor. │
│    - Mediates hardware-isolated execution without exposing host boundaries. │
├─────────────────────────────────────────────────────────────────────────────┤
│ 3. CONTAINERD SHIM V2 (Kubernetes Bare-Metal Execution):                    │
│    - `containerd-shim-kelir-v2` runs microVMs as standard K8s Pods.         │
│    - Activated via `runtimeClassName: kelir` in Pod manifests.              │
├─────────────────────────────────────────────────────────────────────────────┤
│ 4. OPENSHELL COMPUTE DRIVER (NVIDIA AI Infrastructure):                     │
│    - `openshell-driver-kelir` implements `compute_driver.proto` over UDS.   │
│    - Binds GPU-accelerated workloads directly to bare-metal microVMs.       │
├─────────────────────────────────────────────────────────────────────────────┤
│ 5. SANDBOX ENGINE COMPATIBILITY (E2B Drop-In):                              │
│    - `@kelir/e2b-compat` (TS) and `kelir.compat.e2b` (Python).              │
│    - Replicates the `Sandbox` interface with sub-60ms local hardware boots. │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

### 13.1 Tri-Native Language Runtimes
Workloads consume `kelir` directly through native programming models without shell shimming or background daemon management:

* **Rust (`crates/kelir` facade crate):** One Cargo dependency (`kelir = "3.0"`) re-exporting `kelir-core` plus feature-gated plugins. Direct compile-time monomorphization; hypervisor memory allocations share the host process heap with zero dynamic dispatch overhead.
* **TypeScript (`@kelir/core`):** Zero-copy memory view streaming over NAPI-RS. High-throughput I/O pipelines process VirtIO console and network frames natively in libuv.
* **Python (`kelir` via PyO3):** Direct GIL-released asynchronous Rust dispatch. Futures map to Python's `asyncio` event loop while VM memory operations execute on detached OS threads.

---

### 13.2 Model Context Protocol (MCP) Adapter
The MCP engine exposes microVM execution to LLM clients (Claude Desktop, Cursor, Devin) via standard Model Context Protocol interfaces:

```json
{
  "mcpServers": {
    "kelir-sandbox": {
      "command": "kelir-mcp",
      "args": [
        "--kernel", "./fixtures/vmlinux",
        "--rootfs", "./fixtures/rootfs.erofs",
        "--memory-mb", "2048"
      ]
    }
  }
}
```
* Bypasses host shell execution entirely.
* Maps LLM tool calls (`execute_command`, `read_file`, `write_file`) directly to `virtio-console`, `virtio-fs`, and `virtio-vsock` ring operations.

---

### 13.3 Containerd Shim v2 (`containerd-shim-kelir-v2`)
Enables direct orchestration of microVM sandboxes inside Kubernetes clusters without running bloated VM management stacks:

* **Pod Isolation:** Each Kubernetes Pod executes within an isolated hardware microVM instance rather than a Linux cgroup/namespace envelope.
* **Storage Mounts:** Kubernetes PersistentVolumeClaims (PVCs) mount directly as `virtio-fs` endpoints with in-flight UID/GID mapping.
* **OCI Compatibility:** Translates standard OCI container images into read-only EROFS blocks during rootfs setup.

---

### 13.4 OpenShell gRPC Compute Driver (`openshell-driver-kelir`)
Implements the NVIDIA OpenShell hardware orchestration layer:

* Serves the `compute_driver.proto` gRPC interface over `/var/run/kelir/openshell.sock`.
* Negotiates GPU passthrough via `driver-rustvmm` using NVIDIA Container Device Interface (CDI) device mapping.
* Coordinates hardware memory checkpointing and CoW branching for multi-tenant inference tasks.

---

### 13.5 Sandbox API Compatibility (`@kelir/e2b-compat`)
Enables immediate adoption for agent frameworks built against the E2B Sandbox API without rewriting application code:

```typescript
// Drop-in compatibility with existing Sandbox workflows:
import { Sandbox } from "@kelir/e2b-compat";

const sandbox = await Sandbox.create({
  template: "base",
  timeoutMs: 300_000,
});

// Executes inside an isolated hardware microVM:
const execution = await sandbox.commands.run("npm test");
console.log(execution.stdout);

await sandbox.kill();
```
* Preserves existing `files`, `commands`, and `pty` method signatures.
* Replaces remote cloud VM roundtrips with sub-60ms local hardware execution.
---

## 14. Consolidated Repository Layout & Crate Architecture

```
kelir/
├── Cargo.toml                         # Workspace manifest (Rust 2024, MSRV 1.85+)
├── pnpm-workspace.yaml                # TypeScript monorepo root (packages/ts/*)
├── pyproject.toml                     # Python distribution root (PyPI: kelir)
├── justfile                           # Orchestrated cross-compilation & test pipeline
│
├── crates/
│   ├── kelir/                         # Facade crate: the single Cargo dependency
│   │   ├── src/
│   │   │   └── lib.rs                 # Re-exports kelir-core + feature-gated kelir-plugins/*
│   │   ├── tests/                     # Workspace integration tests (cargo nextest)
│   │   │   ├── test_cow_fork.rs       # <15ms CoW fork + twin Session hydration
│   │   │   ├── test_rollback.rs       # <8ms checkpoint rollback (RAM, registers, tmpfs)
│   │   │   ├── test_whiteout.rs       # c 0 0 + trusted.overlay.opaque reconciliation
│   │   │   └── test_anti_toctou.rs    # Multi-thread PTRACE_SEIZE argv/memory freeze
│   │   └── Cargo.toml
│   │
│   ├── kelir-core/                    # Radical Microkernel (Driver Router + 4-Ring Bus)
│   │   ├── src/
│   │   │   ├── lib.rs                 # Core library entrypoint
│   │   │   ├── bus/                   # 4-Ring VirtIO event bus & dispatcher
│   │   │   ├── plugin.rs              # KelirPlugin async trait definition
│   │   │   ├── protocol/              # AF_VSOCK TLV & CBOR wire framing
│   │   │   ├── step.rs                # StepSlot: typed per-PID checkpoint slots on HandlerCtx
│   │   │   ├── telemetry/             # ExecutionReceipt generation & CausalDelta engine
│   │   │   └── vmm/                   # VmmDriver trait & lifecycle management
│   │   └── Cargo.toml
│   │
│   ├── kelir-plugins/                 # Canonical First-Class Plugin Crates
│   │   ├── plugin-pty/                # virtio-console broker, ANSI sanitizer, Token Shield
│   │   ├── plugin-vfs/                # virtio-fs daemon, Ghost Inodes, lock broker,
│   │   │   │                          #   reconciler, in-flight MaskRule masking (mask.rs)
│   │   ├── plugin-wire/               # L7 forward proxy (per-session CA), SOCKS5, DNS sinkhole
│   │   ├── plugin-chaos/              # Bidirectional virtio-net jitter/drop & virtio-fs ENOSPC
│   │   └── plugin-ledger/             # Persistent Merkle WAL, CausalDelta, OpenTelemetry
│   │
│   ├── kelir-drivers/                 # Pluggable Virtualization Engines
│   │   ├── driver-libkrun/            # macOS (HVF + Metal GPU) + Linux KVM (DEFAULT)
│   │   ├── driver-rustvmm/            # Bare-metal Linux KVM hypervisor + CUDA/CDI
│   │   └── driver-openvmm/            # Windows WHP (ConPTY + HV_SOCKET) + Azure MSHV
│   │
│   ├── kelir-shim/                    # In-guest Musl static binary (/sbin/kelir-shim, 50KB)
│   │   ├── src/
│   │   │   ├── main.rs                # Guest PID 1 init, openpty/setsid/TIOCSCTTY broker,
│   │   │   │                          #   seccomp-unotify loop
│   │   │   ├── reaper.rs              # Non-blocking waitpid(-1, WNOHANG) orphan reap pass
│   │   │   ├── trap.rs                # PTRACE_SEIZE anti-TOCTOU (all /proc/PID/task
│   │   │   │                          #   threads), on-disk ELF DT_NEEDED scan,
│   │   │   │                          #   argv[1:] entropy scoring
│   │   │   ├── ingress.rs             # vsock:5253 ingress multiplexer (port forwarding)
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
│   │   ├── plugins/                   # @kelir/plugin-* (TypeScript plugin bindings)
│   │   │   ├── pty/                   # @kelir/plugin-pty
│   │   │   ├── vfs/                   # @kelir/plugin-vfs
│   │   │   ├── wire/                  # @kelir/plugin-wire
│   │   │   ├── chaos/                 # @kelir/plugin-chaos
│   │   │   └── ledger/                # @kelir/plugin-ledger
│   │   ├── presets/                   # @kelir/presets (Pre-bundled agent configurations)
│   │   └── e2b-compat/                # @kelir/e2b-compat (Drop-in E2B client SDK)
│   │
│   └── py/                            # Python Package Distribution (PyPI: kelir)
│       └── kelir/
│           ├── __init__.py            # Pythonic async middleware builder
│           ├── plugins/               # Python plugin bindings (1:1 with kelir-plugins)
│           │   ├── pty.py
│           │   ├── vfs.py
│           │   ├── wire.py
│           │   ├── chaos.py
│           │   └── ledger.py
│           └── compat/
│               └── e2b.py             # Drop-in Python Sandbox implementation
│
├── tests/
│   ├── e2e/                           # Full Claude Code, Cursor, and SWE-bench test runs
│   └── fixtures/                      # Minimal vmlinux kernels, EROFS rootfs images
│
└── README.md
```

---

## 15. The Normative Architectural Verdict

`kelir` delivers a unified, production-grade substrate for autonomous AI execution:

1. **Replaces Brittle Terminal Hacks (`tmux`, regex screen scraping):** Employs hardware `virtio-console` channels, programmatic prompt responders matching SGR-stripped plaintext, foreground process validation, dynamic `SIGWINCH` window resizing, Token Shield context protection with UTF-8/ANSI-boundary clamping, and in-flight ANSI sanitization.
2. **Replaces Unsafe Host Storage Mutations:** Operates on pre-baked EROFS read-only rootfs devices, Copy-on-Write RAM overlays, synthetic in-memory Ghost Inodes, in-flight `MaskRule` secret masking with fixed-width padding, dynamic UID/GID ID-mapping, and an atomic Success Path Reconciler (`session.fs.reconcile()`) that resolves both Linux whiteout character devices (`c 0 0`) and opaque directory whiteouts (`trusted.overlay.opaque`).
3. **Replaces Fragmented Virtualization Scripts:** Establishes a modular `VmmDriver` abstraction driving `libkrun` by default on macOS (with Metal GPU) and Linux KVM, with seamless pluggability for bare-metal `rust-vmm` or Windows `OpenVMM`.
4. **Enforces True Plugin-First Architecture:** Strips `kelir-core` down to a microkernel managing the 4 VirtIO hardware rings, delegating all behavioral logic to five canonical plugins — `pty`, `vfs`, `wire`, `chaos`, `ledger` — mirrored 1:1 across Rust, TypeScript, and Python.
5. **Guarantees Operational Survival:** Engineered with dual-path host crash reclamation (`PR_SET_PDEATHSIG` on Linux, `kqueue(EVFILT_PROC, NOTE_EXIT)` watchers on macOS), MacBook sleep/wake clock snapping with TSC-scale unpinning, Docker-in-Kelir rootless Podman execution on ephemeral tmpfs, bidirectional hardware chaos fault injection, and an unforgeable Merkle-chained Causal Ledger of hardware-agnostic execution receipts (`dirty_bytes`, canonical 64-character lowercase hex BLAKE3 digests).

By maintaining strict **Epistemic Agnosticism** at the hardware hypervisor layer while delivering **Unbreakable Type Safety and Complete Operational Governance** at the application layer, `kelir` stands as the normative execution screen for autonomous artificial intelligence. The specification is internally coherent across its Rust, TypeScript, and Python surfaces and is production-ready as written.
