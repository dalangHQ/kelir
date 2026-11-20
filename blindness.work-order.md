Here is the execution blueprint as `work-order.md` to spawn **5 parallel tracks (A–E)** with disjoint file ownership, followed by **1 sync wave (F)** for codegen and gates.

Every track implements **L1/L2 contracts, config/profile schemas, enum/payload shapes, pure state machines, and `TODO (L3) [tag]: ...; slop: ...` stubs only**—zero live OS/hypervisor/I/O implementations.

---

# `work-order.md`

## Global Guardrails (All Agents Must Obey)
1. **Maturity split (`kelir-maturity`)**:
   - `crates/kelir`, `crates/kelir-engine`, `crates/kelir-interface` only allow `L1` (`.rs`) and `L2` (`.flow.rs`). add `.run.rs` with signature & todo marker , `async fn` in L1, or `std::fs`/`std::net`/`std::process`/`libc` calls in `.flow.rs`.
   - L3 stubs live in `*.run.rs` (only inside crates with `allowed_levels = ["L1", "L2", "L3"]`) or behind trait defaults/stubs, and **must** use `// TODO (L3) [domain_or_case_id]: <what>; slop: <failure mode>` and return `Err(...)` fail-closed (never silent `Ok(())` on security/enclave ops).
2. **Law 8 (`layers_cli`)**:
   - `crates/kelir` must never import or name `plugin-*` crates.
   - `crates/kelir-plugins/*` may only depend downward (`Tier 3 -> Tier 2 -> Tier 1`). Never same-tier or upward.
3. **Codegen SSOT (`sdk_codegen` + `model_codegen`)**:
   - **Never hand-edit** `packages/py/kelir/plugins/*.py`, `packages/ts/core/src/plugins.registry.ts`, `generated/*`, or `telemetry.attest.generated.ts`.
   - In `*Config` structs under `crates/kelir-plugins/*/src/lib.rs`, **only** use `model_codegen`-supported types (`u8..u128`, `usize`, `i8..i128`, `isize`, `f32`, `f64`, `bool`, `String`, `PathBuf`, `Option<T>`, `Vec<T>`, tuples, `[u8; N]`, nested structs, unit enums with `Default`). **No `HashMap`, `BTreeMap`, or data-bearing enums** inside exported `*Config` structs.
4. **Slop & Anchor Sync**: Any new `[tag]` in `TODO (L3) [tag]` must be a valid domain or case id in `cans/artifacts/test-plan/l3-slop-register.yaml` (reconciled in Track F).

---

## Track A — Core Protocol, Verdict & Engine Contracts
**Owned files:**
- `crates/kelir-interface/src/{protocol_wire.flow.rs, handlers_plugin.flow.rs, handles_lifecycle.flow.rs}`
- `crates/kelir-interface/src/wire/{dns_ops.flow.rs, net_ops.flow.rs}`
- `crates/kelir-interface/src/storage/storage_reconciler.rs`
- `crates/kelir-engine/src/dispatch_plugin/engine.flow.rs`
- `crates/kelir-engine/src/planes/storage/config.flow.rs`
- `crates/kelir-engine/src/planes/wire/plane.flow.rs`
- `crates/kelir-engine/src/trap_gate.flow.rs`

**Clues:**
- **`protocol_wire.flow.rs`**:
  - Add `PatchPayload::NetRedirect { host, port, guidance }` and `PatchPayload::SignalMutation { new_sig, guidance }`.
  - Add optional `guidance: Option<String>` / `redirect_tool: Option<String>` steering fields where `Drop` / `Abort` / `Patch` surface to L7/EXEC.
  - Extend `SuspendUntil::Approval` (or add metadata struct) with hold slot cap & expiry TTL fields.
  - Document/enforce `ExecveMutation::set_env` key deduplication invariant before `envp` prepend.
- **`dns_ops.flow.rs`**:
  - Add `DnsVerdict::Redirect { target: String }` (or IP answer override) alongside `Forward` / `Sinkhole`.
- **`handlers_plugin.flow.rs` & `handles_lifecycle.flow.rs`**:
  - Expand `SignalContext` with `syscall_nr`, `target_relation` (`Self_`, `Children`, `Session`, `ShimPid1`, `Peer`) for `kill`, `tkill`, `tgkill`, `rt_sigqueueinfo`, `rt_tgsigqueueinfo`, `pidfd_send_signal`.
  - Add `ExecHandle::steer(exec_id, msg)` stub (`TODO (L3) [terminal_plane]: ...`) for EXEC sub-lane `0x02` guidance flush before EOF.
- **`engine.flow.rs`**:
  - Wire `dispatch_signal` to evaluate `SignalContext` / `MountScope` instead of skipping scope filtering; add `Patch(SignalMutation)` / `Absorb` verdict handling.
  - Track per-session pending `SuspendUntil::Approval` hold count & TTL expiration check.
- **`trap_gate.flow.rs`**:
  - **Delete** `compute_shannon_entropy`, `EntropyHook`, `entropy_hook`, `entropy_veto`, and the `supply-chain.md#Entropy-analysis` comment (`blindess.patch.md` killed entropy gating).
- **`net_ops.flow.rs` & `planes/wire/plane.flow.rs`**:
  - Add `dns_to_ip_pinning: bool` to `WireWiring` and a pure L2 TTL-bounded DNS-to-IP pin table (`DnsPinTable`) + `connect_redirect` / `dns_redirect` config slots.
- **`planes/storage/config.flow.rs` & `storage_reconciler.rs`**:
  - Add `blocking_wait_timeout_ms: u64` to `LockBrokerWiring` and `default_permissions: bool` to `StorageConfig`.
  - Add `ReconcileFailurePolicy` / comments clarifying `EBUSY` retries up to 2000ms while `EPERM` aborts fail-closed (never in-place overwrite).

---

## Track B — Host Enclaves, Shim & Drivers (L3 Stubs Only)
**Owned files:**
- `crates/kelir-shim/src/{trap_protocol.run.rs, multiplexer_protocol.run.rs}`
- `crates/kelir-storage/src/{lock_broker.run.rs, reconciler.run.rs, fuse.run.rs}`
- `crates/kelir-wire/src/{smoltcp.run.rs, dns.run.rs}`
- `crates/kelir-drivers/driver-{bwrap,proot,hvf}/src/*.flow.rs`

**Clues:**
- **`kelir-shim/src/trap_protocol.run.rs`**:
  - Expand `DefenseFlags` with `block_io_uring`, `block_keyrings`, `blocked_socket_families`, `deny_abi_compat` (`AUDIT_ARCH` check rejecting x32 / `int 0x80`).
  - Add `TODO (L3) [protocol_mux]` items inside `trap_filter_program` for `io_uring_setup/enter/register`, `add_key/request_key/keyctl`, and `AF_ALG/AF_RXRPC/NETLINK_XFRM`.
- **`kelir-shim/src/multiplexer_protocol.run.rs`**:
  - Add `per_exec_cgroup: bool` + `steer_exec(exec_id, msg)` stub (flushes sub-lane `0x02` before exit EOF) + `TODO (L3) [protocol_mux]` for per-`ExecSlot` cgroup v2 (`pids.max`, `memory.max`) instead of shared-UID `RLIMIT_NPROC`.
- **`kelir-storage/src/{lock_broker.run.rs, reconciler.run.rs, fuse.run.rs}`**:
  - `lock_broker.run.rs`: split `SETLK` (`EWOULDBLOCK` on conflict) vs `SETLKW` (bounded waiter pool + `EDEADLK` timeout) in `TODO (L3) [sp_19_host_inode_flock_contention_ewouldblock]`.
  - `reconciler.run.rs`: ensure `TODO (L3)` explicitly forbids `EPERM` in-place write fallback; add `TODO (L3) [storage_plane]` for `symlink_cwd_deny`, `/tmp`↔`/workspace` cross-mount `EXDEV`, and APFS/NTFS case-collision ambiguity.
  - `fuse.run.rs`: add `TODO (L3) [storage_plane]` for `default_permissions` mount flag (avert guest `clone` deadlock).
- **`kelir-wire/src/{smoltcp.run.rs, dns.run.rs}`**:
  - Fix stale comment in `smoltcp.run.rs` (`// TODO (L3) [wire_plane]: resolve connecting guest PID via TrapInterposer socket-tuple correlation...` — remove netlink assumption).
  - Add `TODO (L3) [wire_plane]` stubs for unconnected UDP `sendto`/`sendmmsg` per-target policy, `DnsPinTable` lookup on L4 ingress, `connect_redirect`, and `DnsVerdict::Redirect` answer rewriting in `dns.run.rs`.
- **`driver-bwrap`, `driver-proot`, `driver-hvf`**:
  - `bwrap_driver.flow.rs` & `proot_driver.flow.rs`: add `TODO (L3) [hypervisor_driver]` pre-boot behavioral probe (`seccomp_install_probe`, `wait_killable_probe`) and identical BPF syscall/socket-family inheritance.
  - `hvf_driver.flow.rs`: ensure Stage-2 CoW fork L3-GAP explicitly states in-process `hv_vm_protect` without OS `fork()` on multi-threaded hosts.

---

## Track C — Tier 1 & Tier 2 Plugins (Schemas + Pure L2 Engines + L3 Stubs)
**Owned files:** `crates/kelir-plugins/plugin-{mcp,tool-guard,supply-chain,vault,l7-http,egress-proxy,redaction,db-proxy}/src/**`

**Clues:**
- **`plugin-mcp`**:
  - Add `deny_server_sampling: bool` (default `true`) and `sampling_allowlist: Vec<String>` to `McpConfig`.
  - Reject `sampling/createMessage` in `ast.flow.rs` / `mcp_service.flow.rs` unless explicitly allowlisted.
- **`plugin-tool-guard`**:
  - Add to `ToolGuardConfig`: `detect_cross_server_exfil: bool`, `detect_confusable_names: bool`, `scan_tool_results: bool`, `approval_auth: ApprovalAuthConfig` (`deny_self_approval`, `methods: Vec<String>`, `max_pending_holds: u32`, `hold_ttl_seconds: u64`).
  - Implement pure L2 logic in `service_tool_guard.flow.rs`:
    - Capability classifier (`Read`, `Write`, `Send`, `Run`) + sliding-window `read_then_send` cross-server detector.
    - Levenshtein distance check on `tools/list` ingestion (`detect_confusable_names`).
    - `ApprovalAuth` role separation / self-approval denial / hold cap & TTL validator (leave WebAuthn/TOTP/OIDC crypto verify as `TODO (L3) [governance_tools]`).
    - JSON-RPC `-32001` error data shape `{ errno, reason, guidance, redirect_tool }` + `TODO (L3) [governance_tools]` for hidden-instruction / prompt-injection result scanning.
- **`plugin-supply-chain`**:
  - **Delete** `shannon_entropy_threshold_bits` from `PackageGuardConfig` and **delete** `flags_obfuscated_argv` / `sc_02_argv_high_entropy_obfuscation_kill` from `state.run.rs`.
  - Add to `PackageGuardConfig`: `resolve_lockfiles: bool`, `advisories: Vec<String>`, `strip_private_scopes: bool`, `vet_skills: bool`.
  - In `interpreter.flow.rs`: cap recursion at depth `8` (fail closed), handle opaque shell AST fallback to child `execve` traps, add pure `strip_private_scope(pkg)` helper + BusyBox applet `argv[0]` attestation check stub.
  - Add `TODO (L3) [supply_chain_security]` in `state.run.rs` for dry-run lockfile resolver (`npm/pip/pnpm/yarn/uv/poetry`), OSV/Snyk/Socket/deps.dev lookups, and `SKILL.md` unicode/injection vetting.
- **`plugin-vault`**:
  - Add `credsub_fake_tokens: bool` and `scrub_responses: bool` to `VaultConfig`.
  - Implement pure L2 `CredSubTable` (format/length-preserving fake token registry + outbound swap + inbound response scrubber) in `service_vault.flow.rs`; wire `TODO (L3) [governance_vault]` in `metadata_endpoint.run.rs` for live stream binding.
- **`plugin-l7-http`**:
  - Add `DeclaredServiceConfig` (`name`, `env_var`, `upstream`, `block_direct_egress`, `llm_dialect`, `dlp_redact_prompts`, `inspect_sse_tool_use`), `threat_feeds: Vec<String>`, `hash_only_llm_retention: bool` to `L7HttpConfig`.
  - Add `guidance: Option<String>` to `synthetic_403_receipt`, pure `..` path-traversal rejection (`403` before dial), pure OpenAI/Anthropic token-usage normalizer + SSE `tool_use` chunk inspector in `l7_parser.flow.rs`.
  - Add `TODO (L3) [wire_plane]` in `service_l7_http.run.rs` for L4 direct-egress block binding, DLP prompt redaction, and URLhaus/phishing/Onionoo feed sync.
- **`plugin-egress-proxy`**:
  - Add `block_tor: bool` and `deny_socks5_resolve: bool` to `EgressProxyConfig`.
  - Normalize both lowercase & uppercase `http_proxy`/`HTTP_PROXY` etc. in `from_env()`; reject `.onion` targets and SOCKS5 `0xF0` (`RESOLVE`) in `service_egress_proxy.flow.rs`; add `TODO (L3) [wire_plane]` for `obfs4proxy`/`snowflake-client` trap blocks and `NO_PROXY` L4 spoof guard.
- **`plugin-redaction`**:
  - Add `SoftDeleteConfig { enabled: bool, trash_dir: String }` to `RedactionConfig`.
  - Add L2 quarantine manifest token generator in `service_redaction.flow.rs` and `on_fs_unlink` L3 stub (`PatchPayload::FsPathRedirect`) in `service_redaction.run.rs`.
- **`plugin-db-proxy`**:
  - Expand `PgWireRewrite` with `ast_parser: String` (`"libpgquery"`), `require_where_on_mutations: bool`, `deny_mode_in_tx: String` (`"terminate"`), `block_unsafe_io: bool`, `catalog_resolution: bool`, `synthetic_cancel_keys: bool`.
  - Add pure L2 `PgExtendedQueryState` (`Parse`/`Bind`/`Execute`/`Sync`), `CancelKeyMap`, `PreparedCache` (invalidates on `search_path`/role change), collect-all-any-deny-wins SQL policy evaluator, Redis dangerous-command gate (`EVAL`/`CONFIG`/`DEBUG`/`FLUSH*`), and pgwire `ErrorResponse` `HINT ('H')` builder; leave `libpgquery` C-FFI/live framing as `TODO (L3) [relational_transaction]`.

---

## Track D — Tier 3 Plugins, Facade Profile & Bundle Schema
**Owned files:**
- `crates/kelir-plugins/plugin-{ledger,step-eval,anomaly}/src/**`
- `crates/kelir/src/{profile_schema.flow.rs, profile_validate.flow.rs, planes/wire_plane.rs}`
- `crates/kelir-bundle/src/plugin_profile_schema.flow.rs`

**Clues:**
- **`plugin-ledger`**:
  - Add `strict: bool`, `ocsf_mapping: bool` to `LedgerConfig`; add `kms_provider: Option<String>`, `min_signing_key_bytes: u32`, `verify_key_fingerprint: bool` to `MerkleWalConfig`.
  - Add L2 `AuditDropCounters` (`overflow`, `crc_corruption`, `mapper`, `auth_loss`), `OcsfClassId` mapper (exhaustive enum match over event kinds), and `CRC32C` WAL segment header packer in `wal_ledger.flow.rs`.
  - Add `TODO (L3) [telemetry_audit]` in `wal.run.rs` / `ledger_plugin.run.rs` for `key_fingerprint` rotation check, KMS >= 32-byte key fetch, persisted ACK high-watermark, and `ledger.strict` backpressure halt.
- **`plugin-step-eval`**:
  - Add `TODO (L3) [lifecycle_checkpoint]` / hook in `step_dag.flow.rs` & `service_step_eval.flow.rs` aligning reviewer attribution with `ApprovalAuth` (role separation, non-empty reviewer/TTL).
- **`plugin-anomaly`**:
  - Expand `TelemetrySample` / `AnomalyDetectionService` with `raw_ip_without_dns`, `denial_burst`, and `privilege_drift` signals in `service_anomaly.flow.rs`.
- **`crates/kelir/src/profile_schema.flow.rs` & `profile_validate.flow.rs`**:
  - Add to `ShimProfile`: `per_exec_cgroup`, `deny_abi_compat`, `blocked_syscalls`, `blocked_socket_families`.
  - Add `SteeringProfile` (`steer_on_deny`, `steer_on_redirect`, `inject_exec_stderr`) on `SessionProfile`.
  - Add `blocking_wait_timeout_ms` on `CowBrokerProfile` / lock broker, and `dns_to_ip_pinning` on `WireProfile`.
  - Enforce monotonic overlay rules in `validate_overlay` (never allow child overlay to disable `per_exec_cgroup`, `deny_abi_compat`, `dns_to_ip_pinning`, or remove entries from `blocked_syscalls` / `blocked_socket_families`).
- **`crates/kelir-bundle/src/plugin_profile_schema.flow.rs`**:
  - Add `*PluginProfile` structs, `validate_plugins`, `validate_plugins_overlay`, and `to_*_config()` converters for `db_proxy`, `supply_chain`, `vault`, `l7_http`, `egress_proxy`, `redaction`, plus the new fields on `ledger`, `tool_guard`, `mcp`. Keep `crates/kelir` untouched by plugin types (Law 8).

---

## Track E — CLI, Hand-Written SDKs & NAPI/PyO3 Bridges
**Owned files:**
- `crates/kelir-plugin-cli/src/{doctor_cli.flow.rs, spec_cli.flow.rs, parity_cli.flow.rs, dispatch.flow.rs, main.rs}` + new `{report_cli.flow.rs, emergency_cli.flow.rs}`
- `crates/kelir-bindings/{napi,pyo3}/src/{builder.flow.rs, handles.flow.rs, storage_bridge.rs}`
- `packages/py/kelir/{__init__.py, profile_config.py, kelir_builder.py, lifecycle_session.py}`
- `packages/ts/core/src/{verdict.protocol.ts, types.core.ts, context.lifecycle.types.ts, profile.schema.ts, session.lifecycle.ts, session.lifecycle.types.ts, hook.registry.ts}`

*(Skip `packages/py/kelir/plugins/*.py` and `packages/ts/core/src/plugins.registry.ts`—Track F regenerates them.)*

**Clues:**
- **`kelir-plugin-cli`**:
  - `doctor_cli.flow.rs`: add `HostProbe` variants (`SeccompInstall`, `WaitKillable`, `Userfaultfd`) returning `ProbeOutcome::NotImplemented("... (L3)")` with `L3-GAP` / `TODO (L3) [hypervisor_driver]`.
  - `spec_cli.flow.rs`: add `spec generate <session>` arg parsing + `TODO (L3) [plugin_engine]`.
  - Add `report_cli.flow.rs` (`kelir report <session>`) and `emergency_cli.flow.rs` (`kelir emergency <kill-switch|break-glass>`), wire in `dispatch.flow.rs` + `main.rs`.
  - `parity_cli.flow.rs`: add `--ocsf-coverage` and `--adversarial-cve-coverage` sub-checks (L2 structural assertion + `TODO (L3)` for live CVE replay).
- **Hand-written Python & TypeScript SDKs + Bindings**:
  - Mirror Track A's new `PatchPayload` (`NetRedirect`, `SignalMutation`), `DnsVerdict.Redirect`, `ExecHandle.steer`, and steering/seccomp fields in `profile_config.py` and `profile.schema.ts`.
  - Forward `blocking_wait_timeout_ms` / `dns_to_ip_pinning` / `per_exec_cgroup` / `deny_abi_compat` in `napi`/`pyo3` `builder.flow.rs` & `storage_bridge.rs` (or keep explicit `L3-GAP` where native bridge doesn't consume yet).

---

## Track F — Sync Wave (Run Sequentially After Tracks A–E Merge)
**Owned files:** Generated artifacts, test-plan metadata, and CI gates.

**Execution sequence:**
1. **Regenerate SDKs & Wire/Telemetry tables**:
   - Run `cargo run -p kelir-interface --bin codegen`.
   - Fix any `model_codegen` projection errors in Track C `*Config` structs if non-supported types slipped in.
2. **Reconcile Test-Plan & Slop Registry**:
   - Update `cans/artifacts/test-plan/l3-slop-register.yaml` and `case-to-test-map.yaml` to match removed `sc_02_argv_high_entropy_obfuscation_kill` and any new `TODO (L3) [tag]` entries.
   - Regenerate `crates/kelir-testkit/anchors.json` via `cargo run -p kelir-testkit --bin anchors`.
3. **Run All Verification Gates**:
   - `cargo run -p kelir-maturity -- check` (L1/L2/L3 placement & `os-reach` purity)
   - `cargo run -p kelir-plugin-cli -- check --layers` and `--graph` (Law 8 + downward-only tiers)
   - `cargo run -p kelir-testkit --bin check-manifest` and `check-tests`
   - `cargo check --workspace` / `mypy` / `tsc -p tsconfig.check.json`
   - *(Optional L3 milestone, out-of-scope for stub tracks)*: first end-to-end KVM/HVF run of `autonomous-swe-agent` + `db-backed-runtime`.