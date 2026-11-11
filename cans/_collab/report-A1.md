# work-order-4 A1 report — Foundation & SDK

## Files touched
- cans/00-overview.md
- cans/01-architecture.md
- cans/02-plugin-model.md
- cans/03-sdk-architecture.md
- cans/_collab/conflicts.md (append-only, "## work-order-4 A1 handoffs")

## Baseline findings fixed
- ERROR cans/02-plugin-model.md:426 node_length (107 > 90): fixed — split into two bullets (trap-scope-entry / host trap(filter, f) name clash, E0592).
- WARNING cans/03-sdk-architecture.md:175 "Rust SDK" 21 children: fixed — merged the two .plugin() bullets into one parent with children (21 → 20). The session-surfaces merge alone did not touch that node (work-order guess was wrong); consolidation done inside my ownership without content loss.

## Redundancies removed (24)
1. 01#FFI-boundary-economics <2µs restatement → see: 02#Latency-ceiling
2. 01#Host-plane-subsystem "Gate budget ... <2µs" → see: 02#Latency-ceiling
3. 02#Unified-interposition-pipeline "Fast Filter budget: <2µs; Dispatcher budget: <1µs" + separate ceiling pointer → single "Gate budgets: see: #Latency-ceiling"; the <1µs dispatcher number moved INTO #Latency-ceiling (sole numeric home)
4. 03 Python prerequisite check "<2µs" → split: O(plugins)/boot/once facts kept, budget → see: 02#Latency-ceiling
5. 01#Workload-contract "no guest-side SDK, shim, or agent" → see: #Zero-workload-intrusion
6. 01#Params-ingress-contract "No guest-side SDKs, CLI shims, or agents" → see: #Zero-workload-intrusion
7. 00#What-it-is-not daemon paraphrase merged: "Zero background daemon architecture: see: 01-architecture.md#In-process-microkernel" (duplicate "Standalone from host daemon runtimes" line removed)
8. 01#Capability-taxonomy "Plugin" node → single see: 02#Plugin-tiering (Native/Foreign definitions and tier phrasing deleted; [1] footnote retired, trait-contract line expanded to full path)
9. 02 monotonic-order: canonical bullet added to #Unified-verdict; #Dual-plane-execution-architecture and #Static-dispatch now point instead of restating
10. 02#Foreign-plugin-proxy "Pass, Patch, Drop, Suspend, Abort" enumeration → see: #Unified-verdict
11. 03#Unified-verdict-surface: framing line "naming deltas only, not semantics"; split Python line merged
12. 02#Unified-interposition-pipeline Stage 1/2/3 → Gate 1/2/3 (one term, per 11#Trap-fast-path)
13. 01#Snapshot-and-rollback-scope: ECONNRESET rollback restatement deleted (see: 08#Rollback); base-snapshot invisibility + CausalDelta exclusion → see: 09 (see conflicts for anchor spelling); labelled fast-boot-cache pointer
14. 01#VirtIO-Device-Complex "Channel allocation is frozen" → see: [3]=11#Control-channel; malformed line "see [3]    - Primary VirtIO Bus" split; "Primary VirtIO Bus" restored as the device-group node
15. 01#Host-state-root: 4 row-schema/incarnation restatements deleted; "Lease schema and row kinds" → see: 06#Entity-model
16. 01#Host-memory-pressure-watcher: balloon loop lead → see: 08#Balloon-convergence; internal duplicate madvise(MADV_DONTNEED) line removed
17. 01#Trust-boundaries sidecar one-liner → "Optional external sidecar: see: [4]=plugins/vault#Optional-external-egress-rewriting"
18. 02#Fluent-plugin-creation-builder: verb/scope tables removed from history — #Chainable-verbs and #Nested-scopes MOVED under #Unified-builder (unchanged titles; all inbound refs incl. 15-operations and code still resolve); history collapsed to shape paragraph + historical entry points + historical fork_with TS/Py signatures (Rust dup deleted, referenced)
19. 02 fluent history: "Context, prerequisite, scope, fork, and vsock service verbs" dup deleted; "Verb and scope names below stay current" line dropped (now structural)
20. 03#Fluent-plugin-surface: Closed-set/Structural-verbs/Ring-verbs/Scopes blocks deleted (verbatim dups of #Unified-builder-surface); fork_with historical shapes deleted (home: 02 fluent history); post-.build()/prerequisite dups → pointers to 02#Terminal-build / #Compile-time-prerequisites
21. 03#Session-surfaces: one verb→see: table with data | control row markers; Rollback-semantics subtree collapsed to one row (+ "no socket-preserving flag" kept); both group node titles preserved (code refs from kelir-bindings pyo3/napi)
22. 02#Coverage-matrix → pure plugin→example map (tier phrase out of title); "Userland SDK policy demonstrations" folded in as two map rows
23. 03#Unified-builder-surface duplicate "Entry-point names are the group names" line deleted (home: 02#Nested-scopes)
24. 02:426 split + 03 Rust-SDK child merge (baseline findings above)

## Canonical homes claimed (A1)
- 02-plugin-model.md#Latency-ceiling — all gate latency numbers (now incl. Gate 2 dispatcher <1µs)
- 01-architecture.md#Zero-workload-intrusion — no-guest-SDK invariant
- 02-plugin-model.md#Plugin-tiering — tier + Native/Foreign definitions
- 02-plugin-model.md#Unified-verdict — verdict/FilterAction enum + monotonic evaluation order
- 02-plugin-model.md#Chainable-verbs / #Nested-scopes — verb + scope name homes (relocated under #Unified-builder)
- 02-plugin-model.md#First-class-plugins (#Catalog-tiering) — plugin roster
- 01-architecture.md#Boot-budget — untouched, remains canonical (boot numbers stay)

## Notes / deviations
- Fix 2's 03 part was a no-op: no bullet in 03#Session-surfaces or 03#Params-typed-views restates "no guest SDK" (verified by grep).
- Fix 5's "digest recorded in host-side cache manifest" fact already lives at 09#Base-snapshot, so 01 does not restate it (restating would create a new duplication).
- "Snapshots capture compute + disk, not distributed network state" (01) left in place: fix A1-5/A2-9 name only the base-snapshot and rollback restatements; flagged here for a future pass.
- cans --touched-files does not exist in 0.21.0 (justfile ahead of CLI); write envelope followed work-order ownership. budget write not run (ownership decided by work order where they disagree, per tasking).

## Handoffs unresolved (see conflicts.md "work-order-4 A1 handoffs")
- A2: 3 assigned (snapshot/rollback split, host-state-root schema pointer, pressure-watcher focus) + 2 open (06 deep-hop unblock via A2-10; optional 09 node retitle)
- A3: 2 assigned (11 frozen-canonical confirmation; 15#--fluent-builder-coverage collapse)
- lead: anchors.json regeneration; --touched-files CLI gap

## Gate results (worktree temp/wo4-a @ 0501be1 + collab files)
- cans check --refs-only --json: ok:false, exit 2 — refs 522 total, broken 0, deepHops 1 (01-architecture.md:249 → 06-storage-catalog.md; externally unblockable from A1 lane, see conflicts entry; clears once 06 gains its first outbound see: per A2-10)
- cans check --json: errors 1 (the same deep hop), warnings 1 (cans/plugins/profile.md:79 tbd cap — A3's file, out of scope). Zero structure/style/refs findings attributable to 00/01/02/03.
