# Kelir Platform Scorecard

Compiled by `kelir-testkit scorecard` from `cans/artifacts/test-plan/manifest.yaml` + `scenarios.yaml`.
Do not edit by hand: CI runs `just check-scorecard`.

## Invariant Test Inventory

| Total Invariants | Domains | Integration Tier | Unit Tier | E2E Tier |
|---:|---:|---:|---:|---:|
| 254 | 17 | 168 | 73 | 13 |

## Domain Coverage Matrix

| Domain | Prefix | Cases | Status | Scope Description |
|---|---|---:|---|---|
| `adapters_ingestion` | `ad` | 12 | pass | kelir-pack OCI to EROFS ingestion, containerd shim v2, MCP stdio bridge, OpenShell gRPC driver, E2B compat SDK wrapper. |
| `attestation_receipts` | `ar` | 14 | pass | Hardware attestation tokens, vmlinux/EROFS BLAKE3 hashing, ExecutionReceipt generation, CausalDelta diff aggregation, Class 1-3 provenance. |
| `bindings_py` | `bp` | 4 | pass | Python SDK bindings parity: boot-time EConfig on missing Pydantic v2 dependency, ./vectors.json frame/verdict equality, AOT hook fusion single FFI trampoline. |
| `bindings_ts` | `bt` | 4 | pass | TypeScript SDK bindings parity: boot-time EConfig on missing TypeBox dependency, ./vectors.json frame/verdict equality, AOT hook fusion single FFI trampoline. |
| `governance_tools` | `gt` | 14 | pass | plugin-tool-guard, plugin-mcp, JSON-RPC 2.0 tool-call interposition, taint tracking, immutable schema locking, call quota token buckets. |
| `governance_vault` | `gv` | 12 | pass | plugin-vault, metadata service (169.254.169.254), host keyring mlock, scoped short-lived token issuance, audience locking, zeroization on drop. |
| `hypervisor_driver` | `hd` | 28 | pass | VmmDriver abstraction, KVM/HVF/WHP/AVF hardware virtualization, register/MMU rollback, CPUID synthetic filters, cold-boot budgets (<50ms). |
| `lifecycle_checkpoint` | `lc` | 25 | pass | Hypervisor state machine, CoW fork (userfaultfd / Mach vm_copy), instant rollback (<8ms), suspend to NVMe (0MB RSS, <50ms), step evaluation loops. |
| `plugin_engine` | `pe` | 21 | pass | KelirPlugin trait, segregated ring traits, unified Verdict / FilterAction evaluation, static dispatch tables, filter-before-FFI gate (<2µs). |
| `protocol_mux` | `pm` | 14 | pass | Vsock port 5252, Control channel (0x0000), Stream Mux (0x0001), sub-lanes (EXEC, VFS, BULK), framing limits, window-update flow control. |
| `relational_transaction` | `rt` | 12 | pass | plugin-db-proxy, plugin-transaction, L7 pgwire parsing, SQLite UDS bridge, multi-resource 2PC commit/abort coordination. |
| `storage_plane` | `sp` | 25 | pass | VirtIO-FS host CoW broker, sparse upper file, tombstone tracking, atomic leaf commit, containment engine (openat2), synthetic FUSE inodes, quotas. |
| `supply_chain_security` | `sc` | 10 | pass | plugin-supply-chain, dynamic W^X JIT mediation (seccomp-unotify), lifecycle script execution traps, Shannon entropy scanning (>5.2 bits/char). |
| `telemetry_audit` | `ta` | 12 | pass | plugin-ledger, Merkle WAL, RFC 6962 BLAKE3 leaf/node hashing, OTLP gRPC/HTTP streaming, W3C traceparent propagation. |
| `terminal_plane` | `tp` | 15 | pass | VirtIO-console Port 0 PTY, line discipline, token shield, ANSI sanitizer (OSC 52/title drops), EXEC stream mux, append-only cursor stream replay. |
| `virtio_rings` | `vr` | 15 | pass | Paravirtual VirtIO complex: SPSC shared memory rings, virtio-vsock, virtio-console, virtio-fs, virtio-net, virtio-rng, virtio-balloon. |
| `wire_plane` | `wp` | 17 | pass | Embedded smoltcp netstack, default-deny ingress, L4 diversion, DNS policy filter, synthetic gateway (192.168.127.1:53), UDP NAT slab reaper. |

## Scenario Tier

Composition scenarios from `cans/artifacts/test-plan/scenarios.yaml`;
each step references a domain case_id (non-redundancy contract, CDR-09..CDR-12).

| Scenario Tier | Count | Status |
|---|---:|---|
| lifecycle | 5 | pass |
| agent | 5 | pass |
| concurrency | 5 | pass |
| recovery | 4 | pass |
| pressure | 4 | pass |
| network | 4 | pass |
| parity | 4 | pass |
| smoke | 4 | pass |

## Scenario Coverage Composition

Per-scenario plane composition (domain → plane map; adjacent pairs enforced by check-scenarios).

| Scenario | Planes Composed |
|---|---|
| sc_lifecycle_full_roundtrip | terminal, storage, wire, checkpoint |
| sc_lifecycle_fork_from_mid_chain | checkpoint |
| sc_lifecycle_rollback_recovers_state | storage, wire, checkpoint |
| sc_lifecycle_suspend_resume_clock_snap | checkpoint |
| sc_lifecycle_graceful_vs_abrupt_shutdown | checkpoint |
| sc_agent_mcp_tool_call_roundtrip | terminal |
| sc_agent_mcp_taint_blocks_mutation | — |
| sc_agent_mcp_schema_drift_halts | — |
| sc_agent_mcp_tool_quota_exhaustion | — |
| sc_agent_agent_loop_with_egress_denial | wire |
| sc_concurrency_n_sessions_isolation | storage, wire, checkpoint |
| sc_concurrency_shared_workspace_lock_contention | storage |
| sc_concurrency_concurrent_catalog_writes | storage, checkpoint |
| sc_concurrency_port_forward_no_collision | wire |
| sc_concurrency_balloon_pressure_under_n_sessions | host |
| sc_recovery_host_crash_reattach | checkpoint |
| sc_recovery_detached_lease_expiry_sweep | storage, checkpoint |
| sc_recovery_catalog_corruption_fail_closed | storage, checkpoint |
| sc_recovery_partial_scm_rights_abort | checkpoint |
| sc_pressure_sustained_memory_pressure_converges | checkpoint, host |
| sc_pressure_sustained_disk_pressure_quota_holds | storage |
| sc_pressure_sustained_wal_growth_rotates | — |
| sc_pressure_sustained_wire_udp_reaper | wire |
| sc_network_allow_deny_matrix | wire |
| sc_network_dns_split_horizon_under_policy | wire |
| sc_network_l4_diversion_with_rollback | wire |
| sc_network_park_and_suspend_webhook | wire, checkpoint |
| sc_parity_full_roundtrip | terminal, storage, wire, checkpoint |
| sc_parity_error_surface | — |
| sc_parity_session_lifecycle_verbs | checkpoint |
| sc_parity_plugin_mount_surface | — |
| sc_smoke_boot_exec_shutdown | terminal, checkpoint |
| sc_smoke_mcp_roundtrip | — |
| sc_smoke_rollback_clean | storage, wire |
| sc_smoke_catalog_reattach | checkpoint |

## Test Plan Coverage

Status counts from `cans/artifacts/test-plan/case-to-test-map.yaml`;
state machine is gap → red → implemented, one direction only.

| Implemented | Red | Gap |
|---:|---:|---:|
| 72 | 107 | 75 |

## Hypervisor Performance Floor (Budgets)

Thresholds below come from perf/budgets.toml (enforce = true) where a key is declared; rows without a declared key keep their spec-fixed floor.

| Scenario | Target Metric | Enforcement | Status |
|---|---|---|---|
| Cold Boot to Workload Exec | ≤ 60 ms p95 | Hard Budget (Stopwatch) | pass |
| Hardware RAM/vCPU Rollback | ≤ 8 ms p95 | Hard Budget (Driver) | pass |
| Suspend to NVMe Page Dump | ≤ 50 ms p95 | Hard Budget (io_uring) | pass |
| Gate 2 Native Scope Eval | ≤ 2 µs p95 | Hard Budget (Native Rust) | pass |
| Fast-boot mmap hydration | ≤ 500 µs p95 | CI (KVM/HVF) | pending |
| Idle memory contraction | ≤ 4 MB p95 | CI (KVM/HVF) | pending |
| Host Wired RSS Post-Suspend | 0 MB (Purged) | Mach / madvise Purge | pass |

## Platform Executable Subset

Host: linux/x86_64; hypervisors present: none. Executable on this host: 227 / 254 cases.

| Hypervisor | Cases | Executable Here |
|---|---:|---:|
| `avf` | 2 | 0 |
| `emulated` | 4 | 2 |
| `hvf` | 6 | 0 |
| `kvm` | 13 | 0 |
| `none` | 227 | 225 |
| `whp` | 2 | 0 |

## Anti-Slop Enforcement

| Law | Rule Enforced | Enforcement Mechanism | Status |
|---|---|---|---|
| AS-01 | Banned Hypervisor Mocking | Compiler AST analysis (`check-tests`) | pass |
| AS-02 | Physical Mutation & Rollback Proof | Sparse upper block probing (st_blocks == 0) | pass |
| AS-03 | Typed Verdict & Receipt Assertion | Deny untyped `.is_ok()`/`.is_err()` | pass |
| AS-04A | Deterministic Replay | Wall-clock sampling, PMU cross-host comparison, and unseeded randomness banned in deterministic runs | pass |
| AS-04B | Physical Budget Floor | Benchmark suites enforce perf/budgets.toml thresholds (enforce = true) | pass |
| AS-05 | Merkle WAL Chain Integrity | RFC 6962 BLAKE3 leaf chain verification | pass |
| AS-06 | Anti-Tautology Assertion | Hardcoded literals enforced in test fixtures | pass |
| AS-07 | Hardware Platform Tier | Strict KVM/HVF/WHP probing via `doctor` | pass |
| AS-08 | Zero-Dangling Host Resources | Post-run host FD and tmpfs sweep | pass |
