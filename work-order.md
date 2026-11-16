# `work-order.md` — Follow-Up Implementation Work Order

**Sources of intent:**
- `patch.md` — 8-part example-surface patch series (Rust/Python/TS + `kelir.toml`/`kelir.yaml`), eliminating every `TODO (L3)` / `L3-GAP` by calling live APIs.
- `patch2.md` — 6-part normative specification sweep (`cans/*.md`), formalizing the same 15 primitives.

**Core rule:** the patched examples are now **acceptance tests**. The real codebase must be grown until every patched example compiles, links, boots, and passes its in-file `assert!`/`assert_eq!`/`throw` gates without any `TODO (L3)` / `L3-GAP` / speculative stub remaining. The patched `cans/*.md` are **normative**: CI must fail on doc↔code drift.

---

## 0. Primitive → Workstream Coverage Matrix

| Prim | Name (short) | Spec home(s) | Example acceptance test(s) | Workstream |
|---|---|---|---|---|
| P1 | In-ring L7 HTTP/TLS ingress | `01-arch`, `07-wire`, `11-proto` | `serverless-webhook.{rs,py,ts}`, `autonomous-swe-agent.rs` mTLS | **WS-B1, WS-C7** |
| P2 | Granular task freeze + synthetic refusal | `08-life`, `11-proto` | `policy-matrix.rs`, `serverless-webhook.*` | **WS-B4, WS-D** |
| P3 | 16 MB zero-copy SHM streaming | `01-arch`, `11-proto`, `mcp.md` | `serverless-webhook.*`, `mcp` frame path | **WS-B2, WS-A** |
| P4 | Async worker pool + hook deadlines | `02-plugin`, `profile.md` | all async hooks (200 ms default) | **WS-B3, WS-A** |
| P5 | Signed operator lease / dynamic policy | `01-arch`, `02-plugin`, `profile.md` | `policy-matrix.rs` reload path | **WS-B5, WS-F** |
| P6 | Unified cross-ring capability access | `03-sdk`, `08-life`, `db-backed-runtime.*`, `autonomous-swe-agent.rs` | `ctx.cap::<T>()`, `c.cap(...)` | **WS-A1** |
| P7 | Pre-redaction RFC 6962 dual hash + HookSpan + witness | `10-attest`, `ledger.md` | `db-backed-runtime.*`, `hook-matrix-full.rs` | **WS-E1, WS-E2** |
| P8 | Multi-tenant catalog (Postgres) + rootless fallback | `06-cat`, `01-arch`, `10-attest` | `multi-vm-swarm.py`, `kelir.*` catalog block | **WS-D1, WS-D2** |
| P9 | Outbox deferral + 2PC savepoints | `07-wire`, `transaction.md`, `db-proxy.md` | `db-backed-runtime.*`, `autonomous-swe-agent.rs` | **WS-B6, WS-C3** |
| P10 | Branching step DAG + signed attribution / SoD | `08-life`, `09-snap` | `hook-matrix-full.rs`, `policy-matrix.rs` | **WS-B7** |
| P11 | Dynamic OCI ingestion (interior mutability) | `container.md`, `12-adapters` | `container.{rs,py,ts}` | **WS-C1** |
| P12 | Telemetry ring + 16-dim feature extraction | `14-obs`, `hook-matrix-full.rs` | `hook-matrix-full.rs` counters | **WS-B8** |
| P13 | MCP rug-pull / AST semantic diff | `mcp.md`, `tool-guard.md` | `tool-guard` paths in SWE agent | **WS-C5** |
| P14 | Cross-VM virtio_shm SPSC/MPMC + vector clocks + winner election | `mesh.md`, `01-arch` | `multi-vm-swarm.{rs,py,ts}` | **WS-B9, WS-C4** |
| P15 | OrgScope envelope + VFS containment | `01-arch`, `05-storage`, `06-cat`, `profile.md` | `kelir.yaml` `isolation_envelope`, `multi-vm-swarm` | **WS-B10, WS-D** |

---

## WS-A — Interface & SDK Surfaces

**Targets:** `crates/kelir-interface`, `crates/kelir-engine` facade re-exports, `packages/py` (`kelir`), `packages/ts` (`@kelir/core`).

### WS-A1 — Capability DAG accessor (P6, P4)
- [ ] Replace positional `HCons`-only lookup with a dynamic DAG registry. Keep `HCons` for typestate compile-time bounds, but add runtime resolution.
- [ ] Rust: `Session::cap::<T>() -> Option<&T>` and `HandlerCtx::cap::<T>() -> Option<&T>` where `T: Capability`; **infallible** variants (`cap_infallible`) only under existing `Has<T>` bound.
- [ ] `LifecycleCtx::cap::<T>() -> Option<&T>` — must deliver `LedgerHandle`/`LedgerCtx` on **both** `VmReady` and `VmShutdown` (fixes every "pre-boot capture" workaround).
- [ ] TS: `session.cap<T>(name): T | undefined`, `c.cap<T>(name)`, `ctx.cap<T>(name)`.
- [ ] Py: `session.cap("ledger")`, `c.cap("ledger")`, `ctx.cap("ledger")` returning `Optional[...]`.
- [ ] Canonical keys registered: `ledger`, `db-proxy`, `tool-guard`, `transaction`, `container`, `mesh`, `vault`, `replay`.
- **Acceptance:** `db-backed-runtime.rs` no longer captures `ledger_ctx` pre-boot; `autonomous-swe-agent.rs` shutdown handler flushes via `ctx.cap::<LedgerHandle>()`.

### WS-A2 — New typed handles (P6, P9, P11, P14)
Add and re-export from the `kelir` facade and both SDKs:
- [ ] `ContainerHandle { async ingest_image(path: &Path) -> Result<ImageId> }` — **`&self`**, thread-safe interior mutability (`Arc<Mutex<...>>`). Delete the `&mut self` bottleneck. (P11)
- [ ] `SqlProxyHandle { async rewrite_target() -> &str }` (P9)
- [ ] `TransactionHandle { async scope(name, paths, schemas) -> Tx; }`, `Tx { savepoint, rollback_to_savepoint, commit }` (P9)
- [ ] `LedgerHandle { flush, finalize(FinalizeRequest), get_causal_delta_from_parent, blake3_of_producing_process(path), count_denials(pid) }` (P7)
- [ ] `MeshHandle / MeshCtx { broadcast_winner(shard_id), await_winner(Duration) -> u32 }`; TS `broadcastWinner/awaitWinner(ms)`, Py `broadcast_winner/await_winner(timeout_secs)` (P14)
- [ ] `EphemeralCredentials { placeholder, ttl_seconds }` re-exported with `Default` (P9)
- [ ] `Errno::Epolicy` added to closed set; spec `02-plugin-model` conflict note updated (P2)
- **Acceptance:** `container.rs`, `db-backed-runtime.*`, `multi-vm-swarm.*` compile against these types only.

### WS-A3 — Ingress & spawn surfaces (P1, P2, P6)
- [ ] `Session::handle_ingress(port, sock_path) -> IngressChannel<Req, Res>` with `next_request()`, `stream.request()`, `stream.respond(res)`. Same surface in Py (async iterator) and TS (`for await`). (P1)
- [ ] `Session::spawn(bin, args, opts { input, cwd, params, env })` unified across SDKs; ensure `input` is actually plumbed to guest stdin.
- [ ] `LifecycleCtx::exec_in_guest(args)` for `VmReady` (npm ci path).
- [ ] `ctx.vm.adjust_balloon(n)` and `ctx.vm.adjustBalloon(n)` aliases; fix the `spec spelling vs live` drift noted in the TS patch.
- [ ] `Session::vsock.channel(port)` multiplexed stream (needed by `policy-matrix.rs` 0x0013).
- [ ] `Session::shm(region) -> ShmMapping` (P14).
- [ ] `LifecycleEvent::{VmInit, VmReady, VmShutdown}` typed events on builder `.on(...)` in all three SDKs; `on_vm_shutdown` legacy alias kept.
- **Acceptance:** `serverless-webhook.*` serve ≥1 webhook and survive a `HumanAuthorizationHold` suspend; `policy-matrix.rs` vsock ping-pong asserts `policy-matrix:ping`.

### WS-A4 — Builder surface parity (P8, P12, P14)
- [ ] Py `.lock_broker(globs=[...], non_blocking=True)` (fixes `multi-vm-swarm.py`).
- [ ] Top-level compute invariants `network_off`, `ns_per_instruction`, `entropy_seed` accepted by **builder** (not just replay plugin config) in Py/TS; map onto `DriverPolicy`/replay engine. (P2, P14)
- [ ] `session.checkpoint.{create, create_step, commit_step, revert_step, rollback}` consistent signatures across SDKs.
- **Acceptance:** `replay.*` builder compiles without the removed TODO and `date +%s.%N` is byte-identical across replay checkpoints.

---

## WS-B — Engine Runtime Primitives

**Targets:** `crates/kelir-engine`, `crates/kelir-interface`, `crates/kelir-checkpoint`, `crates/kelir-storage*`.

### WS-B1 — In-ring L7 HTTP/TLS parsing (P1)
- [ ] Native L7 stream parser in `wire` plane: HTTP/1.1 request/response, header splitting, SSE, JSON-RPC bodies; TLS decrypt via host keyring (no external sidecar).
- [ ] Rewrite in-place without breaking keep-alive; preserve attestation lineage.
- [ ] Remove `vault.external_egress = { envoy | squid }` code path entirely (spec `vault.md` patched).
- **Acceptance:** webhook responses validated; `autonomous-swe-agent.rs` L4 proxy on 5432 injects mTLS cert/key at boundary without guest TLS termination.

### WS-B2 — 16 MB zero-copy SHM streaming (P3)
- [ ] `max_buffer_bytes = 16_777_216`; `zero_copy_shm_threshold_bytes = 65_536`.
- [ ] Frame flag `0x20 CONTINUATION` and `0x40 SHM_OFFLOAD` implemented in mux; CBOR string cap raised to 16 MB.
- [ ] Handshake capability bit `0x08` advertised and negotiated; mismatch behavior unchanged (`E_INCOMPATIBLE_SHIM_VERSION`).
- [ ] mux frame path, MCP framing, tool-guard `max_frame_bytes` all honor 16 MB with virtqueue progress.
- **Acceptance:** a >64 KB MCP frame round-trips via SHM_OFFLOAD; `hook-matrix-full.rs` payload path >64 KB passes.

### WS-B3 — Async worker pool & hook deadlines (P4)
- [ ] `worker_threads = 8` default (was 2); `allow_async_worker_yield = true`.
- [ ] Slow L2/L3 evaluators offload to worker queue; Gate 2 fast path stays <2 µs.
- [ ] Per-hook timeout defaults: sync rings (Fs/Net/Host) 5 ms; **async policy hooks 200 ms**. Timeout returns `Action::Drop{EWOULDBLOCK, "hook timeout"}`; note the `Errno` set conflict is owned by the enum home.
- **Acceptance:** a deliberately slow evaluator does not stall sibling virtqueues in `policy-matrix.rs` loop.

### WS-B4 — Granular task freeze & synthetic refusals (P2)
- [ ] `granular_task_freeze = true`: freeze only the trapped task cgroup, not the whole VM; `cgroup_parent` honored.
- [ ] `rejection_mode = synthetic_refusal`: denied tool invocations return `MSG_EXEC_REFUSAL` synthetic bodies without aborting the VM; traps disarmed.
- [ ] Inbound TCP during `HumanAuthorizationHold` parks and resumes without `TCP RST`; rollback issues clean RST.
- **Acceptance:** webhook connection survives hold; `policy-matrix.rs` custom-quarantine path records `quarantined_pids` and VM continues.

### WS-B5 — Signed operator lease / dynamic policy plane (P5)
- [ ] Split policy plane into **immutable hardware invariants** (MMU W^X, capability lattice monotonicity) vs **mutable dynamic rules**.
- [ ] `policy_plane_mutability = "dynamic_signed"`; `inheritance_lattice = "monotonic_with_signed_operator_lease"`.
- [ ] Hot-reload dynamic fragments on `PROFILE_SYNC` stream; signature verification; leases cannot loosen hardware invariants.
- **Acceptance:** `kelir.toml`/`kelir.yaml` load; a signed lease fragment hot-reloads without VM restart; unsigned lease rejected `ESecurityViolation`.

### WS-B6 — Egress outbox deferral (P9)
- [ ] Buffer outbound frames for irreversible network ops up to `67108864` bytes; hold physical egress until step verification.
- [ ] Release on step verification; purge on rollback (<8 ms target); compensating undo webhook on failure (`compensator_webhook_timeout_ms = 5000`).
- [ ] Wire into `plugin-transaction` 2PC commit/abort path.
- **Acceptance:** `db-backed-runtime.*` migration rollback leaves zero leaked egress frames.

### WS-B7 — Step DAG, attribution, SoD (P10)
- [ ] `checkpoint.step_dag` content-addressed storage; branching head nodes.
- [ ] Ed25519 reviewer attribution on step approvals; `require_separation_of_duties` rejects same-actor steps.
- [ ] `snapshot` archive retains signed DAG + dependency lineage; Merkle root over step heads.
- **Acceptance:** `hook-matrix-full.rs` builds a 2-branch DAG and verifies roots; `policy-matrix.rs` attribution asserts pass.

### WS-B8 — Telemetry ring + feature extraction (P12)
- [ ] 300-entry sliding ring; normalized 16-dim feature vector (syscall rate, arg entropy, burst, etc.); zero-copy export to host anomaly filter.
- [ ] OTLP exporter endpoint from `plugins.ledger.otel.endpoint` (`http://localhost:4318/v1/traces`).
- **Acceptance:** `hook-matrix-full.rs` asserts `handle.traps() > 0`, `fs_locks()>0 || exec_stdout()>0`, `failures()==0`.

### WS-B9 — Cross-VM SHM mesh (P14)
- [ ] `virtio_shm` SPSC/MPMC regions shared across sibling microVMs; hardware vector-clock attribution on write.
- [ ] `broadcast_winner` / `await_winner(timeout)` election; deterministic winner rule.
- [ ] SPIFFE identity bound to mesh peers.
- **Acceptance:** `multi-vm-swarm.*` 8-shard run elects exactly one winner; `await_winner` returns it in ≤2 s.

### WS-B10 — OrgScope isolation envelope (P15)
- [ ] Session binds immutable `tenant_id` / `org_id`; storage/cgroups/virtqueues partitioned by `(tenant_id, session_id)`.
- [ ] Host VFS containment resolves beneath `tenant_root`; `openat2(RESOLVE_BENEATH|RESOLVE_NO_MAGICLINKS)` enforced; cross-tenant traversal fails closed.
- **Acceptance:** `kelir.yaml` `isolation_envelope.enforce_openat2_beneath` honored; negative test cannot escape tenant root.

---

## WS-C — Plugin Implementations

### WS-C1 — `plugin-container` (P11)
- [ ] Replace `&mut self` service with `ContainerHandle` interior-mutability impl; async `ingest_image` streams raw OCI tar to EROFS/VirtIO-FS without guest overlayfs.
- [ ] Re-export handle from facade; fix TS/Py cap lookup.
- [ ] Enforce guest privilege confinement: `raw_device_mount` and `/dev/kvm` access return `EPERM` at hypervisor intercept.
- **Acceptance:** `container.{rs,py,ts}` ingest `oci-hello-world.tar`, spawn `podman run --rm --rootless`, exit 0.

### WS-C2 — `plugin-ledger` (P7)
- [ ] Dual hash `RFC_6962_DUAL_BLAKE3_SHA256`; leaves include `0x00 || record_bytes`, nodes `0x01 || left || right`.
- [ ] `record_pre_redaction_hash`, `include_hook_spans_in_root`.
- [ ] STH witness cosigning client (`witness_cosigning_endpoint`); sign leaves; fsync after reap.
- [ ] `finalize(FinalizeRequest { include_delta: Option<CausalDelta>, include_merkle_root })`.
- **Acceptance:** `db-backed-runtime.rs` final receipt carries non-empty `merkle_root` + valid `signed_jws`; HookSpan inclusion proof verifies.

### WS-C3 — `plugin-transaction` + `plugin-db-proxy` (P9)
- [ ] 2PC scope/savepoint/rollback; `on_pre_commit` flush with `ctx.cap::<LedgerHandle>()`.
- [ ] `flush_fuse_before_commit` mandatory (reject `false` with `ESecurityViolation`).
- [ ] Coordinate outbox deferral; discard buffered side effects on abort.
- **Acceptance:** `db-backed-runtime.*` rollback restores savepoint; no leaked outbox frames.

### WS-C4 — `plugin-mesh` (P14)
- [ ] `broadcast_winner` / `await_winner`; virtio_shm peer rings; vector clocks; mTLS framing.
- **Acceptance:** covered by `multi-vm-swarm.*`.

### WS-C5 — `plugin-tool-guard` + `plugin-mcp` (P13, P3)
- [ ] JSON-RPC AST parser with type validation before dispatch; track transaction IDs.
- [ ] `tools/list` semantic AST diff; halt on description rewrite / smuggled params (`ESecurityViolation`), emit audit telemetry before halt.
- [ ] Byte-range taint propagation from untrusted ingress through args into downstream calls (`propagate_byte_range_taints`).
- [ ] MCP frame max 16 MB via SHM continuation.
- **Acceptance:** a hostile `tools/list` update during SWE run halts with structured event; benign update passes.

### WS-C6 — `plugin-vault` (P1)
- [ ] In-ring carrier secret injection: substitute `vault://` placeholders on outbound L7 streams; drop external sidecar code.
- [ ] mTLS cert/key injection for `pg.internal.net:5432` via `L4Proxy.inject_mtls`.
- **Acceptance:** SWE agent reaches Postgres with injected mTLS; plaintext secret never in guest RAM.

### WS-C7 — `plugin-wire-ingress` (P1, P2)
- [ ] Splice ingress directly to guest shim via vsock stream; parked connections survive holds; rollback purges with clean RST.
- **Acceptance:** `serverless-webhook.*`.

### WS-C8 — `plugin-replay` (P2, P14)
- [ ] Consume top-level `network_off`, `ns_per_instruction`, `entropy_seed`; `deterministic_clock`, `pinned_clocksource="kvm-clock"`, `preemption_grace_ms=0`.
- [ ] `clock.skew(...)` per fork supported for hypothesis branches.
- **Acceptance:** `replay.*` byte-for-byte identical `date +%s.%N` across checkpoint/rollback cycle.

---

## WS-D — Config, Profile & Catalog

### WS-D1 — Profile schema (P1–P15)
- [ ] Parse and validate every key introduced in `kelir.toml`/`kelir.yaml`:
  - core: `policy_plane_mutability`, `inheritance_lattice` new enum value, `allow_emulated_fallback`, `emulated_tier_classification="Class2_RootlessUserNS"`.
  - driver: `ffi_boundary.{worker_threads, max_ops_per_second, queue_capacity, max_buffer_bytes, zero_copy_shm_threshold_bytes, allow_async_worker_yield}`.
  - session: `org_id`/`tenant_id`, `isolation_envelope.{enforce_openat2_beneath, cgroup_parent}`, `shim.{granular_task_freeze, rejection_mode}`.
  - wire: `l7_protocol_inspection`, `outbox_deferral.{enabled, buffer_capacity_bytes, release_on_step_verification, drop_on_rollback, compensator_webhook_timeout_ms}`.
  - checkpoint: `step_dag.{enabled, require_signed_attribution, require_separation_of_duties, storage="content_addressed"}`.
  - host: `catalog.{backend, endpoint, schema_tenant_partitioning, fallback_sqlite_path}`, `telemetry_ring.{feature_extraction, sliding_window_depth, export_feature_vector}`.
  - ledger: `hashing_standard`, `record_pre_redaction_hash`, `include_hook_spans_in_root`, `witness_cosigning_endpoint`, `otel.endpoint`.
  - mcp/tool-guard: `max_frame_bytes`, `detect_semantic_rug_pull`, `strict_ast_parameter_validation`, `propagate_byte_range_taints`.
- [ ] Strict monotonic validation on all budget fields; reject overlays widening ceilings (`ESecurityViolation`).
- [ ] Map every key to the corresponding `DriverPolicy`/`VmConfig`/`*Config` field (no `TBD`).
- **Acceptance:** `kelir.toml` and `kelir.yaml` parse to identical effective config (parity test).

### WS-D2 — Catalog backends (P8, P15)
- [ ] Add `backend = "postgres" | "sqlite"`; `fallback_sqlite_path`; `schema_tenant_partitioning = true`.
- [ ] Schema: `sessions(tenant_id, session_id)` composite unique; `cas_leases` TTL 30 s; `incarnation_id` UUIDv7; `boot_id`, `start_time_ns`, `pid`.
- [ ] Migrations + tenant partition DDL; WAL mode preserved for SQLite.
- **Acceptance:** Postgres catalog boots; tenant partition test isolates two tenants; SQLite fallback works offline.

---

## WS-E — Attestation & Ledger Integrity

### WS-E1 — Receipt & Merkle contents (P7, P10)
- [ ] Receipt leaves include `HookSpan` policy descriptors and pre-redaction payload digests (raw committed before mask).
- [ ] Branching step DAG represented in receipt lineage; signed reviewer identity embedded.
- [ ] Inclusion proof verifies policy execution without revealing payloads.

### WS-E2 — Class 2 attestation expansion (P8)
- [ ] `Class 2 (Paravirtual Interception & Rootless Containers)` includes unprivileged bwrap/proot namespaces.
- [ ] Higher class never downgrades; Class 2 cannot satisfy Class 1 assertions.

**Acceptance (WS-E):** `hook-matrix-full.rs` fails if `pre_redaction` hash missing on any masked record; `db-backed-runtime.rs` causal delta appears in finalized receipt.

---

## WS-F — CI Gates, Docs Parity & Cleanup

### WS-F1 — Example-as-test wiring
- [ ] Add each patched example to CI matrix:
  - `cargo run --example {serverless_webhook, container, db_backed_runtime, multi_vm_swarm, autonomous_swe_agent, policy_matrix, git_gate, hook_matrix_full, replay}` on the target VMM (hardware tier required for `git_gate`, `hook_matrix_full`; emulated Class 2 allowed for others).
  - Py: `pytest examples/` with the same files.
  - TS: `npm test -- examples/`.
- [ ] Remove any `#[ignore]`/skip annotations added while stubbing.

### WS-F2 — Coverage gates (from `cans/15-operations.md`, patch2 Part 6)
- [ ] `--l7-stream-coverage` (P1): assert L7 parsing exercised.
- [ ] `--outbox-coverage` (P9): assert buffered frames released on commit, purged on rollback.
- [ ] `--step-dag-coverage` (P10): assert branching DAG + attribution + SoD paths.
- [ ] `--swarm-mesh-coverage` (P14): assert ≥1 broadcast/await winner cycle.
- [ ] Remove legacy vector-regeneration prose from `15-operations.md`.

### WS-F3 — Doc↔code parity
- [ ] CI job `spec-drift`: every normative statement in patched `cans/*.md` maps to a code symbol or test; fail on missing.
- [ ] Purge deprecation apologetics from `01-architecture.md`, `02-plugin-model.md`, `03-sdk-architecture.md` (done in specs; ensure no code comments resurrect them).
- [ ] Reconcile SSOT pins: `DriverPolicy` field names, `Errno` set, `TrapFilter` builder set, capability keys.

### WS-F4 — Comment hygiene
- [ ] `grep -R "L3-GAP\|TODO (L3)\|aspirational\|TBD (no .* field"` must return **zero** hits across `cans/artifacts/examples/**` and `cans/**`.
- [ ] Any remaining `TODO` requires a linked issue and an owner; none may hide a primitive.

---

## Sequencing & Dependencies

```
Phase 0 (unblock):      WS-A1, WS-A2, WS-A3, WS-A4   [SDK surfaces]
Phase 1 (core runtime): WS-B2, WS-B3, WS-B4          [SHM, workers, freeze]
Phase 2 (planes):       WS-B1, WS-B6, WS-B10         [L7, outbox, tenant]
Phase 3 (state/audit):  WS-B7, WS-E1, WS-C2          [DAG, ledger, dual hash]
Phase 4 (plugins):      WS-C1, WS-C3, WS-C4, WS-C5, WS-C6, WS-C7, WS-C8
Phase 5 (fleet):        WS-B9, WS-D2                 [mesh, catalog]
Phase 6 (policy):       WS-B5                        [signed lease hot-reload]
Phase 7 (observability):WS-B8
Phase 8 (gates):        WS-F1..WS-F4
```

Hard blockers:
- WS-A2 `ContainerHandle` blocks WS-C1 and `container.*` acceptance.
- WS-B2 SHM continuation blocks `mcp` 16 MB acceptance.
- WS-A1 `LifecycleCtx::cap` blocks WS-C2 flush and WS-C3 pre-commit.
- WS-D1 schema must land before any example uses new keys (otherwise boot fails).

---

## Definition of Done

1. `patch.md` examples compile and pass on hardware tier (or documented Class 2 tier for emulated cases).
2. `patch2.md` normative spec has a `spec-drift` green run against code symbols.
3. All four CI coverage gates green: `--l7-stream-coverage`, `--outbox-coverage`, `--step-dag-coverage`, `--swarm-mesh-coverage`.
4. Zero `L3-GAP` / `TODO (L3)` / speculative `TODO` in examples and specs.
5. `kelir.toml` ≡ `kelir.yaml` effective-config parity test passes.
6. A single end-to-end run of `autonomous-swe-agent.rs` produces a finalized receipt with `include_delta = Some(CausalDelta)`, `include_merkle_root = true`, and a verifiable witness cosignature.

---

## Risks / Open Questions

- **`Errno` closed set:** `Epolicy` addition and the `ETIMEDOUT` question noted in `02-plugin-model.md` need the enum home owner to sign off.
- **External sidecar removal (vault):** confirm no downstream consumer depends on `envoy`/`squid` diversion; provide migration note.
- **16 MB SHM ring memory footprint:** verify balloon (`adjust_balloon`) and `VmReady` shrink interact correctly on constrained SBC profile.
- **Postgres catalog multi-node:** define HA/failover semantics before fleet rollout (fallback SQLite is per-node only).
- **Lease hot-reload safety:** signed lease must not be able to widen hardware invariants — enforce at seal plus on every reload.