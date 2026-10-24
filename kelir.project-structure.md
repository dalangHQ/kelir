**Monorepo Standard:** Cargo Workspaces (Rust Core) + PNPM Workspaces (TypeScript) + UV/Hatch (Python).

```
kelir/
├── .github/
│   └── workflows/
│       ├── ci.yml                     # Unified fast CI (Lint, Typecheck, Cross-Shim, KVM test)
│       └── release.yml                # Automated binary stripping, NPM & PyPI publishing
│
├── justfile                           # Zero-friction local development commands
├── Cargo.toml                         # Root Cargo workspace manifest (Rust 2024 / MSRV 1.85+)
├── Cargo.lock
├── pnpm-workspace.yaml                # PNPM workspace definition
├── package.json                       # Root devDependencies (Turborepo/Biome/TypeScript)
├── pyproject.toml                     # Root Python configuration (Ruff/Mypy/Hatchling)
│
├── crates/                            # ── RUST SYSTEMS LAYER ──
│   │
│   ├── kelir/                         # Facade crate: single Cargo dependency re-exporting kelir-core + feature-gated kelir-plugins/*
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   └── lib.rs                 # Re-export surface: kelir-core API + feature-gated plugin modules (pty, vfs, wire, chaos, ledger)
│   │   └── tests/                     # Integration tests (kelir crate test target)
│   │       ├── test_cow_fork.rs       # Verifies userfaultfd / HVF memory cloning in <15ms
│   │       ├── test_rollback.rs       # Verifies hardware rollback in <8ms
│   │       ├── test_whiteout.rs       # Verifies reconciler converts c 0 0 into host unlink()
│   │       └── test_anti_toctou.rs    # Concurrent thread execution race testing
│   │
│   ├── kelir-core/                    # In-process stripped microkernel
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                 # Core initialization exports
│   │       ├── step.rs                # StepSlot typed checkpoint store (c.step.mark / c.step.last)
│   │       ├── bus/                   # 4-Ring VirtIO Hardware Multiplexer
│   │       │   ├── mod.rs             # BusDispatcher trait
│   │       │   ├── console.rs         # Ring 1: Raw terminal stream broker
│   │       │   ├── vsock.rs           # Ring 2: Syscall trap & IPC multiplexer
│   │       │   ├── fs.rs              # Ring 3: virtio-fs in-memory bridge
│   │       │   └── net.rs             # Ring 4: virtio-net frame interception
│   │       ├── plugin.rs              # KelirPlugin trait & static dispatch pipeline
│   │       ├── protocol/              # Wire framing between Host and Shim
│   │       │   ├── mod.rs
│   │       │   ├── codec.rs           # 0x4B45 binary TLV framing parser
│   │       │   └── messages.rs        # TrapEvent, TrapAction, WindowResize CBOR models
│   │       ├── vmm/                   # Driver orchestration
│   │       │   ├── mod.rs
│   │       │   ├── driver.rs          # VmmDriver trait (boot, fork, checkpoint, rollback)
│   │       │   └── router.rs          # Dynamic backend loader (libkrun vs rustvmm)
│   │       └── telemetry/             # Hardware verification engines
│   │           ├── mod.rs
│   │           ├── receipt.rs         # Cryptographic ExecutionReceipt (BLAKE3, dirty pages)
│   │           └── delta.rs           # CausalDelta state diff computer
│   │
│   ├── kelir-shim/                    # In-guest Musl static binary (<50KB payload)
│   │   ├── Cargo.toml                 # Configured with lto = "fat", opt-level = "z", panic = "abort"
│   │   └── src/
│   │       ├── main.rs                # Guest PID 1 init loop (binds vsock:5252 control + vsock:5253 ingress, mounts devpts, waitpid(-1, WNOHANG) reaper)
│   │       ├── reaper.rs              # Non-blocking waitpid(-1, WNOHANG) zombie reclamation loop
│   │       ├── ingress.rs             # vsock:5253 ingress multiplexer → guest loopback TCP proxy
│   │       ├── pty_broker.rs          # openpty() master/slave broker: setsid() + TIOCSCTTY controlling terminals
│   │       ├── trap.rs                # seccomp-unotify loop + all-threads PTRACE_SEIZE/INTERRUPT anti-TOCTOU
│   │       ├── entropy.rs             # Shannon entropy on argv[1:] payload
│   │       ├── audit.rs               # ELF static scan (DT_NEEDED, PT_INTERP) + LD_PRELOAD verification
│   │       └── vsock.rs               # /dev/vhost-vsock hardware client socket
│   │
│   ├── kelir-drivers/                 # Pluggable Virtualization Implementations
│   │   ├── driver-libkrun/            # dalangHQ libkrun fork: userfaultfd CoW fork (<15ms), GPA snapshot engine (<8ms), clock skew patches — macOS HVF default
│   │   │   ├── Cargo.toml
│   │   │   └── src/lib.rs             # libkrun C-FFI bindings
│   │   ├── driver-rustvmm/            # Bare-metal Linux KVM + CUDA CDI passthrough
│   │   │   ├── Cargo.toml
│   │   │   └── src/lib.rs             # kvm-ioctls & userfaultfd CoW memory implementation
│   │   └── driver-openvmm/            # Windows WHP + ConPTY + HV_SOCKET
│   │       ├── Cargo.toml
│   │       └── src/lib.rs             # Microsoft WHP & MSHV hypervisor implementation
│   │
│   ├── kelir-plugins/                 # Modular First-Class Rust Plugins
│   │   ├── plugin-pty/                # virtio-console broker, ANSI sanitizer, Token Shield
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── ansi.rs            # Drop OSC 52 clipboard & OSC 0/1/2 title sequences
│   │   │       ├── responder.rs       # onPrompt engine matching ANSI-stripped plaintext (foreground-verified)
│   │   │       └── shield.rs          # UTF-8/ANSI boundary-safe head-and-tail token clamping
│   │   ├── plugin-vfs/                # virtio-fs engine, Synthetic FUSE inodes, sliding-window read replacement
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── mask.rs            # Sliding-window read replacement (MaskRule regex redaction, fixed-width)
│   │   │       ├── ghost.rs           # Synthetic RAM-only inode injector (VfsInodeAttr: mode/uid/gid/size)
│   │   │       ├── reconciler.rs      # Host-boundary reconciliation: c 0 0 whiteouts + opaque dirs → atomic host sync (session.fs.reconcile())
│   │   │       ├── idmap.rs           # FUSE dynamic UID/GID translation (0 -> geteuid())
│   │   │       └── lock_broker.rs     # Cross-VM lock broker keyed on host-mapped physical inodes
│   │   ├── plugin-wire/               # Zero-Knowledge credential proxy & DNS Sinkhole
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── l7_proxy.rs        # Plaintext swap prior to outbound TLS handshake
│   │   │       ├── l4_socks.rs        # Database connection proxy (mTLS & dynamic auth)
│   │   │       ├── ssh_gate.rs        # Secure Enclave host SSH-Agent forwarder
│   │   │       └── dns.rs             # Port 53 UDP interception & NXDOMAIN sinkhole
│   │   ├── plugin-chaos/              # Fault Injection Engine
│   │   │   ├── Cargo.toml
│   │   │   └── src/lib.rs             # Network jitter/drop & VFS ENOSPC simulation
│   │   └── plugin-ledger/             # Compliance & Audit Trail
│   │       ├── Cargo.toml
│   │       └── src/
│   │           ├── lib.rs
│   │           ├── wal.rs             # Append-only Merkle-chained SQLite/DuckDB WAL
│   │           └── otel.rs            # W3C traceparent context propagator
│   │
│   ├── kelir-adapters/                # Ecosystem Interoperability
│   │   └── mcp-server/                # Model Context Protocol stdio terminal server
│   │       ├── Cargo.toml
│   │       └── src/main.rs
│   │
│   └── kelir-bindings/                # FFI Translation Layers
│       ├── napi/                      # Rust -> Node.js NAPI-RS bridge
│       │   ├── Cargo.toml
│       │   └── src/lib.rs
│       └── pyo3/                      # Rust -> Python PyO3 bridge
│           ├── Cargo.toml
│           └── src/lib.rs
│
├── packages/                          # ── LANGUAGE SDK DISTRIBUTION ──
│   │
│   ├── ts/                            # TypeScript Monorepo Packages
│   │   ├── core/                      # @kelir/core
│   │   │   ├── package.json
│   │   │   ├── tsconfig.json
│   │   │   └── src/
│   │   │       ├── index.ts           # kelir() builder entrypoint
│   │   │       ├── context.ts         # Generic accumulation context engine
│   │   │       ├── schema.ts          # TypeBox command validation wrappers
│   │   │       └── binding.ts         # Auto-generated NAPI loader
│   │   ├── plugins/                   # @kelir/plugin-*
│   │   │   ├── pty/                   # TS bindings for virtio-console
│   │   │   ├── vfs/                   # TS bindings for virtio-fs
│   │   │   ├── wire/                  # TS bindings for credential vaulting
│   │   │   ├── chaos/                 # TS bindings for fault injection
│   │   │   └── ledger/                # TS bindings for Merkle WAL audit ledger
│   │   ├── presets/                   # @kelir/presets (Pre-configured agent setups)
│   │   │   └── src/openrig.ts         # Drop-in driver replacing OpenRig's tmux backend
│   │   └── e2b-compat/                # @kelir/e2b-compat
│   │       └── src/index.ts           # Drop-in replacement for @e2b/code-interpreter
│   │
│   └── py/                            # Python Distribution (PyPI: kelir)
│       ├── pyproject.toml
│       └── kelir/
│           ├── __init__.py            # Async builder interface
│           ├── core.py                # PyO3 native binding wrapper
│           ├── context.py             # Type-hinted context chain
│           ├── plugins/               # Python plugin bindings mirroring Rust/TS 1:1
│           │   ├── pty.py             # Python bindings for the pty plugin
│           │   ├── vfs.py             # Python bindings for the vfs plugin
│           │   ├── wire.py            # Python bindings for the wire plugin
│           │   ├── chaos.py           # Python bindings for the chaos plugin
│           │   └── ledger.py          # Python bindings for the ledger plugin
│           └── compat/
│               └── e2b.py             # Drop-in E2B Sandbox replacement class
│
├── tests/                             # ── VERIFICATION SUITES ──
│   ├── fixtures/
│   │   ├── kernel/                    # Per-arch vmlinux binaries (arm64 16K, x86_64 4K) — selected dynamically at boot
│   │   ├── rootfs/                    # Build scripts generating minimal EROFS base images
│   │   │   └── build-erofs.sh
│   │   └── payloads/                  # Binaries with Shannon entropy >5.2 for trap testing
│   └── e2e/
│       ├── test_claude_code.ts        # End-to-end Claude Code session under PTY controls
│       └── test_openrig_driver.ts     # Verifies swarm execution replacing OpenRig tmux
│
└── README.md                          # Quickstart, Invariants & Architecture Summary
```
