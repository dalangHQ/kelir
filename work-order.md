# Maturity-Layer Leakage Audit

## Convention (inferred from the tree)
- `*.run.rs` → **L1** (runtime implementation)
- `*.flow.rs` → **L2** (flow/orchestration logic)
- `*.rs` (plain) → **L3** (signatures, type contracts, TODO-L3 stubs only)

Below are the places where that contract is broken.

---

## 1. L3 files (plain `.rs`) that contain implementation "meat"

### `crates/kelir-engine/src/dispatch_plugin/params_hydration.rs`  ← plain `.rs` = L3
```rust
pub fn hydrate_params_ctx(ctx: &HandlerCtx, session: Option<&Session>) -> Option<HandlerCtx> {
    let exec_ctx = ctx.exec_ctx.as_ref()?;
    let buffer = session.exec_params(exec_ctx.exec_id, exec_ctx.stream_id)?;
    Some(ctx.clone().with_params_buffer(buffer))
}
```
This is a real function body (exec-param buffer lookup and `HandlerCtx` cloning). It should be a signature + `TODO (L3)` stub, with the body in a `.flow.rs` or `.run.rs` file.

### `crates/kelir-plugins/plugin-ledger/src/compute_ledger.rs`  ← plain `.rs` = L3
```rust
pub fn compute_causal_delta(...) -> ... {
    Ok(CausalDelta::new(
        ...
        storage.modified_inodes.clone(),
        storage.spawned_pids.clone(),
        ...
        receipt.leaf_hash(),
    ))
}
```
Real causal-delta construction in an L3 file. The body belongs in `wal_ledger.flow.rs` / `otel_ledger.flow.rs` or a `.run.rs`.

### `crates/kelir-interface/src/storage/storage_snoop.rs`  ← plain `.rs` = L3
`SnoopRule::new`, `SnoopRule::path`, and the `Debug` impl all have real bodies. Trivial, but still implementation in an L3 file; should be signatures + `TODO (L3)` or moved into a `.flow.rs`.

### `crates/kelir-interface/src/storage/storage_io.rs`  ← plain `.rs` = L3 (borderline)
The trait's default methods (`write`, `mkdir`, `unlink`, `rename`) have bodies that only return `Err(PluginError::Halt("TODO (L3)…"))`. This is arguably "signature + TODO marker", but strictly it is still function-body meat in L3. Safer form: declare the trait methods without bodies and put the TODO markers in doc comments.

---

## 2. L2 files (`.flow.rs`) that carry L3 concerns (TODO-L3 stubs / L3-gap markers)

These files embed L3 work-in-progress directly in flow logic instead of deferring to an L3 stub:

| File | Leaked L3 concern |
|---|---|
| `crates/kelir-interface/src/checkpoint/step_dag.flow.rs` | `StepAttribution::verify` contains `TODO (L3) [lifecycle_checkpoint]` and returns `StepDagError::SignatureVerificationPending`. Ed25519 verification is an L3 contract; the stub should live in an L3 file. |
| `crates/kelir-interface/src/storage/isolation_envelope.flow.rs` | `resolve_beneath` contains `TODO (L3) [storage_plane]: open the resolved path with openat2(...)`. The L2 path-resolution code is fine; the L3 `openat2` concern should be a separate L3 stub. |
| `crates/kelir-interface/src/wire/l7_parser.flow.rs` | `HostKeyringDecryptor::decrypt` contains `TODO (L3) [wire_plane]` and returns `L7ParseError::TlsUnavailable("… TODO (L3)")`. TLS keyring decryption is L3; the stub belongs in an L3 file. |
| `crates/kelir-engine/src/session_paths.flow.rs` | Two `L3-GAP` markers (`lifecycle_checkpoint`, `lc_16_session_incarnation_cas_guard`) live in L2 flow code. |
| `crates/kelir/src/profile_validate.flow.rs` | Two `L3-GAP` markers (`hypervisor_driver`, `hd_fast_boot_backend_change`) live in L2 flow code. |

---

## 3. Interface contracts (traits) defined in L2 `.flow.rs` files

Traits are pure signatures and belong in L3 (plain `.rs`). The following `.flow.rs` files define `pub trait` items:

- `crates/kelir-interface/src/host/ops.flow.rs` → `pub trait HostOps` (plus `PressureLevel::from_u8` / `as_u8` impl logic — also L3-ish)
- `crates/kelir-interface/src/checkpoint/ops.flow.rs` → `pub trait CheckpointOps`
- `crates/kelir-interface/src/wire/l4_ops.flow.rs` → `pub trait L4DivertOps`
- `crates/kelir-interface/src/terminal/ops.flow.rs` → `pub trait TerminalOps`

Contrast with `crates/kelir-interface/src/storage/storage_io.rs` and `storage_reconciler.rs`, where traits are correctly in L3 plain `.rs`. The four traits above should move to L3 files (e.g. `host/ops.rs`, `checkpoint/ops.rs`, `wire/l4_ops.rs`, `terminal/ops.rs`), leaving only flow-level wrappers in the `.flow.rs` files.

---

## 4. Summary

| Violation class | Files |
|---|---|
| **L3 file with real body** | `kelir-engine/src/dispatch_plugin/params_hydration.rs`, `plugin-ledger/src/compute_ledger.rs`, `kelir-interface/src/storage/storage_snoop.rs`, (borderline) `kelir-interface/src/storage/storage_io.rs` |
| **L2 file with L3 TODO/stub** | `kelir-interface/src/checkpoint/step_dag.flow.rs`, `kelir-interface/src/storage/isolation_envelope.flow.rs`, `kelir-interface/src/wire/l7_parser.flow.rs`, `kelir-engine/src/session_paths.flow.rs`, `kelir/src/profile_validate.flow.rs` |
| **Trait (L3 contract) in L2 file** | `kelir-interface/src/host/ops.flow.rs`, `checkpoint/ops.flow.rs`, `wire/l4_ops.flow.rs`, `terminal/ops.flow.rs` |

No L1 (`.run.rs`) file in the supplied snapshot was found to contain L2/L3 concerns; the leaks are concentrated in L3-with-meat and L2-with-L3-stubs/markers.



You're right to push back — my first pass was incomplete. On a full re-scan of the file contents actually shown, I missed two clear-cut L3 files that carry real implementation, and I under-flagged two borderline cases. Corrected list below.

---

## Additions — L3 (plain `.rs`) files with real implementation "meat"

### 1. `crates/kelir-plugin-testkit/src/assertions_testkit.rs`  ← **missed**
Plain `.rs` = L3, but it contains seven functions with full bodies:

```rust
pub fn assert_pass(verdict: &Verdict) -> Result<(), PluginError> { ... }
pub fn assert_deny(verdict: &Verdict) -> Result<(), PluginError> { ... }
pub fn assert_patch(verdict: &Verdict) -> Result<(), PluginError> { ... }
pub fn assert_suspend(verdict: &Verdict) -> Result<(), PluginError> { ... }
pub fn assert_abort(verdict: &Verdict) -> Result<(), PluginError> { ... }
pub fn assert_rewrite(input: &[u8], output: &[u8]) -> Result<(), PluginError> { ... }   // byte-slice window scan
pub fn assert_env_injected(env: &[(String, String)], key: &str) -> Result<(), PluginError> { ... }
pub fn assert_disclosure(output: &[u8], secret: &[u8]) -> Result<(), PluginError> { ... } // substring leak scan
```

`assert_rewrite` and `assert_disclosure` in particular contain non-trivial logic (windowed slice comparison, empty-secret guard). Per the rule, this file should be signatures + `TODO (L3)` only, with the bodies in a `.flow.rs`/`.run.rs` sibling.

### 2. `crates/kelir-interface/src/bin/codegen/naming_codegen.rs`  ← **missed**
Plain `.rs` = L3, but it is a fully-implemented naming-conversion module:

```rust
pub fn split_camel(name: &str) -> Vec<String> { ... }   // camel-boundary scanner
pub fn screaming(name: &str) -> String { ... }
pub fn kebab(name: &str) -> String { ... }
pub fn camel(name: &str) -> String { ... }              // underscore + camel handling
pub fn snake(name: &str) -> String { ... }
```

All five have real bodies. The `// see: cans/...` comments are doc refs, not `TODO (L3)` markers — this is implementation, not a signature stub.

---

## Borderline / reverse-direction leaks I under-flagged

### 3. `crates/kelir-drivers/driver-hvf/src/sys_driver.run.rs`  — **L3 concern in an L1 file**
`.run.rs` = L1, but the entire file is:

```rust
pub type hv_return_t = libc::c_int;
pub type hv_vcpu_t = u64;
// SAFETY: declarations only — ... no call is made from safe code yet ...
```

Type aliases + a SAFETY note, no runtime work. That's an L3 contract sitting in an L1 file — the mirror-image of the violation the rule is meant to prevent.

### 4. `crates/kelir-interface/src/storage/storage_io.rs`  — **borderline (I noted it, but understated it)**
The `StorageIo` trait's default methods (`write`, `mkdir`, `unlink`, `rename`) each have a body that returns `Err(PluginError::Halt("TODO (L3) …"))`. Strictly this is "signature + TODO marker", but it is still a function body inside an L3 file. Cleaner form: declare the trait methods body-less and place the `TODO (L3)` markers in doc comments.

### 5. `crates/kelir-interface/src/terminal/log.flow.rs`  — **L3 concern in an L2 file**
`.flow.rs` = L2, but the file is almost entirely type declarations (`LogCursor`, `LogStream`, `LogSegment`) plus two trivial impls (`start`, `advance`, `new`). This is closer to an L3 contract file than to flow orchestration. Not a meat violation, but it is a layer-mixing smell — either it belongs in L3, or the trivial impls should be split out.

---

## Reaffirming what was already correct

- **L3 with meat (caught first pass):** `params_hydration.rs`, `compute_ledger.rs`, `storage_snoop.rs`.
- **L2 with L3 TODO/stub (caught first pass):** `step_dag.flow.rs`, `isolation_envelope.flow.rs`, `l7_parser.flow.rs`, `session_paths.flow.rs`, `profile_validate.flow.rs`.
- **Traits (L3 contracts) sitting in L2 files (caught first pass):** `host/ops.flow.rs`, `checkpoint/ops.flow.rs`, `wire/l4_ops.flow.rs`, `terminal/ops.flow.rs`.

---

## Important caveat

I can only audit files whose contents are shown. Many plain `.rs` files in the tree have **no content supplied** and therefore cannot be cleared — e.g. `addon_abi.rs`, `capability_registry.rs`, `chaos_types.rs`, `hex_codec.rs`, `mount_scope.rs`, `spawn_transport.rs`, `plugin_model.rs` (both `kelir` and `kelir-interface`), `fixtures_testkit.rs`, `vectors_chain_testkit.rs`, `case_map.rs`, `perf_budgets.rs`, and the five `kelir-plugin-cli` plain-`.rs` files. Any of these could hide the same class of violation. So: **not "that's all" — but that is the complete set visible in the provided snapshot.**