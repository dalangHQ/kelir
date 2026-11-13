```markdown
# plan.md — Filename-Driven Maturity

## Objective

Make body maturity (L1/L2/L3) legible to humans via filename, cheap for
agents to drill into, and enforced by one binary reading one policy file.
No comments, attributes, `-impl` suffixes, or prose layer statements.

Enforcement is two layers, deliberately kept separate:

- **Structural (primary, hard).** Filename declares the level; the crate's
  `allowed_levels` declares what may land there; the compiler then forbids
  the cross-crate reference that would violate it. This catches the leak
  that matters: an L3 body inside a crate that cannot name its type.
- **Content (secondary, soft).** A regex pass over `perf/maturity-policy.toml`
  catches the *inverse* leak: L1-shaped code in an L3 file, deep algorithms
  in an L2 file, a trait declaration that drifted into L3. It is small,
  mechanical, grep-able, and data-driven — never a proc-macro in disguise.

## The three invariants — the structural system

1. **Filename → level.**
   `foo.rs` → L1 (contract). `foo.flow.rs` → L2 (wiring). `foo.run.rs` → L3 (body).
   Enforced by: `level_of(filename)` in the checker, ~10 lines.

2. **Crate → allowed levels.**
   Per-crate in `Cargo.toml`:
   ```toml
   [package.metadata.kelir]
   allowed_levels = ["L1", "L2"]      # e.g. kelir-engine
   ```
   Enforced by: one TOML read, ~5 lines.

3. **Cross-product = pass.**
   Every `.rs` file: `level_of(file) ∈ crate.allowed_levels`. Otherwise fail.

Those three are the structural enforcement. `cargo kelir-maturity check`
adds the content pass below to close the soft leak.

## The boundary — what each level must never do

Two enforcement layers, one contract. A *hard leak* is a level landing in a
crate that cannot name its type — caught by (1)+(2)+(3) above and by the
compiler itself. A *soft leak* is the right file in the right crate but the
wrong shape — caught by the content pass.

| Level | Must never **contain** | Must never **leak across the seam** |
|---|---|---|
| **L1** (`foo.rs`) | any `impl` block (derive attrs are source-invisible, so they are fine); `libc`, `std::fs`, `std::net`, `std::process`, `std::os`, `tokio::fs/net/process`, `block_on`; `unsafe`; `dyn Trait` (trait objects are wiring, not contract) | the name of any concrete impl type (`RealFuseOps`, `RealStorageIo`) — if L1 can name it, the seam is fake |
| **L2** (`foo.flow.rs`) | `impl DomainTrait for Type` (the L2 façade holds `Arc<dyn FuseOps>` and *delegates*; the body lives in L3 as `impl FuseOps for RealFuseOps`); `libc`, `std::fs`, `std::net`, `std::process`, `std::os`; `unsafe`; a body over ~30 lines (proxy for "algorithm leaked out of L3") | constructing the L3 type directly (`RealFuseOps::new(...)`) — the seam must go through `Arc<dyn FuseOps>` |
| **L3** (`foo.run.rs`) | `trait Foo { ... }` (traits are L1); `pub use`/`pub mod` that re-exports an L1 interface item (crate-level re-exports belong in `lib.rs`, which is L2 by convention) | a `// TODO` comment in a filled body — L3 is zero unwritten logic, and `todo`/`unimplemented` are already denied by workspace lints; a prepared stub carrying only `// TODO (L3)` markers warns until L3 fills it |

Three concrete seam breaks this contract names:

- **L1 names `RealFuseOps`** — a contract that can see the impl is not a
  contract; the compiler rejects it via (2) because `kelir-interface` cannot
  depend on `kelir-storage`.
- **L2 owns `impl FuseOps for FuseDispatcher`** — the façade must delegate
  through `Arc<dyn FuseOps>`; if it owns the trait impl the split collapsed,
  and the content pass flags it.
- **L3 carries `// TODO`** — the body is not a body; the content pass and
  the existing lints both fire.

The compiler already enforces the hard case: `kelir-engine`
(`allowed_levels = ["L1","L2"]`) cannot name a type defined in
`kelir-storage` (L3) because the crate graph forbids the dependency. The
filename check adds the *placement* guarantee (you cannot sneak a `.run.rs`
into `kelir-engine`). Together they mean the L2/L3 seam is enforced by the
compiler accepting the file, exactly as the acceptance section below states.

The content pass is for the *soft* leak: a deep algorithm written in an L2
file that still compiles, a trait declaration that drifted into L3. Those
are heuristics, and the list stays short.

## Why filename wins

`flow` and `run` are **verbs**, not layer tags. Rename L1/L2/L3 to
"schema/wiring/core" tomorrow and the filenames don't change — proving they
are not annotations. They read as intent (`this file is about flow`, `this
file is about running`), so a human unfamiliar with kelir sees the meaning
before learning the taxonomy.

Glob/grep is the natural interface:

```sh
find . -name '*.run.rs'                          # all L3 bodies
find . -name '*.flow.rs'                         # all L2 wiring
rg 'TODO' --glob '*.flow.rs'                     # TODOs, scoped to L2
rg 'UNLINK' --glob '*.run.rs'                    # one file, one crate
```

An agent's first move for *"fix FUSE UNLINK atomicity"*:
`rg 'UNLINK' --glob '*.run.rs'` → `kelir-storage/src/fuse.run.rs`. Done.
Two-file context (interface + impl), not fifteen.

## Target crate layout

Crate names stay nouns. The `-impl` suffix is never used.

| Crate | allowed_levels | Role |
|---|---|---|
| `kelir-interface` | `["L1"]` | traits + wire types, no impls |
| `kelir-engine` | `["L1", "L2"]` | dispatcher, session, bus, plugin registry |
| `kelir-storage` | `["L1", "L2", "L3"]` | FUSE, reconciler, lock broker, quota |
| `kelir-wire` | `["L1", "L2", "L3"]` | smoltcp stack, DNS, UDP NAT, L4 diversion |
| `kelir-terminal` | `["L1", "L2", "L3"]` | VT100, token shield, log segment |
| `kelir-checkpoint` | `["L1", "L2", "L3"]` | manifest graph, delta compaction |
| `kelir-host` | `["L1", "L2", "L3"]` | pressure, balloon, reaper |
| `kelir-transport` | `["L1", "L2", "L3"]` | SPSC rings (enclave) |
| `kelir-storage-io` | `["L1", "L2", "L3"]` | openat2/renameat2/flock/fallocate (enclave) |
| `kelir-shim` | `["L1", "L2", "L3"]` | seccomp (enclave) |
| `kelir-driver-{rustvmm,hvf,openvmm,avf,bwrap,proot}` | `["L1", "L2", "L3"]` | VmmDriver impls |
| `kelir-plugins/*` | `["L1", "L2", "L3"]` | plugin crates |
| `kelir` | `["L1", "L2"]` | composition root — binds impls, has no body logic of its own |

`kelir-core/` and `kelir-core/src/planes/` are **deleted**. Their five plane
directories (storage, wire, terminal, checkpoint, host) become five crates
above. Their `dispatch_plugin`, `bus`, `session_lifecycle`, `plugin_registry`,
`trap_gate`, `uart_terminal` stay, renamed to `kelir-engine`.

## The trait seam

Each plane is three files, one per level, in three crates:

```rust
// kelir-interface/src/storage/fuse.rs        (L1: ~15 lines)
pub trait FuseOps: Send + Sync {
    fn dispatch(&self, req: &mut FuseRequest, ctx: &HandlerCtx)
        -> Result<FuseResponse, PluginError>;
}

// kelir-engine/src/planes/storage/fuse.flow.rs   (L2: ~30 lines)
pub struct FuseDispatcher {
    ops: Arc<dyn FuseOps>,
    quota: Arc<StorageQuota>,
    metrics: Arc<MetricRegistry>,
}
impl FuseDispatcher {
    pub fn dispatch(&self, req: &mut FuseRequest, ctx: &HandlerCtx)
        -> Result<FuseResponse, PluginError>
    { self.ops.dispatch(req, ctx) }
}

// kelir-storage/src/fuse.run.rs                  (L3: the whole body)
impl FuseOps for RealFuseOps {
    fn dispatch(&self, req: &mut FuseRequest, ctx: &HandlerCtx)
        -> Result<FuseResponse, PluginError>
    { /* everything that lived in fuse_storage.rs */ }
}

// kelir/src/lib.rs                               (composition)
let engine = PipelineEngineBuilder::new()
    .with_storage(Arc::new(RealFuseOps::new(...)))
    .seal()?;
```

`PipelineEngineBuilder::seal()` requires every `*Ops` slot to be bound —
`Option::ok_or(Halt("FuseOps unbound at seal"))`. No default NopFuseOps.
The missing `Option` is the only seam marker.

## Enforcement — three files, one binary, one policy

| File | Contents | Owner |
|---|---|---|
| `crates/kelir-engine/src/bin/maturity.rs` | `level_of()`, `check()`, `list()`, `emit()` | workspace |
| `perf/maturity-policy.toml` | per-level `forbid` rules, `budget.L2_max_delta_per_pr` | policy |
| `Cargo.toml` per crate `[package.metadata.kelir]` | `allowed_levels = […]` | per-crate |

### The policy file

```toml
# perf/maturity-policy.toml
[budget]
L2_max_delta_per_pr = 20        # net new L2 files per PR, not net lines

[[level.L1.forbid]]
id = "impl-block"
pattern = '^\s*impl\b'
hint  = "L1 is contract: no impl blocks (derive attrs are fine)"

[[level.L1.forbid]]
id = "os-reach"
pattern = '\b(libc|std::fs|std::net|std::process|std::os|tokio::fs|tokio::net|tokio::process|block_on)\b'

[[level.L1.forbid]]
id = "trait-object"
pattern = '\bdyn\s+[A-Z]'
hint  = "trait objects are wiring (L2), not contract (L1)"

[[level.L2.forbid]]
id = "domain-trait-impl"
pattern = '^\s*impl\s+(FuseOps|StorageIo|TerminalOps|NetOps|DnsOps|UdpNatOps|TrapInterposer|FsInterposer|NetInterposer|LifecycleParticipant|SystemTelemetryParticipant)\s+for\b'
hint  = "domain trait impls are bodies (L3)"

[[level.L2.forbid]]
id = "os-reach"
pattern = '\b(libc|std::fs|std::net|std::process|std::os)\b'

[[level.L2.forbid]]
id = "direct-impl-construct"
pattern = '\bReal[A-Z]\w*::new\('
hint  = "the seam must go through Arc<dyn Ops>, not the concrete type"

[[level.L3.forbid]]
id = "trait-decl"
pattern = '^\s*(pub\s+)?trait\b'
hint  = "traits are contract (L1)"

[[level.L3.forbid]]
id = "interface-reexport"
pattern = '^\s*pub\s+(use|mod)\b.*\b(Ops|Interposer|Participant|Handle)\b'
hint  = "crate-level re-exports live in lib.rs (L2), not in bodies"

# advisory only — warn, do not fail
[[level.L2.warn]]
id = "dyn-any"
pattern = '\bdyn\s+Any\b'
hint  = "type erasure defeats the seam; prefer a typed handle"
```

### The check binary (~50 lines structural, ~30 lines content)

```rust
fn check(root: &Path, policy: &Policy) -> Result<(), Vec<Fail>> {
    let mut fails = Vec::new();
    for file in walk_rs_files(root) {
        // 1. structural: level ∈ crate.allowed_levels
        let level = level_of(&file);                 // ~10 lines: suffix match
        let krate = crate_of(&file);
        let allowed = read_allowed_levels(&krate)?;  // ~5 lines: one TOML read
        if !allowed.contains(&level) {
            fails.push(Fail::Placement { file, level, allowed });
            continue;
        }
        // 2. content: per-level forbid rules
        let src = fs::read_to_string(&file)?;
        for rule in &policy.level[level].forbid {
            for (lineno, line) in src.lines().enumerate() {
                if rule.regex.is_match(line) {
                    fails.push(Fail::Forbid { file, lineno, rule_id: rule.id });
                }
            }
        }
        // 3. L3 carries no TODO (prepared stubs warn, not fail, until L3 fills them)
        if level == Level::L3 && src.contains("// TODO") {
            warns.push(Warn::TodoInBody { file });
        }
    }
    // 4. budget: L2 file delta per PR ≤ budget
    if policy.budget.l2_delta_exceeded(&fails) {
        fails.push(Fail::Budget);
    }
    if fails.is_empty() { Ok(()) } else { Err(fails) }
}
```

`check()` is ~50 lines for the structural pass plus ~30 for the content
pass. The rules are data; changing policy does not rebuild the binary.

### Commands

- `cargo kelir-maturity check` — structural pass (every `.rs` file's level ∈
  its crate's `allowed_levels`); content pass (per-level forbid rules); L2
  count ≤ budget; `todo`/`unimplemented` forbidden in L3.
- `cargo kelir-maturity list --level L2 --crate kelir-storage`
- `cargo kelir-maturity list --tag sp_08` — reads the existing
  `slop_register.yaml`; tags remain the L2 TODO vocabulary.
- `cargo kelir-maturity emit --out maturity/` — writes `by-crate/`,
  `by-tag/`, `by-level/`, `counts.json`. Generated, never hand-edited.

`AGENTS.md §6` gate becomes: `cargo kelir-maturity check` in place of the
`check_manifest.rs` TODO-regex pass. The slop register, case-to-test-map,
manifest.yaml and budgets.toml keep their own axes.

## The four axes stay independent

| Axis | Values | Owner | Do not conflate with |
|---|---|---|---|
| Body maturity | L1 / L2 / L3 | filename + `allowed_levels` | — |
| Test status | implemented / red / gap | `case-to-test-map.yaml` | maturity |
| Plugin tier | 1 / 2 / 3 | `[package.metadata.kelir.tier]` | maturity |
| Target tier | unit / integration / e2e | `manifest.yaml` cases | maturity |

The "3" in maturity, test status, and plugin tier is coincidence. They are
orthogonal. There is no L4.

## Migration order — one plane at a time, smallest risk first

Each step is a standalone PR that keeps `cargo check --workspace` green.

1. **Tooling first.** Land `crates/kelir-engine/src/bin/maturity.rs` +
   `perf/maturity-policy.toml`. Add `allowed_levels = ["L1","L2","L3"]` to
   *every* existing crate — the checker passes vacuously. Wire into
   `just check-hygiene`.

2. **Storage split** (largest L3 surface, seam already 80% present).
   - `crates/kelir-core/src/planes/storage/io_storage.rs` →
     `crates/kelir-storage-io/src/io_storage.run.rs`; move the trait into
     `kelir-interface/src/storage/io.rs`.
   - `crates/kelir-core/src/planes/storage/fuse_storage.rs` →
     `crates/kelir-storage/src/fuse.run.rs`; trait body extracted to
     `kelir-interface/src/storage/fuse.rs` as `FuseOps`; L2 façade at
     `kelir-engine/src/planes/storage/fuse.flow.rs`.
   - Same for `storage_reconciler.rs`, `lock_broker.rs`,
     `storage_synthetic.rs`, `mod.rs` (`StorageQuota` stays in engine as L2).
   - Set `kelir-storage.allowed_levels = ["L1","L2","L3"]`,
     `kelir-storage-io = ["L1","L2","L3"]`,
     `kelir-engine = ["L1","L2"]`, `kelir-interface = ["L1"]`.
   - Delete `crates/kelir-core/src/planes/storage/`.

3. **Wire split.** Same pattern: `kelir-wire` gains `smoltcp.run.rs`,
   `dns.run.rs`, `udp_nat.run.rs`, `l4.run.rs`; `NetOps` / `DnsOps` /
   `UdpNatOps` traits in `kelir-interface`; façades in
   `kelir-engine/src/planes/wire/`.

4. **Terminal split.** `kelir-terminal` gains `vt100.run.rs`,
   `token_shield.run.rs`, `log_segment.run.rs`; `TerminalOps` in interface.

5. **Checkpoint, host** — mechanical, same pattern.

6. **Drivers.** Each `kelir-driver-*` already has its own crate. Rename any
   `.rs` file containing real syscall bodies to `.run.rs` (or add
   `.run.rs` when splitting stubs from bodies — most drivers are already
   L3-only, so just rename). No structural split needed.

7. **Crate rename.** `kelir-core` → `kelir-engine`. Update every
   `use kelir_core::` (mostly `kelir` and plugins). Mechanical `sed`.

8. **`kelir-core` deletion.** After step 7 no code lives there.

## What changes in AGENTS.md

§2 shrinks from one paragraph to three lines:

```markdown
## 2. Maturity

Level = filename suffix: `.rs` (L1 contract), `.flow.rs` (L2 wiring),
`.run.rs` (L3 body). Crate role = `[package.metadata.kelir].allowed_levels`.
Enforced by `cargo kelir-maturity check`.
```

§6 gate list gains `cargo kelir-maturity check`; the `check_manifest.rs`
TODO-regex pass is removed (its slop-register validation moves into
`kelir-maturity check`).

The `// TODO (L3) [tag]: …; slop: …` comment form is retained (L2 files
still carry it — it is the vocabulary `check` reads to validate tags), but
the `(L3)` marker inside the comment is dropped: **the filename already says
L2**. Same comment, one token lighter.

## What this rejects

- **`-impl` crate suffixes.** Collide with the plane name; the tree reads as
  duplication. Crates are nouns, layers are graph positions.
- **`// L3` prose comments.** Unenforceable; regex hazard; invisible to the
  compiler.
- **`#[kelir::maturity(...)]` proc-macro.** Solves the same problem but
  costs a proc-macro crate and hides the signal in AST instead of the file
  listing. Filename is the human-native form; the compiler sees the filename
  too.
- **Feature flags (`#[cfg(feature = "l3")]`).** Puts the layer in the build
  graph — every L1/L2 crate must know L3 feature names. The crate's
  `allowed_levels` is the same information with zero build coupling.
- **An overgrown forbid list.** The content pass is a *secondary net*, not
  the primary signal. If the per-level regex rules start needing their own
  parser, the framing collapsed — the structural pass (filename +
  `allowed_levels` + compiler) is the enforcement, the content pass is a
  tripwire.
- **"L4."** The maturity axis has three stops: empty, flow, full. A fourth
  "more than full" is impossible. Add a fourth *checkpoint* only if you
  genuinely want to track a sub-state (e.g. `.flow.dyn.rs` vs
  `.flow.impl.rs`); one line in `level_of`.

## Acceptance

- `find . -name '*.run.rs' | wc -l` counts every L3 body in the workspace,
  in one command, with no false positives.
- `cargo kelir-maturity check` runs in <2s and is the only gate for body
  maturity.
- `kelir-engine` compiles with `allowed_levels = ["L1","L2"]` — the crate
  *cannot* hold an L3 body. The split is enforced by the compiler accepting
  the file, not by a comment a human must remember to add.
- A `RealFuseOps::new(...)` call written into an L2 file fails `check` via
  the `direct-impl-construct` rule and fails `cargo check` via the crate
  graph (whichever fires first is the gate).
- An `impl FuseOps for FuseDispatcher` written into an L2 file fails `check`
  via `domain-trait-impl`.
- A `trait Foo { ... }` written into an L3 file fails `check` via
  `trait-decl`.
- A `// TODO` written into a prepared L3 stub file warns via `TodoInBody`
  until the L3 phase fills it (and fails workspace lints if it becomes
  `todo!()`).
- An agent given *"fix <plane> <verb>"* finds the file in one `rg --glob
  '*.run.rs'` and opens it.
```