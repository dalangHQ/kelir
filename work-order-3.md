# work-order.md — L1/L2 spec-misalignment triage

**Phase:** L1/L2 only. Do not fill `TODO (L3)` bodies. Fix contracts, wiring, and dispatch plumbing.

**Rules for agents:** one worktree each, exclusive file scope, no pushes, squash-merge at end. Report decisions after the fact; never ask. Verify each claim against the live tree before editing.

---

## Agent A — `kelir-interface` contract alignment

**Scope (exclusive):** `crates/kelir-interface/src/{plugin_model.rs,protocol_wire.rs,codec_protocol.rs,handlers_plugin.rs,driver_interface.rs}`

**Goal:** make the L1 type surface match the CANS contract.

### Tasks
- [ ] **A1. `ExecveMutation.set_env` type.** CANS/plugins/tool-guard.md specifies `HashMap<String, String>`; code has `Vec<(String, String)>` in `protocol_wire.rs`. Choose one direction and record in `_collab/conflicts.md`:
  - `HashMap` matches spec text; `Vec` preserves wire order.
  - If keeping `Vec`, add a `// see:` note in the spec file (spec change) instead of silently diverging.
- [ ] **A2. `HandlerCtx` verdict helpers.** CANS/03 Rust surface lists `c.pass() / c.patch(m) / c.drop(e,r) / c.suspend(u) / c.abort(r)`. Add these on `HandlerCtx<Plugins, Params>` (return `Verdict`; patch takes `PatchPayload`). TS and Python already have them; Rust does not.
- [ ] **A3. `FilterAction` helpers.** CANS/11 says `c.forward() / c.filter_drop(errno, reason) / c.filter_suspend(until)`. Add to `HandlerCtx` for fs-write/net rings. TS currently exposes only `FilterAction.Forward / .Drop / .Suspend` statics.
- [ ] **A4. `MountScope` network fields.** CANS/02 Mount scoping names a “Network filter: ingress dispatcher bypasses net hooks outside declared CIDR/port rules.” Current `MountScope` has `devices, binaries, paths, stream_kinds` only. Add `net_cidrs: Option<Vec<...>>` and `net_ports: Option<Vec<...>>`. Wire `allows_net` helper. Leave enforcement (L2 dispatch check) to Agent C.
- [ ] **A5. `WatchdogAction::Terminate { emit_diagnostic: bool }`.** CANS/08 says `WatchdogAction::Terminate` always synthesizes `WatchdogDiagnostic`. Change to `Terminate` (no payload) or document the deviation in `_collab/conflicts.md`. Do not silently keep `false` path.
- [ ] **A6. `FLAG_CONTINUATION`.** CANS/plugins/mcp.md references `FLAG_CONTINUATION` for >64KB MCP messages. Not present in `codec_protocol.rs` nor in CANS/11’s mux flag list. **Spec-vs-spec conflict.** Add to `_collab/conflicts.md`; do not invent a new mux flag bit until resolved. Meanwhile add a compile-time constant `FLAG_CONTINUATION_RESERVED: u8 = 0x20;` guarded by a `// see:` pointing at the open conflict, so the bit is reserved.
- [ ] **A7. `StreamKind` plugin-registered variants.** CANS/11 says kinds are numbers; `register_stream_kind` accepts any `u16`. `StreamKind::from_u16` returns `None` for plugin-registered kinds (0x0013..0x7FFF). Either add `StreamKind::Plugin(u16)` or document that MountScope/descriptor uses raw `u16` for plugin kinds. Pick one, record in conflicts if it changes spec.
- [ ] **A8. `on_fs_read` offset gap.** CANS/05 says read masking operates “across offsets” but `FsContext` carries no offset and `FsInterposer::on_fs_read` has no offset argument. Add `offset: u64` to `FsContext`, or note the L3 deferral with a `// TODO (L3):` (do not add offset param now).

### Verify
- [ ] `cargo check -p kelir-interface` passes.
- [ ] `cargo clippy -p kelir-interface -- -D warnings` passes.
- [ ] No new prose comments; every comment is `// see:`, `// TODO…:`, or `// SAFETY:`.

---

## Agent B — `kelir` (SDK facade) session surface

**Scope (exclusive):** `crates/kelir/src/{session_lifecycle.rs,sandbox_runtime.rs,builder_sdk.rs,builder_scopes.rs,builder_plugin.rs}`

**Goal:** align the Rust SDK surface with CANS/03 and CANS/08.

### Tasks
- [ ] **B1. `Session::exec(cmd, limits?)`.** CANS/03 calls the programmatic exec surface `session.exec(cmd, limits?)`. Rust currently exposes only `Session::spawn(binary, argv)`. Add `exec(binary: &str, argv: &[&str], limits: Option<ProcessLimits>) -> SpawnBuilder`; keep `spawn` as a thin alias for backward-compat within this repo (no external API frozen yet). Add `limits: Option<ProcessLimits>` to `SpawnBuilder` and serialize it on the EXEC control sub-lane 0x03 per CANS/11 (stub the encode; leave wire bytes for L3).
- [ ] **B2. `Session::skew_clock`.** CANS/08 lists `skew_clock` on `Session` (not only `session.vm().clock.skew`). Add `Session::skew_clock(&self, delta: Duration) -> Result<(), PluginError>` delegating to `self.vm().clock.skew(delta)`. Check `session_lifecycle.rs`, `sandbox_runtime.rs`, and the pyo3/napi bridges for a matching surface.
- [ ] **B3. `Session::checkpoint.create(label)` on Rust.** CANS/03 lists `session.checkpoint.create(label)`. Current Rust `Session` exposes `checkpoint` only via `Deref` to `kelir_core::Session.checkpoint: Arc<CheckpointPlane>` which has `create_step(lineage, base)`, not `create(label)`. Also `StorageHandle`, `WireHandle`, `VsockHandle`, `StepHandle`, `CheckpointHandle` exist as unused structs (dead code) while pyo3/napi route through the real planes. **Pick one:**
  - Either expose a `Session::checkpoint_handle(&self) -> CheckpointHandle` that wraps `vm().checkpoint(label)` and deprecate the unused structs, or
  - Delete the unused `StorageHandle` / `WireHandle` / `VsockHandle` / `StepHandle` / `CheckpointHandle` structs (they shadow real planes; CANS/03 says planes ARE the surface).
  Record decision in `_collab/conflicts.md`.
- [ ] **B4. `Session::rollback()` signature.** CANS/08 shows `session.rollback()` (no args) from `on_process_exit`. Current code requires `PidLineage`. Add `Session::rollback_here()` that derives `PidLineage` from `self.core.lineage(ctx.pid)` when called from a trap context, or add a doc example showing `session.rollback(session.lineage(pid))`. Do not silently change `rollback(lineage)`.
- [ ] **B5. `KelirBuilder<Plugin, …>` trap scope coverage.** CANS/02 says 30 verbs / 8 scopes exist in plugin mode, but `builder_plugin.rs` only adds `trap(configure)` for `Plugin` mode — it does not add `fs/net/console/exec/host/lifecycle/telemetry` scope entries for `Plugin` mode (they exist generically in `builder_plugin.rs` but on `Mode` generic — verify they compile for `Mode = Plugin`). **Verify with a compile test**: `let b = KelirBuilder::named("x").fs(|s| s.on_fs_lookup(|_| Verdict::Pass));` compiles. If it doesn’t, extend the generic impl.
- [ ] **B6. `seal()` text for host.** CANS/02 says “Host failure text is the bare triple text; the host has no name.” Current `builder_sdk.rs` uses `self.name = "host"` and prefixes errors with `"host: …"`. Change to bare text when `Mode = Host` — or set `self.name = ""` for host and skip the prefix. Small change; keep plugin-mode prefixed.
- [ ] **B7. `Satisfies` order-insensitive.** `hlist_sdk.rs` is currently order-sensitive; CANS/02 lists “order-insensitive subset check” in the contract. This is currently marked `// TODO (L3)`. Leave as-is; add a `// TODO (L2):` if you can implement the recursive subset impl in L2. Do not fill without a decision — note in `_collab/conflicts.md`.

### Verify
- [ ] `cargo check -p kelir --all-targets` passes.
- [ ] Existing Rust example snippets in `cans/artifacts/examples/` (if referenced) still type-check against the new surface — at minimum, spot-check `crates/kelir/src/lib.rs` re-exports.
- [ ] `cargo clippy -p kelir --all-targets -- -D warnings` passes.

---

## Agent C — `kelir-core` wire/storage L2 gaps

**Scope (exclusive):** `crates/kelir-core/src/planes/wire/mod.rs`, `crates/kelir-core/src/planes/storage/mod.rs`, `crates/kelir-core/src/planes/storage/fuse_storage.rs`, `crates/kelir-core/src/session_lifecycle.rs`, `crates/kelir-core/src/dispatch_plugin/*.rs`

**Goal:** close L2 dispatch gaps without filling L3 bodies.

### Tasks
- [ ] **C1. `process_outbound_frame` honors `allow_udp_proxied`.** CANS/07 says “Permitted UDP egress uses endpoint-independent mapping (Full Cone NAT).” Current code drops all `FrameClass::UdpOther` unconditionally. Wire the `WireWiring.allow_udp_proxied` flag into the outbound classifier: if set, forward UDP (the actual NAT is L3); else drop. Add matching `// see:` anchor at CANS/07 UDP-NAT node.
- [ ] **C2. `WirePlane::forward_port` kernel bind.** CANS/07: “host_port = None or 0 binds ephemeral port 0; host kernel allocates free port.” Current impl picks a port from `EPHEMERAL_PORT_MIN..=u16::MAX` and records it in a HashMap without calling `bind(2)`. This is silent-failure wiring — a subsequent OS bind can collide. Add a `bind()` call to the enclave (`kelir-transport` or `driver-*`) with `SO_REUSEADDR | SO_REUSEPORT` (as spec mandates) before inserting into the map; return `Err(WireError::PortOccupied)` only on actual `EADDRINUSE`. Keep the HashMap as the logical registry. Add doc-comment `// see: cans/07-wire-plane.md#Port-forwarding`.
- [ ] **C3. `resolve_synthetic_dns` NXDOMAIN default.** CANS/07 says blocked queries return `NXDOMAIN`; allowed queries forward. Current stub returns `ServFail` unconditionally. Change the default to `ServFail` only when the upstream relay is not yet wired (a `// TODO (L3):`) and return `NxDomain` when the query name is on the sinkhole denylist. Add the denylist lookup plumbing (allowlist already exists; add the inverse check).
- [ ] **C4. EDNS0 downgrade plumbing.** CANS/07 says the synthetic gateway strips the DO bit before upstream and clears AD on the response. Add the `downgrade_edns0` path in the response synthesis (currently a TODO in `DnsResponse`). Keep the wire parsing as `// TODO (L3):`.
- [ ] **C5. `dispatch_fs_write` scope → `MountScope`**. Agent A is adding `MountScope.net_cidrs/net_ports`; no action needed here unless C1 needs it.
- [ ] **C6. `dispatch_net_outbound` / `dispatch_net_inbound` scope**. CANS/02 says “Network filter: ingress dispatcher bypasses net hooks outside declared CIDR/port rules.” Current code only checks `scope.allows_device(VirtioDevice::Net)`; TODO comment already says “CIDR/port scope rules need a frame parser; only the device mask is enforced here.” This is L2 — the frame parser exists in `frame_layout_views.rs` (Agent C can wire `Ipv4View` / `TcpView` to check CIDR/port). Do it: parse the L2/L3 header, extract dst/src IP:port, check `scope.allows_net(...)` (use Agent A’s fields). Do not implement the actual traffic shaping.
- [ ] **C7. `FuseReply::Written(size)`.** CANS/05 says FUSE WRITE reply size is `fuse_write_out.size` (requested chunk size), never the payload bytes. In `dispatch` `FilterAction::Forward` branch currently returns `FuseReply::Written(request.data.len() as u64)`. Use `request.size` instead. (The `Drop { errno: None }` branch already uses `request.size`.)
- [ ] **C8. `Session::boot` double registration.** `boot_with_engine` calls `CatalogDb::open(...).register_session(&row)` unconditionally. If the same `session_id` boots twice (restore path), this fails with a UNIQUE constraint. Add `INSERT OR IGNORE` or a pre-check `lookup_session` first. L2 wiring. Flag as a bug.
- [ ] **C9. `CheckpointPlane::commit_step` and `restore`.** Per CANS/08, `commit_step` should “flush staging delta, advance active step slot”. Currently just inserts into graph. This is L3 — leave the `// TODO (L3):` and skip.
- [ ] **C10. `SnapStore`/`.kelir-snap` archive.** CANS/09 describes a `format version + feature flags + entry table` header with trailer for end-to-end integrity. `Session::export_snapshot` returns `Err(Unsupported("L3"))` — that’s L3. Leave; but verify the `SnapshotDescriptor` field names match CANS/09 (`dirty_bytes` as byte count not page count — already correct; `parent_digest: Option<String>` — already correct; add `// see: cans/09-snapshots.md#Archives-and-portability` above the type).

### Verify
- [ ] `cargo check -p kelir-core` passes.
- [ ] Unit tests: none required at L2, but if you add `C6` parser paths, add `#[cfg(test)]`-free `tests/wire_scope_test.rs` (per AGENTS.md: no `#[cfg(test)]` in `src/`).
- [ ] `cargo clippy -p kelir-core -- -D warnings` passes.
- [ ] Any new `// see:` anchors resolve against `crates/kelir-testkit/anchors.json` (Agent E regenerates).

---

## Agent D — Hygiene audit (read-only + mechanical fixes)

**Scope (exclusive):** mechanical — no design changes. Fix only clear-cut violations.

**Goal:** enforce AGENTS.md section 3 (comment law) and section 6 (hygiene gates).

### Tasks
- [ ] **D1. Comment law sweep.** Run the exact greps from AGENTS.md section 3. For each violation:
  - Delete the prose comment if it duplicates a `see:`/`TODO:`/`SAFETY:` neighbour.
  - Convert to `see:` if it documents an owning concept.
  - Convert to `TODO (Lx):` if it defers work.
  - Delete trailing `//` on the same line as code — merge into a preceding `see:` or `TODO:` on its own line, or drop it.
  - Delete `///` doc comments and Python docstrings; replace with one-line `see:` or `TODO:` as appropriate.
  - Delete `# NOTE:` lines in YAML manifests under `crates/kelir-testkit/*.yaml` — replace with `see:` or drop.
  Report count: before/after per language.
- [ ] **D2. File-size audit.** `find crates packages -name '*.rs' -o -name '*.ts' -o -name '*.py' | xargs wc -l | awk '$1 > 1000'`. Produce the list. For each: **do not refactor** (that breaks agent isolation) — record in `_collab/conflicts.md` as L2/L3 debt. Highest-probability offenders:
  - `crates/kelir-core/src/session_lifecycle.rs`
  - `crates/kelir/src/builder_sdk.rs`
  - `crates/kelir/src/session_lifecycle.rs`
  - `crates/kelir/src/builder_plugin.rs`
  - `crates/kelir-interface/src/plugin_model.rs`
  - `crates/kelir-bindings/napi/src/lib.rs`
  - `crates/kelir-bindings/pyo3/src/lib.rs`
  - `crates/kelir-shim/src/main.rs`
  - `packages/py/kelir/__init__.py`
  - `packages/ts/core/src/builder.lifecycle.ts`
- [ ] **D3. `#[cfg(test)]` in `src/`.** Grep `crates/**/src/**/*.rs` for `#[cfg(test)]`. Must return zero. Fix any hits by moving to `tests/`.
- [ ] **D4. Banned-API audit.** Grep for `unwrap()`, `expect(`, `panic!`, `todo!()`, `unimplemented!()`, `unreachable!()` in non-enclave crates (anywhere outside `kelir-drivers`, `kelir-transport`, `kelir-shim`). Report; do not edit yet unless the fix is a trivial `?` and behaviour-preserving.
- [ ] **D5. `#![forbid(unsafe_code)]` presence.** Every `.rs` library file at crate root must have it except the three enclaves. Grep; report missing.
- [ ] **D6. Missing `// see:` headers.** Certain `crates/kelir-testkit/src/bin/*.rs` files (`scorecard_testkit.rs`, `check_tests.rs`) start with no `see:` header. Add a single top-of-file `// see:` pointing to their owning CANS node (`cans/15-operations.md#Coverage-gates` for both, or the specific node).
- [ ] **D7. `TODO (L3):` growth.** Count current `TODO (L3):` markers across the tree. Report the number; do not delete.

### Verify
- [ ] All four AGENTS.md section 3 greps return empty.
- [ ] `just check-hygiene` (or the equivalent) passes.
- [ ] Deliverable: `_collab/hygiene-report.md` with counts and file lists.

---

## Agent E — Spec anchors + testkit + misc L1/L2 mismatches

**Scope (exclusive):** `crates/kelir-testkit/src/**`, `crates/kelir-plugin-cli/src/**`, `crates/kelir-plugins/plugin-{chaos,container,db-proxy,ledger,mcp,mesh,profile,replay,supply-chain,tool-guard,transaction,vault,wire-ingress}/src/**` (plugin-specific trait impls only), `crates/kelir-adapters/**`, `crates/kelir-catalog/src/**`

**Goal:** make sure spec anchors resolve, testkit vector counts match spec, and no plugin crate violates the tier contract.

### Tasks
- [ ] **E1. Regenerate `anchors.json`.** Run `cargo run -p kelir-testkit --bin anchors`. Verify it matches the spec’s current `cans/*.md` set. If the file is committed stale, fix by regenerating. If `check-manifest` was failing, this is likely the cause.
- [ ] **E2. `crates/kelir-testkit/src/vectors_testkit.rs` count.** Verify the suite produces 58 vectors = 46 builder cases (30 verb + 8 scope + 8 merge) + 12 others. Current `verb_cases` returns 22 ring + 6 structural = 28; base has 2 builder cases (`trap`, `fs_write`); = 30. `nested_cases` = 8; `merge_cases` = 8. Non-builder = 12 (6 traps, 1 heartbeat, 2 watchdog, 3 veto, 1 exec_rewrite = 13? count exactly). If it’s 13, one case is duplicated or misplaced. Fix by audit against CANS/15 coverage spec.
- [ ] **E3. `crates/kelir-plugin-cli/src/parity_cli.rs` canonical roster check.** `assert_canonical_roster` requires declared order to exactly match `kelir-plugin-testkit::fixtures_testkit::canonical_vector_names()`. That function currently calls `VectorSuite::fixtures()` — which now includes builder cases, so order is stable. Verify.
- [ ] **E4. Plugin tier violations.** Re-verify each `crates/kelir-plugins/*/Cargo.toml` against CANS/02:
  - `plugin-mesh` (Tier 1): only `kelir-interface` + `async-trait`. ✓
  - `plugin-mcp` (Tier 1): only `kelir-interface` + `async-trait`. ✓
  - `plugin-supply-chain` (Tier 2): `kelir-interface` + `async-trait` — **no Tier 1 peer import.** Is that legal? CANS/02 says “Tier 2 plugins depend on kelir-interface and **consume** Tier 1 services via ServiceRegistry.” A Tier 2 plugin that consumes no Tier 1 services has zero peer imports and is legal per “dependency invariant: Zero concrete peer imports within the same tier” (downward-only). Verify no violation.
  - `plugin-container` (Tier 2): same shape. ✓
  - `plugin-tool-guard` (Tier 2): imports `plugin-mcp` (Tier 1). ✓
  - `plugin-ledger` (Tier 3): imports `kelir-otlp` (not a plugin) — legal per “external sink delegates persistence.” ✓
  - `plugin-transaction` (Tier 3): imports `plugin-db-proxy` (Tier 2). ✓
  - Run `kelir-plugin-cli check --graph` to validate acyclic and downward-only.
- [ ] **E5. `plugin_supply_chain::WxAllocToken::WIRE_LEN`.** CANS/plugins/supply-chain.md says the token is delivered over vsock to the guest. Current `WIRE_LEN = 16 + 8 + 32` — is that the spec’s token layout? The spec says “Verified JIT runtimes receive dynamic one-time MMU allocation token over vsock.” No wire layout pinned in spec. Leave the size; add `// TODO (L3):` for cross-impl sync if a canonical layout is added.
- [ ] **E6. `plugin_vault::MetadataEndpoint::serve`.** CANS/plugins/vault.md says token response body is `{ token, ttl_seconds, scope, audience }`. Code matches. ✓ `metadata_vsock_port` default is `VAULT_METADATA_VSOCK_PORT = 80` per CANS. ✓ Leave.
- [ ] **E7. `plugin_tool_guard::decode_tools_list`.** Currently returns `Err(Unsupported("L3"))`. That means at boot, `ToolGuardPlugin::on_vm_ready` will fail. This makes `plugin-tool-guard` unable to boot even in L2. Check whether this is intentional (L2 gap) or a bug. If intentional, add `// TODO (L3):` on the call site. If not, at least return an empty `ToolsListResult` so the plugin boots without pinning.
- [ ] **E8. `plugin_profile` `apply_patch_to_base` Move/Copy leaves base stale.** CANS/plugins/profile.md requires RFC 6902 semantics. Current `PatchOp::Move | Copy | Test => {}` silently no-ops. This is a semantic bug even at L2 (patch dispatch succeeds but doesn’t mutate). Add `// TODO (L3):` on the site with a pointed `see:`.
- [ ] **E9. `plugin_db_proxy` `intercept_ethernet_frame` FIN handling.** CANS/07 says “Rollback interaction: session.rollback() closes every host-side socket.” FIN/RST tracking is L3. Leave, but verify `// TODO (L3): FIN half-close state machine` is present.
- [ ] **E10. `crates/kelir-adapters/mcp-server/src/main.rs`.** Empty `main` with `// TODO (L3): direct the stdio wire frames to plugin-mcp`. This is a binary that should compile and run as a no-op. Verify it builds (it does).

### Verify
- [ ] `cargo run -p kelir-testkit --bin anchors` produces deterministic output.
- [ ] `cargo test -p kelir-plugin-testkit` passes (no runtime panics).
- [ ] `kelir-plugin-cli check --graph --layers` passes.
- [ ] Deliverable: `_collab/misc-report.md` with the audit.

---

## Cross-agent conflicts to resolve first

- **A1 vs `_collab/conflicts.md`:** `ExecveMutation.set_env` type — pick HashMap (spec) and either accept nondeterministic serialization for `Verdict::Patch` receipts, or update spec.
- **A6 vs CANS/11:** `FLAG_CONTINUATION` — spec-internal conflict; file in `_collab/conflicts.md`, do not invent a flag bit.
- **B3 vs B1:** unused `StorageHandle`/`WireHandle`/`CheckpointHandle` dead code — decide keep-or-delete as part of `Session` refactor; if deleted, ensure pyo3/napi bridges’ access to `.storage`/`.wire` on the native handle still works.
- **D2:** files >1000 LOC — do not split at L2; record debt. If the user wants splitting, that’s a separate work order.

---

## Definition of done

- [ ] All 5 agents report back with `_collab/*-report.md`.
- [ ] All cross-agent conflicts resolved in `_collab/conflicts.md` with a decision line each.
- [ ] `cargo check --workspace --all-targets` green.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` green.
- [ ] `cargo test --workspace` green (unit tests only; integration tier gated on KVM/HVF).
- [ ] `just check-hygiene`, `just check-spec`, `just check-manifest`, `just check-tests`, `just check-scorecard` all green.
- [ ] No new `TODO (L3):` introduced; existing count unchanged or decreasing (per AGENTS.md D7).