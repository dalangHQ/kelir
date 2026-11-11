// see: cans/02-plugin-model.md#KelirPlugin-trait
// see: cans/02-plugin-model.md#Segregated-ring-traits
use std::future::Future;
use std::marker::PhantomData;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use kelir_interface::driver_interface::{WatchdogEvent, WatchdogVerdict};
use kelir_interface::plugin_model::{
    AnyPlugin, ConsoleInterposer, FsInterposer, HandlerCtx, HostInterposer, KelirPlugin,
    LifecycleCtx, LifecycleParticipant, MountScope, NetInterposer, PipelineBinder, PluginClone,
    PluginDeps, PluginError, SystemTelemetryParticipant, TrapInterposer,
};
use kelir_interface::protocol_wire::{FilterAction, ProcessExit, TrapEvent, Verdict};
use kelir_interface::vsock_mesh::{VsockServiceHandler, VsockServiceSpec};

use crate::hlist_sdk::{HCons, HList, HNil, Satisfies};

// see: cans/plugins/tool-guard.md#Host-extensible-hook-pipeline
#[cfg(feature = "plugin-tool-guard")]
use crate::builder_sdk::{KelirBuilder, Unsealed};
#[cfg(feature = "plugin-tool-guard")]
use kelir_interface::plugin_model::CompositePlugin;
#[cfg(feature = "plugin-tool-guard")]
use plugin_tool_guard::{ToolGuardPlugin, ToolSpec};

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
impl<F> LifecycleParticipant for LifecycleClosureAdapter<F, OnVmInit>
where
    F: for<'c, 'i> Fn(
            &'c mut LifecycleCtx<'i>,
        )
            -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + 'c>>
        + Send
        + Sync,
{
    async fn on_vm_init(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        (self.handler)(ctx).await
    }
}

#[async_trait]
impl<F> LifecycleParticipant for LifecycleClosureAdapter<F, OnVmReady>
where
    F: for<'c, 'i> Fn(
            &'c mut LifecycleCtx<'i>,
        )
            -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + 'c>>
        + Send
        + Sync,
{
    async fn on_vm_ready(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        (self.handler)(ctx).await
    }
}

#[async_trait]
impl<F> LifecycleParticipant for LifecycleClosureAdapter<F, OnVmShutdown>
where
    F: for<'c, 'i> Fn(
            &'c mut LifecycleCtx<'i>,
        )
            -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + 'c>>
        + Send
        + Sync,
{
    async fn on_vm_shutdown(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        (self.handler)(ctx).await
    }
}

#[async_trait]
impl<F> LifecycleParticipant for LifecycleClosureAdapter<F, OnPreCommit>
where
    F: for<'c, 'i> Fn(
            &'c mut LifecycleCtx<'i>,
        )
            -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + 'c>>
        + Send
        + Sync,
{
    async fn on_pre_commit(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        (self.handler)(ctx).await
    }
}

#[async_trait]
impl<F> LifecycleParticipant for LifecycleClosureAdapter<F, OnPostCommit>
where
    F: for<'c, 'i> Fn(
            &'c mut LifecycleCtx<'i>,
            bool,
        )
            -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + 'c>>
        + Send
        + Sync,
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
impl<F> LifecycleParticipant for LifecycleClosureAdapter<F, OnHostPreempt>
where
    F: for<'c, 'i> Fn(
            &'c mut LifecycleCtx<'i>,
        )
            -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + 'c>>
        + Send
        + Sync,
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
impl<F> SystemTelemetryParticipant for TelemetryClosureAdapter<F, OnWatchdog>
where
    F: for<'a> Fn(
            &'a WatchdogEvent,
        ) -> Pin<
            Box<dyn Future<Output = Result<WatchdogVerdict, PluginError>> + Send + 'a>,
        > + Send
        + Sync,
{
    async fn on_watchdog(&self, event: &WatchdogEvent) -> Result<WatchdogVerdict, PluginError> {
        (self.handler)(event).await
    }
}

// see: cans/02-plugin-model.md#KelirPlugin-trait
pub struct PluginBuilder<Ctx, Deps: HList> {
    name: &'static str,
    context_factory: Arc<dyn Fn() -> Ctx + Send + Sync>,
    scope: MountScope,
    fork: Option<ForkFn>,
    trap: Vec<Arc<dyn TrapInterposer>>,
    fs: Vec<Arc<dyn FsInterposer>>,
    net: Vec<Arc<dyn NetInterposer>>,
    console: Vec<Arc<dyn ConsoleInterposer>>,
    host: Vec<Arc<dyn HostInterposer>>,
    lifecycle: Vec<Arc<dyn LifecycleParticipant>>,
    telemetry: Vec<Arc<dyn SystemTelemetryParticipant>>,
    vsock: Vec<(VsockServiceSpec, Arc<dyn VsockServiceHandler>)>,
    deps: PhantomData<fn() -> Deps>,
}

impl PluginBuilder<(), HNil> {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            context_factory: Arc::new(|| ()),
            scope: MountScope::default(),
            fork: None,
            trap: Vec::new(),
            fs: Vec::new(),
            net: Vec::new(),
            console: Vec::new(),
            host: Vec::new(),
            lifecycle: Vec::new(),
            telemetry: Vec::new(),
            vsock: Vec::new(),
            deps: PhantomData,
        }
    }
}

impl<Ctx, Deps: HList> PluginBuilder<Ctx, Deps> {
    fn rebuild<NextCtx, NextDeps: HList>(
        self,
        context_factory: Arc<dyn Fn() -> NextCtx + Send + Sync>,
    ) -> PluginBuilder<NextCtx, NextDeps> {
        PluginBuilder {
            name: self.name,
            context_factory,
            scope: self.scope,
            fork: self.fork,
            trap: self.trap,
            fs: self.fs,
            net: self.net,
            console: self.console,
            host: self.host,
            lifecycle: self.lifecycle,
            telemetry: self.telemetry,
            vsock: self.vsock,
            deps: PhantomData,
        }
    }

    pub fn context<NextCtx>(
        self,
        factory: impl Fn() -> NextCtx + Send + Sync + 'static,
    ) -> PluginBuilder<NextCtx, Deps>
    where
        NextCtx: Clone + Send + Sync + 'static,
    {
        self.rebuild(Arc::new(factory))
    }

    pub fn require<P>(self) -> PluginBuilder<Ctx, HCons<P::Context, Deps>>
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
        self.vsock.push((spec, Arc::new(handler)));
        self
    }

    pub fn on_trap<F>(mut self, f: F) -> Self
    where
        F: Fn(&TrapEvent, &HandlerCtx) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.trap.push(Arc::new(TrapClosureAdapter::<_, OnTrap> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_process_exit<F>(mut self, f: F) -> Self
    where
        F: Fn(&ProcessExit, &HandlerCtx) -> Result<(), PluginError> + Send + Sync + 'static,
    {
        self.trap
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
        self.fs.push(Arc::new(FsClosureAdapter::<_, OnFsLookup> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_fs_read<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, &mut [u8]) -> Result<(), PluginError> + Send + Sync + 'static,
    {
        self.fs.push(Arc::new(FsClosureAdapter::<_, OnFsRead> {
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
        self.fs.push(Arc::new(FsClosureAdapter::<_, OnFsWrite> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_fs_lock<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, i32, i16, u32) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.fs.push(Arc::new(FsClosureAdapter::<_, OnFsLock> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_net_outbound<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut [u8]) -> Result<FilterAction, PluginError> + Send + Sync + 'static,
    {
        self.net
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
        self.net
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
        self.console
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
        self.console
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
        self.console
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
        self.console
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
        self.host
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
        self.host
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
        self.host
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
        self.lifecycle
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
        self.lifecycle
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
        self.lifecycle
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
        self.lifecycle
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
        self.lifecycle
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
        self.lifecycle
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
        self.telemetry
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
        self.telemetry
            .push(Arc::new(TelemetryClosureAdapter::<_, OnHeartbeat> {
                handler: f,
                verb: PhantomData,
            }));
        self
    }

    pub fn on_watchdog<F>(mut self, f: F) -> Self
    where
        F: for<'a> Fn(
                &'a WatchdogEvent,
            ) -> Pin<
                Box<dyn Future<Output = Result<WatchdogVerdict, PluginError>> + Send + 'a>,
            > + Send
            + Sync
            + 'static,
    {
        self.telemetry
            .push(Arc::new(TelemetryClosureAdapter::<_, OnWatchdog> {
                handler: f,
                verb: PhantomData,
            }));
        self
    }

    pub fn build(self) -> PluginWrapper<Ctx, Deps> {
        PluginWrapper {
            name: self.name,
            context_factory: self.context_factory,
            scope: self.scope,
            fork: self.fork,
            trap: self.trap,
            fs: self.fs,
            net: self.net,
            console: self.console,
            host: self.host,
            lifecycle: self.lifecycle,
            telemetry: self.telemetry,
            vsock: self.vsock,
            deps: PhantomData,
        }
    }
}

// see: cans/02-plugin-model.md#KelirPlugin-trait
#[derive(Clone)]
pub struct PluginWrapper<Ctx, Deps: HList> {
    name: &'static str,
    context_factory: Arc<dyn Fn() -> Ctx + Send + Sync>,
    scope: MountScope,
    fork: Option<ForkFn>,
    trap: Vec<Arc<dyn TrapInterposer>>,
    fs: Vec<Arc<dyn FsInterposer>>,
    net: Vec<Arc<dyn NetInterposer>>,
    console: Vec<Arc<dyn ConsoleInterposer>>,
    host: Vec<Arc<dyn HostInterposer>>,
    lifecycle: Vec<Arc<dyn LifecycleParticipant>>,
    telemetry: Vec<Arc<dyn SystemTelemetryParticipant>>,
    vsock: Vec<(VsockServiceSpec, Arc<dyn VsockServiceHandler>)>,
    deps: PhantomData<fn() -> Deps>,
}

impl<Ctx, Deps: HList> KelirPlugin for PluginWrapper<Ctx, Deps>
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
        for stage in &self.trap {
            pipeline.bind_trap_scoped(Arc::clone(stage), self.scope.clone());
        }
        for stage in &self.fs {
            pipeline.bind_fs_scoped(Arc::clone(stage), self.scope.clone());
        }
        for stage in &self.net {
            pipeline.bind_net_scoped(Arc::clone(stage), self.scope.clone());
        }
        for stage in &self.console {
            pipeline.bind_console_scoped(Arc::clone(stage), self.scope.clone());
        }
        for stage in &self.host {
            pipeline.bind_host_scoped(Arc::clone(stage), self.scope.clone());
        }
        for (spec, service) in &self.vsock {
            pipeline.bind_vsock_service_scoped(
                spec.clone(),
                Arc::clone(service),
                self.scope.clone(),
            );
        }
        for stage in &self.lifecycle {
            pipeline.bind_lifecycle_scoped(Arc::clone(stage), self.scope.clone());
        }
        for stage in &self.telemetry {
            pipeline.bind_telemetry_scoped(Arc::clone(stage), self.scope.clone());
        }
    }
}

// see: cans/02-plugin-model.md#KelirPlugin-trait
impl<Ctx, Deps: HList> PluginDeps for PluginWrapper<Ctx, Deps> {
    type Deps = Deps;
}

// see: cans/02-plugin-model.md#Compile-time-prerequisites
impl<Plugins: HList> Satisfies<kelir_interface::plugin_model::HNil> for Plugins {}

// see: cans/02-plugin-model.md#Compile-time-prerequisites
impl<Head, Tail, OtherTail> Satisfies<kelir_interface::plugin_model::HCons<Head, OtherTail>>
    for HCons<Head, Tail>
where
    Head: Clone + Send + Sync + 'static,
    Tail: HList + Satisfies<OtherTail>,
{
}

// see: cans/plugins/tool-guard.md#Host-extensible-hook-pipeline
#[cfg(feature = "plugin-tool-guard")]
pub trait ToolGuardExt: Sized {
    fn tool(self, spec: ToolSpec) -> Self;
}

// see: cans/plugins/tool-guard.md#Host-extensible-hook-pipeline
#[cfg(feature = "plugin-tool-guard")]
struct ToolGuardBundle {
    plugin: ToolGuardPlugin,
}

// see: cans/plugins/tool-guard.md#Host-extensible-hook-pipeline
#[cfg(feature = "plugin-tool-guard")]
impl CompositePlugin for ToolGuardBundle {
    fn unpack(self: Box<Self>, parent_scope: &MountScope) -> Vec<(Box<dyn AnyPlugin>, MountScope)> {
        vec![(Box::new(self.plugin), parent_scope.clone())]
    }
}

// see: cans/plugins/tool-guard.md#Host-extensible-hook-pipeline
#[cfg(feature = "plugin-tool-guard")]
impl<Plugins: HList, Params: 'static> ToolGuardExt for KelirBuilder<Unsealed, Plugins, Params> {
    fn tool(self, spec: ToolSpec) -> Self {
        let plugin = ToolGuardPlugin::new().tool(spec);
        let scope = KelirPlugin::default_scope(&plugin);
        self.with_composite(ToolGuardBundle { plugin }, scope)
    }
}
