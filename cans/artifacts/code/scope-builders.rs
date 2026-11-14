// ILLUSTRATIVE, NON-STANDALONE — do not compile this file directly.
// It pins the canonical Rust scope builders for the unified builder;
// the per-ring scope structs live in
// cans/artifacts/code/rust-plugin-builder-scopes.rs and the normative
// contract is cans/02-plugin-model.md#Nested-scopes. This file pins the
// scopes-only surface: every scope gains config / requires / fork_with /
// timeout / priority / rules, and the five orphan-verb scopes
// (actions, streams, vsock, params, deny) that give the host-only
// register verbs a scope home. Scopes are the only way to reach a ring
// verb: a flat chain exists only as a group() fold that delegates to
// these scope methods and produces the same descriptors.
//
// Owned by: cans/02-plugin-model.md#Nested-scopes
// see: cans/02-plugin-model.md#Nested-scopes
// see: cans/02-plugin-model.md#Chain-decisions
// see: cans/artifacts/code/rust-plugin-builder-scopes.rs

use std::path::Path;
use std::future::Future;
use std::time::Duration;

use kelir_interface::driver_interface::{WatchdogEvent, WatchdogVerdict};
use kelir_interface::plugin_model::{
    AnyPlugin, HandlerCtx, LifecycleCtx, PluginError, TrapFilter,
};
use kelir_interface::protocol_wire::{
    FilterAction, Heartbeat, ProcessExit, TrapEvent, Verdict,
};
use kelir_interface::vsock_mesh::{VsockServiceHandler, VsockServiceSpec};

use crate::hlist_sdk::{HCons, HNil};
use crate::planes::storage_plane::{LockBrokerConfig, ReadMaskEngine};
use crate::builder_sdk::{KelirBuilder, ScopeRules, SnoopRule, MaskRule};

// see: cans/02-plugin-model.md#Nested-scopes
// FsScope sub-nodes: the shape every ring scope shares. Each returns the
// scope, so the chain continues inside the closure; .end() (pinned in
// rust-plugin-builder-scopes.rs) returns the builder and is a
// type-system artifact only — the Rust closure must return the builder,
// and .end() is how it does it, not a semantic close.
impl FsScope {
    // see: cans/02-plugin-model.md#Nested-scopes
    // Scope-level config: fs carries lock_broker, carryover_bytes,
    // scan_engine, snoops. The standalone host-only lock_broker verb's
    // config folds into here.
    pub fn config(
        mut self,
        lock_broker: Option<LockBrokerConfig>,
        carryover_bytes: Option<u32>,
        scan_engine: Option<ReadMaskEngine>,
        snoops: Vec<SnoopRule>,
    ) -> Self {
        self.builder = self.builder.fs_config(
            lock_broker,
            carryover_bytes,
            scan_engine,
            snoops,
        );
        self
    }

    // see: cans/02-plugin-model.md#Nested-scopes
    // Scope-level deps: the Rust form is requires::<P>().
    pub fn requires<P>(mut self) -> Self
    where
        P: KelirPlugin,
    {
        self.builder = self.builder.require::<P>();
        self
    }

    // see: cans/02-plugin-model.md#Nested-scopes
    // Scope-level fork policy.
    pub fn fork_with<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> Box<dyn AnyPlugin> + Send + Sync + 'static,
    {
        self.builder = self.builder.fork_with(f);
        self
    }

    // see: cans/02-plugin-model.md#Nested-scopes
    // Per-scope scheduling. The verbs land here; the SEMANTICS (ordering
    // by descending priority then registration order; timeout ceiling per
    // ring) land with Agent 5's #Hook-ordering and #Per-ring-timeouts
    // nodes. TrapFilter predicates are Agent 5's; none land here.
    pub fn timeout(mut self, d: Duration) -> Self {
        self.builder = self.builder.scope_timeout(d);
        self
    }

    pub fn priority(mut self, i: i32) -> Self {
        self.builder = self.builder.scope_priority(i);
        self
    }

    // see: cans/02-plugin-model.md#Nested-scopes
    // Per-rule policy inside a hook: the mechanical replacement for
    // hook-matrix-full.py's IDENTITY_PATH / DENY_MARKER / CARRYOVER
    // constants.
    pub fn rules(mut self, rules: ScopeRules) -> Self {
        self.builder = self.builder.scope_rules(rules);
        self
    }

    // see: cans/02-plugin-model.md#Nested-scopes
    // The ring verbs stay on the scope; the flat chain is a group() fold.
    pub fn on_fs_lookup<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_fs_lookup(f);
        self
    }

    pub fn on_fs_read<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_fs_read(f);
        self
    }

    pub fn on_fs_write<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, &mut [u8], u32, &HandlerCtx) -> Result<FilterAction, PluginError>
            + Send
            + Sync
            + 'static,
    {
        self.builder = self.builder.on_fs_write(f);
        self
    }

    pub fn on_fs_lock<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, i32, i16, u32) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_fs_lock(f);
        self
    }
}

// see: cans/02-plugin-model.md#Nested-scopes
// The five orphan-verb scopes: actions, streams, vsock, params, deny.
// Each gives a formerly flat host-only register verb its own scope; the
// standalone verbs delete. register_vsock_service was already deleted;
// vsock_service now lives only inside .vsock().
pub struct ActionsScope {
    pub(crate) builder: KelirBuilder,
}

impl ActionsScope {
    // see: cans/02-plugin-model.md#Chain-decisions
    // register_custom_action moves here.
    pub fn register<F, Fut>(mut self, name: &str, schema: serde_json::Value, f: F) -> Self
    where
        F: Fn(ciborium::value::Value) -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>>
            + Send
            + 'static,
    {
        self.builder = self.builder.register_custom_action(name, schema, f);
        self
    }

    pub fn end(self) -> KelirBuilder {
        self.builder
    }
}

pub struct StreamsScope {
    pub(crate) builder: KelirBuilder,
}

impl StreamsScope {
    // see: cans/02-plugin-model.md#Chain-decisions
    // register_stream_kind moves here.
    pub fn register<F, Fut>(mut self, kind: u16, schema: serde_json::Value, f: F) -> Self
    where
        F: Fn(Vec<u8>) -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>>
            + Send
            + 'static,
    {
        self.builder = self.builder.register_stream_kind(kind, schema, f);
        self
    }

    pub fn end(self) -> KelirBuilder {
        self.builder
    }
}

pub struct VsockScope {
    pub(crate) builder: KelirBuilder,
}

impl VsockScope {
    // see: cans/02-plugin-model.md#Chain-decisions
    // vsock_service moves here; only the scope form remains.
    pub fn service<H>(mut self, spec: VsockServiceSpec, handler: H) -> Self
    where
        H: VsockServiceHandler + 'static,
    {
        self.builder = self.builder.vsock_service(spec, handler);
        self
    }

    pub fn end(self) -> KelirBuilder {
        self.builder
    }
}

pub struct ParamsScope {
    pub(crate) builder: KelirBuilder,
}

impl ParamsScope {
    // see: cans/02-plugin-model.md#Chain-decisions
    // params moves here; the typed model rides the scope.
    pub fn model<T>(mut self) -> Self
    where
        T: Default + Send + Sync + 'static,
    {
        self.builder = self.builder.params::<T>();
        self
    }

    pub fn end(self) -> KelirBuilder {
        self.builder
    }
}

pub struct DenyScope {
    pub(crate) builder: KelirBuilder,
}

impl DenyScope {
    // see: cans/02-plugin-model.md#Chain-decisions
    // on_trap_deny moves here; host-only stays host-only.
    pub fn on_trap_deny<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(&TrapEvent, &Verdict, &kelir_engine::Session) -> Fut
            + Clone
            + Send
            + Sync
            + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.builder = self.builder.on_trap_deny(f);
        self
    }

    pub fn end(self) -> KelirBuilder {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Nested-scopes
// Builder entry points for the five orphan scopes. Each takes a closure
// receiving the scope and returns the builder; the Rust closure returns
// via .end() (type-system artifact), TS/Py return the scope or void.
impl KelirBuilder {
    pub fn actions<F>(self, configure: F) -> Self
    where
        F: FnOnce(ActionsScope) -> Self,
    {
        configure(ActionsScope { builder: self })
    }

    pub fn streams<F>(self, configure: F) -> Self
    where
        F: FnOnce(StreamsScope) -> Self,
    {
        configure(StreamsScope { builder: self })
    }

    pub fn vsock<F>(self, configure: F) -> Self
    where
        F: FnOnce(VsockScope) -> Self,
    {
        configure(VsockScope { builder: self })
    }

    pub fn params<F>(self, configure: F) -> Self
    where
        F: FnOnce(ParamsScope) -> Self,
    {
        configure(ParamsScope { builder: self })
    }

    pub fn deny<F>(self, configure: F) -> Self
    where
        F: FnOnce(DenyScope) -> Self,
    {
        configure(DenyScope { builder: self })
    }
}

// see: cans/02-plugin-model.md#Nested-scopes
// The negative vector: f.on_trap(...) inside .fs(...) is a compile error
// in Rust — FsScope has no on_trap method. Uncommenting the body must
// fail the build with E0599 (no method named `on_trap` found for
// `FsScope`). The TS/Py siblings pin the same case as a type error
// (typescript-scope-interface.ts, python-scope-protocol.py).
//
// fn negative_fs_on_trap(builder: KelirBuilder) -> KelirBuilder {
//     builder.fs(|f| f.on_trap(|_, _| Ok(Verdict::Pass)).end())
// }

// see: cans/02-plugin-model.md#Nested-scopes
// Scope typing pin, Rust leg: the scope structs are sealed types; the
// only methods on FsScope are the fs ring verbs plus the shared
// config / requires / fork_with / timeout / priority / rules sub-nodes.
// A trap verb inside .fs is a compile error; an fs verb inside .trap is
// a compile error; the scopes never cross.
