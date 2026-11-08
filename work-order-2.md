### 1. Cross-Language SDK Types & Serializers
* **Files:** `packages/ts/core/src/types.ts`, `packages/py/kelir/__init__.py`, `crates/kelir-interface/src/telemetry/`, `crates/kelir-interface/src/protocol.rs`
* **Current state:** Manual duplicate definitions of `ExecutionReceipt`, `SnapshotDescriptor`, `CausalDelta`, `ExitKind`, `PhaseTimings`, `GuestClock`. Over 100 lines of hand-written camelCase-to-snake_case converters (`fromNativeSnapshotDescriptor`, `fromNativeExecutionReceipt`, `fromNativeCausalDelta`).
* **Fix:** Generate TypeScript interfaces and Pydantic models directly from `crates/kelir-interface` via `typeshare` or `specta`. Eliminate manual converters by configuring Serde `#[serde(rename_all = "camelCase")]` across FFI boundaries.

### 2. Plugin SDK Projections (TypeScript & Python)
* **Files:** `packages/ts/core/src/plugins.ts`, `packages/py/kelir/plugins/*.py`
* **Current state:** 13 community plugin wrappers (classes, config interfaces, default args) written entirely by hand in TS and Python to mirror the Rust struct definitions in `crates/kelir-plugins/`.
* **Fix:** Use AST-driven code generation (`kelir-plugin-cli manifest` or a build script) to parse the Rust plugin crates and emit the typed TS builder extensions and Python `@dataclass` modules automatically.

### 3. Trampoline Ring Tables & Hook Bindings
* **Files:** 
  * `crates/kelir-interface/src/fusion.rs`
  * `packages/ts/core/src/fusion.ts`
  * `packages/py/kelir/fusion.py`
  * `packages/py/kelir/plugin.py` (`HOOK_RINGS`)
* **Current state:** Enums (`TrampolineRing`), string tags (`from_tag`/`as_tag`), validation rules (`validate_hook`), and hook-to-ring lookup dictionaries are manually re-implemented across all three languages.
* **Fix:** Define ring events, hook stages, and lifecycle tags once in `crates/kelir-interface`. Auto-generate the TS enums and Python `Enum`/dictionaries via macro export or code generator.

### 4. Plugin Tiers, Dependency Enforcement, & Rosters
* **Files:** `crates/kelir-plugin-cli/src/audit.rs`, `crates/kelir-core/src/plugin_registry.rs`
* **Current state:** Hardcoded static arrays:
  * `PLUGIN_TIERS = &[("plugin-mcp", 1, "mcp"), ...]`
  * `FORBIDDEN_DEPS = &["kelir", "kelir-core", ...]`
* **Fix:** Read `Cargo.toml` workspace metadata and package keywords/metadata sections (`[package.metadata.kelir.tier]`). Derive dependency trees and tier validation dynamically instead of editing static CLI arrays when adding or re-tiering plugins.

### 5. FUSE Opcodes and POSIX Errno Conversions
* **Files:** `crates/kelir-core/src/planes/storage/fuse.rs`
* **Current state:** Hand-rolled string-to-number errno parser (`drop_errno`: `"EPERM" => Ok(1)`, `"ENXIO" => Ok(6)`, etc.) and manual opcode matching (`FuseOpcode::from_u32`: `1 => Lookup, 15 => Read, 16 => Write`).
* **Fix:** Bind directly to `libc` constants (`libc::EPERM`, `libc::ENXIO`) or generate the enum mapping table directly from Linux kernel UAPI FUSE headers (`linux/fuse.h`).

### 6. Parity Verification JSON Parsing & Test Vectors
* **Files:** `crates/kelir-plugin-cli/src/parity.rs`, `crates/kelir-plugin-testkit/src/fixtures.rs`
* **Current state:** 
  * `parity.rs` contains a 300-line hand-written recursive descent JSON parser (`JsonParser`).
  * `fixtures.rs` contains a hardcoded array of 12 vector names (`canonical_vector_names`).
* **Fix:** Replace the manual JSON parser with `serde_json` (already in workspace). Derive vector fixtures directly from serialized unit test runs in `kelir-plugin-testkit` rather than maintaining static string tables.

### 7. Sandbox Config Serialization (TOML/YAML Parsers)
* **Files:** `packages/ts/core/src/sandbox.ts`, `packages/py/kelir/sandbox.py`
* **Current state:** Bespoke line-by-line scalar string parsers (`parseTomlProfile`, `parseYamlProfile`, `_dump_toml`) written to dodge standard parser imports.
* **Fix:** Use standard runtime parsers (`@ltd/j-toml` / `smol-toml` in TS, built-in `tomllib` in Python 3.11+) and schema-driven serializers instead of homebrew text-munging algorithms.

* ### 8. Test Manifest Case Counts & Domain Totals
* **Files:** `crates/kelir-testkit/src/bin/check_manifest.rs`, `cans/artifacts/test-plan/manifest.yaml`
* **Current state:** Manual double-accounting. `manifest.plan.total_cases` and `manifest.domains.<name>.case_count` are hand-keyed integers in YAML. Any added case requires manually incrementing two counters and updating file arrays.
* **Fix:** Auto-derive total and per-domain case counts by walking `cans/artifacts/test-plan/cases/` at manifest compile-time. Treat the directory structure and individual case YAML headers as the SSOT.

### 9. Scorecard Markdown Generation & Budget Sync
* **Files:** `crates/kelir-testkit/src/bin/scorecard.rs`, `cans/artifacts/scorecard.md`, `perf/budgets.toml`
* **Current state:** `render_scorecard()` hardcodes table formats, text columns, status strings (`"pass"`), rule definitions (`AS-01` to `AS-08`), and latency targets into Rust strings. Changes in `perf/budgets.toml` must be manually reflected in `scorecard.rs`.
* **Fix:** Convert `scorecard.md` into a template populated directly from `perf/budgets.toml` and test execution results. The scorecard generator must consume the config file dynamically rather than embedding metric names and units in procedural code.

### 10. L2–L4 Network Frame Parsing & Header Offsets
* **Files:** `crates/kelir-core/src/planes/wire/mod.rs`, `crates/kelir-plugins/plugin-db-proxy/src/reassembler.rs`
* **Current state:** Hardcoded raw byte slicing across network frames:
  * `frame[12..14] == 0x0800` (EtherType)
  * `ETH_HEADER_BYTES = 14`, `IPV4_MIN_HEADER_WORDS = 5`
  * Magic offset slicing: `14 + 12`, `14 + 16`, `14 + 9 == 6` (TCP protocol check)
* **Fix:** Eliminate manual byte-index arithmetic. Auto-generate zero-copy network header views using `smoltcp::wire` primitives or declare layouts using structured declarative byte-offset macros.

### 11. ELF Header Parsing & Auxv Extraction
* **Files:** `crates/kelir-shim/src/introspect.rs`
* **Current state:** Bespoke ELF parser implementations (`parse_interp_64`, `parse_interp_32`) hardcoding field offsets (`0x20..0x28`, `0x36..0x38`, `0x1c..0x20`) and structure sizes (`phentsize < 56`, `phentsize < 32`).
* **Fix:** Generate ELF header structures directly from Linux UAPI headers (`linux/elf.h`). Do not maintain manual binary offset reading logic inside the shim.

### 12. Protocol Wire Codes, Channel IDs, & Stream Kinds
* **Files:** 
  * `crates/kelir-interface/src/protocol.rs`
  * `crates/kelir-shim/src/codec.rs`
  * `packages/ts/core/src/types.ts`
  * `packages/py/kelir/__init__.py`
* **Current state:** Numeric allocations are duplicated manually:
  * Channels: `0x0000`, `0x0001`
  * Sub-lanes: `0x00` (stdin), `0x01` (stdout), `0x02` (stderr), `0x03` (control)
  * Core stream kinds: `0x0001` to `0x0006`
  * Plugin stream kinds: `0x0010` to `0x0012`
* **Fix:** Maintain a single canonical definition table (`protocol.yaml` or a dedicated macro module in `kelir-interface`). Generate the Rust enum representations, C header `#define`s for the musl shim, TypeScript constants, and Python `IntEnum` classes from that single file.

### 13. Spec Heading Anchors & Law References
* **Files:** `crates/kelir-testkit/src/bin/check_manifest.rs`, `cans/*.md`
* **Current state:** `check_manifest.rs` runs a file-existence check (`repo_root.join(file).is_file()`) and defers anchor validation (`#Anchor`) with a `TODO (L2)` comment, leaving link breakage undetected until manual audits.
* **Fix:** Write an anchor extraction step into the spec gate that scrapes headings and anchors from `cans/*.md` into an indexed JSON list (`anchors.json`). `check-manifest` must validate existence against this generated lookup index.

### 14. Hardware Platform Detection & Capabilities
* **Files:** `crates/kelir-testkit/src/bin/scorecard.rs`, `crates/kelir-plugin-cli/src/doctor.rs`
* **Current state:** Duplicated target capability heuristics:
  * `scorecard.rs`: `host_hypervisors()` manually matches `linux -> kvm`, `macos -> hvf, avf`, `windows -> whp`.
  * `doctor.rs`: `probe_host()` contains a separate implementation of the exact same OS-to-hypervisor capability matrix.
* **Fix:** Centralize hardware probe primitives in `kelir-core` / `kelir-drivers`. The testkit and developer CLI must share this single probe module rather than guessing platform capabilities with divergent `cfg` matches.

### 15. Causal Delta Field Serialization & Merkle Leaves
* **Files:** `crates/kelir-interface/src/telemetry/receipt.rs`, `crates/kelir-otlp/src/lib.rs`, `crates/kelir-plugins/plugin-ledger/src/wal.rs`
* **Current state:** Manual byte packing in `ExecutionReceipt::leaf_hash()`, `WalRecord::pack()`, and OTLP record transforms (`OtlpReceiptRecord`, `OtlpCausalDeltaRecord`). Every struct change requires hand-editing raw byte concatenations and OTLP field mappings.
* **Fix:** Use a procedural derive macro (`#[derive(MerkleLeaf)]`) that automatically packs fields in declaration order using RFC 6962 BLAKE3 leaf hashing. Field addition should automatically update serialization and fail compilation if Merkle serialization is omitted.

---

## CI/CD Coherence Architecture

Generating files without an enforcement gate guarantees drift. To keep the generated files, specifications, and code in sync, enforce this exact pipeline:

```
[ cans/*.md + artifacts ] (SSOT)
         │
         ▼
[ kelir-interface (Macros/Schemas) ]
         │
         ├───> Code Generation: TS Types / Py Types / Wire Codecs / Manifests
         │
         ▼
[ CI Coherence Gate: `git diff --exit-code` ]
         │
         ├─── (Clean) ───> Compile (Cargo, TSC, Mypy) ───> Verification Gates
         └─── (Dirty) ───> FAIL: "Generated code out of sync. Run `just codegen`"
```

### 1. The Hermetic Commit Invariant
- **Rule:** Generated files (TypeScript stubs, Python models, generated manifests, scorecards) **must be checked into git**.
- **Rationale:** Downstream users (npm/PyPI consumers, external plugin authors) must not need a code-generation toolchain or a Rust compiler installed just to read types or run unit tests.

### 2. The Verification Gate (`just check-codegen`)
Every CI run must include a zero-drift verification step before running tests:
```sh
# Step 1: Run all code generators
just codegen

# Step 2: Ensure git tree is completely clean
git status --porcelain
git diff --exit-code
```
If an engineer updates a Rust struct in `kelir-interface` but fails to update the corresponding generated TypeScript or Python types, `git diff --exit-code` exits with status `1` and blocks the PR.

### 3. Pipeline Execution Topology
Orchestrate local tools and CI tasks in this strict linear order in the `justfile`:

1. **`just codegen`**:
   - Scrapes `cans/*.md` anchors -> `anchors.json`.
   - Generates TypeScript interfaces and Python models from `kelir-interface`.
   - Updates `manifest.yaml` totals from disk cases.
   - Generates wire channel/stream constants for `kelir-shim`.
   - Renders `scorecard.md` using `perf/budgets.toml`.
2. **`just check-hygiene`**:
   - Validates comment laws (`see:`, `TODO:`, `SAFETY:`).
   - Validates LOC bounds (<1000 LOC ceiling) and denies `#[cfg(test)]` in `src/`.
3. **`just check-coherence`**:
   - `git diff --exit-code` asserts zero drift between SSOT schemas and committed artifacts.
4. **`just check-types`**:
   - Compiles Rust: `cargo check --workspace --all-targets`
   - Compiles TS: `pnpm -r exec tsc --noEmit`
   - Checks Python: `uv run mypy packages/py`
5. **`just check-gates`**:
   - `check-manifest`: Validates case coverage and spec leaf links.
   - `check-tests`: Executes anti-slop AST checks (no mock hypervisors in integration, no unseeded randoms).
   - `check-scorecard`: Verifies physical hardware capabilities against budget limits.
