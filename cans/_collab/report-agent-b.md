# agent-b report — validation tooling dedup (plugin-cli / testkits)

Branch `temp/agent-b`, worktree `/home/z/my-project/kelir-b`. All six baseline gates plus check/clippy/fmt/test were green before edits and remain green after (see Gates).

## Environment note (affects all cargo gates)

`driver-rustvmm` depends on `userfaultfd-sys`, whose bindgen build script requires libclang; the sandbox has none (no root, no system clang). Every `cargo run` gate fails to BUILD at exit 101 without a workaround. Workaround used (local-only, not committed): downloaded Debian trixie `libclang1-19`/`libclang-cpp19`/`libllvm19` .debs into /tmp/libclang-pkgs and exported `LIBCLANG_PATH=/tmp/libclang-pkgs/extract/usr/lib/x86_64-linux-gnu`. Merge-lead must use the same workaround (or a host with libclang) when re-gating.

## Fix 4 — one vector parser (DONE)

- Verified: parity_cli.rs had `ParityVector`/`ParityWatchdog`/`ParityVeto` + ~300 lines of manual serde_json::Value walking; `expected_verdict_kind`/`event_kind` were only used for a heartbeat reason string (replaced by typed `GuestToHost::Heartbeat` match).
- Parser location decision: **kelir-plugin-testkit** — kelir-plugin-cli ALREADY depends on it (Cargo.toml line 11) and layers_cli RULES only scan crates/kelir-core/src, crates/kelir-plugins, crates/kelir-drivers, so `check --layers` stays green. No new dependency introduced for this fix.
- New `crates/kelir-plugin-testkit/src/vectors_parse.rs` (306 LOC; vectors_testkit.rs was 923 LOC so a same-file impl would break the 1000 cap): `VectorCase::from_json(&Value)` + `VectorSuite::from_json_text(&str)`, typed frame (WireFrame::decode of frame_hex + declared-field cross-checks), typed event/Verdict/WatchdogCase/VetoCase/UnifiedBuilderCase (TrampolineRing::from_tag rings, Verdict|Filter outcomes). `VectorError::Parse(String)` added. plugin_builder entries are now strictly parsed (work-order-2 §3.2 fix).
- parity_cli.rs rewritten to consume typed cases: assert_frame_shape/assert_vectors/assert_canonical_roster operate on `&VectorCase`; dead fields deleted with ParityVector; `parity run` output byte-identical in shape (58 vectors, 2 watchdog, 3 veto).
- Commit 0117a04.

## Fix 5 — verb roster seal→build (DONE)

- Verified: `plugin_builder_verb_cases` structural list had `("seal", ...)`; cans/03-sdk-architecture.md §Fluent-plugin-surface counts `build` among the 30 structural+ring verbs and §Unified-builder-surface lists seal() as host-only. Swapped `("seal", Vec::new(), Vec::new())` → `("build", Vec::new(), Vec::new())`.
- vectors.json: regenerated with the existing generator (`cargo run -p kelir-plugin-cli -- vectors`), NOT hand-edited (Hermetic Commit Invariant). Diff verified surgical: exactly one vector `plugin-builder-seal-only` → `plugin-builder-build-only` (name + plugin_builder), no envp churn this run. `parity run` roster check green.
- Commit 1d4f041.

## Fix 6 — one TOML parser per artifact (DONE)

- graph_cli.rs: line-prefix scan replaced by typed `PluginManifestToml` (package.name + dependencies + dev-dependencies as BTreeMap<String, DepSpec>, untagged `DepSpec::{Table{path}, Version}`; `#[expect(dead_code)]` on Version payload — serde(other) is invalid in untagged enums). Covers every section the old line-scan covered (verified: plugins have no [target.*] tables). Behavior hardening: malformed plugin TOML now fails closed instead of being silently skipped from the graph.
- fixtures_cli.rs: manual split('=') + strip_quotes replaced by typed `FixturesFile { fixtures: BTreeMap<String, FixtureToml> }`. Entry order is now name-sorted (was file order) — no consumer depends on order; gt_14 CLI test passes unchanged.
- budgets: `REQUIRED_KEYS` ad-hoc enforcement DELETED; typed `Budgets`/`ScorecardTables`/`ScorecardFloor`/`ScorecardRule` + `validate_scorecard_budgets` moved to new `crates/kelir-testkit/src/perf_budgets.rs` (module renamed from budgets.rs after check-tests NG-06 filename-grammar gate rejected the single-token name); scorecard_testkit.rs bin and budgets_cli.rs both consume it — one parser, one validation. budgets_cli keeps: read error text, `enforce` must be true, silent success. Layering note: kelir-plugin-cli now depends on kelir-testkit (no gate forbids it; audit_cli's plugin-tier rules only govern crates/kelir-plugins members; no cycle — kelir-testkit does not depend on the CLI).
- Commit 2e7d753.

## Also (smaller items)

- CaseMap dedup (DONE): audit claim stale — check_tests.rs had NO CaseMap struct, only a manual harness_tests line-scan. Added `crates/kelir-testkit/src/case_map.rs` (CaseMap/MapEntry/HarnessTest), check_manifest.rs imports it, check_tests.rs parses it typed (graceful-empty on unreadable/unparseable map, same as old scanner tolerance; TODO L3 comment for fail-closed). Commit 2e7d753.
- bump_core_cli::resolve_pin (DONE): no longer `env!("CARGO_PKG_VERSION")` (CLI's own version); now typed-reads `[package].version` from crates/kelir-core/Cargo.toml at run time (repo-root CWD, matching the CLI's other CWD-relative subcommands). Signature now `Result<ChannelPin, BumpCoreError>`. Verified: `bump-core --channel released` prints "pinned channel released at kelir 1.0.0" (kelir-core's real version). Logged handoff to agent-a for a compile-time KELIR_CORE_VERSION re-export. Commit 6bf63d4.
- check subcommand duplicates (DONE): top-level `check-layers`/`check-budgets` deleted from main.rs match (verified they existed); the justfile flag form `check --layers` / `check --budgets perf/budgets.toml` survives (verified exit 0). cans/15 §Subcommands never listed the aliases → no spec drift. Commit 6bf63d4.
- Silent Ok(()) stubs (DONE): logs/manifest/registry/rings/scaffold/attest now return explicit one-line `Err("... (L3)")` in the crate's existing Box<dyn Error> TODO style; each retains its one-line `// TODO (L3):` comment; main.rs TODO comment updated. Verified: all six subcommands exit 1 with the message; attest still validates flags first (MissingSign path preserved). Commit 6bf63d4.
- Comment law (DONE): pre-existing 6-line prose comment on plugin_builder_scope_cases in vectors_testkit.rs replaced with a `// see: cans/03-sdk-architecture.md#Unified-builder-surface` ref (anchor verified in anchors.json). Commit 4e395b5.

## Gates

| Gate | Baseline | After |
|---|---|---|
| check-manifest | 0 | 0 |
| check-tests | 0 | 0 (115→118 conforming src modules) |
| check-scenarios | 0 | 0 |
| scorecard -- check | 0 | 0 |
| plugin-cli check --layers | 0 | 0 |
| plugin-cli check --graph | 0 | 0 |
| plugin-cli check --budgets perf/budgets.toml | 0 | 0 |
| plugin-cli parity run (bonus, pure-Rust; NOT the justfile parity) | 0 | 0 |
| cargo check 3 crates --all-targets | 0 | 0 |
| clippy -D warnings 3 crates --all-targets | 0 | 0 |
| cargo fmt --all -- --check | 0 | 0 |
| cargo test -p kelir-plugin-cli -p kelir-plugin-testkit | 0 | 0 (2+4 pass, 1 ignored, unchanged) |
| Comment-law greps (my 3 crates) | 1 pre-existing prose block in vectors_testkit.rs | 0 prose / 0 doc comments |

Not run (per instructions): `parity run` via the justfile (build-ts), `fixtures --fetch`. Interim false-red caught and fixed during the run: check-tests failed (exit 1) with `budgets.rs` violating the NG-06 source-module filename grammar until the module was renamed `perf_budgets.rs`.

## Skipped / deferred

- vectors_testkit envp CBOR ordering byte-stability: pre-existing TODO (L3) — regeneration can churn `trap-git-commit-pass` frame hex ~50% of runs; not addressed (L3).
- Repo-wide Box<dyn Error> refactor in cli subcommands: explicitly out of scope this round.
- kelir-host-probe crate extraction (work-order smaller note): not in this round's task list; untouched.
- gt_13/gt_14-style test coverage for the new from_json parser: tests live only in tests/ dirs and adding a new test-plan case requires cans/artifacts SSOT updates owned by merge-lead; parser is exercised by `parity run` + the vectors round-trip instead.

Commits (all pushed to origin/temp/agent-b): 0117a04, 1d4f041, 2e7d753, 6bf63d4, 4e395b5 + this report commit.
