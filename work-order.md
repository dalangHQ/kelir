# work-order.md — Pre-L3: Red Tests First

- **Base:** `scaffolding` @ `9ae0962`, 2026-11-10. Supersedes the Scope 23424 order (EXECUTED, archived at `work-order-23424.md`) and the 615-line verified order (preserved in history at `9ae0962`).
- **Target phase:** Pre-L3 red. L3 starts only when the user says so (AGENTS.md §2).
- **Rule:** Write failing tests for every L3 gap. Write zero L3 meat. A test that cannot fail is not a test (AGENTS.md §5.2); a test that passes without the implementation working is a defect, not a pass (§5.8).

## 0. The gate

Done when the 240 cases read **72 implemented (green) + 168 red + 0 gap**, with default gates green and `just test-red` failing with exactly 168 failures and 0 passes. Until then L3 does not start: without red, L3 has nothing falsifiable to fill, and a green suite proves only that the L2 stubs return `Ok` zero-values.

## 1. Starting state (verified live 2026-11-10)

- 240 cases / 17 domains. Map (`cans/artifacts/test-plan/case-to-test-map.yaml`): **72 implemented / 168 gap**. Gap by tier: unit 34, integration 121, e2e 13.
- **433 `TODO (L3)`** in `crates/`: kelir-core 130, kelir-interface 39, kelir-shim 23, kelir-plugin-cli 21, kelir facade 20, rest spread across drivers / plugins / bindings / transport / otlp.
- Existing tests: 37 Rust test files; last full run 71 passed, 0 failed (`c63e3a3`). Zero failing tests today: the suite is green-by-stub, not green-by-behavior.
- TS/Py: 6 Py + 5 TS test files exist (2 under `packages/ts/core/tests/`, 3 colocated under `packages/ts/core/src/`), but no runner is wired (no `pnpm test` script, no pytest invocation in `justfile`/CI — `justfile:149`).

| Domain | Impl / Total | Gap | | Domain | Impl / Total | Gap |
|---|---|---|---|---|---|---|
| adapters_ingestion (`ad`) | 0 / 12 | 12 | | protocol_mux (`pm`) | 3 / 14 | 11 |
| lifecycle_checkpoint (`lc`) | 3 / 23 | 20 | | attestation_receipts (`ar`) | 4 / 14 | 10 |
| hypervisor_driver (`hd`) | 8 / 24 | 16 | | plugin_engine (`pe`) | 7 / 16 | 9 |
| storage_plane (`sp`) | 7 / 22 | 15 | | terminal_plane (`tp`) | 6 / 15 | 9 |
| virtio_rings (`vr`) | 2 / 15 | 13 | | relational_transaction (`rt`) | 4 / 12 | 8 |
| governance_tools (`gt`) | 3 / 14 | 11 | | wire_plane (`wp`) | 9 / 17 | 8 |
| governance_vault (`gv`) | 5 / 12 | 7 | | telemetry_audit (`ta`) | 5 / 12 | 7 |
| bindings_ts (`bt`) | 0 / 4 | 4 | | supply_chain_security (`sc`) | 5 / 10 | 5 |
| bindings_py (`bp`) | 1 / 4 | 3 | | **Total** | **72 / 240** | **168** |

## 2. Hard rules

1. **Tests only.** No change to `crates/*/src`, `packages/*/src`, or any YAML `expected_observations`. The only src change allowed is the Phase 0 harness (testkit + `justfile`/CI + TS/Py runners). No spec (`cans/`) edit to make a red test disappear.
2. **Location and naming.** Tests live in `tests/` only, never `#[cfg(test)]` in `src/` (AGENTS.md §6). One test fn per `case_id`, fn name == `case_id`, file per the map entry. `check-manifest` fails on any `crates/**/tests/*.rs` file not listed in the map, and on any map entry whose fn is missing — so map and test land in the same commit.
3. **What red is.** Red = compiles, runs, assertion fails on the L3 behavior named by the case. A compile error is not red. A test referencing a surface that does not exist is not red: assert the observable boundary instead (§5).
4. **Red is isolated, not breaking.** Every red test is `#[ignore]`d and runs only via `just test-red`. Default `cargo test` / `cargo nextest run` stays green. A red test that passes is a defect: it flips to green only in L3, with `#[ignore]` removed and map status flipped in the same commit.
5. **Anti-slop applies to red tests too.** AS-01–AS-08 and CDR-01–CDR-08 (`cans/artifacts/test-plan/test-plan-structure.md` §§2–3) bind every new test: no mocking `VmmDriver`/MMU/rings in integration/e2e (AS-01), no naked `.is_ok()`/`.is_err()` — match the typed `Verdict`/`ExitKind` (AS-03), hardcoded literal expectations, never derived from the subject (AS-06), deterministic clock / seeded ChaCha20, no wall-clock sleep (AS-04A), budgets from `perf/budgets.toml` with `enforce = true` (AS-04B), Merkle chain proof where a ledger is active (AS-05), zero dangling FDs/mounts/locks (AS-08), hardware law (AS-07): KVM/HVF/WHP cases run only on real hardware; emulated backends must fail closed with `E_NOHARDWAREVMM` unless `allow_emulated_fallback: true`.
6. **Source of truth per test.** The case YAML + its map `reason` field name the L3 TODO / missing surface. Do not re-derive the expected behavior from memory; do not edit the YAML expectation to match stub output.

## 3. Phase 0 — harness (before any red test)

1. **New map status `red`.** `check-manifest` today accepts only `implemented` / `gap` (`crates/kelir-testkit/src/bin/check_manifest.rs:562`) and errors on anything else — so a red test filed as `implemented` would look done. Extend the match with `red`: requires `test_file` + `test_fn` set and a non-empty `reason` carrying the observed failure signature. `scorecard` reports three counts: implemented / red / gap. State machine is gap → red → implemented, one direction only.
2. **`just test-red` + CI stage.** Nextest: `cargo nextest run --run-ignored ignored-only` (cargo-test equivalent: `cargo test -- --ignored`). CI runs it as an expected-fail stage: it must exit non-zero with exactly N failures and 0 passes, where N is the red count in the map. A red run that passes, errors, or miscounts fails the stage.
3. **Audit the 72 green.** Any implemented test that passes when its subject returns an `Ok` zero-value / `Err(Pending)` stub is a tautology: reclassify it to red and rewrite it under §2.5. The 3 `TODO (L3)` markers already sitting in `crates/kelir-core/tests/` go first.
4. **Wire the TS/Py runners.** `pnpm test` for `packages/ts`, pytest for `packages/py`, both invoked from `justfile`/CI. Until then Wave 4 is blocked, not skipped.

## 4. Phases 1–4 — the waves (168 tests)

Per-test checklist, every wave: read case YAML → write fn (`#[ignore]`, name == `case_id`) → run it → capture the failure → update the map entry (`status: red`, `test_file`/`test_fn` set, `reason:` = observed failure signature, one line) → `check-manifest` + `check-tests` 0 violations. Commit per domain, with the wave's `just test-red` count in the message.

- **Wave 1 — unit gaps, 29.** No hardware, fastest. `pe` 6, `gt` 5, `tp` 4, `pm` 3, `gv` 2, `rt` 2, `vr` 2, `ar` 1, `hd` 1, `sp` 1, `ta` 1, `sc` 1. (Bindings unit gaps live in Wave 4.)
- **Wave 2 — integration gaps, 119.** Core first, then planes/plugins. `lc` 20, `hd` 15, `sp` 14, `vr` 11, `pm` 8, `wp` 8, `ar` 9, `gt` 6, `rt` 6, `ta` 6, `tp` 5, `gv` 4, `sc` 4, `pe` 3. (Bindings integration gaps live in Wave 4.)
- **Wave 3 — e2e gaps, 13.** Last: needs the full path. `ad` 12, `gv` 1.
- **Wave 4 — bindings gaps, 7.** Blocked on Phase 0.4. `bt` 4 (unit 3 + integration 1), `bp` 3 (unit 2 + integration 1). `bp_01`/`bt_01` assert the missing-dependency `EConfig` boot path via the SDK trigger, not via a Rust API.

## 5. Missing surfaces — how to write red without inventing L3

Several gaps have no live surface at all. The test asserts the spec-required observable outcome against the closest live boundary and fails today; it does not reference the missing type, and it does not create it.

- `hd_01`: no typed `NoHardwareVmm` variant in `VmmError` — assert boot on a no-hardware host halts with `E_NOHARDWAREVMM`. Fails today (returns `Unsupported` / wrong path).
- `lc_01`: `Session::fork` is a stub — assert fork yields a distinct incarnation with warm-memory sharing. Fails today.
- `tp_01`: no `HostTerminalGuard` type, spec-only — assert the terminal is restored on guard drop via the live terminal path. Fails today.
- `vr_02`: no doorbell/eventfd surface — assert the MMIO write produces the observable eventfd signal. Fails today.
- `pm_01`: no public mux send path — assert control-channel priority/backlog behavior at the live mux boundary. Fails today.
- Same pattern for every other gap whose map `reason` says "no X exists in-tree": assert the behavior, name the missing surface in the test's `// see:` + map reason, create nothing.

## 6. Verification gate (exit criteria)

```sh
cargo run -p kelir-testkit --bin check-manifest  # 240/240; 72 implemented / 168 red / 0 gap
cargo run -p kelir-testkit --bin check-tests     # 0 violations over 240 cases + all new tests
cargo run -p kelir-testkit --bin scorecard -- check
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo test --workspace            # green: 72 green + 168 ignored, 0 failures
just test-red                     # fails with exactly 168 failures, 0 passes
pnpm -r exec tsc --noEmit; uv run mypy packages/py
just check-spec                   # cans check refs-only + full, 0 broken
```

Plus the red-proof itself: every map `reason` on a `red` entry states the observed failure (assertion + actual value), so the 168 can be re-run and re-counted from the map alone. A wave is not done when its tests exist; it is done when `just test-red` shows its count failing for the recorded reasons and default gates stay green.

## 7. Forbidden

- No L3 implementation anywhere in this order, including "just a small" fill-in of a TODO.
- No `#[ignore]` left on a test that passes; no map status flipped to `implemented` without the green run.
- No editing YAML expectations, budgets, or spec anchors to match stub behavior.
- No mocks, fixtures, or helpers that return the expected value; no expected value computed from the subject (AS-06).
- No new test file without its map entry; no map entry without its failure signature.
