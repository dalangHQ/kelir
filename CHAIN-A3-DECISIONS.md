# CHAIN-A3 decisions (Rust track, L1/L2)

Record of decisions taken while fixing the Rust builder chain on branch
`chain-a3`. Spec pins live in `cans/` and are A1's job; nothing here edits
`cans/`. Live code behaviour that contradicts an artifact is kept as-is
(item 7 below) and flagged for A1.

## 1. `plugin()` returns `Self`, duplicate check moved to `seal()`

`plugin()` no longer returns `Result`. The duplicate-name check now runs
in `seal()` with the same `EConfig` / `plugin already mounted: <name>`
diagnostic, alongside the existing mounted vsock/scope checks.

## 2. Host `trap` / `trap_async` / `post_exec` return `Self`

Empty-filter validation is deferred to `seal()`. Each filter-form verb
records its `TrapFilter` in `HookRegistry.trap_filters`; `seal()` validates
the host registry's filters and every mounted plugin's filters, using the
mounted plugin's name in the diagnostic. Same rules as before (empty
binary, empty arg); nothing else changed.

## 3. `LifecycleMut` chains and exits; `lifecycle()` closure form

- `register_veto` now takes `self` and returns `Self`, so vetoes chain.
- `LifecycleMut` now holds `&mut KelirBuilder<Host, Unsealed, ...>` (it
  previously held only `&mut VetoList`) and gains `end(self) -> &mut
  KelirBuilder<...>`, so the caller gets the builder back.
- The `lifecycle(|l| ...)` closure form already existed (audit claim
  overstated: it was present, taking `LifecycleScope`); it was missing
  veto registration, so `LifecycleScope::register_veto` was added and the
  closure form now requires `.end()`, like the other scopes.

## 4. All 7 scopes gain `.end(self) -> KelirBuilder<...>`

Trap, Fs, Net, Console, Exec, Host, Lifecycle, Telemetry scopes all have
`end()`. The scope-entry closures (`trap_scope`, `fs`, `net`, `console`,
`exec`, `host`, `lifecycle`, `telemetry`) now require the closure to
return the builder, i.e. the closure must end with `.end()`; a scope can
no longer be left dangling. Call sites updated accordingly.

## 5. `trap` name collision: both modes get both forms

Decision: rename, don't overload. Rust has no overloading, so the two
same-name signatures could not coexist.

- Filter form `trap(filter, f) -> Self` is now defined once, generically
  over `Mode`, so Host and Plugin builders both have it.
- Scope form is renamed to `trap_scope(closure) -> Self`, also generic
  over `Mode`, so Host and Plugin builders both have it.
- The old Plugin-only `trap(closure)` is gone; call sites use
  `trap_scope`.
- `trap_async` / `post_exec` stay Host-only: they are host-ring verbs and
  the live code (compile error in Plugin mode) is kept per item 7.

## 6. `.use()`, `group()`, `guard()`, `CompositeBundle`

- `.use()` is an alias for `.plugin()`, spelled `r#use` (`use` is a
  keyword in Rust). Same signature, same deferred duplicate check.
- `group(f)` applies a `FnOnce(Self) -> Self` to the builder: a named
  block of chain steps. Cheap, added.
- `guard(enabled, f)` applies `f` only when `enabled` is true, else the
  builder passes through unchanged. Semantics chosen by this track: a
  boolean conditional group (Elysia's `.guard()` scopes hooks; there is
  no route table here to scope over, so conditional application is the
  honest equivalent).
- `CompositeBundle`: NO mount verb added, type NOT deleted. Its
  `CompositePlugin::unpack` is an L3 stub returning an empty vec, so a
  mount verb now would silently mount nothing. Mounting a
  `CompositeBundle` is unsupported until L3 fills `unpack`; the type
  stays as the declared shape for that work.

## 7. SSOT discrepancy left alone

The artifact/code mismatch on host-only verbs is A1's job. No code was
changed to match an artifact; Plugin-mode use of Host-only verbs
(`trap_async`, `post_exec`) still fails to compile.

## 8. `boot()`: `seal()?.boot()` is the only route

Decision: delete, don't document-around. `boot()` on
`KelirBuilder<Host, Unsealed, ...>` is deleted; `boot()` exists only on
`KelirBuilder<Host, Sealed, ...>`. Validation lives in `seal()` and
`boot()` is the single terminal. All call sites (library
`sandbox_runtime.rs`, all `crates/kelir/tests/` boot sites) now go
through `.seal()?.boot()`.

## Open gaps (deliberate, not faked)

- `CompositeBundle` mounting: unsupported until L3 fills `unpack` (see 6).
- No other open items: `group()`/`guard()` landed (see 6); every scope
  has `end()`; both `trap` forms exist in both modes.
