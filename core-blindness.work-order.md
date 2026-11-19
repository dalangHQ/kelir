# Work Order: Core Blindness — `crates/kelir` knows zero plugins

see: cans/01-architecture.md#Composition-root
see: cans/02-plugin-model.md#Plugin-tiering
see: cans/02-plugin-model.md#Facade-blindness (new, Agent G adds)
Branch: `core-blindness` off `primitives-addition` (`879af02`). Same pinning convention as Law 7: spec, artifacts, and code move together.

## Ideal (one sentence)

The facade `crates/kelir` **is core**: it compiles, links, and type-checks with no plugin crate in its dependency graph, no `plugin_` token in its source, and no plugin name in its schema, errors, or sugar; all plugin aggregation moves to a new `kelir-bundle` crate.

## Law 8 — Core Blindness (pinned alongside Laws 1–7)

1. `crates/kelir/Cargo.toml` `[dependencies]` contains zero `plugin-*` entries.
2. `grep -rn "plugin_" crates/kelir/src` (case-insensitive) returns zero hits — imports, types, cfg gates, doc comments.
3. Shared protocol types that core and plugins both speak (verdicts, post-exec actions) live in `kelir-interface`; specific plugins re-export them. Zero type duplication (`cans/01-architecture.md:157`).
4. The facade never names a plugin in errors, YAML sub-schemas, or builder sugar; sugar lives in `kelir-bundle`.
5. `kelir-plugin-cli check --layers` gains a facade rule and a manifest rule; regression fails CI (new case `pe_21`).
6. `just check-core-blindness` (G1–G4, see "Hard CI gating") runs in the `lint-and-types` CI job, is `needs:`-ed by every test job, and has no override path.

## Agents

### Agent A — contracts into `kelir-interface` (merge FIRST, enables all others)

1. Move `PostExecAction`, `StepVerdict`, gate shapes from `plugin-step-eval` into `crates/kelir-interface/src/step_verdict.flow.rs`; `plugin-step-eval` re-exports both (compat, no duplication).
2. Add generic `PluginConfigParser` trait to `plugin_model`: each plugin parses its own `plugins.<name>` raw TOML section. Plugin crates implement it (L3 stubs acceptable for not-yet-migrated plugins).
3. Spec: `cans/02-plugin-model.md` gains both seams under a new `## Facade blindness` heading.

**Gate:** `cargo build -p kelir-interface -p plugin-step-eval` green; no facade dep introduced.

### Agent B — facade purge + `kelir-bundle` creation (after A)

1. Manifest: delete all 21 plugin deps (`crates/kelir/Cargo.toml:26-46`), the two mandatory ones included (`:35 plugin-profile`, `:38 plugin-step-eval`), the `all-plugins` feature (`:50-56`), and the plugin dev-dep (`:62`). Create `crates/kelir-bundle`: depends on `kelir` + all 21 plugins, owns the `all-plugins` feature, re-exports the old facade plugin surface. `kelir-integration` switches to `kelir-bundle`.
2. Source purge, all verified sites:
   - `src/protocol_wire.rs:5` — `pub use plugin_step_eval::{PostExecAction, StepVerdict}` → `kelir_interface::step_verdict`.
   - `src/builder_sdk.flow.rs:20,38,40`, `builder_adapters.flow.rs:17`, `builder_plugin.flow.rs:22`, `session_lifecycle.flow.rs:16` — plugin imports → interface types or generic registration.
   - `src/sandbox_runtime.flow.rs:23` — profile boot becomes generic registration path; profile materialization is `plugin-profile`'s job via its lifecycle participation.
   - `src/lib.rs:81-165` — delete every `pub use plugin_*` block (including ungated `:107`, `:120`, `:128`, `:148`).
3. Facade tests needing plugins move to `kelir-bundle` tests.

**Gate:** `cargo build -p kelir --no-default-features` green; `cargo tree -p kelir` shows zero plugin crates; `grep -rn "plugin_" crates/kelir/src` zero hits.

### Agent C — plugin sugar repatriation (after B; parallel with D, E)

1. `ToolGuardExt` (`src/builder_sdk.flow.rs:858-869`) moves to `kelir-bundle` as an ext-trait on `KelirBuilder` (plugins may not depend on the facade — `cans/02-plugin-model.md:524` — so the bundle is the only legal host for facade sugar).
2. Session ledger methods (`src/session_lifecycle.flow.rs:401-422`, `Capability<plugin_ledger::LedgerCtx>`) move to `kelir-bundle` as a Session ext over the generic `cap::<T>()` lookup; `plugin-ledger` keeps only `LedgerCtx` helpers.
3. Spec: `cans/plugins/tool-guard.md`, `cans/plugins/ledger.md` — sugar sections point at `kelir-bundle`.

**Gate:** fluent-chain examples still compile turbofish-free (Law 7 intact) via `kelir-bundle`.

### Agent D — profile schema displacement (after B; parallel with C, E)

1. `src/profile_schema.flow.rs:568,611,625,640` (`LedgerPluginProfile`, `ToolGuardPluginProfile`, `McpPluginProfile`, `PluginsProfile`) and `src/profile_validate.flow.rs:244-322` (`to_ledger_config`, `to_tool_guard_config`, `to_mcp_config`) + field error strings (`:445,453,709-785`) — facade keeps plane configs only; each plugin parses its own section via Agent A's `PluginConfigParser`; `plugin-profile` owns the aggregate `plugins:` section as an opaque map.
2. This completes `config-parity.work-order.md`'s "Zero Displaced State" invariant for the facade; mark that item done there.

**Gate:** `grep -rn "tool_guard\|ledger\|mcp" crates/kelir/src` zero hits; profile round-trip tests live in plugin crates + `kelir-bundle`.

### Agent E — interface concept scrub (after D)

1. `crates/kelir-interface/src/{credential_vault.rs, vsock_mesh.flow.rs, chaos_types.rs}` — plugin-specific shapes move to their plugin crates; interface keeps only generic interposer/trap shapes (`patch2.md:391`: core `TrapFilter` accepts POSIX attributes only; `vault://` rejection is plugin-owned).
2. Coordinate with `config-parity.work-order.md` Item 1 (strip `vaults: Vec<CredentialDescriptor>` from `WireConfig`) — close it if this completes it.

**Gate:** `_rules.yaml:55-60` forbidden-target check clean; no plugin-doc `see:` anchors from interface modules except `plugin_model` itself.

### Agent F — enforcement closure (draft in parallel, merge LAST)

1. `crates/kelir-plugin-cli/src/layers_cli.run.rs:8-18` RULES: add `("crates/kelir/src", &["plugin_"])` plus a manifest check — `crates/kelir/Cargo.toml` must contain no `plugin-` dependency entry.
2. Close pe_01 (red, `case-to-test-map.yaml:903-910`): per-tier plugin-to-plugin source scan — for each plugin dir under `crates/kelir-plugins`, forbid `use` of any higher-tier plugin crate name from the tier roster (closes the TODO L3 at `layers_cli.run.rs` check body).
3. Close pe_02 (red, `:911-917`): cycle detection in `resolve_plugin_registrations` → `EConfig` abort at boot. pe_03 (`AmbiguousMutation` scope conflict) same family — stretch goal.
4. New case `cans/artifacts/test-plan/cases/plugin_engine/pe_21_facade_zero_plugin_knowledge.yaml` + map entry + live test in `crates/kelir-plugin-cli/tests/`: mock tree with a plugin dep in the facade manifest and a `plugin_` token in facade src both fail `check --layers`.
5. CI wiring per "Hard CI gating": `justfile` gains `check-core-blindness` (G1–G4); `ci.yml:212` layering step and `plugin-ci-reusable.yml:643` become `just check-core-blindness`; gate is landed dark in this same merge.

**Gate:** `kelir-plugin-cli` tests green including pe_21; pe_01/pe_02 flip to green in the map.

### Agent G — convergence audit (last)

1. Re-baseline `cans/artifacts/scorecard.md` core purity row (`patch2.md:945` precedent: numbers-only pass).
2. Cross-check every agent's spec + artifact edits landed (879af02 convention: spec, artifacts, code move together).
3. Final grep gates; no new `TODO (L3)` markers introduced by this work order.

## Merge order

`A → B → {C, D} → E`; `F` merges last (flips enforcement on); `G` closes. A, F, G can be drafted in parallel from day one.

## Global acceptance gates

- `cargo tree -p kelir` — zero plugin crates; `cargo build -p kelir --no-default-features` succeeds.
- `grep -rni "plugin_" crates/kelir/src | wc -l` = 0.
- `just check-core-blindness` green (see Hard CI gating below).
- `kelir-plugin-cli check --layers` green with the facade rule; pe_01/pe_02 green; pe_21 green.
- Full workspace `cargo test` green; Law 7 compile_fail tests untouched and green.
- Scorecard core purity row re-baselined.

## Hard CI gating (Law 8 is enforced, not convention)

`justfile` gains one recipe — the flag string lives in justfile, CI never calls raw commands (same convention as `check-spec`, `justfile:53`):

```just
# ── Law 8: core blindness gate ──
# G1 facade manifest: zero plugin-* deps. G2 facade src: zero plugin tokens.
# G3 facade dep graph: zero plugin crates at any depth. G4 structural lint.
check-core-blindness:
    #!/usr/bin/env bash
    set -euo pipefail
    ! grep -qE '^[[:space:]]*plugin-' crates/kelir/Cargo.toml
    [ "$(git grep -iE 'plugin_' -- crates/kelir/src | wc -l)" -eq 0 ]
    [ "$(cargo tree -p kelir -e normal 2>/dev/null | grep -cE 'plugin-')" -eq 0 ]
    cargo run -p kelir-plugin-cli -- check --layers
```

Wiring (Agent F's merge, both workflow files):

1. `ci.yml` `lint-and-types` job: the "Validate Crate Layering Hierarchy" step (`ci.yml:212`) becomes `just check-core-blindness` — named `Core blindness gate (Law 8)`. Red on any of G1–G4; the job is already `needs:`-ed by every test job, so this blocks merge, not just reports.
2. `plugin-ci-reusable.yml` (`:643`, `check --layers` step) swaps to the same recipe — no path back in through the reusable pipeline.
3. `justfile:150` `check-layers` and `justfile:200` `check-cli` delegate unchanged; the new recipe is additive so local devs hit the same gate CI does.

Three independent tripwires, all red-fail in CI:

- **G2 grep** catches tokens (case-insensitive `git grep -i`), so renames, casing, and alias-hiding (`type X = plugin_y::X`) all fail — the token survives aliasing.
- **G3 cargo tree** catches the dependency graph at any depth (`-e normal`), closing the optional-dep-behind-a-feature hole: a plugin re-added as `optional = true` shows in `cargo tree` even when the feature is off, so feature-gating cannot smuggle a dep back in.
- **G4 layers** catches structure (facade source-prefix rule + manifest rule from Agent F) and is the only tripwire that also polices plugin-to-plugin tiers (pe_01 closure).

Sequencing note: the gate is *landed dark* — F wires it in the same merge that flips pe_01/pe_02/pe_21 green, because before B's purge it would fail on the current facade (21 deps, `plugin_` tokens). After that merge, any regression — a plugin dep, a plugin token, a plugin type alias — breaks CI with no override path; the only way to `plugin_` in the facade is to change this file first.

## Accepted seams — do NOT purge (cans/02-plugin-model.md:161-169)

- Generic model in `kelir-interface`: `KelirPlugin`, interposer ring traits, `PluginError`, `PluginDeps` HList, `register_stream_kind` — core hosting the generic extension points is the design, not a leak.
- Engine ring hooks, `PipelineEngine` dispatch, panic quarantine (`catch_unwind` → `EFAULT`).
- `plugin_registry` name/tier strings — data, not plugin identity.
- `kelir-bundle`'s entire job is knowing plugins; `kelir-adapters/mcp-server`'s optional `plugin-mcp` dep is out of scope.

## Slop risks (l3-slop-register style — reject on review)

1. Renaming crates, deps, or paths to dodge the `plugin_` prefix scan instead of moving code.
2. Hiding plugin types behind local facade aliases (`type StepVerdict = plugin_step_eval::StepVerdict`) to keep the token out of grep.
3. Moving the schema to the bundle but leaving `to_*_config` validation in the facade "temporarily" — L3 drift re-imports the whole dependency.
4. Deleting `ToolGuardExt` without the `kelir-bundle` replacement — silently regresses the fluent-chain UX (Law 7).

## Evidence (verified on 879af02)

| Site | Finding |
| --- | --- |
| `crates/kelir/Cargo.toml:26-46,50-56,62` | 21 plugin deps; `plugin-profile` (:35), `plugin-step-eval` (:38) mandatory |
| `crates/kelir/src/protocol_wire.rs:5` | ungated plugin type re-export in core protocol |
| `crates/kelir/src/builder_sdk.flow.rs:20,38,40,858-869` | plugin imports + `ToolGuardExt` |
| `crates/kelir/src/{builder_adapters,builder_plugin,session_lifecycle}.flow.rs:17/22/16,401-422` | ungated imports; ledger-typed Session methods |
| `crates/kelir/src/sandbox_runtime.flow.rs:23` | ungated `plugin_profile` wiring |
| `crates/kelir/src/lib.rs:81-165` | ~28 plugin re-export blocks (incl. ungated :107,:120,:128,:148) |
| `crates/kelir/src/profile_{schema,validate}.flow.rs:568-679,243-322,445-785` | plugin-named schema structs, config builders, field error strings |
| `crates/kelir-plugin-cli/src/layers_cli.run.rs:8-18` | RULES scans engine/plugins/drivers dirs only — **no facade rule exists** |
| `cans/artifacts/test-plan/case-to-test-map.yaml:903-930` | pe_01, pe_02 red (lint blind spots) |
| `crates/kelir-interface/src/{credential_vault.rs,vsock_mesh.flow.rs,chaos_types.rs}` | plugin concepts in the abstraction crate |
