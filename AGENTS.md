# AGENTS.md — kelir

Operating contract for any agent working in this repo. Read this before touching code.

If this file and another doc disagree on *how* work is done, this file wins. If this file and `cans/` + `cans/artifacts/` disagree on *what* is true, the spec wins.

## 0. How to work

- Decide yourself. Never hold an item for a decision and never ask the user for one. Decide from the overall intent of the work, fix it, report the decision briefly after the fact.
- Verify every audit claim against the live tree before editing. Audits re-raise fixed items with stale line numbers and misattribute lines.
- Spec text, code artifacts, and ASCII diagrams move together. A change that updates one and not the others is incomplete.
- Finish with gates green (section 6). A gate you did not run is not green.
- Parallel work: one agent per worktree/branch, exclusive scope, no push from agents. Converge by squash-merge into one commit, re-gate after.
- Never force-push a divergent head. Pull, rebase, re-audit, then push.
- Push only when the user supplies a token in that turn. Use it transiently, never store or record it, restore the remote to plain HTTPS after. Rotation stays advised until confirmed. No token in the turn: commit locally and stop; say so once.

## 1. Source of truth

- `cans/*.md` nodes own normativity: ordering, placement, threat model, invariants, rationale.
- `cans/artifacts/` owns content: field layouts, schemas, wire formats, pipeline diagrams, code signatures. Artifacts are terminal — no outbound refs.
- A node that re-lists artifact content is a defect. It must point at the artifact with a bare `see:` ref.
- Where `work-order.md` contradicts `cans/` + `cans/artifacts/`, follow the artifacts and record the decision in the commit message.

## 2. L1 / L2 / L3

- **L1 — The Contract.** Compilable interfaces, types, skeletons, empty method signatures only. No implementation logic. A cheatsheet the compiler accepts.
- **L2 — The Skeleton & Flow (Wiring).** Internal state, pipeline dispatch, invocation plumbing, the whole execution path wired end to end. Complex algorithms stay as `// TODO` markers returning a typed `Result` or zero-value.
- **L3 — The Meat (Deep Implementation).** Fill in the final TODOs. Zero unwritten logic, zero stubs, zero band-aids.
- Level is the filename suffix: `.rs` is L1, `.flow.rs` is L2, `.run.rs` is L3 (cans/01-architecture.md#Maturity-discipline).
- Current phase: **L1/L2 only. No L3, no inline tests.** L3 starts only when the user says so.

## 3. Comment law (hard rule)

Codebase comments are one-liners only, of exactly three kinds. Nothing else.

| Kind | Form | Use |
|---|---|---|
| `see:` | `// see: cans/06-storage-catalog.md#Entity-model` | Point at the spec node that owns this code. Target is a `#Leaf` anchor, or an anchored artifact path: `// see: cans/artifacts/code/vmm-driver-trait.rs#VmmDriver` |
| `TODO:` | `// TODO: wire to the live per-process receipt.` | L2 deferral. Tag the level where known: `// TODO (L2): ...` / `// TODO (L3): ...` |
| `SAFETY:` | `// SAFETY: status is a valid out-pointer; WNOHANG never blocks.` | Unsafe blocks only. The single exception: `cans/01` mandates SAFETY invariants on unsafe code. |

- Rust / TypeScript use `//`, Python uses `#`. Same three kinds, same one-line limit.
- Banned in scope: `///`, `//!`, prose `//` / `#`, banners, trailing comments, Python docstrings, manifest `# NOTE:` lines.
- `todo!()` the macro is denied by lints (section 6). `// TODO:` the comment is required at L2. Different things.
- Scope: `*.rs` under `crates/`, `*.ts` under `packages/ts/`, `*.py` under `packages/py/` (excluding `.venv/`, `node_modules/`, `target/`). Out of scope: `cans/artifacts/` (SSOT content, not codebase), `justfile` / CI workflows (byte-identical to artifacts), `scripts/` shell headers.
- Checks — each must print nothing:
  ```sh
  grep -rn --include='*.rs' -E '^\s*//' crates/ | grep -vE '// (see:|TODO[^:]*:|SAFETY:)'
  grep -rn --include='*.ts' -E '^\s*//' packages/ts/ | grep -vE '// (see:|TODO[^:]*:|SAFETY:)'
  grep -rn --include='*.py' -E '^\s*#' packages/py/ --exclude-dir=.venv | grep -vE '#(!| (see:|TODO[^:]*:|SAFETY:))'
  grep -rn --include='*.rs' -E '^\s*(///|//!)' crates/
  ```
- Baseline at `01b35f7` (2026-11-06): Rust 139 `see:` / 191 `TODO:` / 4 `SAFETY:` / 0 other; TS 19 / 4 / 0; Python 14 / 59 / 0; doc comments 0; trailing `//` 0. TODO count falls as L3 fills them; it never rises by prose.

## 4. Decisions (anti-plenger)

Numbered as supplied, renumbered where the source duplicated 3. On conflict, the higher rule wins; rule 14 beats rules 10–11.

1. Pure functions (zero side effects).
2. DRY, SRP, SSOT.
3. YAGNI.
4. Performance & security.
5. High cohesion, loose coupling.
6. Immutability.
7. Polymorphism over conditionals.
8. Observability.
9. Absolute idempotency.
10. Same logic, fewest code / LOC possible.
11. Deletion over addition.
12. Native-first.
13. One-line code first, one-line doc only.
14. Never simplify the basics and explicit requests.
15. Comments: only the section 3 one-liners. (As supplied this read "see: only"; section 3 supersedes it — `see:` + `TODO:` + `SAFETY:`.)

## 5. Plenger traits — forbidden patterns

1. **Backward-compatibility slaves.** No bloated adapters to preserve deprecated patterns. Break cleanly to the new standard.
2. **Tautology.** Green tests, broken system. A test that cannot fail is not a test.
3. **Context blindness.** Breaking global architecture, async workflows, or system constraints.
4. **Band-aids.** Patching symptoms instead of refactoring root structural rot.
5. **Bloat (DRY violations).** Reinventing utilities, interfaces, or logic that already exists in the global system.
6. **Hallucination.** Inventing libraries, fabricating methods, speculative logic never mentioned in `cans/` or `cans/artifacts/`. Every dependency and method must exist in the tree or be introduced by the patch itself.
7. **Happy-path bias.** Zero defensive programming. Every failure mode the spec names gets a path.
8. **Goodhart's law.** Never optimize for green by hardcoding expected outputs or hallucinating mocks. A test that passes without the implementation working is a defect, not a pass.

## 6. Hygiene and gates

- Max 1000 LOC per file, target 500. No `#[cfg(test)]` in `src/`; tests live in `tests/` only.
- Denied by workspace lints and clippy `-D warnings`: `unwrap()`, `expect()`, `panic!`, `todo!()`, `unimplemented!()`, `unreachable!()`.
- `#![forbid(unsafe_code)]` everywhere except the driver / transport / shim enclaves.
- Run before calling work done:
  ```sh
  cargo check --workspace --all-targets
  cargo clippy --workspace --all-targets -- -D warnings
  cargo fmt --all -- --check
  cargo test --workspace            # nextest per justfile for core/plugins/drivers
  pnpm -r exec tsc --noEmit
  uv run mypy packages/py           # or python3 -m compileall packages/py
  just check-hygiene
  just check-spec                   # when cans/ changed: cans check refs-only + full
  just check-manifest check-tests check-scorecard  # test-plan gates: manifest parity, anti-slop lint, scorecard
  # codebase see: pointers resolve inside just check-spec (cans 0.21.0 --codebase, enabled in cans/_rules.yaml)
  ```
  If `just` is not installed locally, run the underlying commands and let CI run the justfile path. Rust MSRV is 1.85.

## 7. Repo map

- `crates/kelir-interface/` — L1 contract SSOT. `crates/kelir-engine/` — engine, planes, dispatch. `crates/kelir-transport/`, `crates/kelir-shim/` — unsafe enclaves.
- `crates/kelir-plugins/` — 13 community plugin crates. `crates/kelir-catalog/`, `crates/kelir-otlp/`, `crates/kelir-adapters/`, `crates/kelir-bindings/`, `crates/kelir-plugin-cli/`, `crates/kelir-plugin-testkit/`, `crates/kelir-integration/` — supporting crates.
- `packages/ts/`, `packages/py/` — SDK projections. `cans/` — spec. `cans/artifacts/` — SSOT content. `scripts/` — coverage / canon checks. `perf/` — budgets.
