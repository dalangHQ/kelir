// see: cans/02-plugin-model.md#KelirPlugin-trait
// see: cans/02-plugin-model.md#Segregated-ring-traits
use std::future::Future;
use std::marker::PhantomData;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use kelir_interface::driver_interface::{
    DaxWindowMode, FastBootMode, WatchdogEvent, WatchdogVerdict,
};
use kelir_interface::fusion_plugin::FilterSnapshot;
use kelir_interface::plugin_model::{
    AnyPlugin, ConsoleInterposer, FsInterposer, HandlerCtx, HostInterposer, KelirPlugin,
    LifecycleCtx, LifecycleParticipant, MountScope, NetInterposer, PipelineBinder, PluginClone,
    PluginDeps, PluginError, SystemTelemetryParticipant, TrapInterposer,
};
use kelir_interface::protocol_wire::{FilterAction, ProcessExit, TrapEvent, Verdict};
use kelir_interface::vsock_mesh::{VsockServiceHandler, VsockServiceSpec};

use crate::builder_scopes::{
    ConsoleScope, ExecScope, FsScope, HostScope, LifecycleScope, NetScope, TelemetryScope,
    TrapScope,
};
use crate::builder_sdk::{
    Host, KelirBuilder, Plugin, Registration, TrapFilter, Unsealed, filter_matches,
};
use crate::hlist_sdk::{HCons, HList, HNil, NoParams};

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

#[async_trait]
impl<F> TrapInterposer for TrapClosureAdapter<F, OnTrap>
where
    F: Fn(&TrapEvent, &HandlerCtx) -> Result<Verdict, PluginError> + Send + Sync,
{
    async fn on_trap(&self, event: &TrapEvent, ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        (self.handler)(event, ctx)
    }
}

#[async_trait]
impl<F> TrapInterposer for TrapClosureAdapter<F, OnProcessExit>
where
    F: Fn(&ProcessExit, &HandlerCtx) -> Result<(), PluginError> + Send + Sync,
{
    async fn on_process_exit(
        &self,
        event: &ProcessExit,
        ctx: &HandlerCtx,
    ) -> Result<(), PluginError> {
        (self.handler)(event, ctx)
    }
}

impl<F> FsInterposer for FsClosureAdapter<F, OnFsLookup>
where
    F: Fn(&Path) -> Result<Verdict, PluginError> + Send + Sync,
{
    fn on_fs_lookup(&self, path: &Path) -> Result<Verdict, PluginError> {
        (self.handler)(path)
    }
}

impl<F> FsInterposer for FsClosureAdapter<F, OnFsRead>
where
    F: Fn(&Path, &mut [u8]) -> Result<(), PluginError> + Send + Sync,
{
    fn on_fs_read(&self, path: &Path, buffer: &mut [u8]) -> Result<(), PluginError> {
        (self.handler)(path, buffer)
    }
}

impl<F> FsInterposer for FsClosureAdapter<F, OnFsWrite>
where
    F: Fn(&Path, &mut [u8], u32, &HandlerCtx) -> Result<FilterAction, PluginError> + Send + Sync,
{
    fn on_fs_write(
        &self,
        path: &Path,
        data: &mut [u8],
        pid: u32,
        ctx: &HandlerCtx,
    ) -> Result<FilterAction, PluginError> {
        (self.handler)(path, data, pid, ctx)
    }
}

impl<F> FsInterposer for FsClosureAdapter<F, OnFsLock>
where
    F: Fn(&Path, i32, i16, u32) -> Result<Verdict, PluginError> + Send + Sync,
{
    fn on_fs_lock(
        &self,
        path: &Path,
        flock_cmd: i32,
        flock_type: i16,
        pid: u32,
    ) -> Result<Verdict, PluginError> {
        (self.handler)(path, flock_cmd, flock_type, pid)
    }
}

impl<F> NetInterposer for NetClosureAdapter<F, OnNetOutbound>
where
    F: Fn(&mut [u8]) -> Result<FilterAction, PluginError> + Send + Sync,
{
    fn on_net_outbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        (self.handler)(frame)
    }
}

impl<F> NetInterposer for NetClosureAdapter<F, OnNetInbound>
where
    F: Fn(&mut [u8]) -> Result<FilterAction, PluginError> + Send + Sync,
{
    fn on_net_inbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        (self.handler)(frame)
    }
}

#[async_trait]
impl<F> ConsoleInterposer for ConsoleClosureAdapter<F, OnConsoleInput>
where
    F: Fn(u32, &mut Vec<u8>) -> Result<(), PluginError> + Send + Sync,
{
    async fn on_console_input(&self, port: u32, bytes: &mut Vec<u8>) -> Result<(), PluginError> {
        (self.handler)(port, bytes)
    }
}

#[async_trait]
impl<F> ConsoleInterposer for ConsoleClosureAdapter<F, OnConsoleOutput>
where
    F: Fn(u32, &mut Vec<u8>) -> Result<(), PluginError> + Send + Sync,
{
    async fn on_console_output(&self, port: u32, chunk: &mut Vec<u8>) -> Result<(), PluginError> {
        (self.handler)(port, chunk)
    }
}

#[async_trait]
impl<F> ConsoleInterposer for ConsoleClosureAdapter<F, OnExecStdout>
where
    F: Fn(u32, u64, &mut Vec<u8>) -> Result<(), PluginError> + Send + Sync,
{
    async fn on_exec_stdout(
        &self,
        pid: u32,
        exec_id: u64,
        chunk: &mut Vec<u8>,
    ) -> Result<(), PluginError> {
        (self.handler)(pid, exec_id, chunk)
    }
}

#[async_trait]
impl<F> ConsoleInterposer for ConsoleClosureAdapter<F, OnExecStderr>
where
    F: Fn(u32, u64, &mut Vec<u8>) -> Result<(), PluginError> + Send + Sync,
{
    async fn on_exec_stderr(
        &self,
        pid: u32,
        exec_id: u64,
        chunk: &mut Vec<u8>,
    ) -> Result<(), PluginError> {
        (self.handler)(pid, exec_id, chunk)
    }
}

impl<F> HostInterposer for HostClosureAdapter<F, OnHostLsmExec>
where
    F: Fn(&Path, u32) -> Result<Verdict, PluginError> + Send + Sync,
{
    fn on_host_lsm_exec(&self, path: &Path, pid: u32) -> Result<Verdict, PluginError> {
        (self.handler)(path, pid)
    }
}

impl<F> HostInterposer for HostClosureAdapter<F, OnHostPressure>
where
    F: Fn(u32) -> Result<Verdict, PluginError> + Send + Sync,
{
    fn on_host_pressure(&self, level: u32) -> Result<Verdict, PluginError> {
        (self.handler)(level)
    }
}

impl<F> HostInterposer for HostClosureAdapter<F, OnHostKeyringAccess>
where
    F: Fn(&str) -> Result<Verdict, PluginError> + Send + Sync,
{
    fn on_host_keyring_access(&self, key: &str) -> Result<Verdict, PluginError> {
        (self.handler)(key)
    }
}

#[async_trait]
impl<F, Fut> LifecycleParticipant for LifecycleClosureAdapter<F, OnVmInit>
where
    F: for<'c, 'i> Fn(&'c mut LifecycleCtx<'i>) -> Fut + Send + Sync,
    Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
{
    async fn on_vm_init(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        (self.handler)(ctx).await
    }
}

#[async_trait]
impl<F, Fut> LifecycleParticipant for LifecycleClosureAdapter<F, OnVmReady>
where
    F: for<'c, 'i> Fn(&'c mut LifecycleCtx<'i>) -> Fut + Send + Sync,
    Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
{
    async fn on_vm_ready(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        (self.handler)(ctx).await
    }
}

#[async_trait]
impl<F, Fut> LifecycleParticipant for LifecycleClosureAdapter<F, OnVmShutdown>
where
    F: for<'c, 'i> Fn(&'c mut LifecycleCtx<'i>) -> Fut + Send + Sync,
    Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
{
    async fn on_vm_shutdown(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        (self.handler)(ctx).await
    }
}

#[async_trait]
impl<F, Fut> LifecycleParticipant for LifecycleClosureAdapter<F, OnPreCommit>
where
    F: for<'c, 'i> Fn(&'c mut LifecycleCtx<'i>) -> Fut + Send + Sync,
    Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
{
    async fn on_pre_commit(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        (self.handler)(ctx).await
    }
}

#[async_trait]
impl<F, Fut> LifecycleParticipant for LifecycleClosureAdapter<F, OnPostCommit>
where
    F: for<'c, 'i> Fn(&'c mut LifecycleCtx<'i>, bool) -> Fut + Send + Sync,
    Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
{
    async fn on_post_commit(
        &self,
        ctx: &mut LifecycleCtx<'_>,
        committed: bool,
    ) -> Result<(), PluginError> {
        (self.handler)(ctx, committed).await
    }
}

#[async_trait]
impl<F, Fut> LifecycleParticipant for LifecycleClosureAdapter<F, OnHostPreempt>
where
    F: for<'c, 'i> Fn(&'c mut LifecycleCtx<'i>) -> Fut + Send + Sync,
    Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
{
    async fn on_host_preempt(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        (self.handler)(ctx).await
    }
}

#[async_trait]
impl<F, Fut> SystemTelemetryParticipant for TelemetryClosureAdapter<F, OnMemoryPressure>
where
    F: Fn(u32, u32) -> Fut + Send + Sync,
    Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
{
    async fn on_memory_pressure(
        &self,
        available_mb: u32,
        total_mb: u32,
    ) -> Result<(), PluginError> {
        (self.handler)(available_mb, total_mb).await
    }
}

#[async_trait]
impl<F, Fut> SystemTelemetryParticipant for TelemetryClosureAdapter<F, OnHeartbeat>
where
    F: Fn(u64) -> Fut + Send + Sync,
    Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
{
    async fn on_heartbeat(&self, uptime_ms: u64) -> Result<(), PluginError> {
        (self.handler)(uptime_ms).await
    }
}

#[async_trait]
impl<F, Fut> SystemTelemetryParticipant for TelemetryClosureAdapter<F, OnWatchdog>
where
    F: Fn(&WatchdogEvent) -> Fut + Send + Sync,
    Fut: Future<Output = Result<WatchdogVerdict, PluginError>> + Send + 'static,
{
    async fn on_watchdog(&self, event: &WatchdogEvent) -> Result<WatchdogVerdict, PluginError> {
        (self.handler)(event).await
    }
}

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
        F: Fn(&Path, &mut [u8]) -> Result<(), PluginError> + Send + Sync + 'static,
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
        F: Fn(u32, &mut Vec<u8>) -> Result<(), PluginError> + Send + Sync + 'static,
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
        F: Fn(u32, &mut Vec<u8>) -> Result<(), PluginError> + Send + Sync + 'static,
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
        F: Fn(u32, u64, &mut Vec<u8>) -> Result<(), PluginError> + Send + Sync + 'static,
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
        F: Fn(u32, u64, &mut Vec<u8>) -> Result<(), PluginError> + Send + Sync + 'static,
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

    pub fn on_vm_init<F, Fut>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(&'c mut LifecycleCtx<'i>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.registry
            .lifecycle
            .push(Arc::new(LifecycleClosureAdapter::<_, OnVmInit> {
                handler: f,
                verb: PhantomData,
            }));
        self
    }

    pub fn on_vm_ready<F, Fut>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(&'c mut LifecycleCtx<'i>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.registry
            .lifecycle
            .push(Arc::new(LifecycleClosureAdapter::<_, OnVmReady> {
                handler: f,
                verb: PhantomData,
            }));
        self
    }

    pub fn on_vm_shutdown<F, Fut>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(&'c mut LifecycleCtx<'i>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.registry
            .lifecycle
            .push(Arc::new(LifecycleClosureAdapter::<_, OnVmShutdown> {
                handler: f,
                verb: PhantomData,
            }));
        self
    }

    pub fn on_pre_commit<F, Fut>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(&'c mut LifecycleCtx<'i>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.registry
            .lifecycle
            .push(Arc::new(LifecycleClosureAdapter::<_, OnPreCommit> {
                handler: f,
                verb: PhantomData,
            }));
        self
    }

    pub fn on_post_commit<F, Fut>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(&'c mut LifecycleCtx<'i>, bool) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.registry
            .lifecycle
            .push(Arc::new(LifecycleClosureAdapter::<_, OnPostCommit> {
                handler: f,
                verb: PhantomData,
            }));
        self
    }

    pub fn on_host_preempt<F, Fut>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(&'c mut LifecycleCtx<'i>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
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
        F: Fn(u64) -> Fut + Send + Sync + 'static,
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
        ) -> FsScope<Plugins, Params, Mode, Ctx, Deps>,
    {
        configure(FsScope { builder: self }).builder
    }

    pub fn net<F>(self, configure: F) -> Self
    where
        F: FnOnce(
            NetScope<Plugins, Params, Mode, Ctx, Deps>,
        ) -> NetScope<Plugins, Params, Mode, Ctx, Deps>,
    {
        configure(NetScope { builder: self }).builder
    }

    pub fn console<F>(self, configure: F) -> Self
    where
        F: FnOnce(
            ConsoleScope<Plugins, Params, Mode, Ctx, Deps>,
        ) -> ConsoleScope<Plugins, Params, Mode, Ctx, Deps>,
    {
        configure(ConsoleScope { builder: self }).builder
    }

    pub fn exec<F>(self, configure: F) -> Self
    where
        F: FnOnce(
            ExecScope<Plugins, Params, Mode, Ctx, Deps>,
        ) -> ExecScope<Plugins, Params, Mode, Ctx, Deps>,
    {
        configure(ExecScope { builder: self }).builder
    }

    pub fn host<F>(self, configure: F) -> Self
    where
        F: FnOnce(
            HostScope<Plugins, Params, Mode, Ctx, Deps>,
        ) -> HostScope<Plugins, Params, Mode, Ctx, Deps>,
    {
        configure(HostScope { builder: self }).builder
    }

    pub fn lifecycle<F>(self, configure: F) -> Self
    where
        F: FnOnce(
            LifecycleScope<Plugins, Params, Mode, Ctx, Deps>,
        ) -> LifecycleScope<Plugins, Params, Mode, Ctx, Deps>,
    {
        configure(LifecycleScope { builder: self }).builder
    }

    pub fn telemetry<F>(self, configure: F) -> Self
    where
        F: FnOnce(
            TelemetryScope<Plugins, Params, Mode, Ctx, Deps>,
        ) -> TelemetryScope<Plugins, Params, Mode, Ctx, Deps>,
    {
        configure(TelemetryScope { builder: self }).builder
    }
}

// see: cans/02-plugin-model.md#Filter-before-FFI-gate
impl<Plugins: HList, Params: 'static, Ctx, Deps: HList>
    KelirBuilder<Host, Unsealed, Ctx, Deps, Plugins, Params>
{
    pub fn trap<F>(mut self, filter: impl Into<TrapFilter>, f: F) -> Result<Self, PluginError>
    where
        F: Fn(&TrapEvent, &HandlerCtx) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        let filter: TrapFilter = filter.into();
        validate_trap_filter("host", &filter)?;
        let snapshot = FilterSnapshot::from(&filter);
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
        Ok(self)
    }
}

// see: cans/02-plugin-model.md#Filter-before-FFI-gate
fn validate_trap_filter(name: &str, filter: &TrapFilter) -> Result<(), PluginError> {
    if filter.binary.as_deref() == Some("") {
        return Err(PluginError::reject(
            "EConfig",
            format!("{name}: trap filter binary must not be empty"),
        ));
    }
    if filter.args.iter().any(|arg| arg.is_empty()) {
        return Err(PluginError::reject(
            "EConfig",
            format!("{name}: trap filter arg must not be empty"),
        ));
    }
    Ok(())
}

// see: cans/02-plugin-model.md#Unified-builder
impl<Plugins: HList, Params: 'static, Ctx, Deps: HList>
    KelirBuilder<Plugin, Unsealed, Ctx, Deps, Plugins, Params>
{
    pub fn trap<F>(self, configure: F) -> Self
    where
        F: FnOnce(
            TrapScope<Plugins, Params, Plugin, Ctx, Deps>,
        ) -> TrapScope<Plugins, Params, Plugin, Ctx, Deps>,
    {
        configure(TrapScope { builder: self }).builder
    }
}

// see: cans/02-plugin-model.md#KelirPlugin-trait
impl<SealState, Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList> KelirPlugin
    for KelirBuilder<Mode, SealState, Ctx, Deps, Plugins, Params>
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
impl<SealState, Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList> PluginClone
    for KelirBuilder<Mode, SealState, Ctx, Deps, Plugins, Params>
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
impl<SealState, Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList> AnyPlugin
    for KelirBuilder<Mode, SealState, Ctx, Deps, Plugins, Params>
where
    Ctx: Clone + Send + Sync + 'static,
{
    fn name(&self) -> &'static str {
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
impl<SealState, Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList> PluginDeps
    for KelirBuilder<Mode, SealState, Ctx, Deps, Plugins, Params>
{
    type Deps = Deps;
}
