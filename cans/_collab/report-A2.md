# work-order-4 — Agent A2 (Planes & Lifecycle) report

Branch: temp/wo4-b (base 339ca53 = origin/scaffolding tip). Spec edits landed at 9b3ffb3.

## Files touched

- cans/04-terminal-plane.md
- cans/05-storage-plane.md (verified only — zero edits; canonical CoW broker home already correct)
- cans/06-storage-catalog.md
- cans/07-wire-plane.md
- cans/08-lifecycle.md
- cans/09-snapshots.md
- cans/_collab/conflicts.md (append-only: "work-order-4 A2 handoffs" section)

## Redundancies removed (17)

1. 08#Break-glass definitional sentence ("sole in-process break-glass surface") → see: 04#Terminal-pipeline (fix 1).
2. 04#Binary-exec-channel "Port 0 remains the sole human break-glass console" duplicate → see: #Terminal-pipeline (fix 1, intra-04).
3. 08#Autonomous-fast-boot-hydration `<500µs` budget restatement → see: 01-architecture.md#Boot-budget; mmap mechanism kept (fix 3).
4. 09#Base-snapshot "Stored under cas/blobs/" cache-path restatement → see: 08#Autonomous-fast-boot-hydration; parent_digest: None format fact kept (fix 4).
5. 08#Suspend-and-resume CLOCK_REALTIME sampling/dispatch restatement → fact moved to 08#Clock-skew ("Resync origin"), see: back-pointer (fix 5).
6. 08#Lid-sleep/wake resync-frame restatement + 2-hop via #Suspend-and-resume → direct see: #Clock-skew (fix 5).
7. 08#Detached-lifecycle "Row carries session_id, runner child pid, and expires_at lease TTL" → see: 06#Entity-model (fix 7).
8. 08#Detached-lifecycle Liveness block: row-kinds enumeration, ephemeral/detached reap semantics, "share one row schema" → see: 06#Entity-model; platform reap mechanisms kept (fix 7).
9. 08#Session-incarnation "mints a 128-bit UUIDv7 incarnation_id" column restatement → see: 06#Entity-model; verb semantics kept (fix 8).
10. 09#Archives "sockets gone on first access" → fact moved to 08#Rollback, see: from 09 restore semantics (fix 9).
11. 07#Wire-architecture#Rollback-interaction 5-line rollback network-truth restatement → single pointer line "see: 08-lifecycle.md#Rollback" (fix 9; canonical rollback home is 08).
12. 06#Entity-model broker whiteout/state-paths restatement → see: 05#Host-CoW-broker; "catalog stores no rows" + commit flow kept (fix 10).
13. 08#Rollback three-phase storage reset (upper file + tombstones + FALLOC_FL_PUNCH_HOLE purge) → see: 05#Host-CoW-broker; phase ordering kept (fix 10).
14. 08#Delta-chain-compaction StepVerdict::Rollback "discarding the broker upper file + tombstones" tail → trimmed to session.rollback() + <8ms RAM fact (fix 10).
15. 09#Archives "upperdir.tar.zst is broker state: sparse upper file + tombstones" mechanics → see: 05#Host-CoW-broker (fix 10).
16. 07#Vault-and-credential-flow external-sidecar one-liner → see: plugins/vault.md#Optional-external-egress-rewriting (receiving handoff A3-4).
17. 04#Binary-exec-channel vague "Sub-lane framing defined on EXEC stream kind" → see: 11-protocol.md#Binary-exec-channel (genuine pointer; also resolves the refs.deep_hop flag, see notes).

Fixes 11 and 12 verified, no edit required: 05#FUSE-opcode-table is a lookup table (the WRITE reply-size fact is UAPI protocol, write gating stays in 05#Write-gating, no duplication); 06#Entity-model is silent on quotas.

## Canonical homes claimed (A2-owned, per work order)

- Port-0 break-glass: 04-terminal-plane.md#Terminal-pipeline (+ 04#Binary-exec-channel for the "PTY reserved for human attach" clause)
- FilterAction wire-plane errno mapping: 07-wire-plane.md#Frame-drop-and-forward-semantics
- Wire-plane L4 diversion: 07-wire-plane.md#Transparent-L4-stream-diversion (unchanged; no restatements found in A2 files)
- Clock skew mechanics: 08-lifecycle.md#Clock-invariants + 08-lifecycle.md#Clock-skew (now also owns the CLOCK_REALTIME resync origin)
- Rollback: 08-lifecycle.md#Rollback (now also owns restore/import socket loss)
- Base snapshot format: 09-snapshots.md#Base-snapshot
- Lease schema + incarnation column + row kinds: 06-storage-catalog.md#Entity-model
- Detached runner behavior: 08-lifecycle.md#Detached-lifecycle
- CoW broker: 05-storage-plane.md#Host-CoW-broker
- Balloon convergence: 08-lifecycle.md#Balloon-convergence (verified: no balloon-loop leak into 08#Suspend-and-resume)

## Handoffs unresolved (waiting on receiving lanes)

All logged in cans/_collab/conflicts.md under "work-order-4 A2 handoffs":
- to A3: 11#Binary-exec-channel (fix A2-1); plugins/chaos + plugins/mesh errno (item A3-2); 15#Enforced-invariants budgets (item A3-8); plugins/replay#Kernel-clocksource-lockdown + #Deterministic-RNG (fix A2-5, PMU routing shared with A3-5).
- to A1: 01#Host-state-root row-schema + incarnation restatements (fixes A2-7/A2-8); 01#Snapshot-and-rollback-scope socket restatement (fix A2-9); 01#Host-memory-pressure-watcher balloon loop (fix A2-6).
- Receiving side fully implemented in A2's lane: base-snapshot split (fix 4), sockets-gone move (fix 9), 07 vault sidecar (A3-4).

## Gate results (cans 0.21.0, run on final tree)

- `cans check --refs-only --json`: ok=true, exitCode=0 — refs total 532, broken 0, deepHops 0.
- `cans check --json`: ok=false, exitCode=2 — exactly the 3 pre-existing baseline findings, all outside A2 ownership: 02-plugin-model.md:426 node_length error (107>90), 03-sdk-architecture.md:175 siblings warning (21>20), plugins/profile.md:79 tbd warning (11>10). Zero findings in A2's six files.
- `just`/cargo not installed in this sandbox (code gates not attempted, per briefing). `cans check --touched-files` does not exist in 0.21.0 (known justfile/CLI gap, noted, not fought).

## Notes for the merge lead

- refs.deep_hop mechanics: 04 had no outbound spec refs, so the work-order-mandated 08→04 pointer flagged DEEP HOP (01→08→04). Fixed by adding the genuine 04→11 sub-lane-framing pointer, which merges 04 into the 24-file ref SCC. A3's mandated 11→04 edit (fix A2-1) is now legal too, but only while 04 keeps at least one outbound ref into the mesh — flagged in conflicts.md.
- budget write disagrees with work-order ownership on two concepts (break-glass home: budget says 08#Break-glass, work order says 04; lease-schema home: budget says 01#Host-state-root, work order says 06#Entity-model). Work order followed; both logged in conflicts.md.
- crates/kelir-testkit/anchors.json is now stale (node adds/retitles/removals in 04/06/07/08/09). Regenerate at converge with `cargo run -p kelir-testkit --bin anchors` (cargo unavailable here; crates/ outside A2 ownership).
- Observations outside A2 lane, not in the work order, no conflicts.md entry (report-only): 14-observability.md "Health signals" restates the 06 lease row-kinds/reaping block; plugins/mcp.md line 29 restates the Port-0 break-glass one-liner; plugins/transaction.md restates broker upper/tombstone commit-discard mechanics; FLAG_ALLOC_PTY per-exec-PTY facts are stated in both 04#Binary-exec-channel and 11#Binary-exec-channel (11 is A3's; 04's line is the terminal-plane behavior view).
- 08#Rollback "State boundary" enumeration still names "broker upper file, tombstones" — intentional: it is the rollback-scope list, not the broker mechanics (mechanics live in 05#Host-CoW-broker via the storage-phase see:).
