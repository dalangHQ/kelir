# WO3-E misc report — spec anchors, testkit, plugin tier governance, misc L1/L2 mismatches

Agent E, work-order-3, branch `temp/wo3-e`, base 339ca53. All claims below verified live in this
worktree before editing. Phase: L1/L2 only — no `TODO (L3)` body was filled.

## E1 — anchors.json regeneration

- `cargo run -p kelir-testkit --bin anchors` → `31 files, 3986 anchors`.
- First run produced **zero diff** against the committed `crates/kelir-testkit/anchors.json` — the
  committed file is in sync with the current `cans/*.md` + `cans/plugins/*.md` corpus (top-level md
  only; the corpus reader does not recurse into `cans/artifacts/`).
- Second run: byte-identical again (deterministic; `git status` clean both times). No commit needed.
- Agent A edits `cans/plugins/tool-guard.md` in its own worktree in parallel; the orchestrator
  re-runs anchors at integration. My regen covers my tree only (recorded, no action).

## Extra assignment — check-manifest baseline (prior conflicts.md open item)

- Prior decision: "agent-a check-manifest gate red at origin/scaffolding baseline;
  `crates/kelir-interface/tests/causal_delta_hex_test.rs` absent from case-to-test-map.yaml".
- **Verified live: GREEN at my baseline.** `cargo run -p kelir-testkit --bin check-manifest` →
  `Manifest verification passed: 254/254 cases synchronized with filesystem. … 71 implemented /
  108 red / 75 gap. Red by language: rust=100 typescript=5 python=3`. Exit 0.
- The map already carries the resolution: `case-to-test-map.yaml` lists
  `crates/kelir-interface/tests/causal_delta_hex_test.rs` under `harness_tests` with a reason
  ("CausalDelta serde-hex pin … not a case test"), and `causal` appears 5x in the map (case
  `ar_09` + harness entry + reasons). No fix required; the prior open item can be closed as
  resolved-by-someone-before-WO3-E. Recorded, not re-litigated.

## E2 — vector suite count audit (58 = 46 builder + 12 others)

Actual file: `crates/kelir-plugin-testkit/src/vectors_testkit.rs` (work order's
`crates/kelir-testkit/src/...` path is stale — `kelir-testkit` holds the anchors/manifest/scorecard
bins only). Live accounting, verified by regenerating the golden set
(`kelir-plugin-cli vectors /tmp/…` → 58 vectors, name order identical to the committed
`./vectors.json`):

- **46 builder cases = 30 verb + 8 scope + 8 merge.**
  - 30 verb: base 2 (`plugin-builder-trap-only`, `plugin-builder-fs-write-trap`) +
    `plugin_builder_verb_cases()` 28 (22 ring verbs: process-exit, fs-lookup, fs-read, fs-lock,
    net-outbound, net-inbound, console-input, console-output, exec-stdout, exec-stderr,
    host-lsm-exec, host-pressure, host-keyring-access, vm-init, vm-ready, vm-shutdown, pre-commit,
    post-commit, host-preempt, memory-pressure, heartbeat, watchdog; + 6 structural: context,
    require, scope, fork-with, vsock-service, seal).
  - 8 scope: `plugin-builder-nested-{trap,fs,net,console,exec,host,lifecycle,telemetry}`.
  - 8 merge: `plugin-merge-{trap,fs,net,console,exec,host,lifecycle,telemetry}`.
- **12 non-builder cases = 5 plain traps + 1 exec_rewrite + 1 heartbeat + 2 watchdog + 3 veto.**
  - 5 `trap_case`: trap-git-commit-pass, trap-git-push-deny-epolicy,
    trap-mutating-tool-suspend-approval, trap-clock-skew-deadline-suspend,
    trap-unverified-caller-abort-ewxviolation.
  - 1 exec_rewrite: trap-pipe-install-rewrite-execve-mutation (also TrapEvent-shaped, but its own
    category).
  - 1 heartbeat, 2 watchdog (reboot-suppress, terminate-mitigate), 3 veto (fork, suspend,
    clock-skew).

**The work order's "13" was a double-count**: it counted the exec_rewrite case both as one of "6
traps" and again as "1 exec_rewrite". There are only 5 plain trap cases; the 6th trap-shaped case
IS the exec_rewrite case. 5+1+1+2+3 = 12. No duplication, no misplacement, **no edit to
vectors_testkit.rs required**. Matches CANS/15 pins verbatim: "46 builder cases: one per verb (30),
one per scope (8), one per merge (8)" / "58 vectors total: 46 builder cases plus the 12 other
cases" (cans/15-operations.md#--unified-builder-coverage and #Multi-language-parity-gate).

Secondary finding: the only regen-vs-committed diff is `frame_hex` of `trap-git-commit-pass` (the
only vector with a populated `envp` HashMap) — the known CBOR map-ordering nondeterminism already
pinned by `// TODO (L3): canonicalize CBOR map ordering (envp HashMap iteration)` at the `cbor()`
helper. Left untouched (L3).

**Stale 12-pins reconciled to 58** (the count CANS/15 pins), because the red suites would stay red
forever on a stale count even after the L3 parity consumers land:
- `cans/artifacts/test-plan/cases/bindings_py/bp_02_*.yaml` and `bindings_ts/bt_02_*.yaml`:
  `vector_count: 12→58`, "consumes all 12 vectors"→58, diagnostic culprit 12→58, AS-06
  verification snippet 12→58 (authorized: cans/artifacts/test-plan/** for E2 coherence).
- `cans/artifacts/test-plan/case-to-test-map.yaml`: the two red reasons "the 12 ./vectors.json …
  literals"→58.
- The bt_02/bp_02 red suites themselves live in **packages/** (`packages/ts/core/tests/red/
  vectors.bindings.test.ts`, `packages/py/tests/red/test_bindings_vectors.py`), NOT in
  kelir-plugin-testkit as the orchestrator note said — location corrected here. Updated their
  `length !== 12` / `len(...) == 12` asserts and messages to 58 (minimal, orchestrator-authorized
  intent; the suites stay red for their pinned reason: `runParityVectors` /
  `kelir.run_parity_vectors` do not exist yet). `python3 -m py_compile` green on the py file.
- Gates after the edit: check-manifest, check-tests green (see gate log).

## E3 — parity_cli canonical roster

- `parity_cli.rs::assert_canonical_roster` compares the declared vectors.json name list
  (order-sensitive `!=`) against `fixtures_testkit::canonical_vector_names()`, which is
  `VectorSuite::fixtures()?.vectors.iter().map(name)`.
- `VectorSuite::fixtures()` ordering is structurally deterministic: a fixed `vec![…]` base literal
  + three `extend()` calls over fixed slices (`plugin_builder_verb_cases`, `nested`, `merge`);
  HashMap iteration affects only CBOR payload bytes, never names or order.
- **Verified live**: `kelir-plugin-cli parity run` (against committed ./vectors.json) and `parity
  run /tmp/vectors-e2-check.json` (fresh regen) both pass the roster gate: "58 vector(s) …
  frame-validated 58; watchdog-armed 2; veto-registered 3". No change required.

## E4 — plugin tier governance

Live Cargo.toml audit vs CANS/02 (all 13 crates):

| crate | tier | workspace deps | verdict |
|---|---|---|---|
| plugin-mesh | 1 | kelir-interface | legal |
| plugin-mcp | 1 | kelir-interface | legal |
| plugin-supply-chain | 2 | kelir-interface | legal (zero Tier 1 peer import; "zero concrete peer imports within the same tier" bans same-tier edges, downward-only is a permission not a duty) |
| plugin-container | 2 | kelir-interface | legal (same shape) |
| plugin-replay | 2 | kelir-interface (+ dev: driver-rustvmm) | legal — see below |
| plugin-db-proxy | 2 | kelir-interface | legal |
| plugin-wire-ingress | 2 | kelir-interface | legal |
| plugin-vault | 2 | kelir-interface | legal |
| plugin-tool-guard | 2 | kelir-interface, plugin-mcp (T1) | legal downward |
| plugin-ledger | 3 | kelir-interface, kelir-otlp | legal per cans/plugins/ledger.md#OpenTelemetry — see fix |
| plugin-transaction | 3 | kelir-interface, plugin-db-proxy (T2) | legal downward |
| plugin-chaos | 3 | kelir-interface | legal |
| plugin-profile | 3 | kelir-interface | legal |

(async-trait/blake3/serde/tokio etc. are crates.io deps, not workspace crates; the tier rules
govern workspace/plugin deps.)

Gates `check --graph` and `check --layers` were green at baseline. But the per-plugin CI gate
(`just check-plugin <name>` / ci.yml plugin-audit matrix) was RED for two legal shapes, because
`audit_cli.rs::forbidden_workspace_crates` forbade *every* workspace crate that is not a
plugin/interface/testkit:

1. `check -p plugin-ledger` → "depends on forbidden crate `kelir-otlp`". Spec analysis:
   cans/plugins/ledger.md pins OTLP as the **default** ledger sink ("Default ledger sink is OTLP",
   "Config lives in plugin-ledger (`OtelConfig`)"), kelir-otlp is a 388-LOC leaf schema crate
   (deps: kelir-interface + thiserror only), and conflicts.md already records "Merkle-WAL and
   OpenTelemetry nodes state plugin-ledger ownership". The checker wrongly rejected a spec-legal
   edge. **Fix (my lane):** `ALLOWED_SUPPORT_CRATES = ["kelir-otlp"]` in audit_cli.rs, skipped in
   `forbidden_workspace_crates`, `// see: cans/plugins/ledger.md#OpenTelemetry`. This resolves the
   prior conflicts.md open item "folding otel into plugin-ledger is an L3 restructure" — no
   restructure needed; the checker was wrong, not the shape.
2. `check -p plugin-replay` → "dev-depends on forbidden crate `driver-rustvmm`". The dev-dep
   exists solely for the test-plan-mandated integration case lc_23
   (`crates/kelir-plugins/plugin-replay/tests/step_window_replay_test.rs`, `#[ignore]`d KVM-gated,
   uses the real `RustVmmDriver`), matching the integration tier ("In-process engine tests using
   real drivers", matrix.yaml). CANS/02's tier rules govern the plugin's production dep graph;
   production scans (layers_cli over src/) still reject driver references. **Fix (my lane):**
   `ALLOWED_DEV_CRATES = ["driver-rustvmm"]` consulted only in `check_dev_dep`,
   `// see: cans/15-operations.md#Integration-tier`. Regular deps on drivers remain forbidden.

After the fix: **all 13 `check -p plugin-*` green**, plus `check --graph` / `check --layers` green.
No plugin Cargo.toml was touched; no spec ambiguity found, so nothing left code-unchanged for
conflicts arbitration on this item.

## E5 — plugin_supply_chain WxAllocToken::WIRE_LEN

Spec pins no wire layout (only "Verified JIT runtimes receive dynamic one-time MMU allocation
token over vsock"). `WIRE_LEN = 16 + 8 + 32` exactly matches the struct (u128 nonce, u64
issued_at_ns, [u8;32] binary_blake3, all LE). Size left unchanged. Added the mandated one-liner
`// TODO (L3): cross-impl wire sync if a canonical token layout is pinned; see:
cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation` on the const.
(**TODO (L3) +1**, mandated by E5.)

## E6 — plugin_vault MetadataEndpoint::serve

Verified live, no-op confirmed: response body is exactly
`{"token":…,"ttl_seconds":…,"scope":…,"audience":…}` (`ok_response`), matching
cans/plugins/vault.md#Metadata-service-(vault) "Response body: { token, ttl_seconds, scope,
audience }"; `metadata_vsock_port` defaults to `VAULT_METADATA_VSOCK_PORT = 80`
(kelir-interface/src/vsock_mesh.rs:45). No change.

## E7 — plugin_tool_guard decode_tools_list boot failure

Live shape differs from the work order's claim: `decode_tools_list` returns
`Err(PluginError::Halt("tools/list envelope decode pending (L3)"))` (not `Unsupported("L3")`), and
the *first* failing call in `on_vm_ready` is actually `self.mcp.route_request(TOOLS_LIST_QUERY)`
— plugin-mcp's `route_request` is itself a `TODO (L3)` stub returning Halt. So boot-blocking has
TWO intentional L3 stubs upstream of the decode.

Decision (per §0): the deferral is intentional. cans/plugins/tool-guard.md#Immutable-schema-locking
mandates the boot-time fetch + BLAKE3 digest + catalog lock; returning an empty `ToolsListResult`
would (a) not fix boot (`route_request` halts first), (b) fake the spec-mandated lock, and (c) arm
a mutation trap — the first real advertisement would diverge from the locked empty catalog. That
is a band-aid/Goodhart pattern (§5.4/§5.8). Action: added the mandated
`// TODO (L3): boot-time tools/list fetch+lock halts here until route_request and decode_tools_list
land; an empty-catalog boot would fake the spec-mandated lock.` at the `on_vm_ready` call site.
(**TODO (L3) +1**, mandated by E7.)

## E8 — plugin_profile apply_patch_to_base Move/Copy/Test no-op

Site already carried `// TODO (L3): Move/Copy leave the stored base stale …` but with a bare
prose file reference instead of the pointed anchor. Upgraded to the mandated pointed form (single
line): `// TODO (L3): Move/Copy leave the stored base stale (Test is read-only); cross-path base
update needs owner-state verification; see: cans/plugins/profile.md#RFC-6902-dynamic-policy-patching`.
Marker replaced in place — **net TODO (L3) ±0**. RFC 6902 semantics (Move/Copy mutating the base
document) remain L3.

## E9 — plugin_db_proxy intercept_ethernet_frame FIN handling

`// TODO (L3): FIN half-close state machine per RFC 793.` is present at lib.rs:148, directly at
the FIN site after the RST flow-removal (which IS wired at L2). Left as-is. No change.

## E10 — crates/kelir-adapters/mcp-server/src/main.rs

Empty `main` with `// TODO (L3): direct the stdio wire frames to plugin-mcp …` present.
`cargo check -p mcp-server` green (also verified with `--features plugin-mcp-bridge`). No change.

## Gates (final state, all green)

| Gate | Result |
|---|---|
| `cargo run -p kelir-testkit --bin anchors` ×2 | 31 files / 3986 anchors, no drift on 2nd run |
| `cargo test -p kelir-plugin-testkit` | 4 passed, 0 failed (wire_frame_decode_test) |
| `cargo run -p kelir-plugin-cli -- check --graph` | exit 0 |
| `cargo run -p kelir-plugin-cli -- check --layers` | exit 0 |
| `cargo run -p kelir-testkit --bin check-manifest` | 254/254 synchronized; 71 impl / 108 red / 75 gap |
| `cargo run -p kelir-testkit --bin check-tests` | anti-slop lint + filename grammar, zero violations |
| `cargo run -p kelir-testkit --bin scorecard -- check` | "physical reality matches scorecard.md" |
| `cargo check -p mcp-server` | green (default + plugin-mcp-bridge feature) |
| `cargo clippy -p <edited crate> --all-targets -- -D warnings` | green: kelir-plugin-cli, plugin-supply-chain, plugin-tool-guard, plugin-profile (and kelir-plugin-testkit) |
| `cargo fmt -p <edited crate>` | no changes needed |
| `kelir-plugin-cli parity run` (bonus, E3) | 58 frame-validated, roster match |
| `check -p plugin-*` ×13 (bonus, E4) | all green |
| check-spec (`cans` CLI) | **NOT RUN — the `cans` spec-checker binary is not available in this sandbox; CI gate.** New `// see:` anchors manually verified against anchors.json: all 5 resolve (supply-chain#Dynamic-W^X…, profile#RFC-6902…, ledger#OpenTelemetry, 15-operations#Integration-tier, tool-guard#Immutable-schema-locking) |

## Environment hazard encountered (shared target dir)

Mid-session, `cargo clippy -p plugin-supply-chain --all-targets` failed with 5 × E0308
"expected `Box<TrapEvent>`" inside kelir-plugin-testkit — while my worktree's
`GuestToHost::TrapEvent(TrapEvent)` is unboxed. Cause: the shared `CARGO_TARGET_DIR`
(/home/z/my-project/kelir-target) lets a sibling agent's rebuilt kelir-interface rmeta (Agent A
boxes TrapEvent in its worktree) collide with mine — same crate name/version/features → same
metadata hash → artifact clobber. Fix for my runs: `touch crates/kelir-interface/src/lib.rs &&
cargo check -p kelir-interface` to restore my rmeta, then re-run; all gates above are from clean
post-fix runs. Siblings may see the same transient red in reverse; at integration the merged tree
has one kelir-interface and the hazard disappears.

## TODO (L3) count delta

Before: 519. After: **521 (+2)** — both additions explicitly mandated by the work order
(E5 supply-chain WIRE_LEN sync marker, E7 tool-guard on_vm_ready boot-halt marker). E8's marker
replaced an existing one (net 0). No TODO (L3) body was filled.

## Files changed (339ca53..HEAD)

cans/artifacts/test-plan/case-to-test-map.yaml, cases/bindings_py/bp_02_*.yaml,
cases/bindings_ts/bt_02_*.yaml, crates/kelir-plugin-cli/src/audit_cli.rs,
crates/kelir-plugins/plugin-profile/src/lib.rs, crates/kelir-plugins/plugin-supply-chain/src/lib.rs,
crates/kelir-plugins/plugin-tool-guard/src/lib.rs, packages/py/tests/red/test_bindings_vectors.py,
packages/ts/core/tests/red/vectors.bindings.test.ts (9 files, +36/−21).
