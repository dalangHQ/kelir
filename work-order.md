# work-order.md — Unbreakable Fluent Plugin-Creation Builder

## Goal

Ship one fluent, terminal-typed plugin-creation builder that behaves identically across Rust, TypeScript, and Python. New plugins are authored through this builder; `impl KelirPlugin` directly remains as the advanced escape hatch and is not removed. The chain is unbreakable in the sense that (a) Rust and TS reject invalid chains at compile time, (b) Python rejects invalid chains at `.build()` with `EConfig`, and (c) `kelir-plugin-cli parity run` blocks any behavioral divergence across languages. L1/L2 only: interfaces, typestates, and wiring — no deep handler logic.

## The Contract

These six clauses are pinned by Agent 1 in `cans/02-plugin-model.md#Fluent-plugin-creation-builder` and `cans/03-sdk-architecture.md#Fluent-plugin-surface`. Agents 2–5 implement them; parity vectors enforce them.

1. **One entry point per SDK.**
   - Rust: `kelir::plugin_builder::PluginBuilder::new(name)`
   - TS: `createPlugin(name)` (existing symbol; surface aligned)
   - Python: `kelir.PluginBuilder(name)` (new)

2. **Chainable verbs. Each verb returns the builder (or a typed successor) and registers exactly one handler or capability. Verb set is closed to the ring traits + lifecycle + vsock service + prerequisite + context + scope + fork, no more:**
   - `.context(fn)` — Rust/TS; Python infers from overrides.
   - `.require::<P>()` / `.requires<P>()` / `.require(P)` — prerequisite.
   - `.scope(MountScope)` / `.scope(scope)` / `.scope(scope)` — default mount scope.
   - `.fork_with(fn)` — custom `fork_state`.
   - `.on_trap`, `.on_process_exit`, `.on_fs_lookup`, `.on_fs_read`, `.on_fs_write`, `.on_fs_lock`, `.on_net_outbound`, `.on_net_inbound`, `.on_console_input`, `.on_console_output`, `.on_exec_stdout`, `.on_exec_stderr`, `.on_host_lsm_exec`, `.on_host_pressure`, `.on_host_keyring_access`, `.on_vm_init`, `.on_vm_ready`, `.on_vm_shutdown`, `.on_pre_commit`, `.on_post_commit`, `.on_host_preempt`, `.on_memory_pressure`, `.on_heartbeat`, `.on_watchdog`, `.vsock_service(spec, handler)`.

3. **Terminal verb `.build()` returns a `KelirPlugin`-conforming wrapper.** The wrapper:
   - Holds `Arc<dyn Trait>` per bound ring.
   - `impl KelirPlugin` such that `register_interceptors` calls `pipeline.bind_<ring>_scoped(arc, scope)` for each bound ring.
   - `fork_state` re-runs the caller's `.fork_with` closure if provided; otherwise `clone_box` of the wrapper (state re-initialized).
   - Post-`.build()` calls are type errors in Rust (self consumed), method-not-found in TS (returns non-builder), `EConfig` in Python.

4. **Compile-time prerequisites.**
   - Rust: `PluginBuilder<Ctx, Deps: HList>`; `build()` returns `PluginWrapper<Ctx, Deps>`; `builder.plugin(w)` bound is `Plugins: Satisfies<P::Deps>`.
   - TS: `.requires<P>()` accumulates a `Deps` union; `.use(plugin)` overload rejects a `Deps` not in caller `Ctx`.
   - Python: `.require(P)` records deps; `builder.boot()` raises `EConfig` listing unsatisfied names.

5. **Fused trampoline descriptor preserved.** The `.build()` wrapper participates in `PipelineEngine` exactly as a hand-written plugin does — same `FusedTrampolineDescriptor` shape, same ordinals. Parity vectors diff these.

6. **Parity vectors extended.** `vectors.json` gains a `plugin_builder` case family. Each case pins: builder name, `requires` list, ring set with ordinals, ordered event payloads, expected Verdict/FilterAction results. `kelir-plugin-cli parity run` executes the Rust, TS, and Python builds of the equivalent chain and diffs outputs.

## Ground Rules (L1/L2 phase)

- L1 = compilable interface only. L2 = wiring + `// TODO (L3):` markers. No handler logic beyond what the existing plugin crates already have.
- Comment law: only `see:` / `TODO:` / `SAFETY:` one-liners. No doc comments. No prose.
- No `#[cfg(test)]` in `src/`. Tests in `tests/`.
- Zero `unwrap`/`expect`/`panic!`/`todo!`/`unimplemented!`/`unreachable!`.
- Zero unsafe outside drivers/transport/shim.
- Per-agent build isolation: `export CARGO_TARGET_DIR=target/agent-<id>`.
- Never push. Commit locally. Convergence is a single squash-merge by the integrator.
- Every file you create or modify must carry a `// see:` line pointing at the clause you satisfy.

## Agent Roster & Scopes

Five agents. Each owns an exclusive file set. Zero overlap.

### Agent 1 — Spec (normative contract)

**Owns:**
- `cans/02-plugin-model.md` — new node `Fluent plugin-creation builder` under `Plugin model`, with sub-nodes matching Contract clauses 1–6.
- `cans/03-sdk-architecture.md` — new node `Fluent plugin surface` under `SDK architecture`, split per SDK (Rust / TS / Python), with `see:` pointers to `cans/artifacts/code/rust-plugin-builder.rs`, `typescript-plugin-builder.ts`, `python-plugin-builder.py`.
- `cans/15-operations.md` — extend `Multi-language parity gate` with a new sub-node `Fluent-builder parity vectors` describing the `plugin_builder` case family, and extend `Coverage-gates` with `--fluent-builder-coverage` (gate spec only; execution lands with Agent 2's codegen).

**Deliverables:**
- Every Contract clause (1–6) has a normative home with a `see:` pointer to its artifact / code.
- Verb table with per-ring signatures pinned at the artifact level, `see:` pointer only from spec.
- `EConfig` / `ESecurityViolation` / compile-error trigger for each prerequisite failure mode, per SDK.
- Post-`.build()` behaviour pinned per SDK.
- Parity vector schema (JSON shape) pinned at artifact level; spec `see:`-points to it.

**Must not touch:** `cans/artifacts/`, `crates/`, `packages/`, `vectors.json`.

**Gates:**
```sh
cans check --refs-only --json | jq -e '.ok and .exitCode == 0'
cans check --json          | jq -e '.ok and .exitCode == 0'
```

**Unblocks:** Agents 2–5 read this as the authority.

---

### Agent 2 — Artifacts & Codegen

**Owns:**
- **New** `cans/artifacts/code/rust-plugin-builder.rs` — illustrative, non-standalone (marked at top). Shows the `PluginBuilder<Ctx, Deps>` surface, verb signatures, `PluginWrapper<Ctx, Deps>`, `Satisfies` bound usage.
- **New** `cans/artifacts/code/typescript-plugin-builder.ts` — illustrative. Aligns `PluginContract` to Contract clauses.
- **New** `cans/artifacts/code/python-plugin-builder.py` — illustrative. `PluginBuilder` class with `@overload`-free chain; `EConfig` triggers pinned.
- **New** `cans/artifacts/code/plugin-builder-vectors.rs` — illustrative. Shape of one `plugin_builder` vector case.
- `cans/artifacts/config/project-structure.txt` — add the three new artifact paths + any new codegen module.
- `crates/kelir-interface/src/bin/codegen/plugin_builder_codegen.rs` — **new**. Emits per-plugin wrapper factories into `packages/ts/core/src/plugins.registry.ts` (`export function <name>(config): PluginBuilder<...>`) and `packages/py/kelir/plugins/plugin_<name>.py` (`def <name>(config) -> PluginBuilder`). Reads `crates/kelir-plugins/*/Cargo.toml` `[package.metadata.kelir.tier]` for name + level, reads `crates/kelir-plugins/*/src/` for peer `require` declarations (from `kelir_interface::plugin_model::PluginDeps` trait impl or a `// require:` marker — pick one, pin it in the spec via a follow-up `see:` to Agent 1, use the other as fallback).
- `crates/kelir-interface/src/bin/codegen/main.rs` — wire the new generator in.
- `crates/kelir-plugin-testkit/src/vectors_testkit.rs` — add `plugin_builder` case family: `PluginBuilderCase { name, requires, rings: Vec<(RingEvent, ordinal)>, events, expected_outcomes }`, extend `VectorSuite::fixtures()` with at least two cases (one trap-only chain, one fs_write+trap chain). Regenerate `vectors.json`.
- `crates/kelir-interface/src/bin/codegen/fusion_codegen.rs` — extend `emit_typescript` and `emit_python` to emit the new `PluginBuilder` surface in `fusion.tables.generated.ts` and `fusion_tables.py` if ordinals require it.

**Deliverables:**
- All three illustrative artifacts present with correct illustrative markers and `see:` back-pointers.
- `plugin_builder_codegen.rs` compiles and produces byte-stable output (`just codegen && git diff --exit-code` clean after commit).
- `vectors.json` regenerated with `plugin_builder` cases; `kelir-plugin-cli parity run` reports the new cases as "not-executable (pending engine)" — the parity gate itself remains green because it flags unexecuted verdicts explicitly (see `parity_cli.rs::assert_vectors`).
- `cargo run --quiet -p kelir-interface --bin codegen` and `cargo run --quiet -p kelir-plugin-testkit --bin vectors` both pass on clean tree.

**Must not touch:** `cans/*.md` (Agent 1's), `crates/kelir/src/*` (Agent 3's), `crates/kelir-interface/src/plugin_model.rs` (Agent 3's), `packages/ts/core/src/plugin.model.ts`, `packages/ts/core/src/builder.lifecycle.ts` (Agent 4's), `packages/py/kelir/model_plugin.py`, `packages/py/kelir/plugin_fusion.py`, `packages/py/kelir/__init__.py` (Agent 5's).

**Gates:**
```sh
cargo check -p kelir-interface --bins
cargo run --quiet -p kelir-interface --bin codegen
cargo run --quiet -p kelir-plugin-testkit --bin vectors
cargo run -p kelir-plugin-cli -- parity run
just codegen && git diff --exit-code
```

**Depends on:** Agent 1's spec clauses (read-only).

---

### Agent 3 — Rust Builder

**Owns:**
- **New** `crates/kelir/src/plugin_builder.rs` — `PluginBuilder<Ctx, Deps: HList>`, `PluginWrapper<Ctx, Deps>`, all `.on_*()` verbs, `.context`, `.require`, `.scope`, `.fork_with`, `.vsock_service`, `.build()`. Each `on_<verb>` wraps a closure in a per-ring `ClosureAdapter<F>` (one adapter struct per ring trait). Adapter trait objects are stored on the builder; `.build()` returns `PluginWrapper` that owns `Arc<dyn Trait>` per bound ring.
- `crates/kelir/src/lib.rs` — `pub mod plugin_builder; pub use plugin_builder::{PluginBuilder, PluginWrapper};`.
- `crates/kelir/src/hlist_sdk.rs` — add `Satisfies<Other: HList>` trait for type-level Deps-vs-Plugins check; impl on `HCons`/`HNil`.
- `crates/kelir/src/builder_sdk.rs` — extend `KelirBuilder::plugin<P>` bound from `P: KelirPlugin + Clone + 'static` to also require `P: PluginDeps where Plugins: Satisfies<P::Deps>`.
- `crates/kelir-interface/src/plugin_model.rs` — add `pub trait PluginDeps { type Deps: HList; }` (or equivalent: `type Deps: Satisfies<...>`); `PluginWrapper<Ctx, Deps>` implements it. Do not change existing ring trait signatures.
- `crates/kelir-plugins/plugin-chaos/src/lib.rs` — **migration proof**. Rewrite to construct via `PluginBuilder` and `builder.plugin(PluginBuilder::new("chaos").context(...).scope(...).on_net_outbound(...).on_fs_write(...).build())`. Keep `ChaosPlugin` (the struct) intact for `ChaosConfig` consumers, but its `KelirPlugin` impl becomes trivial forwarding to the builder's wrapper. Zero behavioral change; `cargo nextest run -p plugin-chaos` must pass unchanged.
- `crates/kelir-integration/src/lib.rs` — add a `// see:` pointer + `// TODO (L3):` marker; no test code, no imports.

**Deliverables:**
- `PluginBuilder` compiles, `.build()` returns `PluginWrapper`, `.plugin()` on `KelirBuilder` rejects a missing dependency at compile time. Verify with a doc-less compile-only assertion in `crates/kelir/tests/fluent_builder_plugin_test.rs` (new test file, compile-only `fn _compiles()` with no assertions — AS-06 anti-tautology does not apply to compile checks).
- `plugin-chaos` migrated, tests green.
- No regressions to `cargo check --workspace`.

**Must not touch:** `cans/`, `crates/kelir-interface/src/bin/codegen/`, `crates/kelir-plugin-testkit/`, `packages/`.

**Gates:**
```sh
cargo check -p kelir -p kelir-interface -p plugin-chaos
cargo clippy -p kelir -p kelir-interface -p plugin-chaos --all-targets -- -D warnings
cargo nextest run -p plugin-chaos
cargo test -p kelir --test fluent_builder_plugin_test
```

**Depends on:** Agent 1's Contract clauses 1, 2, 3, 4 (Rust subsection).

---

### Agent 4 — TypeScript Builder

**Owns:**
- `packages/ts/core/src/plugin.model.ts` — align `PluginContract` to the contract: `.context<Ctx>()` explicit (or keep inference via `createPlugin<Name, Ctx>()`, pin whichever and note in `see:`), `.requires<P>()` (existing), `.scope(scope)`, `.forkWith(fn)` (new), `.build()` (new terminal verb). Currently the contract IS the plugin (implements `KelirPlugin` directly). Add `.build(): KelirPlugin` as terminal; keep the contract-builder's `.build()` returning a frozen `{ name, enabled, config, hooksForFusion, registerInterceptors }` shape that `KelirBuilder.use()` accepts. `.build()` must be called before `.use()` — `.use()` overload accepts `KelirPlugin`, and the builder type no longer satisfies `KelirPlugin` after `.requires()` (only after `.build()`).
- `packages/ts/core/src/builder.lifecycle.ts` — update `.use(plugin)` overload to accept `KelirPlugin` (post-build) only, not the pre-build `PluginContract`. Update existing `plugins.registry.ts` factory stubs to end in `.build()`.
- `packages/ts/core/tests/matrix.fluent-builder.test.ts` — new. Compile-time assertion: `const p: KelirPlugin = createPlugin("x").build();` and `// @ts-expect-error` on `createPlugin("x").requires<"missing">().build()` used with a session that lacks `"missing"`. Runtime assertion: `.build()` returns a frozen object; `.onTrap()` after `.build()` throws `TypeError`.

**Deliverables:**
- `createPlugin(name).context<Ctx>(...).requires<P>().scope(scope).onTrap(...).build()` chain compiles for valid chains, fails compilation for invalid ones.
- `pnpm -r exec tsc --noEmit` green.
- `node --test` over the new test passes.
- No behavioral change to existing registries; codegen output (Agent 2) consumes the new surface unchanged.

**Must not touch:** `cans/`, `crates/`, `packages/py/`, `packages/ts/core/src/plugins.registry.ts` (codegen output, Agent 2 owns it).

**Gates:**
```sh
pnpm -r exec tsc --noEmit
pnpm --filter @kelir/core run typecheck
node --test packages/ts/core/tests/matrix.fluent-builder.test.ts
```

**Depends on:** Agent 1's Contract clauses 1, 2, 3, 4 (TS subsection). Agent 2's regenerated `plugins.registry.ts` for final gate (develop against a locally stubbed shape; rebase before final gate).

---

### Agent 5 — Python Builder

**Owns:**
- **New** `packages/py/kelir/plugin_builder.py` — `PluginBuilder` class mirroring the Rust shape. Methods: `.context(fn)`, `.require(plugin_type_or_name)`, `.scope(scope)`, `.fork_with(fn)`, one `on_<verb>` per ring trait, `.vsock_service(spec, handler)`, `.build()`. `.build()` returns a `_PluginWrapper` that implements the internal `_KelirPluginProtocol` (a `typing.Protocol` for the shape `KelirBuilder.use()` accepts). `.build()` before `.use()`. Post-`.build()` mutation raises `EConfig`.
- `packages/py/kelir/model_plugin.py` — keep `BasePlugin` for backward compatibility (subclass plugins still work); add `def to_builder(self) -> PluginBuilder` so subclass plugins can be converted if needed. Update `hook_descriptors()` to work with `_PluginWrapper` too.
- `packages/py/kelir/plugin_fusion.py` — `FusedTrampoline.fuse()` accepts hooks from a `_PluginWrapper` (post-build) as well as a `BasePlugin`.
- `packages/py/kelir/__init__.py` — `PluginBuilder` exported in `__all__`. `KelirBuilder.use(plugin)` accepts `_PluginWrapper`.
- `packages/py/tests/test_plugin_fluent_builder.py` — new. `PluginBuilder("x").require(LedgerPlugin).on_trap(...).build()` works. `.require` with a name not in the builder's `_plugin_registrations` raises `EConfig` at `builder.boot()`. `.on_trap(...)` after `.build()` raises `EConfig`.
- `packages/py/kelir/plugins/plugin_chaos.py` — migration proof. Rewrite factory to `PluginBuilder("chaos")...build()` (parallel to Rust Agent 3).

**Deliverables:**
- `PluginBuilder` fluent chain works and mirrors Rust/TS behaviour.
- `uv run mypy packages/py` green (no `type: ignore` except where stubs are legitimately incomplete; each carries a `# TODO (L3):` reason).
- `uv run pytest packages/py/tests/test_plugin_fluent_builder.py` green.
- `plugin_chaos.py` migrated; existing tests still pass.

**Must not touch:** `cans/`, `crates/`, `packages/ts/`, `packages/py/kelir/generated/` (codegen output, Agent 2 owns it), `packages/py/kelir/plugins/plugin_*.py` other than `plugin_chaos.py`.

**Gates:**
```sh
uv run mypy packages/py
uv run pytest packages/py/tests/test_plugin_fluent_builder.py
uv run pytest packages/py   # full suite; no regressions
```

**Depends on:** Agent 1's Contract clauses 1, 2, 3, 4 (Python subsection). Agent 2's regenerated `packages/py/kelir/plugins/plugin_*.py` for final gate.

---

## Dependency & Merge Order

```
Agent 1 (spec) —───────┐
                       │
Agent 2 (artifacts) ───┼──► (Agent 3, Agent 4, Agent 5 final gates)
   ↳ depends on Agent 1│
                       │
Agent 3 (Rust) ────────┤
Agent 4 (TS) ──────────┤
Agent 5 (Python) ──────┘
```

- **Wave 1 (parallel):** Agents 1, 2. Agent 3, 4, 5 spawn simultaneously but develop against the spec clauses Agent 1 pins and the shape Agent 2 will produce. If Agent 2's codegen shape diverges from what Agents 3–5 assumed, Agents 3–5 rebase before final gate; no forward-compat shims, no `#[deprecated]` — break cleanly.
- **Merge order (integrator, single squash-merge sequence):** 1 → 2 → 3 → 4 → 5 → convergence commit.
- **No push from agents.** Every agent commits locally. The integrator force-squashes into the target branch after all five are green on the rebased tree.

## Convergence Protocol

After all five land:

1. Integrator rebases all branches onto `main`.
2. Run `just codegen && git diff --exit-code`. If drift: reject the wave, re-open the diverging agent.
3. Run the full Definition of Done (below) on the convergence commit.
4. If any gate fails: locate the owning agent by `git blame`, re-open that agent only, do not re-open the wave.
5. Squash into one commit titled `feat: unbreakable fluent plugin builder (Rust · TS · Python)`.

## Definition of Done

All of the following, on the convergence commit, clean tree:

```sh
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo nextest run --workspace
pnpm -r exec tsc --noEmit
uv run mypy packages/py
uv run pytest packages/py
just codegen && git diff --exit-code
just check-hygiene
just check-spec
just check-manifest
just check-tests
just check-scorecard
cargo run -p kelir-plugin-cli -- check --graph
cargo run -p kelir-plugin-cli -- check --layers
cargo run -p kelir-plugin-cli -- check-plugin-all
cargo run -p kelir-plugin-cli -- parity run
```

Plus, verifiable by inspection:

- `PluginBuilder` present in Rust (`crates/kelir/src/plugin_builder.rs`), TS (`createPlugin` in `packages/ts/core/src/plugin.model.ts`), Python (`packages/py/kelir/plugin_builder.py`).
- All three `.build()` return values accepted by `builder.plugin(p)` / `.use(p)` / `.use(p)` without adaptation.
- Compile-time rejection demonstrated: Rust via a compile-fail test marker (not in DoD — flagged as `// TODO (L3): compile-fail harness`), TS via `// @ts-expect-error` in `matrix.fluent-builder.test.ts`, Python via `EConfig` in `test_plugin_fluent_builder.py`.
- `vectors.json` carries `plugin_builder` cases; `kelir-plugin-cli parity run` reports them (executable or explicitly not-executable pending engine — the report line must mention the new count).
- One plugin per language migrated as proof (`plugin-chaos` in Rust and Python; TS has no plugin crates — migration proof for TS is the `matrix.fluent-builder.test.ts`).
- No `#[cfg(test)]` in `src/`; no `unwrap`/`expect`/`panic!` outside the shim; no comment outside `see:`/`TODO:`/`SAFETY:`.

## Integration note

The integrator owns:
- Rebase and squash-merge.
- Opening `_collab/conflicts.md` if any agent's scope claim overlaps at rebase time.
- Final push gate. No agent pushes.

If a gate fails and the owner cannot be isolated (e.g., a codegen output drift only reproduces after all five merge), the integrator reverts the most recent merge in the sequence, re-opens the wave with the failing constraint pinned to that agent's file set, and repeats. Do not silence a failing gate.