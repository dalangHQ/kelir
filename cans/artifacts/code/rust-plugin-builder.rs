// ILLUSTRATIVE, NON-STANDALONE — do not compile this file directly.
// It pins the fluent plugin-creation builder surface; the live shape is
// crates/kelir/src/plugin_builder.rs (Agent 3) and the normative contract is
// cans/02-plugin-model.md#Fluent-plugin-creation-builder (Agent 1).
//
// Target shape: the 30-verb closed set (6 structural + 24 ring), the 8
// closure-scoped entry points, and the validating build. The scope entry
// points and the Result build are the A3 contract (DX-3): they do not exist
// on the live builder yet. Flat verbs stay; a nested chain and the
// equivalent flat chain produce identical descriptors (same rings, same
// ordinals). Post-.build() calls are type errors: build consumes self and
// returns PluginWrapper, which exposes no verb methods.
//
// Owned by: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// see: cans/03-sdk-architecture.md#Fluent-plugin-surface

use std::future::Future;
use std::marker::PhantomData;
use std::path::Path;
use std::sync::Arc;

use kelir_interface::driver_interface::{WatchdogEvent, WatchdogVerdict};
use kelir_interface::plugin_model::{
    AnyPlugin, ConsoleInterposer, FsInterposer, HandlerCtx, HostInterposer, KelirPlugin,
    LifecycleCtx, LifecycleParticipant, MountScope, NetInterposer, PipelineBinder, PluginDeps,
    PluginError, SystemTelemetryParticipant, TrapInterposer,
};
use kelir_interface::protocol_wire::{FilterAction, ProcessExit, TrapEvent, Verdict};
use kelir_interface::vsock_mesh::{VsockServiceHandler, VsockServiceSpec};

use crate::hlist_sdk::{HCons, HList, HNil};

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

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
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

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
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

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
pub struct TrapScope<'a> {
    trap: &'a mut Vec<Arc<dyn TrapInterposer>>,
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
pub struct FsScope<'a> {
    fs: &'a mut Vec<Arc<dyn FsInterposer>>,
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
pub struct NetScope<'a> {
    net: &'a mut Vec<Arc<dyn NetInterposer>>,
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
pub struct ConsoleScope<'a> {
    console: &'a mut Vec<Arc<dyn ConsoleInterposer>>,
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
pub struct ExecScope<'a> {
    console: &'a mut Vec<Arc<dyn ConsoleInterposer>>,
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
pub struct HostScope<'a> {
    host: &'a mut Vec<Arc<dyn HostInterposer>>,
    lifecycle: &'a mut Vec<Arc<dyn LifecycleParticipant>>,
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
pub struct LifecycleScope<'a> {
    lifecycle: &'a mut Vec<Arc<dyn LifecycleParticipant>>,
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
pub struct TelemetryScope<'a> {
    telemetry: &'a mut Vec<Arc<dyn SystemTelemetryParticipant>>,
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
impl<'a> TrapScope<'a> {
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
        self.trap.push(Arc::new(TrapClosureAdapter::<_, OnProcessExit> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
impl<'a> FsScope<'a> {
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
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
impl<'a> NetScope<'a> {
    pub fn on_net_outbound<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut [u8]) -> Result<FilterAction, PluginError> + Send + Sync + 'static,
    {
        self.net.push(Arc::new(NetClosureAdapter::<_, OnNetOutbound> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_net_inbound<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut [u8]) -> Result<FilterAction, PluginError> + Send + Sync + 'static,
    {
        self.net.push(Arc::new(NetClosureAdapter::<_, OnNetInbound> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
impl<'a> ConsoleScope<'a> {
    pub fn on_console_input<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, &mut Vec<u8>) -> Result<(), PluginError> + Send + Sync + 'static,
    {
        self.console.push(Arc::new(ConsoleClosureAdapter::<_, OnConsoleInput> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_console_output<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, &mut Vec<u8>) -> Result<(), PluginError> + Send + Sync + 'static,
    {
        self.console.push(Arc::new(ConsoleClosureAdapter::<_, OnConsoleOutput> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
impl<'a> ExecScope<'a> {
    pub fn on_exec_stdout<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, u64, &mut Vec<u8>) -> Result<(), PluginError> + Send + Sync + 'static,
    {
        self.console.push(Arc::new(ConsoleClosureAdapter::<_, OnExecStdout> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_exec_stderr<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, u64, &mut Vec<u8>) -> Result<(), PluginError> + Send + Sync + 'static,
    {
        self.console.push(Arc::new(ConsoleClosureAdapter::<_, OnExecStderr> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// on_host_preempt is reachable via .host() but binds the lifecycle ring:
// its adapter is a LifecycleParticipant, so HostScope carries both vecs.
impl<'a> HostScope<'a> {
    pub fn on_host_lsm_exec<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, u32) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.host.push(Arc::new(HostClosureAdapter::<_, OnHostLsmExec> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_host_pressure<F>(mut self, f: F) -> Self
    where
        F: Fn(u32) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.host.push(Arc::new(HostClosureAdapter::<_, OnHostPressure> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_host_keyring_access<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.host.push(Arc::new(HostClosureAdapter::<_, OnHostKeyringAccess> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_host_preempt<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(&mut LifecycleCtx<'_>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.lifecycle.push(Arc::new(LifecycleClosureAdapter::<_, OnHostPreempt> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
impl<'a> LifecycleScope<'a> {
    pub fn on_vm_init<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(&mut LifecycleCtx<'_>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.lifecycle.push(Arc::new(LifecycleClosureAdapter::<_, OnVmInit> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_vm_ready<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(&mut LifecycleCtx<'_>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.lifecycle.push(Arc::new(LifecycleClosureAdapter::<_, OnVmReady> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_vm_shutdown<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(&mut LifecycleCtx<'_>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.lifecycle.push(Arc::new(LifecycleClosureAdapter::<_, OnVmShutdown> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_pre_commit<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(&mut LifecycleCtx<'_>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.lifecycle.push(Arc::new(LifecycleClosureAdapter::<_, OnPreCommit> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }

    pub fn on_post_commit<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(&mut LifecycleCtx<'_>, bool) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.lifecycle.push(Arc::new(LifecycleClosureAdapter::<_, OnPostCommit> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
impl<'a> TelemetryScope<'a> {
    pub fn on_memory_pressure<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(u32, u32) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.telemetry.push(Arc::new(TelemetryClosureAdapter::<_, OnMemoryPressure> {
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
        self.telemetry.push(Arc::new(TelemetryClosureAdapter::<_, OnHeartbeat> {
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
        self.telemetry.push(Arc::new(TelemetryClosureAdapter::<_, OnWatchdog> {
            handler: f,
            verb: PhantomData,
        }));
        self
    }
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
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

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
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
        self.trap(|s| s.on_trap(f))
    }

    pub fn on_process_exit<F>(mut self, f: F) -> Self
    where
        F: Fn(&ProcessExit, &HandlerCtx) -> Result<(), PluginError> + Send + Sync + 'static,
    {
        self.trap(|s| s.on_process_exit(f))
    }

    pub fn on_fs_lookup<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.fs(|s| s.on_fs_lookup(f))
    }

    pub fn on_fs_read<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, &mut [u8]) -> Result<(), PluginError> + Send + Sync + 'static,
    {
        self.fs(|s| s.on_fs_read(f))
    }

    pub fn on_fs_write<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, &mut [u8], u32, &HandlerCtx) -> Result<FilterAction, PluginError>
            + Send
            + Sync
            + 'static,
    {
        self.fs(|s| s.on_fs_write(f))
    }

    pub fn on_fs_lock<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, i32, i16, u32) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.fs(|s| s.on_fs_lock(f))
    }

    pub fn on_net_outbound<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut [u8]) -> Result<FilterAction, PluginError> + Send + Sync + 'static,
    {
        self.net(|s| s.on_net_outbound(f))
    }

    pub fn on_net_inbound<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut [u8]) -> Result<FilterAction, PluginError> + Send + Sync + 'static,
    {
        self.net(|s| s.on_net_inbound(f))
    }

    pub fn on_console_input<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, &mut Vec<u8>) -> Result<(), PluginError> + Send + Sync + 'static,
    {
        self.console(|s| s.on_console_input(f))
    }

    pub fn on_console_output<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, &mut Vec<u8>) -> Result<(), PluginError> + Send + Sync + 'static,
    {
        self.console(|s| s.on_console_output(f))
    }

    pub fn on_exec_stdout<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, u64, &mut Vec<u8>) -> Result<(), PluginError> + Send + Sync + 'static,
    {
        self.exec(|s| s.on_exec_stdout(f))
    }

    pub fn on_exec_stderr<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, u64, &mut Vec<u8>) -> Result<(), PluginError> + Send + Sync + 'static,
    {
        self.exec(|s| s.on_exec_stderr(f))
    }

    pub fn on_host_lsm_exec<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, u32) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.host(|s| s.on_host_lsm_exec(f))
    }

    pub fn on_host_pressure<F>(mut self, f: F) -> Self
    where
        F: Fn(u32) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.host(|s| s.on_host_pressure(f))
    }

    pub fn on_host_keyring_access<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.host(|s| s.on_host_keyring_access(f))
    }

    pub fn on_vm_init<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(&mut LifecycleCtx<'_>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.lifecycle(|s| s.on_vm_init(f))
    }

    pub fn on_vm_ready<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(&mut LifecycleCtx<'_>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.lifecycle(|s| s.on_vm_ready(f))
    }

    pub fn on_vm_shutdown<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(&mut LifecycleCtx<'_>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.lifecycle(|s| s.on_vm_shutdown(f))
    }

    pub fn on_pre_commit<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(&mut LifecycleCtx<'_>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.lifecycle(|s| s.on_pre_commit(f))
    }

    pub fn on_post_commit<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(&mut LifecycleCtx<'_>, bool) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.lifecycle(|s| s.on_post_commit(f))
    }

    pub fn on_host_preempt<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(&mut LifecycleCtx<'_>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.host(|s| s.on_host_preempt(f))
    }

    pub fn on_memory_pressure<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(u32, u32) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.telemetry(|s| s.on_memory_pressure(f))
    }

    pub fn on_heartbeat<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(u64) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.telemetry(|s| s.on_heartbeat(f))
    }

    pub fn on_watchdog<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(&WatchdogEvent) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<WatchdogVerdict, PluginError>> + Send + 'static,
    {
        self.telemetry(|s| s.on_watchdog(f))
    }

    pub fn trap<F>(mut self, scope_fn: F) -> Self
    where
        F: for<'a> FnOnce(TrapScope<'a>) -> TrapScope<'a>,
    {
        let _ = scope_fn(TrapScope { trap: &mut self.trap });
        self
    }

    pub fn fs<F>(mut self, scope_fn: F) -> Self
    where
        F: for<'a> FnOnce(FsScope<'a>) -> FsScope<'a>,
    {
        let _ = scope_fn(FsScope { fs: &mut self.fs });
        self
    }

    pub fn net<F>(mut self, scope_fn: F) -> Self
    where
        F: for<'a> FnOnce(NetScope<'a>) -> NetScope<'a>,
    {
        let _ = scope_fn(NetScope { net: &mut self.net });
        self
    }

    pub fn console<F>(mut self, scope_fn: F) -> Self
    where
        F: for<'a> FnOnce(ConsoleScope<'a>) -> ConsoleScope<'a>,
    {
        let _ = scope_fn(ConsoleScope {
            console: &mut self.console,
        });
        self
    }

    pub fn exec<F>(mut self, scope_fn: F) -> Self
    where
        F: for<'a> FnOnce(ExecScope<'a>) -> ExecScope<'a>,
    {
        let _ = scope_fn(ExecScope {
            console: &mut self.console,
        });
        self
    }

    pub fn host<F>(mut self, scope_fn: F) -> Self
    where
        F: for<'a> FnOnce(HostScope<'a>) -> HostScope<'a>,
    {
        let _ = scope_fn(HostScope {
            host: &mut self.host,
            lifecycle: &mut self.lifecycle,
        });
        self
    }

    pub fn lifecycle<F>(mut self, scope_fn: F) -> Self
    where
        F: for<'a> FnOnce(LifecycleScope<'a>) -> LifecycleScope<'a>,
    {
        let _ = scope_fn(LifecycleScope {
            lifecycle: &mut self.lifecycle,
        });
        self
    }

    pub fn telemetry<F>(mut self, scope_fn: F) -> Self
    where
        F: for<'a> FnOnce(TelemetryScope<'a>) -> TelemetryScope<'a>,
    {
        let _ = scope_fn(TelemetryScope {
            telemetry: &mut self.telemetry,
        });
        self
    }

    pub fn build(self) -> Result<PluginWrapper<Ctx, Deps>, PluginError> {
        for (spec, _) in &self.vsock {
            if spec.port == 0 || spec.port > 65535 {
                return Err(PluginError::reject(
                    "EINVAL",
                    "vsock service port outside 1..=65535",
                ));
            }
            if spec.service.is_empty() {
                return Err(PluginError::reject(
                    "EINVAL",
                    "vsock service declares an empty service name",
                ));
            }
        }
        if let Some(paths) = &self.scope.paths {
            for path in paths {
                if path.as_os_str().is_empty() {
                    return Err(PluginError::reject(
                        "EINVAL",
                        "mount scope declares an empty path",
                    ));
                }
            }
        }
        Ok(PluginWrapper {
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
        })
    }
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
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

    fn fork_state(&self, _child_session_id: &str) -> Box<dyn AnyPlugin> {
        match &self.fork {
            Some(fork) => fork(_child_session_id),
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

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
impl<Ctx, Deps: HList> PluginDeps for PluginWrapper<Ctx, Deps> {
    type Deps = Deps;
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// Ring resolution at .build() holds by construction: every verb is typed,
// so no registered verb can name an unknown ring. The missing-prerequisite
// check stays at KelirBuilder::plugin() (Satisfies<P::Deps>), reported as
// `missing prerequisite: <dep> required by <plugin>`. An empty plugin (no
// handlers) still builds. VsockServiceSpec is (port: u32, service: String);
// it carries no schema field live, so the (port, service, schema) spec is
// plugin-level in TS/Python and (port, service) in Rust.
