# `work-order.md` (Scope 23424) — Tasks 1–6

> Provenance: user-pasted order, 2026-11-10 09:56 WIB. It was saved to
> `work-order.md` uncommitted, then `b767b0b` replaced `work-order.md` with the
> pre-L3 red-test order (which claims Scope 23424 "EXECUTED" — that claim
> refers to the Phases 1–6 doc archived at `work-order-23424.md`, NOT to this
> Tasks 1–6 order; none of Tasks 1–6 was in the tree at `b767b0b`: no
> `PluginError::Reject`, no `serde_hex_32`, no `unregister_session`,
> no `Session.from_native`, no `hydrate_step` caller). This file preserves the
> Tasks 1–6 text verbatim so the order is not lost. Implemented on scaffolding
> on top of `b767b0b` (work-order.md untouched).

## Target Phase: L1/L2 Scaffolding & Plumbing Stabilization
**Target Branch:** `scaffolding`
**Rule:** Fix all plumbing bugs, contract mismatches, and data drops. Do NOT write L3 meat logic.

---

### Task 1: Wire Protocol & Serialization Parity
1. **Synchronize `ExecveMutation` Across Languages:**
   - In `crates/kelir-interface/src/protocol_wire.rs`, annotate `ExecveMutation` with serde aliases or update field names:
     ```rust
     #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
     #[serde(rename_all = "camelCase")]
     pub struct ExecveMutation {
         #[serde(alias = "binary")]
         pub new_binary: Option<String>,
         #[serde(alias = "argv")]
         pub new_argv: Option<Vec<String>>,
         #[serde(default)]
         pub set_env: Vec<(String, String)>,
         #[serde(default)]
         pub unset_env: Vec<String>,
     }
     ```
   - In TypeScript (`packages/ts/core/src/types.core.ts`) and Python (`packages/py/kelir/__init__.py`), ensure `ExecveMutation` matches this representation.
2. **Standardize `CausalDelta` Byte Arrays:**
   - In `crates/kelir-interface/src/telemetry/delta_attest.rs`, serialize `storage_delta_root` and `receipt_leaf_hash` as hex strings when targeting JSON/SDKs, or use a custom serde helper `serde_hex`.

### Task 2: Structured Error Envelope Governance
1. **Add `PluginError::Reject` Variant:**
   - In `crates/kelir-interface/src/plugin_model.rs`, change `PluginError::reject` to construct a real structured variant:
     ```rust
     #[derive(Error, Debug)]
     pub enum PluginError {
         #[error("Rejected with {errno}: {reason}")]
         Reject { errno: String, reason: String },
         ...
     ```
2. **Plumb `Reject` to `ErrorEnvelope`:**
   - In `crates/kelir-interface/src/error_envelope.rs`:
     ```rust
     PluginError::Reject { errno, reason } => Self {
         code: format!("E{}", errno),
         message: reason,
         driver: None,
         ring: None,
         pid: None,
         binary: None,
         errno: Some(errno),
     },
     ```

### Task 3: Python SDK Native Bridge Connection
1. **Plumb PyO3 Session Handle:**
   - In `packages/py/kelir/__init__.py`, update `KelirBuilder.boot()` to call `native.boot()`:
     ```python
     native_handle = native.boot()
     return Session.from_native(native_handle)
     ```
   - Implement `Session.from_native(handle)` to delegate `fork()`, `suspend()`, `resume()`, `stop()`, `terminate()`, `storage.reconcile()`, etc., directly to the PyO3 handle.

### Task 4: Connect Builder Lifecycle Callbacks & Dispatch Engines
1. **Wire Engine Trampolines & Callbacks:**
   - In `crates/kelir/src/builder_sdk.rs`, pass `self.registration.custom_actions`, `vsock_services`, and lifecycle handlers into `PipelineEngine`.
2. **Execute Step Hydration in Dispatch Loop:**
   - In `crates/kelir-core/src/dispatch_plugin.rs`, invoke `self.hydrate_step(ctx, PidLineage::new_in_session(ctx.pid, ctx.session_id))` before evaluating traps so that `c.step.last()` is populated.

### Task 5: Catalog Lease Cleanup Mechanics
1. **Add Session Unregistration in `kelir-catalog`:**
   - In `crates/kelir-catalog/src/lib.rs`, add:
     ```rust
     pub fn unregister_session(&self, session_id: &[u8; 16]) -> Result<bool, CatalogError> {
         let count = self.pool.execute(
             "DELETE FROM sessions WHERE session_id = ?1",
             rusqlite::params![&session_id[..]],
         )?;
         Ok(count > 0)
     }
     ```
2. **Hook Cleanup into Session Teardown:**
   - In `crates/kelir-core/src/session_lifecycle.rs`, invoke `catalog.unregister_session(&self.session_id)` inside `reap_ephemeral_session_dir` and during `terminate()`.

### Task 6: Workspace Module Pruning & Deduplication
1. **Remove Ghost Re-exports:**
   - Delete `crates/kelir/src/plugins/` entirely.
   - Clean up `crates/kelir/src/lib.rs` to re-export plugins consistently and cleanly directly from their external crates under `feature` gates.
2. **Consolidate CLI Flags:**
   - In `crates/kelir-plugin-cli/src/main.rs`, replace separate command arms with clap subcommand aliases (`check-layers` and `layers` aliasing to `check --layers`).

---

### Verification Gate

Execute and verify clean passes:
```bash
# 1. Spec & manifest synchronization
cargo run -p kelir-testkit --bin check-manifest
cargo run -p kelir-testkit --bin check-tests
cargo run -p kelir-testkit --bin scorecard -- check

# 2. Rust workspace checks
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --exclude kelir-integration

# 3. SDK Type checking & unit tests
pnpm --filter @kelir/core run typecheck
pnpm --filter @kelir/core run test
mypy packages/py/kelir
```
