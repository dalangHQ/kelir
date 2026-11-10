# work-order.md

**Scope:** Fix L1/L2-phase defects in kelir: silent no-ops in security-boundary code, cross-SDK incoherence, structural duplication, dead config fields. **Not in scope:** implementing `TODO (L3)` markers (real hypervisor boot, SPSC-ring wire framing, snapshot archive, interpreter payload parser, DNS resolver, PTY, LLM-call gate).

**Phase contract:**
- **L1** = compiles, tests pass, lints clean.
- **L2** = behavior matches the cited `cans/` anchor; no security-boundary path silently allows what the anchor denies.
- **L3** = `TODO (L3)` is real. **Agents must not implement L3.** If a fix requires an L3 primitive, downgrade the fix to "correct L2 shape + typed error or `tracing::warn!`" and leave the `TODO (L3)` in place.

**Hard rules:**
1. Every commit cites one or more `cans/...md#Anchor` in its message. A PR without anchors is rejected.
2. Do **not** edit `cans/`. If a premise doesn't hold up against the spec, add an entry to `_collab/conflicts.md` and stop.
3. Do **not** add new features, plugins, wire messages, or SDK verbs beyond what a fix explicitly names.
4. Do **not** delete `TODO (L3)` comments — leave them, or upgrade them to `tracing::warn!` + typed error where the code currently lies.
5. Build isolation per `cans/AGENTS.md`: `CARGO_TARGET_DIR=target/agent-<id>`.
6. **File ownership is exclusive** (see §6). Cross-agent edits require a `_collab/handoffs.md` entry.

---

## 1. Agent A — Plugin Integrity

**Files owned:** `crates/kelir-plugins/**`, `crates/kelir-interface/src/plugin_model.rs` (MountScope only), `crates/kelir-core/src/session_lifecycle.rs` (only the `revoke_session_tokens` call in `rollback_to`).

**Mission:** make the security-boundary plugins fire where the spec says they must.

| # | Fix | Spec anchor | Current bug | Required change |
|---|---|---|---|---|
| A.1 | tool-guard binds only on `MCP_CONSOLE` | `cans/plugins/tool-guard.md#Total-tool-call-interposition` | `register_interceptors` binds a `ConsoleInterposer` with `MountScope::default()` (all console ports) | Add a stream-kind filter to `MountScope` (or a dedicated `bind_console_for_stream_kinds`). Bind with `[StreamKind::McpConsole]` only. PTY port 0 and EXEC stdout/stderr must not reach the guard. |
| A.2 | tool-guard locks on first `tools/list`, not at `on_vm_ready` | `cans/plugins/tool-guard.md#Immutable-schema-locking` ("Intercepts initial tools/list advertisement … locks the catalog") | `on_vm_ready` calls `lock_catalog()` against the empty catalog; first real advertisement is rejected `ESecurityViolation` | Move the lock to fire after the first `ingest_tools_list`. Subsequent differing advertisements → `ESecurityViolation`. |
| A.3 | wire-ingress uses the L4_FORWARD stream kind, not the L2 net ring | `cans/plugins/wire-ingress.md#Ingress-stream-framing-on-stream-kind-L4-FORWARD-(mux-0x0001)` | `on_net_inbound` calls `L4ForwardFrame::decode` on raw Ethernet bytes → always `None` | Bind a stream-kind handler for `StreamKind::L4Forward` and decode there. Keep the `NetInterposer` only if a real use exists (currently none) — otherwise remove. |
| A.4 | `MountScope::allows_binary` matches by basename | `cans/11-protocol.md#Trap-fast-path` ("binary_path is a display hint; policy never matches on it alone"), `cans/02-plugin-model.md#Filter-before-FFI-gate` ("Gate 2 evaluates canonical identity") | `item == binary` compares basename against full path — never matches, so container policy is skipped | Compare `Path::new(binary).file_name()` against `item`. Add `// TODO (L3): canonical (st_dev, st_ino, binary_blake3) — cans/11-protocol.md#Trap-fast-path`. |
| A.5 | vault revocation is keyed on `session_id`, not `incarnation_id` | `cans/plugins/vault.md#Revocation`; `cans/08-lifecycle.md#Rollback` | `Session::rollback_to` calls `revoke_session_tokens(&encode_hex(&self.incarnation_id))`; `on_vm_init` binds `ctx.session_id`. Nothing is revoked. | Change the one line in `kelir-core/src/session_lifecycle.rs` to `&encode_hex(&self.session_id)`. |
| A.6 | container `Default` is safe | `cans/plugins/profile.md#Default-zero-capability-invariant` | `Default` sets `isolate_var_lib_containers: true` + `scratch_device: None`; `on_vm_ready` hard-errors on `None` | Default `isolate_var_lib_containers: false`. Caller opts in to both fields. Document the pairing on `ContainerConfig`. |
| A.7 | `FaultDirection` lives in `kelir-interface` | `cans/plugins/chaos.md#Network-chaos` | `FaultDirection` exists only in `plugin-chaos`; TS/Py re-declare a literal union | Move to `kelir-interface` (e.g. `chaos_types.rs`), re-export from `plugin-chaos`. Do **not** touch SDK sources (codegen picks it up). Verify `kelir-interface/src/bin/codegen/model_codegen.rs::classify` handles it. |
| A.8 | one trait impl per service | `cans/02-plugin-model.md#KelirPlugin-trait` | `SqlProxy` + `SqlProxyCtx` both impl `SqlProxyService`; `SharedContainerService` + `ContainerCtx` both impl `ContainerService` | Keep impl on the concrete service. Context type only exposes `service() -> &Arc<Inner>`. Callers use `ctx.service().dialect().await`. Matches `TransactionCtx::coordination()`. |
| A.9 | remove empty trait impl | `cans/plugins/ledger.md#Invariants-and-policy-(ledger)` ("Pure reactive observer") | `impl TrapInterposer for LedgerPlugin {}` — empty, misleads | Delete the impl. |

**Accept:** `cargo nextest run -p plugin-tool-guard -p plugin-wire-ingress -p plugin-container -p plugin-vault -p plugin-chaos -p plugin-db-proxy -p plugin-ledger` passes. `kelir-plugin-cli check -p <each>` passes. Manual: `sh -c` is not trusted-skip; tool-guard trap on port 0 not fired; tool-guard trap on MCP_CONSOLE fired; container `--privileged` drop fires; rollback revokes a token issued before rollback.

---

## 2. Agent B — SDK Surface Parity

**Files owned:** `crates/kelir/src/{session_lifecycle, sandbox_runtime, builder_sdk}.rs`, `packages/ts/core/src/{session.lifecycle, builder.lifecycle, sandbox.runtime, verdict.protocol, profile.schema}.ts`, `packages/py/kelir/{__init__, lifecycle_sandbox, profile_config}.py`.

**Mission:** three SDKs agree on verbs, return types, and semantics for L2-observable behavior.

| # | Fix | Spec anchor | Current bug | Required change |
|---|---|---|---|---|
| B.1 | one `relay_socket_path` | `cans/08-lifecycle.md#Detached-lifecycle` | Three reimplementations (Rust / TS / Py) | Expose from the native bridge (`_kelir`/NAPI). Delete pure-language implementations. Verify identical output for `"ABCDEF0123456789ABCDEF0123456789"`. |
| B.2 | `Sandbox` facade parity | `cans/03-sdk-architecture.md#Sandbox-high-level-facade` | Rust lacks `resume`; `terminate` == `stop` in Rust; `exec` return type differs across SDKs | Add `Sandbox::resume(path)`. `Sandbox::terminate` = `self.session.kill()` (abrupt, per `cans/08-lifecycle.md#Pre-teardown-flush`). Add `Sandbox::stop_with_timeout(d)`. See B.3 for `exec`. |
| B.3 | `exec`/`spawn` return types agree | `cans/03-sdk-architecture.md#Idiomatic-Surfaces-Law` | Rust `exec`/`spawn` return `SpawnBuilder` (future); TS/Py return `Promise<SpawnResult>` | Make `Sandbox::exec`/`Session::spawn` async and return `Result<SpawnResult>`. Keep `SpawnBuilder` only as an internal type (or add `Session::spawn_builder` if any in-tree caller needs it). |
| B.4 | Python `Verdict.pass_` alias | `cans/03-sdk-architecture.md#Idiomatic-Surfaces-Law` | Python `pass_()`, Rust/TS `Pass` | Add `Pass = pass_` module-level alias in `packages/py/kelir/__init__.py`. Do not rename existing. |
| B.5 | `HandlerContext` convenience methods | `cans/02-plugin-model.md#Unified-verdict` | TS `TrapContext` has `drop/patch/suspend/abort`; Python `HandlerContext` doesn't | Add `drop`, `patch`, `suspend`, `abort`, `pass_` methods to `HandlerContext` that construct `Verdict` (matching TS). Add a vector asserting equal wire bytes. |
| B.6 | `attach` is async everywhere | `cans/03-sdk-architecture.md#Session-surfaces` | Rust `Result<Session>`; TS `Promise<Session>`; Py `Session` | Add Rust `async fn Session::attach(...)`. Signature shapes differ by language; behavior must be identical (relay.sock connect, catalog lease check). |
| B.7 | clock-skew input types | `cans/08-lifecycle.md#Clock-skew` | Rust `Duration`; TS `{days?,hours?,ms?}`; Py `timedelta \| float` | Py accepts `timedelta` only (drop float). TS accepts `{days?,hours?,ms?}`. Rust `Duration`. Add a parity vector asserting `Duration::from_secs(86400)` ⇒ `{days:1}` ⇒ `timedelta(days=1)`. |
| B.8 | `SandboxProfile` round-trips `kelir.yaml` | `cans/plugins/profile.md` (full schema), `cans/artifacts/examples/fixtures/kelir.yaml` | Only `version` + `session` parsed; other sections silently dropped by `export_spec` | Add `driver`, `storage`, `wire`, `terminal`, `checkpoint`, `host`, `plugins`, `attestation` as `Option<serde_json::Value>` (pass-through). Preserve order in `export_spec`. Add `#[serde(deny_unknown_fields)]` to `SandboxProfile` (spec tightens, and `SessionProfile` already has it). Mirror in TS `profile.schema.ts` and Py `profile_config.py`. |

**Accept:** `cargo nextest run -p kelir` passes. `just parity` passes (`kelir-plugin-cli parity run vectors.json`). `Sandbox::from_config("kelir.yaml").export_spec("out.yaml")` byte-round-trips to the fixture.

---

## 3. Agent C — Wire Protocol & Config Schema

**Files owned:** `crates/kelir-interface/src/{protocol_wire, driver_interface}.rs`, `crates/kelir-shim/src/multiplexer_protocol.rs`, `crates/kelir/src/planes/storage_plane.rs`, `crates/kelir-bindings/{napi,pyo3}/src/storage_bridge.rs`.

**Mission:** one wire encoding per type, one config shape per concept.

| # | Fix | Spec anchor | Current bug | Required change |
|---|---|---|---|---|
| C.1 | one hex encoding for `[u8; 32]` | `cans/10-attestation.md#Execution-receipts`, `cans/11-protocol.md#Wire-framing` | `TrapEvent.binary_blake3: [u8;32]`; `ExecutionReceipt.binary_blake3: String`; `CausalDelta` uses `serde_hex_32`; `WalRecord` uses raw bytes | Apply `#[serde(with = "crate::hex_codec::serde_hex_32")]` uniformly: `TrapEvent.binary_blake3`, `ResolvedBinary.blake3`, `ExecutionReceipt.binary_blake3`, `CausalDelta.{storage_delta_root, receipt_leaf_hash}`, `WalRecord.{prev_hash, receipt_blake3, delta_root, signature}`. Update `otel_codegen.rs`, `ts_codegen.rs`, `py_codegen.rs` so projected types remain `String`. |
| C.2 | one `env` representation | `cans/11-protocol.md#Wire-framing` | `TrapEvent.envp: HashMap<String,String>`; `ExecveMutation.set_env: Vec<(String,String)>` with a custom deserializer that accepts both | Pick one. **Recommended:** `Vec<(String,String)>` for both. Add a `serde` alias so both wire forms deserialize. Delete `deserialize_env_pairs`. If the spec does not pin the encoding, mark `TBD` in `_collab/conflicts.md` and pick the map form. |
| C.3 | shim uses `ProcessLimits` | `cans/11-protocol.md#Trap-fast-path` | `Rlimits` in `kelir-shim` has identical fields to `ProcessLimits` | Delete `Rlimits`. `ExecSpec::parse` produces `ProcessLimits`. Shim imports from `kelir_interface`. |
| C.4 | audit `VmConfig` fields | `cans/01-architecture.md#Embedded-profile`, `cans/plugins/replay.md`, `cans/08-lifecycle.md#Watchdog`, `cans/plugins/profile.md#Driver-engine-schema-(profile)` | Most fields are set but never read | **Delete** (not in spec): `ns_per_instruction`, `enable_gpu_passthrough` (CDI is profile-level per `cans/plugins/profile.md#Container-Device-Interface-(CDI)-device-mapping`), `entropy_seed` (replay uses `set_entropy_backend`). **Document with `// TODO (L3): consumed by <driver>` and add `tracing::warn!` in `Session::boot` if non-default and driver doesn't support it:** `page_size_kb`, `pinned_clocksource`, `preemption_grace_ms`. **Kept:** `deterministic_clock`, `tsc_locked`, `network_off`, `enable_watchdog`. |
| C.5 | storage config shape | `cans/05-storage-plane.md#Storage-architecture` | Five shapes (`StorageConfig`, `StorageWiring`, `NativeStorageConfig`, `NativeStorageConfigBinding`, `PyStorageConfig`) + duplicate `MaskReadRule`/`MaskRule` | One canonical Rust struct `StorageConfig` (rename `StorageWiring` → `StorageConfig`; it already has the right shape). Delete `MaskRule` alias (keep `MaskReadRule`). Handler-bearing `snoops` stays in `kelir::planes::storage_plane` and translates to `StorageConfig` at boot. Bindings project to `StorageConfig`. |

**Accept:** `cargo check --workspace --all-targets` passes. `just check-tiered` passes. `just parity` passes. Golden frame bytes in `vectors.json` unchanged except for the specific items above (note delta in PR).

---

## 4. Agent D — Structural Hygiene

**Files owned:** `crates/kelir-interface/src/{plugin_model (PluginError, bind_*_scoped — see §6), telemetry, error_envelope}.rs`, `crates/kelir-bindings/{napi,pyo3}/src/lib.rs`, `crates/kelir-core/src/{plugin_registry, dispatch_plugin}.rs`, `crates/kelir-plugin-cli/**`, `crates/kelir-plugin-testkit/**`.

**Mission:** delete or merge duplicates so the L3 push doesn't inherit four of everything.

| # | Fix | Spec anchor | Current bug | Required change |
|---|---|---|---|---|
| D.1 | one `PluginRegistration` | `cans/02-plugin-model.md#Plugin-ownership` | Four copies (`kelir-core`, `kelir`, `kelir-bindings/napi`, `kelir-bindings/pyo3`) | Define in `kelir-interface::plugin_model`. Re-export from `kelir-core`, `kelir`, both bindings. |
| D.2 | one `lifecycle_ctx` testkit helper | `cans/02-plugin-model.md#Plugin-testkit` | `handlers_testkit::lifecycle_ctx` == `lifecycle_ctx_fixture` | Delete one (keep whichever has callers). |
| D.3 | delete `handler()` | — | `kelir::builder_sdk::handler` is identity and unused | Delete function + re-export from `kelir/src/lib.rs`. |
| D.4 | reverse `bind_*_scoped` default | `cans/02-plugin-model.md#Mount-scoping` (scope intersection is not optional) | Trait default drops the scope and calls the scoped version's sibling | `fn bind_fs(&mut self, stage) { self.bind_fs_scoped(stage, MountScope::default()); }` — the *scoped* method is the primitive. Same for `bind_trap`, `bind_console`, `bind_host`, `bind_net`, `bind_vsock_service`. |
| D.5 | `PluginError: PartialEq` | `cans/02-plugin-model.md#Declarative-error-mapping` (parity asserts on typed errors) | No `PartialEq`; testkit stringifies | `#[derive(Error, Debug, PartialEq, Eq)]`. Wrap `Io(std::io::ErrorKind)` (newtype) so `Eq` compiles. |
| D.6 | `exec_started`/`exec_finished` pair | `cans/14-observability.md#Metrics-pipeline` (`in_flight_execs` gauge) | `exec_finished` called without `exec_started`; `in_flight_execs` always 0 | Add `exec_started()` at the top of `dispatch_trap_inner` when `event.syscall.nr ∈ {SYS_execve, SYS_execveat}`. |
| D.7 | remove `#[allow(deprecated)]` | — | Stale attribute on `MetricRegistry::exec_finished` | Delete. |
| D.8 | one `parse_plugin_manifest` | `cans/02-plugin-model.md#Auto-generated-manifest-contract` | Duplicated in `kelir-core::plugin_registry` and `kelir-plugin-cli::audit_cli` | `audit_cli` calls `kelir_core::plugin_registry::parse_plugin_manifest`. Delete local `ManifestToml`/`PackageToml`/`MetadataToml`/`KelirToml`/`TierToml`. |
| D.9 | `TrapEvent::new` constructor | `cans/artifacts/code/messages.rs#TrapEvent` | Hand-constructed with `..Default::default()` in testkit + shim | Add `TrapEvent::new(seq, pid, ppid, binary_path)`. Not a full builder — pins required fields. Update `vectors_testkit.rs`, `introspect_trap.rs`. |
| D.10 | dead-field doc | `cans/10-attestation.md#Attestation-provenance-classification` | `AttestationToken` has no constructor or consumer | Add `// TODO (L3): consumed by cans/10-attestation.md#Session-attestation-token verification`. Do not delete (spec-mandated type). |
| D.11 | wire-only field docs | `cans/artifacts/code/messages.rs#TrapEvent`, `cans/11-protocol.md#Trap-fast-path` | `TrapEvent.task_id`, `interpreter_chain`, `shell_payload` declared, never read | Add `// on-the-wire; consumed by <anchor>` doc comments. Do not delete. |
| D.12 | nits | — | `#[allow(dead_code)]` on `subid_owner.name`; `watchdog_action_eq` helper; `UartRingBuffer::read` unused | Remove the spurious `allow`; derive `PartialEq` on `WatchdogAction` and delete helper; delete `UartRingBuffer::read`. Add `tracing::warn!` on duplicate in `resolve_plugin_registrations`. |

**Accept:** `cargo check --workspace --all-targets` passes. `just check-tiered`, `just check-layers`, `just check-plugin-all`, `just check-plugin-coverage`, `just check-hygiene` pass. `cargo nextest run -p kelir-plugin-testkit -p kelir-plugin-cli` passes.

---

## 5. Agent E — Runtime Plumbing

**Files owned:** `crates/kelir-core/src/{trap_gate, uart_terminal, planes/wire, planes/checkpoint, planes/terminal}.rs`, `crates/kelir-core/src/session_lifecycle.rs` (only `Session::boot`/`suspend`/`resume` bodies — see §6).

**Mission:** the primitives plugins depend on actually fire where the spec says they must.

| # | Fix | Spec anchor | Current bug | Required change |
|---|---|---|---|---|
| E.1 | forfeit trusted-prefix skip on `-c`/`-e` | `cans/11-protocol.md#Trap-fast-path` ("The -c/-e string is inspected as code") | `interpreter_argv_escapes` only checks absolute paths; `sh -c "…"` passes as trusted | If `-c` or `-e` in `argv[1..]`, return `true` (forfeit). Also if a non-flag relative arg resolves outside trusted prefixes. Full payload parsing stays `TODO (L3)`. |
| E.2 | `ExitKind::HypervisorFault` from UART | `cans/01-architecture.md#Early-boot-diagnostics` | `decode_early_boot_exit` only matches "Kernel panic" | Add an out-of-band `hypervisor_fault: Option<&str>` parameter; when supplied, return `ExitKind::HypervisorFault(...)`. L2 caller passes `None`. |
| E.3 | `FilterAction::Suspend` shape + warning | `cans/07-wire-plane.md#Frame-drop-and-forward-semantics` ("yield frame to host for async policy") | Wire plane returns `Suspend` but the vCPU loop (`TODO L3`) doesn't consume it — silent passthrough | Return `Suspend` unchanged with `// TODO (L3): vCPU loop must act on this` and `tracing::warn!("FilterAction::Suspend returned without consumer")`. Do **not** convert to `Forward`. |
| E.4 | checkpoint watermark | `cans/08-lifecycle.md#Clock-invariants` ("Restored monotonic time clamps to max(current_monotonic, snapshot_monotonic)") | `CheckpointPlane::restore` clamps against `self.monotonic_watermark_ns`, which only `restore` itself bumps — no-op | Add `guest_monotonic_ns: u64` param to `restore`. `Session::rollback` supplies the live guest monotonic time via `VmHandle`. Store `max(current, snapshot.monotonic_offset_ns)`. |
| E.5 | `Session::suspend`/`resume` call driver | `cans/08-lifecycle.md#Hypervisor-primitives` ("All primitives execute via active VmmDriver backend") | `Session::suspend` only advances the state machine; `VmHandle::suspend` calls the driver — divergence | Wire `Session::suspend`/`resume` to `driver.lock().await.suspend/resume(...)`. Until the driver is real (L3), return `Err(PluginError::Halt("driver suspend is TODO (L3)"))` instead of silently advancing state. |
| E.6 | `enable_watchdog` warns when unarmed | `cans/08-lifecycle.md#Watchdog` ("Typically called from on_vm_init by a policy plugin") | `VmConfig.enable_watchdog=true` reaches `LifecycleCtx` but no plugin arms it | In `Session::boot`, if `config.enable_watchdog` is true and no `on_vm_init` callback invokes `ctx.set_watchdog(...)`, `tracing::warn!`. Leave the actual arming to a real `plugin-*` (L3) — do not add a new plugin. |
| E.7 | `network_off` reaches the wire plane | `cans/plugins/replay.md#Invariants-and-policy-(replay)` ("wire plane drops all egress") | `VmConfig.network_off` set by `default_vm_config` but `WirePlane` doesn't consult it; `ReplayPlugin` checks its own private flag | `Session::boot` builds `WireWiring` with `drops_all = config.network_off`. Replay keeps its own check as defense-in-depth. |
| E.8 | `page_size_kb` / `pinned_clocksource` / `preemption_grace_ms` boot contract | `cans/01-architecture.md#Page-size-dynamic-boot-selection`, `cans/08-lifecycle.md#Clock-invariants`, `cans/08-lifecycle.md#Host-crash-reaper` | Set on `VmConfig`, not read by drivers | Emit `tracing::warn!` when non-default and the driver returns `VmmError::Unsupported`; document each as `// TODO (L3): consumed by <driver>`. Coordination with C.4. |

**Accept:** `cargo nextest run -p kelir-core` passes. Manual: `sh -c "…"` forfeits skip; `sh /usr/bin/ls` doesn't; `Session::suspend` returns `Err(Halt(...))` not `Ok(())`; `Sandbox::from_config` with `network_off: true` mounts the wire plane in drop-all mode.

---

## 6. Coordination

### 6.1 File-ownership matrix

| Path | Owner | Notes |
|---|---|---|
| `kelir-interface/src/plugin_model.rs` (MountScope) | **A** | Commit A.4 first; D rebases |
| `kelir-interface/src/plugin_model.rs` (PluginError, bind_*_scoped) | **D** | After A.4 lands |
| `kelir-interface/src/protocol_wire.rs` | **C** | |
| `kelir-interface/src/driver_interface.rs` | **C** | |
| `kelir-interface/src/{telemetry, error_envelope}.rs` | **D** | |
| `kelir-interface/src/bin/codegen/**` | **D** (D.8 only) + **C** (C.1/C.2 projections) | Coordinate via `_collab/handoffs.md` |
| `kelir-core/src/trap_gate.rs` | **E** | A reads, does not edit |
| `kelir-core/src/session_lifecycle.rs` | **A** (A.5 revocation line) + **E** (E.5 suspend/resume, E.6 watchdog warning, E.7 network_off) | Land A.5 first, then E |
| `kelir-core/src/planes/**` | **E** | |
| `kelir-core/src/{dispatch_plugin, plugin_registry}.rs` | **D** | |
| `kelir-core/src/uart_terminal.rs` | **E** | |
| `kelir-shim/src/multiplexer_protocol.rs` | **C** | |
| `kelir/src/session_lifecycle.rs`, `sandbox_runtime.rs`, `builder_sdk.rs` | **B** | |
| `packages/ts/**`, `packages/py/**` | **B** | |
| `kelir-plugins/**` | **A** | |
| `kelir-bindings/**` | **D** (lib.rs) + **C** (storage_bridge.rs) | |
| `kelir-plugin-cli/**` | **D** | |
| `kelir-plugin-testkit/**` | **D** | |

### 6.2 Branch names

`agent-a/plugin-integrity`, `agent-b/sdk-parity`, `agent-c/wire-schema`, `agent-d/structural-hygiene`, `agent-e/runtime-plumbing`.

### 6.3 Merge order

1. **C** (wire/schema shapes). A, B, E consume them.
2. **A** (plugins). Depends on C + `MountScope`.
3. **E** (runtime). Depends on A's `MountScope` + C's `VmConfig`.
4. **B** (SDKs). Depends on all of the above; needs C's type shapes and A/E's behavior.
5. **D** (hygiene). Deletes/merges after functional changes settle.

Every merge runs full `just pre-flight`.

### 6.4 Conflict log

`_collab/conflicts.md` — one line per premise the spec does not actually mandate. Do **not** edit `cans/`.

---

## 7. Definition of Done (per agent)

- [ ] Every issue in the agent's section has a fix **or** a `_collab/conflicts.md` entry.
- [ ] Every commit cites a `cans/…md#Anchor`.
- [ ] `cargo nextest run -p <agent's crates>` passes.
- [ ] `just check-spec` passes.
- [ ] `just check-tiered` passes.
- [ ] `just check-layers` passes.
- [ ] `just check-hygiene` passes.
- [ ] `just parity` passes (A/C/D/E; B additionally regenerates vectors).
- [ ] Diff reviewed against the issue table.

---

## 8. What L2 "done" means (so an agent knows when to stop)

1. **No silent no-ops on security-boundary code.** If the spec says "deny", the code denies or returns a typed error — it does not return `Pass` / `Forward` / `Ok(())` when the mechanism is not wired.
2. **No config field is set without a reader.** A field is either read by a caller, or its doc comment carries a `TODO (L3): consumed by <anchor>` and the boot path warns.
3. **No two SDKs disagree on a verb's return type.** Method spelling may differ per `cans/03-sdk-architecture.md#Idiomatic-Surfaces-Law`; the *shape* must match.
4. **No two impls of the same trait** unless a doc comment names the deliberate reason.
5. **`TODO (L3)` surfaces as `Err(PluginError::Halt("TODO (L3): …"))` or `tracing::warn!`** when reached. Never as `Ok(())`.

L3 — real driver boot, real SPSC framing, real snapshot archive, real interpreter payload parsing, real DNS resolver, real PTY — is explicitly out of scope for all five agents.
