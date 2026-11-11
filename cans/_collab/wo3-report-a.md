# WO3-A report — kelir-interface contract alignment (Agent A, 2026-11-11)

Branch `temp/wo3-a`, base 339ca53. All tasks A1–A9 done plus the lane clippy duty. Phase L1/L2 respected: no `TODO (L3)` bodies filled, no new `TODO (L3)` markers.

## Per-task outcomes

- **A1 set_env — done. Kept `Vec<(String, String)>`.** Every executable pin (policy-matrix.rs, autonomous-swe-agent.rs, py `to_wire`, vectors.json golden `set_env` bytes) says ordered pairs and the dual-form deserializer was already deleted in a prior order; a HashMap flip would break the golden wire bytes. Spec text moved with the code: `cans/plugins/tool-guard.md` line now reads `set_env: Vec<(String, String)> (ordered injected environment pairs)`. The node's leaf anchor text changed → `crates/kelir-testkit/anchors.json` needs an E1 regeneration after merge (no code `see:` targeted the old anchor).
- **A2 verdict helpers — done.** `HandlerCtx<Plugins, Params>` now has `pass()`, `patch(PatchPayload)`, `drop(Option<String>, String)`, `suspend(SuspendUntil)`, `abort(String)` returning `Verdict`, per CANS/03 `#Unified-verdict-surface`. Clippy accepts the `drop` method name (no should_implement_trait / unused_self hits).
- **A3 filter helpers — done.** `forward()`, `filter_drop(Option<String>, String)`, `filter_suspend(SuspendUntil)` returning `FilterAction` per CANS/11 `#Verdict-method-mapping`. **Cross-SDK parity gap (outside my lane, recorded in conflicts.md):** TS exposes only `FilterAction.Forward/.Drop/.Suspend/.Abort` statics (verdict.protocol.ts) and Py only FilterAction classmethods (`packages/py/kelir/__init__.py:276-289`); neither SDK exposes ctx-level `forward()/filter_drop()/filter_suspend()`.
- **A4 MountScope net fields — done EARLY, pushed immediately after A1 so Agent C could consume it.** `net_cidrs: Option<Vec<String>>` (CIDR notation) + `net_ports: Option<Vec<u16>>`; `allows_net(ip: [u8; 4], port: u16) -> bool` with a fail-closed CIDR matcher (parse failure / bad prefix → deny) whose signature matches `Ipv4View::dst()/src()` + `TcpView` port extraction for C6; `with_net_cidrs`/`with_net_ports` builders; `intersect` extended and the four copy-pasted match arms deduped into `intersect_list` (DRY; keeps plugin_model.rs at 995 LOC, under the 1000 cap). Enforcement dispatch left to Agent C as ordered.
- **A5 WatchdogAction::Terminate — decision: KEEP `Terminate { emit_diagnostic: bool }`, deviation recorded (not silent).** Verified live: artifact `cans/artifacts/code/vmm-driver-trait.rs` pins the field; the vectors.json golden vector `watchdog-terminate-mitigate` embeds `{"Terminate":{"emit_diagnostic":true}}` in its CBOR payload bytes; three out-of-scope sites construct the payload form (parity_cli.rs:96 parser, vectors_testkit.rs:298, driver-rustvmm/tests/virtio_shm_wdt_test.rs:67). A fieldless change breaks other lanes and golden parity bytes; CANS/08's "Terminate synthesizes WatchdogDiagnostic" reads unconditional → node-vs-artifact conflict filed unresolved in conflicts.md (TBD spec revision direction).
- **A6 FLAG_CONTINUATION — done as ordered.** Spec-vs-spec conflict (mcp.md#MCP-contract references it; CANS/11 flag list and wire-protocol.rs artifact stop at 0x10) recorded. Added `FLAG_CONTINUATION_RESERVED: u8 = 0x20` in codec_protocol.rs with `// see:` pointers at both nodes; no mux semantics invented; bit not projected to generated TS/PY/C (codegen imports only the five canonical flags).
- **A7 StreamKind plugin kinds — decision: no `Plugin(u16)` variant; plugin kinds stay raw u16.** CANS/11 says "Kinds are numbers; no per-kind spec edits required"; `register_stream_kind` already takes raw u16 in Rust/TS/Py; a variant would change serde shape and the closed parity tables. No spec change. **Flagged for Agent C:** `kelir-core/src/bus/mod.rs` `register_open_stream` currently rejects every plugin kind with EUnknownKind because `StreamKind::from_u16` returns None — wiring the u16 registry into the bus is C's lane.
- **A8 on_fs_read offset — done via the field option.** `FsContext.offset: u64` added (L1 surface) with a `// TODO (L2):` plumbing note; `on_fs_read` signature intentionally unchanged per the work order. Both FsContext (handlers_plugin.rs) and FsInterposer (plugin_model.rs) live in my files, so nothing was out-of-lane except the one mechanical literal. Observation recorded for the reviewer: CANS/05 names FsContext fields "(inode, path, locks, journal)" — a `locks` field exists in no FsContext; untouched, not tasked.
- **A9 SnapshotDescriptor — done.** `// see: cans/09-snapshots.md#Archives-and-portability` added above the type in driver_interface.rs. Fields verified against CANS/09 + the vmm-driver-trait.rs artifact: `dirty_bytes` is a byte count ✓, `parent_digest: Option<String>` ✓ (base snapshot carries None), `id`/`created_at_ns`/`guest_clock` ✓. No code change needed.

## Lane duty: clippy large_enum_variant — fixed

Baseline was actually GREEN at 339ca53 because a prior commit had parked `#[allow(clippy::large_enum_variant)]` on `GuestToHost` (the orchestrator's "known-RED" claim was stale). Treated the allow as the band-aid it is: `GuestToHost::TrapEvent(Box<TrapEvent>)`, allow deleted. `Box` is serde-transparent → wire bytes and the vectors.json golden set are unchanged (cargo test -p kelir-interface passes, including the hex fixtures). Artifact `messages.rs#GuestToHost` still pins the unboxed form — divergence recorded in conflicts.md alongside the prior VmConfig-deletions precedent.

## Gates (all run, all green)

- `cargo check -p kelir-interface` — PASS
- `cargo clippy -p kelir-interface --all-targets -- -D warnings` — PASS (no allow attributes added anywhere)
- `cargo fmt -p kelir-interface` — run; one import re-wrap committed; tree clean after
- Comment law: every comment I added is `// see:` / `// TODO (L2):`; pre-existing prose at `crates/kelir-plugin-testkit/src/vectors_testkit.rs:603-607` predates 339ca53 (verified via `git show`) — left for D's D1 sweep, not my lane
- Extra sanity (not required): `cargo check -p kelir-core -p kelir -p kelir-plugin-cli -p kelir-plugin-testkit -p plugin-tool-guard -p plugin-mcp` all PASS; `cargo test -p kelir-interface` 8/8 PASS
- Codegen: `cargo run -p kelir-interface --bin codegen` ran ("13 plugins ok") and modified NOTHING — no codegen commit needed; projections are byte-stable after my changes

## Files changed (339ca53..HEAD)

```
cans/_collab/conflicts.md                          | 12 +  (new WO3-A section, append-only)
cans/_collab/wo3-report-a.md                       | (this file)
cans/plugins/tool-guard.md                         |  2 +-  (A1 spec note)
crates/kelir-core/src/planes/storage/fuse_storage.rs |  1 +  (A8 mechanical offset:0 — Agent C's lane, compilation-required)
crates/kelir-interface/src/codec_protocol.rs       |  3 +   (A6)
crates/kelir-interface/src/driver_interface.rs     |  1 +   (A9)
crates/kelir-interface/src/handlers_plugin.rs      | 40 +-  (A2/A3/A8)
crates/kelir-interface/src/plugin_model.rs         | 105 +- (A4 + intersect dedup)
crates/kelir-interface/src/protocol_wire.rs        |  3 +-  (clippy fix)
crates/kelir-plugin-testkit/src/vectors_testkit.rs | 12 +-  (5 mechanical Box::new sites — compilation-required by the clippy fix)
```

Out-of-scope mechanical follow-through (each compilation-required, each recorded in conflicts.md): vectors_testkit.rs lines 354/383/407/470/820 (Box::new), fuse_storage.rs FsContext literal (offset: 0).

## TODO (L3) count

Before: 519 (454 crates/ + 65 packages/). After: 519. Delta 0 — no TODO (L3) added or removed; one new TODO (L2) (A8 plumbing note).

## For the orchestrator / other lanes

1. **Agent C:** A4 surface is pushed (commit 3803676) — `MountScope::allows_net(ip: [u8;4], port: u16)`, `net_cidrs`/`net_ports`, `with_net_*` builders. Also: bus EUnknownKind gap for plugin kinds (A7 note), and my one-line `offset: 0` in fuse_storage.rs may textually collide with C7's neighboring edit — trivial merge.
2. **Agent E:** regenerate anchors.json after merge — A1 changed a tool-guard.md leaf anchor ("set-env:-HashMap..." → Vec form).
3. **Reviewer-owned, untouched:** SuspendReason::External(ExternalSuspendKind) vs CANS/08 External(u32) (no WO3-A task touched it); CANS/05 "locks" field naming; artifact amendments pending for messages.rs#GuestToHost (boxed) and plugin-trait.rs#MountScope (net fields).
4. **Open conflicts filed:** A5 (Terminate emit_diagnostic vs CANS/08), A6 (FLAG_CONTINUATION mcp.md vs 11-protocol.md) — both unresolved, both with reserved/recorded interim states.
