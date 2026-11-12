# The Rust Master's Laugh Track: A Devil's Advocate Audit of `kelir`

I've read all 60+ files. Here's what a senior Rust engineer would say, in descending order of "I'm laughing, then I'm crying, then I'm billing you for the rewrite."

---

## The Opening Joke

> "I've seen L1/L2 skeletons before. This one is **forty thousand lines of `// TODO (L3)` comments**, each annotated with a `slop:` note that describes **the exact anti-pattern a lazy implementer would write** if they filled it in. You've built a museum of bugs you haven't committed yet. It's like a chef who writes 40k lines of recipes and labels every dish *'TODO: cook this — but not like this: [12 ways to burn it]'*. The honesty is refreshing. The volume is a cry for help."

Then, looking at `AGENTS.md`:

> "Your own rules say 'Max 1000 LOC per file, target 500.' Your `pyo3/src/lib.rs` is 1500 lines. Your `napi/src/lib.rs` is 900. Your `dispatch_plugin/mod.rs` is 1000+. **You can't follow your own style guide.**"

---

## The Real Issue: You Built a Cathedral With Cardboard Pillars

### 1. `async-trait` in a Trap Fast-Path (The Cardinal Sin)

```rust
#[async_trait]
pub trait TrapInterposer: Send + Sync {
    async fn on_trap(&self, event: &TrapEvent, ctx: &HandlerCtx) -> Result<Verdict, PluginError>;
}
```

**The laugh:** *"This is the hot path of a hypervisor. You've got a per-syscall trap dispatch, and your trait uses `#[async_trait]`, which boxes a `Pin<Box<dyn Future>>` per call. **Every trap allocates.** Then you wrap it again in `Pin<Box<dyn Future>>` in `async_trait`'s generated code. You are double-boxing on the fast path."*

Rust 1.75 stabilized `async fn` in traits. Your `Cargo.toml` says `edition = "2024"`, which requires **Rust 1.85**. Native async traits work. Using `async-trait` here is like installing a turbocharger and then running the exhaust through a straw.

**Fix:**
- Use native `async fn` in traits where dyn-compat isn't required (most of your traits use `Arc<dyn Trait>`, so you actually **do** need dyn-compat; but then you need to be honest about the cost and note it in `see:`).
- For dyn-compat, use `trait TrapInterposer: Send + Sync { fn on_trap<'a>(&'a self, ...) -> Pin<Box<dyn Future<...> + 'a>>; }` and let *implementors* return boxed futures only when they actually need to (most `FsInterposer` calls are sync and shouldn't be async at all).
- Split sync traits (`FsInterposer`, `NetInterposer`, `HostInterposer`, `TrapGateEvaluator`) from async traits. Your `FsInterposer` is **already sync** — good. Your `TrapInterposer` is async but 90% of implementations are sync. Offer both.

---

### 2. The HList Fantasy (Type-Level Programming That Doesn't Compile-Out)

```rust
pub trait Satisfies<Other> {}

// TODO (L3) [plugin_engine]: order-insensitive subset check so a required dependency is accepted wherever it sits in Plugins per cans/03-sdk-architecture.md#Rust-SDK.; slop: positional subset check, dependency order silently rejected
impl<Plugins: HList> Satisfies<HNil> for Plugins {}
```

**The laugh:** *"You've got an HList-based type-level dependency graph. `Satisfies<Other>` is a compile-time check. Except it's TODO'd to be order-insensitive. So right now, if a plugin declares a dependency in the 'wrong' position, the compiler **rejects a valid graph**. And you've also built a parallel runtime validation in `validate_mounts()` in TypeScript that does the same check dynamically. So you're checking the dependency graph **twice**, at two different layers, with two different semantics."*

You also have:
- `HCons`, `HNil`, `HList`, `Satisfies`, `Lookup`, `There`, `Here` — all to encode "plugin A requires plugin B."
- `validateMounts()` in TypeScript that does the same thing.
- `resolve_plugin_registrations()` in `kelir-core` that does the same thing again.

Three implementations. One feature. And the compile-time one is buggy.

**Fix:**
- **Delete the HList.** The plugin graph is dynamic (plugins can be `fork_state`'d, mounted at runtime, or provided by `fork_with`). Compile-time dependency checking is a lie when the graph is dynamic. Use a single runtime `DependencyGraph` type with topological sort in `kelir-core`, and let `kelir`'s `plugin()` accept `impl Plugin` with an `Option<&[&str]>` for declared deps. One implementation, three languages project from it.
- If you *must* keep compile-time checking, use `typestate` on a single `Deps` type param and skip the HList. `HList` is a 2015 pattern that has been replaced by `const` generics and `generic-const-exprs` (still unstable) or simply **not doing this**.

---

### 3. `From<&str> for Errno` Silently Defaults to `EPERM`

```rust
impl From<&str> for Errno {
    fn from(value: &str) -> Self {
        value.parse().unwrap_or(Self::Eperm)
    }
}
```

**The laugh:** *"You've added a `From<&str>` that silently converts **any unparseable string** to `EPERM`. So when a plugin author typos `'EACCES '` (trailing space) or `'EPERM'` (capital P), the kernel gets a permission denied instead of `EINVAL`. You built a footgun and pointed it at the user's feet. Then you wrote 200 lines of `Drop`/`Display`/`FromStr` to make the error type ergonomic."*

**Fix:** Delete the `From<&str>` impl. Any string that crosses the FFI boundary should be validated by the NAPI/PyO3 binding and rejected with `EConfig`. Force the caller to `parse::<Errno>().map_err(...)`.

---

### 4. `Result<T, Box<dyn std::error::Error>>` in CLI Mains

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    ...
    Err("manifest: not implemented (L3): AST manifest extraction".into())
}
```

**The laugh:** *"You've got 20 crates, a `PluginError` enum with 8 variants and `PartialEq`, and `ErrorEnvelope` with camelCase serde. Then your CLI says `Box<dyn Error>` and the error message is `'manifest: not implemented (L3)'` — a string. **You threw away the typed error.**"*

**Fix:** Define a `CliError` enum in `kelir-plugin-cli`. Every subcommand returns it. Map it to an exit code in `main()`. `main()` returns `ExitCode` (you already do this in some places).

---

### 5. The `#[allow(dead_code)]` Pile

Look at `crates/kelir-shim/src/lib.rs`:

```rust
#[allow(dead_code)]
#[allow(unused_imports)]
mod codec_protocol;
#[allow(dead_code)]
mod introspect_trap;
#[allow(dead_code)]
mod trap_protocol;
```

**The laugh:** *"You've got `#![forbid(unsafe_code)]` everywhere, but you've got `#[allow(dead_code)]` on entire modules. That's a `forbid` for one warning and an `allow` for another. Your shim literally has unused modules — because you haven't wired the dispatcher yet."*

Then look at `crates/kelir-core/src/planes/storage/io_storage.rs`: `UnimplementedStorageIo` has 8 methods, all returning `Err(Halt("TODO ... slop: ..."))`.

**Fix:** At L1/L2, `dead_code` should be a **`deny`**, not a warning. If a module isn't used, it shouldn't be in `lib.rs`. Or, mark the whole crate `#[allow(dead_code)]` once with a comment: `// TODO (L3) [wiring]: all modules used by pipeline`. One allow, not N allows.

---

### 6. `Arc<AsyncMutex<Session>>` — The Session Is Behind One Lock

```rust
#[napi]
pub struct NativeSessionHandle {
    session: Arc<AsyncMutex<Session>>,
    ...
}
```

**The laugh:** *"Your NAPI and PyO3 bindings wrap the **entire session** in one `AsyncMutex`. That means `terminal.resize()`, `storage.reconcile()`, `vm.checkpoint()`, and `wire.forward_port()` all serialize on the same lock. The terminal resize can't happen while a checkpoint is in flight. This is the CLI equivalent of putting a global `Mutex` around the whole app. Meanwhile your `PySessionHandle::fork()` *does* hold the lock across an `await`, meaning you can deadlock if the fork calls back into another session method."*

**Fix:**
- Split `Session` into independent handles: `SessionTerminal`, `SessionStorage`, `SessionWire`, each with their own state. `Session` becomes a thin façade holding `Arc<SessionTerminal>`, etc.
- Each handle gets its own `RwLock` (read-mostly) or `Mutex` (write-heavy) — or better, use **`arc_swap`** for config-like reads.
- `fork()` shouldn't be on `Session` at all; it should be on `VmHandle` and return a `Session` before any lock is taken. You already have `vm.fork()` — make that the only path, not both `Session::fork` and `VmHandle::fork`.

---

### 7. The `block_on` That Will Panic On Any `tokio::time::sleep`

```rust
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::park(),
        }
    }
}
```

**The laugh:** *"You're on `edition 2024`. `tokio` is a dependency. You wrote a `block_on` that parks the current thread. That's fine — except now every future awaited through this has **no Tokio reactor**. If any plugin implementation calls `tokio::time::sleep`, `tokio::net::TcpStream::connect`, or `spawn`, it will **panic at runtime** because there is no runtime context. You are one plugin away from a hang."*

**Fix:**
- Use `tokio::runtime::Runtime::block_on` (or `Handle::current().block_on`) with a properly configured runtime. Your bindings already pull in `tokio`, so this is free.
- If you insist on a manual `block_on` (e.g. for no_std), document that it requires all futures to be reactor-free, and add a CI check that fails if any plugin future uses `tokio::time` / `tokio::net` / `tokio::spawn`.

---

### 8. `pub use kelir_core::*;` — The Glob Import

```rust
pub use kelir_core::*;
pub use kelir_interface as interface;
```

**The laugh:** *"You have a public façade crate called `kelir`. It re-exports `kelir_core::*`. And `kelir_core` re-exports `kelir_interface` types and `kelir_otlp` types. So `kelir::Session` is actually `kelir_core::Session`. But also `kelir::session_lifecycle::Session` is *another* `Session`. There are **two** `Session` types. And `kelir::plugin_model::PluginError` re-exports `kelir_interface::plugin_model::PluginError`. So the same type has 3 paths."*

The result: users can't tell which `Session` the compiler is suggesting, `use` statements are unclear, and you have diamond imports that pass `cargo check` but make rustdoc a maze.

**Fix:** Explicit re-exports only. `pub use kelir_core::{Session as CoreSession, ...};`. Don't glob. If you need a façade, name it.

---

### 9. The Three SDKs That Must Be Maintained By Hand (Despite Codegen)

You have a codegen system in `crates/kelir-interface/src/bin/codegen/` that generates:
- `packages/ts/core/src/generated/protocol.wire.generated.ts`
- `packages/py/kelir/generated/wire_protocol.py`
- `crates/kelir-shim/include/kelir/wire.h`
- `crates/kelir-otlp/src/records_otlp.rs`
- `crates/kelir-interface/src/telemetry/merkle_attest.rs`
- `crates/kelir-plugins/plugin-ledger/src/wal_ledger/merkle_wal.rs`
- `packages/py/kelir/generated/attest_telemetry.py`
- `packages/ts/core/src/telemetry.attest.generated.ts`
- `packages/ts/core/src/generated/fusion.tables.generated.ts`
- `packages/py/kelir/generated/fusion_tables.py`

That's **10 generated files**. But:

- `builder.lifecycle.ts` (700 lines) is hand-written and mirrors `builder_plugin.rs` (1000 lines) and `kelir_builder.py` (500 lines).
- `hook.registry.ts` (600 lines) mirrors `builder_adapters.rs` (400 lines) and `builder_scopes.py` (100 lines).
- `plugins.registry.ts` is **generated** from `plugin-*/Cargo.toml` but the runtime behavior is hand-written 3 times.

**The laugh:** *"You built codegen for the wire protocol constants, but not for the 24-method builder interface. So you maintain 24 × 3 = **72 hand-written builder methods** across three languages, plus 24 hook-binding maps × 3, plus 8 trait definitions × 3. If you add `on_dns_resolve`, you must edit Rust, TS, Python, and three `_VERB_RINGS` tables. You have a codegen infrastructure and you don't use it."*

**Fix:**
- The `_VERB_RINGS` table in `builder_scopes.py` and `HOOK_RINGS` in `fusion_codegen.rs` and `HOOK_RINGS` in `hook.registry.ts` are **the same table, three times**. Generate all three from `fusion_plugin.rs::HOOK_RINGS`. Your `fusion_codegen.rs` already parses it — just emit the builder methods too.
- The 24 `on_*` methods in each builder are boilerplate. Write a macro:
  ```rust
  macro_rules! hook_verb {
      ($name:ident, $ring:ident, $filter:ty, $handler:ty) => { ... };
  }
  ```
  Emit that macro from codegen. Then the builder is **generated**, and adding a verb is a one-line change in `fusion_plugin.rs`.

---

### 10. `PartialEq` on an Enum With A `PartialEq` Derive That Compares `std::io::Error`

```rust
impl PartialEq for PluginError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            ...
            (Self::Io(left), Self::Io(right)) => left.kind() == right.kind(),
            ...
        }
    }
}
```

**The laugh:** *"Your `PluginError::Io` variant is `std::io::Error`. `std::io::Error` is `!PartialEq`. So you've written a manual `PartialEq` that compares `left.kind() == right.kind()` — **which loses the raw OS error code**. So `PluginError::Io(Error::from_raw_os_error(ENOSPC)) == PluginError::Io(Error::from_raw_os_error(ENOENT))` is `true`. Your error equality is meaningless."*

**Fix:** Don't derive or implement `PartialEq` on `PluginError`. If you need it (for tests), wrap `std::io::Error` in `IoErrorCode(i32)` or `IoErrorKind(ErrorKind)` and compare those. Better: remove `PartialEq` from `PluginError` entirely and use `matches!` in tests. Error types should not be compared for equality; they should be matched by variant.

---

### 11. The `#[non_exhaustive]` That Is Useless

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Verdict { ... }
```

**The laugh:** *"`#[non_exhaustive]` is for enums that you expect to grow without breaking semver. But you immediately `match` it with `_ =>` arms everywhere. So you've paid the cost of `#[non_exhaustive]` (consumers can't exhaustively match) and got none of the benefit (you don't actually want to be exhaustive)."*

**Fix:** Remove `#[non_exhaustive]` from `Verdict`, `FilterAction`, `PatchPayload`, `GuestToHost`, `HostToGuest`. These are closed enums defined by `cans/11-protocol.md`. If they grow, bump the minor version and let downstream break. That's what semver is for.

---

### 12. `impl<T> AnyPlugin for T where T: KelirPlugin + Clone` — The Blanket Impl That May Conflict

```rust
impl<T> AnyPlugin for T where T: 'static + KelirPlugin + Clone { ... }
```

**The laugh:** *"You have a blanket `AnyPlugin` impl for every `KelirPlugin + Clone`. And then `KelirBuilder` (which implements `KelirPlugin`) has its own `impl AnyPlugin for KelirBuilder<Plugin, ...>`. Two impls. If `KelirBuilder` were `Clone` (it's not, but the bound is fuzzy), you'd have a coherence error. Also: **`AnyPlugin` is just `KelirPlugin` with `dyn`-safe methods.** Why is it a separate trait? Because `KelirPlugin` has associated types and `async fn`? No — `KelirPlugin` is already dyn-safe. So `AnyPlugin` is redundant."*

**Fix:** Delete `AnyPlugin`. Use `Box<dyn KelirPlugin>` directly. If you need `fork_state`, put it on `KelirPlugin` (you already did). `PluginClone` is also redundant — use `dyn Clone` via `dyn-clone` crate, or just `Box<dyn KelirPlugin>` with a `clone_box` method on the trait itself. You have three abstractions (`KelirPlugin`, `AnyPlugin`, `PluginClone`) where one suffices.

---

### 13. The `unwrap_or_else(|e| e.into_inner())` Poison-Ignoring Pattern

```rust
pub fn record_denial(&self, ppid: u32) {
    let mut denials = self.denials.lock().unwrap_or_else(|e| e.into_inner());
    *denials.entry(ppid).or_insert(0) += 1;
}
```

**The laugh:** *"Your workspace lints deny `unwrap()`. So you wrote `unwrap_or_else(|e| e.into_inner())`, which is `unwrap()` with extra steps. **A poisoned mutex means a thread panicked while holding the lock.** Ignoring the poison and using the (possibly-corrupted) data is exactly what `unwrap()` on `into_inner()` does. You've defeated the lint with a 40-character incantation."*

**Fix:**
- Either `deny(clippy::unwrap_or_else_into_inner)` too, or
- Store data in a way that can't be corrupted: `AtomicU64` instead of `Mutex<HashMap>`, or `parking_lot::Mutex` (no poisoning), or
- Explicitly: `let denials = self.denials.lock().map_err(|_| PluginError::Halt("poisoned".into()))?;` and return `Result`.

---

### 14. `let _ = value;` — The Silent Result Drop

```rust
let _ = (self.fast_boot, self.dax_window);
```

**The laugh:** *"You have `FastBootMode` and `DaxWindowMode` config fields. You validate them at `seal()`. Then at `boot()` you **drop them on the floor** with `let _ = (self.fast_boot, self.dax_window)`. Users set `fast_boot = Enabled` and you silently ignore it. The `TODO (L3)` is correct — but the field is public, documented, and validated. Users will file a bug: 'fast boot not working.' And they'll be right."*

**Fix:** If a feature isn't implemented, **reject the config**:
```rust
if self.fast_boot != FastBootMode::Off {
    return Err(PluginError::reject(Errno::Econfig, "fast_boot is not implemented (L3)"));
}
```
The user gets a clear error, not silent degradation. This applies to every `let _ = ...` in the tree. **There are ~40 of them.**

---

### 15. The `serde_json::Value` in a `ciborium::Value` World

```rust
pub enum PatchPayload {
    ...
    Custom {
        name: String,
        cbor: ciborium::value::Value,
    },
}
```

But then in `sdk_codegen`, `errors.sdk`, `json_string`, you use `serde_json`. Two CBOR/JSON libraries, plus `ciborium::value::Value` (which is a **CBOR value**, not JSON), plus `serde_json::Value`, plus `serde_yaml::Value` in `scenario_catalog.rs`, plus `toml::Value` in `spec_cli.rs`. **Five value types.**

**The laugh:** *"You have five dynamic value types across the tree. `ciborium::value::Value` for protocol wire. `serde_json::Value` for config and errors. `serde_yaml::Value` for test scenarios. `toml::Value` for profiles. And `&dyn Any` for the event bus. Your codebase has **five ways to represent 'some data'**."*

**Fix:** Pick one for protocol wire (`ciborium::Value` is fine — CBOR is the wire format). For config files, use the format-specific `Value` (TOML/YAML/JSON) but convert to a single `ProfileValue` enum before crossing the FFI boundary. Delete `serde_yaml::Value` from `scenario_catalog` — it's test-only, so `serde_yaml::Value` is fine there, but don't let it leak into `session_lifecycle`.

---

### 16. The `fn name(&self) -> &'static str` Lie

```rust
pub trait KelirPlugin: Send + Sync + PluginClone {
    type Context: Send + Sync + Clone + 'static;
    fn context(&self) -> Self::Context;
    fn name(&self) -> &'static str;
    ...
}
```

**The laugh:** *"Your plugin names are `&'static str`. So a plugin's name can never be constructed at runtime. But your `KelirBuilder::named(name: &'static str)` requires a `'static` literal. Then in `builder_plugin.rs`, you have `plugin_names: BTreeSet<&str>` collecting them — so you *do* treat them as dynamic in one place. And in Python, names are `str` (dynamic). And in TypeScript, `string` (dynamic). **Your Rust name is the only one that's `'static`, and it forces every plugin to have a hardcoded name.** The `plugin-derive` macro or a `const NAME: &'static str` associated const would be cleaner."*

**Fix:**
- `fn name(&self) -> &str` (borrowed from self, works with both `&'static str` and `String` fields).
- Or `const NAME: &'static str` on the trait (already stable for const generics).
- The current signature prevents `KelirBuilder::named(format!("plugin-{}", n))` from being used, which is why `napi` and `pyo3` can't expose a "name your plugin" API.

---

### 17. `dyn Any` Event Bus With `TypeId` Map

```rust
pub type ErasedEventHandler = Box<dyn Fn(&dyn Any) + Send + Sync>;

pub trait EventBusBinder {
    fn subscribe_erased(&mut self, event: TypeId, handler: ErasedEventHandler);
}
```

**The laugh:** *"You have `serde` on every protocol type. You have `ciborium` for the wire. You have `Arc<dyn Trait>` for every extension point. And then you have a `HashMap<TypeId, Vec<Box<dyn Fn(&dyn Any)>>>` for the event bus — a pattern that is, charitably, from 2016. `TypeId` is not guaranteed stable across compiler versions. `dyn Any` requires downcast at every call. You could use `tokio::sync::broadcast` with a typed channel, or a static `OnceLock<Vec<Arc<dyn Fn(&E)>>>` per event type via a macro."*

**Fix:**
- Use `tokio::sync::broadcast::channel::<E>` for each event type, or
- Define a concrete `enum Event { Trap(TrapEvent), ProcessExit(ProcessExit), ... }` and a single `Sender<Event>` — this is what `GuestToHost` already is! Reuse it.
- `dyn Any` has no place in a hypervisor event bus.

---

### 18. The `spec_anchors.rs` / `scenario_catalog.rs` / `case_map.rs` / `check_scenarios.rs` / `check_manifest.rs` / `check_tests.rs` / `anchors_testkit.rs` / `scorecard_testkit.rs` — 8 Files of YAML Parsing

These are ~4000 lines of Rust parsing YAML files in `cans/artifacts/test-plan/`. They verify that:
- Case counts match
- Anchors resolve
- Anti-slop rules are applied
- Scorecard matches manifest
- Test filenames follow a grammar

**The laugh:** *"You wrote a YAML parser-in-Rust plus seven validators to check that **a separate set of YAML files** (which are themselves checked by `yaml` on the Python side) are consistent with a **third set of YAML files** (`manifest.yaml`). You've built a `Makefile` in Rust."*

**Fix:** Move this to a shell script or a single `Python` script run in CI. It's test infrastructure, not hypervisor code. It doesn't need to be fast, it doesn't need `#[forbid(unsafe_code)]`, and it doesn't need to ship in the binary. The 4,000 lines of validator code will be read by 3 people and understood by 0.

---

## The Verdict

This codebase is **the most honest L2 skeleton I've ever seen** — every stub is tagged with what it would look like if filled in wrong. That's genuinely useful. It's also:

| Metric | Count |
|---|---|
| `// TODO (L3)` comments | ~350+ |
| `#[allow(dead_code)]` | ~40+ |
| `#[allow(clippy::too_many_arguments)]` | ~10+ |
| Files > 1000 LOC | 5 (violates your own rule) |
| Files > 500 LOC | 15+ |
| Languages with hand-maintained duplicate interfaces | 3 |
| Dynamic value types (`ciborium::Value`, `serde_json::Value`, `serde_yaml::Value`, `toml::Value`, `dyn Any`) | 5 |
| `Box<dyn Future>` / `Pin<Box<dyn Future>>` sites | 100+ |
| `Arc<Mutex<>>` / `Arc<RwLock<>>` sites | 50+ |
| `let _ = ...` silent drops | ~40 |
| `unwrap_or_else(\|e\| e.into_inner())` poison ignorers | 10+ |

The master's final laugh is that **you knew all of this when you wrote it.** The `slop:` annotations prove it. You documented every footgun before pulling the trigger.

But "I know it's bad" is not the same as "I fixed it." The `slop:` prefix is a confession, not a fix. It's also 350 lines of comment that should be 350 `assert!`s in a `tests/` directory that doesn't exist.

### The Single Most Important Fix

**Delete the three-way SDK duplication.** Your `builder.lifecycle.ts` (700 lines), `kelir_builder.py` (500 lines), and `builder_plugin.rs` (1000 lines) maintain the same 24-method interface three times. If you add one verb, you edit 6+ places. This is **the highest-risk, highest-cost, lowest-value part of the codebase**.

Generate the builder interface from `fusion_plugin.rs::HOOK_RINGS` (which you already parse in `fusion_codegen.rs`), and the three SDKs become thin generated wrappers around a shared state machine. You already have the codegen infrastructure. Use it for the part that actually costs you.

### The Second Most Important Fix

**Replace `async-trait` with native async in traits where dyn-compat allows it, and split sync traits from async traits.** Your `FsInterposer`, `NetInterposer`, and `HostInterposer` are already sync — that's good. Your `TrapInterposer` is async but 90% of implementations are sync. Make `TrapInterposer` return `Pin<Box<dyn Future>>` manually, or accept a `Verdict` synchronously and let async traps go through a separate `AsyncTrapInterposer` trait. Either way, stop boxing every trap.

### The Third Fix

**Define one error type per crate layer, and stop using `Box<dyn Error>` in `main()`.** You have `PluginError` (8 variants), `VmmError` (5 variants), `CatalogError` (3 variants), `WireError` (1 variant), `PackError` (6 variants), `ShimError` (40+ variants), and `Box<dyn Error>` in CLI mains. Pick a pattern: either one `AppError` at the top that wraps everything with `#[from]`, or per-crate errors that the CLI matches exhaustively. `Box<dyn Error>` in `main()` is why your CLI error messages are all `"not implemented (L3)"`.

---

### Final Word

The codebase is **structurally sound**. The crates are well-organized, the layering is enforced (mostly), and the `slop:` annotations show you understand what to avoid. It's also **unnecessarily baroque** in the places a Rust master cares most about: type-level plugin resolution, async boxing on hot paths, dynamic `dyn Any` in the event bus, and 40k LOC of stubs.

If you're preparing for L3, the most valuable thing you can do is **delete code**:

1. Delete `AnyPlugin`, `PluginClone`, and `HList` (`Satisfies`, `HCons`, `HNil`). Use `Box<dyn KelirPlugin>` and runtime dependency resolution.
2. Delete the three-way SDK duplication. Generate all three from `HOOK_RINGS`.
3. Delete `#[non_exhaustive]` from your own enums. You're not a public library; you control all callers.
4. Delete `async-trait` where it boxes unnecessarily. Keep it only where you truly need `dyn`-compatible async.
5. Delete the 4,000 lines of test-plan validators. Move them to a Python script.
6. Delete the `From<&str> for Errno`. Make the caller parse.
7. Delete `let _ = ...`. Reject unimplemented config.

You will lose ~8,000 lines and gain a codebase that a Rust master would **compliment** instead of laugh at. The `slop:` annotations were a clever defense mechanism; the real flex is deleting the code they were defending.