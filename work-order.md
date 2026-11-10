# Work order — verified against live tree + cans/ SSOT

- **Base:** `scaffolding` @ `989701f` ("Update work-order.md"), 2026-11-09.
- **Verification:** every premise below was checked against the live tree at that
  commit and the `cans/` + `cans/artifacts/` SSOT, in three parallel passes
  (§§1–3, §§4–6, §§7–10); the three refutations that change scope (§3c, §6b, §6c)
  were re-checked by hand from `git show HEAD:` before this rewrite.
- **Verdicts:** CONFIRMED / PARTIAL / REFUTED, per sub-claim. A PARTIAL keeps the
  substance with corrected wording; a REFUTED claim is struck and replaced by
  the real finding underneath it — it is not deleted silently.
- **Paths are post-rename paths.** A domain-first Rust rename wave has since landed as
  `353e0b5` (e.g. `session.rs` → `session_lifecycle.rs`, `codec.rs` →
  `codec_protocol.rs`); the paths below are the `353e0b5` spellings, and
  the content was verified identical across the renames. The rename pass over
  this file is done.
- **Phase is L1/L2 only (AGENTS.md §2).** Items tagged L3 are deferred by design,
  not accidental rot. Separate them from wrong-shape-now items: a `TODO (L3)`
  stub is a scheduled gap; a duplicated constant, a dead export, or a
  mislocated erasure is a shape error to fix now.
- **Where this order contradicts `cans/` + `cans/artifacts/`, the artifacts win**
  (AGENTS.md §1) and the decision is recorded, not silently resolved.

## 1. Blatant code & logic duplication — CONFIRMED in substance, 3 of 5 sub-claims corrected

### 1a. Wire layout views: shared skeleton, not clones — PARTIAL
- Both files exist: `crates/kelir-core/src/planes/wire/layout_wire.rs` (119 lines) vs
  `crates/kelir-plugins/plugin-db-proxy/src/reassembler_wire/layout_wire.rs` (176 lines).
  Shared: `ETH_HEADER_BYTES = 14`, `ETHERTYPE_IPV4`, `IP_PROTO_TCP`,
  `IPV4_MIN_HEADER_BYTES` / `TCP_MIN_HEADER_BYTES` = 20, `EthView` / `Ipv4View` /
  `TcpView` with `new_unchecked`; `EthView::etype()` is byte-identical.
- Not near-verbatim clones: diff is 125 lines. Db-proxy adds `PgHeaderView`, TCP
  flags/SEQ/FLAGS, fragment masking, `SEQ_HALF_WINDOW`, derives min sizes from
  words; core has `UDP_HEADER_BYTES`, `L4_PORT_PAIR_BYTES`, and `*_checked()`
  accessors where db-proxy uses unchecked indexing throughout.
- "Unsafe zero-copy views" is wrong: `grep unsafe` returns nothing in either
  file; `new_unchecked` is a misnomer for bounds-checked indexing.
- No `cans/` anchor mandates a single layout module. Repo rule only: AGENTS.md §4.2 (DRY).
- **Fix (L2):** extract one shared Eth/IPv4/TCP view module into
  `kelir-interface`, keep `PgHeaderView` in db-proxy, delete the duplicated
  skeleton. Decide the canonical home first; do not invent a third copy.

### 1b. Driver stubs: 25 methods, not ~20; Mock is not a stub — PARTIAL
- Trait `crates/kelir-interface/src/driver_interface.rs:415-458` has **25 methods, zero with
  default bodies** — the "no default impls" half is confirmed.
- `VmmError::Unsupported` per `lib.rs`: avf 23, bwrap 25, hvf 26, openvmm 23,
  proot 25, rustvmm 25. "~20" understates. Not all identical: `boot` /
  `capabilities` are real-ish (e.g. avf `boot` does `check_hw_lockfile` +
  `open_node`), stub bodies differ in backend-name string and TODO wording.
- `MockVmmDriver` is misdescribed in the original order:
  `crates/kelir-plugin-testkit/src/lib.rs:64+` implements all 25 methods as
  `Ok(...)` zero-values, **zero** `Unsupported` returns. It duplicates the
  method list, not the stub pattern.
- **Fix (L2):** add default trait impls returning `Unsupported` in
  `kelir-interface`, delete the per-driver stubs except real overrides.
  Mock stays as the Ok-zero test double.

### 1c. Hex reinvented 9 times — CONFIRMED, one double-count
- All 9 sites exist with the named functions: codegen `otlp_codegen.rs:273`,
  `kelir-otlp/src/records_otlp.rs:5`, ledger `otel_ledger.rs:16,26,36`, vault
  `service_vault.rs:271`, cli `attach_cli.rs:6,15`, cli `fixtures_cli.rs:142`, cli
  `parity_cli.rs:41,58`, testkit `vectors_testkit.rs:293`, core `session_lifecycle.rs:32`
  (`incarnation_hex`). Three encode styles: lookup-table vs
  `format!("{byte:02x}")`; decode via `hex_digit` / `hex_val` / `to_digit(16)`.
- Correction: `otlp_codegen.rs:273` is the **codegen template string that emits**
  `records_otlp.rs:5` (`main.rs:150` writes it) — generator + output, not two
  independent reinventions. So **8 independent implementations + 1 template**.
  No `hex` crate in root `Cargo.toml`, no shared hex util in `kelir-interface`.
- **Fix (L2):** one hex util in `kelir-interface`; delete the 8, make the
  template emit a call to it.

### 1d. Session relay path: 3 implementations, not 4 — PARTIAL
- Independent: core `crates/kelir-core/src/session_lifecycle.rs:68-85`
  (`relay_socket_path`, HOME env + hex loop + `PathBuf` join); TS
  `packages/ts/core/src/session.lifecycle.ts:32-43` (`relaySocketPath`,
  `/^[0-9a-fA-F]{32}$/` regex); Py `packages/py/kelir/__init__.py:486-495`
  (charset check, `expanduser("~")`, `Path` join).
- **Refuted as a 4th implementation:** `crates/kelir-plugin-cli/src/attach_cli.rs:60`
  calls `kelir_core::Session::relay_socket_path(&session_id)` — it only adds
  its own `parse_session_id` / `hex_val` (lines 6–27) and a catalog lookup.
  Regex validation exists only in TS; core takes `[u8; 16]` with no validation.
- **Fix (L2):** one canonical `relay_socket_path` in `kelir-core`; TS/Py go
  through it (FFI) or share the same rule (join + 32-hex validation).

### 1e. Ad-hoc `block_on` / `ThreadWaker`: 6 copies, not 2 — CONFIRMED, understated
- `crates/kelir-bindings/pyo3/src/lib.rs:53-82` and
  `crates/kelir-plugins/plugin-vault/src/lib.rs:321-350` are byte-identical
  except the TODO wording. Plus 4 test copies:
  `plugin-supply-chain/tests/jit_wx_governance_test.rs:15`,
  `plugin-tool-guard/tests/interposer_tool_guard_test.rs:10`,
  `plugin-transaction/tests/twopc_transaction_test.rs:13`,
  `plugin-vault/tests/token_metadata_vault_test.rs`.
- **Fix (L2):** one shared `block_on` in `kelir-interface` (testkit for the
  test copies); delete all 6.

## 2. The "facade illusion" (builder config black holes) — CONFIRMED in substance; 2a/2d wording corrected

### 2a. Rust builder: retained-but-never-read, not dropped — PARTIAL (wording)
- `crates/kelir/src/builder_sdk.rs:314-331` store the configs; `finish_boot()`
  (522–562) forwards only `self.config`, plugin name/enabled
  `PluginRegistration`s, engine, metrics, and `synthetic_files` (545–550, the
  sole `storage_cfg` extraction). `terminal_cfg` / `wire_cfg` /
  `lock_broker_cfg` appear only at set / narrow / `Default` sites — never read.
- Correction: they are **retained-but-never-read, not dropped** —
  `Registration` is carried into `Session::from_parts`
  (`crates/kelir/src/session_lifecycle.rs:27,32-43`), and `Registration.traps` *is*
  consumed by `Session::dispatch_trap` (session_lifecycle.rs:65). Only the three plane
  cfgs are dead.
- No `cans/` anchor states builder configs must reach core;
  `cans/03-sdk-architecture.md:12-15` only describes the fluent-builder model.
- **Fix — spec decision first, then L2:** either forward the three cfgs into
  core (needs `Session` to accept plane cfgs) or delete the builder methods +
  `Registration` fields (deletion over addition, AGENTS.md §4.11). Decide
  explicitly; do not leave dead fluent methods.

### 2b. TypeScript builder no-ops — CONFIRMED
- `packages/ts/core/src/builder.lifecycle.ts:161-192`: `.terminal()`,
  `.storage()`, `.wire()`, `.params()` all `return this` with unused args,
  under explicit `TODO (L3)` at line 161. `boot()` (363–385) *does* forward
  vcpus / memoryMb / kernel / rootfsErofs / tscLocked / enableWatchdog +
  plugin registrations + `fuseHooks(descriptor.rings)` to native.
- **Fix:** same decision as 2a, applied per SDK.

### 2c. Python builder drops plane configs — CONFIRMED, stronger than stated
- `packages/py/kelir/__init__.py:633-645` records `terminal` / `storage` /
  `wire` kwargs in `_config` with `TODO (L3)`; `boot()` (807–822) forwards
  only vcpus / memory_mb / kernel / rootfs_erofs / tsc_locked /
  enable_watchdog to `PyKelirBuilder` — `_config["driver"]` / `["detached"]`
  are also never forwarded. `params()` (795–796) stores `_params_model`,
  which is never read anywhere (only lines 606, 795–796). And `boot()` raises
  `EConfig("kelir native PyO3 boot binding is not implemented (L3)")` (~824)
  — it never boots at all.
- **Fix:** same decision as 2a, applied per SDK; the Py boot path is L3-gated
  regardless.

### 2d. HList "erasure": wrong location, "useless" overstated — PARTIAL
- The erasure is **not in `finish_boot()`**: `Registration.instances` is
  already `Vec<(Box<dyn AnyPlugin>, MountScope)>` (builder_sdk.rs:197); boxing
  happens in `.plugin()` (295–310). `finish_boot()` only maps those to
  name/enabled `PluginRegistration`s (525–534).
- The HList is **not erased at boot**: `self.plugins` (the `HCons` chain) is
  passed into `Session<Plugins, Params>` (session_lifecycle.rs:21–43) and powers typed
  lookup `Session::ctx::<T>()` (session_lifecycle.rs:46–51 via `Has`) — exactly the model
  `cans/03-sdk-architecture.md:23-30` specifies ("Rust: `c.ctx::<T>()`
  infallible, statically resolved").
- Real disconnect: the *engine* only ever sees name/enabled registrations —
  the typed machinery serves the SDK session, not the microkernel.
- **Fix — spec decision, not deletion:** either thread typed contexts into the
  engine (design decision, L2/L3) or record explicitly that HList serves the
  SDK session only and the engine intentionally sees registrations. Do NOT
  delete HList as "useless".

## 3. Architectural incoherence & leaky abstractions — MIXED

### 3a. db-proxy "bypasses" WirePlane L4 — PARTIAL (specified-future vs present code, not a rogue plugin)
- Confirmed: db-proxy implements `NetInterposer` (lib.rs:243–253), parses
  Eth/IPv4/TCP itself (`reassembler_wire.rs:44-100` `parse_tcp_segment`), runs
  `TcpStreamReassembler` (reassembler_wire.rs:110–116, driven at lib.rs:148–165),
  and never calls `WirePlane` / `divert_l4` (zero hits in the crate).
- But "pulls raw Ethernet frames via NetInterposer" is the **designed contract**:
  `NetInterposer` (`crates/kelir-interface/src/plugin_model.rs:608-620`, artifact
  `cans/artifacts/code/plugin-trait.rs:382-388`) is "Sync L2 outbound/inbound
  buffer transform" over `&mut [u8]` frames.
- Spec side: `cans/07-wire-plane.md:107-123` ("Transparent L4 stream
  diversion") says the plane "Exposes raw duplex stream to registered
  NetInterposer proxies" and "Interposer receives reassembled stream bytes; it
  never terminates TLS", with L7 interposition only when the builder mounts a
  NetInterposer with an L7 scope — db-proxy declares none: its `default_scope`
  mounts `[Vsock, Net]` (lib.rs:231–234), and `MountScope` has no L7 field at
  all (`plugin_model.rs:96–100`), so no L7 scope exists to declare in code.
  `cans/plugins/db-proxy.md:7` says it "binds to native Wire Plane".
- And there is no working L4 abstraction to bypass:
  `WirePlane::divert_l4` (`planes/wire/mod.rs:486-490`) is a `TODO (L3)` stub
  returning `Err("L4 stream diversion over 0x0005 pending (L3)")`, and
  `process_inbound_frame` only classifies / forwards / drops.
- **Fix (L3, not a db-proxy fix):** implement `divert_l4` so the spec model
  becomes real, then move reassembly into the plane. Until then this is a
  specified-future gap, not a plugin going rogue.

### 3b. Trampoline generated, validated, then not wired — CONFIRMED, one correction
- TS `fusedDescriptor()` + `boot()` calls `fuseHooks(descriptor.rings)` and
  checks `trampolineEntryCount() == rings.length`
  (builder.lifecycle.ts:347, 363–385); Py `fuse()` / `describe()` +
  `native.fuse_hooks(descriptor)` (__init__.py:804, 807–822). NAPI `fuse_hooks`
  builds `TrampolineTable::from_descriptor` (lib.rs:640–655); `boot()`
  (658–710) verifies the count **and validates every fused hook's plugin is
  registered + enabled** (671–695) — so the table is *validated*, not "ignored
  completely" — then hits `TODO (L3)` (line 698) and calls
  `Session::boot_with_plugins(config, &plugins)` (705) with only name/enabled
  registrations. PyO3 mirrors this exactly (lib.rs:613–676). Core
  `boot_with_plugins` (`crates/kelir-core/src/session_lifecycle.rs:131-142`) takes only
  `(config, plugins)` and builds a fresh `PipelineEngine` — no
  trampoline/descriptor parameter exists anywhere in the chain.
- **Fix (L2 wiring + L3 fusion execution):** add a descriptor parameter through
  bindings → `boot_with_plugins` → `PipelineEngine`. Zero fused hooks reach
  dispatch today.

### 3c. AS-01 vs testkit mock exports — REFUTED as a self-contradiction; real finding underneath
- Exact rules in `crates/kelir-testkit/src/bin/check_tests.rs`: `lint_snippet`
  (638–647) fires on any YAML verification snippet containing `mockall` /
  `MockVmmDriver` / `MockHostInterposer` / `MockBusDispatcher`; `lint_file`
  (680–692) fires only on files whose path contains `/integration/` containing
  `mockall` / `MockVmmDriver` / `MockHostInterposer` — **`MockBusDispatcher`
  is absent from the file-level list**, an inconsistency between the two arms.
- Scope kills the contradiction: `main` (276–330) lints only
  `crates/**/{tests,benches}/**/*.rs` plus YAML cases —
  `kelir-plugin-testkit/src/` is never linted. And nothing uses the mocks:
  `MockVmmDriver` appears only in `testkit/src/lib.rs:64,67` + `check_tests.rs`;
  `MockBusDispatcher` only in `mock_bus.rs:13` + `check_tests.rs`; zero test
  files, zero YAML snippets. Zero test paths contain `/integration/`, so that
  arm is currently dead.
- **Real finding (replaces the claim):** the mocks are **exported but used
  nowhere** (dead public API), and the two AS-01 arms disagree on
  `MockBusDispatcher`.
- **Fix (L2):** either delete the mocks or use them in a unit-tier test, and
  align the two AS-01 arms (add `MockBusDispatcher` to `lint_file` or remove
  it from `lint_snippet` — decide, do not leave them disagreeing).

### 3d. Schizophrenic error handling — PARTIAL (one example wrong; spec anchor found)
- Confirmed forms: (1) POSIX ints only at the FUSE boundary — `drop_errno`
  (`crates/kelir-core/src/planes/storage/fuse_storage.rs:269-281`) maps string codes
  to `libc::EPERM` etc. (`i32`); (2) typed `VmmError`
  (`driver_interface.rs:14-23`: `Unsupported` / `InvalidState` / `Io` /
  `ResourceExhausted`, no errno variant) and `PluginError`
  (`plugin_model.rs:25-36`); (3) errno-as-string in `FilterAction::Drop { errno:
  Option<String> }` (`protocol_wire.rs:121,138`, e.g. `errno: Some("EPERM")` in
  tests/vectors); (4) catch-all `EConfig`: TS `errors.sdk.ts:3-8` (message-only),
  Py `sdk_errors.py:5` (bare `class EConfig(Exception): pass`).
- Wrong example: **no `PluginError::Halt("EPERM: ...")` literal exists in
  `src/`** (grep empty excluding tests/examples; closest is
  `examples/autonomous-swe-agent.rs:227`). Across FFI the conclusion holds:
  NAPI maps errors via `Error::from_reason(error.to_string())`
  (lib.rs:705–707), PyO3 via `PyRuntimeError::new_err(error.to_string())`
  (676–678) — stringified, unstructured.
- Spec anchor the original order omitted — it **strengthens** the claim:
  `cans/03-sdk-architecture.md:21` "All three produce the same structured
  error code: EConfig" and `:125-127` "Declarative error maps convert host
  exceptions to exact POSIX errnos" + "Structured error envelope carries
  driver, ring, pid, binary, and errno";
  `cans/02-plugin-model.md:169-172` "Declarative error mapping — Host
  exceptions map directly to POSIX errnos via builder error registry".
- **Fix (spec decision on envelope shape if underspecified, then L2 wiring):**
  implement the structured error envelope per 03/02 across the FFI boundary.
  This is spec-mandated, not anchor-less.

## 4. Fragmented & conflicting session identity models — CONFIRMED, 3 wording fixes

- **Schema — CONFIRMED.** `crates/kelir-catalog/src/schema_catalog.rs:8-9`:
  `session_id BLOB PRIMARY KEY,` / `incarnation_id BLOB NOT NULL,`. Spec:
  `cans/06-storage-catalog.md` "Entity model" ("sessions row carries
  incarnation_id: 128-bit UUIDv7 minted per spawn", "Both row kinds are the sole
  SSOT for session identity and liveness"); `cans/08-lifecycle.md:357-359`
  ("Session incarnation"). Note: the spec never states in one sentence that
  `session_id` is stable across spawns — implied by PRIMARY KEY + CAS, not
  spelled out.
- **Collapse — CONFIRMED, stronger than stated.**
  `crates/kelir-core/src/session_lifecycle.rs:103-105`: `pub fn session_id(&self) -> [u8; 16]
  { self.incarnation_id }`. `Session` has **no** `session_id` field at all —
  only `incarnation_id: [u8; 16]` (line 26) — and a separate `incarnation_id()`
  accessor (344–346) returns the same bytes.
- **Catalog definitions — CONFIRMED.** `schema_catalog.rs:3` (`SCHEMA_V1_SQL`), `:67`
  (`CasLease`), `:75` (`SessionRow`); `crates/kelir-catalog/src/lib.rs:31-35`:
  `CatalogDb::open` executes `SCHEMA_V1_SQL`.
- **Core/catalog disconnect — CONFIRMED.**
  `crates/kelir-core/Cargo.toml` deps have no `kelir-catalog`
  (`kelir-interface`, `kelir-transport`, `kelir-otlp`, serde/toml/ciborium/
  async-trait/thiserror/tokio/tracing/smoltcp/libc); `grep kelir_catalog
  crates/kelir-core/src/` is empty. Only crate depending on `kelir-catalog` is
  `kelir-plugin-cli` (`Cargo.toml:9`).
- **"Random ID" — WRONG WORD.** `mint_incarnation_id()` (session_lifecycle.rs:40–58,
  called from `boot_with_engine:151`): no RNG anywhere — ID is timestamp-ms +
  `AtomicU64` counter + pid + subsec-nanos, shaped UUIDv7 (version nibble
  `0x70` line 48, variant `0x80` line 52). Correct: "mints a
  time/counter-derived UUIDv7-shaped ID in memory, never persisted".
- **"/tmp" — IMPRECISE.** session_lifecycle.rs:160–163:
  `std::env::temp_dir().join("kelir").join(&session_hex).join("workspace")`.
  `temp_dir()` honours `$TMPDIR`; `/tmp` is only the Unix default. Correct:
  "workspace under the OS temp dir (default `/tmp`)".
- **"Only touched by the CLI attach tool" — PARTIAL.** Zero rows: follows from
  the disconnect above + `register_session` has **no production callers at
  all** — only `crates/kelir-catalog/tests/session_lease_catalog_test.rs:43,73`.
  CLI attach (`crates/kelir-plugin-cli/src/attach_cli.rs:49-50`) only does
  `CatalogDb::open(catalog_path()?)` + `lookup_session` — a **read**.
  `Session::attach` (session_lifecycle.rs:88–97) is an L3 TODO stub returning
  `Err("TODO (L3): Session::attach")`. So in production the catalog is
  read-only-from-CLI and never written by anything.
- **Fix — spec decision first, then L2:** give `Session` a real `session_id`
  field distinct from `incarnation_id`, and have boot register the row in the
  catalog — OR decide explicitly that the catalog is CLI-only by design and
  core stays in-memory, and record that in 06/08. Do not invent schema to
  close the gap.

## 5. Fragmented codegen architecture — CONFIRMED, count + paths corrected

- **Four pipelines exist — CONFIRMED in substance, count is FIVE emit steps.**
  1. Interface codegen bin — `crates/kelir-interface/src/bin/codegen/main.rs`,
     syn-based `parse_model`, bin still named `codegen`
     (`crates/kelir-interface/Cargo.toml:17-19`).
  2. `crates/kelir-core/codegen/gen-fuse-uapi.sh`: `cc fuse_uapi.c` →
     `src/planes/storage/fuse_storage/uapi_fuse.rs`.
  3. `crates/kelir-shim/codegen/gen-elf-uapi.sh`: `cc elf_uapi_kernel.c` +
     `elf_uapi_glibc.c` → `src/introspect_trap/uapi_trap.rs`.
  4. `crates/kelir-testkit/src/bin/anchors_testkit.rs`: scans Markdown, writes
     `crates/kelir-testkit/anchors.json` (`anchors_testkit.rs:16-17`).
  5. `justfile:205-219` `codegen` runs a fifth emit step the original order
     missed: `cargo run -p kelir-testkit --bin scorecard -- generate` (writes
     `target/scorecard.md`; `scorecard_testkit.rs:112,132-136`). "Four" undercounts
     unless scorecard is explicitly excluded as a report, not a generator.
  - `gen_plugin_sdk.py` / `gen-fusion-tables.mjs` deletion — CONFIRMED: no
    source files remain anywhere (excluding `node_modules` / `target`); only
    stale bytecode `packages/py/scripts/__pycache__/gen_plugin_sdk.cpython-312.pyc`.
    Fusion emission is now inside the Rust bin (`fusion_codegen.rs`; `main.rs:213`
    still prints `gen-fusion-tables: wrote ...` — stale log string, fix it).
- **Self-overwriting crate boundary — CONFIRMED.** `main.rs:124-173` writes:
  `crates/kelir-shim/include/kelir/wire.h` (:145),
  `crates/kelir-otlp/src/records_otlp.rs` (:148),
  `crates/kelir-plugins/plugin-ledger/src/wal_ledger/merkle_wal.rs` (:154), plus its own
  crate's `crates/kelir-interface/src/telemetry/merkle_attest.rs`, and TS/Py SDK
  files (`packages/ts/...`, `packages/py/kelir/generated/...`,
  `packages/py/kelir/plugins/plugin_<name>.py`).
- **Fix (L2):** `just codegen` already exists as the single entry point — the
  fix is consolidation of the UAPI shell scripts + anchors + scorecard into it
  (or an explicit exclusion for scorecard), and either stop cross-crate
  emission or record it as intended (spec decision). Note: the rename wave
  broke both UAPI scripts (targets renamed, scripts still wrote `uapi.rs` —
  the fuse script failed `Directory nonexistent`). The rename pass fixed their
  output paths (`uapi_fuse.rs`, `uapi_trap.rs`) and re-ran both; outputs
  byte-identical to the committed files.

## 6. Wire framing & protocol layout incoherence — MIXED: 6a partial, 6b/6c refuted as attributed

### 6a. Header lengths — PARTIAL on the name, CONFIRMED on the layout
- Bus defines **`FRAME_HEADER_LEN: usize = 9`**, not `HEADER_LEN`
  (`crates/kelir-core/src/bus/mod.rs:13`). `encode_frame` (:104–124) and
  `take_frame` (:128–155) implement exactly stream_id LE u32 / flags u8 /
  len LE u32, payload after byte 9. Shim `crates/kelir-shim/src/codec_protocol.rs:13`
  defines `HEADER_LEN = size_of::<u32>() + size_of::<u8>() + size_of::<u32>()`
  = 9, with matching `encode_frame` / `decode_header`.
- `msg_type`-as-payload — CONFIRMED and by design: shim `decode_msg_type(payload)`
  (:60–65) strips the first u16 of the payload; bus `send_control_msg`
  (:273–289) prepends `msg_type as u16` to the body then frames it;
  `take_frame` returns the payload verbatim (msg_type bytes included);
  consumers `crates/kelir-shim/src/multiplexer_protocol.rs:235-236,274-275` call
  `decode_msg_type`.
- Spec sides with the bus/shim: `cans/11-protocol.md#Wire-framing` (:163–164):
  "Layout: 9-byte header, stream_id u32, flags u8, len u32, LE, no
  magic/version" / "Message-type carrier: msg_type is first u16 of payload
  (LE); header carries no msg_type". `#Stream-mux-framing` (:242): "Frame
  layout: stream_id: u32, flags: u8, len: u32, payload" + flag values
  0x01/0x02/0x04/0x08/0x10. Ownership: `cans/11` says "see:
  artifacts/code/wire-protocol.rs"; the artifact says "Owned by:
  cans/11-protocol.md#Wire-framing" and "The header on the wire is 9 bytes …
  with no magic, version, channel, or sequence field".
- **Fix (L2, trivial):** rename the bus constant to `HEADER_LEN` for parity.
  No layout change.

### 6b. "11-byte frame header" — REFUTED; vectors, bus, shim, and spec all agree
- `crates/kelir-plugin-testkit/src/vectors_testkit.rs:64-92` (`WireFrame::encode`):
  `body_len = payload.len().checked_add(2)` (:69–73), `len` field =
  `body_len`, capacity `body_len.checked_add(9)` (:80–84), then writes the
  9-byte header + `msg_type` u16 + payload. The wire header is still **9
  bytes**; `+2` is inside the `len` field (msg_type is the first 2 bytes *of
  the payload*), and `checked_add(9)` is total capacity, not header length.
  "11-byte header" misreads the arithmetic.
- `WireFrame::encode` is byte-identical in layout to bus `send_control_msg`
  (same +2, same LE order, same 64 KiB cap, same `MAX_STREAM_ID` check) and
  to shim `decode_header` + `decode_msg_type`. The "test vectors vs runtime
  bus disagree" claim is **REFUTED**.
- **Genuine narrower gap (replaces the claim):** `WireFrame` has **no `decode`**
  in the testkit (no `fn decode` in `vectors_testkit.rs`); decode lives only in shim
  `codec_protocol.rs` and bus `take_frame` (which does not strip `msg_type`).
- **Fix (L2):** add `WireFrame::decode` in the testkit, or drop `WireFrame` in
  favour of the bus/shim codec.

### 6c. "codec_protocol.rs vs protocol_wire.rs define the 7 constants twice" — REFUTED as attributed; real duplication is elsewhere
- `protocol_wire.rs` defines **zero** of `FLAG_OPEN` / `FLAG_DATA` / `FLAG_CLOSE` /
  `FLAG_RST` / `FLAG_WINDOW_UPDATE` / `MAX_FRAME_PAYLOAD` / `MAX_STREAM_ID`
  (it defines `MsgType` / `StreamKind` enums, `EXEC_STREAM_*`, `PROT_*`,
  `MFD_*`). Shim `codec_protocol.rs:5-9` does **not** redefine them — it `pub use`s all
  8 items from `kelir_interface::codec` (a re-export, not a duplicate).
- The real duplication: **`crates/kelir-core/src/bus/mod.rs` redefines**
  `MAX_FRAME_PAYLOAD` (:16), `MAX_STREAM_ID` (:19), `FLAG_OPEN` (:22),
  `FLAG_DATA` (:25) with identical values instead of importing them — and
  only 2 of the 5 flags. Canonical home is
  `crates/kelir-interface/src/codec_protocol.rs:1-15` (all 7 + `MAX_CONTROL_PAYLOAD`,
  `MuxFrameHeader`).
- Third copy: the SSOT artifact `cans/artifacts/code/wire-protocol.rs:19-35`
  defines all 7 constants again (by design — artifacts own content — and its
  line-1 comment cited `// crates/kelir-interface/src/codec.rs`, correct at
  989701f; the rename wave made it stale, and the rename pass updated it to
  `codec_protocol.rs`).
- **Fix (L2):** bus imports the constants from `kelir-interface::codec`;
  delete the 4 redefinitions.

## 7. Isolated terminal plane vs. pipeline engine silo — PARTIAL ("completely isolated" overstates; write path is dead)

- Structs confirmed: `crates/kelir-core/src/planes/terminal/mod.rs:20`
  `ScreenMatrix`, `:51` `Vt100StateMachine`, `:142` `TerminalPlane`, `:177`
  `feed`, `:220` `append_log_chunk`, `:139` `.join("index.bin")`.
- Nuances the original order missed: (a) `append_log_chunk` appends raw chunk
  bytes to the file *named* `index.bin` — it is not an index structure;
  (b) `read_from` (:198–217) reads only in-memory `segment.bytes`, and
  `TODO (L3)` at `:197` says replay from durable files is unimplemented, map
  starts empty each boot — writes reach disk, durable read-back does not exist;
  (c) `Vt100StateMachine::feed` (:72–79) is itself a `TODO (L3)` stub returning
  `Err` — the VT100 engine parses nothing yet.
- Dispatch confirmed: `crates/kelir-core/src/dispatch_plugin.rs:520,535,550,566`
  defines `dispatch_console_input` / `dispatch_console_output` /
  `dispatch_exec_stdout` / `dispatch_exec_stderr`. Each only loops
  `console_interceptors` calling the `ConsoleInterposer` hook
  (`plugin_model.rs:524+`).
- Dispatcher never routes bytes to `session.terminal` — CONFIRMED:
  `PipelineEngine` struct (dispatch_plugin.rs:88–111) has **no terminal field** —
  only interceptor vecs, gate, metrics, FFI queue. Zero occurrences of
  `terminal` in `dispatch_plugin.rs`.
- `TerminalPlane::feed` / `append_log_chunk` never called by the pipeline —
  CONFIRMED, stronger: zero callers in all of `crates/` src; `append_log_chunk`
  is called only by tests
  (`crates/kelir-core/tests/terminal_vt100_matrix_test.rs:45,66,87`). The four
  `dispatch_console_*` fns themselves have **zero callers** anywhere in `crates/`.
- "Completely isolated" is PARTIAL: `session.terminal` *is* reachable on the
  read/resize path — `crates/kelir-bindings/napi/src/lib.rs:267,273,281` and
  `pyo3/src/lib.rs:264,271,281` call `resize` / `export_screen_matrix` /
  `read_from`; examples call `resize`. What is dead is specifically the
  **write** path (`feed`, `append_log_chunk`) and the dispatch→terminal link.
  Also `TerminalPlane::feed` only feeds the state machine — it never appends
  to the log, so feed and append are two unlinked entry points even if wired.
- Spec: real. `cans/04-terminal-plane.md#Terminal-pipeline` (:27–32:
  "Interposer dispatch" is step 4 of the host filter pipeline; :51–56:
  append-only logs, 64-bit cursor, `index.bin`) and
  `cans/artifacts/diagrams/terminal-pipeline.txt` ("Stream Replay →
  append-only logs under sessions/<id>/logs/ + index.bin, read_from(port,
  cursor)"). No spec line found names a dispatch→`TerminalPlane.feed` call —
  the silo is a code fact; the spec implies the pipeline without mandating the
  wiring call.
- **Fix (L2 wiring, then L3):** wire `dispatch_console_*` →
  `session.terminal.feed` + `append_log_chunk`; implement VT100 `feed` and
  durable read-back (L3); decide whether `index.bin` is a log file or an
  index (spec decision — the name lies either way).

## 8. Triple-written profile interpreters — CONFIRMED, worse than stated; TS wording corrected

- Rust — CONFIRMED: `crates/kelir/src/sandbox_runtime.rs:16-70`:
  `SandboxProfile` / `SessionProfile` / `ComputeProfile` / `ArtifactsProfile` /
  `ClockProfile` / `WatchdogProfile`, all `Serialize+Deserialize`;
  `to_vm_config` at `:74-103`.
- TS — PARTIAL (wording): crawler confirmed —
  `packages/ts/core/src/sandbox.runtime.ts:54-67` `profileValueAt`, `:31-46`
  `PROFILE_FIELDS` (7 entries), `:84-93` `configFromProfile`, `:106-127`
  `renderToml`; no TypeBox import (imports `:3-10` only). Imprecise wording:
  (a) there is **no TypeBox in the TS SDK at all** — `schema.config.ts:9`
  TODO says "emit real TypeBox schemas … without the typebox dependency";
  (b) the file **does** use static TS types (`ProfileRecord`, `KelirConfig`,
  `ProfileField` interface). Correct claim: no *runtime* schema validation;
  hand-rolled crawler.
- Py — CONFIRMED: `packages/py/kelir/lifecycle_sandbox.py:24-35`
  `PROFILE_FIELDS`, `:37-44` `_dig`, `:62-77` `_render_toml`, `:111-116`
  `from_profile` loop.
- Divergence — CONFIRMED, worse than stated: TS and Py lists are **identical
  to each other** (same 7 paths/kinds; only key spelling differs, e.g.
  `memoryMb` vs `memory_mb`). Rust has **12** leaf fields; TS/Py cover 7,
  missing 5: top-level `version`, `session.compute.page_size_kb`,
  `session.clock.deterministic_clock`, `session.clock.pinned_clocksource`,
  `session.clock.preemption_grace_ms`. The sinks can't carry them anyway:
  `KelirConfig` (`packages/ts/core/src/builder.lifecycle.ts:22-31`) and Py
  `KelirBuilder.__init__` (`packages/py/kelir/__init__.py:577-602`) have no
  such keys. Further: even Rust is a subset of the spec — no
  `deny_unknown_fields`, so Rust silently drops `session.topology`,
  `max_vcpus`, `balloon.*`, `devices.*`, `watchdog.timeout_seconds/action`
  (fixture `kelir.yaml:5,65-91`). So TS/Py ⊂ Rust ⊂ spec fixture, not three
  parsers of "the exact same" file.
- Spec: real. `cans/plugins/profile.md#Session-compute-schema-(profile)`
  (`:80-106`) lists all of the above fields, incl. `page_size_kb`, all four
  clock fields, and watchdog `enabled` / `timeout_seconds` / `action`; SSOT
  field list is `cans/artifacts/examples/fixtures/kelir.yaml`.
- **Fix (L2):** one profile schema SSOT (profile.md + fixture); generate the
  TS/Py parsers from it, or hand-sync with a parity test covering all 12 Rust
  fields; add `deny_unknown_fields` or an explicit ignore list in Rust
  (spec decision).

## 9. Type flattening and information loss in SDK models — CONFIRMED, worse than stated; TS is the counterexample

- Rust `ExitKind` — CONFIRMED: `crates/kelir-interface/src/telemetry/receipt_attest.rs:20-28`:
  `Normal` / `Signal(u32)` / `KernelPanic(String)` / `HypervisorFault(String)` /
  `DirtyShutdown` / `HardKilled`, verbatim as quoted. Artifact SSOT
  `cans/artifacts/code/execution-receipt.rs:30-38` has the identical 6
  variants. Serde is externally-tagged + `rename_all="camelCase"`, so on the
  wire they are distinct keys (`{"kernelPanic": "…"}` vs
  `{"hypervisorFault": "…"}`).
- Py generated `ExitKind` — CONFIRMED:
  `packages/py/kelir/generated/attest_telemetry.py:35-38`:
  `kind: str = "normal"`, `signal: int | None = None`,
  `message: str | None = None`, verbatim.
- Generator — CONFIRMED: `crates/kelir-interface/src/bin/codegen/py_codegen.rs`,
  `emit_telemetry` (~`:93-110`): hardcodes `kind: str = "normal"`, emits
  `signal` iff any variant has a `U32` payload, `message` iff any has a `Str`
  payload.
- Squash — CONFIRMED, worse than stated: `kind` is an unconstrained bare
  `str` — no `Literal`, no validator, no from-native converter anywhere in
  `packages/py/` (grep for `kernel-panic` / `hypervisor` / `from_native`:
  zero hits). Zero `ExitKind` references in `crates/kelir-bindings/` — **no
  FFI producer ever populates `kind` / `signal` / `message`**. The two `String`
  variants don't just share a slot; nothing ever sets the discriminator to
  distinct values at all.
- TS has the same flattening — **REFUTED**: `ts_codegen.rs:112-133` `emit_exit_kind`
  generates a discriminated union:
  `packages/ts/core/src/telemetry.attest.generated.ts:32-38`
  (`"kernel-panic"` / `"hypervisor-fault"` distinct tags with
  `message: string`) plus `NativeExitKindRecord` and `fromNativeExitKind`
  converter (`:87-110`). TS is the counterexample; the Py generator is the
  outlier.
- Spec: partial. `cans/10-attestation.md#Execution-receipts` is cited in the
  generated headers and the artifact enum is SSOT, but no spec sentence found
  mandates per-SDK variant preservation. The parity gate
  (`#Multi-language-parity-gate`, `cans/15-operations.md:32,61`) is cited in
  generated headers, but its leaf body states a release gate, not an
  enum-projection rule — so the flattening violates the artifact enum's
  integrity, not a quoted spec sentence.
- **Fix (L2):** make the Py generator emit the TS-style discriminated union +
  `from_native` converter (copy the TS emitter shape); add an FFI producer
  populating it, or mark the Py receipt L3 TODO explicitly. Do not "fix TS".

## 10. Split and unsynchronized state enums — CONFIRMED, best-anchored of the ten

- Spec — CONFIRMED: `cans/08-lifecycle.md:5-8`, verbatim: "Centralized
  Hypervisor Lifecycle State Machine: States: Initializing, Running, Draining,
  Suspended(SuspendReason), Terminated" + "Eliminates independent suspend
  implementations outside core". Also `:12`: "Entered via
  `session.drain(timeout_ms)`".
- `DriverState` — CONFIRMED: `crates/kelir-interface/src/driver_interface.rs:41-45`:
  `Unbooted` / `Booted` / `Paused`. Only ever constructed as `Unbooted` in all
  6 drivers; `ensure_booted` treats `Booted|Paused` as ok. No
  `Initializing` / `Running` / `Draining` / `Suspended` / `Terminated` member;
  no mapping to spec states.
- `Session` — CONFIRMED, with nuance: struct
  `crates/kelir-core/src/session_lifecycle.rs:12-28` has no phase field — only
  `detached: bool` (plus terminal, storage, wire, checkpoint, host, metrics,
  uart, driver, owned_bus, engine, incarnation_id, plugins). Lifecycle
  **verbs** exist as methods — `suspend` (`:411`), `resume` (`:422`),
  `drain` (`:429`), `stop` (`:437`), `stop_with_timeout` (`:445`), `kill` —
  but all are `TODO (L3)` stubs returning
  `Err(PluginError::Halt("TODO (L3): …"))` and store no phase.
- `plugin-wire-ingress` — CONFIRMED: `crates/kelir-plugins/plugin-wire-ingress/src/lib.rs:141-146`
  `IngressState`: `parked`, `pending_wake: Mutex<bool>`, `suspended: Mutex<bool>`,
  plus a third piece the original order omits — `suspend_path:
  Mutex<Option<String>>`. `begin_parking` sets `suspended=true`. This directly
  contradicts spec `08-lifecycle.md:8` ("Eliminates independent suspend
  implementations outside core").
- No unified supervisor state machine — CONFIRMED: no `SessionState` /
  `LifecycleState` / `SupervisorState` / `VmLifecycleState` enum in `crates/`,
  `packages/`, or `cans/` (grep: only prose "supervisor" + a shim TODO comment).
  Closest enums are events, not state: `LifecycleObservation`
  (`plugin_model.rs:174-179`: `Checkpoint` / `Rollback` / `Resume` / `Terminate`)
  and `LifecycleEvent` (`:182-187`: `VmInit` / `VmReady` / `VmPreempt` /
  `VmShutdown`) — no current-state field, no transition function.
  `SuspendReason` **does** exist (`driver_interface.rs:356-359`:
  `External(ExternalSuspendKind) | ResourcePressure`, matching the spec shape)
  but nothing holds a `Suspended(SuspendReason)` state.
- **Fix (L2 state + L3 transitions):** introduce a single supervisor state
  machine in `kelir-core` (states per 08:5–8, transitions,
  `Suspended(SuspendReason)`), and replace `DriverState` + the `detached` bool
  + the ingress booleans with references to it; drivers/ingress become
  observers. The spec already mandates it — no spec change needed.

---

## Dropped / reframed claims (do not re-raise in the original wording)

- §3c "test plan contradicts itself" — refuted; the real item is dead mock
  exports + the AS-01 arm mismatch (see §3c).
- §6b "11-byte header; vectors vs bus disagree" — refuted; the real item is
  the missing `WireFrame::decode` (see §6b).
- §6c "codec_protocol.rs vs protocol_wire.rs duplicate the 7 constants" — refuted; the real
  item is `bus/mod.rs` redefining 4 of them (see §6c).
- §9 "TS flattens ExitKind too" — refuted; TS is the correct shape to copy
  (see §9).
- §1d "attach_cli.rs is a 4th relay-path implementation" — refuted; it calls core
  (see §1d).
- §2d "HList is useless compile-time overhead" — refuted; it powers typed
  `ctx::<T>()` per spec (see §2d).
- §4 "random ID" / "/tmp" — corrected to time/counter-derived UUIDv7-shaped
  ID and OS temp dir (see §4).

## Open decisions (decide from intent, record after the fact — do not hold the order for them)

1. §2a–2c: forward the three plane cfgs into core, or delete the dead builder
   methods in Rust/TS/Py?
2. §2d: thread typed contexts into the engine, or record that HList serves the
   SDK session only?
3. §4: boot registers the catalog row, or the catalog is CLI-only by design
   (record in 06/08)?
4. §5: consolidate the UAPI/anchors/scorecard steps into the single codegen
   entry, or explicitly exclude scorecard as a report? Cross-crate emission:
   stop it, or record it as intended?
5. §7: is `index.bin` a log file or an index? (Name lies either way.)
6. §8: `deny_unknown_fields` in Rust, or an explicit ignore list?
7. §3c: delete the unused mocks, or use them in a unit-tier test? And align
   the AS-01 arms which way?

## Execution notes

- One agent per worktree/branch, exclusive scope, no push from agents
  (AGENTS.md §0). Converge by squash-merge into one commit, re-gate after.
- Gates before calling any part done: `cargo check` / `clippy -D warnings` /
  `fmt --check` / `cargo test` / `tsc --noEmit` / `mypy` / `just check-hygiene`
  / `just check-spec` when `cans/` changes / `check-manifest check-tests
  check-scorecard`. A gate not run is not green.
- This rewrite changes `work-order.md` only. No code, spec, or artifact was
  touched in this turn.
