```markdown
# work-order.md — Filename-Driven Maturity

**Read first:** [`plan.md`](plan.md), then [`AGENTS.md`](AGENTS.md) §0, §2, §6, then [`cans/AGENTS.md`](cans/AGENTS.md) (the CANS rules you must obey while editing `cans/`).

---

## Mission

Land plan.md in one coordinated wave. Five agents, each owning a vertical
slice. Every slice edits, in order: **`cans/` → `cans/artifacts/` → codebase**.

The maturity axis is orthogonal to test status, plugin tier, and target tier.
Do not conflate them. Do not invent an "L4".

---

## Plan corrections (do not silently follow plan.md)

Three points in plan.md need interpreting before any agent starts.

1. **The checker binary location.** plan.md says `crates/kelir-engine/src/bin/maturity.rs`. That file is L3-shaped (`std::fs`, `walkdir`). `kelir-engine = ["L1","L2"]` forbids `std::fs` in L2. **Work-order:** the checker goes in a new binary crate `crates/kelir-maturity/` with `allowed_levels = ["L1","L2","L3"]`.
2. **`kelir-interface` stays `["L1","L2"]`.** plan.md tightens it to `["L1"]`, which forbids `impl` blocks. `kelir-interface` has `impl` blocks today (`Errno::as_str`, `From<PluginError> for ErrorEnvelope`, serde impls). Tightening is deferred to a follow-up PR that amends the L1 policy.
3. **`.run.rs` rename is deferred to the L3 phase.** Every file we move in this wave is a stub returning `Halt` with a `// TODO (L3) [tag]: ...; slop: ...` comment. That comment is forbidden in `.run.rs` per plan.md §The-boundary. **Work-order:** all moved stubs land as `.flow.rs` (L2). The `.run.rs` rename happens when L3 bodies are actually written. `allowed_levels = ["L1","L2","L3"]` still lands now so those renames are one `mv` later.

---

## Coordination

| Rule | Detail |
|---|---|
| Branch per agent | `maturity/agent-<n>` |
| Worktree per agent | one worktree per branch; no shared checkout |
| `CARGO_TARGET_DIR` | `target/agent-<n>` — eliminates Cargo lock contention |
| Merge order | **1 → 5a → (2, 3, 4 in any order) → 5b** |
| Parallel start | 2, 3, 4 open their branches immediately; they rebase onto `main` after 1 and 5a land, before opening PR |
| Shared files | Only the owning agent lands edits to shared files (`cans/01-architecture.md`, `cans/artifacts/config/project-structure.txt`, root `Cargo.toml`, `AGENTS.md`, `perf/maturity-policy.toml`). If your slice needs a shared-file edit, note it in your PR body; the owning agent folds it in. |
| Push | never from an agent. Converge by squash-merge into one commit per agent. |
| Rebase | every open agent rebases onto `main` after every merge |
| CI | a PR is not opened until the gates below pass locally on the rebased branch |

---

## Merge chain

```
main
 ├── Agent 1  (maturity tooling)                        ✓ vacuous pass everywhere
 ├── Agent 5a (rename kelir-core → kelir-engine)        ✓ everyone now writes `kelir_engine::`
 ├── Agent 2  (storage split)                           ┐
 ├── Agent 3  (wire split)                              ├ independent, any order
 ├── Agent 4  (terminal + checkpoint + host split)      ┘
 └── Agent 5b (composition root + final crate topology)
```

---

## Agent 1 — Maturity tooling

**Blocks on:** nothing. **Blocks:** every other agent's local gate.

### Spec (`cans/`)

- `cans/15-operations.md`
  - **§Developer-CLI**: add a `kelir-maturity` subcommand list: `check`, `list --level <L> --crate <crate>`, `list --tag <slop-tag>`, `emit --out maturity/`. Note it runs in <2s and is the only gate for body maturity.
  - **§CI-matrix**: add a `maturity-check` job to the enforced-invariants list; add it to the per-PR gates.
- `cans/01-architecture.md`
  - **§Codebase-invariants**: add a `Maturity discipline` subsection — one sentence per invariant:
    - Level = filename suffix (`.rs` = L1, `.flow.rs` = L2, `.run.rs` = L3).
    - Crate role = `[package.metadata.kelir].allowed_levels`.
    - Placement invariant: `level_of(file) ∈ crate.allowed_levels`, enforced by `kelir-maturity check`.
    - The four axes (maturity, test status, plugin tier, target tier) are orthogonal.

### Artifacts (`cans/artifacts/`)

- `perf/maturity-policy.toml` — new file. Copy the policy verbatim from plan.md §The-policy-file.
- `cans/artifacts/config/justfile` — add `check-maturity` target: `cargo run -p kelir-maturity -- check`.
- `cans/artifacts/config/ci.yml` — add `maturity-check` job to the matrix; wire to `just check-maturity`.
- `cans/artifacts/config/project-structure.txt` — add `crates/kelir-maturity/` with `Cargo.toml`, `src/main.rs`, `src/lib.rs`.

### Codebase

- New crate `crates/kelir-maturity/`:
  - `Cargo.toml` — `[package] name = "kelir-maturity"`, `[package.metadata.kelir] allowed_levels = ["L1","L2","L3"]`; deps: `serde`, `toml`, `regex`, `walkdir`.
  - `src/lib.rs` — `level_of(&Path) -> Level`, `crate_of(&Path) -> PathBuf`, `read_allowed_levels(&CargoToml) -> Vec<Level>`, `Policy::from_toml(&str) -> Policy`. ~60 lines.
  - `src/main.rs` — `check`, `list`, `emit` subcommands; the ~80-line `check()` from plan.md §The-check-binary.
- Root `Cargo.toml` — add `crates/kelir-maturity` to `[workspace.members]`.
- Every `crates/*/Cargo.toml` — add `[package.metadata.kelir] allowed_levels = ["L1","L2","L3"]`. Every existing crate. Vacuously passes today; agents 2/3/4 tighten their own new crates in their PRs.

### Gates

```sh
cargo check --workspace --all-targets
cargo run -p kelir-maturity -- check    # must exit 0 (vacuous)
cargo run -p kelir-maturity -- list --level L2 --crate kelir-interface  # sanity
just check-spec
```

---

## Agent 5a — Rename `kelir-core` → `kelir-engine`

**Blocks on:** nothing (parallel with Agent 1). **Blocks:** Agents 2, 3, 4's `Cargo.toml` edits, `kelir/src/lib.rs`, and every `use kelir_core::`.

### Spec (`cans/`)

Pure `sed`: `kelir-core` → `kelir-engine`, `kelir_core` → `kelir_engine` across:
- `cans/01-architecture.md`, `cans/02-plugin-model.md`, `cans/03-sdk-architecture.md`
- `cans/11-protocol.md`, `cans/12-adapters.md`, `cans/13-cli.md`, `cans/14-observability.md`
- `cans/00-overview.md`, `cans/06-storage-catalog.md`, `cans/10-attestation.md`
- `cans/15-operations.md`

Do **not** change `KELIR_CORE_VERSION` (the const stays named) or the `bump-core` CLI command. Only the crate name renames.

### Artifacts (`cans/artifacts/`)

Same `sed` over:
- `cans/artifacts/config/project-structure.txt`
- `cans/artifacts/diagrams/host-runtime.txt`
- `cans/artifacts/code/plugin-trait.rs`
- `cans/artifacts/code/vmm-driver-trait.rs`
- `cans/artifacts/code/step-evaluation-loop.ts`
- any other file that references `kelir-core` or `kelir_core`

### Codebase

```sh
git mv crates/kelir-core crates/kelir-engine
```

Then:
- `crates/kelir-engine/Cargo.toml` — `name = "kelir-engine"`; leave `allowed_levels` as Agent 1 set it.
- Root `Cargo.toml` — `[workspace.members]` line updated.
- Every `Cargo.toml` in `crates/`, `packages/ts/*/package.json` (if it refs), `crates/kelir-shim/Cargo.toml` — dependency line `kelir-core = { path = ... }` → `kelir-engine = { path = ... }`.
- Every `.rs` file — `sed -i 's/kelir_core/kelir_engine/g; s/kelir-core/kelir-engine/g'` across `crates/`, `packages/`. Re-grep to confirm zero hits.
- `crates/kelir-engine/src/lib.rs` — `pub const KELIR_CORE_VERSION` stays; `#![forbid(unsafe_code)]` stays.
- `crates/kelir/src/lib.rs` — `pub use kelir_core::*;` → `pub use kelir_engine::*;`.
- `AGENTS.md` §7 — `crates/kelir-interface/ ... crates/kelir-core/` → `crates/kelir-engine/`.

**Do not** touch composition root wiring (`RealFuseOps::new(...)` etc.) — that is Agent 5b.

### Gates

```sh
cargo check --workspace --all-targets
cargo run -p kelir-maturity -- check
grep -rn "kelir_core\|kelir-core" crates/ packages/   # must be empty
just check-spec
```

---

## Agent 2 — Storage plane split

**Blocks on:** Agents 1, 5a. **Blocks:** Agent 5b (composition root).

### Spec (`cans/`)

- `cans/05-storage-plane.md`
  - **§Storage-architecture**: replace "Owner: kelir-core native storage subsystem (kelir-core::storage)" with "Owner: `kelir-storage` (L3) + `kelir-storage-io` (L3) + `kelir-engine::planes::storage` (L2)".
  - Add a `Crate topology` node under §Storage-architecture listing the three crates and the three-level split per plan.md §Target-crate-layout.
  - Add a `Trait seam` node under §Storage-architecture: `FuseOps` and `StorageIo` live in `kelir-interface::storage`; the L2 façade holds `Arc<dyn FuseOps>`; the L3 impl is `RealFuseOps`; `kelir-engine` cannot name `RealFuseOps`.
  - Every `see:` that currently points at `fuse_storage.rs` / `storage_reconciler.rs` / `io_storage.rs` / `lock_broker.rs` / `storage_synthetic.rs` retargets at `crates/kelir-storage/src/*.flow.rs` or `crates/kelir-storage-io/src/*.flow.rs`.

### Artifacts (`cans/artifacts/`)

- `cans/artifacts/diagrams/storage-architecture.txt` — update crate labels.
- `cans/artifacts/code/reconciler.rs` — note new path; keep body.
- `cans/artifacts/config/project-structure.txt` — add `crates/kelir-storage/`, `crates/kelir-storage-io/`; delete the `crates/kelir-engine/src/planes/storage/` sub-tree entries that moved.

### Codebase

New crates:
- `crates/kelir-interface/src/storage/mod.rs` — `pub mod fuse; pub mod io;`
- `crates/kelir-interface/src/storage/fuse.rs` (L1, ~15 lines) — `FuseOps` trait; move `FuseRequest`, `FuseReply`, `FuseResponse`, `FuseOpcode`, `FuseDirEntry` here from `kelir-core/src/planes/storage/fuse_storage.rs`.
- `crates/kelir-interface/src/storage/io.rs` (L1, ~20 lines) — `StorageIo` trait, `RootHandle`, `HostFileLock`; move from `kelir-core/src/planes/storage/io_storage.rs`.
- `crates/kelir-interface/Cargo.toml` — `allowed_levels = ["L1","L2"]` (not `["L1"]`, see Plan corrections).

- `crates/kelir-engine/src/planes/storage/mod.rs` (L2) — `FuseDispatcher` façade holding `Arc<dyn FuseOps>`, `Arc<dyn StorageIo>`, `StorageQuota`, lock broker config, metrics. Delegates to the ops.
- `crates/kelir-engine/src/planes/storage/fuse.flow.rs` (L2) — `FuseDispatcher::dispatch` from `fuse_storage.rs`.
- `crates/kelir-engine/src/planes/storage/storage_quota.flow.rs` (L2) — `StorageQuota` (today `kelir-core/src/planes/storage/mod.rs`).

- New `crates/kelir-storage/` (L3):
  - `Cargo.toml` — `name = "kelir-storage"`, `allowed_levels = ["L1","L2","L3"]`, deps: `kelir-interface`, `kelir-engine`, `kelir-storage-io`, `kelir-otlp`.
  - `src/lib.rs` (L2) — `pub use real_fuse::RealFuseOps;` etc.
  - `src/fuse.flow.rs` (L2 for now; renames to `fuse.run.rs` when the body lands) — move `kelir-core/src/planes/storage/fuse_storage.rs` body here; keep the `// TODO (L3) [...]` markers.
  - `src/reconciler.flow.rs`, `src/lock_broker.flow.rs`, `src/storage_synthetic.flow.rs` — move `kelir-core/src/planes/storage/storage_reconciler.rs`, `lock_broker.rs`, `storage_synthetic.rs` bodies here.

- New `crates/kelir-storage-io/` (L3):
  - `Cargo.toml` — `name = "kelir-storage-io"`, `allowed_levels = ["L1","L2","L3"]`, deps: `kelir-interface`, `libc`.
  - `src/lib.rs` (L2) — `pub use real_io::RealStorageIo;`
  - `src/io.flow.rs` (L2 for now) — move `kelir-core/src/planes/storage/io_storage.rs` body here (the `UnimplementedStorageIo` impl), rename to `RealStorageIo`.

- Deletions:
  - `crates/kelir-core/src/planes/storage/*.rs` — every file (all bodies moved).
  - `crates/kelir-core/src/planes/storage/` — directory removed.

- `crates/kelir-engine/Cargo.toml` — depends on `kelir-storage-io` (for the L1 `StorageIo` trait it re-exports). `allowed_levels` stays `["L1","L2","L3"]` from Agent 1; Agent 5b tightens to `["L1","L2"]`.
- Root `Cargo.toml` — add `crates/kelir-storage`, `crates/kelir-storage-io` to `[workspace.members]`.
- `crates/kelir/src/lib.rs` — keep `pub mod planes::storage_plane;` forwarding to `kelir_engine::planes::storage::*`. Agent 5b rewrites the composition root.

### Gates

```sh
cargo check --workspace --all-targets
cargo run -p kelir-maturity -- check
cargo nextest run -p kelir-storage -p kelir-storage-io -p kelir-interface -p kelir-engine
just check-layers   # downward-only still holds
just check-spec
```

---

## Agent 3 — Wire plane split

**Blocks on:** Agents 1, 5a. **Blocks:** Agent 5b.

### Spec (`cans/`)

- `cans/07-wire-plane.md`
  - **§Wire-architecture**: replace "Owner: kelir-core native wire subsystem (kelir-core::wire)" with the three-crate split per plan.md.
  - Add `Crate topology` and `Trait seam` nodes: `NetOps`, `DnsOps`, `UdpNatOps`, `L4DivertOps` in `kelir-interface::wire`; `RealNetOps` etc. in `kelir-wire`; `kelir-engine::planes::wire` holds the `Arc<dyn *Ops>` façades.

### Artifacts (`cans/artifacts/`)

- `cans/artifacts/diagrams/wire-plane.txt` — update crate labels.
- `cans/artifacts/config/project-structure.txt` — add `crates/kelir-wire/`; delete `crates/kelir-engine/src/planes/wire/` entries that moved.

### Codebase

- `crates/kelir-interface/src/wire/mod.rs` — `pub mod net; pub mod dns; pub mod udp_nat; pub mod l4;`
- `crates/kelir-interface/src/wire/net.rs` (L1) — `NetOps` trait: `fn intercept_ethernet_frame(&self, direction: FrameDirection, frame: &mut [u8]) -> Result<FilterAction, PluginError>`.
- `crates/kelir-interface/src/wire/dns.rs` (L1) — `DnsOps` trait; move `DnsQuery`, `DnsResponse`, `DnsRcode` from `kelir-core/src/planes/wire/mod.rs`.
- `crates/kelir-interface/src/wire/udp_nat.rs` (L1) — `UdpNatOps` trait; move `UdpNatEntry`, `UDP_NAT_SLAB_CAPACITY`, `UDP_NAT_IDLE_TIMEOUT_SECS`.
- `crates/kelir-interface/src/wire/l4.rs` (L1) — `L4DivertOps` trait; move `L4Stream`.

- `crates/kelir-engine/src/planes/wire/mod.rs` (L2) — `WirePlane` façade holding `Arc<dyn NetOps>`, `Arc<dyn DnsOps>`, `Arc<dyn UdpNatOps>`, `Arc<dyn L4DivertOps>`, imds_sink, port_forwards, host_listeners. Delegates every call.
- `crates/kelir-engine/src/planes/wire/layout_wire.flow.rs` (L2) — `layout_wire` (frame views used by the L2 façade and by db-proxy's L3; keep in `kelir-engine` because both consumers name it).

- New `crates/kelir-wire/` (L3):
  - `Cargo.toml` — `name = "kelir-wire"`, `allowed_levels = ["L1","L2","L3"]`, deps: `kelir-interface`, `kelir-engine`, `smoltcp`, `kelir-otlp`, `kelir-transport`.
  - `src/smoltcp.flow.rs` (rename to `.run.rs` when body lands) — `SmoltcpStack` body.
  - `src/dns.flow.rs` — `resolve_query`, `resolve_synthetic_dns`, `parse_resolv_conf_nameservers`, `host_resolver_nameservers` body.
  - `src/udp_nat.flow.rs` — `udp_nat_alloc`, `reap_stale_udp_sockets`.
  - `src/l4.flow.rs` — `divert_l4`.
  - `src/lib.rs` (L2) — `pub use real_wire::{RealNetOps, RealDnsOps, RealUdpNatOps, RealL4DivertOps};`

- Deletions:
  - `crates/kelir-core/src/planes/wire/mod.rs` — moved.
  - `crates/kelir-core/src/planes/wire/layout_wire.rs` — moved to `kelir-engine`.

- Root `Cargo.toml` — add `crates/kelir-wire`.

### Gates

```sh
cargo check --workspace --all-targets
cargo run -p kelir-maturity -- check
cargo nextest run -p kelir-wire -p kelir-interface -p kelir-engine
just check-spec
```

---

## Agent 4 — Terminal + Checkpoint + Host split

**Blocks on:** Agents 1, 5a. **Blocks:** Agent 5b.

Three planes, one agent, three crates.

### Spec (`cans/`)

- `cans/04-terminal-plane.md` — §Terminal-pipeline: add `Crate topology` + `Trait seam` nodes for `kelir-terminal` and `TerminalOps`.
- `cans/08-lifecycle.md` — §Native-checkpoint-plane: add `Crate topology` + `Trait seam` for `kelir-checkpoint` and `CheckpointOps`. §Host-crash-reaper, §Host-memory-pressure-watcher: same for `kelir-host` and `HostOps`.
- `cans/09-snapshots.md` — §Snapshot-taxonomy, §Archives-and-portability: retarget `see:` at `crates/kelir-checkpoint/src/*.flow.rs`.

### Artifacts (`cans/artifacts/`)

- `cans/artifacts/diagrams/terminal-pipeline.txt`, `cans/artifacts/diagrams/host-runtime.txt` — update crate labels.
- `cans/artifacts/code/causal-delta.rs`, `cans/artifacts/code/execution-receipt.rs` — note new homes.
- `cans/artifacts/config/project-structure.txt` — add `crates/kelir-terminal/`, `crates/kelir-checkpoint/`, `crates/kelir-host/`; delete the corresponding `crates/kelir-engine/src/planes/*/` entries.

### Codebase

- `crates/kelir-interface/src/terminal/mod.rs` (L1) — `TerminalOps` trait: `resize`, `feed`, `export_screen_matrix`, `read_from`, `append_stream_chunk`, `record_output`, `output_filters`, `token_shield`, `prompts`. Move `TokenShieldConfig`, `PromptRuleConfig`, `TerminalWiring`, `LogStream`, `LogCursor`, `LogSegment` here from `kelir-core/src/planes/terminal/mod.rs`.
- `crates/kelir-interface/src/checkpoint/mod.rs` (L1) — `CheckpointOps` trait; move `PidLineage`, `StepSlot`, `CheckpointNode`, `ManifestGraph` here.
- `crates/kelir-interface/src/host/mod.rs` (L1) — `HostOps` trait; move `PressureLevel`, `PressureEvent`, `PressureSource`, `BalloonRequest`, `ReclaimRequest`, `HostReaperKind`.

- `crates/kelir-engine/src/planes/terminal/mod.rs` (L2) — `TerminalPlane` façade holding `Arc<dyn TerminalOps>`.
- `crates/kelir-engine/src/planes/checkpoint/mod.rs` (L2) — `CheckpointPlane` façade holding `Arc<dyn CheckpointOps>`.
- `crates/kelir-engine/src/planes/host/mod.rs` (L2) — `HostPlane` façade holding `Arc<dyn HostOps>`.

- New `crates/kelir-terminal/` (L3):
  - `Cargo.toml` — `name = "kelir-terminal"`, `allowed_levels = ["L1","L2","L3"]`, deps: `kelir-interface`, `kelir-engine`, `kelir-otlp`.
  - `src/vt100.flow.rs`, `src/token_shield.flow.rs`, `src/log_segment.flow.rs` — move `kelir-core/src/planes/terminal/mod.rs` body here, split by module.
  - `src/lib.rs` (L2) — `pub use real_terminal::RealTerminalOps;`

- New `crates/kelir-checkpoint/` (L3):
  - `Cargo.toml` — `name = "kelir-checkpoint"`, `allowed_levels = ["L1","L2","L3"]`, deps: `kelir-interface`, `kelir-engine`, `kelir-otlp`.
  - `src/manifest_graph.flow.rs`, `src/step_slot.flow.rs`, `src/compaction.flow.rs` — move `kelir-core/src/planes/checkpoint/mod.rs` body.
  - `src/lib.rs` (L2) — `pub use real_checkpoint::RealCheckpointOps;`

- New `crates/kelir-host/` (L3):
  - `Cargo.toml` — `name = "kelir-host"`, `allowed_levels = ["L1","L2","L3"]`, deps: `kelir-interface`, `kelir-engine`, `kelir-otlp`, `libc`.
  - `src/pressure.flow.rs`, `src/balloon.flow.rs`, `src/reaper.flow.rs` — move `kelir-core/src/planes/host/mod.rs` body.
  - `src/lib.rs` (L2) — `pub use real_host::RealHostOps;`

- Deletions:
  - `crates/kelir-core/src/planes/terminal/`, `crates/kelir-core/src/planes/checkpoint/`, `crates/kelir-core/src/planes/host/` — directories removed.

- Root `Cargo.toml` — add `crates/kelir-terminal`, `crates/kelir-checkpoint`, `crates/kelir-host`.

### Gates

```sh
cargo check --workspace --all-targets
cargo run -p kelir-maturity -- check
cargo nextest run -p kelir-terminal -p kelir-checkpoint -p kelir-host -p kelir-interface -p kelir-engine
just check-spec
```

---

## Agent 5b — Composition root + final crate topology

**Blocks on:** Agents 2, 3, 4. **Blocks:** nothing; lands last.

### Spec (`cans/`)

- `cans/01-architecture.md`
  - **§Host-runtime**: rewrite the crate list to the final topology. `kelir-core` disappears. `kelir-engine: stripped microkernel (BusDispatcher, PipelineEngine, EventBus, DriverRouter)`. Add the six new crates (`kelir-storage`, `kelir-storage-io`, `kelir-wire`, `kelir-terminal`, `kelir-checkpoint`, `kelir-host`) with their roles from plan.md §Target-crate-layout.
  - **§Core-invariants**: confirm no `kelir-core` refs remain.
- `cans/02-plugin-model.md`, `cans/03-sdk-architecture.md` — same crate-list rewrite where they list `kelir-core` sub-modules.
- `cans/12-adapters.md`, `cans/13-cli.md`, `cans/14-observability.md` — confirm no `kelir-core` refs remain.

### Artifacts (`cans/artifacts/`)

- `cans/artifacts/config/project-structure.txt` — final tree with all six new crates; delete every `crates/kelir-core/` entry (should already be `crates/kelir-engine/` from 5a; rewrite the plane sub-dirs).
- `cans/artifacts/diagrams/host-runtime.txt` — final crate topology diagram.
- `cans/artifacts/code/vmm-driver-trait.rs`, `cans/artifacts/code/plugin-trait.rs` — confirm crate refs are `kelir-engine`.

### Codebase

- `crates/kelir-engine/Cargo.toml` — `allowed_levels = ["L1","L2"]`.
- `crates/kelir-engine/src/lib.rs` — `pub mod planes;` now just re-exports the façades from the new crates. The planes sub-dirs (`storage/`, `wire/`, `terminal/`, `checkpoint/`, `host/`) live in `kelir-engine` as thin L2 façades delegating to `Arc<dyn *Ops>`.
- `crates/kelir-engine/src/planes/mod.rs` — `pub mod storage; pub mod wire; pub mod terminal; pub mod checkpoint; pub mod host;` with each sub-module holding only the L2 `.flow.rs` façade files.
- `crates/kelir/src/lib.rs` — **composition root**: bind `RealFuseOps`, `RealStorageIo`, `RealTerminalOps`, `RealCheckpointOps`, `RealHostOps`, `RealNetOps`, `RealDnsOps`, `RealUdpNatOps`, `RealL4DivertOps` into the `PipelineEngineBuilder`. Every `*Ops` slot is required — `seal()` returns `Halt("FuseOps unbound at seal")` on any missing binding. No default `NopFuseOps`.
- `crates/kelir/src/builder_sdk.rs` — `finish_boot()` constructs the `Real*` types and passes them to `kelir_engine::Session::boot_with_engine`. Verify `kelir-engine`'s builder does not name any `Real*` type.

### Gates

```sh
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p kelir-maturity -- check
cargo nextest run --workspace
pnpm -r exec tsc --noEmit
uv run mypy packages/py
just check-hygiene
just check-spec
```

---

## Gates (every agent runs before opening PR)

```sh
just check-spec
cargo check --workspace --all-targets
cargo run -p kelir-maturity -- check
git diff --stat   # must touch only your slice + noted shared-file edits
```

Agents 2, 3, 4 also run `just check-layers` after landing the split; the
downward-only invariant must still hold (`kelir-storage` depends on
`kelir-engine`, `kelir-engine` never on `kelir-storage`).

---

## Test matrix

| Agent | New tests | Existing tests to re-run |
|---|---|---|
| 1 | `crates/kelir-maturity/tests/check_test.rs` — one passing tree, one failing tree per rule | none |
| 2 | none new (deferred to L3 phase) | `kelir-interface`, `kelir-engine`, `kelir-storage`, `kelir-storage-io` |
| 3 | none new | `kelir-wire`, `kelir-interface`, `kelir-engine` |
| 4 | none new | `kelir-terminal`, `kelir-checkpoint`, `kelir-host`, `kelir-interface`, `kelir-engine` |
| 5a | none (mechanical rename) | `cargo test --workspace` |
| 5b | `crates/kelir/tests/composition_seal_test.rs` — unbound `FuseOps` fails `seal()` | `cargo test --workspace` |

Tests do not land in this wave beyond the checker's own unit tests and the
composition-root seal test. Body tests are L3-phase work.

---

## What "done" looks like

- `find crates -name '*.run.rs' | wc -l` → 0 (deferred to L3 phase per Plan corrections).
- `cargo run -p kelir-maturity -- check` exits 0 on `main`.
- `cargo check --workspace --all-targets` passes with `kelir-engine` set to `allowed_levels = ["L1","L2"]` and no `.run.rs` file anywhere in it.
- `rg 'kelir_core|kelir-core' crates/ packages/ cans/ cans/artifacts/` → empty.
- `kelir/src/lib.rs` names every `Real*Ops` type; `kelir-engine/src/lib.rs` names none of them.
- An agent given *"fix storage FUSE UNLINK atomicity"* finds the file in one `rg 'UNLINK' --glob '*.flow.rs' crates/kelir-storage/` and opens `crates/kelir-storage/src/reconciler.flow.rs`. Two-file context (L1 trait + L2 façade), not fifteen.
```