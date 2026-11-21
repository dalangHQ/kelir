The current `kelir-maturity` checker is security theater. It fails because it relies on naive substring matching, allows massive loopholes in stub detection, and never checks for downward L3 leaks into L1/L2.

### Why the Current Gating Fails

1. **Zero Enforcement Against L3 Leaking Downward (`L1`/`L2`)**
   * `tree.flow.rs` only inspects `TODO` inside `Level::L3` files. Nothing prevents dumping `// TODO (L3)` or `Err("... (L3)")` into `.rs` (L1) or `.flow.rs` (L2).
   * `allowed_levels = ["L1", "L2"]` in `Cargo.toml` only validates filename suffixes, not code contents—so `kelir-engine`, `kelir-interface`, and `kelir` freely harbor L3 stubs.
2. **L1 Checks Only Async/Spawn, Not Logic (`S1`)**
   * `structure.flow.rs` checks L1 for `async fn`, `.await`, `tokio::`, and `std::thread::spawn`. Synchronous L1 files can hold full business logic, error-returning stubs, and helper functions (`profile_validate.rs`, `session_paths.rs`, `assertions_testkit.rs`) without triggering a single warning.
3. **Two Fatal Loopholes in L3 Stub Enforcement (`S6`)**
   * **Comment bypass:** `if body.contains("// TODO (L3)") { return true; }` immediately approves the entire function. A 200-line implementation with one `// TODO (L3)` comment passes.
   * **Tail-expression bypass:** `is_single_expression` passes any function lacking `;` and control-flow keywords. In Rust, tail expressions omit `;`, so active delegation (`self.intercept_ethernet_frame(...)` in `service_db_proxy.run.rs`), struct builders (`bpf_stmt`), and constructors (`SeccompNotify::install`) pass without a `TODO (L3)` marker.
4. **Brittle String-Matching & Hardcoded Exemptions**
   * `runtime_spawn_present` looks for literal `"std::thread::spawn"`, missing `std::thread::Builder::new().spawn(...)` (`mesh_state.flow.rs`, `worker_pool.flow.rs`).
   * `tree.flow.rs` and `check-packages-maturity.py` maintain hardcoded lists of files, functions, and crates (`TOOLING_CRATES`, `is_named_profile_read`, `CONTRACT_TS`, `PINNED_MIXED`, `PY_IO_EXEMPT`).

---

### How to Prevent It Without Hardcoding

Replace regex/substring heuristics and hardcoded file lists with **AST-based structural invariants** (`syn` for Rust, `ast` for Python, TS Compiler API for TypeScript) keyed strictly off file suffix and crate metadata.

#### 1. Make File Suffixes + Manifest Metadata the Single Source of Truth
* **Rule:** Suffix = Level everywhere across Rust, TS, and Py:
  * Unmarked (`*.rs`, `*.ts`, `*.py`): **L1**
  * `*.flow.*`: **L2**
  * `*.run.*`: **L3**
* Delete `CONTRACT_TS`, `CONTRACT_PY`, `PINNED_MIXED`, `TOOLING_CRATES`, and `is_named_profile_read`.
* Any host-side or tooling exemption belongs in the package's `Cargo.toml` / `package.json` metadata (e.g., `role = "tooling"`), or an explicit lint directive—never hardcoded paths in the checker.

#### 2. Enforce Strict AST Containment Per Level
* **L1 (`*.rs`):** Parse with `syn`. Allow **only**:
  * `Item::Struct`, `Item::Enum`, `Item::Type`, `Item::Const`, `Item::Use`, `Item::Mod`
  * `Item::Trait` where every `TraitItem::Fn` has `default: None` (no default bodies)
  * `Item::Impl` only for pure marker/conversion/display traits or single-field getters (or ban all `Item::Fn` and `ImplItem::Fn` bodies outright per `AGENTS.md`).
  * **Automatic kill:** Any free function or method body in an `.rs` file fails immediately (`profile_validate.rs`, `session_paths.rs`, `compute_ledger.rs`, `naming_codegen.rs`, `assertions_testkit.rs`).
* **L2 (`*.flow.rs`):**
  * **Zero L3 markers anywhere:** Scan AST comments *and* string literals (`Lit::Str`). Any occurrence of `TODO (L3)`, `L3-GAP`, or `(L3)` in an L1 or L2 file is a hard fail. If work is L3-deferred, its stub belongs in `.run.*`, and `.flow.*` dispatches to it.
  * **AST Call Resolution for IO/Spawn:** Instead of matching `"std::thread::spawn"`, walk `ExprCall` / `ExprMethodCall` / `ExprPath` for `thread::Builder`, `thread::spawn`, `tokio::runtime`, `std::fs::*`, `std::process::*`, `std::net::*` (excluding pure value types), and `libc::*` calls.
* **L3 (`*.run.rs`):**
  * **Kill `is_single_expression` and `body.contains(TODO)`:** Every function in `.run.rs` (when gating L1/L2 phases) must satisfy an exact AST shape:
    1. Carries a `// TODO (L3)` comment (unless it is a pure trait-default empty impl or a field-only `Self { ... }` struct literal in `new`/`build_*` with zero calls).
    2. Its `Block` contains **zero** `Expr::Call`, `Expr::MethodCall` (on `self` or external modules), `Expr::If`, `Expr::Match`, `Expr::Loop`, `Expr::ForLoop`, `Expr::While`, or `Expr::Unsafe`.
    3. Allowed statements: only `let _ = ...;` silencers, followed by a single terminal expression: `()`, a literal, `Default::default()`, `Vec::new()`, or `Ok(trivial)` / `Err(ErrorCtor(Lit::Str))`.
  * **Automatic kill:** Catches `service_db_proxy.run.rs` (calls `self.intercept_ethernet_frame`), `bpf_stmt`/`bpf_jump`, `SeccompNotify::install`, and any function hiding meat behind a `// TODO (L3)` comment.

#### 3. Enforce Bidirectional Seam Integrity (`C1` Fixed)
* **If `allowed_levels` omits `"L3"`:** Zero `.run.*` files AND zero `(L3)` / `TODO (L3)` tokens in comments or error strings across the entire crate. Fixes `kelir-engine`, `kelir-interface`, and `kelir`.
* **If `allowed_levels` includes `"L3"`:**
  * Require at least one `.run.rs` file if the crate defines any L3-capable trait impls or async/hardware backends—or fail whenever a crate with `"L3"` in `allowed_levels` has `.flow.rs` files with zero `.run.rs` counterparts. Immediately catches all 6 `kelir-drivers` crates and `plugin-egress-proxy` / `plugin-terminal-guard`.
  * Disallow `#[allow(dead_code)]` on functions in `.flow.rs` that shadow stubbed `.run.rs` methods (`kelir-host`, `kelir-checkpoint`, `kelir-terminal`).

#### 4. Apply the Same AST Rules to TS and Python
* **Python:** `check-packages-maturity.py` already uses `ast`. Enforce:
  * Unmarked `.py` (L1) = `ClassDef` (shapes/dataclasses/Enums/Exceptions only) + type aliases; no `FunctionDef` with bodies.
  * `.flow.py` (L2) = no `TODO (L3)` / `(L3)` in comments or strings; no IO AST nodes (`Call` to `open`, `subprocess`, `socket`, `os.system`, etc.).
  * `.run.py` (L3) = every `FunctionDef` must have `# TODO (L3)` and a single `return <trivial>` or `raise <Err>("...")` AST body.
* **TypeScript:** Replace regex line-scanning in `check-packages-maturity.py` with a 50-line script using the `typescript` compiler API (`ts.createSourceFile`) already in `node_modules`, enforcing the exact same L1/L2/L3 AST node rules (`step-eval.protocol.ts` and `session.lifecycle.ts` fail immediately).


latest manual finding >>> 


### 1. L3 Files Containing "Meat" / Implementation Logic (S6 Violations)

* **`crates/kelir-plugins/plugin-db-proxy/src/service_db_proxy.run.rs`**
  * `on_net_outbound` and `on_net_inbound` contain working delegation logic (`self.intercept_ethernet_frame(...)`).
  * No `// TODO (L3)` marker, contains active statements. This is wiring/execution logic sitting in an L3 file.
* **`crates/kelir-shim/src/trap_protocol.run.rs`**
  * `bpf_stmt` and `bpf_jump`: concrete helper functions constructing `libc::sock_filter` structs without `// TODO (L3)`.
  * `SeccompNotify::install`: functional constructor `Ok(Self { fd: filter.fd() })` without `// TODO (L3)`.

---

### 2. L3 Concerns Leaking into L1 Contract Files (No `.run.rs` Extension)

L1 files must hold only contracts, types, and empty signatures. These L1 files contain explicit L3 stubs and error returns:

* **`crates/kelir/src/profile_validate.rs`**
  * `to_full_vm_config`: concrete function body returning `VmConfig::default()`.
  * `route_fast_boot_and_dax_window`: returns `Err(PluginError::Halt("... (L3)"))`.
* **`crates/kelir-engine/src/session_paths.rs`**
  * `sweep_orphaned_session_dirs`, `register_incarnation_proof_row`, and `upgrade_detached_lease_row` all return `Err(PluginError::Halt("... (L3)"))`.
* **`crates/kelir-engine/src/dispatch_plugin/params_hydration.rs`**
  * `hydrate_params_ctx` contains a `// TODO (L3)` stub returning `None`.
* **`crates/kelir-plugins/plugin-ledger/src/compute_ledger.rs`**
  * `compute_causal_delta` returns `Err(PluginError::Halt("TODO (L3) ..."))`.
* **`crates/kelir-plugin-testkit/src/assertions_testkit.rs`**
  * `assert_pass`, `assert_deny`, `assert_patch`, `assert_suspend`, `assert_abort`, `assert_rewrite`, etc. all return `Err(PluginError::Halt("TODO (L3) ..."))`.
* **`crates/kelir-testkit/src/perf_budgets.rs`**
  * `validate_scorecard_budgets` contains a `// TODO (L3)` returning `Err(...)`.
* **`crates/kelir-interface/src/bin/codegen/naming_codegen.rs`**
  * `split_camel`, `screaming`, `kebab`, `camel`, `snake` all contain `// TODO (L3)` returning empty values.
* **`crates/kelir-plugin-cli/src/manifest_cli.rs`, `logs_cli.rs`, `registry_cli.rs`**
  * All three are bare `.rs` files implementing `run()` returning `Err("... (L3) ...")`.

---

### 3. L3 Concerns Leaking into L2 (`.flow.rs`) Files

L2 files must hold execution flow and pipeline dispatch. These files hold L3 stubs, L3 error returns, or dead-code L3 implementations:

* **`crates/kelir-engine/src/vmm/router.flow.rs`**
  * `DriverRouter::load_native_driver` returns `Err(VmmError::Unsupported("TODO (L3) [hd_01_nohardware_vmm_fatal_abort] ..."))`.
* **`crates/kelir-engine/src/session_lifecycle.flow.rs`**
  * `DetachedSpawnTransport::dispatch_exec` returns `Err(PluginError::Halt("TODO (L3): guest exec dispatch"))`.
  * `Session::attach`, `Session::memory_handoff`, `Session::export_snapshot` all return `TODO (L3)` halts.
* **`crates/kelir-engine/src/planes/storage/plane.flow.rs`**
  * `StoragePlane::discard` returns `Err(PluginError::Halt("TODO (L3) [sp_01_host_cow_upper_discard_on_rollback] ..."))`.
* **`crates/kelir-engine/src/planes/checkpoint/plane.flow.rs`**
  * `compact_delta_blocks` contains `TODO (L3) [lc_20_portable_snapshot_archive_export]`.
* **`crates/kelir-engine/src/planes/host/plane.flow.rs`**
  * `cgroup_pressure_events` contains `TODO (L3) [hd_16_virtio_balloon_pressure_reclaim]`.
* **`crates/kelir-interface/src/policy_plane.flow.rs`**
  * `OperatorLease::verify_signature` returns `Err(PolicyPlaneError::SignatureVerificationPending("... (L3)"))`.
* **`crates/kelir-interface/src/wire/dns_ops.flow.rs`**
  * `DnsQuery::parse` returns `Err(PluginError::Halt("DNS query parse pending (L3)"))`.
* **`crates/kelir-transport/src/fallback_transport.flow.rs`**
  * `VsockFallbackTransport` and `NamedPipeFallbackTransport` methods return `Err(crate::unsupported("... pending (L3)"))`.
* **`crates/kelir-checkpoint/src/compaction.flow.rs`**
  * Implements `compact` and `rollback` with `#[allow(dead_code)]` and L3-GAPs; dead-code because `real_checkpoint.run.rs` stubs them out.
* **`crates/kelir-host/src/balloon.flow.rs`, `pressure.flow.rs`, `reaper.flow.rs`**
  * Contain `reclaim`, `subscribe_pressure`, `poll_pressure_events`, `arm_host_reaper` returning `TODO (L3)` halts with `#[allow(dead_code)]`, duplicating stubs in `real_host.run.rs`.
* **`crates/kelir-terminal/src/log_segment.flow.rs` & `vt100.flow.rs`**
  * `Vt100StateMachine::feed` and `filter_ansi_shadow` contain L3-GAPs and dead code suppression in `.flow.rs`.
* **`crates/kelir-plugins/plugin-ledger/src/otlp_encoder.flow.rs`**
  * `encode_receipt`, `encode_phase_timings`, `encode_span`, `encode_causal_delta`, `encode_metrics` all return `Err(OtlpError::NotImplemented("... (L3)"))`, while `otlp_encoder.run.rs` contains zero code.
* **`crates/kelir-plugins/plugin-ledger/src/ledger_plugin.flow.rs`**
  * `blake3_of_producing_process`, `finalize` (signing), and `get_causal_delta_from_parent` contain `TODO (L3)` halts.
* **`crates/kelir-plugins/plugin-step-eval/src/step_dag.flow.rs`**
  * `StepAttribution::verify` returns `Err(StepDagError::SignatureVerificationPending("... (L3)"))`.
* **`crates/kelir-plugins/plugin-supply-chain/src/interpreter.flow.rs`**
  * `attest_busybox_applet` and `resolve_shebang` return `TODO (L3)` halts.
* **`crates/kelir-plugins/plugin-mcp/src/schema.flow.rs` & `mcp_service.flow.rs`**
  * `WithParamsPredicate::matches` returns `TODO (L3)` halt.
  * `McpEndpointService::route_request` returns `Err(PluginError::Halt("MCP_CONSOLE transport routing pending (L3)..."))`.
* **`crates/kelir-plugins/plugin-egress-proxy/src/service_egress_proxy.flow.rs`**
  * `guard_no_proxy` and `dial` return `TODO (L3)` errors.
* **`crates/kelir-plugins/plugin-db-proxy/src/service_db_proxy.flow.rs`**
  * `ast_parse_sql` returns `Err(PluginError::Halt("TODO (L3): libpgquery AST parse pending (L3)"))`.
* **`crates/kelir-plugins/plugin-vault/src/metadata_endpoint.flow.rs`**
  * `register_boot_diversion` returns `Err(PluginError::Halt("vault IMDS boot diversion install is TODO (L3)"))`.
* **`crates/kelir-adapters/kelir-pack/src/pack.flow.rs`**
  * `fetch_manifest`, `phase1_scan_headers`, and `phase2_emit_erofs` return L3 pending errors, while `erofs.run.rs` and `oci.run.rs` contain only comments.
* **`crates/kelir-bindings/napi/src/handles.flow.rs` & `crates/kelir-bindings/pyo3/src/handles.flow.rs`**
  * Numerous L3 stubs (`map_shared_region`, `spawn`, `exec_in_guest`, `steer`, `rollback`, `console`, `get_causal_delta_from_parent`, `handle_ingress`) live in `.flow.rs`, while `handles.run.rs` in both crates are empty comment shells.
* **`crates/kelir/src/session_lifecycle.flow.rs` & `builder_sdk.flow.rs`**
  * `Session::handle_ingress`, `Session::shm`, `VsockHandle::channel`, `SpawnBuilder::poll`, and `SdkStreamServiceSink::on_stream_frame` all contain `TODO (L3)` halts.
* **`crates/kelir-plugin-cli/src/*.flow.rs`**
  * `attach_cli.flow.rs`, `emergency_cli.flow.rs`, `report_cli.flow.rs`, `rings_cli.flow.rs`, `attest_cli.flow.rs`, `spec_cli.flow.rs`, `setup_cli.flow.rs` all return `TODO (L3)` errors in `.flow.rs`.

---

### 4. Crates Violating Declared `allowed_levels`

The following crates declare `allowed_levels = ["L1", "L2"]` in `Cargo.toml`, but harbor L3 stubs and deferrals:

* **`kelir-engine`**:
  * Manifest pins `allowed_levels = ["L1", "L2"]`.
  * Leaks L3 into: `session_paths.rs`, `params_hydration.rs`, `vmm/router.flow.rs`, `session_lifecycle.flow.rs`, `planes/storage/plane.flow.rs`, `planes/checkpoint/plane.flow.rs`, `planes/host/plane.flow.rs`.
* **`kelir-interface`**:
  * Manifest pins `allowed_levels = ["L1", "L2"]`.
  * Leaks L3 into: `policy_plane.flow.rs`, `wire/dns_ops.flow.rs`, `bin/codegen/naming_codegen.rs`.
* **`kelir`**:
  * Manifest pins `allowed_levels = ["L1", "L2"]`.
  * Leaks L3 into: `profile_validate.rs`, `session_lifecycle.flow.rs`, `builder_sdk.flow.rs`.

---

### 5. Driver Crates Completely Missing the L2/L3 Seam

All 6 hypervisor driver crates declare `allowed_levels = ["L1", "L2", "L3"]`, but contain **zero `.run.rs` files**:
* `driver-avf` (`avf_driver.flow.rs`)
* `driver-bwrap` (`bwrap_driver.flow.rs`)
* `driver-hvf` (`hvf_driver.flow.rs`)
* `driver-openvmm` (`openvmm_driver.flow.rs`)
* `driver-proot` (`proot_driver.flow.rs`)
* `driver-rustvmm` (`rustvmm_driver.flow.rs`)

All hypervisor primitives (`open_node`, `create_vm`, `create_partition`, `init_backend`, `spawn_namespace`, `init_tracer`) and `VmmDriver` implementations are written directly inside `.flow.rs` with `Err(VmmError::Unsupported("... (L3)"))`.

---

### 6. TypeScript and Python SDK Layering Violations

* **`packages/ts/core/src/step-eval.protocol.ts`**
  * Non-run protocol file containing an L3 stub: `throw new Error("TODO: runStepEvaluationLoop (L3)")`.
* **`packages/ts/core/src/session.lifecycle.ts`**
  * L2 lifecycle file containing `throw new EConfig("TODO: Session.attach (L3)")` and `(L3)` fallback planes.
* **`packages/py/kelir/lifecycle_session.py`**
  * L2 lifecycle file containing multiple L3 methods (`rollback`, `spawn`, `shm`, `handle_ingress`, `exec_in_guest`, `console`, `attach`) raising `EConfig("... (L3)")`.
* **`packages/py/kelir/service_caps.py`**
  * Capability contract file containing L3 stubs (`LedgerCap`, `TransactionScope`, `ContainerCap`, `MeshHandle`) raising `_missing("... (L3)")`.

