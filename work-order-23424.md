# WORK ORDER: Scope 23424 — Plumbing Defect Remediation & Scaffolding Stabilization

**Document:** `work-order-23424.md`  
**Status:** EXECUTED 2026-11-10 (squash-merged into scaffolding)  
**Target Phase:** L1/L2 Scaffolding Hardening (Zero L3 Feature Creep)  
**Normative SSOT:** `cans/` specification corpus + `cans/artifacts/`  

---

## 0. Rules of Engagement

1. **Plumbing over Meat:** Do not implement full L3 engines (no full VT100 state machine, no OTLP protobuf network clients, no in-kernel L4 diversion). Fix the *plumbing*: contracts, wire formats, paths, error envelopes, and FFI perimeters.
2. **No Lying Stubs:** An L3 stub must either return an explicit non-fatal fallback (e.g. `Ok(())` for optional scaffolding) or fail gracefully without taking down the entire microkernel console pipeline. Stubs that halt production flows on standard paths are bugs.
3. **DRY & SSOT (`cans/01-architecture.md`):** `kelir-interface` is the sole source of truth for shared protocol constants, layouts, and codecs. Duplicate definitions in leaf crates or tests must be deleted.
4. **Zero Silent Drops:** If a builder or binding method accepts configuration (`idmap`, `encapsulation`, `mask_read`), it must either be plumbed to the plane or rejected at boot with `EConfig`. Silent discard is forbidden.

---

## Execution Matrix Overview

| Phase | Domain | Primary Deliverable |
|---|---|---|
| **Phase 1** | Wire & Protocol | Unify frame codecs, fix `ExecveMutation` CBOR serialization, enforce ring capacity. |
| **Phase 2** | Kernel & Planes | Fix console pipeline crash, unify session storage root, throttle UART scraping. |
| **Phase 3** | Boundary & FFI | Enforce FFI rate-limits on all 10 rings, fix Error Envelope amnesia, wire trampolines. |
| **Phase 4** | SDK & Facade | Plumb `StorageConfig`, replace TS/Py profile crawlers with TypeBox/Pydantic, unify `PidLineage`. |
| **Phase 5** | Plugins & Telemetry | Stop `ledger.flush()` from halting, plumb `c.step.last()`, formalize scaffolding stubs. |
| **Phase 6** | Tooling & Codegen | Purge codegen patches, fix scorecard generation target, align AS-01 linter targets. |

---

## Phase 1: Wire & Protocol Plumbing

### 1.1 Unify 9-Byte Frame Codec into `kelir-interface`
* **Defect:** `crates/kelir-core/src/bus/mod.rs`, `crates/kelir-shim/src/codec_protocol.rs`, and `crates/kelir-plugin-testkit/src/vectors_testkit.rs` independently re-implement the 9-byte `[stream_id: u32, flags: u8, len: u32]` wire frame.
* **Fix:**
  1. Move canonical `encode_frame`, `decode_header`, and `WireFrame` codec into `crates/kelir-interface/src/codec_protocol.rs`.
  2. Refactor `kelir-core::bus`, `kelir-shim`, and `kelir-plugin-testkit` to consume the codec from `kelir-interface`.
  3. Ensure all header length constants use the single SSOT: `kelir_interface::codec::HEADER_LEN` (9 bytes).

### 1.2 Strongly Type and Standardize `PatchPayload::ExecveMutation`
* **Defect:** `PatchPayload::ExecveMutation(Vec<u8>)` (`crates/kelir-interface/src/protocol_wire.rs`) stores an untyped byte vector. Examples encode JSON via `serde_json`; test vectors encode CBOR via `ciborium`.
* **Fix:**
  1. Refactor `PatchPayload::ExecveMutation` to hold the strongly typed `ExecveMutation` struct directly:
     ```rust
     pub enum PatchPayload {
         ExecveMutation(ExecveMutation),
         FsInode { mode: u32, uid: u32, gid: u32, size: u64, content: Vec<u8> },
         Custom { name: String, cbor: ciborium::value::Value },
     }
     ```
  2. Standardize wire serialization on **CBOR** per `cans/11-protocol.md#Wire-framing`.
  3. Update `policy-matrix.rs`, `autonomous-swe-agent.rs`, and testkit codecs to serialize and deserialize `ExecveMutation` via `ciborium` exclusively.

### 1.3 Enforce Power-of-Two Ring Buffer Sizing
* **Defect:** `crates/kelir-transport/src/spsc_ring.rs` calculates index via `head as usize % capacity`. If `capacity` is not a power of two, 32-bit counter overflow causes stream corruption.
* **Fix:**
  1. In `OwnedSpscRing::new(capacity)`: assert `capacity.is_power_of_two()`, or automatically clamp `capacity` to `capacity.next_power_of_two()`.
  2. Replace modulo arithmetic with bitwise masking: `head as usize & (capacity - 1)`.

---

## Phase 2: Hyperkernel Runtime & Plane Wiring

### 2.1 Prevent Console Dispatch from Crashing on Scaffolding Stub
* **Defect:** `TerminalPlane::record_output` calls `self.feed()`, which delegates to `Vt100StateMachine::feed()`. Because VT100 parsing is an L3 stub returning `Err(PluginError::Halt)`, any stdout/stderr chunk immediately aborts execution.
* **Fix:**
  1. In `crates/kelir-core/src/planes/terminal/mod.rs`, modify `Vt100StateMachine::feed`:
     ```rust
     pub fn feed(&mut self, _bytes: &[u8]) -> Result<(), PluginError> {
         // L2 Scaffolding: no-op until full L3 VT100 state machine arrives.
         Ok(())
     }
     ```
  2. Keep `export_screen_matrix()` returning the default/wireframe grid. Ensure guest console output logs to disk without blowing up the supervisor.

### 2.2 Unify Session State Root & Workspace Directory Layout
* **Defect:** `Session::boot_with_engine` sets workspace to `std::env::temp_dir().join("kelir").join(&encode_hex(&incarnation_id)).join("workspace")`, while sockets and logs are keyed to `~/.kelir/sessions/<session_id>/` per `cans/01-architecture.md#Host-state-root`.
* **Fix:**
  1. Align workspace root resolution in `crates/kelir-core/src/session_lifecycle.rs` with `cans/01` and `cans/06`:
     ```rust
     let session_hex = encode_hex(session_id);
     let session_dir = home_dir()?.join(".kelir").join("sessions").join(&session_hex);
     let workspace_root = session_dir.join("workspace");
     ```
  2. Ensure ephemeral cleanup routines reap `workspace`, `relay.sock`, and `logs` together under the same session directory tree.

### 2.3 Throttle UART Early Boot String Scraping
* **Defect:** `crates/kelir-core/src/uart_terminal.rs` triggers `decode_exit_kind()` on every byte write, allocating a fresh 64KB `String` and running multi-line regex/find checks.
* **Fix:**
  1. Restrict panic scraping to the spec-mandated condition (`cans/01-architecture.md#Early-boot-diagnostics`): scrape *only* when boot budget expires without reaching `VmReady`, or on hypervisor fault transitions.
  2. Remove inline allocation from `record_early_boot_output`.

### 2.4 Wire `LockBrokerWiring` into Storage Plane Lazy Initialization
* **Defect:** `StoragePlane::with_lock_broker(wiring)` stores `self.lock_broker_wiring = Some(wiring)`, but `self.lock_broker_arc()` instantiates `LockBroker::new(interposer)` without applying the wiring.
* **Fix:**
  1. Plumb `LockBrokerWiring` (`scope_globs`, `non_blocking`) into `LockBroker::with_wiring(interposer, wiring)`.

---

## Phase 3: Boundary & FFI Governance

### 3.1 Close the 7-Ring FFI Governance Bypass
* **Defect:** `try_foreign_submit()` and `foreign_submit_complete()` are only called on `dispatch_trap`, `dispatch_fs_write`, and `dispatch_net_outbound`. The remaining 7 rings bypass rate limiting and the 4,096-capacity queue.
* **Fix:**
  1. In `crates/kelir-core/src/dispatch_plugin.rs`, wrap foreign execution in:
     * `dispatch_net_inbound`
     * `dispatch_console_input`
     * `dispatch_console_output`
     * `dispatch_exec_stdout`
     * `dispatch_exec_stderr`
     * `dispatch_host_lsm_exec`
     * `dispatch_process_exit`
  2. Reject or queue submissions when `ffi_rate_window_check()` or `foreign_queue.try_submit()` returns saturated.

### 3.2 Eliminate Error Context Amnesia in `ErrorEnvelope`
* **Defect:** `From<PluginError> for ErrorEnvelope` flattens all rich Rust errors into `code: "EConfig"` and drops `driver`, `ring`, `pid`, `binary`, and `errno` to `None`.
* **Fix:**
  1. In `crates/kelir-interface/src/error_envelope.rs`, map `PluginError` variants to concrete fields:
     * `PluginError::WxViolation { pid, operation }` → `pid = Some(pid)`, `code = "EWXVIOLATION"`, `errno = Some(libc::EPERM)`.
     * `PluginError::Reject { errno, reason }` → populate `errno` and `message`.
     * `PluginError::FfiQueueSaturated` → `code = "EFFIQUEUESATURATED"`, `errno = Some(libc::EBUSY)`.
  2. Ensure the NAPI and PyO3 bridges serialize the full populated JSON envelope to TS and Python.

### 3.3 Connect Trampoline Descriptor to Pipeline Dispatch
* **Defect:** `PipelineEngine` accepts `with_trampoline(descriptor)`, but dispatch never reads `self.trampoline`.
* **Fix:**
  1. In `crates/kelir-core/src/dispatch_plugin.rs`, add the dispatch check: if `self.trampoline` is present, check whether the active hook matches an AOT-fused ordinal before invoking separate interposers.
  2. Document the exact hook-fusion handoff boundary between native Rust interposers and the foreign NAPI/PyO3 trampoline function.

---

## Phase 4: SDK, Builder & Profile Parity

### 4.1 Plumb `StorageConfig` Across Rust, NAPI, and PyO3
* **Defect:** `StorageConfig` / `NativeStorageConfig` exposes `idmap`, `shared_caches`, `lock_broker`, `mask_read`, `encapsulation`, and `scan_engine`, but builder boot implementations drop everything except `synthetic_files`.
* **Fix:**
  1. Expand `kelir_core::session_lifecycle::PlaneWiring` to carry `StorageWiring`:
     ```rust
     pub struct StorageWiring {
         pub shared_caches: Vec<PathBuf>,
         pub lock_broker: Option<LockBrokerWiring>,
         pub idmap: Option<IdmapConfig>,
         pub mask_read: Option<MaskReadConfig>,
         pub encapsulation: WorkspaceEncapsulation,
     }
     ```
  2. In `crates/kelir/src/builder_sdk.rs`, `crates/kelir-bindings/napi/src/lib.rs`, and `crates/kelir-bindings/pyo3/src/lib.rs`: map incoming configuration into `StorageWiring` and forward to `Session::boot_with_engine`.

### 4.2 Replace Hand-Rolled Profile Crawlers in TS and Python
* **Defect:** `packages/ts/core/src/sandbox.runtime.ts` and `packages/py/kelir/lifecycle_sandbox.py` use manual 28-entry string-path tuples and ad-hoc dictionary crawlers, violating `cans/03-sdk-architecture.md#Params-ingress-contract`.
* **Fix:**
  1. **TypeScript:** Plumb validation through static models or pure TypeBox schemas. Eliminate `profileValueAt` and manual TOML string concatenation in favor of structured serialization.
  2. **Python:** Plumb `ProfileConfig` through a Pydantic v2 `BaseModel` (already an explicit dependency in `pyproject.toml`). Replace `_dig` and `_render_toml` with `.model_dump()` and standard serialization.

### 4.3 Unify `PidLineage` Scoping
* **Defect:** Half the test suite uses `PidLineage::new(1)` (hardcoded zero incarnation UUID), bypassing multi-tenant/session isolation in `CheckpointPlane`.
* **Fix:**
  1. Mark `PidLineage::new(root_pid)` as `#[deprecated]` or make it crate-private.
  2. Update `git-gate.rs`, `replay.rs`, and `policy-matrix.rs` to construct lineage strictly via `session.lineage(1)` or `PidLineage::new_in_session(1, session.incarnation_id)`.

---

## Phase 5: Plugins & Telemetry Stabilization

### 5.1 Prevent `c.ledger.flush()` from Bricking the Runtime
* **Defect:** `OtlpEncoder::encode_metrics` returns `Err(OtlpError::NotImplemented)`, causing `LedgerCtx::flush()` to return `PluginError::Halt`. This crashes every integration test calling `.flush()`.
* **Fix:**
  1. In `crates/kelir-plugins/plugin-ledger/src/lib.rs`, handle OTLP export failures per `cans/plugins/ledger.md#Invariants-and-policy-(ledger)` (*"Export failure never blocks the pipeline"*):
     ```rust
     if let Err(err) = exporter.export_metrics(&snapshot) {
         tracing::warn!("OTLP export skipped during scaffolding: {err}");
         return Ok(());
     }
     ```

### 5.2 Connect `HandlerCtx.step` to Checkpoint Storage
* **Defect:** `HandlerCtx` initializes an empty `StepStack::default()`. Calls to `c.step.last()` return `None`, causing post-exec checkpoints in examples to halt with `"no pre-exec checkpoint"`.
* **Fix:**
  1. In `crates/kelir-core/src/dispatch_plugin.rs`, before dispatching pre-exec traps, query `CheckpointPlane::active_step(lineage)` and populate `c.step.push(descriptor)`.
  2. Ensure `c.step.last()` accurately returns the active pre-step descriptor.

### 5.3 Formalize Plugin Scaffolding Boundaries
* **Defect:** Leaf plugins have incomplete stubs that create illusions of security (e.g. `McpFrame::decode` always returning `McpMethod::Other` in `plugin-tool-guard`).
* **Fix:**
  1. **Tool-Guard:** If JSON-RPC decoding is deferred, fail closed on unparsed MCP frames rather than silently returning `Verdict::Pass`.
  2. **DB-Proxy:** Demarcate the TCP reassembly engine in `crates/kelir-plugins/plugin-db-proxy/src/reassembler_wire.rs` with an explicit architectural marker:
     `// SCAFFOLDING STAND-IN: To be replaced by WirePlane::divert_l4 (cans/07-wire-plane.md) in L3.`

---

## Phase 6: Tooling, Codegen & Hygiene Cleanups

### 6.1 Purge Dirty Codegen Monkey-Patches
* **Defect:** `crates/kelir-interface/src/bin/codegen/sdk_py.rs` maintains hardcoded tables `legacy_fields` and `legacy_aliases` that manually mutate AST models.
* **Fix:**
  1. Align Python AST definitions directly with Rust struct definitions.
  2. Remove `legacy_fields` and `legacy_aliases`. Regenerate `packages/py/kelir/generated/` and verify parity.

### 6.2 Fix Scorecard Generator Synchronization
* **Defect:** `cargo run -p kelir-testkit --bin scorecard -- generate` outputs to `target/scorecard.md`, while `--check` diffs against `cans/artifacts/scorecard.md`.
* **Fix:**
  1. Add `--in-place` flag to `scorecard -- generate` that updates `cans/artifacts/scorecard.md` directly.
  2. Update `justfile` so `just scorecard` automatically updates the checked artifact.

### 6.3 Align AS-01 Anti-Slop Linter Targets
* **Defect:** `crates/kelir-testkit/src/bin/check_tests.rs` scans for `"MockVmmDriver"`, `"MockHostInterposer"`, and `"MockBusDispatcher"`, none of which exist.
* **Fix:**
  1. Update `check_tests.rs` to scan for current test double types: `"FixtureVmmDriver"` and `"LoopbackStream"`.
  2. Consolidate `lint_snippet` and `lint_file` rules so their prohibited type lists match exactly.

### 6.4 Delete Duplicate `VmmDriver` Boilerplate in `testkit`
* **Defect:** `FixtureVmmDriver` (`crates/kelir-plugin-testkit/src/handlers_testkit.rs`) manually copy-pastes 20+ stub methods that already have default implementations on the `VmmDriver` trait.
* **Fix:**
  1. Delete all redundant method implementations from `FixtureVmmDriver`. Implement only `capabilities()` and `boot()`.

### 6.5 Deduplicate `kelir-plugin-cli` Commands
* **Defect:** `crates/kelir-plugin-cli/src/main.rs` triplicates `check-layers`, `layers`, and `check --layers` across separate match arms.
* **Fix:**
  1. Normalize CLI arguments with `clap` aliases: alias `check-layers` and `layers` to a single subcommand handler.
  2. Normalize `check-budgets` and `budgets` identically.

---

## Acceptance & Verification Gate

The work order is complete when and only when all of the following commands execute cleanly with zero errors:

```bash
# 1. Spec & Hygiene Verification
just cans-check
just check-hygiene
cargo run -p kelir-testkit --bin check_tests

# 2. Rust Workspace Compilation & Lint
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --exclude kelir-integration

# 3. SDK Parity & Type Checking
pnpm --filter @kelir/core run build
pnpm --filter @kelir/core run test
mypy packages/py/kelir

# 4. End-to-End Example Execution (No Runtime Bombs)
cargo run -p kelir-integration --example autonomous-swe-agent
cargo run -p kelir-integration --example policy-matrix
cargo run -p kelir-integration --example git-gate
```
