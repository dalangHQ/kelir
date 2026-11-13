# Maturity Layer Enforcement: L1 / L2 / L3

Below is the analysis of the current concern leaks plus a concrete, prioritized enforcement plan (checker rules → file renames → L3 stubs). The codebase is already disciplined in most places (`kelir-storage`, `kelir-wire`, `kelir-terminal`, `kelir-checkpoint`, `kelir-host` use `.run.rs` cleanly), but the remaining leaks are concentrated and can be fixed mechanically.

---

## Part 1 — Current State Analysis

### Naming law (as enforced by `kelir-maturity::level_of`)

| Suffix | Level | Meaning |
|---|---|---|
| `.rs` (any, including `lib.rs` / `mod.rs` / `main.rs`) | **L1** | type defs, trait surfaces, re-exports |
| `.flow.rs` | **L2** | orchestration/flow logic |
| `.run.rs` | **L3** | execution/runtime logic |

### Concern leaks found

#### A. `mod.rs` / `lib.rs` carrying L2 logic in **L1-only or L1-L2-only crates**

These are treated as L1 by the checker, but hold `async fn`, `tokio`, `Mutex`, `RwLock`, or large `impl` blocks — a **hard concern leak**. Crates that only allow `["L1","L2"]` cannot legally hold these as `.run.rs`, so the logic must move into `.flow.rs` siblings.

| Crate | File | Approx. logic | Suggested split |
|---|---|---|---|
| `kelir-engine` | `dispatch_plugin/mod.rs` | ~900 lines — `PipelineEngine` | `dispatch_plugin/mod.rs` (re-exports), `dispatch_plugin/engine.flow.rs`, `dispatch_plugin/quarantine.flow.rs`, `dispatch_plugin/registration.flow.rs` |
| `kelir-engine` | `bus/mod.rs` | ~250 lines — `BusDispatcher`, `OwnedBusDispatcher` | `bus/mod.rs`, `bus/dispatcher.flow.rs`, `bus/frame.flow.rs` |
| `kelir-engine` | `planes/storage/mod.rs` | ~350 lines — `StoragePlane` | `planes/storage/mod.rs`, `plane.flow.rs`, `config.flow.rs`, `unbound_io.flow.rs` |
| `kelir-engine` | `planes/wire/mod.rs` | ~400 lines — `WirePlane` + `Unbound*Ops` | `planes/wire/mod.rs`, `plane.flow.rs`, `unbound_ops.flow.rs` |
| `kelir-engine` | `planes/terminal/mod.rs` | ~130 lines | `plane.flow.rs` |
| `kelir-engine` | `planes/checkpoint/mod.rs` | ~100 lines | `plane.flow.rs` |
| `kelir-engine` | `planes/host/mod.rs` | ~100 lines | `plane.flow.rs` |
| `kelir-engine` | `vmm/mod.rs` | ~50 lines — `DriverRouter` | `router.flow.rs` |
| `kelir-engine` | `plugin_registry.rs` | ~100 lines — `resolve_plugin_registrations`, `parse_plugin_manifest` | `plugin_registry/mod.rs` + `resolve.flow.rs` + `manifest.flow.rs` |
| `kelir-interface` | `bin/codegen/merkle_codegen.rs` | emit logic | `.flow.rs` |
| `kelir-interface` | `bin/codegen/otlp_codegen.rs` | emit logic | `.flow.rs` |
| `kelir-interface` | `bin/codegen/py_codegen.rs` | emit logic | `.flow.rs` |
| `kelir-interface` | `bin/codegen/sdk_py.rs` | emit logic | `.flow.rs` |
| `kelir-interface` | `bin/codegen/sdk_ts.rs` | emit logic | `.flow.rs` |
| `kelir-interface` | `bin/codegen/ts_codegen.rs` | emit logic | `.flow.rs` |
| `kelir-interface` | `bin/codegen/wire_codegen.rs` | emit logic | `.flow.rs` |

`naming_codegen.rs` stays `.rs` (pure transform helpers). `main.rs` stays.

#### B. `lib.rs` carrying L2 logic in **L1-L2-L3-allowed** crates

These crates allow the fix, so this is a *softer* leak (still a leak because `.rs` = L1).

| Crate | File | Suggested split |
|---|---|---|
| `kelir-catalog` | `lib.rs` — `CatalogDb` + all DB I/O | `lib.rs` re-exports; `catalog_db.flow.rs` (or `.run.rs` — see Part 3) |
| `kelir-otlp` | `lib.rs` — `OtlpExporter`, `OtlpEncoder`, `OtlpEndpoint`, `phase_spans` | `lib.rs` re-exports; `exporter.flow.rs`, `encoder.flow.rs`, `span.flow.rs` |
| `kelir-plugin-cli` | `main.rs` — 130-line dispatcher | Keep as binary entry, but move `check`, `check_layers`, `check_budgets`, `dispatch` into `dispatch.flow.rs` |
| `plugin-chaos` | `lib.rs` — `FaultTable`, `apply_network_rules`, `apply_storage_rules` | `lib.rs` (trait impl) + `fault_table.flow.rs` |
| `plugin-mcp` | `lib.rs` — `McpEndpointService`, `McpCtxStreamSink` | `lib.rs` + `mcp_service.flow.rs` |
| `plugin-mesh` | `lib.rs` — `MeshState`, `HandshakeDriver`, `decode_peer_handshake`, `MeshCtx` | `lib.rs` + `mesh_state.flow.rs` + `handshake.flow.rs` |
| `plugin-profile` | `lib.rs` — `ProfileState`, `NamespaceRouter`, `JsonPatch` application | `lib.rs` (types only) + `profile_state.flow.rs` + `patch.flow.rs` |
| `plugin-replay` | `lib.rs` — `ReplayState`, `ReplayPmuSampler` | `lib.rs` + `replay_state.flow.rs` |
| `plugin-supply-chain` | `lib.rs` — `WxTokenLedger`, `WxTokenChannel`, `SupplyChainState` | `lib.rs` + `wx_token.flow.rs` + `state.flow.rs` |
| `plugin-tool-guard` | `lib.rs` — `ToolGuardState`, `TraceQuota`, `guard_tool_call` | `lib.rs` + `tool_guard_state.flow.rs` |
| `plugin-transaction` | `lib.rs` — `PeerCoordination` | `lib.rs` + `coordination.flow.rs` |
| `plugin-vault` | `lib.rs` — `MetadataEndpoint`, `serve_metadata_session` | `lib.rs` + `metadata_endpoint.flow.rs` |
| `plugin-wire-ingress` | `lib.rs` — `IngressState`, `IngressRoutingService` | `lib.rs` + `ingress_state.flow.rs` |
| `kelir-bindings/napi` | `lib.rs` — `NativeKelirBuilder` + 10 handle classes | `lib.rs` + `handles.flow.rs` + `builder.flow.rs` |
| `kelir-bindings/pyo3` | `lib.rs` — `PyKelirBuilder` + 10 handle classes | `lib.rs` + `handles.flow.rs` + `builder.flow.rs` |

`plugin-container`, `plugin-db-proxy`, `plugin-ledger` already split correctly — use them as the reference pattern.

#### C. `kelir-adapters/kelir-pack/src/main.rs`

Has `fetch_manifest`, `phase1_scan_headers`, `phase2_emit_erofs` — L3 work-in-progress. This is the clearest L3 target:

```
crates/kelir-adapters/kelir-pack/src/
  main.rs                     (thin: arg parse + dispatch)
  pack.flow.rs                (run orchestration)
  oci.run.rs                  (registry fetch — TODO L3)
  erofs.run.rs                (EROFS emit — TODO L3)
```

#### D. Confirmed clean (no action)

- `kelir/src/**` — `lib.rs` thin, logic in `.flow.rs`. `planes/storage_plane.rs` and `planes/wire_plane.rs` are pure config structs (L1).
- `kelir-storage`, `kelir-storage-io`, `kelir-terminal`, `kelir-checkpoint`, `kelir-host`, `kelir-wire` — `.run.rs` already used for implementations.
- `kelir-shim` — `.run.rs` already correct.
- `kelir-testkit`, `kelir-plugin-testkit` — `.run.rs` correct.

---

## Part 2 — Enforcement Rules for `kelir-maturity`

The current checker only compares filename suffix to `allowed_levels`. It does **not** catch concern leaks. Add a structural check (new module `kelir-maturity/src/structure.rs`) that flags:

### Rule S1 — `.rs` (L1) must be pure

For any `.rs` file **not** ending in `.flow.rs` / `.run.rs`:
- **forbid** `async fn`, `.await`, `tokio::`, `spawn(`, `std::thread::spawn`, `#[tokio::main]`, `#[tokio::test]`
- **forbid** `Mutex<`, `RwLock<` **only when** the file also contains `impl` blocks with `&mut self` methods (pure state-holder types stay L1)
- **hint**: `move to <stem>.flow.rs`

### Rule S2 — `lib.rs` / `mod.rs` must be aggregators

- **forbid** when line count > 120 **and** file contains `impl` blocks with method bodies beyond simple derive/constructor shims
- **hint**: `extract <Type>.flow.rs from <path>`

### Rule S3 — `.flow.rs` (L2) must not spawn runtime

- **forbid** `std::thread::spawn`, `#[tokio::main]`, `tokio::runtime::Runtime::new` in `.flow.rs`
- **hint**: `move to <stem>.run.rs` (this catches L3 logic that leaked into L2)

### Rule S4 — `.run.rs` (L3) placement

- Crates with `allowed_levels = ["L1","L2"]` **must not** contain any `.run.rs` file (already enforced by existing check — keep).
- Crates with `allowed_levels` **including L3** that contain `async fn` + `std::process`/`tokio::fs`/`libc::` in a `.flow.rs` — suggest `.run.rs` rename.

### Rule S5 — `mod.rs` sibling logic

- **forbid** a `mod.rs` in a crate whose `allowed_levels` excludes L2 containing any `impl` with `async fn` (this is the precise signature of the current `kelir-engine` leaks).

Add these to `perf/maturity-policy.toml` as `[level.L1].forbid`, `[level.L2].forbid`, and make **forbid rules error** (currently warn-only). Also extend `kelir-maturity/src/main.rs`:

```rust
// kelir-maturity/src/main.rs
fn cmd_check(args: &[String]) -> ExitCode {
    let strict = args.iter().any(|a| a == "--strict");
    // existing structural check …
    let report = check_tree(&root, &policy);
    let structural = kelir_maturity::structure::check(&root, &policy);
    for fail in structural.fails.iter().chain(report.fails.iter()) {
        print_line(&format!("FAIL {fail}"));
    }
    if strict && !(report.fails.is_empty() && structural.fails.is_empty()) {
        return ExitCode::from(1);
    }
    // …
}
```

Wire `check --strict` into `just check-maturity` and CI matrix.

---

## Part 3 — L3 Stub Preparation

Crates at `["L1","L2","L3"]` that currently have `.flow.rs` for things that must become `.run.rs` once implemented. **Create these files now as stubs** with a structured `TODO (L3) [tag]: …; slop: …` comment so the slop register and maturity checker both see them:

| Crate | File | Purpose |
|---|---|---|
| `kelir-catalog` | `src/catalog_db.run.rs` | SQLite I/O (`open`, `lookup_session`, `unregister_session`, `disk_lock_handoff`, `heartbeat_lease`, `prune_*`) |
| `kelir-otlp` | `src/exporter.run.rs` | gRPC/HTTP export transport (`export_receipt` / `export_metrics` / `export_causal_delta`) |
| `kelir-otlp` | `src/encoder.run.rs` | OTLP protobuf encoding (`encode_*` currently all `NotImplemented`) |
| `plugin-ledger` | `src/wal.run.rs` | LMDB WAL append + Ed25519 leaf signing (`LocalMerkleDbSink::emit_delta`, `LedgerCtx::finalize`) |
| `plugin-tool-guard` | `src/guard.run.rs` | taint-gated approval, JSON-RPC error synthesis (currently `evaluate_tool_call` returns `Ok(Pass)`) |
| `plugin-mesh` | `src/federation.run.rs` | peer publish / await-winner / mTLS handshake framing |
| `plugin-vault` | `src/metadata.run.rs` | metadata HTTP response writer / JSON encoder (currently hand-rolled) |
| `kelir-adapters/kelir-pack` | `src/oci.run.rs` | streaming HTTPS fetch of OCI manifest + layers |
| `kelir-adapters/kelir-pack` | `src/erofs.run.rs` | single-pass EROFS emit + BLAKE3 CAS blob write |
| `kelir-bindings/napi` | `src/handles.run.rs` | async handle impls for `NativeVmHandle` / `NativeSessionHandle` (currently in `lib.rs`) |
| `kelir-bindings/pyo3` | `src/handles.run.rs` | async handle impls for `PySessionHandle` (currently in `lib.rs`) |

Each stub follows the existing `TODO (L3) [tag]: …; slop: …` convention already present in the codebase, so the slop register gate continues to pass.

`.flow.rs` siblings for those should shrink to orchestration only (already covered by Part 2 file moves).

---

## Part 4 — Prioritized Execution Order

| Priority | Action | Effort |
|---|---|---|
| **P0** | Extend `kelir-maturity` with `structure::check` (Rules S1–S5) and switch `forbid` to error under `--strict` | ~1 file |
| **P1** | Split `kelir-engine` `mod.rs` files (biggest concern leak; 9 files) — move `PipelineEngine`, `BusDispatcher`, `StoragePlane`, `WirePlane`, etc. into sibling `.flow.rs` | ~9 moves, mechanical |
| **P1** | Rename `kelir-interface/bin/codegen/*.rs` → `.flow.rs` (7 files) | 7 renames |
| **P2** | Split plugin `lib.rs` files (13 plugins) — each one file split out | ~13 moves |
| **P2** | Split `kelir-catalog/lib.rs`, `kelir-otlp/lib.rs` | 2 files |
| **P2** | Split `kelir-bindings/napi/lib.rs`, `kelir-bindings/pyo3/lib.rs` | 2 files |
| **P3** | Create L3 `.run.rs` stubs (Part 3 table) | 11 stubs |
| **P3** | Split `kelir-adapters/kelir-pack/src/main.rs` into `main.rs` + `pack.flow.rs` | 1 file |
| **P3** | Split `kelir-plugin-cli/src/main.rs` dispatcher into `dispatch.flow.rs` | 1 file |

The invariant after P0–P2 is that:

- every `.rs` in a `["L1","L2"]` crate contains no `async fn`, `tokio::`, `spawn`, or `Mutex`-with-`&mut self` impls;
- every `lib.rs`/`mod.rs` is a thin aggregator;
- every `.flow.rs` that spawns threads or blocks on runtime is renamed `.run.rs`;
- every `["L1","L2","L3"]` crate has explicit `.run.rs` files for its I/O surface.

That lets L3 proceed by filling stubs without re-touching the L2 layer — the exact goal stated.

# CI Gating: Where It's Actually Weak, and How to Close Each Hole

You're right — several gates are **fail-open** right now. Below is the exact set, ranked by impact, with the minimal patch for each.

---

## Part 1 — Fail-Open Points (with file references)

| Gate | Current behavior | Where it fails open | Impact |
|---|---|---|---|
| `kelir-maturity check` | `forbid` rules → `report.warns` | `crates/kelir-maturity/src/lib.rs` ~line 190 | **Highest** — no content rule can fail CI today |
| `kelir-maturity check` | `l2_max_delta_per_pr` parsed | `Policy::from_toml` stores it; `check_tree` never reads it | No budget enforcement |
| `kelir-maturity check` | No structural rules | Missing `structure.rs` module | No concern-leak detection at all |
| `kelir-plugin-cli check --layers` | Only 3 hardcoded prefixes | `crates/kelir-plugin-cli/src/layers_cli.run.rs` `RULES` | Tier-to-tier rule stubbed (`// TODO (L3) [pe_01_…]`) |
| `check-manifest` | `quadrant` field parsed | `CaseEntry.quadrant: Option<String>` never read | No quadrant validation |
| `check-manifest` | `target_tier`/`target_crate` cross-checks manifest ↔ case file only | Not against the tier roster | A case can target a nonexistent crate as long as both sides agree |
| `check-tests` | `harness_exempt_files` returns `HashSet::new()` on parse error | `check_tests.run.rs` `harness_exempt_files` | **Fails open**: a malformed map lets every test file pass exemption |
| `scorecard` | `CaseEntry { case_id, quadrant, title }` parsed | `scorecard_testkit.run.rs` | `quadrant`/`title` never validated, no drift detection |

The **kelir-maturity** gap is the dominant one — it's the only gate with content-level rules, and those rules cannot currently fail.

---

## Part 2 — The Patch Set

### P0.1 — Promote `forbid` → `fails` in `kelir-maturity`

**`crates/kelir-maturity/src/lib.rs`** (replace the two rule-scanning loops):

```rust
// BEFORE
for r in policy.rules_for(level, true) {
    for (n, line) in src.lines().enumerate() {
        if r.regex.is_match(line) {
            report.warns.push(format!(
                "{}:{}: {} (forbid, warn-only this wave)",
                file.display(), n + 1, r.id
            ));
        }
    }
}

// AFTER
for r in policy.rules_for(level, true) {
    for (n, line) in src.lines().enumerate() {
        if r.regex.is_match(line) {
            let hint = r.hint.as_deref().unwrap_or("");
            report.fails.push(format!(
                "{}:{}: [forbid:{}] {}",
                file.display(), n + 1, r.id, hint
            ));
        }
    }
}
```

Add a `[policy] forbid_mode = "warn" | "error"` toggle in `perf/maturity-policy.toml` — default `"error"`. Phased rollout only if the codebase isn't green yet; the default must be `"error"` so a future contributor cannot silently reintroduce the leak.

### P0.2 — Add `kelir-maturity/src/structure.rs` (new)

This is the missing enforcement for the concern leaks you flagged. It applies the L1/L2/L3 contract *structurally*, not just by filename.

```rust
// crates/kelir-maturity/src/structure.rs
#![forbid(unsafe_code)]

use std::path::Path;
use std::fs;
use crate::{level_of, Level, Policy, CheckReport, crate_of};

/// Rules applied to every .rs file, independent of filename suffix.
pub fn check_file(path: &Path, policy: &Policy, report: &mut CheckReport) {
    let Ok(src) = fs::read_to_string(path) else { return };
    let level = level_of(path);
    let lines = src.lines().count();

    // S1: L1 (.rs) must be pure — no async fn / .await / tokio:: / spawn()
    if level == Level::L1 && has_runtime_concern(&src) {
        report.fails.push(format!(
            "{}: [structure:S1] L1 file carries runtime concern (async fn/.await/tokio::/spawn); move to <stem>.flow.rs",
            path.display()
        ));
    }

    // S2: lib.rs / mod.rs must be aggregators (≤120 lines, no impl bodies with &mut self)
    if is_aggregator(path) && lines > 120 && has_mutating_impl(&src) {
        report.fails.push(format!(
            "{}:{}: [structure:S2] lib.rs/mod.rs is not an aggregator; extract the impl to <Type>.flow.rs",
            path.display(), lines
        ));
    }

    // S3: .flow.rs (L2) must not spawn runtime or use tokio::runtime::Runtime
    if level == Level::L2 && (src.contains("std::thread::spawn")
        || src.contains("#[tokio::main]")
        || src.contains("tokio::runtime::Runtime::new")
        || src.contains("tokio::runtime::Builder"))
    {
        report.fails.push(format!(
            "{}: [structure:S3] L2 file spawns a runtime; move to <stem>.run.rs (requires allowed_levels += L3)",
            path.display()
        ));
    }

    // S4: L3-allowed crate with async fn + real I/O in .flow.rs → require .run.rs
    if level == Level::L2 && crate_allows_l3(&crate_of(path))
        && has_runtime_concern(&src)
        && has_io_concern(&src)
    {
        report.fails.push(format!(
            "{}: [structure:S4] .flow.rs carries I/O (std::fs/tokio::fs/libc/std::process) inside an L3-allowed crate; move to <stem>.run.rs",
            path.display()
        ));
    }

    // S5: mod.rs with async impls is always a leak
    if is_aggregator(path) && src.contains("async fn") && has_mutating_impl(&src) {
        report.fails.push(format!(
            "{}: [structure:S5] mod.rs carries async impl bodies; extract to <impl>.flow.rs",
            path.display()
        ));
    }
}

fn has_runtime_concern(src: &str) -> bool {
    src.contains("async fn")
        || src.contains(".await")
        || src.contains("tokio::")
        || src.contains("std::thread::spawn")
        || src.contains("#[tokio::main]")
        || src.contains("#[tokio::test]")
}

fn has_io_concern(src: &str) -> bool {
    src.contains("std::fs::")
        || src.contains("tokio::fs::")
        || src.contains("libc::")
        || src.contains("std::process::")
        || src.contains("rusqlite::")
}

fn has_mutating_impl(src: &str) -> bool {
    // A crude but effective detector: an `impl` block containing `&mut self`.
    src.split("impl").skip(1).any(|block| {
        let body = block.split('{').next().unwrap_or("");
        body.contains("&mut self")
    })
}

fn is_aggregator(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|n| n.to_str()),
        Some("lib.rs" | "mod.rs")
    )
}

fn crate_allows_l3(crate_dir: &Path) -> bool {
    let levels = crate::read_allowed_levels(&crate_dir.join("Cargo.toml"));
    levels.contains(&Level::L3)
}
```

Wire into `check_tree`:

```rust
// kelir-maturity/src/lib.rs
pub fn check_tree(root: &Path, policy: &Policy) -> CheckReport {
    let mut report = CheckReport::default();
    for file in rs_files(root) {
        report.files += 1;
        // existing placement + content checks…
        crate::structure::check_file(&file, policy, &mut report); // NEW
    }
    report
}
```

Expose as a public module:

```rust
// kelir-maturity/src/lib.rs
pub mod structure;
```

### P0.3 — Enforce `l2_max_delta_per_pr`

**`crates/kelir-maturity/src/main.rs`** — add a `--base` flag and a delta counter:

```rust
fn cmd_check(args: &[String]) -> ExitCode {
    let root = flag(args, "--root").map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let strict = args.iter().any(|a| a == "--strict");
    let policy = load_policy(&root);
    let mut report = check_tree(&root, &policy);

    // NEW: L2 delta budget vs the PR base
    if let Some(base) = flag(args, "--base") {
        match l2_delta_lines(&root, Path::new(&base)) {
            Ok(delta) => {
                let max = policy.l2_max_delta_per_pr;
                if max > 0 && delta > max as u64 {
                    report.fails.push(format!(
                        "L2 delta budget exceeded: {} .flow.rs lines changed vs {} (max {})",
                        delta, base, max
                    ));
                }
            }
            Err(e) => {
                report.fails.push(format!("L2 delta: {e}"));
            }
        }
    }

    for fail in &report.fails { print_line(&format!("FAIL {fail}")); }
    // …warns, summary…
    if report.passed() || (!strict && report.fails.is_empty()) {
        ExitCode::from(if report.passed() { 0 } else { 1 })
    } else {
        ExitCode::from(1)
    }
}

fn l2_delta_lines(root: &Path, base: &Path) -> Result<u64, String> {
    // Count lines changed in *.flow.rs between base and working tree.
    let output = std::process::Command::new("git")
        .arg("-C").arg(root)
        .arg("diff").arg("--numstat").arg(base)
        .arg("--").arg("*.flow.rs")
        .output()
        .map_err(|e| format!("git diff failed: {e}"))?;
    let mut total = 0u64;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let mut parts = line.split('\t');
        let (Some(add), Some(del)) = (parts.next(), parts.next()) else { continue };
        total += add.parse::<u64>().unwrap_or(0);
        total += del.parse::<u64>().unwrap_or(0);
    }
    Ok(total)
}
```

`--strict` makes the process non-zero on any `fails`; without `--strict`, only structural content failures fail. In CI, always run `--strict`.

### P0.4 — Extend `perf/maturity-policy.toml` with the concern rules

```toml
# perf/maturity-policy.toml
[budget]
# L2 delta budget per PR; enforced by `kelir-maturity check --base origin/main`
L2_max_delta_per_pr = 800

[policy]
forbid_mode = "error"

[level.L1]
forbid = [
  # S1: any L1 purity violation, expressed as a rule for the existing content pass
  { id = "pe_20_l1_async", pattern = "async fn .*\\bimpl\\b", hint = "move to <Type>.flow.rs" },
  { id = "pe_20_l1_spawn", pattern = "std::thread::spawn|tokio::spawn", hint = "move to <stem>.run.rs" },
  { id = "pe_20_l1_await", pattern = "\\.await\\b", hint = "move to <stem>.flow.rs" },
]

[level.L2]
forbid = [
  { id = "pe_20_l2_spawn", pattern = "std::thread::spawn|#\\[tokio::main\\]", hint = "move to <stem>.run.rs" },
  { id = "pe_20_l2_runtime", pattern = "tokio::runtime::Runtime::new|tokio::runtime::Builder", hint = "move to <stem>.run.rs" },
]

[level.L3]
warn = [
  { id = "pe_20_l3_todo", pattern = "// TODO \\(L3\\)", hint = "L3 stubs are visible in `kelir-maturity list --level L3`" },
]
```

`kelir-maturity check --strict` now **fails CI** on any `forbid` match. It also fails on structural rules S1–S5.

---

### P1.1 — Implement the tier-to-tier rule in `layers_cli`

**`crates/kelir-plugin-cli/src/layers_cli.run.rs`** — replace the stub `check()`:

```rust
pub fn check() -> Result<(), CliError> {
    // Part A: existing prefix bans (kept for defense in depth)
    let root = crate::workspace_root()?;
    for (path, forbidden) in RULES {
        check_dir(&root.join(path), forbidden)?;
    }

    // Part B: per-plugin tier-to-tier rule per cans/02-plugin-model.md#Plugin-tiering
    let roster = crate::audit_cli::tier_roster()?;
    let mut violations = Vec::new();
    for plugin in &roster {
        for dep in &plugin.deps {
            if let Some(dep_tier) = crate::audit_cli::tier_of(&roster, dep) {
                if dep_tier.level >= plugin.tier.level {
                    violations.push(format!(
                        "tier violation: `{}` (tier {}) depends on `{}` (tier {}); dependencies must be downward-only",
                        plugin.tier.crate_name, plugin.tier.level,
                        dep_tier.crate_name, dep_tier.level,
                    ));
                }
            }
        }
        for dep in &plugin.dev_deps {
            if let Some(dep_tier) = crate::audit_cli::tier_of(&roster, dep) {
                if dep_tier.level >= plugin.tier.level {
                    violations.push(format!(
                        "tier violation: `{}` (tier {}) dev-depends on `{}` (tier {})",
                        plugin.tier.crate_name, plugin.tier.level,
                        dep_tier.crate_name, dep_tier.level,
                    ));
                }
            }
        }
    }
    if !violations.is_empty() {
        return Err(violations.join("\n").into());
    }
    Ok(())
}
```

This also clears the existing `// TODO (L3) [pe_01_…]` in `layers_cli.run.rs`.

The helper functions `tier_roster` and `tier_of` already exist in `audit_cli.run.rs` (private). Promote them to `pub(crate)` — no behavioral change, just visibility.

### P1.2 — Close the fail-open `harness_exempt_files`

**`crates/kelir-testkit/src/bin/check_tests.run.rs`** — replace:

```rust
// BEFORE
fn harness_exempt_files(repo_root: &Path) -> HashSet<String> {
    let Ok(raw) = fs::read_to_string(&map_path) else { return HashSet::new(); };
    let Ok(map) = serde_yaml::from_str::<CaseMap>(&raw) else { return HashSet::new(); };
    // …
}

// AFTER
fn harness_exempt_files(repo_root: &Path) -> Result<HashSet<String>, String> {
    let map_path = repo_root.join("cans/artifacts/test-plan/case-to-test-map.yaml");
    let raw = fs::read_to_string(&map_path)
        .map_err(|e| format!("check-tests: cannot read {}: {e}", map_path.display()))?;
    let map: CaseMap = serde_yaml::from_str(&raw)
        .map_err(|e| format!("check-tests: cannot parse {}: {e}", map_path.display()))?;
    Ok(map.harness_tests.into_iter().filter(|t| !t.file.is_empty()).map(|t| t.file).collect())
}

// in main():
let harness_exempt = match harness_exempt_files(&repo_root) {
    Ok(set) => set,
    Err(e) => { eprintln!("ERROR: {e}"); return ExitCode::FAILURE; }
};
```

This clears the `// TODO (L3) [governance_tools]: fail closed when the typed map parse fails` comment in `check_tests.run.rs`.

### P1.3 — Validate `quadrant` in `check-manifest`

**`crates/kelir-testkit/src/bin/check_manifest.run.rs`** — add the missing check:

```rust
const QUADRANTS: &[&str] = &["happy-path", "sad-path", "security", "performance"];

// in main(), inside the per-case loop:
match &case.quadrant {
    Some(q) if QUADRANTS.contains(&q.as_str()) => {}
    Some(q) => {
        eprintln!("ERROR: {}: invalid quadrant: {}", case.case_id, q);
        errors += 1;
    }
    None => {
        eprintln!("ERROR: {}: missing quadrant", case.case_id);
        errors += 1;
    }
}
```

Also add a `target_crate` validation against the tier roster:

```rust
// after roster is available
let roster = crate::audit_cli::tier_roster()?;
let roster_names: std::collections::HashSet<&str> =
    roster.iter().map(|p| p.tier.crate_name.as_str()).collect();

match &case.target_crate {
    Some(krate) if valid_target_crate(&repo_root, krate) && roster_names.contains(krate.as_str()) => {}
    Some(krate) => {
        eprintln!("ERROR: {}: target_crate `{}` not in tier roster", case.case_id, krate);
        errors += 1;
    }
    None => { /* existing missing-field error */ }
}
```

This closes `// TODO (L3) [governance_tools]: read CaseEntry.quadrant in the manifest gate`.

### P1.4 — Read and validate `quadrant`/`title` in `scorecard`

**`crates/kelir-testkit/src/bin/scorecard_testkit.run.rs`** — extend the coverage matrix rendering to include the `quadrant` and `title` fields already in `CaseEntry`:

```rust
out.push_str("| Domain | Prefix | Cases | Happy | Sad | Security | Performance | Status |\n");
out.push_str("|---|---|---:|---:|---:|---:|---:|---|\n");
for (name, domain) in &manifest.domains {
    let domain_cases: Vec<&CaseEntry> = manifest.cases.iter().filter(|c| &c.domain == name).collect();
    let count = |q: &str| domain_cases.iter().filter(|c| c.quadrant == q).count();
    let status = if domain_cases.len() == domain.case_count { "pass" } else { "drift" };
    out.push_str(&format!(
        "| `{}` | `{}` | {} | {} | {} | {} | {} | {} |\n",
        name, domain.prefix, domain_cases.len(),
        count("happy-path"), count("sad-path"), count("security"), count("performance"),
        status
    ));
}
```

Also assert `title` is non-empty and doesn't start with `TODO`:

```rust
if case.title.trim().is_empty() || case.title.starts_with("TODO") {
    eprintln!("ERROR: {}: title is empty or a TODO placeholder", case.case_id);
    errors += 1;
}
```

This closes `// TODO (L3) [governance_tools]: read CaseEntry case_id/quadrant/title in the scorecard gate`.

---

## Part 3 — CI Command Matrix

CI must run **all** of these as hard gates. Order: cheap → expensive, so failures surface fast.

| Step | Command | Fails on |
|---|---|---|
| 1 | `cargo run -p kelir-maturity --bin kelir-maturity -- check --base origin/main --strict` | placement + **forbid** content rules + **structural rules S1–S5** + **L2 delta budget** |
| 2 | `cargo run -p kelir-plugin-cli -- check --layers` | prefix bans + **per-plugin tier-to-tier** |
| 3 | `cargo run -p kelir-plugin-cli -- check --graph` | dependency cycles / forbidden crates |
| 4 | `cargo run -p kelir-plugin-cli -- check --budgets` | perf budgets |
| 5 | `for p in $(ls crates/kelir-plugins); do cargo run -p kelir-plugin-cli -- check -p $p; done` | per-plugin tier/deps/rings |
| 6 | `cargo run -p kelir-testkit --bin check-manifest` | manifest ↔ disk sync, **quadrant**, `target_crate ∈ roster`, TODO tags ↔ register |
| 7 | `cargo run -p kelir-testkit --bin check-tests` | anti-slop AS-01..AS-08, filename grammar, **fail-closed on map parse** |
| 8 | `cargo run -p kelir-testkit --bin check-scenarios` | scenario tier + adjacent plane coverage |
| 9 | `cargo run -p kelir-testkit --bin scorecard -- check` | scorecard drift incl. **quadrant counts + title** |
| 10 | `cargo run -p kelir-plugin-cli -- parity run` | parity vectors |
| 11 | `cargo run -p kelir-plugin-cli -- fixtures --verify` | fixture digests |

Add to `just`:

```make
check-maturity:
    cargo run -p kelir-maturity --bin kelir-maturity -- check --base origin/main --strict

check-all: check-maturity
    cargo run -p kelir-plugin-cli -- check --layers
    cargo run -p kelir-plugin-cli -- check --graph
    cargo run -p kelir-plugin-cli -- check --budgets
    cargo run -p kelir-testkit --bin check-manifest
    cargo run -p kelir-testkit --bin check-tests
    cargo run -p kelir-testkit --bin check-scenarios
    cargo run -p kelir-testkit --bin scorecard -- check
    cargo run -p kelir-plugin-cli -- parity run
    cargo run -p kelir-plugin-cli -- fixtures --verify
```

CI `.github/workflows/ci.yml`:

```yaml
- name: Maturity gate (strict)
  run: cargo run -p kelir-maturity --bin kelir-maturity -- check --base origin/${{ github.base_ref }} --strict
- name: Layer gate
  run: cargo run -p kelir-plugin-cli -- check --layers
- name: Graph gate
  run: cargo run -p kelir-plugin-cli -- check --graph
# …remaining gates…
```

---

## Part 4 — Rollout Order

| Wave | Changes | Blocks on |
|---|---|---|
| **Wave 1** | `kelir-maturity/structure.rs` + `--strict` + `forbid` → `fails` + `--base` L2 budget + extended `maturity-policy.toml` + `layers_cli` tier rule | None — this makes the gate real |
| **Wave 2** | `check-manifest` quadrant + `target_crate ∈ roster` + `check-tests` fail-closed parse + `scorecard` quadrant/title | Wave 1 green (else the new gate fails Wave 1's own code) |
| **Wave 3** | Apply the file moves from the earlier analysis (`kelir-engine/*/mod.rs` → `.flow.rs`, `kelir-catalog/lib.rs` → `catalog_db.run.rs`, etc.) so `--strict` passes | Wave 1 only — the gate is live; violations surface as CI red |
| **Wave 4** | Create the L3 stubs (`.run.rs` files from the previous analysis) so `kelir-maturity list --level L3` shows the surface | Wave 3 green |

**The key property of Wave 1:** once `--strict` is live and `forbid` is promoted, no new concern leaks can be introduced. Wave 3 then proceeds as clean-up, not as new enforcement work.

---

## What "prepare L3 as files" means for CI, concretely

After Waves 1–4, the invariant is enforceable by machine:

1. **L3 stubs exist** → `kelir-maturity list --level L3` enumerates them, and `check-manifest` sees each `TODO (L3) [tag]` in the slop register. If a `.run.rs` stub is missing for a required trait implementation, `structure::check_file` (Rule S4) flags the `.flow.rs` that *would* host the trait impl.
2. **L2 cannot regress into L3** → Rules S3, S4, S6 fail any `.flow.rs` that spawns a runtime, calls `tokio::runtime::Runtime::new`, or uses I/O in an L3-allowed crate.
3. **L1 cannot regress into L2** → Rules S1, S2, S5 fail any `lib.rs`/`mod.rs`/pure `.rs` carrying `async fn`, `.await`, or `tokio::`.
4. **L2 growth is bounded** → `l2_max_delta_per_pr` in `maturity-policy.toml` fails the `--base` check if a PR adds too much `.flow.rs` logic without moving into `.run.rs`.

That's the point where CI transitions from "checks a filename pattern" to "enforces the maturity contract" — and the L3 stubs you create are already routed into the enforcement surface (`check-manifest`'s slop register, `kelir-maturity list --level L3`, and the S4 rule that requires `.run.rs` for the trait impls).