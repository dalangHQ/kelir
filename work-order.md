# work-order.md — Fluent plugin-builder DX: nested cohesiveness + tri-SDK parity

```markdown
# Work Order: Ideal DX for the fluent plugin-creation builder (5 parallel agents)

Base: scaffolding @ 340b48d (23b962a + WO2 scenario tier + CANS L1/L2 alignment).
Phase: L1/L2 only. No L3. No handler ever executes; no engine work; no
plugin-chaos Rust migration (cargo cycle stands, `// TODO (L3)` stays).
Verdict this order closes: DX audit 2026-11-11 — good core, NOT ideal/amazing.
Flat chain everywhere, no nested cohesiveness, TS 12/24 verbs, Python 6 verbs
raise at `.build()`, parity surface-level only. This order fixes exactly that.

## 0. Frozen decisions (decided, not to be re-litigated)

Decided by the integrator from the audit; agents execute, they do not re-decide.
Where this order contradicts `cans/` + `cans/artifacts/`, follow the artifacts
and record the decision in the commit message (repo AGENTS.md §1).

1. **Nested shape: closure-scoped sub-builders, additive.** Flat verbs stay
   (they are the landed API; removing them breaks tests/vectors). Each SDK
   gains 8 scope entry points taking a closure that receives a scope builder
   and returns it; the parent chain resumes after the closure:
   `trap` (on_trap, on_process_exit), `fs` (on_fs_lookup/read/write/lock),
   `net` (on_net_outbound/inbound), `console` (on_console_input/output),
   `exec` (on_exec_stdout/stderr), `host` (on_host_lsm_exec,
   on_host_pressure, on_host_keyring_access, on_host_preempt),
   `lifecycle` (on_vm_init/ready/shutdown, on_pre_commit, on_post_commit),
   `telemetry` (on_memory_pressure, on_heartbeat, on_watchdog).
   Scope methods carry the SAME verb names as the flat chain (`on_trap`,
   etc.). A nested chain and the equivalent flat chain MUST produce
   identical descriptors (same rings, same ordinals) — tested in every SDK.
   The existing `.scope(MountScope)` verb is untouched; scope entry points
   are named by ring group (`trap`/`fs`/`net`/...) so there is no clash.
2. **Attach verb stays per-language.** Rust `.plugin()`, TS/Python `.use()`
   (+ Python `.plugin()` alias). Rust `use` is a keyword; renaming is not
   the fix. The fix is uniform enforcement reporting: a missing prerequisite
   is reported in every SDK as `missing prerequisite: <dep> required by
   <plugin>` — Rust at compile time (Satisfies), TS at `.use()` (type +
   runtime), Python at `boot()` (backstop, unchanged point) with the same
   message text.
3. **Repeat semantics: append, everywhere.** Rust/TS already append; Python
   changes its per-verb dict to an ordered list. A second `.on_trap`
   fires after the first; it never silently replaces it.
4. **Vsock: multi, everywhere, plugin-level.** Python single slot becomes a
   list; TS gains plugin-level `vsock_service` (sandbox-level
   `registerVsockService` stays). Same spec `(port, service, schema)`.
5. **`.build()` validates the same three things in all SDKs** and fails with
   the same message text: (a) every registered verb resolves to a known ring
   (backstop for future drift), (b) vsock spec valid (port 1..=65535,
   non-empty service), (c) scope value well-formed. Error type per language:
   Rust `PluginError` (build becomes `-> Result<PluginWrapper, PluginError>`),
   TS/Python `EConfig`. An empty plugin (no handlers) still builds.
6. **No doc comments, ever.** Repo comment law (AGENTS.md §3): one-liner
   `// see:` / `// TODO:` / `// SAFETY:` only; `///`, `//!`, prose comments,
   Python docstrings are banned. Discoverability comes from names,
   signatures, and the canonical example — not from prose in code.
7. **No L3, no inline tests.** Tests live in `tests/` only, never
   `#[cfg(test)]` in `src/`. Tests assert L1/L2 wiring only (descriptors,
   ordinals, validation errors, freeze/built-flag behaviour) — never
   handler execution.

## 1. Hard rules (all agents)

1. **Exclusive ownership.** Edit only files in your "Owns" section. A needed
   edit elsewhere goes through `_collab/handoffs.md` or a frozen contract
   in §3. Do not touch another agent's files to make your gates green.
2. **Spec → artifacts → code, in that order.** A change updating only one
   layer is incomplete. Artifacts own field layouts, schemas, signatures.
3. **Verify every audit claim against the live tree before editing.**
   Audits re-raise fixed items with stale line numbers. The audit's TS
   "zero examples" claim was wrong (`cans/artifacts/examples/hook-matrix.ts`
   and `policy-matrix.ts` use `createPlugin`); the gap is that no ONE
   example plugin is identical across all three languages.
4. **Never loosen a spec gate to ease code.** If the only green path edits
   `cans/`, the fix belongs upstream / in `_collab/conflicts.md`, not in a
   push. Never edit `cans/_rules.yaml` thresholds.
5. **Scaffolding constraints:** ≤1000 LOC/file (target 500); no
   `unwrap()` / `expect()` / `panic!` / `todo!()` / `unimplemented!()` /
   `unreachable!()`; `#![forbid(unsafe_code)]` except driver/transport/shim
   enclaves; Rust MSRV 1.85.
6. **No push from any agent.** Agents commit on their own branch only.
   Convergence (squash-merge + re-gate + push) is integrator work.
7. **Commit message** records the work-order clause (e.g. `DX-2`, `DX-5`)
   and, where a decision from §0 was applied against a live-tree
   contradiction, the decision number.

## 2. Ownership matrix

| Agent | Owns (code) | Owns (spec/artifacts) |
|---|---|---|
| **A1 — Spec** | none | `cans/02-plugin-model.md` (#Fluent-plugin-creation-builder), `cans/03-sdk-architecture.md` (#Fluent-plugin-surface), `cans/15-operations.md` (#--fluent-builder-coverage, #Fluent-builder-parity-vectors) |
| **A2 — Artifacts, codegen, vectors, canonical example** | `crates/kelir-plugin-testkit/src/vectors_testkit.rs`, `vectors.json`, codegen inputs for fusion tables + plugin factories (verify live paths first) | `cans/artifacts/code/{rust,typescript,python}-plugin-builder.{rs,ts,py}`, `cans/artifacts/examples/fluent-builder-chaos.{rs,ts,py}` (new, one plugin, three files) |
| **A3 — Rust** | `crates/kelir/src/plugin_builder.rs`, `crates/kelir/src/builder_sdk.rs` (attach + scope plumbing only), `crates/kelir/src/lib.rs` (re-exports only), `crates/kelir/tests/fluent_builder_plugin_test.rs` | none |
| **A4 — TypeScript** | `packages/ts/core/src/plugin.model.ts`, `packages/ts/core/src/builder.lifecycle.ts` (`.use()` runtime check only), `packages/ts/core/tests/matrix.fluent-builder.test.ts` | none |
| **A5 — Python** | `packages/py/kelir/plugin_builder.py`, `packages/py/kelir/generated/fusion_tables.py` (+ its generator, if generated — verify live), `packages/py/kelir/plugins/plugin_*.py` (factories only), `packages/py/kelir/__init__.py` (`.use()`/`.plugin()` message text only), `packages/py/tests/test_plugin_fluent_builder.py` | none |

**Untouched by all:** `cans/_rules.yaml`, `AGENTS.md`, `crates/kelir-plugins/`
(Rust chaos migration stays blocked on the cargo cycle), `justfile`, CI.

## 3. Frozen cross-agent contracts

Signatures below are the interface. Do not change without a
`_collab/conflicts.md` entry. Verify field/type names against the live tree;
if a name here contradicts the tree, the tree wins and the agent records it.

### 3.1 Nested scopes (all SDKs, same 8 groups, same verb names)

```rust
// Rust — scope structs collect into the parent's Vecs, in closure order.
PluginBuilder::new("x")
    .trap(|t| t.on_trap(|_, _| Ok(Verdict::Pass)))
    .fs(|f| f.on_fs_write(|_, _, _, _| Ok(FilterAction::Forward)))
    .build() // -> Result<PluginWrapper, PluginError>
```

```ts
// TS — scope interfaces, same shape; build() throws EConfig on DX-5 failure.
createPlugin("x")
    .trap((t) => t.onTrap((e, c) => c.pass()))
    .fs((f) => f.onFsWrite((p, b, pid, c) => c.pass()))
    .build()
```

```python
# Python — scope classes, same shape; build() raises EConfig on DX-5 failure.
PluginBuilder("x") \
    .trap(lambda t: t.on_trap(lambda e, c: Verdict.PASS)) \
    .fs(lambda f: f.on_fs_write(lambda p, b, pid, c: FilterAction.FORWARD)) \
    .build()
```

Scope entry points take a closure/lambda receiving the scope builder and
returning it. No `.end()` / `.done()` verb. Nested == flat in descriptors.

### 3.2 Closed verb set (30: 6 structural + 24 ring)

`context`, `require(s)`, `scope`, `fork_with`/`forkWith`, `vsock_service`,
`build` + the 24 ring verbs from `cans/02-plugin-model.md:398`. Every SDK
exposes all 30 by the end of this order. `fork_with` keeps its per-language
signature (Rust `Fn(&str) -> Box<dyn AnyPlugin>`, TS `() -> PluginContract`,
Python `Fn(_PluginWrapper) -> _PluginWrapper`); the three signatures are
pinned in spec as language-idiomatic, not a gap.

### 3.3 Canonical example plugin (A2 owns, A3/A4/A5 mirror in tests)

One plugin, identical behaviour in all three languages, named `chaos`:
requires `ledger`, mount scope restricted to `/workspace` (MountScope fields
as live in-tree), `.fs(|f| f.on_fs_write(...))` + `.net(|n| n.on_net_outbound(...))`,
handlers return the pass/forward policy (L1/L2: descriptors only, no
execution). Lives as `cans/artifacts/examples/fluent-builder-chaos.{rs,ts,py}`
and is the copy-paste starting point in each SDK's test file. The three
existing divergent "chaos" shapes (Rust test chain, TS artifact
`buildChaosPlugin`, Python `plugin_chaos.py`) are replaced by this one shape.

### 3.4 Vector cases (A2 owns)

`vectors.json` 14 → 50: the 2 existing `plugin-builder-*` cases stay; add
28 `plugin-builder-<verb>-only` cases (one per verb not yet covered) and 8
`plugin-builder-nested-<scope>` cases (one per §3.1 scope, asserting
nested == flat descriptors). Cases are descriptors (L1/L2); `parity run`
reports them not-executable per `cans/15-operations.md` — that is the
specified outcome, not a failure.

## 4. Per-agent task lists

### A1 — Spec: pin the nested shape + the gates

- DX-1: `cans/02-plugin-model.md#Fluent-plugin-creation-builder` — add the
  §3.1 nested-scope shape (8 groups, closure entry points, nested == flat
  descriptors), the §0.5 build-validation triple, append repeat semantics,
  multi-vsock, and the per-language attach-verb + enforcement table (§0.2).
  Point at artifacts with bare `see:` refs; do not re-list artifact content.
- DX-1: `cans/03-sdk-architecture.md#Fluent-plugin-surface` — per-SDK
  surface table for all 30 verbs + 8 scopes; note Rust `build()` returns
  `Result`, TS/Python throw/raise `EConfig`; note `fork_with`'s three
  signatures as pinned, not divergent.
- DX-1: `cans/15-operations.md` — `--fluent-builder-coverage` covers 30
  verbs + 8 scopes; `#Fluent-builder-parity-vectors` asserts one vector
  case per verb + one per scope (38 cases, §3.4). Gate text must match what
  A2 lands; if the current "every verb has a case" text already says it,
  keep it and cite the case count.
- **DoD:** `cans check --refs-only` + full `cans check` green (A2's
  artifacts may be stubbed temporarily to prove refs; A1's gates read red
  only on missing artifact files, never on spec errors). Anchors regen
  clean, no diff beyond the new nodes.

### A2 — Artifacts, codegen, vectors, canonical example

- DX-2: rewrite the three plugin-builder artifacts to the full §3.2 surface:
  fix the TS artifact's false `.onFsLock / .onNetInbound / .onVmInit`
  claims against live TS (they do not exist until A4 lands them — artifact
  shows the target shape, marked as the A4 contract); fix the Py artifact's
  3-verb sketch to all 30 verbs; fix the Rust artifact's `fork_with`
  signature drift (`Fn(&Ctx) -> Ctx` in the artifact vs live
  `Fn(&str) -> Box<dyn AnyPlugin>` — artifact follows the live code).
- DX-2: codegen — fix the generator(s) so regen emits the §3.2 surface:
  TS `PLUGIN_BUILDER_VERBS` / `HOOK_RINGS` must match the live builder
  (today they claim `onVmInit` the class lacks); Python `PLUGIN_BUILDER_VERBS`
  (15 verbs, never consulted) is either wired into `plugin_builder.py` or
  deleted — decide from the tree, do not leave a dead table beside a live
  one. Factories: codegen emits `.requires()` + config mapping for all 13
  plugins (no more empty `PluginBuilder("<name>").build()` shells).
- DX-2: `vectors.json` + `vectors_testkit.rs` — add the 36 cases from §3.4
  (28 verb + 8 nested). Regen path is `kelir-plugin-cli vectors` (verify
  live); the `cargo run -p kelir-plugin-testkit --bin vectors` path does
  not exist — do not use it, do not invent it.
- DX-2: canonical example — write
  `cans/artifacts/examples/fluent-builder-chaos.{rs,ts,py}` per §3.3.
  Verify placement live: if `cans/artifacts/examples/` takes only TS today,
  place the `.rs`/`.py` where the tree puts that language's examples and
  record the placement in the commit message.
- **DoD:** `just codegen` clean + byte-stable on re-run (if regen flips
  ordering run-to-run, fix the ordering or record it as a known defect in
  the commit message — do not silently commit a diff that re-flips);
  `just check-manifest check-tests check-scorecard` green; vectors load via
  the testkit; `cans check --refs-only` green.

### A3 — Rust: nested scopes + validating build + async ergonomics

- DX-3: implement §3.1 — 8 scope structs + 8 entry points on
  `PluginBuilder<Ctx, Deps>`; scopes append into the parent's existing
  `Vec` fields in closure order; nested == flat descriptors (test it).
- DX-3: `build()` becomes `-> Result<PluginWrapper<Ctx, Deps>, PluginError>`
  implementing the §0.5 triple (ring resolution, vsock spec, scope shape).
  Update the existing call sites in-tree (test file + any `builder_sdk.rs`
  use). `Satisfies` prefix-order check stays at `KelirBuilder::plugin()`;
  the missing-prerequisite message becomes
  `missing prerequisite: <dep> required by <plugin>` (§0.2).
- DX-3: async ergonomics — all 9 async verbs (`on_vm_init`, `on_vm_ready`,
  `on_vm_shutdown`, `on_pre_commit`, `on_post_commit`, `on_host_preempt`,
  `on_watchdog`, `on_memory_pressure`, `on_heartbeat`) accept a plain
  `async fn` (the `on_memory_pressure`/`on_heartbeat` form); the 7 that
  demand hand-written `Pin<Box<dyn Future>>` lose that requirement.
  Keep the 24 flat verbs; keep `PluginWrapper`'s zero public methods
  (consumption-only, no post-build mutation surface).
- DX-3: tests (`crates/kelir/tests/fluent_builder_plugin_test.rs`) —
  convert the never-called `_compiles` / `_chaos_migrates` fns into real
  `#[test]` fns asserting L1/L2 wiring: nested == flat descriptors, build
  validation errors (bad vsock port, unresolvable ring if constructible),
  append order of duplicate handlers, multi-vsock registration, canonical
  §3.3 chaos shape. No execution, no engine.
- **DoD:** `cargo check --workspace --all-targets`, `cargo clippy
  --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`,
  `cargo test -p kelir` green; `just check-hygiene` green.

### A4 — TypeScript: the missing 13 verbs + real enforcement

- DX-4: add the 12 missing ring verbs (`onFsLock`, `onNetInbound`,
  `onHostLsmExec`, `onHostPressure`, `onHostKeyringAccess`, `onVmInit`,
  `onVmReady`, `onVmShutdown`, `onPreCommit`, `onPostCommit`,
  `onHostPreempt`, `onMemoryPressure`) + plugin-level `vsockService`
  (same `(port, service, schema)` spec; sandbox `registerVsockService`
  stays). Handler signatures follow the live Rust verb types; verify each
  against `plugin_builder.rs` + the fusion tables (the rings already exist
  in `generated/fusion.tables.generated.ts`).
- DX-4: implement §3.1 — 8 scope entry points + scope interfaces on
  `PluginContract`, delegating to the existing `addHook` store.
- DX-4: enforcement, no longer type-only — `requires()` records dep names
  at runtime (in addition to the type-level `requires<Req>`); `derive()` /
  `context()` store their fn on the built plugin instead of `void fn`
  discarding it (stored, not executed — no engine at L1/L2); `build()`
  implements the §0.5 triple and throws `EConfig`; the contract gains a
  built flag so any verb after `.build()` throws (Python already does this;
  TS's silent post-build divergence ends); `.use()` rejects an unbuilt
  contract at runtime (remove the `hooksForFusion()` fallback for unbuilt
  contracts; `hooksForFusion` stays public only on `BuiltPlugin`).
- DX-4: tests (`matrix.fluent-builder.test.ts`) — nested == flat
  descriptors, all 24 verbs present, build validation errors, post-build
  mutation throws, `.use(unbuilt)` throws, canonical §3.3 chaos shape.
- **DoD:** `pnpm -r exec tsc --noEmit` green; `just check-hygiene` green.

### A5 — Python: the 6 broken verbs + enforcement + factories

- DX-5: add the 6 missing `RingEvent` members (`fs-lock`,
  `host-lsm-exec`, `host-pressure`, `host-keyring-access`, `host-preempt`,
  `memory-pressure`) to `generated/fusion_tables.py` — fix at the generator
  and regen if the file is generated (verify live); hand-edit only if the
  file is committed-by-hand. All 24 verbs must resolve at `.build()`;
  the `test_unmapped_ring_raises_econfig_at_build` test is updated to
  assert resolution, not failure. `_descriptors()` keeps the `EConfig`
  backstop for future unmapped rings (§0.5a).
- DX-5: implement §3.1 — 8 scope entry points + scope classes delegating
  to the existing `_on()` store; change the per-verb store from dict
  (last-wins) to ordered list (append, §0.3); vsock single slot becomes a
  list (multi, §0.4).
- DX-5: registration-time arity check — each `on_*` verifies the handler
  arity via `inspect.signature` and raises `EConfig` at registration on
  mismatch (wrong-arity handlers fail fast, not at `boot()`). `require()`
  keeps its dedupe-by-name; the missing-prerequisite message becomes
  `missing prerequisite: <dep> required by <plugin>` (§0.2, still raised
  at `boot()`). `scope()` validates the value against the live MountScope
  shape (unknown keys → `EConfig` at `.build()`); `context()` stays stored.
- DX-5: factories — all 13 `*_builder()` factories in
  `packages/py/kelir/plugins/` emit `.require()` for their real deps +
  map their config (chaos + ledger first, template fixed so regen keeps
  them; verify the codegen path live — A2 owns the generator, A5 owns the
  factory bodies and requests the template change via `_collab/handoffs.md`
  if the generator is A2's).
- DX-5: tests (`test_plugin_fluent_builder.py`) — nested == flat
  descriptors, all 24 verbs build, append order, multi-vsock, arity
  `EConfig`, canonical §3.3 chaos shape, post-build `EConfig` (already
  covered — keep).
- **DoD:** `pytest packages/py/tests/test_plugin_fluent_builder.py` green;
  full Py suite no worse than baseline (94 passed, 4 skipped at 23b962a);
  `mypy packages/py` no new errors (the 2 pre-existing errors in
  `test_lifecycle_session.py` stay, do not fix them here); `python3 -m
  compileall packages/py` green if `uv` is absent; `just check-hygiene`
  green.

## 5. Coordination and handoffs

`_collab/handoffs.md`, one line each:

```
- from: agent-<id> | to: agent-<id> | file: <path> | request: <one-line ask> | blocking: yes|no
```

Pre-agreed handoffs (no file edit needed):
- A1 provides §3.1/§3.2/§3.3/§3.4 text — A2/A3/A4/A5 consume it. A1 lands
  first; others rebase onto A1 before starting code.
- A2 provides the codegen template change for factories — A5 consumes it.
- A2 provides the canonical §3.3 example — A3/A4/A5 mirror it in tests.
- A3/A4/A5 provide their nested-scope descriptor shape to A2 only if it
  differs from §3.1 (it must not — report a conflict instead).

## 6. Merge order

1. A1 (spec) — everything else builds on it.
2. A2 (artifacts/codegen/vectors/example) — A3/A4/A5 reference it.
3. A3 + A4 + A5 in parallel (independent files; rebase onto 1+2 first).
4. Integrator converges: rebase each onto the previous, single squash
   commit, full DoD gates, push on a supplied token only.

## 7. Definition of done for the work order (all agents)

- All 30 verbs + 8 scopes exist in Rust, TS, Python with identical
  descriptors for nested vs flat chains (tested in each SDK).
- `.build()` validates the §0.5 triple in all three SDKs; Python's 6
  broken verbs build; TS's 13 missing verbs exist.
- `vectors.json` has 50 vectors (14 + 28 verb + 8 nested); `parity run`
  reports the builder family not-executable (specified L1/L2 outcome).
- One canonical `chaos` example, identical in all three languages.
- Full gates on the merged tree: `cargo check --workspace --all-targets`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
  --all -- --check`, `cargo test --workspace`, `pnpm -r exec tsc --noEmit`,
  `mypy packages/py`, `just check-hygiene`, `just check-spec` (refs-only +
  full), `just check-manifest check-tests check-scorecard`, `just codegen`
  clean + byte-stable.
- No file exceeds 1000 LOC. No new `// TODO (L3)` beyond the pre-existing
  set. No `cans/` edit made only to ease code. No agent pushed.
```

---

**Environment notes (from the last run, verify live):** `cans` 0.21.0 via
`npm install -g cans-spec@0.21.0` if the binary is gone after a reboot;
`pnpm` 9.0.0 via npm; `uv` absent — Py gates run under `python3 -m`;
`LIBCLANG_PATH` must point at `~/.local/lib/python3.12/site-packages/clang/native`
for the testkit gates. Worktrees `~/workspace/kelir-fb2-a1` … `a5`,
branches `wo-fb2-a1` … `wo-fb2-a5`, converge worktree
`~/workspace/kelir-fb2-converge` on `wo-fb2-converge`. Each agent uses its
own `CARGO_TARGET_DIR` to avoid build-lock contention.
