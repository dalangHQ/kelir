# work-order.md

## Deprecation & Diff-Apologetics Cleanup — Phase L1/L2

### Intent
Remove every artifact that reads as a diff against a prior version of Kelir: `#[deprecated]`, serde aliases, `_V1` schema suffixes, SDK rename shims, "superseded"/"fallback"/"today" language, and `#[allow(deprecated)]` suppressions. Ship one canonical name and one canonical shape per concept. **This is a cleanup order, not a feature order.**

### Phase constraint: L1/L2 only
- **Do not implement any L3 TODO.** Every `// TODO (L3):` stays exactly as it is.
- **Do not touch L3 code paths.** Wire protocol, FUSE worker, shim fast-path, driver boot, etc. remain untouched.
- The only behavior changes allowed are the public-surface renames explicitly called out below (`TelemetryExtension` → `ReceiptExtension`, `NativeSnapshotRecord` → `NativeSnapshotDescriptorRecord`, `SCHEMA_V1_SQL` → `SCHEMA_SQL`). Everything else is a no-op cleanup that must keep `cargo test --workspace` green.
- **Cross-hypervisor, cross-SDK, and CLI behavior must not change** — only the source spelling of names, derives, and comments.

### Global guardrails (all agents)
- Never add a compatibility shim (`type X = Y`, `#[serde(alias)]`, `#[deprecated]`, dual `pub use`, dual CLI verb, dual field name) as the resolution.
- Never write a `// TODO: delete once …` comment. If a type needs a derive, add the derive.
- Do not add new public items to hide the rename; rename in place and update every caller.
- Respect per-crate `[lints]` in `Cargo.toml` (e.g. `unwrap_used = "deny"`, `panic = "deny"`, `expect_used = "deny"`). The `fetch_update` replacement loops in Agent 3 must use `compare_exchange_weak` and must not `unwrap`.
- Run `cargo check --workspace --all-features` and `cargo clippy --workspace --all-features` before declaring done.
- Do not touch files outside your ownership map unless the work order says otherwise.

### Verification gate (run by the merge lead after all 5 agents land)
```sh
cargo check --workspace --all-features
cargo clippy --workspace --all-features
cargo run -p kelir-interface --bin codegen      # must produce a clean tree
cargo test --workspace
grep -rn 'serde(alias' crates/                  # empty
grep -rn 'TelemetryExtension' crates/ packages/ # empty
grep -rn 'NativeSnapshotRecord' packages/       # empty
grep -rn '\bMaskRule\b' crates/                 # only MaskReadRule hits
grep -rn 'check-layers\|check-budgets' crates/  # empty
grep -rn 'fallback carrier\|primary carrier' crates/  # empty
grep -rn '#\[deprecated' crates/                # empty (except Agent 1's own PartialEq derives on WatchdogAction are fine; only #[deprecated] is banned)
```
If any grep produces output, the offending agent re-opens.

---

## Ownership map

| Agent | Owns |
|---|---|
| **A1** | `crates/kelir-interface/src/protocol_wire.rs`, `crates/kelir-interface/src/driver_interface.rs` |
| **A2** | `crates/kelir-interface/src/telemetry/*`, `crates/kelir-interface/src/bin/codegen/*`, all `packages/py/kelir/generated/*`, all `packages/py/kelir/plugins/plugin_*.py`, all `packages/ts/core/src/generated/*`, `packages/ts/core/src/telemetry.attest.generated.ts`, `packages/ts/core/src/plugins.registry.ts`, `crates/kelir-otlp/src/records_otlp.rs`, `crates/kelir-plugins/plugin-ledger/src/wal_ledger/merkle_wal.rs` |
| **A3** | `crates/kelir-otlp/src/registry_otlp.rs`, `crates/kelir-interface/src/plugin_model.rs`, `crates/kelir-plugins/plugin-replay/src/lib.rs`, `crates/kelir-plugin-testkit/src/vectors_testkit.rs` |
| **A4** | `crates/kelir-core/src/planes/checkpoint/mod.rs`, `crates/kelir-core/src/session_lifecycle.rs`, `crates/kelir/src/planes/storage_plane.rs` + any callers found by grep |
| **A5** | `crates/kelir-catalog/src/schema_catalog.rs`, `crates/kelir-catalog/src/lib.rs`, `crates/kelir-plugin-cli/src/main.rs`, `crates/kelir-plugin-cli/src/layers_cli.rs`, `crates/kelir-shim/src/main.rs`, `crates/kelir-drivers/driver-rustvmm/src/host_probe.rs`, `crates/kelir-core/src/dispatch_plugin.rs` |

**No two agents own the same file.** A1, A2, A3 all work inside `kelir-interface/src/` but never the same file. A4 and A5 both work inside `kelir-core/src/` but never the same file.

---

## A1 — Interface core types

**Scope:** `protocol_wire.rs`, `driver_interface.rs` only. Do not touch other files in `kelir-interface/src/` (A2, A3 own those).

**Findings to close:**
- **Finding 2** — Remove `#[serde(alias = "binary")]`, `#[serde(alias = "argv")]`, `#[serde(default, alias = "env", ...)]` from `ExecveMutation`. Only `new_binary`, `new_argv`, `set_env`, `unset_env` are acceptable wire field names. Keep `#[serde(default)]` where it is semantically required (empty defaults), but drop every `alias`.
- **Finding 12** — Remove `#[allow(clippy::derivable_impls)]` and either `#[derive(Default)]` on `StreamKind` with `#[default]` on `Exec`, or hand-write the impl without the lint suppression. Preferred: derive, because that's the canonical form. If the derive changes semantics (it doesn't — `Exec = 0x0001` is the current default), keep hand-written but drop the allow.
- **Finding 7 prerequisite** — Add `#[derive(..., PartialEq, Eq)]` to `WatchdogAction` in `driver_interface.rs`. A3 consumes this; do not tell A3 to add it.

**Done when:** `grep -n 'serde(alias' crates/kelir-interface/src/protocol_wire.rs` is empty; `grep -n 'derivable_impls'` in `protocol_wire.rs` is empty; `WatchdogAction` derives `PartialEq, Eq`.

---

## A2 — Codegen SDK unification

**Scope:** the `codegen/` directory, `telemetry/receipt_attest.rs`, `telemetry/mod.rs`, `telemetry/merkle_attest.rs`, and every file the codegen binary emits. **You are the only agent that runs codegen.**

**Findings to close:**
- **Finding 4** — Remove the rename shims in `naming_codegen.rs`:
  - Delete `ts_sdk_name` and `py_sdk_name` special-casing for `TelemetryExtension`. Then **rename the Rust struct** `TelemetryExtension` → `ReceiptExtension` in `receipt_attest.rs` and `telemetry/mod.rs`. Rename every Rust usage in `kelir-interface` (`merkle_attest.rs` destructure) and every plugin that imports it (should be none; grep to confirm). Delete `py_sdk_name`/`ts_sdk_name` from `naming_codegen.rs` and inline `rust_name.to_string()` at their call sites (`py_codegen::py_type`, `ts_codegen::ts_type`, `sdk_py`/`sdk_ts` if used there).
  - Delete `ts_native_record_name`'s `SnapshotDescriptor` special case so it returns `format!("Native{rust_name}Record")` uniformly. This makes the emitted TS type `NativeSnapshotDescriptorRecord`, which is the pattern-correct name; update `ts_codegen::ts_native_type` if it references the old literal.
  - Update `ts_codegen::ts_convert_expr`'s `if name != "TelemetryExtension"` guard to `if name != "ReceiptExtension"`.
  - Update any codegen header `// see:` lines only if they became false; otherwise leave them.
- Run `cargo run -p kelir-interface --bin codegen` **twice** — the second run must show zero writes (`plugin sdk projections: N plugins ok` with no `wrote` lines on the second invocation). Commit every regenerated file.

**Done when:** `grep -rn 'TelemetryExtension\|ts_sdk_name\|py_sdk_name\|ts_native_record_name' crates/ packages/` is empty; `grep -rn 'NativeSnapshotRecord\b' packages/` matches nothing (only `NativeSnapshotDescriptorRecord`); a second codegen run is a no-op.

---

## A3 — Suppression & shim removal

**Scope:** `registry_otlp.rs`, `plugin_model.rs`, `plugin-replay/lib.rs`, `vectors_testkit.rs`. Do not touch `driver_interface.rs` (A1 owns it) — `WatchdogAction` will have `PartialEq` by the time you merge.

**Findings to close:**
- **Finding 11** — Replace every `#[allow(deprecated)]`-wrapped `fetch_update` with a `compare_exchange_weak` loop. Three call sites:
  - `registry_otlp.rs::exec_finished` — `in_flight_execs` saturating-decrement, must not underflow to `u64::MAX`.
  - `plugin_model.rs::FfiDispatchQueue::try_submit` — saturating-increment bounded by `capacity`, must return `Err(PluginError::FfiQueueSaturated { depth })` with the observed depth on failure (preserve current API).
  - `plugin_model.rs::FfiDispatchQueue::complete` — saturating-decrement to zero.
  - `plugin-replay/lib.rs::ReplayState::record_pmu_sample` — two `fetch_update` saturating-adds (`retired_total`, `window_retired`). Replace with the same loop shape.
  - Each loop must handle `Err(actual) => current = actual;` and never `unwrap`.
- **Finding 7** — In `vectors_testkit.rs`, delete the `watchdog_action_eq` helper (including its `// TODO (L3): delete once …` comment). Replace its single call site with `armed_action == event.action`. This compiles because A1 adds `PartialEq` to `WatchdogAction`.

**Done when:** `grep -n 'allow(deprecated)' crates/kelir-otlp/src/registry_otlp.rs crates/kelir-interface/src/plugin_model.rs crates/kelir-plugins/plugin-replay/src/lib.rs` is empty; `grep -n 'watchdog_action_eq\|delete once' crates/kelir-plugin-testkit/src/vectors_testkit.rs` is empty; `cargo clippy -p kelir-otlp -p kelir-interface -p plugin-replay -p kelir-plugin-testkit` is clean.

---

## A4 — Core runtime & SDK façade

**Scope:** `kelir-core/src/planes/checkpoint/mod.rs`, `kelir-core/src/session_lifecycle.rs`, `kelir/src/planes/storage_plane.rs`, plus any callers surfaced by grep.

**Findings to close:**
- **Finding 1** — Delete `PidLineage::new` (the `#[deprecated]` one) entirely. Grep the workspace for `PidLineage::new(` callers; migrate each to `PidLineage::new_in_session(root_pid, incarnation_id)` using the caller's available incarnation context. If a caller has no incarnation context, that caller is itself an L3 pending path — leave it as a TODO, do **not** re-add a shim. `Session::lineage` in `session_lifecycle.rs` already returns the safe form; leave it.
- **Finding 5** — Delete `crates/kelir-core/src/session_lifecycle.rs::Session::from_parts` entirely, including the `// TODO (L2): from_parts is superseded …` comment and the `#[allow(clippy::too_many_arguments)]` attribute. Grep the workspace for `Session::from_parts` / `core::session_lifecycle::Session::from_parts` callers. `crates/kelir/src/session_lifecycle.rs::Session::from_parts` is a **different** function and is used by `builder_sdk.rs` — do not touch it.
- **Finding 8** — In `crates/kelir/src/planes/storage_plane.rs`, change `pub use kelir_core::planes::storage::{..., MaskReadRule as MaskRule, ...}` to `pub use kelir_core::planes::storage::{..., MaskReadRule, ...}`. Grep `crates/` for `MaskRule` (with word boundary) — update each importer to `MaskReadRule`. If zero importers, nothing else to do.

**Done when:** `grep -rn '#\[deprecated' crates/kelir-core crates/kelir` is empty; `grep -rn 'superseded' crates/kelir-core` is empty; `grep -rn 'MaskRule as\|::MaskRule\b' crates/` is empty; `cargo check --workspace` succeeds.

---

## A5 — CLI, shim, driver language & schema

**Scope:** `kelir-catalog/*`, `kelir-plugin-cli/{main,layers_cli}.rs`, `kelir-shim/src/main.rs`, `kelir-drivers/driver-rustvmm/src/host_probe.rs`, `kelir-core/src/dispatch_plugin.rs`. **Comment-only edits in the last two; do not change code semantics.**

**Findings to close:**
- **Finding 3** — In `crates/kelir-catalog/src/schema_catalog.rs`:
  - Rename `SCHEMA_V1_SQL` → `SCHEMA_SQL`. Update the sole caller in `kelir-catalog/src/lib.rs`.
  - Remove the `CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY);` statement from `SCHEMA_SQL`. Grep the workspace for `schema_version` (word boundary) first; if any query reads/writes it, abort and report to the merge lead — do not delete a live column.
  - Do not add a replacement "schema version" table. L1/L2 has no migrations.
- **Finding 6** — In `crates/kelir-plugin-cli/src/main.rs`, delete the alias match arms:
  - `"check-layers" | "layers" => …` becomes `"check-layers" => …`. Only `check-layers` is the CLI verb.
  - `"check-budgets" | "budgets" => …` becomes `"check-budgets" => …`.
  - Check `README`, scripts, or test files for the removed aliases; if found, update those invocations to the canonical verb.
- **Finding 9** — In `crates/kelir-shim/src/main.rs`, reword the `// TODO (L3): SPSC ring primary carrier …; the AF_VSOCK fallback carrier is wired below.` comment to describe what the code **is**: `// The AF_VSOCK carrier is wired below; the SPSC ring carrier per cans/11-protocol.md#Wire-framing replaces this transport once its ring allocation lands.` No "primary", no "fallback".
- **Finding 10** — Reword the three "today"/"L2 … L3" comments to state what the code is or what is missing, without a before/after narrative:
  - `host_probe.rs`: `// TODO (L3): gate every backend on a real capability probe; only kvm exposes a device node today.` → `// TODO (L3): gate every backend on a real capability probe; kvm is the only backend with a device-node surface.`
  - `layers_cli.rs`: `// TODO (L3): …; the scan bans only core/driver/adapter prefixes today.` → `// TODO (L3): enforce the per-tier plugin-to-plugin rule per cans/02-plugin-model.md#Plugin-tiering; the scan bans core/driver/adapter prefixes only.`
  - `dispatch_plugin.rs`: `// TODO (L3): exact token-bucket accounting; this native 1s window is the L2 pre-insertion rate gate.` → `// TODO (L3): exact token-bucket accounting; the 1s window below is the pre-insertion rate gate.`
  - No code changes in these files beyond the reworded comments.

**Done when:** `grep -rn 'SCHEMA_V1_SQL\|schema_version' crates/kelir-catalog` is empty; `grep -rn 'check-layers\|check-budgets' crates/kelir-plugin-cli` matches only the single-arm forms; `grep -rn 'today\|fallback carrier\|primary carrier\|L2 .* L3' crates/kelir-shim/src/main.rs crates/kelir-drivers/driver-rustvmm/src/host_probe.rs crates/kelir-plugin-cli/src/layers_cli.rs crates/kelir-core/src/dispatch_plugin.rs` is empty.

---

## Merge protocol

1. **A1 and A4 may land first**, in either order. A1's `PartialEq` derive is a no-op for codegen; A4's deletions are dead-code. Run `cargo check --workspace` after each.
2. **A5 lands next.** No codegen input is touched. `cargo check --workspace` must stay green.
3. **A3 lands next.** It needs A1's `PartialEq` to compile `vectors_testkit.rs`. `cargo clippy --workspace --all-features` must stay green.
4. **A2 lands last.** Run `cargo run -p kelir-interface --bin codegen`, inspect `git diff`, commit the regenerated files. Then rerun to confirm idempotence.
5. **Merge lead runs the Verification gate** in full. Any grep producing output re-opens the offending agent.
6. **Do not squash commits across agents.** Each agent's diff must be self-contained so a revert is surgical.

---

## What NOT to do

- Do **not** add `#[deprecated]`, `#[allow(deprecated)]`, `#[serde(alias)]`, `type X = Y`, `pub use … as …`, or a dual CLI verb anywhere in the tree. Those are the exact smells this work order removes.
- Do **not** implement any `// TODO (L3):`. The diff stays a cleanup.
- Do **not** rename anything outside the two renames listed (ReceiptExtension, NativeSnapshotDescriptorRecord). In particular, `SnapshotDescriptor`, `TelemetryExtension` at the wire level if it exists, `MaskReadRule`, and the `StreamKind::Exec` default are unchanged in *meaning* — only in spelling/derive form as specified.
- Do **not** touch the `#[non_exhaustive]` derives, `serde(rename_all = "camelCase")`, `Session::attach`, the `kelir/src/session_lifecycle.rs::Session::from_parts` (the used one), or `MaskReadRule`'s struct definition. The prior audit cleared those as forward-compat, not diff-apologetics.
- Do **not** commit a codegen output that differs from a second run — if the second run writes anything, the emitter is non-deterministic; stop and report.
