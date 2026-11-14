// see: cans/02-plugin-model.md#KelirPlugin-trait
// see: cans/02-plugin-model.md#Segregated-ring-traits
use std::future::Future;
use std::marker::PhantomData;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use kelir_interface::driver_interface::{
    DaxWindowMode, FastBootMode, WatchdogEvent, WatchdogVerdict,
};
use kelir_interface::fusion_plugin::FilterSnapshot;
use kelir_interface::plugin_model::{
    AnyPlugin, HandlerCtx, KelirPlugin, LifecycleCtx, MountScope, PipelineBinder, PluginClone,
    PluginDeps, PluginError,
};
use kelir_interface::protocol_wire::{
    FilterAction, Heartbeat, PostExecAction, ProcessExit, TrapEvent, Verdict,
};
use kelir_interface::vsock_mesh::{VsockServiceHandler, VsockServiceSpec};

use crate::builder_scopes::{
    ConsoleScope, ExecScope, FsScope, HostScope, LifecycleScope, NetScope, TelemetryScope,
    TrapScope,
};
use crate::builder_sdk::{
    Host, KelirBuilder, Plugin, Registration, TrapFilter, Unsealed, filter_matches,
};
use crate::hlist_sdk::{HCons, HList, HNil, NoParams};

use crate::builder_adapters::{
    AsyncTrapClosureAdapter, ConsoleClosureAdapter, FsClosureAdapter, HostClosureAdapter,
    LifecycleClosureAdapter, NetClosureAdapter, OnConsoleInput, OnConsoleOutput, OnExecStderr,
    OnExecStdout, OnFsLock, OnFsLookup, OnFsRead, OnFsWrite, OnHeartbeat, OnHostKeyringAccess,
    OnHostLsmExec, OnHostPreempt, OnHostPressure, OnMemoryPressure, OnNetInbound, OnNetOutbound,
    OnPostCommit, OnPreCommit, OnProcessExit, OnTrap, OnVmInit, OnVmReady, OnVmShutdown,
    OnWatchdog, PostExecGateAdapter, TelemetryClosureAdapter, TrapClosureAdapter,
};

// see: cans/02-plugin-model.md#Unified-builder
impl<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList>
    KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params>
{
    pub fn context<NextCtx>(
        self,
        factory: impl Fn() -> NextCtx + Send + Sync + 'static,
    ) -> KelirBuilder<Mode, Unsealed, NextCtx, Deps, Plugins, Params>
    where
        NextCtx: Clone + Send + Sync + 'static,
    {
        KelirBuilder {
            config: self.config,
            plugins: self.plugins,
            registration: self.registration,
            fast_boot: self.fast_boot,
            dax_window: self.dax_window,
            name: self.name,
            context_factory: Arc::new(factory),
            scope: self.scope,
            fork: self.fork,
            registry: self.registry,
            seal: PhantomData,
            deps: PhantomData,
        }
    }

    pub fn require<P>(
        self,
    ) -> KelirBuilder<Mode, Unsealed, Ctx, HCons<P::Context, Deps>, Plugins, Params>
    where
        P: KelirPlugin,
    {
        KelirBuilder {
            config: self.config,
            plugins: self.plugins,
            registration: self.registration,
            fast_boot: self.fast_boot,
            dax_window: self.dax_window,
            name: self.name,
            context_factory: self.context_factory,
            scope: self.scope,
            fork: self.fork,
            registry: self.registry,
            seal: PhantomData,
            deps: PhantomData,
        }
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

    // see: cans/02-plugin-model.md#Filter-before-FFI-gate
    pub fn trap<F>(mut self, filter: impl Into<TrapFilter>, f: F) -> Self
    where
        F: Fn(&TrapEvent, &HandlerCtx) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        let filter: TrapFilter = filter.into();
        let snapshot = FilterSnapshot::from(&filter);
        self.registry.trap_filters.push(filter);
        self.registry
            .trap
            .push(Arc::new(TrapClosureAdapter::<_, OnTrap> {
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

    // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
    pub fn trap_scope<F>(self, configure: F) -> Self
    where
        F: FnOnce(
            TrapScope<Plugins, Params, Mode, Ctx, Deps>,
        ) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params>,
    {
        configure(TrapScope { builder: self })
    }

    pub fn group<F>(self, configure: F) -> Self
    where
        F: FnOnce(Self) -> Self,
    {
        configure(self)
    }

    pub fn guard<F>(self, enabled: bool, configure: F) -> Self
    where
        F: FnOnce(Self) -> Self,
    {
        if enabled { configure(self) } else { self }
    }

    pub fn on_trap<F>(mut self, f: F) -> Self
    where
        F: Fn(&TrapEvent, &HandlerCtx) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.registry
            .trap
            .push(Arc::new(TrapClosureAdapter::<_, OnTrap> {
                handler: f,
                verb: PhantomData,
            }));
        self
    }

    pub fn on_process_exit<F>(mut self, f: F) -> Self
    where
        F: Fn(&ProcessExit, &HandlerCtx) -> Result<(), PluginError> + Send + Sync + 'static,
    {
        self.registry
            .trap
            .push(Arc::new(TrapClosureAdapter::<_, OnProcessExit> {
                handler: f,
                verb: PhantomData,
            }));
        self
    }

    pub fn on_fs_lookup<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.registry
            .fs
            .push(Arc::new(FsClosureAdapter::<_, OnFsLookup> {
                handler: f,
                verb: PhantomData,
            }));
        self
    }

    pub fn on_fs_read<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync + 'static,
    {
        self.registry
            .fs
            .push(Arc::new(FsClosureAdapter::<_, OnFsRead> {
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
        self.registry
            .fs
            .push(Arc::new(FsClosureAdapter::<_, OnFsWrite> {
                handler: f,
                verb: PhantomData,
            }));
        self
    }

    pub fn on_fs_lock<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, i32, i16, u32) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.registry
            .fs
            .push(Arc::new(FsClosureAdapter::<_, OnFsLock> {
                handler: f,
                verb: PhantomData,
            }));
        self
    }

    pub fn on_net_outbound<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut [u8]) -> Result<FilterAction, PluginError> + Send + Sync + 'static,
    {
        self.registry
            .net
            .push(Arc::new(NetClosureAdapter::<_, OnNetOutbound> {
                handler: f,
                verb: PhantomData,
            }));
        self
    }

    pub fn on_net_inbound<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut [u8]) -> Result<FilterAction, PluginError> + Send + Sync + 'static,
    {
        self.registry
            .net
            .push(Arc::new(NetClosureAdapter::<_, OnNetInbound> {
                handler: f,
                verb: PhantomData,
            }));
        self
    }

    pub fn on_console_input<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync + 'static,
    {
        self.registry
            .console
            .push(Arc::new(ConsoleClosureAdapter::<_, OnConsoleInput> {
                handler: f,
                verb: PhantomData,
            }));
        self
    }

    pub fn on_console_output<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync + 'static,
    {
        self.registry
            .console
            .push(Arc::new(ConsoleClosureAdapter::<_, OnConsoleOutput> {
                handler: f,
                verb: PhantomData,
            }));
        self
    }

    pub fn on_exec_stdout<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, u64, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync + 'static,
    {
        self.registry
            .console
            .push(Arc::new(ConsoleClosureAdapter::<_, OnExecStdout> {
                handler: f,
                verb: PhantomData,
            }));
        self
    }

    pub fn on_exec_stderr<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, u64, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync + 'static,
    {
        self.registry
            .console
            .push(Arc::new(ConsoleClosureAdapter::<_, OnExecStderr> {
                handler: f,
                verb: PhantomData,
            }));
        self
    }

    pub fn on_host_lsm_exec<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, u32) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.registry
            .host
            .push(Arc::new(HostClosureAdapter::<_, OnHostLsmExec> {
                handler: f,
                verb: PhantomData,
            }));
        self
    }

    pub fn on_host_pressure<F>(mut self, f: F) -> Self
    where
        F: Fn(u32) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.registry
            .host
            .push(Arc::new(HostClosureAdapter::<_, OnHostPressure> {
                handler: f,
                verb: PhantomData,
            }));
        self
    }

    pub fn on_host_keyring_access<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.registry
            .host
            .push(Arc::new(HostClosureAdapter::<_, OnHostKeyringAccess> {
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
        self.registry
            .lifecycle
            .push(Arc::new(LifecycleClosureAdapter::<_, OnVmInit> {
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
        self.registry
            .lifecycle
            .push(Arc::new(LifecycleClosureAdapter::<_, OnVmReady> {
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
        self.registry
            .lifecycle
            .push(Arc::new(LifecycleClosureAdapter::<_, OnVmShutdown> {
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
        self.registry
            .lifecycle
            .push(Arc::new(LifecycleClosureAdapter::<_, OnPreCommit> {
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
        self.registry
            .lifecycle
            .push(Arc::new(LifecycleClosureAdapter::<_, OnPostCommit> {
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
        self.registry
            .lifecycle
            .push(Arc::new(LifecycleClosureAdapter::<_, OnHostPreempt> {
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
        self.registry
            .telemetry
            .push(Arc::new(TelemetryClosureAdapter::<_, OnMemoryPressure> {
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
        self.registry
            .telemetry
            .push(Arc::new(TelemetryClosureAdapter::<_, OnHeartbeat> {
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
        self.registry
            .telemetry
            .push(Arc::new(TelemetryClosureAdapter::<_, OnWatchdog> {
                handler: f,
                verb: PhantomData,
            }));
        self
    }

    pub fn fs<F>(self, configure: F) -> Self
    where
        F: FnOnce(
            FsScope<Plugins, Params, Mode, Ctx, Deps>,
        ) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params>,
    {
        configure(FsScope { builder: self })
    }

    pub fn net<F>(self, configure: F) -> Self
    where
        F: FnOnce(
            NetScope<Plugins, Params, Mode, Ctx, Deps>,
        ) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params>,
    {
        configure(NetScope { builder: self })
    }

    pub fn console<F>(self, configure: F) -> Self
    where
        F: FnOnce(
            ConsoleScope<Plugins, Params, Mode, Ctx, Deps>,
        ) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params>,
    {
        configure(ConsoleScope { builder: self })
    }

    pub fn exec<F>(self, configure: F) -> Self
    where
        F: FnOnce(
            ExecScope<Plugins, Params, Mode, Ctx, Deps>,
        ) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params>,
    {
        configure(ExecScope { builder: self })
    }

    pub fn host<F>(self, configure: F) -> Self
    where
        F: FnOnce(
            HostScope<Plugins, Params, Mode, Ctx, Deps>,
        ) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params>,
    {
        configure(HostScope { builder: self })
    }

    pub fn lifecycle<F>(self, configure: F) -> Self
    where
        F: FnOnce(
            LifecycleScope<Plugins, Params, Mode, Ctx, Deps>,
        ) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params>,
    {
        configure(LifecycleScope { builder: self })
    }

    pub fn telemetry<F>(self, configure: F) -> Self
    where
        F: FnOnce(
            TelemetryScope<Plugins, Params, Mode, Ctx, Deps>,
        ) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params>,
    {
        configure(TelemetryScope { builder: self })
    }
}

// see: cans/02-plugin-model.md#Filter-before-FFI-gate
impl<Plugins: HList, Params: 'static, Ctx, Deps: HList>
    KelirBuilder<Host, Unsealed, Ctx, Deps, Plugins, Params>
{
    // see: cans/10-attestation.md#Step-evaluation-loop
    pub fn trap_async<F, Fut>(mut self, filter: impl Into<TrapFilter>, f: F) -> Self
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
    pub fn post_exec<F, Fut>(mut self, filter: impl Into<TrapFilter>, f: F) -> Self
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
}

// see: cans/02-plugin-model.md#KelirPlugin-trait
// see: cans/02-plugin-model.md#Chain-decisions
impl<SealState, Plugins: HList, Params: 'static, Ctx, Deps: HList> KelirPlugin
    for KelirBuilder<Plugin, SealState, Ctx, Deps, Plugins, Params>
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

    // see: cans/02-plugin-model.md#KelirPlugin-trait
    // TODO (L2): fork_state is synchronous on the runtime thread; clone state without blocking_lock/block_on per cans/02-plugin-model.md#KelirPlugin-trait.
    fn fork_state(&self, child_session_id: &str) -> Box<dyn AnyPlugin> {
        match &self.fork {
            Some(fork) => fork(child_session_id),
            None => self.clone_box(),
        }
    }

    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        self.bind_registry(pipeline);
    }
}

// see: cans/02-plugin-model.md#KelirPlugin-trait
// see: cans/02-plugin-model.md#Chain-decisions
impl<SealState, Plugins: HList, Params: 'static, Ctx, Deps: HList> PluginClone
    for KelirBuilder<Plugin, SealState, Ctx, Deps, Plugins, Params>
where
    Ctx: Clone + Send + Sync + 'static,
{
    fn clone_box(&self) -> Box<dyn AnyPlugin> {
        let cloned: KelirBuilder<Plugin, Unsealed, Ctx, Deps, HNil, NoParams> = KelirBuilder {
            config: None,
            plugins: HNil,
            registration: Registration::default(),
            fast_boot: FastBootMode::Off,
            dax_window: DaxWindowMode::Off,
            name: self.name,
            context_factory: Arc::clone(&self.context_factory),
            scope: self.scope.clone(),
            fork: self.fork.clone(),
            registry: self.registry.clone(),
            seal: PhantomData,
            deps: PhantomData,
        };
        Box::new(cloned)
    }
}

// see: cans/02-plugin-model.md#KelirPlugin-trait
// see: cans/02-plugin-model.md#Chain-decisions
impl<SealState, Plugins: HList, Params: 'static, Ctx, Deps: HList> AnyPlugin
    for KelirBuilder<Plugin, SealState, Ctx, Deps, Plugins, Params>
where
    Ctx: Clone + Send + Sync + 'static,
{
    fn name(&self) -> &str {
        KelirPlugin::name(self)
    }

    fn fork_state(&self, child_session_id: &str) -> Box<dyn AnyPlugin> {
        KelirPlugin::fork_state(self, child_session_id)
    }

    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        KelirPlugin::register_interceptors(self, pipeline)
    }

    fn default_scope(&self) -> MountScope {
        KelirPlugin::default_scope(self)
    }

    fn is_enabled(&self) -> bool {
        true
    }
}

// see: cans/02-plugin-model.md#KelirPlugin-trait
// see: cans/02-plugin-model.md#Chain-decisions
impl<SealState, Plugins: HList, Params: 'static, Ctx, Deps: HList> PluginDeps
    for KelirBuilder<Plugin, SealState, Ctx, Deps, Plugins, Params>
{
    type Deps = Deps;
}
