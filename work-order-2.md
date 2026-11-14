# `work-order.md` — Five-Agent Parallel Edit to `cans/`

**Purpose:** Land the design fixes agreed in the review session as targeted, non-conflicting edits across `cans/*.md` and `cans/artifacts/`. Artifacts are frozen per `artifacts/README.md`; every spec edit must land with its artifact counterpart in lockstep.

**Ground rules (all agents):**
- One agent owns one file at a time. Do not touch files outside your scope list. If a cross-cutting edit is needed, write a `HANDOFF` line to `_collab/handoffs.md` and let the owning agent land it.
- Respect the SSOT-A1 pins in the spec **unless** your work order explicitly overrides one. When it does, update the pin **and every downstream `See:` reference** in the same commit.
- Examples in `cans/artifacts/examples/` are content-owned. Update them to compile against the new surface, or mark the offending line with `// WORK-ORDER-PENDING: <agent>-<num>` and file the gap in `_collab/handoffs.md`. Do **not** leave dangling TODOs without an owner.
- Run `just check-spec`, `just check-examples`, `cans check --refs-only` locally before handing back. A local pass that CI rejects is a bug in your edit, not the CLI.
- Parity vectors (`cans/artifacts/.../vectors.json` or equivalent) are the CI gate. If your edit changes an observable outcome, update the vector **and** the gate description in `15-operations.md#Unified-builder-parity-vectors` / `#Coverage-gates`.

---

## Agent 1 — Action Algebra (unify Verdict / FilterAction / PluginError::reject)

**Goal:** One `Action` enum. Kill the three-way split. Kill stringly-typed errnos.

**Overrides:** This supersedes the SSOT-A1 `Hook-return pin` in `02-plugin-model.md#Unified-verdict` and the `Reject mapping pin`. Update both pins and every `See:` that cites them.

**Owns:**
- `cans/02-plugin-model.md` — sections `#Unified-verdict`, `#Filter-before-FFI-gate`, `#Hook-return-semantics`
- `cans/11-protocol.md` — sections `#Verdict-method-mapping`, `#Custom-dispatch-contract`, `#Trap-decision-invariants`
- `cans/plugins/*.md` — every `## Error and return semantics` section (13 files)
- `cans/artifacts/examples/hook-matrix-full.py` — remove `E_PATH_TRAVERSAL` string errno
- `cans/artifacts/examples/git-gate.ts`, `hook-matrix.ts` — replace `throw new Error("EPERM: …")` with `Action::Drop`
- `cans/artifacts/examples/policy-matrix.{py,rs,ts}` — verdict call sites
- New artifact: `cans/artifacts/code/action-algebra.rs` (canonical `Action` enum + `Errno` closed set)
- New artifact: `cans/artifacts/code/action-algebra.ts`, `action-algebra.py`

**Concrete edits:**

1. Define `Action` in the new artifacts:
   ```rust
   #[non_exhaustive]
   pub enum Action {
       Forward,
       Pass,
       Drop { errno: Errno, reason: String },
       Suspend { until: SuspendUntil },
       Abort { reason: String },
       Patch(PatchPayload),
   }
   ```
2. State in `02-plugin-model.md#Unified-verdict`: `Verdict` and `FilterAction` are **type aliases** of `Action`, restricted at each ring by the trait signature (`FsInterposer::on_fs_write` accepts only `Forward | Drop | Suspend | Abort`; `FsInterposer::on_fs_lookup` accepts only `Pass | Drop | Suspend | Abort | Patch(FsInode)`). Ring-specific acceptance is enforced by `From<Action>` on each ring's return type.
3. Delete `PluginError::reject`. A plugin that wants to deny returns `Action::Drop`. `Err(PluginError::*)` is reserved for "I broke" (panic, I/O, invariant violation). State this as a new pin: `PluginError-scope pin`.
4. `Errno` is a closed enum in all three SDKs. Add `Errno::PathTraversal` mapped to `EPERM` in the driver. Delete the `"E_PATH_TRAVERSAL"` literal from `hook-matrix-full.py`.
5. Update `plugins/*.md` error sections: replace "returns Reject { errno, reason }" with "returns `Action::Drop { errno, reason }`". The errno table in each plugin file stays.
6. Update `03-sdk-architecture.md#Unified-verdict-surface` spelling table to:
   ```rust
   // all three languages
   c.pass() / c.drop(errno, reason) / c.suspend(until) / c.abort(reason) / c.patch(payload)
   ```
   Delete `Verdict.pass_()` trailing underscore (Python sugar: `c.pass()`).

**Acceptance:** `cans/artifacts/examples/policy-matrix.{rs,ts,py}` compile against the unified enum; `--action-coverage` vectors still enumerate every `Action` variant; `hook-matrix-full.py` has zero string errnos.

---

## Agent 2 — Session & Context Accessors (kill the four-representations problem)

**Goal:** One accessor for planes, one for plugin caps. Kill naming drift.

**Overrides:** Supersedes `03-sdk-architecture.md#Service-lookup-across-languages`, `#Plugin-extension-pattern`, and the SSOT-A1 `Capability keys are plugin names` pin (it stays true, but the *accessor* is unified).

**Owns:**
- `cans/03-sdk-architecture.md` — sections `#Service-lookup-across-languages`, `#Plugin-extension-pattern`, `#Runtime-surface`, `#Session-surfaces`, `#Unified-builder-surface`
- `cans/02-plugin-model.md` — sections `#First-class-plugins`, `#KelirPlugin-trait` (context clause only)
- `cans/plugins/*.md` — every `c.cap.*` / `c.<key>` / `session.cap.*` mention
- `cans/artifacts/examples/*` — all call sites
- New artifact: `cans/artifacts/code/session-accessors.rs`, `.ts`, `.py`

**Concrete edits:**

1. Unify plane access: `session.plane::<VmPlane>()` (Rust), `session.plane("vm")` (Py/TS). `session.vm`, `session.terminal`, `session.storage`, `session.wire`, `session.checkpoint`, `session.host`, `session.vsock` become `session.plane("<name>")` return values. Keep the property form as **sugar on the accessor** so examples don't all break — document `session.vm` as `session.plane("vm")` sugar.
2. Unify cap access: `session.cap::<LedgerCap>()` (Rust), `session.cap("ledger")` (Py/TS). Delete `session.ctx<T, _>()`, `session.ctx<T>("name")`, `session.ledger`, `session.cap.ledger`. Document `session.cap("ledger")` returns `Option<LedgerCap>` (`None` if not mounted).
3. Unify hook-context cap access: `c.cap::<RepoPolicyCtx>()` (Rust), `c.cap("repo-policy")` (Py/TS). Delete `c.cap.repo_policy`, `c["repo-policy"]`, `c.cap.<key>` attribute access. `session.ctx<>` and `c.ctx<>` are gone.
4. **Naming sweep** (mechanical, do not litigate):
   - `kelir.model_plugin` ↔ `kelir::plugin_model` → pick `kelir.plugin_model` for Python; note the rename in a deprecation table.
   - `session.terminate()` ↔ `session.stop()` → keep both as aliases, document `terminate` as primary, `stop` as sugar; do not remove either.
   - `plugin_instance` / `.use` / `.plugin` for trait-instance mount → keep `.use()` as universal; `.plugin_instance()` is Rust-only sugar for the trait path (`P: KelirPlugin + PluginDeps`). Document the three in one table.
   - `Verdict.pass_()` → `Verdict.pass()` (Python sugar on the same `Action`, landed by Agent 1).
   - `kelir.plugins.plugin_<name>` import path → document as unstable implementation detail; the public path is `kelir.plugins.<name>`.
5. **One source of truth for plugin deps:** declare in `#KelirPlugin-trait` that Rust's `type Deps` is *derived* from `require::<T>()` calls (or vice versa), not hand-maintained. Name the macro/derive that lands this. Same for TS (`requires<"ledger">()` populates `Deps`) and Python (`requires: tuple[str]` populates the same registry that `boot()` walks).
6. Update `03-sdk-architecture.md#Service-lookup-across-languages` to read: "three accessors, one meaning: `session.plane()`, `session.cap()`, `c.cap()`. Every example uses these."

**Acceptance:** `cans/artifacts/examples/autonomous-swe-agent.{rs,ts,py}` and `db-backed-runtime.{rs,ts,py}` use only `session.plane`, `session.cap`, `c.cap` (plus documented sugar); no `c["repo-policy"]`; no `c.cap.repo_policy` attribute access; no `session.ctx<>`.

---

## Agent 3 — Nesting & Scopes (make nesting the only surface)

**Goal:** Scopes are the only way to reach a ring verb. Extend scopes with config, deps, fork, rules. Delete the flat form and `.end()`.

**Overrides:** Supersedes `02-plugin-model.md#Chain-decisions` items (a), (e), and the flat-form equivalence claim in `#Nested-scopes`.

**Owns:**
- `cans/02-plugin-model.md` — sections `#Unified-builder`, `#Chainable-verbs`, `#Nested-scopes`, `#Chain-decisions`
- `cans/03-sdk-architecture.md` — section `#Unified-builder-surface` (builder chain only — do not touch cap accessors; that is Agent 2)
- `cans/15-operations.md` — sections `#Unified-builder-parity-vectors`, `#--unified-builder-coverage`
- `cans/artifacts/examples/unified-builder-surface.{rs,ts,py}`
- `cans/artifacts/examples/fluent-builder-chaos.{rs,ts,py}`
- New artifact: `cans/artifacts/code/scope-builders.rs` (canonical scope types)

**Concrete edits:**

1. **Delete the flat form.** Remove `build_flat_plugin` / `buildFlatPlugin` / `build_flat_plugin` from the examples. State in `#Nested-scopes`: "A flat chain exists only as `group(|g| g.on_trap(…).on_fs_write(…))` — a readability fold that produces the same descriptors. There is no separate flat surface."
2. **Delete `.end()` in TS/Py.** In Rust, `.end()` stays (type-system requirement) but the docstring notes it is a type-system artifact, not a semantic one. Update the examples accordingly.
3. **Type-check Py/TS scopes.** Python scopes are `Protocol`s; TS scopes are generic interfaces. State in `#Nested-scopes`: `f.on_trap(...)` inside `.fs(...)` must be a type error in all three SDKs. New artifact: `cans/artifacts/code/python-scope-protocol.py` and `typescript-scope-interface.ts`.
4. **Give the orphan verbs their own scopes:**
   ```
   .actions(|a| a.register("quarantine", schema, handler))
   .streams(|s| s.register(0x0013, schema, handler))
   .vsock(|v| v.service(0x0013, "echo", handler))
   .params(|p| p.model::<MigrateParams>())
   .deny(|d| d.on_trap_deny(|event, verdict, session| …))
   ```
   Update `#Chain-decisions` (a) accordingly: `trap(filter, handler)` and `trap_scope(closure)` stay; `register_custom_action`, `register_stream_kind`, `vsock_service`, `params`, `on_trap_deny` move into their scopes.
5. **Extend scope shape** (new sub-nodes in `#Nested-scopes`):
   - `.config(cfg)` — scope-level config. Move `lock_broker`, `carryover_bytes`, `scan_engine`, `snoops` into `.fs(|f| f.config(…))`. Move `storage.lock_broker.scope_globs` into `.fs(|f| f.config(lock_broker=…))`.
   - `.requires::<P>()` — scope-level deps.
   - `.fork_with(f)` — scope-level fork policy.
   - `.timeout(d)` / `.priority(i)` — per-scope scheduling. Coordinates with Agent 5; do not land the actual predicate grammar here — Agent 5 owns `TrapFilter` predicates.
   - `.rules(|r| r.read.allow(glob).deny(glob).write.mask(regex))` — per-rule policy inside a hook. This is the mechanical replacement for `hook-matrix-full.py`'s `IDENTITY_PATH`/`DENY_MARKER`/`CARRYOVER` constants.
6. **Update the parity gate.** `--unified-builder-coverage` in `15-operations.md` changes from "50 builder cases: one per verb (30), one per scope (8), one per merge (8), plus one per chain verb (5)" to a new count: 8 scopes × (verbs + config + requires + fork + timeout + priority + rules) + 5 chain cases. Leave the exact number `TBD` and file a `HANDOFF` to Agent 5 for the predicate enumeration; the vector count is recomputed once Agent 5's `TrapFilter` grammar lands.

**Acceptance:** `unified-builder-surface.{rs,ts,py}` have no flat chain, no `.end()` in TS/Py, all 5 orphan verbs are scoped, and `.fs(|f| f.on_trap(…))` is a compile/type error in a new negative test vector. `#Chain-decisions (e)` is rewritten; `(i)` composite unpack path unaffected.

---

## Agent 4 — Config Defaults, TODO Cleanup, Scenario Runner Deletion

**Goal:** Kill the `TODO (L3)` graveyard. Delete the no-op harness. Make configs `Default`.

**Overrides:** Supersedes `15-operations.md#Coverage-gates` claims about the scenario runner and `artifacts/scorecard.md` coverage counts.

**Owns:**
- `cans/01-architecture.md` — section `#Codebase-invariants` (add a `Config-default invariant`)
- `cans/15-operations.md` — sections `#Coverage-gates`, `#Examples-compile`, `#Unified-builder-parity-vectors` (only the "scenario runner" references)
- `cans/artifacts/scorecard.md` — remove scenario runner coverage claims; mark them as `TBD` or delete
- `cans/artifacts/examples/scenario-runner.{py,rs,ts}` — **delete all three files**
- `cans/artifacts/examples/container.{rs,ts}` — move the `TODO (L3)` body into a single top-of-file `// STATUS: aspirational; see §4 of work-order` block; leave the boot/spawn path intact
- `cans/artifacts/examples/autonomous-swe-agent.{rs,ts,py}` — collapse multiple `TODO (L3)` markers into one `// L3-GAP: <one-line summary>` per file
- `cans/artifacts/examples/db-backed-runtime.{rs,ts,py}` — same
- New artifact: `cans/artifacts/code/vm-config-defaults.rs` (all config structs with `#[derive(Default)]`)

**Concrete edits:**

1. **Delete the scenario runner.** Remove `cans/artifacts/examples/scenario-runner.{py,rs,ts}`. Update `15-operations.md#Coverage-gates` to state: "The scenario runner is not part of the artifact set. Coverage is asserted by `kelir-plugin-cli parity run` and the per-plugin matrix; there is no separate scenario-execution harness." Remove the `_invoke_example` reference from any docs.
2. **Scorecard honesty.** In `cans/artifacts/scorecard.md`, strike the `Test Plan Coverage` section and replace with: `Coverage is per-plugin per the #Per-plugin-CI matrix; see `02-plugin-model.md#First-class-plugins`. Scenario counts are TBD until the runner is rebuilt against a live SDK.` Do **not** invent new numbers.
3. **Config defaults.** In `01-architecture.md#Codebase-invariants`, add a new sub-node:
   - `Config-default invariant`: every `VmConfig`, `StoragePlaneConfig`, `WireConfig`, `PtyConfig`, `DriverPolicy`, and plugin `*Config` struct derives `Default`. Adding a field is not a breaking change to callers. The `default_vm_config(...)` helper in `01-architecture.md#Embedded-profile` is deprecated in favor of typestate setters on `KelirBuilder`.
4. **Typestate setters.** New artifact `vm-config-defaults.rs` shows:
   ```rust
   KelirBuilder::new()
       .vcpus(4).memory_mb(4096)
       .kernel(format!("./fixtures/vmlinux-6.6-{ARCH}-kelir"))
       .rootfs("./fixtures/rootfs-swebench.erofs")
       .tsc_locked(true).watchdog(true)
       .driver(DriverPolicy::default().wx_enforcement(WxPolicy::Strict))
   ```
   Update `01-architecture.md#Embedded-profile` to note the helper is deprecated; keep the note for one release.
5. **TODO triage.** Every example with `TODO (L3)` markers gets either:
   - Deleted (if the whole example is aspirational), **or**
   - Collapsed into a single `// L3-GAP: <one-line>` top block listing what is stubbed, **or**
   - Landed with a real implementation.
   File a per-file `HANDOFF` entry so the gap is visible. Do not leave `TODO (L3)` scattered inline.
6. **Examples compile tier.** Update `15-operations.md#Examples-compile`: aspirational examples are excluded from `just check-examples`. Add `cans/artifacts/examples/aspirational/` as a directory and move files there when the whole example is fiction. `examples/` (top-level) compiles in all three languages or it is not in `examples/`.

**Acceptance:** no `scenario-runner.*`; every `examples/*.rs|ts|py` compiles or lives under `aspirational/`; `config-defaults.rs` compiles and is referenced from `01-architecture.md#Codebase-invariants`; `scorecard.md` no longer claims scenario-runner coverage.

---

## Agent 5 — New Granularities

**Goal:** Add the missing primitives. Everything from the review's §7 that landed in scope.

**Overrides:** Extends `02-plugin-model.md#Segregated-ring-traits`, `10-attestation.md#Execution-receipts`, `11-protocol.md#Trap-fast-path`, `08-lifecycle.md#Watchdog`, `07-wire-plane.md#DNS-policy-filter`. Does **not** override Agent 1's `Action` enum — new primitives must return `Action`.

**Owns:**
- `cans/02-plugin-model.md` — section `#Segregated-ring-traits` (new sub-nodes: `#TrapFilter-predicates`, `#Hook-ordering`, `#Per-ring-timeouts`)
- `cans/10-attestation.md` — section `#Execution-receipts` (`#Hook-trace`, `#Typed-extensions`)
- `cans/11-protocol.md` — section `#Trap-fast-path` (`SuspendUntil::Condition`), `#Custom-dispatch-contract` (already correct)
- `cans/08-lifecycle.md` — section `#Watchdog` (`Veto { event, by, reason }`), `#Lifecycle-vetoes`
- `cans/07-wire-plane.md` — section `#DNS-policy-filter` (`SinkholeRule` per-domain)
- `cans/03-sdk-architecture.md` — sections `#Unified-verdict-surface` and `#Session-surfaces` (only the `on_trap_deny` and `SuspendUntil` clauses)
- New artifacts: `trap-filter-predicates.rs`, `hook-trace.rs`, `sinkhole-rules.rs`
- `cans/artifacts/examples/hook-matrix-full.*` — add `on_trap_deny(event, verdict, session)` example; `hook_trace` read

**Concrete edits:**

1. **`TrapFilter` predicates.** Add to `#TrapFilter-predicates`:
   ```rust
   TrapFilter::binary("git").with_arg("push")
       .with_cwd("/workspace/**")
       .with_env("CI", "1")
       .with_uid(0)
       .argv_matches(r"^--force$")
   ```
   Python/TS spellings: `with_cwd` / `withCwd`, `with_env` / `withEnv`, `with_uid` / `withUid`, `argv_matches` / `argvMatches`. Document that all predicates are AND-ed. The current `hook-matrix-full` cwd/argv checks in the handler body become redundant and should be removed from the example.
2. **Hook ordering.** Add to `#Hook-ordering`: every registered handler carries a `priority: i32` (default `0`). Within a ring, handlers fire by descending priority, then by registration order. Expose `.priority(i)` as a scope-level config (Agent 3 lands the verb; Agent 5 lands the semantics). Document that mount order is *not* execution order.
3. **`SuspendUntil::Condition`.** Add to `#Trap-fast-path`:
   ```rust
   SuspendUntil::Condition(Box<dyn Fn(&SessionContext) -> bool + Send + Sync>)
   ```
   TS/Py spellings: `.condition(fn)`. The predicate polls on a host-driven cadence (default 100 ms, configurable). Resume executes on the first true. Does not replace `Deadline` or `Approval`; both stay.
4. **Per-ring timeouts.** Add to `#Per-ring-timeouts`: every hook can carry `.timeout(Duration)` (default: none, i.e., ring's own ceiling; hot-path rings default to `5 ms`). On timeout the ring returns `Action::Drop { errno: Errno::Etimeout, reason: "hook timeout" }`. Host-side, the trap is released.
5. **`SinkholeConfig` per-domain rules.** Replace `SinkholeConfig { allowlist, drop_all }` with `SinkholeConfig { rules: Vec<SinkholeRule> }` where:
   ```rust
   struct SinkholeRule { host: String, methods: Option<Vec<String>>, paths: Option<Vec<String>>, allow: bool }
   ```
   Update `07-wire-plane.md#DNS-policy-filter` and the examples that configure `sinkhole`. Keep the old `allowlist`/`drop_all` shape as sugar that expands to rules, and document the desugaring.
6. **`Veto` with reason and recovery.** Replace `register_veto(event, Box<dyn Fn() -> bool>)` with:
   ```rust
   LifecycleCtx::register_veto(event, Box<dyn Fn() -> Veto>)
   struct Veto { allowed: bool, by: String, reason: String, recovery: Option<String> }
   ```
   Update `08-lifecycle.md#Lifecycle-vetoes`. `Veto::allowed(true)` is the success case; the other three fields are populated only on denial. Every denied veto surfaces in `LifecycleCtx` and in the receipt hook_trace (item 8).
7. **`on_trap_deny(event, verdict, session)`.** Update `02-plugin-model.md` and `03-sdk-architecture.md`. The signature receives the *denying* `Action` so the break-glass handler can say "denied by X for Y, quarantine." Remove the `TODO` in `autonomous-swe-agent.rs` where this was stubbed.
8. **`ExecutionReceipt.hook_trace`.** Add to `10-attestation.md#Execution-receipts`:
   ```rust
   struct HookSpan { plugin: String, ring: Ring, verdict_kind: VerdictKind, duration_ns: u64, ordinal: u32 }
   struct ExecutionReceipt { /* … */ hook_trace: Vec<HookSpan> }
   ```
   Populated for every hook that fired for the trapped process. Diagnostic only; excluded from Merkle roots (consistent with `phase spans`). Emitted to OTel as spans.
9. **Typed receipt extensions.** Replace `extensions: Vec<{ namespace, payload }>` with typed accessors:
   ```rust
   receipt.fs_vfs()      -> Option<&FsVfsExt>
   receipt.net_l7_sql()  -> Option<&NetL7SqlExt>
   receipt.hooks()       -> &[HookSpan]
   receipt.ext_raw()     -> &[(String, Vec<u8>)]   // third-party fallback
   ```
   Update `10-attestation.md#Execution-receipts` and every `.extensions.some(…)` / `.extensions.iter().find(…)` in the examples.
10. **Coverage.** Coordinate with Agent 3: `--unified-builder-coverage` counts change because `TrapFilter` has new predicates. Add a `--trapfilter-coverage` gate that asserts every predicate method has at least one vector. Add a `--hook-trace-coverage` gate that asserts every `Ring` value appears in at least one `HookSpan` vector.

**Acceptance:** all new predicates/verbs appear in `unified-builder-surface.{rs,ts,py}` and `hook-matrix-full.*`; `hook_trace` populated in a new test vector; `on_trap_deny` signature changed; `SinkholeConfig` desugaring documented; every existing example still compiles (or is marked aspirational by Agent 4).

---

## Cross-agent coordination

| Touch | Agent 1 | Agent 2 | Agent 3 | Agent 4 | Agent 5 |
|---|---|---|---|---|---|
| `02-plugin-model.md` | `#Unified-verdict`, error sections | `#First-class-plugins`, `#KelirPlugin-trait` ctx | `#Unified-builder`, `#Chainable-verbs`, `#Nested-scopes`, `#Chain-decisions` | — | `#Segregated-ring-traits`, new sub-nodes |
| `03-sdk-architecture.md` | `#Unified-verdict-surface` (spellings) | `#Service-lookup`, `#Plugin-extension`, `#Session-surfaces` | `#Unified-builder-surface` (chain) | — | `#Unified-verdict-surface` (on_trap_deny, SuspendUntil) |
| `15-operations.md` | — | — | `#Unified-builder-parity-vectors`, `#--unified-builder-coverage` | `#Coverage-gates`, `#Examples-compile` | new coverage gates |
| `examples/` | `policy-matrix.*`, `git-gate.ts`, `hook-matrix.ts`, `hook-matrix-full.py` | all call sites | `unified-builder-surface.*`, `fluent-builder-chaos.*` | `scenario-runner.*` (delete), aspirational triage | `hook-matrix-full.*`, new vectors |
| `plugins/*.md` | all `## Error and return semantics` | every `c.cap.*` mention | every `use`/`plugin_instance` mount mention | — | — |

**Overrides requiring a pin bump (do these in a single dedicated commit per agent, on a branch named `<agent>-pins`):**
- A1: `Hook-return pin`, `Reject mapping pin` in `02-plugin-model.md#Unified-verdict`
- A2: `Capability keys are plugin names` — pin stays, accessor changes; `SSOT-A1` reference in `03-sdk-architecture.md#Service-lookup-across-languages`
- A3: `Chain decisions (a)`, `(e)` in `02-plugin-model.md#Chain-decisions`
- A4: `Coverage-gates` count in `15-operations.md`
- A5: `SuspendUntil` and `Veto` sections in `08-lifecycle.md` and `11-protocol.md`

**Sequencing:** A1, A2, A4 can start immediately. A3 depends on A2 (accessors must be stable before scopes are rewritten). A5 depends on A1 (new primitives must return the unified `Action`) and A3 (new predicates ride the scope builder). Estimated critical path: A1 → A5, A2 → A3 → A5. Total wall time: ~3 days if agents run in parallel and hand off at the gates.

**Shared validation:**
```
just check-spec                 # all agents
just check-examples             # all agents, per-agent subset allowed
just check-plugin-coverage      # A4 (deleted runner)
just check-action-coverage      # A1
just check-hook-coverage        # A5
just check-unified-builder-coverage   # A3
cans check --refs-only          # all agents, before any pin bump
```