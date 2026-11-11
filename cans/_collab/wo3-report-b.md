# WO3-B report — Agent B, kelir SDK session surface

Branch `temp/wo3-b`, base 339ca53. All gates green. L1/L2 only; no `TODO (L3)` body filled.

## Per-task outcomes

### B1 — `Session::exec(cmd, limits?)` — DONE
- Added `Session::exec(&self, binary: &str, argv: &[&str], limits: Option<ProcessLimits>) -> SpawnBuilder` (`session_lifecycle.rs`), anchored `// see: cans/03-sdk-architecture.md#Session-surfaces`.
- `spawn(binary, argv)` kept as an async thin alias (`self.exec(binary, argv, None).await`) — same signature, so the `Sandbox` facade, `crates/kelir/tests/*`, and pyo3/napi keep compiling.
- `SpawnBuilder` grew `pub limits: Option<ProcessLimits>` (`ProcessLimits` already exists in `kelir_interface::protocol_wire` with the exact CANS/11 field set — no new types, no new deps).
- Stubbed `encode_exec_control_limits()` is wired into `Future::poll`; actual sub-lane 0x03 wire bytes left as `// TODO (L3):` per the mandate.
- Decision: `exec` returns the builder (not a Future result) so callers can chain `cwd`/`params`, matching how `spawn_builder` is consumed today.

### B2 — `Session::skew_clock` — DONE
- Added `Session::skew_clock(&self, delta: Duration) -> Result<(), PluginError>` delegating to `self.vm().clock.skew(delta)`, anchored `// see: cans/08-lifecycle.md#Clock-skew`.
- Parity gaps found (bridges outside lane, NOT edited): pyo3 has session-level `clock_skew(seconds)`; napi and TS expose only `vm.clock.skew(...)` while CANS/08 pins skew_clock to the Session typestate boundary. Recorded in conflicts.md.

### B3 — checkpoint surface + dead handles — DONE (hybrid of option (b))
- Deleted `StorageHandle` and `WireHandle` from `session_lifecycle.rs`: both were unused (only the lib.rs re-export referenced `WireHandle`), both shadow the real planes `session.storage` / `session.wire` already reachable through `Deref` to `kelir_core::Session`, and `WireHandle::forward_port` even had the wrong signature (`Result<(), _>` vs CANS/07's `Result<u16, WireError>`).
- Kept `VsockHandle`/`VsockChannel` and added `Session::vsock()` — CANS/03 pins `session.vsock.channel(kind)` and vsock is not one of the 5 planes, so there is nothing to shadow; the struct was unreachable dead code, now it is the L2 carrier of the spec surface (runtime stays `TODO (L3)`).
- No `CheckpointHandle` wrapper added: `checkpoint.create(label)`'s semantics are the driver snapshot `session.vm().checkpoint(label)` — the exact mapping pyo3 (`PyCheckpointHandle`), napi (`NativeCheckpointHandle`), and TS (`CheckpointHandle`) wrap. A new Rust wrapper would duplicate `VmHandle::checkpoint` (bloat, rule 5/11). The literal name `session.checkpoint.create(label)` cannot land without `CheckpointPlane::create(label)` (kelir-core, Agent C lane) or an SDK method shadowing the Deref'd plane field — left to the orchestrator.
- Stale claim: `StepHandle`/`CheckpointHandle` never existed in `crates/kelir` — only `Py*`/`Native*` structs in the bindings and TS/Py classes (which route through real planes and are unaffected).
- `rg "StorageHandle|WireHandle|VsockHandle|StepHandle|CheckpointHandle" crates/ packages/` run first: zero references to the kelir-crate structs outside their definition + lib.rs re-export; bindings define their own. No kelir-bindings follow-through was required (deletion compiled clean).

### B4 — `Session::rollback()` signature — DONE
- Added `Session::rollback_here(&mut self, pid: u32) -> Result<(), PluginError>`: derives `PidLineage` via `self.core.lineage(pid)` then delegates to the unchanged `rollback(lineage)`. Callers in a trap/exit context pass `ctx.pid` or `ProcessExit.pid`.
- `rollback(lineage)` NOT changed. True no-arg `rollback()` stays L3 — consistent with the napi stub text ("lineage-derived head rollback lands with PID tracking (L3)").

### B5 — Plugin-mode trap scope coverage — VERIFIED, no change
- The `Mode`-generic impl in `builder_plugin.rs` (line ~369) provides all 24 ring verbs plus `fs/net/console/exec/host/lifecycle/telemetry` scopes for `Mode = Plugin`; `trap` scope is plugin-only per CANS/02 (the host's `trap(filter, f)` occupies the name — E0592 by design).
- Existing test `nested_scopes_match_flat_descriptors` (`crates/kelir/tests/fluent_builder_plugin_test.rs`) compiles and passes all 8 scopes chained on `KelirBuilder::named("nested")` — ran green in this session.
- The order's literal snippet `|_| Verdict::Pass` fails only because Rust closures must return `Result<Verdict, PluginError>` (`|_| Ok(Verdict::Pass)` compiles) — a closure return-type issue, not a mode-coverage gap. The "30 verbs" figure is stale; live CANS/02 says 24 ring verbs + 8 scopes.

### B6 — host seal() bare text — DONE
- `KelirBuilder::new` now sets `name: ""` ("the host has no name"); `triple_failure(name, failure)` helper makes seal() triple failures bare when the name is empty; plugin-mode failures stay `<plugin>: <triple failure>`.
- One out-of-scope mechanical edit was required: `crates/kelir/tests/fluent_builder_plugin_test.rs` host_bad assertion asserted `text.contains("host")` — it pinned spec-violating text. Updated to assert the bare triple text (`vsock service port 0 out of range 1..=65535`) plus `!text.contains("host")`. Spec wins per AGENTS.md §1; recorded in conflicts.md.
- Verified no other consumer of the host builder's name: bindings use their own builder structs (kelir-core/kelir-interface deps only), `finish_boot` registers only instances/mounted names.

### B7 — Satisfies order-insensitivity — DEFERRED (per order)
- `hlist_sdk.rs` untouched (outside exclusive scope). Decision recorded in conflicts.md.
- Recommendation for the orchestrator: implementable at L2 as pure type-level wiring — keep `Satisfies<NoDeps>`/`Satisfies<HNil>` base impls, replace the head-matching impl with `impl<Plugins: HList, Need, Rest> Satisfies<HCons<Need, Rest>> for Plugins where Plugins: Has<Need> + Satisfies<Rest>` (verify `Has`'s impl reaches arbitrary depth first). No vectors.json / parity-roster change needed.

## Gates (all green)

| Command | Result |
|---|---|
| `cargo check -p kelir --all-targets` | PASS |
| `cargo clippy -p kelir --all-targets -- -D warnings` | PASS |
| `cargo fmt -p kelir` | PASS (no residual diff) |
| `cargo test -p kelir` (extra) | PASS (14 passed, 0 failed; integration tier ignored as at baseline) |
| `cargo check -p kelir-integration --all-targets` (extra — sole reverse dep of kelir) | PASS |
| lib.rs re-export spot-check | `Session`, `SpawnResult`, `VsockHandle`, `VsockChannel`, `VmHandle`… all still exported; `WireHandle` removed (deleted struct); new methods need no re-exports |
| Comment-law greps on `crates/kelir` (extra) | 0 prose, 0 `///`, 0 trailing |

## Files changed (339ca53..HEAD)

```
cans/_collab/conflicts.md                          | +11 (WO3-B section, append-only)
cans/_collab/wo3-report-b.md                       | new (this file)
crates/kelir/src/builder_sdk.rs                    | 22 +/- (B6)
crates/kelir/src/lib.rs                            |  2 +/- (drop WireHandle re-export — compile-required)
crates/kelir/src/session_lifecycle.rs              | 62 +/- (B1–B4)
crates/kelir/tests/fluent_builder_plugin_test.rs   |  4 +/- (B6 mechanical test fix)
```

Out-of-scope edits (recorded, both mechanical/required): `crates/kelir/src/lib.rs` (compile), `crates/kelir/tests/fluent_builder_plugin_test.rs` (spec-mandated text change). No Cargo.toml/Cargo.lock changes. No new dependencies.

## TODO (L3) count

- Before: **519** (`grep -rn "TODO (L3):" crates/ packages/ | wc -l`)
- After: **516** (net −3: B3 deleted 4 occurrences — 2 comments + 2 error-string literals from the removed handle stubs; B1 added 1 mandated marker for the 0x03 wire bytes)

## Commits

- `fb1cd46` WO3-B1..B4,B6: session exec/skew_clock/rollback_here/vsock surface, dead handle deletion, host bare-text seal (+ this report/conflicts commit)

## Needs orchestrator attention

1. `session.checkpoint.create(label)` literal naming (B3): needs `CheckpointPlane::create(label)` in kelir-core (Agent C lane) or an SDK-side shadowing decision; concept is carried by `session.vm().checkpoint(label)` today.
2. Bridge parity gaps (B1/B2): TS `SpawnOptions` has no `limits`; pyo3/napi `exec_in_guest` take no limits; napi/TS lack session-level `skew_clock`.
3. B7 recommendation (order-insensitive `Satisfies` at L2) — needs lane authorization for `hlist_sdk.rs`.
4. Sandbox facade `box.rollback()` (no-arg, CANS/03) still maps to `Sandbox::rollback(lineage)`; a facade-level `rollback_here(pid)` or no-arg variant is L3 (PID tracking), same as the napi stub.
