# work-order.md — Unified builder: host builder IS the plugin builder

```markdown
# Work Order: Unify the host builder and the plugin builder (5 parallel agents)

Base: scaffolding @ 9d75fe5 (fluent plugin-builder DX squash, 57 files).
Phase: L1/L2 only. No L3. No handler ever executes; no engine work; no
plugin-chaos Rust migration (cargo cycle stands, `// TODO (L3)` stays).
Verdict this order closes: the comparison 2026-11-11 — host builder and
plugin builder are two builders doing the same job with different machinery.
The ideals, concurred by the user: one builder, one chain; a plugin IS a
builder instance; mounting is merging, not converting; one validation; one
deps system, one scope system, one descriptor table; host-only verbs are just
methods the plugin does not call; the KelirPlugin trait stays underneath;
nothing removed, ~30% less code, every line cut adapter or duplicate
validation, never a capability. Model: ElysiaJS, where a plugin is the same
Elysia instance and `.use()` merges it.

## 0. Frozen decisions (decided, not to be re-litigated)

Decided by the integrator from the comparison; agents execute, they do not
re-decide. Where this order contradicts `cans/` + `cans/artifacts/`, follow
the artifacts and record the decision in the commit message (repo AGENTS.md
§1). Deletion over addition (AGENTS.md rule 11); break cleanly, no
backward-compatibility adapters for the deleted builder types (rule 5.1).

0. **One builder type per SDK. No second builder.**
    - Rust: `KelirBuilder` is the one type. `PluginBuilder`, `PluginWrapper`,
      `plugin_builder_scopes` module, and the duplicated `Satisfies` impls
      (today in both `hlist_sdk.rs` and `plugin_builder.rs`) DELETE.
    - TypeScript: one class `KelirBuilder`. `PluginContract` / `BuiltPlugin`
      types DELETE. `createPlugin(name, options?)` stays as a factory
      function and returns a `KelirBuilder` in plugin mode.
    - Python: one class `KelirBuilder`. `PluginBuilder`, `_PluginWrapper`,
      `_Scope` DELETE (no alias). Factories use `KelirBuilder.plugin(name)`.
    - A1 pins the exact Rust generic arity in spec (§4 A1); A3 implements
      exactly what A1 pins. Constraints on that shape: ONE struct; host
      creation `KelirBuilder::new(VmConfig)`; plugin creation
      `KelirBuilder::named(name)` (the `plugin` name stays with the mount
      method — see §0.2); host-only verbs exist only in Host mode; the 24
      ring verbs + 8 scopes + `context` / `require` / `scope` / `vsock` /
      `fork` exist in BOTH modes. A4/A5 own their file layout within the
      same constraints (one class, one verb definition, ≤1000 LOC/file).

1. **Mount = merge, not convert.** `.plugin()` takes a plugin-mode builder
   BY VALUE and moves its hook registry + vsock list + deps straight into
   the host's `Registration`. No wrapper type, no `build()`-then-mount, no
   `Box<dyn AnyPlugin>` round-trip, no `register_interceptors` in the mount
   path. The mount method name stays `.plugin()` in Rust (it is the landed
   host API); TS/Py keep `.use()` (`.plugin()` alias stays in Python).
   - Rust: `KelirBuilder::named(name)` creates a plugin-mode builder;
     `host.plugin(plugin_builder)` mounts it.
   - TS: `createPlugin(name)` returns a plugin-mode `KelirBuilder`;
     `host.use(plugin_builder)` mounts it.
   - Python: `KelirBuilder.plugin(name)` classmethod creates a plugin-mode
     builder; `host.use(plugin_builder)` mounts it.
2. **One validation, at seal/boot, over host + mounted plugins together.**
   The DX-round triple (a: every verb resolves to a known ring; b: vsock
   spec valid, port 1..=65535, non-empty service; c: scope well-formed) runs
   once, plus (d: host-only verbs rejected on a plugin-mode builder —
   Rust compile-time, TS/Python `EConfig` at mount). Rust: validation in
   `seal()` (today `seal()` validates nothing — that is the gap this order
   closes). TS/Python: validation in `boot()` / `fuse()` entry. Empty host
   still seals/boots. A plugin-mode builder that fails validation fails the
   HOST seal/boot with the same message text, naming the plugin.
3. **One deps system, one scope system, one descriptor table.**
   - One `Satisfies` (Rust), one `requires()` recording (TS), one
     `require()` (Python). Missing-prerequisite message stays uniform:
     `missing prerequisite: <dep> required by <plugin>`, raised at mount
     in TS/Python (Rust compile-time; the mount path must not silently
     accept an unsatisfied plugin).
   - One `MountScope` story: plugin-mode `scope()` sets the plugin's scope;
     mount applies it. `with_composite()` DELETES (its composite-bundle case
     is mounting a plugin-mode builder with a scope). Host `scope()` on a
     plugin-mode builder and host-side scope application are the same
     mechanism, not two.
   - One descriptor table: plugin `_descriptors()` and host
     `fuse().describe()` become the same code path (host descriptors =
     host hooks + mounted plugin hooks, same rings, same ordinals).
4. **Host-only verbs are just methods the plugin does not call.**
   `storage` / `terminal` / `wire` / `lock_broker` / `detached` /
   `fast_boot` / `dax_window` / `seal` / `boot` / `params` live on the one
   type and only the host calls them — Elysia `.listen()`. A plugin-mode
   builder that calls one is an error per §0.2(d), never a silent no-op.
5. **The `KelirPlugin` trait stays underneath.** It is the low-level
   interface, not the authoring surface. The 13 plugin crates keep their
   `impl KelirPlugin` and keep mounting — via a separate
   `.plugin_instance(P: KelirPlugin + PluginDeps + Clone)` method (new name;
   the trait path stays alive without overloading `.plugin()`). The unified
   builder ALSO implements `KelirPlugin` (compat only: code that takes
   `Box<dyn AnyPlugin>` — `fork_state`, composite children — keeps working);
   its `register_interceptors` binds its own registry, but it is NOT the
   mount path. No hand-written plugin changes behavior in this order.
6. **No doc comments ever** (comment law, AGENTS.md §3). No L3, no inline
   tests; tests assert L1/L2 wiring only (merge = descriptor union, mount
   rejects bad plugins, host verbs rejected on plugin-mode builders,
   validation triple at seal/boot, trait plugins still mount).

## 1. Repo rules (binding, read AGENTS.md first)

- L1/L2 only, no L3, no inline tests (tests in `tests/` only, no
  `#[cfg(test)]` in `src/`).
- Comment law (hard): one-liners only, exactly `// see:` / `// TODO:` /
  `// SAFETY:` (Python `#`); `///`, `//!`, prose comments, docstrings
  BANNED. Discoverability via names/signatures/examples, not docs.
- ≤1000 LOC/file (target 500); no `unwrap()/expect()/panic!()/todo!()/
  unimplemented!()/unreachable!()`; `#![forbid(unsafe_code)]` except
  driver/transport/shim; Rust MSRV 1.85.
- Mandatory order: `cans/*.md` → `cans/artifacts/*` → code. Where
  work-order contradicts cans/artifacts, follow artifacts, record the
  decision in the commit message.
- Gates: `cargo check/clippy -D warnings/fmt/test --workspace`,
  `pnpm -r exec tsc --noEmit`, `mypy packages/py` (or
  `python3 -m compileall` if `uv` absent), `just check-hygiene`,
  `just check-spec` (cans 0.21.0), `just check-manifest check-tests
  check-scorecard`, `just codegen`.
- Parallel discipline: one agent per worktree/branch, exclusive scope,
  **no push from agents**; converge by squash-merge into one commit,
  re-gate after; never force-push a divergent head.
- Env notes: `cans` 0.21.0 via `npm install -g cans-spec@0.21.0` if binary
  gone; `pnpm` 9.0.0 via npm; `uv` absent → `python3 -m`;
  `LIBCLANG_PATH=~/.local/lib/python3.12/site-packages/clang/native` for
  testkit gates; per-agent `CARGO_TARGET_DIR`. Default toolchain is 1.85;
  workspace check/clippy/test and `kelir-plugin-cli` need `+stable`
  (1.99.1) — kelir-shim `as_chunks` and `bump_core_cli.rs` let-chain are
  pre-existing 1.85 failures, repo practice is `+stable`.
- Pre-existing baseline at `9d75fe5`: `just check-spec` exit 1 (0 errors,
  1 deliberate warning — profile TBD 11/10, ruling 2026-11-10, do NOT
  resolve); `just check-plugin-all` exit 1 (pre-existing, untouched);
  `just codegen` re-corrupts `uapi_fuse.rs` on this host (drops 6 FUSE_*
  consts — revert, never commit); vectors regen flips
  `trap-git-commit-pass` envp order run-to-run (known defect, never
  silently commit a re-flipping diff). `plugin_builder.rs` 997/1000 LOC,
  `plugin.model.ts` 991/1000 LOC — both DELETE in this order, freeing the
  headroom. TODO (L3) set at base: 538 (must stay identical).

## 2. Ownership (exclusive — do not touch another agent's files)

- **A1 Spec** — `cans/02-plugin-model.md` (#Unified-builder,
  #Fluent-plugin-creation-builder), `cans/03-sdk-architecture.md`
  (#Unified-builder-surface), `cans/15-operations.md`
  (#--unified-builder-coverage, #Unified-builder-parity-vectors). No code.
- **A2 Artifacts/codegen/vectors/example** —
  `cans/artifacts/code/{rust,typescript,python}-plugin-builder.*` (rewrite
  to the unified surface; the three files are renamed in place if the
  artifact set renames — record it), `cans/artifacts/examples/
  fluent-builder-chaos.{rs,ts,py}` (rewrite to the unified shape),
  `crates/kelir-plugin-testkit/src/vectors_testkit.rs`, `vectors.json`,
  codegen inputs for fusion tables + plugin factories.
- **A3 Rust** — `crates/kelir/src/builder_sdk.rs`,
  `crates/kelir/src/plugin_builder.rs` (DELETE),
  `crates/kelir/src/plugin_builder_scopes.rs` (DELETE),
  `crates/kelir/src/lib.rs` (re-exports only),
  `crates/kelir/src/session_lifecycle.rs` (only the `traps: Vec::new()`
  see: pointer if the merge changes it — otherwise untouched),
  `crates/kelir/tests/fluent_builder_plugin_test.rs` (rewrite to the
  unified shape; rename the file ONLY if the test-plan map requires it —
  check `cans/artifacts/test-plan/cases/case-to-test-map.yaml` first).
- **A4 TypeScript** — `packages/ts/core/src/plugin.model.ts` (DELETE or
  become the one class file — A4 decides the layout, records it),
  `packages/ts/core/src/builder.lifecycle.ts` (the one class lives here
  unless A4 records otherwise), `packages/ts/core/tests/
  matrix.fluent-builder.test.ts` (rewrite to the unified shape).
- **A5 Python** — `packages/py/kelir/plugin_builder.py` (DELETE),
  `packages/py/kelir/__init__.py` (KelirBuilder unification only),
  `packages/py/kelir/plugins/plugin_*.py` (factories only),
  `packages/py/tests/test_plugin_fluent_builder.py` (rewrite to the
  unified shape).
- **Untouched by all:** `cans/_rules.yaml`, `AGENTS.md`,
  `crates/kelir-plugins/` (13 crates keep their trait impls; A3's
  `.plugin_instance()` path must not change their behavior),
  `crates/kelir-interface/` (except codegen bins A2 owns),
  `crates/kelir-testkit/src/scenario_catalog.rs`, `justfile`, CI.

## 3. Contracts (frozen — agents implement, do not redesign)

### 3.1 One builder (§0.0)
- Rust: ONE struct `KelirBuilder`. `KelirBuilder::new(VmConfig)` = host;
  `KelirBuilder::named(name)` = plugin mode. The 24 ring verbs + 8 scopes
  + `context` / `require` / `scope` / `vsock` / `fork` exist in BOTH
  modes with the SAME signatures as the DX-round plugin builder (the host
  gains them; the plugin keeps them). Host-only verbs exist only in Host
  mode. `PluginBuilder` / `PluginWrapper` / `plugin_builder_scopes` /
  duplicated `Satisfies` delete; zero references remain in
  `crates/` + `packages/` + `cans/artifacts/code/` (grep gate in §6).
- TypeScript: ONE class `KelirBuilder`. `new KelirBuilder(config)` =
  host; `createPlugin(name, options?)` returns a plugin-mode
  `KelirBuilder`. Same 24 verbs + 8 scopes on both. `PluginContract` /
  `BuiltPlugin` delete; `build()` deletes (a plugin-mode builder mounts
  directly — there is no build step).
- Python: ONE class `KelirBuilder`. `KelirBuilder(...)` with VmConfig
  kwargs = host; `KelirBuilder.plugin(name)` classmethod = plugin mode.
  Same 24 verbs + 8 scopes on both. `PluginBuilder` / `_PluginWrapper` /
  `_Scope` delete; `build()` deletes (mount directly via `.use()`).

### 3.2 Mount = merge (§0.1)
- Rust `host.plugin(plugin_builder)`: moves the plugin's hook registry +
  vsock list + deps into the host's `Registration` (append, in registration
  order; duplicate hooks from two plugins both kept — append semantics,
  per the DX round). Mounting the same plugin-mode builder twice is an
  error (`EConfig`, "plugin already mounted: <name>").
- TS `host.use(plugin_builder)` / Python `host.use(plugin_builder)`:
  same merge, same duplicate rule, same already-mounted error.
- Trait plugins mount via `.plugin_instance(P)` (Rust) /
  `.use(plugin)` (TS/Python — the existing trait-plugin path is unchanged
  there). Behavior of the 13 trait plugins is unchanged; A3/A4/A5 prove it
  with the existing plugin tests, not new ones.
- `fork_with` / `forkState` / `fork_state`: the plugin-mode builder's fork
  fn moves into the host registration on mount; host `fork()` behavior
  unchanged. The three `fork_with` signatures stay pinned as
  language-idiomatic (DX round), NOT a gap.

### 3.3 One validation (§0.2)
- Seal/boot runs the DX-round triple (ring resolution, vsock spec, scope
  shape) over the host AND every mounted plugin, plus (d) host-only verbs
  rejected on plugin-mode builders. Same message text in all SDKs; the
  message names the plugin that failed (`<plugin>: <triple failure>`).
- Rust: validation lives in `seal()`. TS: `boot()` / `fuse()` entry.
  Python: `boot()`. Empty host still seals/boots. A plugin that fails
  validation fails the HOST seal/boot — mounting alone does not validate
  (mount is a merge; validation is a seal/boot concern).

### 3.4 One descriptor table (§0.3)
- Host descriptors = host hooks + mounted plugin hooks, same rings, same
  ordinals (host hooks first, then plugins in mount order; within a plugin,
  registration order). One code path in each SDK; the old plugin-only
  `_descriptors()` / `describe()` split deletes.

### 3.5 Canonical example (§3.6 of the DX round, unified)
- One `chaos` plugin, identical behavior in all three languages, built
  with the UNIFIED shape: Rust `KelirBuilder::named("chaos")`, TS
  `createPlugin("chaos")` (returns `KelirBuilder`), Python
  `KelirBuilder.plugin("chaos")`; requires `ledger`, mount scope
  `/workspace`, `.fs(on_fs_write)` + `.net(on_net_outbound)`, pass/forward
  handlers. Lives as `cans/artifacts/examples/fluent-builder-chaos.
  {rs,ts,py}` (rewritten, not new files) and is the copy-paste starting
  point in each SDK's test file.

### 3.6 Vectors
- `vectors.json` 50 → **58**: 8 new `plugin-merge-<scope>` cases (one per
  scope: mount a plugin-mode builder built with that scope's verbs, assert
  the host descriptors include them with the plugin's scope). Descriptors
  only; parity run reports not-executable per `cans/15` (L1/L2). The 50
  existing cases stay byte-identical except the plugin-builder cases whose
  construction changes to the unified shape (record the rename in the
  commit if case names change — prefer keeping names).

## 4. Per-agent task lists

### A1 — Spec (UB-1)
- Pin the Rust generic arity in `cans/02` #Unified-builder (ONE struct,
  Mode-gated host-only verbs, `named()` creation, `.plugin()` merge,
  `.plugin_instance()` trait path) — this is the contract A3 implements;
  A2/A3/A4/A5 build against it as written, they do not wait for A1 but
  they do not redesign it either: if A1's pinned shape contradicts §3,
  §3 wins and A1 records the contradiction.
- Update `cans/02` #Fluent-plugin-creation-builder to point at
  #Unified-builder (the fluent builder IS the unified builder now; the
  old node stays as history, not as a second surface).
- Per-SDK surface table in `cans/03` #Unified-builder-surface: one builder
  per SDK, creation forms, mount forms, host-only verb list, validation
  point (seal vs boot), error forms (Rust `Result`, TS/Python `EConfig`).
- Update `cans/15` #--unified-builder-coverage +
  #Unified-builder-parity-vectors: 30 verbs + 8 scopes on host AND plugin
  mode, 8 merge cases, one vector case per verb + one per scope + one per
  merge scope (58 total).
- Gate text must match what A2 lands. `cans check` refs-only + full: 0
  errors (the deliberate profile TBD 11/10 warning stays). Anchors regen
  clean. DoD: spec only, no code.

### A2 — Artifacts / codegen / vectors / example (UB-2)
- Rewrite the three plugin-builder artifacts to the unified surface
  (one builder per SDK; creation + mount + host-only list; the old
  PluginBuilder/PluginContract shapes are gone, not documented as
  alternatives).
- Codegen: TS factories emit `createPlugin(name, { config })` returning
  `KelirBuilder`; Py factories emit `KelirBuilder.plugin(name)` + config
  mapping (`.context(lambda: cfg)` — the DX-round emitter fix stays);
  Rust factories (if any) emit `KelirBuilder::named(name)`. The factory
  template is the SSOT for the generated files — hand edits to generated
  factories are wiped by regen by design; if a factory needs a hand case,
  fix the template, never the file.
- Vectors 50 → 58 (§3.6) via the real path (`kelir-plugin-cli vectors`,
  `+stable` — the `cargo run -p kelir-plugin-testkit --bin vectors` path
  does not exist). `just codegen` clean + byte-stable on re-run except the
  known `uapi_fuse.rs` host corruption (revert, never commit) and the
  known `trap-git-commit-pass` envp flip (record, never silently commit a
  re-flipping diff).
- Rewrite the canonical chaos example to the unified shape (§3.5); verify
  placement live for `.rs` / `.py` files and record it.
- DoD: `just codegen` clean + byte-stable (modulo the two known defects),
  `just check-manifest check-tests check-scorecard` green, vectors load via
  the testkit, `cans check --refs-only` 0 errors.

### A3 — Rust (UB-3)
- Delete `plugin_builder.rs` + `plugin_builder_scopes.rs`; extend
  `builder_sdk.rs` with the 24 verbs + 8 scopes + `context` / `require` /
  `scope` / `vsock` / `fork` (same signatures as the DX-round plugin
  builder); `KelirBuilder::named(name)`; `.plugin()` merge;
  `.plugin_instance(P)` trait path; validation in `seal()` (§3.3).
- `lib.rs` re-exports only. `PluginBuilder` / `PluginWrapper` /
  `plugin_builder_scopes` / duplicated `Satisfies` — zero references
  remain in `crates/` (grep gate).
- The unified builder implements `KelirPlugin` (compat only, §0.5); its
  `register_interceptors` binds its own registry. The 13 plugin crates are
  untouched; prove their mount path with the existing plugin tests.
- Tests: rewrite `fluent_builder_plugin_test.rs` to the unified shape
  (host verbs, plugin-mode verbs, merge = descriptor union, mount rejects
  already-mounted + host-only verbs on plugin-mode builders, seal
  validation errors naming the plugin, trait plugin still mounts,
  canonical §3.5 chaos shape). No execution, no engine, no L3.
- DoD: `cargo check/clippy -D warnings/fmt/test` (workspace, `+stable`
  where the repo needs it) green; `just check-hygiene` green.

### A4 — TypeScript (UB-4)
- One class `KelirBuilder`: `new KelirBuilder(config)` = host,
  `createPlugin(name, options?)` returns a plugin-mode `KelirBuilder`.
  Same 24 verbs + 8 scopes on both; `.use()` merges (runtime
  already-mounted + host-only-verb rejection, `EConfig`); `build()`
  DELETES (mount directly); `PluginContract` / `BuiltPlugin` delete.
  `derive()` / `context()` stored on the builder (same as the DX round);
  `requires()` records deps at runtime; missing-prerequisite message
  uniform (§0.3).
- Host gains the 24 verbs: the host's existing `trap()` / `on()` /
  `onTrapDeny()` stay as the host's trap surface; the 24 ring verbs are
  the same methods the plugin had (same names, same handler signatures —
  verify each against the live Rust verb types, as in the DX round).
- Tests: rewrite `matrix.fluent-builder.test.ts` to the unified shape
  (same assertions as A3, TS form; canonical §3.5 chaos shape).
- DoD: `pnpm -r exec tsc --noEmit` green; `pnpm -r test` green;
  `just check-hygiene` green.

### A5 — Python (UB-5)
- One class `KelirBuilder`: `KelirBuilder(...)` with VmConfig kwargs =
  host; `KelirBuilder.plugin(name)` classmethod = plugin mode. Same 24
  verbs + 8 scopes on both; `.use()` merges (already-mounted +
  host-only-verb rejection, `EConfig` at mount); `build()` DELETES
  (mount directly; the `_check_open` built-flag machinery deletes with
  it — mount-time validation replaces it). `PluginBuilder` /
  `_PluginWrapper` / `_Scope` delete (no alias). `require()` keeps
  dedupe-by-name; missing-prerequisite message uniform, raised at mount
  (Python moves from boot-time to mount-time — record the change).
  Arity check stays (registration-time, `inspect.signature` → `EConfig`).
  `RingEvent` + `HOOK_RINGS` untouched (already complete from the DX
  round); `_descriptors()` becomes the one descriptor table (§3.4).
- Factories: all 13 `*_builder()` use `KelirBuilder.plugin(name)` +
  `.context(lambda: cfg)` (A2 owns the generator; handoff via
  `_collab/handoffs.md` if the template needs a change).
- Tests: rewrite `test_plugin_fluent_builder.py` to the unified shape
  (same assertions as A3, Python form; canonical §3.5 chaos shape).
- DoD: full Py suite no worse than baseline (101 passed, 4 skipped at
  `9d75fe5`); `mypy packages/py` no NEW errors; `python3 -m compileall`
  green; `just check-hygiene` green.

## 5. Merge order and push

- A1 → A2 → A3/A4/A5 (rebase onto 1+2). Integrator converges, re-gates,
  squash-merges into `scaffolding` as a single commit, verifies tree
  byte-identical to the converge tip.
- **Push only when the user supplies a token in that turn** (AGENTS.md
  §0). No token in the turn: commit locally and stop; say so once. Token
  use is transient only, never stored; restore the remote to plain HTTPS
  after; rotation stays advised until confirmed.

## 6. DoD gates (all green on the merged tree, or the work is not done)

- `cargo check --workspace --all-targets` (+stable), `cargo clippy
  --workspace --all-targets -- -D warnings` (+stable), `cargo fmt --all
  -- --check`, `cargo test --workspace` (+stable; 124 passed / 0 failed /
  100 ignored at base — no worse), `pnpm -r exec tsc --noEmit`,
  `pnpm -r test`, `pytest packages/py/` (101 passed / 4 skipped at base —
  no worse), `mypy packages/py` (0 issues at base — no new errors),
  `just check-hygiene`, `just check-manifest check-tests check-scorecard`,
  `cans check --refs-only` 0 errors, `cans check` 0 errors + the 1
  deliberate profile TBD warning, `just codegen` byte-stable (modulo the
  two known defects), `just check-scenarios` OK, parity run 58/58
  frame-validated 0/58 executed (not-executable, L1/L2).
- New grep gates (run on the merged tree, must print nothing):
  `grep -rn "PluginBuilder\|PluginWrapper" crates/ packages/ --include=
  '*.rs' --include='*.ts' --include='*.py'` (except the test-plan map if
  it must name the old test file, and `_collab/` history);
  `grep -rn "PluginContract\|BuiltPlugin" packages/ts/`;
  `grep -rn "plugin_builder" crates/kelir/src/ packages/py/kelir/`.
- TODO (L3) set identical to base (538). Comment law greps clean.
  `plugin_builder.rs` (997 LOC) and `plugin.model.ts` (991 LOC) delete —
  the LOC pressure they carried goes with them.
```
