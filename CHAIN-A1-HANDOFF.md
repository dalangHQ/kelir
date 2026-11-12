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
- use()/plugin(): move duplicate-mount, missing-prerequisite, and
  host-only-verb checks from mount time to seal(). Live code still
  throws at mount; the spec pins seal-time.
- TS examples (policy-matrix.ts, hook-matrix.ts,
  autonomous-swe-agent.ts, serverless-webhook.ts) boot directly today
  because seal() does not exist live; add `.seal()` before `.boot()`
  there once seal() lands.
- forkState() host-mode divergence, recorded not unified: live TS
  returns a clone, live Py raises EConfig. Pin says the factory is
  plugin-mode only; hosts fork via Session.fork().
- TS instance .plugin() alias is kept in the pin (live, delegates to
  use()). If A4 deletes it like A5 deletes the Py one, the pin follows;
  tell the coordinator.
- No standalone lockBroker verb: the lock broker stays the storage
  config lockBroker flag.

## A5 (Python) — kelir_builder.py

- Add seal() -> Self raising EConfig; any verb after seal() raises
  EConfig; boot() seals implicitly when not sealed yet.
- fuse(): make internal (not a chain verb, not a terminal).
- use(): move duplicate-mount, missing-prerequisite, and host-only-verb
  checks from mount time to seal(). Live code still raises at mount.
- Delete the instance .plugin() mount alias (`plugin =
  _PluginMount()`); mount is use() only. KelirBuilder.plugin(name)
  stays as the plugin-mode factory. A1 already re-chained every Py
  example to .use() (policy-matrix.py:118-119, hook-matrix.py:142,
  plus autonomous-swe-agent.py, db-backed-runtime.py, container.py).
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
  (schema kelir-builder-verbs/v1; 2 entries, 54 verbs, 8 scopes,
  3 open gaps). The gate should read this file, not the prose pin.
- Open gaps recorded there as open-gap, not as verbs: group, guard,
  end. Scopes are closures; the closure return closes the scope.
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
- Rust artifact cans/artifacts/code/rust-plugin-builder.rs is 1320
  lines (1299 at base, pre-existing over the 1000 LOC rule); A1 added
  the trap_async/post_exec pins and seal checks.

## Convergence (coordinator, 2026-11-12)

Reconciled against the landed code; the pin above was corrected where it
contradicted the code tracks:
- (b) Rust has no boot() on Unsealed; seal()?.boot() is the only route.
  TS/Py boot() seals implicitly.
- (e) group()/guard()/.end() landed per SDK (Rust group+guard, TS group,
  all .end()); Python group/guard stay open gaps.
- (f) Python host-mode .context() raises EConfig.
- TS/Py duplicate-mount and missing-prerequisite checks run at mount
  time (.use()); host-only verbs throw at the call site.
- TS lockBroker(config) and Py lock_broker(enabled=True) are standalone
  host-only verbs; TS method renamed from lock_broker to lockBroker.
- TS recordHostOnly/hostOnlyCalls deleted as dead (zero callers since
  host-only verbs throw at the call site).
- Open drift, not faked: the Rust artifact's scope structs borrow the
  hook vecs while live builder_scopes.rs scopes own the builder; its
  scope-entry closures return the scope while live closures return the
  builder via .end(). The Python artifact still records host-only verbs
  (_record_host_only) while live code raises at the call site, and its
  seal() raises on plugin-mode builders while live seal() validates.
  vectors.json stays 58; the 4 chain cases are pinned in
  plugin-builder-vectors.rs pending generator work (62 total).
