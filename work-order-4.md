```markdown
# work-order.md — Spec Redundancy Cleanup (3 parallel agents)

## Mission
Detect and remove **textual** redundancies (duplicate sentences, near-identical bullets) and **substantive** redundancies (same fact or invariant stated in 2+ files with no canonical home, or with the wrong home). One canonical home per concept. Everything else is `see:`.

- Do not change behavior, don't invent rules, don't rename real identifiers (`0x0011`, `EWOULDBLOCK`, `session.vm.set_vcpus`, `KeepCacheKey`, `lease_kind='ephemeral'`, etc.).
- Do not delete concepts. Delete duplicate *statements* of a concept. Preserve the canonical home.
- Preserve the `see:` 1-hop rule from `AGENTS.md`. If a fix would create an A→B→C hop, stop and log to `_collab/conflicts.md`.
- Unknown outcome → `TBD` + `_collab/conflicts.md`. Never silently resolve.

## Ownership rule (all agents)
- **One file → one agent.** Never edit a file another agent owns.
- If a fix requires edits outside your file set, hand off via `_collab/conflicts.md` with `from / to / file / edit`.
- AGENTS.md is read-only for all agents.
- Run `just check-spec` before declaring done (must match CI flags per AGENTS.md).

## File ownership

| Agent | Owns |
|---|---|
| **A1 — Foundation & SDK** | `00-overview.md`, `01-architecture.md`, `02-plugin-model.md`, `03-sdk-architecture.md` |
| **A2 — Planes & Lifecycle** | `04-terminal-plane.md`, `05-storage-plane.md`, `06-storage-catalog.md`, `07-wire-plane.md`, `08-lifecycle.md`, `09-snapshots.md` |
| **A3 — Protocol, Ops, Plugins** | `10-attestation.md`, `11-protocol.md`, `12-adapters.md`, `13-cli.md`, `14-observability.md`, `15-operations.md`, `plugins/*.md` |

Cross-agent canonical ownership summary:
- A1 owns canonical homes for: zero-workload-intrusion, tier taxonomy, unified verdict enum, unified builder, boot budget, latency ceiling, shim identity.
- A2 owns canonical homes for: port-0 break-glass, FilterAction wire-plane errno, wire-plane L4 diversion, clock skew, rollback, base snapshot format, lease schema, CoW broker, session incarnation semantics.
- A3 owns canonical homes for: PMU-in-receipts invariant, stream kind registry, wire framing, adapter boundary, CLI command surface, plugin leaves.

---

## Agent A1 — Foundation & SDK

### 1. Latency budgets — one numeric home
- Canonical: `02-plugin-model.md#Latency-ceiling` (self-declares sole home).
- Remove the `<2µs` restatement in `01-architecture.md#FFI-boundary-economics`; replace with `see: 02-plugin-model.md#Latency-ceiling`.
- Scan `01`, `02`, `03` for any other latency numbers (`<1µs`, `50-150µs`, `2µs`); every one outside `02#Latency-ceiling` becomes `see:`.

### 2. "Zero workload intrusion" / "no guest-side SDK"
- Canonical: `01-architecture.md#Zero-workload-intrusion`.
- Consolidate the restatements in `01#Workload-contract` and `01#Params-ingress-contract` into a single bullet each pointing at `#Zero-workload-intrusion`.
- Trim `00#What-it-is` / `00#What-it-is-not` so they don't paraphrase the invariant (keep the product framing; point to 01 for the invariant).
- In `03`, any `03#Session-surfaces` / `03#Params-typed-views` bullet that restates "no guest SDK" → `see:`.

### 3. Plugin tier definitions
- Canonical: `02-plugin-model.md#Plugin-tiering`.
- Trim `01#Capability-taxonomy` "Plugin" bullet to a single `see:` line; remove the "Optional domain extensions (13 catalog plugins)" repetition of tier phrasing.
- In `02#Dual-plane-execution-architecture`, the "monotonic order" phrase restates `#Unified-verdict` — point to it.

### 4. Unified verdict + FilterAction
- Canonical enum: `02-plugin-model.md#Unified-verdict`.
- `03#Unified-verdict-surface` restates the full variant list (Pass/Patch/Drop/Suspend/Abort + TS/Py spellings). Trim to: "per-SDK spellings of `02#Unified-verdict` variants" + a compact table of **naming deltas only**, not the semantics.
- `02#Filter-before-FFI-gate` and `02#Unified-interposition-pipeline` both describe "Gate" and "Stage". Pick one term (`Gate` per `11#Trap-fast-path`) and rewrite the other sections as bullets under it. Do **not** rename the other file's usage — A3 will align `11`.
- If a section defines `FilterAction` variants in full, replace the full list with `see: 02#Unified-verdict` + the section's *domain-specific* additions only.

### 5. Base snapshot / fast-boot cache
- Canonical format: `09-snapshots.md#Base-snapshot` (A2 owns 09).
- `01#Snapshot-and-rollback-scope` restates "Base snapshots are engine-internal; never user-visible" — replace with `see: 09-snapshots.md#Base-snapshot` and keep only the 01-specific fact ("digest recorded in host-side cache manifest").
- Log a handoff to A2 noting that `08#Autonomous-fast-boot-hydration` must own *cache-key formula + capture timing*, and `09#Base-snapshot` must own *format + visibility*.

### 6. Session surface verbs in 03
- `03#Session-surfaces` should be a table of `verb → see:` pairs, not a description. For each entry below, remove the descriptive sentence and replace with `see:`:
  - `set_vcpus`, `adjust_balloon`, `drain`, `rollback`, `register_veto` → `08-lifecycle.md`
  - `forward_port` → `07-wire-plane.md#Port-forwarding`
  - `export_snapshot` / `import_snapshot` → `09-snapshots.md#Archives-and-portability`
  - `map_shared_region` → `01-architecture.md#Shared-memory-channel`
- The `session.rollback()` description restates `08#Rollback` verbatim; replace with `see: 08-lifecycle.md#Rollback`.

### 7. "Frozen" phrasing
- Canonical: `11-protocol.md#Control-channel` (A3 owns 11).
- `01#VirtIO-Device-Complex` "Channel allocation is frozen" → `see: 11-protocol.md#Control-channel`. Do not restate "frozen".
- Log a handoff to A3 confirming 11 is canonical.

### 8. Historical "fluent builder" duplication
- `02#Fluent-plugin-creation-builder` and `03#Fluent-plugin-surface` both carry 30-verb + 8-scope tables that duplicate the *current* `#Unified-builder` / `#Unified-builder-surface` tables.
- **Delete the duplicate verb/scope tables** in both history sections. Keep one paragraph per history section that names the shape and links to the current section. Keep only the *pinned per-language signature shapes* that aren't in the current sections (Rust `Fn(&str) -> Box<dyn AnyPlugin>`, TS `() -> PluginContract`, Python `Fn(_PluginWrapper) -> _PluginWrapper`) — but **only if** those shapes differ from the unified builder's current shapes; if identical, delete and reference.
- Log a handoff to A3 to make `15#--fluent-builder-coverage` defer to `#--unified-builder-coverage` (one paragraph of history, no duplicate case tables).

### 9. Plugin roster duplication in 02
- `02#First-class-plugins` and `02#Coverage-matrix` both enumerate the community plugin roster. The roster belongs in `#First-class-plugins`; `#Coverage-matrix` should be a pure `plugin → example` map with no tier restatement.

### 10. `03#Session-surfaces` / `03#Session-control-surface` split
- The "session data operations" and "session control surface" lists are two lists over the same surface. Merge into one table and mark rows `data | control`, so the rollback/snapshot/vCPU set appears once.

---

## Agent A2 — Planes & Lifecycle

### 1. Port 0 = human break-glass
- Canonical: `04-terminal-plane.md#Terminal-pipeline` (and `04#Binary-exec-channel` for the "PTY reserved for human attach" clause).
- `08#Break-glass` restates "Port 0 master is the sole in-process break-glass surface" — keep the 08-specific facts (`session.console()`, `EXEC` for programmatic break-glass, refuse-states in `Paused`/`Stopped`); replace the definitional sentence with `see: 04-terminal-plane.md#Terminal-pipeline`.
- Log a handoff to A3 for `11#Binary-exec-channel` (A3 trims its restatement).

### 2. FilterAction::Drop errno
- Canonical: `07-wire-plane.md#Frame-drop-and-forward-semantics`.
- This section is authoritative for `Drop { errno: ECONNREFUSED }` → TCP RST, `Drop { errno: EACCES }` → HTTP 403 JSON, `Drop { errno: None }` → silent discard.
- Log a handoff to A3: `plugins/chaos` and `plugins/mesh` must replace their errno-restating bullets with `see: 07-wire-plane.md#Frame-drop-and-forward-semantics` plus their domain-specific behavior.

### 3. Boot budget
- Canonical: `01#Boot-budget` (A1).
- `08#Autonomous-fast-boot-hydration` restates `<500µs` and `<60ms` — trim to `see: 01-architecture.md#Boot-budget`. Keep only 08-specific facts: capture timing, cache key formula, fallback semantics, `enabled` vs `auto` behavior.
- Log a handoff to A3 for `15#Enforced-invariants` (trim numeric restatement; keep gate names and exit-code behavior).

### 4. Base snapshot split
- `08#Autonomous-fast-boot-hydration` owns: **capture timing, cache-key formula `BLAKE3(vmlinux || erofs || boot_params || driver_id)`, cache path `cas/blobs/blake3_<hash>`, `cas_leases` row, `enabled`/`auto` behavior**.
- `09#Base-snapshot` owns: **format, `parent_digest: None`, invisibility in `session.lineage()` / `export_snapshot`, base snapshot exclusion from user-visible CausalDelta chains**.
- Remove any cross-restatement. In `09#Base-snapshot`, the "Not visible in session.lineage(); not exported by session.vm.export_snapshot" line stays. In `08`, "Base snapshots are engine-internal" → `see: 09-snapshots.md#Base-snapshot`.

### 5. Clock skew / monotonic / deterministic RNG
- Canonical: `08-lifecycle.md#Clock-invariants` + `08#Clock-skew` (mechanics + `KVM_SET_CLOCK` / `CNTVOFF_EL2`).
- Log a handoff to A3: `plugins/replay#Kernel-clocksource-lockdown` and `plugins/replay#Deterministic-RNG` must replace the clock-mechanism bullets ("Advance guest KVM_SET_CLOCK / CNTVOFF_EL2…") with `see: 08-lifecycle.md#Clock-skew` and keep only replay-specific facts (`step_window_instructions`, `deterministic_rng_seed`, "PMU values drive local stepping; never appear in receipts" — that last one is arguably 10's, hand off to A3 too).
- In `08#Suspend-and-resume`, the "Host samples CLOCK_REALTIME and dispatches a paravirtual clock update frame" bullet points to `08#Clock-skew` rather than restating.

### 6. Memory pressure vs suspend
- `01#Host-memory-pressure-watcher` (A1) is the incremental reclaim loop; `08#Suspend-and-resume` is the full page-dump.
- These are distinct. Keep both. Verify `08` does not describe the balloon-convergence loop in detail — that belongs in `08#Balloon-convergence`. If it leaks, fold into the correct home.
- Log a handoff to A1 to keep `01#Host-memory-pressure-watcher` focused on the *pressure watcher* (cgroup v2 / DispatchSource) and to `see:` `08#Balloon-convergence` for the balloon loop.

### 7. Lease schema vs detached lifecycle
- Canonical column set + row kinds: `06-storage-catalog.md#Entity-model` (self-declares "Single Source of Truth for runtime persistence schema").
- Canonical runner behavior: `08#Detached-lifecycle`.
- In `08#Detached-lifecycle`, remove the "row carries session_id, runner child pid, and expires_at lease TTL" restatement and point to `06#Entity-model`. Keep the verbs, the watchdog, and the reattachment protocol.
- Log a handoff to A1: `01#Host-state-root` must reference `06#Entity-model` for the row schema instead of restating it.

### 8. Session incarnation
- Canonical column: `06#Entity-model` (128-bit UUIDv7, minted per spawn).
- Canonical verb semantics: `08#Session-incarnation` (destructive verbs accept `expected incarnation_id`, CAS on catalog row, `E_STALE_INCARNATION`).
- These don't overlap; keep both. Verify neither restates the other. Log a handoff to A1 to trim `01#Host-state-root`'s restatement.

### 9. Rollback
- Canonical: `08-lifecycle.md#Rollback`.
- `09#Archives-and-portability` implicitly covers restore semantics; add a cross-ref from `09` back to `08#Rollback` for the "sockets gone on first access" fact — or move that fact to `08#Rollback` (recommend move, then `see:` from 09).
- Log a handoff to A1: `01#Snapshot-and-rollback-scope` restates "Rollback closes session sockets; guests observe ECONNRESET" → replace with `see: 08-lifecycle.md#Rollback`.

### 10. CoW broker / upper / tombstones / whiteouts
- Canonical: `05-storage-plane.md#Host-CoW-broker`.
- `06#Entity-model` restates "No whiteout, overlay, or union-driver state is ever persisted" and "broker state lives under sessions/<id>/upper and /tombstones". Keep the catalog-specific fact ("catalog stores no rows for broker state"); replace the rest with `see: 05-storage-plane.md#Host-CoW-broker`.
- `08#Rollback` and `09#Archives-and-portability` also describe "broker upper file + tombstones" — cross-ref to `05#Host-CoW-broker` for the mechanics; keep format/behavior specifics (e.g., `upperdir.tar.zst` in 09).

### 11. FUSE opcode table vs write gating
- `05#FUSE-opcode-table` and `05#Write-gating` both touch the FUSE write path. Verify the opcode table is a *lookup table* only and the write semantics stay in `05#Write-gating`. No cross-file duplication.

### 12. `05#Storage-quotas` / `06#Entity-model`
- `06#Entity-model` doesn't mention quotas. Verify no restatement. If `06` is silent, no fix needed.

---

## Agent A3 — Protocol, Ops, Plugins

### 1. Stream kind numeric IDs in plugin leaves
- Canonical: `11-protocol.md#Control-channel`.
- `plugins/mcp`: remove `Kind 0x0012;` — replace with `see: 11-protocol.md#Control-channel`.
- `plugins/db-proxy`: remove `Kind 0x0011;` — same.
- `plugins/profile`: remove `Kind 0x0010;` — same.
- `plugins/commercial`: `cas-snapshot` "Channels: reserved through frozen-table revision; see 11-protocol.md" is already a reference — keep.
- `plugins/wire-ingress`: "Kind 0x0005; see: 11-protocol.md#Control-channel" — remove the inline numeric, keep the `see:`.

### 2. Verdict / FilterAction restatement in plugins
- Canonical enum: `02-plugin-model.md#Unified-verdict` (A1).
- Canonical wire-plane errno mapping: `07-wire-plane.md#Frame-drop-and-forward-semantics` (A2).
- `plugins/chaos#Return-errno-and-rejection-behavior`: replace the `FilterAction::Drop { errno: None }` and `FilterAction::Drop { errno: ECONNREFUSED }` bullets with `see: 07-wire-plane.md#Frame-drop-and-forward-semantics` + the chaos-specific fact ("chaos reserves ECONNREFUSED for RST injection") if that fact is genuinely chaos-owned. If it is wire-plane-general, hand off to A2.
- `plugins/mesh#Return-errno-and-rejection-behavior`: replace "Guest observes ECONNREFUSED: see: 07-wire-plane.md#Frame-drop-and-forward-semantics" — it already references. Trim the "mTLS validation failure / Err(Reject { errno: ECONNREFUSED })" restatement to a single domain sentence + the `see:`.
- `plugins/db-proxy#Error-and-return-semantics`: SQLSTATE mapping is db-proxy domain — keep. Add a cross-ref to `07#Frame-drop-and-forward-semantics` for the underlying wire-plane rule; remove any sentence restating wire-plane behavior.
- `plugins/tool-guard`, `plugins/wire-ingress`, `plugins/vault`, `plugins/transaction`, `plugins/supply-chain`: scan for any sentence restating `Verdict::Pass / Patch / Drop / Suspend / Abort` semantics. Each leaf should name only its *own* errno mapping and point to `02#Unified-verdict` for the enum.

### 3. Tier labels
- `02#Plugin-tiering` is canonical for tier *definitions*.
- Each plugin keeps its own `Tier N <label> (<plugin>)` line — that's a leaf's own tier assignment, not a definition.
- Delete any plugin bullet that *defines* what "Tier 3 cross-cutting aggregator" means. Keep only the label.

### 4. Vault external sidecar
- Canonical: `plugins/vault#Optional-external-egress-rewriting`.
- `plugins/commercial#egress-sidecar`: keep `EgressSidecarConfig`, `Depends: kelir-interface`, `Channels: none`, "runs as a separate OS process". Replace "External egress rewriting daemon (Envoy/Squid wrapper) / Out-of-process / Implements optional L7 rewriting for legacy workloads" with a single `see: plugins/vault.md#Optional-external-egress-rewriting` line plus the interface name.
- Log a handoff to A1 (`01#Trust-boundaries`) and A2 (`07#Vault-and-credential-flow`) to replace their one-liners with `see:` pointers.

### 5. "No PMU in receipts / CausalDelta / Merkle"
- Canonical: `10-attestation.md#Field-semantics` (attestation owns receipts).
- `10#Causal-delta` ("No PMU samples appear in CausalDelta") — keep, same file, cross-ref.
- `14#Metrics-pipeline` ("Never enter receipts, CausalDelta, or Merkle leaves. Never compared across hosts.") — replace the negative invariant with `see: 10-attestation.md#Field-semantics`; **keep** the positive facts (`retired_instructions`, `cycle counts`, `cache misses` are OTel gauges; per-microarchitectural variance expected) — those are 14's own.
- `plugins/ledger#Merkle-WAL` ("No PMU samples appear in any leaf") — replace with `see: 10-attestation.md#Field-semantics`.

### 6. Merkle WAL
- Canonical: `plugins/ledger#Merkle-WAL`.
- Verify no restatement in `10#Causal-delta` or `14`. `10#Causal-delta`'s "Merkle root chaining links granular receipts to state diffs" is a cross-concept fact — keep.
- `10#Causal-delta` and `10#Field-semantics` both mention "Never enter Merkle leaves". Consolidate to one bullet in `10#Field-semantics` and `see:` from `#Causal-delta`.

### 7. MCP 64KB frame
- `plugins/mcp#MCP-contract`: `Max frame: 64 KB; larger split via FLAG_CONTINUATION` — replace with `Max frame: 64 KB (see 11-protocol.md#Wire-framing); larger split via FLAG_CONTINUATION`.

### 8. Boot budget + shim size
- Canonical: `01#Boot-budget` and `01#Zero-workload-intrusion` (A1).
- `15#Enforced-invariants`: keep gate names + `enforce = true` behavior + exit codes. Replace `<60ms`, `<50ms`, "shim musl binary size gate" numbers with `see: 01-architecture.md#Boot-budget` / `see: 01-architecture.md#Zero-workload-intrusion`.
- `11#Guest-paravirtual-micro-init-boundary`: "musl PID 1 (<50KB binary)" → `see: 01-architecture.md#Zero-workload-intrusion`.

### 9. "No guest-side SDK" / "no in-guest daemon"
- Canonical: `01#Zero-workload-intrusion` (A1).
- `12#E2B-compatibility`: "no in-guest shims or daemons" → `see: 01-architecture.md#Zero-workload-intrusion`.
- `12#Process-vs-plugin-boundary`: keep the *adapter-specific* rule ("shim process owns wire; kelir-pack owns image→EROFS translation"). Remove the general "plugin owns translation and policy; the binary owns the wire" if `12#Ingestion-matrix#Two-shapes-one-rule` already states it; otherwise consolidate to one bullet in `12#Ingestion-matrix` and `see:` from the process-boundary section.
- `13-cli`: "Runner commands absent (no kelir run)" — `00#What-it-is-not` and `15#Developer-CLI` both state this. Consolidate to `00` (product-level "not") or `15` (tooling-level "not"); `13-cli` should be a thin `see:` in that bullet.

### 10. "Frozen table" / "frozen stream kinds"
- Canonical: `11#Control-channel` "Core kinds (frozen)".
- `plugins/commercial` already references. `plugins/profile`, `plugins/mcp`, `plugins/db-proxy` — verify no `frozen-table` restatement. If any exists, replace with `see:`.

### 11. CLI `logs` command
- Canonical: `13-cli.md#Inspection-commands` (13 owns CLI surface).
- `14#Log-streaming` ("kelir logs follows the same convention; thin consumer of index.bin") — keep the observability fact ("thin reader over flat log index files", "watchers fan out cleanly"), replace "kelir logs follows the same convention" with `see: 13-cli.md#Inspection-commands`.
- `15#Developer-CLI` ("`kelir logs <session_id>`: Tails session output using cursor index.bin") — replace with `see: 13-cli.md#Inspection-commands` and keep any 15-specific facts (none, probably).

### 12. Adapter boundary
- `12#Ingestion-matrix` and `12#Process-vs-plugin-boundary` both state "the plugin owns translation and policy; the binary owns the wire". Keep the statement in `12#Ingestion-matrix`; make `12#Process-vs-plugin-boundary` a table of `adapter → wire owner → policy owner`, no rule restatement.

### 13. Historical "fluent-builder" coverage
- `15#--fluent-builder-coverage` is explicitly superseded by `#--unified-builder-coverage`.
- **Delete** the duplicate "38 plugin_builder cases" table and the "one per verb + one per scope" enumeration. Keep one paragraph: "History, not a second gate — see `#--unified-builder-coverage` for the current 46-case / 58-vector count."
- Aligns with A1's deletion of the fluent tables in `02` / `03`.

### 14. "Zero-config" / "zero-cooperation" / "zero-alloc" overuse
- Stylistic only. Do **not** do a bulk sweep. If a specific instance contradicts another file's fact, treat that as a substantive fix; otherwise leave alone. Do not delete brand-voice `Zero-X` unless it also restates an invariant owned by another file.

### 15. Traceparent
- `14#Metrics-pipeline` and `plugins/mesh#Traceparent-continuity` — `plugins/mesh` already references `14`. No fix.

### 16. Coverage gates vs invariants
- Verify each `15#--*` gate references its canonical home and does not restate the invariant. `--lifecycle-coverage`, `--action-coverage`, `--hook-coverage` should all be thin.
- `15#Runtime-density-coverage`: keep case file names (`hd-fast-boot-hit`, etc.); drop any sentence that restates `01#Boot-budget` or `05#EROFS-DAX-execution` mechanics.

### 17. `14#PMU` gauge section
- `14#Metrics-pipeline` PMU bullets: keep the "gauges only / microarchitectural variance expected" facts. Delete the "never enter receipts, CausalDelta, Merkle leaves" negative (moved to `10` per fix #5).

---

## Cross-agent handoffs (write to `_collab/conflicts.md`)

Each handoff line format:
```
- from: A<n> | to: A<m> | file: <path> | edit: <one-line description> | canonical: <path#anchor>
```

Required handoffs generated by this work order:

**A1 → A2**
- `01#Snapshot-and-rollback-scope` → trim base-snapshot + rollback restatements (fixes A1-5, A2-9).
- `01#Host-state-root` → replace row-schema and incarnation restatements with `see: 06-storage-catalog.md#Entity-model` (fixes A2-7, A2-8).
- `01#Host-memory-pressure-watcher` → keep pressure-watcher focus; `see: 08-lifecycle.md#Balloon-convergence` for balloon loop (fix A2-6).

**A1 → A3**
- `01#VirtIO-Device-Complex` "Channel allocation is frozen" → `see: 11-protocol.md#Control-channel` (fix A1-7).
- `15#--fluent-builder-coverage` → collapse to a history pointer (fix A1-8 / A3-13).

**A2 → A1**
- `08#Detached-lifecycle` → reference `06#Entity-model` for row schema; A1 confirms `01#Host-state-root` mirrors (fix A2-7).

**A2 → A3**
- `plugins/chaos`, `plugins/mesh` → replace FilterAction errno restatement with `see: 07-wire-plane.md#Frame-drop-and-forward-semantics` (fix A2-2).
- `plugins/replay#Kernel-clocksource-lockdown`, `#Deterministic-RNG` → replace clock-mechanism restatement with `see: 08-lifecycle.md#Clock-skew` (fix A2-5).
- `15#Enforced-invariants` → replace `<60ms` / `<50ms` / shim size with `see:` (fix A2-3).
- `11#Binary-exec-channel` "Port 0 reserved for human" → `see: 04-terminal-plane.md#Terminal-pipeline` (fix A2-1).

**A3 → A1**
- `plugins/*` tier definitions → confirm `02#Plugin-tiering` is canonical (fix A3-3).

**A3 → A2**
- `01#Trust-boundaries` external-sidecar one-liner → `see: plugins/vault.md#Optional-external-egress-rewriting` (fix A3-4, combined with A1's edit).
- `07#Vault-and-credential-flow` external-sidecar line → `see: plugins/vault.md#Optional-external-egress-rewriting` (fix A3-4).

---

## Validation (each agent, before declaring done)

1. `just check-spec` (must match CI flags per `AGENTS.md`).
2. `just check-spec-touched` (concept canEdit set).
3. Grep for the identifiers the agent touched; every remaining occurrence must be either (a) the canonical home, or (b) a `see:` line.
4. No `see:` chain exceeds 1 hop (per `AGENTS.md`).
5. All handoffs in `_collab/conflicts.md` are marked `resolved` by the receiving agent (or explicitly `TBD` if policy-level).
6. Report in `_collab/report-A<n>.md`:
   - Files touched (paths).
   - Redundancies removed (count + one-line each).
   - Canonical homes claimed (path#anchor).
   - Handoffs unresolved.

---

## Definition of done (across all three agents)

- Every redundancy listed above is either removed at the source and referenced at the use-site, **or** explicitly logged as `TBD` in `_collab/conflicts.md` with a rationale.
- No concept has two *definitions* — only one definition + N `see:` pointers.
- No numeric budget appears in more than one file (exception: tests/fixtures under `artifacts/`, which are not spec).
- No enum variant list appears in more than one canonical file.
- `just check-spec` green on all three agents' file sets.
```