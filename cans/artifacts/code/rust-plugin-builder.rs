// ILLUSTRATIVE, NON-STANDALONE — do not compile this file directly.
// It pins the unified builder surface; the live shape is
// crates/kelir/src/builder_sdk.flow.rs + crates/kelir/src/builder_plugin.flow.rs
// and the normative contract is cans/02-plugin-model.md#Unified-builder.
// Scopes are pinned in cans/artifacts/code/rust-plugin-builder-scopes.rs.
//
// Target shape: ONE struct KelirBuilder. KelirBuilder::new(VmConfig) is
// the host; KelirBuilder::named(name) is a plugin-mode builder. The 24
// ring verbs + 8 closure scopes + context / require / scope / vsock /
// fork exist in BOTH modes with the same signatures as the DX-round
// plugin builder (the host gains them; the plugin keeps them). Mount is
// merge by value: host.plugin(builder) appends the plugin's hooks after
// the host's own hooks; host.plugin_instance(trait plugin) keeps the
// live trait path. Validation happens once, in seal(), over the host +
// mounted plugins together. There is no build(): a plugin-mode builder
// is mounted as-is, and a verb after seal is a type error (seal consumes
// self). PluginBuilder / PluginWrapper / with_composite are deleted.
//
// Generic arity: pinned by cans/02-plugin-model.md#Unified-builder (A1).
// This artifact names the type parameters (Ctx, Deps, Plugins, Params,
// SealState) but does not fix their list or order; method signatures
// below return Self where the arity would otherwise be spelled. The
// host-only verbs are not implemented for Plugin mode: calling one on
// a plugin-mode builder is a compile error, not a recorded call seal()
// rejects (live: builder_sdk.flow.rs Host,Unsealed impl only). There is no
// record-and-reject path in Rust; params() is host-only by type (the
// arity pin). Every chain verb returns Self; seal() is the only
// fallible chain step, and config after seal() is a compile error.
//
// Owned by: cans/02-plugin-model.md#Unified-builder
// see: cans/02-plugin-model.md#Unified-builder
// see: cans/03-sdk-architecture.md#Unified-builder-surface

use std::future::Future;
use std::pin::Pin;
use std::marker::PhantomData;
use std::path::Path;
use std::sync::Arc;

use kelir_interface::driver_interface::{
    DaxWindowMode, FastBootMode, VmConfig, WatchdogEvent, WatchdogVerdict,
};
use kelir_interface::plugin_model::{
    AnyPlugin, ConsoleInterposer, FsInterposer, HandlerCtx, HostInterposer, KelirPlugin,
    LifecycleCtx, LifecycleParticipant, MountScope, NetInterposer, PipelineBinder, PluginDeps,
    PluginError, SystemTelemetryParticipant, TrapInterposer,
};
use kelir_interface::errno_types::Errno;
use kelir_interface::protocol_wire::{FilterAction, Heartbeat, ProcessExit, TrapEvent, Verdict};
use kelir_interface::vsock_mesh::{VsockServiceHandler, VsockServiceSpec};

use crate::hlist_sdk::{HCons, HNil, NoParams};
use crate::planes::storage_plane::{LockBrokerConfig, StoragePlaneConfig};
use crate::planes::terminal_plane::PtyConfig;
use crate::planes::wire_plane::WireConfig;

type ForkFn = Arc<dyn Fn(&str) -> Box<dyn AnyPlugin> + Send + Sync>;

struct OnTrap;
struct OnProcessExit;
struct OnFsLookup;
struct OnFsRead;
struct OnFsWrite;
struct OnFsLock;
struct OnNetOutbound;
struct OnNetInbound;
struct OnConsoleInput;
struct OnConsoleOutput;
struct OnExecStdout;
struct OnExecStderr;
struct OnHostLsmExec;
struct OnHostPressure;
struct OnHostKeyringAccess;
struct OnVmInit;
struct OnVmReady;
struct OnVmShutdown;
struct OnPreCommit;
struct OnPostCommit;
struct OnHostPreempt;
struct OnMemoryPressure;
struct OnHeartbeat;
struct OnWatchdog;

// see: cans/02-plugin-model.md#Segregated-ring-traits
pub struct TrapClosureAdapter<F, Verb> {
    handler: F,
    verb: PhantomData<fn() -> Verb>,
}

// see: cans/02-plugin-model.md#Segregated-ring-traits
pub struct FsClosureAdapter<F, Verb> {
    handler: F,
    verb: PhantomData<fn() -> Verb>,
}

// see: cans/02-plugin-model.md#Segregated-ring-traits
pub struct NetClosureAdapter<F, Verb> {
    handler: F,
    verb: PhantomData<fn() -> Verb>,
}

// see: cans/02-plugin-model.md#Segregated-ring-traits
pub struct ConsoleClosureAdapter<F, Verb> {
    handler: F,
    verb: PhantomData<fn() -> Verb>,
}

// see: cans/02-plugin-model.md#Segregated-ring-traits
pub struct HostClosureAdapter<F, Verb> {
    handler: F,
    verb: PhantomData<fn() -> Verb>,
}

// see: cans/02-plugin-model.md#Segregated-ring-traits
pub struct LifecycleClosureAdapter<F, Verb> {
    handler: F,
    verb: PhantomData<fn() -> Verb>,
}

// see: cans/02-plugin-model.md#Segregated-ring-traits
pub struct TelemetryClosureAdapter<F, Verb> {
    handler: F,
    verb: PhantomData<fn() -> Verb>,
}

// see: cans/02-plugin-model.md#Unified-builder
// One struct, two modes. new() is the host (config Some, name "");
// named() is a plugin-mode builder (config None, name set). One
// HookRegistry holds every ring vec for both faces: the host face's
// ring verbs append here, exactly like the plugin face's. Registration
// keeps plane config, mounted plugins, and host-only registrations
// only; the former on_vm_init / on_vm_ready / on_vm_shutdown
// single-handler fields were folded into the lifecycle vec, and the
// on_vm_init / on_vm_ready / on_vm_shutdown methods remain as the ring
// verbs that append to it. Registration, TrapFilter, CustomActionEntry,
// StreamKindServiceEntry, and LifecycleMut are defined in builder_sdk.flow.rs
// alongside the builder; the trap-closure adapter (TrapClosureAdapter) and
// its OnTrap/OnProcessExit verbs live in builder_plugin.flow.rs and register on
// the HookRegistry vec — one trap path, one engine dispatch point.
// VmConfig has no Default live, so the host config
// is Option: Some on the host face, None on the plugin face. Scopes live
// in cans/artifacts/code/rust-plugin-builder-scopes.rs; each scope owns
// the builder and closes via .end().
#[derive(Clone)]
pub struct KelirBuilder {
    config: Option<VmConfig>,
    plugins: Plugins,
    registration: Registration,
    registry: HookRegistry,
    fast_boot: FastBootMode,
    dax_window: DaxWindowMode,
    name: &'static str,
    context_factory: Arc<dyn Fn() -> Ctx + Send + Sync>,
    scope: MountScope,
    fork: Option<ForkFn>,
    registry: HookRegistry,
    seal: PhantomData<fn() -> SealState>,
    deps: PhantomData<fn() -> Deps>,
}

// see: cans/02-plugin-model.md#Unified-builder
struct MountedPlugin {
    name: &'static str,
    scope: MountScope,
    fork: Option<ForkFn>,
    registry: HookRegistry,
}

// see: cans/02-plugin-model.md#Unified-builder
impl KelirBuilder {
    pub fn new(config: VmConfig) -> Self {
        Self::with_config(Some(config), "")
    }

    pub fn named(name: &'static str) -> Self {
        Self::with_config(None, name)
    }

    fn with_config(config: Option<VmConfig>, name: &'static str) -> Self {
        Self {
            config,
            plugins: HNil,
            registration: Registration::default(),
            registry: HookRegistry::default(),
            fast_boot: FastBootMode::Off,
            dax_window: DaxWindowMode::Off,
            name,
            context_factory: Arc::new(|| ()),
            scope: MountScope::default(),
            fork: None,
            seal: PhantomData,
            deps: PhantomData,
        }
    }

    fn rebuild<NextCtx>(
        self,
        context_factory: Arc<dyn Fn() -> NextCtx + Send + Sync>,
    ) -> Self {
        Self {
            config: self.config,
            plugins: self.plugins,
            registration: self.registration,
            fast_boot: self.fast_boot,
            dax_window: self.dax_window,
            name: self.name,
            context_factory,
            scope: self.scope,
            fork: self.fork,
            registry: self.registry,
            seal: PhantomData,
            deps: PhantomData,
        }
    }

    // see: cans/02-plugin-model.md#Unified-builder
    // Type-level: context() swaps Ctx, require() prepends P::Context to
    // Deps; the exact spelling is the spec arity pin (A1).
    pub fn context<NextCtx>(
        self,
        factory: impl Fn() -> NextCtx + Send + Sync + 'static,
    ) -> Self
    where
        NextCtx: Clone + Send + Sync + 'static,
    {
        self.rebuild(Arc::new(factory))
    }

    pub fn require<P>(self) -> Self
    where
        P: KelirPlugin,
    {
        let factory = Arc::clone(&self.context_factory);
        self.rebuild(factory)
    }

    pub fn scope(mut self, scope: MountScope) -> Self {
        self.scope = scope;
        self
    }

    pub fn fork_with(
        mut self,
        fork: impl Fn(&str) -> Box<dyn AnyPlugin> + Send + Sync + 'static,
    ) -> Self {
        self.fork = Some(Arc::new(fork));
        self
    }

    pub fn vsock_service<H>(mut self, spec: VsockServiceSpec, handler: H) -> Self
    where
        H: VsockServiceHandler + 'static,
    {
        self.registry.vsock.push((spec, Arc::new(handler)));
        self
    }

    pub fn on_trap<F>(mut self, f: F) -> Self
    where
        F: Fn(&TrapEvent, &HandlerCtx) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.registry.trap.push(Arc::new(TrapClosureAdapter::<_, OnTrap> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_process_exit<F>(mut self, f: F) -> Self
    where
        F: Fn(&ProcessExit, &HandlerCtx) -> Result<(), PluginError> + Send + Sync + 'static,
    {
        self.registry.trap.push(Arc::new(TrapClosureAdapter::<_, OnProcessExit> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_fs_lookup<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.registry.fs.push(Arc::new(FsClosureAdapter::<_, OnFsLookup> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_fs_read<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync + 'static,
    {
        self.registry.fs.push(Arc::new(FsClosureAdapter::<_, OnFsRead> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_fs_write<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, &mut [u8], u32, &HandlerCtx) -> Result<FilterAction, PluginError>
            + Send
            + Sync
            + 'static,
    {
        self.registry.fs.push(Arc::new(FsClosureAdapter::<_, OnFsWrite> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_fs_lock<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, i32, i16, u32) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.registry.fs.push(Arc::new(FsClosureAdapter::<_, OnFsLock> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_net_outbound<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut [u8]) -> Result<FilterAction, PluginError> + Send + Sync + 'static,
    {
        self.registry.net.push(Arc::new(NetClosureAdapter::<_, OnNetOutbound> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_net_inbound<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut [u8]) -> Result<FilterAction, PluginError> + Send + Sync + 'static,
    {
        self.registry.net.push(Arc::new(NetClosureAdapter::<_, OnNetInbound> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_console_input<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync + 'static,
    {
        self.registry.console.push(Arc::new(ConsoleClosureAdapter::<_, OnConsoleInput> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_console_output<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync + 'static,
    {
        self.registry.console.push(Arc::new(ConsoleClosureAdapter::<_, OnConsoleOutput> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_exec_stdout<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, u64, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync + 'static,
    {
        self.registry.console.push(Arc::new(ConsoleClosureAdapter::<_, OnExecStdout> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_exec_stderr<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, u64, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync + 'static,
    {
        self.registry.console.push(Arc::new(ConsoleClosureAdapter::<_, OnExecStderr> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_host_lsm_exec<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, u32) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.registry.host.push(Arc::new(HostClosureAdapter::<_, OnHostLsmExec> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_host_pressure<F>(mut self, f: F) -> Self
    where
        F: Fn(u32) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.registry.host.push(Arc::new(HostClosureAdapter::<_, OnHostPressure> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_host_keyring_access<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.registry.host.push(Arc::new(HostClosureAdapter::<_, OnHostKeyringAccess> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_vm_init<F>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(
                &'c mut LifecycleCtx<'i>,
            )
                -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + 'c>>
            + Send
            + Sync
            + 'static,
    {
        self.registry.lifecycle.push(Arc::new(LifecycleClosureAdapter::<_, OnVmInit> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_vm_ready<F>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(
                &'c mut LifecycleCtx<'i>,
            )
                -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + 'c>>
            + Send
            + Sync
            + 'static,
    {
        self.registry.lifecycle.push(Arc::new(LifecycleClosureAdapter::<_, OnVmReady> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_vm_shutdown<F>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(
                &'c mut LifecycleCtx<'i>,
            )
                -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + 'c>>
            + Send
            + Sync
            + 'static,
    {
        self.registry.lifecycle.push(Arc::new(LifecycleClosureAdapter::<_, OnVmShutdown> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_pre_commit<F>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(
                &'c mut LifecycleCtx<'i>,
            )
                -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + 'c>>
            + Send
            + Sync
            + 'static,
    {
        self.registry.lifecycle.push(Arc::new(LifecycleClosureAdapter::<_, OnPreCommit> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_post_commit<F>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(
                &'c mut LifecycleCtx<'i>,
                bool,
            )
                -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + 'c>>
            + Send
            + Sync
            + 'static,
    {
        self.registry.lifecycle.push(Arc::new(LifecycleClosureAdapter::<_, OnPostCommit> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_host_preempt<F>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(
                &'c mut LifecycleCtx<'i>,
            )
                -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + 'c>>
            + Send
            + Sync
            + 'static,
    {
        self.registry.lifecycle.push(Arc::new(LifecycleClosureAdapter::<_, OnHostPreempt> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_memory_pressure<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(u32, u32) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.registry.telemetry.push(Arc::new(TelemetryClosureAdapter::<_, OnMemoryPressure> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_heartbeat<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(&Heartbeat) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.registry.telemetry.push(Arc::new(TelemetryClosureAdapter::<_, OnHeartbeat> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_watchdog<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(&WatchdogEvent) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<WatchdogVerdict, PluginError>> + Send + 'static,
    {
        self.registry.telemetry.push(Arc::new(TelemetryClosureAdapter::<_, OnWatchdog> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    // see: cans/02-plugin-model.md#Unified-builder
    // trap(filter, handler) is the filter form, in both modes; the
    // filter is retained and validated at seal(), so this verb never
    // fails and the chain never breaks on it.
    pub fn trap<F>(mut self, filter: impl Into<TrapFilter>, f: F) -> Self
    where
        F: Fn(&TrapEvent, &HandlerCtx) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        let filter: TrapFilter = filter.into();
        let snapshot = FilterSnapshot::from(&filter);
        self.registry.trap_filters.push(filter);
        self.registry.trap.push(Arc::new(TrapClosureAdapter::<_, OnTrap> {
            handler: move |event: &TrapEvent, ctx: &HandlerCtx| {
                if !filter_matches(&snapshot, event) {
                    return Ok(Verdict::Pass);
                }
                f(event, ctx)
            },
            verb: PhantomData,
        }));
        self
    }

    pub fn trap_scope<F>(self, configure: F) -> Self
    where
        F: FnOnce(TrapScope) -> Self,
    {
        configure(TrapScope { builder: self })
    }

    pub fn group<F>(mut self, configure: F) -> Self
    where
        F: FnOnce(Self) -> Self,
    {
        configure(self)
    }

    pub fn guard<F>(mut self, enabled: bool, configure: F) -> Self
    where
        F: FnOnce(Self) -> Self,
    {
        if enabled { configure(self) } else { self }
    }

    pub fn fs<F>(self, configure: F) -> Self
    where
        F: FnOnce(FsScope) -> Self,
    {
        configure(FsScope { builder: self })
    }

    pub fn net<F>(self, configure: F) -> Self
    where
        F: FnOnce(NetScope) -> Self,
    {
        configure(NetScope { builder: self })
    }

    pub fn console<F>(self, configure: F) -> Self
    where
        F: FnOnce(ConsoleScope) -> Self,
    {
        configure(ConsoleScope { builder: self })
    }

    pub fn exec<F>(self, configure: F) -> Self
    where
        F: FnOnce(ExecScope) -> Self,
    {
        configure(ExecScope { builder: self })
    }

    pub fn host<F>(self, configure: F) -> Self
    where
        F: FnOnce(HostScope) -> Self,
    {
        configure(HostScope { builder: self })
    }

    pub fn lifecycle<F>(self, configure: F) -> Self
    where
        F: FnOnce(LifecycleScope) -> Self,
    {
        configure(LifecycleScope { builder: self })
    }

    pub fn telemetry<F>(self, configure: F) -> Self
    where
        F: FnOnce(TelemetryScope) -> Self,
    {
        configure(TelemetryScope { builder: self })
    }
}

// see: cans/02-plugin-model.md#Unified-builder
// Mount. plugin() merges a plugin-mode builder by value: one
// MountedPlugin (name, scope, fork, registry) appends to the host
// Registration; no hook vec merges at mount, bind_registry merges at
// boot. plugin() and plugin_instance() return Self: mounting the same
// plugin twice is recorded here and rejected by seal(). A plugin-mode
// builder has no plugin() method, so plugin-into-plugin mount is a
// compile error in Rust, not a merge. use() is the same verb.
impl KelirBuilder {
    pub fn plugin(mut self, plugin: Self) -> Self {
        // Live, plugin() takes KelirBuilder<Plugin, Unsealed, ...>: a
        // host-mode builder cannot be passed, so the name is always set.
        self.registration.mounted.push(MountedPlugin {
            name: plugin.name,
            scope: plugin.scope,
            fork: plugin.fork,
            registry: plugin.registry,
        });
        self
    }

    pub fn r#use(self, plugin: Self) -> Self {
        self.plugin(plugin)
    }

    pub fn plugin_instance<P>(mut self, plugin: P) -> Self
    where
        P: KelirPlugin + PluginDeps + Clone + 'static,
    {
        let context = plugin.context();
        let scope = plugin.default_scope();
        let mut registration = self.registration;
        registration.instances.push((Box::new(plugin), scope));
        self.plugins = HCons {
            head: context,
            tail: self.plugins,
        };
        self.registration = registration;
        self
    }
}

// see: cans/02-plugin-model.md#Unified-builder
// Host-only verbs. They exist only on the Host-mode impl (live:
// builder_sdk.flow.rs Host,Unsealed); a plugin-mode builder has no such
// methods, so calling one is a compile error. params() is host-only by
// type (the arity pin). The live host's on_vm_init / on_vm_ready /
// on_vm_shutdown methods are the ring verbs above (append, plain async
// fn); on_trap_deny stays: it is not one of the 24 rings. trap(filter,
// handler) is not here: it is a both-modes verb in the impl above.
impl KelirBuilder {
    pub fn storage(mut self, config: StoragePlaneConfig) -> Self {
        self.registration.storage_cfg = Some(config);
        self
    }

    pub fn terminal(mut self, config: PtyConfig) -> Self {
        self.registration.terminal_cfg = Some(config);
        self
    }

    pub fn wire(mut self, config: WireConfig) -> Self {
        self.registration.wire_cfg = Some(config);
        self
    }

    pub fn lock_broker(mut self, config: LockBrokerConfig) -> Self {
        self.registration.lock_broker_cfg = Some(config);
        self
    }

    pub fn detached(mut self, detached: bool) -> Self {
        self.registration.detached = detached;
        self
    }

    pub fn fast_boot(mut self, mode: FastBootMode) -> Self {
        self.fast_boot = mode;
        self
    }

    pub fn dax_window(mut self, mode: DaxWindowMode) -> Self {
        self.dax_window = mode;
        self
    }

    pub fn is_detached(&self) -> bool {
        self.registration.detached
    }

    pub fn params<T>(self) -> Self {
        self
    }

    // see: cans/02-plugin-model.md#Unified-builder
    // trap(filter, handler) is not here: it is a both-modes verb in the
    // impl above. trap_async and post_exec are host-only, and infallible
    // like trap: each retains its filter and seal() validates it.
    // see: cans/10-attestation.md#Step-evaluation-loop
    pub fn trap_async<F, Fut>(
        mut self,
        filter: impl Into<TrapFilter>,
        f: F,
    ) -> Self
    where
        F: Fn(HandlerCtx) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Verdict, PluginError>> + Send + 'static,
    {
        let filter: TrapFilter = filter.into();
        let snapshot = FilterSnapshot::from(&filter);
        self.registry.trap_filters.push(filter);
        self.registry.trap.push(Arc::new(AsyncTrapClosureAdapter {
            filter: snapshot,
            handler: f,
        }));
        self
    }

    // see: cans/10-attestation.md#Step-evaluation-loop
    pub fn post_exec<F, Fut>(
        mut self,
        filter: impl Into<TrapFilter>,
        f: F,
    ) -> Self
    where
        F: Fn(HandlerCtx) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<PostExecAction, PluginError>> + Send + 'static,
    {
        let filter: TrapFilter = filter.into();
        let snapshot = FilterSnapshot::from(&filter);
        self.registry.trap_filters.push(filter);
        self.registry.post_exec.push(Arc::new(PostExecGateAdapter {
            filter: snapshot,
            handler: f,
        }));
        self
    }

    pub fn on_trap_deny<F, Fut, T>(mut self, f: F) -> Self
    where
        F: Fn(&TrapEvent, &kelir_engine::Session) -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = Result<T, PluginError>> + Send + 'static,
    {
        self.registration.on_trap_deny = Some(Box::new(
            move |event: &TrapEvent, session: &kelir_engine::Session| {
                let fut = f.clone()(event, session);
                Box::pin(async move { fut.await.map(drop) })
            },
        ));
        self
    }

    pub fn register_custom_action<F, Fut>(
        mut self,
        name: &str,
        schema: serde_json::Value,
        f: F,
    ) -> Self
    where
        F: Fn(ciborium::value::Value) -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'static,
    {
        self.registration.custom_actions.push(CustomActionEntry {
            name: name.to_string(),
            schema,
            handler: Box::new(move |payload: ciborium::value::Value| {
                let fut = f.clone()(payload);
                Box::pin(fut)
            }),
        });
        self
    }

    pub fn register_stream_kind<F, Fut>(
        mut self,
        kind: u16,
        schema: serde_json::Value,
        f: F,
    ) -> Self
    where
        F: Fn(Vec<u8>) -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>>
            + Send
            + 'static,
    {
        self.registration
            .stream_kind_services
            .push(StreamKindServiceEntry {
                kind,
                schema,
                handler: Box::new(move |request: Vec<u8>| {
                    let fut = f.clone()(request);
                    Box::pin(fut)
                }),
            });
        self
    }

    pub fn lifecycle_mut(&mut self) -> LifecycleMut<'_> {
        LifecycleMut {
            vetoes: &mut self.registration.vetoes,
        }
    }

    // see: cans/02-plugin-model.md#Unified-builder
    // The one validation. seal() validates and returns Result. (a) vsock
    // triple and (b) scope triple over the host's own registrations and
    // every mounted plugin's, with the plugin named on failure. (c) deps:
    // enforced at mount by the Satisfies bound on plugin() /
    // plugin_instance(); there is no runtime deps check in Rust, the
    // uniform message is the TS/Python runtime form. (d) trap-filter
    // triple: every trap filter has a non-empty binary and non-empty
    // args, host and mounted alike. (e) duplicate mount names, recorded
    // by plugin() / plugin_instance(), rejected here. There is no
    // host-only-verb check in Rust: those verbs do not exist on a
    // plugin-mode builder, so they cannot be recorded.
    pub fn seal(self) -> Result<Self, PluginError> {
        self.registry.validate_vsock(self.name)?;
        validate_scope(self.name, &self.scope)?;
        for filter in &self.registry.trap_filters {
            validate_trap_filter("host", filter)?;
        }
        let mut seen: Vec<&'static str> = Vec::new();
        for mounted in &self.registration.mounted {
            if seen.contains(&mounted.name) {
                return Err(PluginError::reject(
                    Errno::Econfig,
                    format!("plugin already mounted: {}", mounted.name),
                ));
            }
            seen.push(mounted.name);
            mounted.registry.validate_vsock(mounted.name)?;
            validate_scope(mounted.name, &mounted.scope)?;
            for filter in &mounted.registry.trap_filters {
                validate_trap_filter(mounted.name, filter)?;
            }
        }
        Ok(self)
    }
}

// see: cans/02-plugin-model.md#Unified-builder
// A plugin-mode builder IS a KelirPlugin: the trait stays underneath as
// the low-level interface, and mounting via plugin_instance() takes any
// KelirPlugin. register_interceptors binds the seven vecs and the vsock
// services with the builder's scope, exactly as the deleted PluginWrapper
// did. Trait-object plugins mounted via plugin_instance() validate
// vsock/scope at bind time through the binder, not at seal(): their specs
// are only visible inside register_interceptors.
impl KelirPlugin for KelirBuilder
where
    Ctx: Clone + Send + Sync + 'static,
{
    type Context = Ctx;

    fn context(&self) -> Self::Context {
        (self.context_factory)()
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn default_scope(&self) -> MountScope {
        self.scope.clone()
    }

    fn fork_state(&self, _child_session_id: &str) -> Box<dyn AnyPlugin> {
        // Plugin-mode only by type: clone_box rebuilds the builder in
        // plugin mode, so a host-mode builder never forks itself; hosts
        // fork via Session.fork().
        match &self.fork {
            Some(fork) => fork(_child_session_id),
            None => self.clone_box(),
        }
    }

    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        self.bind_registry(pipeline);
    }
}

// see: cans/02-plugin-model.md#Unified-builder
impl PluginDeps for KelirBuilder {
    type Deps = Deps;
}

// see: cans/02-plugin-model.md#Unified-builder
// Deleted shapes: PluginBuilder, PluginWrapper, plugin_builder_scopes,
// the duplicated Satisfies (one Satisfies lives in hlist_sdk.flow.rs),
// build(), with_composite(). Nothing is documented as an alternative:
// the old shapes are gone, not deprecated.
// Ring resolution holds by construction: every verb is typed, so no
// registered verb can name an unknown ring. An empty builder (no
// handlers) still seals. VsockServiceSpec is (port: u32, service:
// String); it carries no schema field live, so the (port, service,
// schema) spec is plugin-level in TS/Python and (port, service) in Rust.
// Descriptors: host hooks first, then mounted plugins in mount order;
// within a plugin, registration order. The host's fuse()/describe()
// reads the merged vecs through this one path.
// Seal-time messages name the plugin on mounted-plugin failures:
// `<plugin>: <triple failure>`. Host-level failures use the bare triple
// message. The duplicate-mount message is `plugin already mounted:
// <name>`, raised by seal(), not by plugin(); the missing-prerequisite
// message is `missing prerequisite: <dep> required by <plugin>`
// (runtime form in TS/Python; Satisfies bound in Rust). group(f) applies
// f to the builder; guard(enabled, f) applies f only when enabled; every
// scope has .end() returning the builder, and scope closures must return
// the builder via .end(). fork_state is a factory, not a chain verb:
// it always yields a plugin-mode builder; hosts fork via Session.fork().
// Scope shape: see: cans/artifacts/code/rust-plugin-builder-scopes.rs.
// Scopes own the builder; scope-entry closures return the builder via
// .end(), never the scope. use() is the same verb as plugin().
// validate_trap_filter(name, filter) rejects an empty binary or an
// empty arg; it runs at seal(), never at registration.
