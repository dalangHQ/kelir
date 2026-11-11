# work-order-4 — Agent A3 report (Protocol, Ops, Plugins)

Branch: temp/wo4-c. Base: 339ca53 (origin/scaffolding tip). Commits df80688..7e32c28.

## Files touched

- cans/10-attestation.md
- cans/11-protocol.md
- cans/12-adapters.md
- cans/13-cli.md
- cans/14-observability.md
- cans/15-operations.md
- cans/plugins/chaos.md
- cans/plugins/commercial.md
- cans/plugins/db-proxy.md
- cans/plugins/ledger.md
- cans/plugins/mcp.md
- cans/plugins/profile.md
- cans/plugins/replay.md
- cans/plugins/tool-guard.md
- cans/plugins/wire-ingress.md
- cans/_collab/conflicts.md (append-only: "work-order-4 A3 handoffs" section)
- cans/_collab/report-A3.md (this file)

Untouched by design: vault.md, mesh.md's non-errno sections, container.md, transaction.md, supply-chain.md (scanned per A3-2/A3-14, no restatements found), all cans/00-09, AGENTS.md files, _rules.yaml, all code.

## Fixes 1-17 status

All 17 implemented or verified:

- A3-1 done (mcp, db-proxy, profile, wire-ingress numerics removed; commercial cas-snapshot kept as-is per work order; commercial openshell-coordinator "Channels: 0x0010 (profile-sync)" also normalized to the symbolic kind + pointer).
- A3-2 done (chaos, mesh, db-proxy, tool-guard; scanned wire-ingress, vault, transaction, supply-chain, container, ledger, replay, profile, mcp — only domain errno facts remain in leaves).
- A3-3 done as verification: every plugin leaf carries only its assignment label; no tier definitions exist in leaves.
- A3-4 done (commercial egress-sidecar collapses to see: vault; EgressSidecarConfig / Depends / Channels / "separate OS process" kept).
- A3-5 done (10#Field-semantics is the canonical PMU-in-receipts/CausalDelta/Merkle invariant; 14 and ledger see: there; 10#Causal-delta keeps its line + intra-file see:).
- A3-6 done (10's two Merkle mentions consolidated into one canonical bullet; #Causal-delta points at it).
- A3-7 done (mcp max-frame bullet carries the 11#Wire-framing pointer; FLAG_CONTINUATION preserved — it is referenced from crates/kelir-plugins/plugin-tool-guard).
- A3-8 done (15#Enforced-invariants keeps gate names, enforce = true, exit codes; <500µs/<60ms/<50ms/<2µs replaced with pointers; 11#Guest-paravirtual-micro-init-boundary "<50KB binary" → 01#Zero-workload-intrusion).
- A3-9 done (12#E2B "no in-guest shims or daemons" → see: 01#Zero-workload-intrusion; 12#Process-vs-plugin-boundary keeps only adapter-specific rows; 13-cli "no kelir run" bullet verified already thin at base — 00 and 13 both point at 15#Developer-CLI, the single statement).
- A3-10 done as verification: no frozen-table restatement in profile/mcp/db-proxy; commercial already references.
- A3-11 done (13#Inspection-commands is the logs home; 14 and 15 see: there).
- A3-12 done (#Process-vs-plugin-boundary is a pure adapter → wire owner → policy owner table; the general rule stays once in #Ingestion-matrix).
- A3-13 done (--fluent-builder-coverage and Fluent-builder-parity-vectors collapse to history pointers at the unified-builder sections).
- A3-14 done as verification: no bulk sweep; no Zero-X instance in my files contradicts another file's fact.
- A3-15 done as verification: plugins/mesh already references 14#Metrics-pipeline.
- A3-16 done as verification: --lifecycle/--action/--hook coverage gates are thin (assert + home pointer); Runtime-density coverage keeps case file names, restates no budget or EROFS-DAX mechanics.
- A3-17 done (14#Metrics-pipeline keeps gauges-only + variance facts; the negative invariants moved to 10 per A3-5).

## Redundancies removed (24)

1. plugins/mcp: inline "Kind 0x0012" numeric (registry home: 11#Control-channel).
2. plugins/db-proxy: inline "Kind 0x0011".
3. plugins/profile: inline "Kind 0x0010".
4. plugins/wire-ingress: inline "Kind 0x0005".
5. plugins/commercial: openshell-coordinator "Channels: 0x0010 (profile-sync)" numeric form.
6. plugins/chaos: "Silently discards frame, injects no RST, surfaces no errno" (07's Drop{None} mapping).
7. plugins/chaos: "FilterAction::Drop { errno: ECONNREFUSED } retains policy semantics" (07's mapping; chaos-owned errno choice kept).
8. plugins/mesh: "Guest observes ECONNREFUSED:" observation clause (07's fact).
9. plugins/db-proxy: "engine Verdict (Pass | Patch | Drop)" enum subset (02#Unified-verdict).
10. plugins/db-proxy: "Frame bytes mutated in place before upstream forward" (07's in-place rule).
11. plugins/tool-guard: ExecveMutation field list (code-owned layout; also carried the stale set_env HashMap claim from conflict C.2).
12. plugins/commercial egress-sidecar: "External egress rewriting daemon (Envoy/Squid wrapper)".
13. plugins/commercial egress-sidecar: "Out-of-process; runs outside the supervisor binary".
14. plugins/commercial egress-sidecar: "Implements optional L7 rewriting for legacy workloads".
15. 14#Metrics-pipeline: "Never enter receipts, CausalDelta, or Merkle leaves".
16. 14#Metrics-pipeline: "Never compared across hosts" (10#Field-semantics owns it).
17. plugins/ledger: "No PMU samples appear in any leaf" (now a see:).
18. 10#Causal-delta + 10#Field-semantics: two partial PMU/Merkle statements consolidated into one canonical bullet.
19. plugins/replay: KVM_SET_CLOCK / CNTVOFF_EL2 advance bullet (08#Clock-skew).
20. plugins/replay: pinned clocksource boot parameters line (08#Clock-skew states them).
21. 15#Enforced-invariants: <500µs, <60ms (x2), <50ms, <2µs budget numbers.
22. 11#Guest-paravirtual-micro-init-boundary: "<50KB binary" (01#Zero-workload-intrusion).
23. 11#Binary-exec-channel: two port-0-for-human restatements ("stays reserved ... break-glass attach only", "remains the interactive human TTY") — one bare pointer to 04 remains.
24. 15: fluent-builder duplicate case table (10 lines) + duplicate vector-family block (11 lines) + 14/15 "kelir logs" restatements; 12#Process-vs-plugin-boundary rule-instantiation sentences rewritten as a pure owner table.

## Canonical homes claimed (A3-owned)

- cans/10-attestation.md#Field-semantics — PMU samples never enter receipts, CausalDelta, or Merkle leaves; never compared across hosts.
- cans/11-protocol.md#Control-channel — stream kind registry + frozen kind table (plugin leaves defer).
- cans/11-protocol.md#Wire-framing — 64KB max frame (mcp defers).
- cans/12-adapters.md#Ingestion-matrix — adapter boundary rule; #Process-vs-plugin-boundary is its owner table.
- cans/13-cli.md#Inspection-commands — kelir logs command surface (14/15 defer).
- cans/plugins/vault.md#Optional-external-egress-rewriting — external egress sidecar (commercial defers).
- Plugin leaves keep their own: Tier N assignment labels, domain errno/SQLSTATE mappings (db-proxy SQLSTATE, vault errnos, mcp JSON-RPC codes, chaos errno choices), replay-specific stepping facts.

## Baseline finding fixed

WARNING cans/plugins/profile.md:79 content.tbd.max ("11 TBD nodes exceed 10") — fixed via path (a): the two driver.ffi_boundary TBD nodes (queue_capacity, max_buffer_bytes) state the SAME unknown ("no DriverPolicy field yet") and were consolidated into one node. TBD count 11 → 10; the warning is gone; no unknown deleted; max_tbd_per_file untouched.

## Handoffs unresolved

Assigned (receiving agent owns the edit; logged in conflicts.md):
- A3 → A1: cans/01-architecture.md#Trust-boundaries external-sidecar one-liner → see: plugins/vault.md#Optional-external-egress-rewriting.
- A3 → A2: cans/07-wire-plane.md#Vault-and-credential-flow external-sidecar line → see: form of the same pointer.

Open observations (logged, no policy decision needed from this lane):
- crates/kelir-testkit/anchors.json (generated) is stale after spec edits; needs `cargo run -p kelir-testkit --bin anchors` at converge (cargo unavailable here).
- plugins/mesh tier label "Tier 1-adjacent" vs 02#Plugin-tiering's plain Tier 1 listing.
- tool-guard C.2 residual: envp-vs-set_env encoding question still TBD with the messages.rs owner (the misstating prose is now gone).
- plugins/ledger "Determinism comes from logical trace, seeded RNG, frozen clock" is a near-duplicate of 10#Identity (left: not enumerated by the work order).

## Gate results

- `cans check --refs-only --json`: ok: true, exitCode: 0, refs 527, broken 0, deepHops 0. Green at base, green now.
- `cans check --json`: 1 error + 1 warning, both pre-existing and outside my files (02-plugin-model.md:426 node_length 107>90 — A1; 03-sdk-architecture.md:175 siblings 21>20 — A1). Zero findings attributable to cans/10-15 or cans/plugins/*; the profile.md TBD warning is cleared.
- Known gap noted: `--touched-files` does not exist in cans 0.21.0 (justfile is ahead of the CLI); `just`/cargo not installed here, so code gates were not run.

## Checker constraint discovered (important for the merge lead)

cans 0.21.0's deep-hop rule (refs.ts): a file that is itself see:-referenced must not emit see: refs outside its own strongly-connected mesh. 04-terminal-plane.md is mesh-terminal (refs only artifacts), therefore:
- 11 → 04 cannot be a formal see: (01→11 exists) — implemented as a bare-mention pointer (commit 38af749).
- 13 → 04 blocked 13 from being referenced at all; 13's Stream-guarantee ref was downgraded to a bare mention so 14/15 can see: 13#Inspection-commands (A3-11).
One intermediate push (457135a) briefly carried a red refs-only gate (deep hop 10→14→04) and was fixed in the follow-up commit 80b5d5c; final tree is green.
