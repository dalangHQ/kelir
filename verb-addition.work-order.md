# WORK ORDER: 38-VERB RING INTERPOSITION & HARDENING ENFORCEMENT

## Objectives
Execute full-tree synchronization for the canonical interposition substrate:
1. **38 Ring Verbs across 8 Segregated Scopes** (`trap:4`, `fs:11`, `net:5`, `console:2`, `exec:3`, `host:5`, `lifecycle:5`, `telemetry:3`).
2. **2 Shim Defense Flags** (`session.shim.mask_proc_environ = true`, `wire.block_abstract_unix_sockets = true`).
3. **4 Kernel Physics Invariants** (OOM immunity on PID 1, `S_IFSOCK` tmpfs isolation, Stage-2 MMU `dirty_bytes` tracking for `mmap`, interactive TTY geometry pump).
4. **3 Host Pre-flights** (macOS HVF entitlement, Linux `/dev/kvm` 0660 / Class 2 fallback, `kelir-pack` RAM EROFS compiler).

Maintain **L1/L2 scope strictly**. Do not implement L3 algorithm meat. Keep deferred implementation paths as compliant `// TODO (L3)` single-line markers with registered tags and slop definitions per `AGENTS.md`.
Enforce Day-1 canonical tone—zero migration history, zero diff apologetics. 
---

## Task 1: Spec Anchors & Parity Baseline (`kelir-testkit`)
* **File:** `crates/kelir-testkit/src/bin/anchors_testkit.run.rs`
  * Run `cargo run -p kelir-testkit --bin anchors` to index all markdown headings and bullet anchors updated in `cans/*.md`.
  * Ensure `crates/kelir-testkit/anchors.json` matches the AST of the updated specification corpus.
* **File:** `crates/kelir-testkit/src/bin/check_manifest.run.rs`
  * Confirm `anchor-exceptions.yaml` and manifest law references resolve with zero errors against the refreshed `anchors.json`.

---

## Task 2: Interface & Codegen Core (`kelir-interface`)
* **File:** `crates/kelir-interface/src/fusion_plugin.flow.rs`
  * Extend `tag_enum! { pub enum TrampolineRing => "ring" }` to declare all 38 ring tags.
  * In `const HOOK_RINGS`: register binding descriptors for the 12 new verbs with their respective rings and lifecycle tags (`None` for non-lifecycle hooks).
  * In `validate_hook`: update hook stage and ring compatibility guards to recognize the 12 verbs.
* **File:** `crates/kelir-interface/src/plugin_model.flow.rs`
  * Extend segregated interposer traits:
    * `TrapInterposer`: Add `async fn on_signal(&self, sender: u32, target: u32, sig: i32) -> Result<Verdict, PluginError>` and `async fn on_fork(&self, parent: u32, child: u32) -> Result<Verdict, PluginError>`. Default to `Ok(Verdict::Pass)`.
    * `FsInterposer`: Add `on_fs_setattr`, `on_fs_symlink`, `on_fs_mkdir`, `on_fs_readdir`, `on_fs_readlink`. Default mutations to `Ok(Verdict::Pass)`, `on_fs_readdir` to `Ok(FilterAction::Forward)`.
    * `NetInterposer`: Add `on_socket_connect`, `on_dns_query`, `on_socket_bind`. Default verdicts to `Ok(Verdict::Pass)` and DNS to `Ok(DnsVerdict::Forward)`.
    * `ConsoleInterposer`: Add `async fn on_exec_stdin(&self, pid: u32, exec_id: u64, chunk: &[u8]) -> Result<Vec<u8>, PluginError>`. Default to `Ok(chunk.to_vec())`.
    * `HostInterposer`: Add `fn on_host_containment_breach(&self, pid: u32, path: &Path, kind: &str) -> Result<(), PluginError>`. Default to `Ok(())`.
* **File:** `crates/kelir-interface/src/handlers_plugin.flow.rs`
  * Expose helper context constructors on `HandlerCtx` for the new event payloads (`with_signal`, `with_fork`, etc.) without taking runtime locks in L2.
* **Codegen Runner:**
  * Run `cargo run -p kelir-interface --bin codegen`.
  * Validate updated output in:
    * `packages/ts/core/src/generated/fusion.tables.generated.ts`
    * `packages/py/kelir/generated/fusion_tables.py`
    * `packages/py/kelir/generated/attest_telemetry.py`

---

## Task 3: Engine Pipeline & Interposition Dispatch (`kelir-engine`)
* **File:** `crates/kelir-engine/src/dispatch_plugin/engine.flow.rs`
  * In `PipelineBinder`: ensure all scoped registrations store their respective `MountScope` and `RingRegistration` entries.
  * In `PipelineEngine`: implement L2 dispatch entry points for the 12 new verbs:
    * `dispatch_signal(sender, target, sig)`
    * `dispatch_fork(parent, child)`
    * `dispatch_socket_connect(pid, proto, ip, port)`
    * `dispatch_dns_query(pid, domain, qtype)`
    * `dispatch_socket_bind(pid, proto, ip, port)`
    * `dispatch_host_containment_breach(pid, path, kind)`
  * Ensure each dispatch follows the interposition invariant: check `fused_crossing()` fast path, verify `allows_device()`, enforce `quarantine_sync` or `quarantine_unit`, and call `foreign_submit_complete()`.
* **File:** `crates/kelir-engine/src/dispatch_plugin/fs_dispatch.flow.rs`
  * Implement dispatch functions on `PipelineEngine` for the 5 extended FUSE verbs:
    * `dispatch_fs_setattr`, `dispatch_fs_symlink`, `dispatch_fs_mkdir`, `dispatch_fs_readdir`, `dispatch_fs_readlink`.
* **File:** `crates/kelir-engine/src/dispatch_plugin/console_dispatch.flow.rs`
  * Add `dispatch_exec_stdin(pid, exec_id, chunk)`. Wrap chunk mutation in `quarantine_unit`.

---

## Task 4: Storage & Wire Plane Invariants (`kelir-storage`, `kelir-wire`)
* **File:** `crates/kelir-storage/src/fuse.run.rs`
  * Map kernel FUSE opcode numbers to verbs in `dispatch()`:
    * `FUSE_SETATTR` (4) -> `dispatch_fs_setattr`
    * `FUSE_SYMLINK` (6) -> `dispatch_fs_symlink`
    * `FUSE_MKDIR` (9) -> `dispatch_fs_mkdir`
    * `FUSE_READDIR` (28) -> `dispatch_fs_readdir`
    * `FUSE_READLINK` (5) -> `dispatch_fs_readlink`
  * In `FUSE_LOOKUP` and `FUSE_MKNOD`: reject `S_IFSOCK` creation outside guest tmpfs mounts (`/tmp`, `/var/run`, `/run`). Return `EPERM`.
* **File:** `crates/kelir-wire/src/smoltcp.run.rs` & `crates/kelir-wire/src/dns.run.rs`
  * In `intercept_outbound`: call `dispatch_dns_query` before synthetic DNS resolution.
  * In `intercept_inbound` / connection handling: wire PID attribution hooks for `on_socket_connect` and `on_socket_bind`.

---

## Task 5: Guest Shim Hardening & Invariants (`kelir-shim`)
* **File:** `crates/kelir-shim/src/main.rs`
  * In `main()`: write `-1000` to `/proc/1/oom_score_adj` before entering the poll/event loop or executing guest tasks.
* **File:** `crates/kelir-shim/src/trap_protocol.run.rs`
  * In `trap_filter_program()`:
    * Add `SYS_kill`, `SYS_tkill`, `SYS_tgkill` to monitor signal generation.
    * Add `SYS_clone`, `SYS_clone3`, `SYS_fork`, `SYS_vfork` to monitor process tree forks.
    * Add `SYS_connect` and `SYS_bind` to intercept socket operations.
  * Enforce `session.shim.mask_proc_environ`:
    * Add seccomp BPF rules trapping `openat` / `openat2` where the target filename contains `environ` under `/proc`.
    * Deny `SYS_process_vm_readv` with `EPERM`.
  * Enforce `wire.block_abstract_unix_sockets`:
    * Inspect `sockaddr_un` on `SYS_bind` and `SYS_connect`. Trap and reject paths where `sun_path[0] == '\0'`.
* **File:** `crates/kelir-shim/src/multiplexer_protocol.run.rs`
  * In `dispatch_frame`: intercept EXEC control sub-lane `0x03` (window geometry update).
  * Issue `ioctl(slave_fd, TIOCSWINSZ)` and deliver `SIGWINCH` to the foreground process group.

---

## Task 6: Facade & Adapter Closures (`crates/kelir`)
* **File:** `crates/kelir/src/builder_adapters.flow.rs`
  * Implement adapter structs (`OnSignal`, `OnFork`, `OnFsSetattr`, `OnFsSymlink`, `OnFsMkdir`, `OnFsReaddir`, `OnFsReadlink`, `OnSocketConnect`, `OnDnsQuery`, `OnSocketBind`, `OnExecStdin`, `OnHostContainmentBreach`).
  * Implement the interposer traits for each adapter with proper error forwarding.
* **File:** `crates/kelir/src/builder_scopes.flow.rs`
  * Add the 12 chainable verb methods to `TrapScope`, `FsScope`, `NetScope`, `ExecScope`, and `HostScope`. Return `Self`.
* **File:** `crates/kelir/src/builder_plugin.flow.rs`
  * Mirror the 12 verbs as top-level methods on `KelirBuilder`.
* **File:** `crates/kelir/src/profile_schema.flow.rs`
  * Add `mask_proc_environ: Option<bool>` to `ShimProfile`.
  * Add `block_abstract_unix_sockets: Option<bool>` to `WireProfile`.
* **File:** `crates/kelir/src/profile_validate.flow.rs`
  * In `validate_overlay()`: enforce monotonicity for the defense flags:
    * `check_guard_kept("session.shim.mask_proc_environ", base.session.shim.mask_proc_environ, overlay.session.shim.mask_proc_environ)`
    * `check_guard_kept("wire.block_abstract_unix_sockets", base.wire.block_abstract_unix_sockets, overlay.wire.block_abstract_unix_sockets)`

---

## Task 7: TypeScript SDK Surface (`packages/ts/core`)
* **File:** `packages/ts/core/src/hook.registry.ts`
  * Update `TrapScope`, `FsScope`, `NetScope`, `ExecScope`, `HostScope` interfaces to include the 12 verbs.
  * In `HookRegistry`: add verb handler recording methods and update `fuse()` to generate valid descriptors.
* **File:** `packages/ts/core/src/builder.lifecycle.ts`
  * Add the 12 verb chaining methods to `KelirBuilder`.
* **File:** `packages/ts/core/src/profile.schema.ts`
  * Add `mask_proc_environ?: boolean` to the shim schema.
  * Add `block_abstract_unix_sockets?: boolean` to the wire schema.

---

## Task 8: Python SDK Surface (`packages/py/kelir`)
* **File:** `packages/py/kelir/builder_scopes.py`
  * Expand `_VERB_RINGS` to contain all 38 verbs with exact `RingEvent` mappings.
  * Set `_VERB_ARITY` tuples to match the required parameter signatures.
* **File:** `packages/py/kelir/kelir_builder_scope.py`
  * Implement the 12 `on_*` methods delegating to `self._scoped_on(verb, handler)`.
* **File:** `packages/py/kelir/kelir_builder.py`
  * Implement top-level `on_*` builder methods on `KelirBuilder`.
* **File:** `packages/py/kelir/profile_config.py`
  * Add `mask_proc_environ: ProfileBool | None = None` to `ShimProfile`.
  * Add `block_abstract_unix_sockets: ProfileBool | None = None` to `WireProfile`.

---

## Task 9: Native FFI Bindings (`napi` & `pyo3`)
* **File:** `crates/kelir-bindings/napi/src/fusion_plugin.flow.rs` & `crates/kelir-bindings/pyo3/src/fusion_plugin.flow.rs`
  * Verify `TrampolineRing::from_tag` parses all 38 ring tags.
  * Ensure trampoline callback registration handles foreign function conversion for the 12 new verbs.
* **File:** `crates/kelir-bindings/napi/src/builder.flow.rs` & `crates/kelir-bindings/pyo3/src/builder.flow.rs`
  * Ingest and forward `mask_proc_environ` and `block_abstract_unix_sockets` into `PlaneWiring`.

---

## Task 10: CLI Pre-flights & Parity Gates (`kelir-plugin-cli`)
* **File:** `crates/kelir-plugin-cli/src/doctor_cli.run.rs`
  * Implement host pre-flight probes:
    * macOS: Probe `com.apple.security.hypervisor` entitlement.
    * Linux: Check `/dev/kvm` exists and has `0660` permissions.
    * RAM EROFS: Check `kelir-pack` availability for pre-boot image compilation.
  * Support `--enforce` writing to `/etc/kelir/hw.lock`.
* **File:** `crates/kelir-plugin-cli/src/setup_cli.flow.rs`
  * Bind `SetupStep::EnforceHwLock` to run the updated doctor probes.
* **File:** `crates/kelir-plugin-cli/src/parity_cli.run.rs`
  * Update `coarse_ring()` to map all 38 verb tags into their 8 scopes.
* **File:** `crates/kelir-plugin-testkit/src/vectors_scope_cases.flow.rs` & `crates/kelir-plugin-testkit/src/vectors_testkit.flow.rs`
  * Generate vectors for all 38 ring verbs.
  * Update total test suite arithmetic to **96 cases** (38 ring-verb cases + 48 scope sub-node cases + 5 chain cases + 5 predicate cases).
* **Parity Vector Serialization:**
  * Run `cargo run -p kelir-plugin-cli -- vectors` to regenerate `vectors.json` to disk with all 96 vectors.

---

## Task 11: Gate Verification Matrix
Execute the following verification sequence. Every step must pass clean with zero errors:

```sh
# 1. Spec & doc integrity
cargo run -p kelir-testkit --bin anchors
just check-manifest
just check-tests

# 2. Interface codegen
cargo run -p kelir-interface --bin codegen

# 3. Parity vectors regeneration & verification
cargo run -p kelir-plugin-cli -- vectors
cargo run -p kelir-plugin-cli -- parity

# 4. Workspace lint & compile
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

# 5. SDK validation
pnpm -r exec tsc --noEmit
uv run mypy packages/py

# 6. Maturity & hygiene checks
just check-hygiene
cargo run -p kelir-maturity -- check --strict
```