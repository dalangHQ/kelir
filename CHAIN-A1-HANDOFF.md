# CHAIN-A1-HANDOFF — code changes the spec+artifacts pin needs

Track A1 (spec+artifacts) only. A1 did not touch crates/, packages/,
scripts/, or justfile. The items below are code work for the owning
tracks; the spec pin (cans/02-plugin-model.md#Chain-decisions,
cans/03-sdk-architecture.md#Unified-builder-surface) and the artifacts
already describe the fixed shape.

## A3 (Rust) — builder_sdk.rs / builder_plugin.rs / builder_scopes.rs

- trap: both modes get both forms. Rename the plugin-mode scope form
  `trap(closure)` to `trap_scope(closure)`; add `trap(filter, f) ->
  Self` to the Plugin-mode impl (filter validation deferred to seal).
  Host `trap(filter, f)` changes `Result<Self>` -> `Self`.
- trap_async / post_exec (Host impl): `Result<Self>` -> `Self`; retain
  each filter and validate it at seal(). The `?` mid-chain on these
  verbs in cans/artifacts/examples/autonomous-swe-agent.rs stays until
  this lands — A3 removes those `?`s then.
- plugin() / plugin_instance(): `Result<Self>` -> `Self` (live
  plugin_instance already returns Self; live plugin() checks duplicates
  with Err). Move the duplicate-mount check into seal().
- seal(): add (a) trap-filter validation (empty binary / empty arg,
  host label "host", mounted plugins named) and (b) duplicate-mount
  rejection (`plugin already mounted: <name>`); keep vsock/scope
  triples. No host-only-verb check in Rust: those verbs do not exist
  on Plugin-mode builders.
- Typestate: verified live — ring verbs sit on
  `KelirBuilder<Mode, Unsealed, ...>` (builder_plugin.rs:42) and the
  host-only verbs on `KelirBuilder<Host, Unsealed, ...>`
  (builder_sdk.rs:399); Sealed-generic impls hold only bind_registry /
  finish_boot. Config-after-seal is already a compile error; A3 just
  confirms no Sealed-gated config verb remains.
- Plugin-of-plugin stays a compile error (no plugin() on Plugin-mode
  builders). No generics redesign.
- vectors.json: still 58. The 4 chain cases are pinned in
  cans/artifacts/code/plugin-builder-vectors.rs (names
  plugin-builder-seal, plugin-builder-trap-filter,
  plugin-builder-trap-scope, plugin-builder-lock-broker); the generator
  (crates/kelir-plugin-testkit fixtures) must emit them, taking the
  total to 62. Do not hand-edit frame hex.

## A4 (TypeScript) — builder.lifecycle.ts

- Add seal(): this with a sealed flag; any verb after seal() throws
  EConfig. boot() seals implicitly when not sealed yet.
- fuse(): make internal (not a chain verb, not a terminal); boot()
  stays the single terminal. fuse().describe() remains the one
  descriptor path, called inside seal()/boot().
- use()/plugin(): move duplicate-mount and missing-prerequisite checks
  from mount time to seal(). Host-only verbs stay fail-fast at the call
  site. Live code still throws duplicates at mount; the spec pins seal-time.
- Add guard(enabled, fn), boolean-conditional only; forkState() throws
  EConfig on a host-mode builder (live returns a clone); plugin(string)
  throws EConfig.
- TS examples (policy-matrix.ts, hook-matrix.ts,
  autonomous-swe-agent.ts, serverless-webhook.ts) boot directly today
  because seal() does not exist live; add `.seal()` before `.boot()`
  there once seal() lands.
- forkState() host-mode: the pin is plugin-mode only, host throws EConfig
  (A4 lands); live TS returns a clone, live Py raises EConfig. Hosts fork
  via Session.fork().
- TS instance .plugin() alias is kept in the pin (live, delegates to
  use()); D4 keeps it, do not delete.
- No standalone lockBroker verb: the lock broker stays the storage
  config lockBroker flag.

## A5 (Python) — kelir_builder.py

- Add seal() -> Self raising EConfig; any verb after seal() raises
  EConfig; boot() seals implicitly when not sealed yet.
- fuse(): make internal (not a chain verb, not a terminal).
- use(): move duplicate-mount and missing-prerequisite checks from mount
  time to seal(). Host-only verbs stay fail-fast at the call site. Live
  code still raises duplicates at mount.
- D4 reverses the deletion: instance .plugin(builder) mounts by
  delegating to .use(); instance .plugin(name) raises EConfig.
  KelirBuilder.plugin(name) stays the plugin-mode factory. A1 already
  re-chained every Py example to .use() (policy-matrix.py:118-119,
  hook-matrix.py:142, plus autonomous-swe-agent.py,
  db-backed-runtime.py, container.py); those stay valid, .use() is
  still the mount verb.
- Add group(fn) and guard(enabled, fn), boolean-conditional only;
  fork_state() host-mode raise is already live.
- trap(filter, pre, post) on a plugin-mode builder raises host-only
  live; the pin says both modes, so the raise goes (spec wins).
- Py examples call .seal() now (policy-matrix.py, hook-matrix.py,
  autonomous-swe-agent.py, db-backed-runtime.py, container.py). They
  pass py_compile but will raise AttributeError at runtime until seal()
  lands — land seal() or expect those examples to fail on execution.
- context() on host mode is already wired in live; keep it.
  on_trap_deny is a plain method live (artifact fixed to match); the
  old decorator-property shape is gone.
- fork_state() host-mode raises EConfig live; keep.

## A6 (CI parity gate) — scripts/check-builder-parity.sh

- Machine-readable verb set: cans/artifacts/config/builder-verbs.yaml
  (schema kelir-builder-verbs/v1; 2 entries, 58 verbs, 8 scopes,
  0 open gaps). The gate should read this file, not the prose pin.
- group, guard, end are verbs there now (D1), not open gaps.
  open_gaps is empty. Vectors stay 58 until the A3 generator lands
  the 4 chain cases (62 total).
- Found 2026-11-12: the live gate still reads
  scripts/builder-parity-verbs.yaml, not this file; A6 owns aligning
  the consumer.
- User requirement (2026-11-12): parity must be backed by CI gating,
  not just an audit report.
- vectors.json stays 58 until the A3 generator work above lands the
  4 chain cases (total 62).

## Coordinator

- TS examples cannot show .seal() until A4 lands it: the TS leg of
  just check-examples typechecks them against the live dist, so adding
  .seal() there now would fail tsc. Py examples already show .seal()
  (py_compile only, passes); RS examples show .seal()?.boot() live.
- .end() / .group() / .guard() are open gaps, deliberately not faked
  into examples or vectors.
- Rust artifact split 2026-11-12: cans/artifacts/code/rust-plugin-builder.rs
  is 973 lines (surface) plus cans/artifacts/code/rust-plugin-builder-scopes.rs
  at 373 lines (scopes); both under the 1000 LOC rule. The borrowed-vec
  scope shape is fixed: scopes own the builder, scope-entry closures
  return the builder via .end(). Flat ring verbs push to the registry
  directly; plugin() pushes one MountedPlugin; seal() mirrors the live
  triple plus duplicate check with Errno::Econfig.

## Convergence (coordinator, 2026-11-12)

Reconciled against the landed code; the pin above was corrected where it
contradicted the code tracks:
- (b) Rust has no boot() on Unsealed; seal()?.boot() is the only route.
  TS/Py boot() seals implicitly.
- (e) group()/guard()/.end(): one shape in all three SDKs (D1).
  Rust group+guard live; TS group live, TS guard lands in A4; Py
  group/guard land in A5. .end() closes a scope in all three.
- (f) Python host-mode .context() raises EConfig.
- TS/Py duplicate-mount and missing-prerequisite checks are pinned at
  seal() in all three SDKs; host-only verbs throw at the call site.
- TS lockBroker(config) and Py lock_broker(enabled=True) are standalone
  host-only verbs; TS method renamed from lock_broker to lockBroker.
- TS recordHostOnly/hostOnlyCalls deleted as dead (zero callers since
  host-only verbs throw at the call site).
- Open drift, fixed 2026-11-12 (A1 no-gap round): the Rust artifact's
  scope structs owned the hook vecs; they now own the builder and close
  via .end(), and its scope-entry closures return the builder. The
  Python artifact recorded host-only verbs (_record_host_only); it now
  raises at the call site (_raise_host_only), and its seal() no longer
  raises on plugin-mode builders. The TS artifact now declares
  guard(), forkState(), and the plugin() alias, and pins seal-time
  duplicate/missing-prerequisite checks plus .seal() before .boot()
  in examples. vectors.json stays 58; the 4 chain cases are pinned in
  plugin-builder-vectors.rs pending generator work (62 total).
- Closed at convergence: live Py trap(filter, pre, post) no longer
  raises host-only on a plugin-mode builder (kelir_builder.py trap);
  the filter hook is now visible to hook_descriptors() via
  _build_descriptors, so it survives .use() and .fuse() instead of
  being silently dropped. The Py artifact carries _check_mounted
  with _mounted_by_host, mirroring live.
- D5 (j) filled at convergence in cans/02#Chain-decisions with the
  named L3 blockers: sp_15_host_inode_lock_broker_flock_bridge for
  lock-broker globs/nonBlocking, and the plugin_engine code_only
  domain group for CompositeBundle unpack (no dedicated case id
  exists in the register; none invented).

## No-gap A1 pin (2026-11-12)

Decisions D1-D5, pinned in cans/02-plugin-model.md#Chain-decisions,
cans/03-sdk-architecture.md#Unified-builder-surface, and
cans/artifacts/config/builder-verbs.yaml (58 verbs, 8 scopes, 0 gaps):

- D1 group(fn) / guard(enabled, fn): one shape in all three SDKs.
  group closure takes a sub-builder and returns it; guard is
  boolean-conditional only (Rust's shape). Exact signatures per SDK
  in the pin; TS guard and Py group/guard land in A4/A5.
- D2 fork: plugin-mode only. Rust by type (fork always yields a
  plugin-mode builder); TS throws EConfig on host mode (A4 lands);
  Py raises EConfig (live). Hosts fork via Session.fork().
- D3 duplicate-mount / missing-prerequisite checks run at seal() in
  all three SDKs. Host-only verbs stay fail-fast at the call site
  (Rust compile error; TS/Py EConfig).
- D4 instance .plugin(builder) mounts (delegates to .use()); the
  class/factory .plugin(name) creates. A string argument to instance
  .plugin() raises a clear EConfig (Rust: compile error).
- D5 lock-broker is a standalone host-only verb in all three SDKs
  (Rust full LockBrokerConfig; TS/Py boolean only at the bridge,
  globs/non_blocking forwarding is L3). CompositeBundle has no mount
  verb; the type stays for the profile facade's unpack path. Named
  L3 blockers: see (j), filled at convergence; none invented.
- Vectors are 62 live; the A3 generator landed the 4 chain cases.
