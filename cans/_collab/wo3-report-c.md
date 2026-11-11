# WO3-C report — kelir-core wire/storage L2 gaps (Agent C, 2026-11-11)

Branch `temp/wo3-c`, base 339ca53. All tasks C1–C10 executed; L1/L2 only, no `TODO (L3)` bodies filled.
Merged `origin/temp/wo3-a` twice for C6 (first at 855c52b — A1/A4/TrapEvent-boxing; second at 48b54df — A2/A3/A6/A8/A9), both merges clean.

## Per-task outcome

- **C1 — process_outbound_frame honors allow_udp_proxied. DONE.**
  `FrameClass::UdpOther` split out of the egress drop group: `WireWiring.allow_udp_proxied` set → `FilterAction::Forward` (NAT translation stays L3, owned by the pre-existing `udp_nat_alloc` TODO); unset → Drop with `errno: None`. Inbound `UdpOther` stays default-deny per CANS/07 #Default-deny-admission (spec drops unencapsulated UDP at the *ingress* ring; task is outbound-only). Anchor `// see: cans/07-wire-plane.md#UDP-NAT-semantics` added on the arm. Key decision: no new TODO marker — the NAT deferral already has an owner (`udp_nat_alloc`), keeps TODO count at baseline.
- **C2 — forward_port real kernel bind. DONE.**
  New enclave module `crates/kelir-transport/src/host_bind.rs` (`libc` only — already a transport dep; no Cargo.toml change): `socket(SOCK_STREAM|SOCK_CLOEXEC)` → `setsockopt SO_REUSEADDR` + `SO_REUSEPORT` (spec-mandated) → `bind(127.0.0.1:port)` → `listen(128)` → `getsockname`; returns `HostListener` owning the fd so the port stays held. `WirePlane::forward_port` binds **before** inserting into the HashMap (kept as logical registry) and stores the listener in a second map; `Err(WireError::PortOccupied)` only on registry duplicate or actual `EADDRINUSE` from bind(2); other bind errors → `PluginError::Halt`. Ephemeral `None`/`0` now binds port 0 and takes the kernel-allocated port (CANS/07 "zero collision probability"), so the `EPHEMERAL_PORT_MIN` scan, its const, and `WireError::EphemeralExhausted` were **deleted** (verified zero external users). `quiesce()` clears the listener map (fd close = instant release; SO_LINGER-RST on accepted sockets stays L3). Key decisions: (a) helper in kelir-transport per the C2 enclave authorization — clean L2 achieved, no TODO (L3) needed; (b) bind address = loopback (wp_16 skip-reason evidence probes 127.0.0.1; no external-bind opt-in exists in the spec; security default); (c) SO_REUSEPORT means two WirePlanes can co-bind the same port by design — PortOccupied correctness relies on the registry + non-REUSEPORT holders producing real EADDRINUSE.
- **C3 — resolve_synthetic_dns NXDOMAIN default. DONE.**
  New parsed-query policy path `WirePlane::resolve_query(&mut DnsQuery)`: `sinkhole_drop_all` OR (allowlist configured AND name not matched) → synthesized `NxDomain` (+ EDNS0 downgrade); allowed → DO stripped pre-upstream, then `ServFail` with `// TODO (L3):` on the unwired upstream relay. `resolve_synthetic_dns(payload)` keeps the wire-parse `TODO (L3)` (delegates to resolve_query once `DnsQuery::parse` lands) and returns ServFail only as the unwired-upstream answer. `domain_matches` supports exact, `*`, and label-boundary suffix (wildcard suffix mirrors egress rules). Key decision: **no explicit `sinkhole_denylist` WireWiring field** — it would break the field-complete struct literals in `crates/kelir/src/builder_sdk.rs` (Agent B lane) and `crates/kelir-bindings/napi/src/lib.rs`; the spec-observable denylist behavior is the inverse-allowlist lookup ("undeclared external targets return synthetic NXDOMAIN"; wp_02 artifact encodes exactly this). Explicit denylist plumbing needs a profile-schema + SDK change owned by others — recorded in conflicts.md.
- **C4 — EDNS0 downgrade plumbing. DONE.**
  `DnsResponse::downgrade_edns0()` added (sets `edns_downgraded`; wire-byte AD-bit/OPT-DO clearing stays `TODO (L3)`). Query-side `DnsQuery::downgrade_edns0` is invoked on the allowed path before upstream; synthesized NXDOMAIN and ServFail responses both run the downgrade so AD never reaches the guest.
- **C5 — dispatch_fs_write scope → MountScope. VERIFY-ONLY, no action.**
  Live check: `dispatch_fs_write` already routes through `fs_scope_allows` (device/binary/path checks) — untouched. C1 needed no `MountScope` net fields (it uses `WireWiring.allow_udp_proxied`). A4's `net_cidrs`/`net_ports`/`allows_net(ip,port)` verified live post-merge; consumed by C6 only.
- **C6 — dispatch_net_outbound / dispatch_net_inbound scope. DONE (after merge of A4).**
  `net_remote_endpoint(frame, inbound)` parses Eth/IPv4/(TCP|UDP) headers via `kelir_interface::frame_layout_views` (`layout_wire` module made `pub(crate)` so parser constants stay SSOT — one-word change in my own wire/mod.rs); remote endpoint = (dst ip, dst port) outbound, (src ip, src port) inbound. Each interceptor is gated by `scope.allows_net(ip, port)` **before** the FFI submit, after the device-mask check. Key decision — unparseable frames (non-IP/short/non-TCP-UDP): bypass hooks **only when** `net_cidrs`/`net_ports` are declared (fail-closed); when no rules are declared frames keep flowing (preserves `pe_08` and `ffi_queue_saturation` semantics — verified live). Stale `TODO (L3): CIDR/port scope rules need a frame parser` deleted (fulfilled). No traffic shaping. New integration test `crates/kelir-core/tests/wire_scope_test.rs` (plain `#[test]`, no `#[cfg(test)]`, black-box through `dispatch_net_*`): 8 cases — in/out CIDR, out port, inbound in/out, undeclared-scope passthrough, non-IP bypass under declared rules, per-interceptor mixed scopes. All pass.
- **C7 — FuseReply::Written(size). DONE.**
  `FilterAction::Forward` branch now returns `FuseReply::Written(request.size)` (fuse_write_out.size) instead of `request.data.len() as u64`. Verified live: `Drop { errno: None }` branch already used `request.size`; no test asserted the old value.
- **C8 — Session::boot double registration. DONE — flagged as bug fix.**
  `boot_with_engine` called `register_session` unconditionally → same `session_id` boot (restore path; also minted-uuid collision across recycled pids in the same ms) aborted VM boot on the catalog UNIQUE constraint. Now: `CatalogDb::open` → `lookup_session(&session_id)` pre-check → insert only when absent, `tracing::warn!` on the idempotent path. kelir-catalog (Agent E lane) untouched — no `INSERT OR IGNORE` needed.
- **C9 — CheckpointPlane::commit_step / restore. SKIPPED per work order (L3 per CANS/08).** No TODO touched, no code change.
- **C10 — SnapStore/.kelir-snap archive. VERIFY-ONLY, done.**
  `SnapshotDescriptor` fields verified against CANS/09: `dirty_bytes: u64` byte count ✓, `parent_digest: Option<String>` ✓ (None for base snapshots), `id`/`created_at_ns`/`guest_clock: Option<GuestClock>` ✓ (guest_clock.json in the archive stream). A9's delegated `// see: cans/09-snapshots.md#Archives-and-portability` anchor is live on my branch post-merge. `Session::export_snapshot` stays `Err(...L3)` as ordered.

## Gate results (all green)

| Command | Result |
|---|---|
| `cargo check -p kelir-core` | PASS (after invalidating stale shared-target-dir rmeta — see note) |
| `cargo clippy -p kelir-core --all-targets -- -D warnings` | PASS |
| `cargo clippy -p kelir-transport --all-targets -- -D warnings` | PASS (new unsafe enclave module) |
| `cargo fmt -p kelir-core -p kelir-transport` | PASS (committed; no residual diff) |
| `cargo test -p kelir-core --test wire_scope_test` | PASS 8/8 |
| `cargo test -p kelir-core` (full crate) | PASS 68 passed / 0 failed / 26 ignored (incl. un-ignored wp_16) |
| `cargo check -p kelir` (cross-lane sanity for my API deletions) | PASS |
| comment-law greps (AGENTS.md §3) on my files | 0 prose comments (the 6 remaining tree-wide are pre-existing in vectors_testkit.rs, D1's lane) |

Note: `cargo check -p kelir-core` transiently failed with phantom E0599 (`no method allows_net`) — a sibling worktree's pre-A4 `kelir-interface` rmeta in the shared `CARGO_TARGET_DIR`; fixed by touching the interface sources (fingerprint invalidation). Recorded in conflicts.md for the convergence runner.

## Files changed (git diff --stat 339ca53..HEAD)

```
cans/_collab/conflicts.md                          |  12 +   (A's entries; mine appended after this report)
cans/plugins/tool-guard.md                         |   2 +-  (A1, via merge)
crates/kelir-core/src/dispatch_plugin/mod.rs       |  46 +-   (C6)
crates/kelir-core/src/planes/storage/fuse_storage.rs|  3 +-  (C7 + A8 offset:0 via merge)
crates/kelir-core/src/planes/wire/mod.rs           | 146 +-   (C1-C4)
crates/kelir-core/src/session_lifecycle.rs         |  38 +-   (C8)
crates/kelir-core/tests/wire_l4_diversion_test.rs  |  15 +-   (wp_14/wp_16 real-bind alignment)
crates/kelir-core/tests/wire_scope_test.rs         | 246 +   (C6, new)
crates/kelir-interface/src/*                       |  (A's lane via merges)
crates/kelir-plugin-testkit/src/vectors_testkit.rs |  12 +-  (A, via merge)
crates/kelir-transport/src/host_bind.rs            |  90 +   (C2, new enclave module)
crates/kelir-transport/src/lib.rs                  |   1 +   (pub mod host_bind)
```
No Cargo.toml / Cargo.lock changes (libc already a kelir-transport dep).

## Commits (339ca53..HEAD, temp/wo3-c)

```
48b54df Merge remote-tracking branch 'origin/temp/wo3-a' into temp/wo3-c   (2nd merge: A2/A3/A6/A8/A9)
55f6e58 WO3-C1: drop redundant NAT TODO; deferral owned by udp_nat_alloc marker
eb17124 WO3-A: decisions A1-A9 + clippy fix (A's commit, via merge)
1522a39 WO3-C6: dispatch_net_outbound/inbound enforce MountScope net_cidrs/net_ports ...
6ff66f3/920f069/3d9390b/5b7fe51  (A's commits, via merges)
855c52b Merge remote-tracking branch 'origin/temp/wo3-a' into temp/wo3-c   (1st merge: A1/A4/box)
faa7b3e WO3-A: box GuestToHost::TrapEvent payload (A, via merge)
867ada5 WO3-C7+C8: FUSE WRITE reply size uses fuse_write_out request.size; idempotent boot registration
0a66799 WO3-C3+C4: DNS sinkhole policy path returns NXDOMAIN ...; DnsResponse EDNS0 downgrade
136f8e7/3803676 WO3-A1/WO3-A4 (A's commits, via merge)
4b90b4d WO3-C1+C2: outbound UDP honors allow_udp_proxied; forward_port real bind
```
Plus the final commit carrying this report + the conflicts.md append.

## TODO (L3) count

- Before (339ca53): **519**
- After (HEAD incl. both A merges): **519** — unchanged (DoD satisfied). My scoped files: +5 added / −5 removed (removed: 2 forward_port bind TODOs, dispatch CIDR TODO, old resolve TODO, with_wiring sinkhole clause; added: resolve_synthetic_dns delegation, NXDOMAIN wire synthesis, upstream relay, AD/OPT-DO wire clearing, with_wiring reword).

## Skipped / deferred / blocked — needs orchestrator attention

1. **Not blocked: C6 completed** after merging origin/temp/wo3-a (A4 fields present at 3803676).
2. **Pre-existing env red (not mine):** 5 lifecycle_state_test boot tests fail with "unable to open database file: ~/.kelir/catalog.db" when `~/.kelir` doesn't exist — `CatalogDb::open` (kelir-catalog, Agent E lane) never mkdirs the parent. Green once the dir exists. Recommend a catalog-side fix.
3. **A7 note vs my scope:** A recorded that `kelir-core/src/bus/mod.rs register_open_stream` rejects plugin-registered kinds and called it "C's lane" — bus/mod.rs is outside my authorized file list; left untouched, flagged for explicit tasking.
4. **Artifacts out of sync (nobody's write scope in WO3):** `cans/artifacts/config/project-structure.txt` lacks `kelir-transport/src/host_bind.rs`; `case-to-test-map.yaml` still records wp_16 as `status: red` and wp_14's note as "no real bind" (both now pass/real). Convergence to update.
5. **Explicit DNS denylist field deferred** (C3 decision) — needs profile/network-policy schema + SDK literal updates (Agent B + bindings owner).
6. **A8 landed one mechanical line (`offset: 0`) in fuse_storage.rs** (my exclusive file) via merge — compatible with C7, no conflict.
7. **Shared CARGO_TARGET_DIR poisoning** can produce phantom E0599/E0609 for any agent after cross-worktree merges — touch the touched crate's sources or `cargo clean -p <crate>`; documented in conflicts.md.
