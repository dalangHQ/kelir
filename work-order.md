### 1. Blatant Code & Logic Duplication

* **Duplicate Network Layout & Packet View Engines**:
  * `crates/kelir-core/src/planes/wire/layout.rs` and `crates/kelir-plugins/plugin-db-proxy/src/reassembler/layout.rs` are near-verbatim clones. Both independently define byte offsets (`ETH_HEADER_BYTES`, `IPV4_MIN_HEADER_BYTES`, `TCP_MIN_HEADER_BYTES`), bitmasks, and unsafe zero-copy views (`EthView`, `Ipv4View`, `TcpView`).
* **Copy-Pasted Driver Boilerplate**:
  * `driver-avf`, `driver-bwrap`, `driver-hvf`, `driver-openvmm`, `driver-proot`, `driver-rustvmm`, and `MockVmmDriver` each duplicate ~20 identical `VmmDriver` trait stub methods (`adjust_balloon`, `step_clock_window`, `commit_broker_upper`, `discard_broker_upper`, `freeze_vcpus`, `unfreeze_vcpus`, `entropy_position`, `restore_entropy_position`). Default trait implementations in `kelir-interface` would eliminate hundreds of lines of identical `Err(VmmError::Unsupported(...))` stubs.
* **Hex Encoding/Decoding Reinvented 9 Times**:
  * Instead of standardizing on `hex` or a shared utility in `kelir-interface`, bespoke byte-to-hex and hex-to-byte routines are hand-rolled in:
    * `crates/kelir-interface/src/bin/codegen/otlp.rs` (`hex_lower`)
    * `crates/kelir-otlp/src/records.rs` (`hex_lower`)
    * `crates/kelir-plugins/plugin-ledger/src/otel.rs` (`hex`, `hex_digit`, `decode_hex`)
    * `crates/kelir-plugins/plugin-vault/src/service.rs` (`hex_encode`)
    * `crates/kelir-plugin-cli/src/attach.rs` (`hex_val`, `parse_session_id`)
    * `crates/kelir-plugin-cli/src/fixtures.rs` (`hex_digest`)
    * `crates/kelir-plugin-cli/src/parity.rs` (`decode_hex`, `encode_hex`)
    * `crates/kelir-plugin-testkit/src/vectors.rs` (`hex`)
    * `crates/kelir-core/src/session.rs` (`incarnation_hex`)
* **Session Relay Path Resolution**:
  * Regex validation, home directory discovery, and string formatting for `~/.kelir/sessions/<id>/relay.sock` are implemented independently across `kelir-core/src/session.rs`, `kelir-plugin-cli/src/attach.rs`, `packages/ts/core/src/session.lifecycle.ts`, and `packages/py/kelir/__init__.py`.
* **Ad-hoc Thread Wakers (`block_on`)**:
  * Identical `ThreadWaker` and `block_on` runtime implementations are duplicated in `crates/kelir-bindings/pyo3/src/lib.rs` and `crates/kelir-plugins/plugin-vault/src/lib.rs`.

---

### 2. The "Facade Illusion" (Builder Configuration Black Holes)

* **Silent Config Dropping in Rust Builder**:
  * `crates/kelir/src/builder.rs` exposes fluent builder APIs: `.terminal()`, `.storage()`, `.wire()`, and `.lock_broker()`.
  * In `finish_boot()`, `terminal_cfg`, `wire_cfg`, and `lock_broker_cfg` are **never passed to `kelir-core`**. They are stored in `Registration` and silently dropped. Only `synthetic_files` from `storage_cfg` are forwarded.
* **No-Op Builder Methods in TypeScript**:
  * In `packages/ts/core/src/builder.lifecycle.ts`, `.terminal()`, `.storage()`, `.wire()`, and `.params()` accept complex schema objects and literally return `this` without storing the arguments.
* **Ignored Subsystem Configs in Python**:
  * `packages/py/kelir/__init__.py` records `kwargs` in `self._config["terminal"]`, `["storage"]`, and `["wire"]`. When calling `native.PyKelirBuilder` in `boot()`, only compute, memory, and kernel paths are forwarded; all plane configs are discarded.
* **Zero-Value HList Typing in Rust**:
  * `crates/kelir/src/builder.rs` and `hlist.rs` implement type-level `HList` recursion (`HCons`, `HNil`) to statically track plugin capabilities at compile time.
  * In `finish_boot()`, this typed structure is erased into a dynamically allocated vector (`Vec<Box<dyn AnyPlugin>>`), making the entire generic `HList` machinery useless compile-time overhead that doesn't feed the underlying microkernel.

---

### 3. Architectural Incoherence & Leaky Abstractions

* **Layer Violation in `plugin-db-proxy`**:
  * The architecture specifies that `WirePlane` handles raw VirtIO-Net packet interception, Ethernet decoding, and L4 stream diversion.
  * Despite this, `plugin-db-proxy` bypasses `WirePlane`'s L4 abstraction, pulls raw Ethernet frames via `NetInterposer`, parses IPv4/TCP headers from scratch, and runs an internal TCP reassembly engine (`TcpStreamReassembler`). A domain plugin is implementing transport-layer networking.
* **Trampoline Table Generation Disconnect**:
  * Both TypeScript (`builder.lifecycle.ts`) and Python (`__init__.py`) execute AOT hook fusion, generate a `FusedTrampolineDescriptor`, and pass it to the native bindings (`NativeKelirBuilder::fuse_hooks`).
  * In both `kelir-napi` and `kelir-pyo3`, `NativeKelirBuilder::boot()` verifies that `trampoline_entry_count` matches the descriptor, and then **ignores the table completely**. `Session::boot_with_plugins` is invoked without passing the fused trampoline table. None of the fused trampolines are wired to the runtime engine.
* **Test Plan / Anti-Slop Self-Contradiction**:
  * `crates/kelir-testkit/src/bin/check_tests.rs` strictly enforces rule `AS-01`, which rejects test files containing identifiers like `MockVmmDriver` and `MockBusDispatcher`.
  * Yet `crates/kelir-plugin-testkit/src/lib.rs` and `mock_bus.rs` export `MockVmmDriver` and `MockBusDispatcher` as first-class public APIs. The framework exports mock infrastructure that its own verification linter forbids.
* **Schizophrenic Error Handling**:
  * Errnos are alternately passed as native POSIX integers, typed variants in `VmmError`, string codes wrapped inside generic `PluginError::Halt("EPERM: ...")` blobs via string concatenation, and catch-all `EConfig` error types in Python and TypeScript. There is no structured machine-readable error transport across the FFI boundaries.
 
  * ### 4. Fragmented & Conflicting Session Identity Models

* **`session_id` vs. `incarnation_id` Identity Crisis**:
  * In `crates/kelir-catalog/src/schema.rs`, `session_id` and `incarnation_id` are strictly distinct entities (`session_id BLOB PRIMARY KEY, incarnation_id BLOB NOT NULL`). A persistent session has a stable ID, while each boot/spawn mints a unique `incarnation_id`.
  * In `crates/kelir-core/src/session.rs`, `Session` completely collapses this distinction: `session.session_id()` literally returns `self.incarnation_id`. 
* **Complete Disconnect Between `kelir-core` and `kelir-catalog`**:
  * `kelir-catalog` defines the canonical SQLite session persistence schema (`SCHEMA_V1_SQL`, `SessionRow`, `CasLease`). 
  * However, `kelir-core` does not import or depend on `kelir-catalog`. When `Session::boot` or `Session::boot_with_plugins` runs, it mints a random ID in memory, creates a temporary directory in `/tmp`, and writes zero rows to the catalog database. The catalog exists in a vacuum and is only touched by the CLI attach tool.

---

### 5. Fragmented Codegen Architecture

* **Four Disconnected Generation Pipelines**:
  * The workspace maintains four separate, uncoordinated code generators instead of a unified build step:
    1. `crates/kelir-interface/src/bin/codegen/main.rs`: A heavyweight `syn`-based parser generating TS/Python SDK files, wire headers, and OTLP records.
    2. `crates/kelir-core/codegen/gen-fuse-uapi.sh`: Shell script invoking `cc` against `fuse_uapi.c` to emit `uapi.rs`.
    3. `crates/kelir-shim/codegen/gen-elf-uapi.sh`: Shell script invoking `cc` against `elf_uapi_kernel.c` and `elf_uapi_glibc.c` to emit `uapi.rs`.
    4. `crates/kelir-testkit/src/bin/anchors.rs`: Separate binary scanning Markdown files to generate `anchors.json`.
* **Self-Overwriting Crate Boundary Violation**:
  * `kelir-interface`'s codegen binary generates source files located inside completely different workspace crates (`crates/kelir-otlp/src/records.rs`, `crates/kelir-shim/include/kelir/wire.h`, `crates/kelir-plugins/plugin-ledger/src/wal/merkle.rs`). This couples crates across the workspace through out-of-band file emission rather than shared library types.

---

### 6. Wire Framing & Protocol Layout Incoherence

* **Inconsistent Header Lengths and Offsets**:
  * `crates/kelir-core/src/bus/mod.rs` and `crates/kelir-shim/src/codec.rs` define `HEADER_LEN = 9` (4 bytes `stream_id` + 1 byte `flags` + 4 bytes `len`). Under this model, `msg_type` is treated as payload data.
  * In `crates/kelir-plugin-testkit/src/vectors.rs`, `WireFrame::encode()` treats `msg_type` as a required protocol header field, packing it directly after `len`, producing an 11-byte frame header (`checked_add(9) + 2`).
  * Test vectors and runtime bus dispatchers disagree on whether `msg_type` belongs to the transport frame envelope or the application payload.
* **Redundant Wire Enums**:
  * `crates/kelir-interface/src/codec.rs` and `crates/kelir-interface/src/protocol.rs` define redundant framing constants: `FLAG_OPEN`, `FLAG_DATA`, `FLAG_CLOSE`, `FLAG_RST`, `FLAG_WINDOW_UPDATE`, `MAX_FRAME_PAYLOAD`, and `MAX_STREAM_ID` are defined twice in the same crate across two sibling files.

---

### 7. Isolated Terminal Plane vs. Pipeline Engine Silo

* **Dead Pipeline Routing for Terminal I/O**:
  * `crates/kelir-core/src/planes/terminal/mod.rs` implements `Vt100StateMachine`, `ScreenMatrix`, and `TerminalPlane` (with on-disk chunk appending via `index.bin`).
  * `crates/kelir-core/src/dispatch.rs` defines `dispatch_console_input`, `dispatch_console_output`, `dispatch_exec_stdout`, and `dispatch_exec_stderr`.
  * The dispatcher invokes `ConsoleInterposer` trait hooks, but **never routes bytes to `session.terminal`**. `TerminalPlane::feed` and `TerminalPlane::append_log_chunk` are never called by the pipeline engine. The terminal emulator and chunk logger sit completely isolated from the execution streams.

---

### 8. Triple-Written Profile Interpreters

* **Serde vs. Manual Path Crawlers**:
  * `crates/kelir/src/sandbox.rs` uses strongly typed Rust structs (`SandboxProfile`, `SessionProfile`, `ComputeProfile`) to parse `kelir.toml`/`kelir.yaml` via `serde`.
  * `packages/ts/core/src/sandbox.runtime.ts` ignores TypeBox/types and hand-rolls a dynamic recursive dictionary crawler (`profileValueAt`) using a hardcoded `PROFILE_FIELDS` tuple list.
  * `packages/py/kelir/lifecycle_sandbox.py` duplicates the exact same untyped crawler with `_dig()` and its own copy-pasted `PROFILE_FIELDS` tuple list.
  * Three diverging implementations parse the exact same configuration files. Changes to schema fields in `kelir.toml` require manually synchronizing Rust struct fields and two separate string-path list arrays in TS and Python.

---

### 9. Type Flattening and Information Loss in SDK Models

* **Lossy Python `ExitKind` Translation**:
  * In `kelir-interface/src/telemetry/receipt.rs`, `ExitKind` is a strongly typed enum:
    ```rust
    pub enum ExitKind {
        Normal,
        Signal(u32),
        KernelPanic(String),
        HypervisorFault(String),
        DirtyShutdown,
        HardKilled,
    }
    ```
  * In `packages/py/kelir/generated/attest_telemetry.py` (and the `codegen/py.rs` generator that produces it):
    ```python
    class ExitKind(BaseModel):
        kind: str = "normal"
        signal: int | None = None
        message: str | None = None
    ```
  * String variants (`KernelPanic` vs `HypervisorFault`) are squashed into an ambiguous `message` field, destroying Rust's variant integrity across language boundaries.

---

### 10. Split and Unsynchronized State Enums

* **Disjoint Machine States**:
  * The lifecycle specification dictates state transitions through `Initializing`, `Running`, `Draining`, `Suspended`, and `Terminated`.
  * The actual implementation scatters state tracking across conflicting, unsynchronized variables:
    * `kelir-interface::driver::DriverState` tracks `Unbooted`, `Booted`, `Paused`.
    * `kelir-core::session::Session` only tracks an isolated `detached: bool` flag with no lifecycle phase enum.
    * `plugin-wire-ingress` manages its own disconnected lifecycle via `suspended: Mutex<bool>` and `pending_wake: Mutex<bool>`.
  * There is no unified supervisor state machine. Multiple components maintain private, uncoordinated booleans representing whether the VM is running, suspended, or unbooted.
