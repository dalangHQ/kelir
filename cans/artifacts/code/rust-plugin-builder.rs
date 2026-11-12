// ILLUSTRATIVE, NON-STANDALONE — do not compile this file directly.
// It pins the unified builder surface; the live shape is
// crates/kelir/src/builder_sdk.rs + crates/kelir/src/builder_plugin.rs
// and the normative contract is cans/02-plugin-model.md#Unified-builder.
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
// host-only verbs record their use on a plugin-mode builder and seal()
// rejects them; params() is host-only by type (the arity pin).
//
// Owned by: cans/02-plugin-model.md#Unified-builder
// see: cans/02-plugin-model.md#Unified-builder
// see: cans/03-sdk-architecture.md#Unified-builder-surface

use std::future::Future;
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
use kelir_interface::protocol_wire::{FilterAction, ProcessExit, TrapEvent, Verdict};
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
// One struct, two modes. new() is the host (config Some); named() is a
// plugin-mode builder (config None, name Some). The seven vecs are the
// single hook registry for both faces: the host face's ring verbs append
// here, exactly like the plugin face's. Registration keeps plane config
// and host-only registrations only; the former on_vm_init / on_vm_ready
// / on_vm_shutdown single-handler fields were folded into the lifecycle
// vec, and the on_vm_init / on_vm_ready / on_vm_shutdown methods remain
// as the ring verbs that append to it. Registration, TrapFilter,
// CustomActionEntry,
// StreamKindServiceEntry, and LifecycleMut are defined in builder_sdk.rs
// alongside the builder; the trap-closure adapter (TrapClosureAdapter) and
// its OnTrap/OnProcessExit verbs live in builder_plugin.rs and register on
// the HookRegistry vec — one trap path, one engine dispatch point.
// VmConfig has no Default live, so the host config
// is Option: Some on the host face, None on the plugin face.
#[derive(Clone)]
pub struct KelirBuilder {
    config: Option<VmConfig>,
    plugins: Plugins,
    registration: Registration,
    registry: HookRegistry,
    fast_boot: FastBootMode,
    dax_window: DaxWindowMode,
    name: Option<&'static str>,
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
    mounted: Vec<MountedPlugin>,
    deps: PhantomData<fn() -> Deps>,
}

// see: cans/02-plugin-model.md#Unified-builder
struct MountedPlugin {
    name: &'static str,
    host_only_verbs: Vec<&'static str>,
    vsock: Vec<VsockServiceSpec>,
    scope: MountScope,
}

// see: cans/02-plugin-model.md#Unified-builder
pub struct TrapScope<'a> {
    trap: &'a mut Vec<Arc<dyn TrapInterposer>>,
}

// see: cans/02-plugin-model.md#Unified-builder
pub struct FsScope<'a> {
    fs: &'a mut Vec<Arc<dyn FsInterposer>>,
}

// see: cans/02-plugin-model.md#Unified-builder
pub struct NetScope<'a> {
    net: &'a mut Vec<Arc<dyn NetInterposer>>,
}

// see: cans/02-plugin-model.md#Unified-builder
pub struct ConsoleScope<'a> {
    console: &'a mut Vec<Arc<dyn ConsoleInterposer>>,
}

// see: cans/02-plugin-model.md#Unified-builder
pub struct ExecScope<'a> {
    console: &'a mut Vec<Arc<dyn ConsoleInterposer>>,
}

// see: cans/02-plugin-model.md#Unified-builder
pub struct HostScope<'a> {
    host: &'a mut Vec<Arc<dyn HostInterposer>>,
    lifecycle: &'a mut Vec<Arc<dyn LifecycleParticipant>>,
}

// see: cans/02-plugin-model.md#Unified-builder
pub struct LifecycleScope<'a> {
    lifecycle: &'a mut Vec<Arc<dyn LifecycleParticipant>>,
}

// see: cans/02-plugin-model.md#Unified-builder
pub struct TelemetryScope<'a> {
    telemetry: &'a mut Vec<Arc<dyn SystemTelemetryParticipant>>,
}

// see: cans/02-plugin-model.md#Unified-builder
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

// see: cans/02-plugin-model.md#Unified-builder
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

// see: cans/02-plugin-model.md#Unified-builder
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

// see: cans/02-plugin-model.md#Unified-builder
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

// see: cans/02-plugin-model.md#Unified-builder
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

// see: cans/02-plugin-model.md#Unified-builder
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

// see: cans/02-plugin-model.md#Unified-builder
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

// see: cans/02-plugin-model.md#Unified-builder
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

// see: cans/02-plugin-model.md#Unified-builder
impl KelirBuilder {
    pub fn new(config: VmConfig) -> Self {
        Self::with_config(Some(config), None)
    }

    pub fn named(name: &'static str) -> Self {
        Self::with_config(None, Some(name))
    }

    fn with_config(config: Option<VmConfig>, name: Option<&'static str>) -> Self {
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
            trap: Vec::new(),
            fs: Vec::new(),
            net: Vec::new(),
            console: Vec::new(),
            host: Vec::new(),
            lifecycle: Vec::new(),
            telemetry: Vec::new(),
            vsock: Vec::new(),
            mounted: Vec::new(),
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
            trap: self.trap,
            fs: self.fs,
            net: self.net,
            console: self.console,
            host: self.host,
            lifecycle: self.lifecycle,
            telemetry: self.telemetry,
            vsock: self.vsock,
            mounted: self.mounted,
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
}

// see: cans/02-plugin-model.md#Unified-builder
// Mount. plugin() merges a plugin-mode builder by value: its hook vecs
// append after the host's own hooks (host hooks first, then plugins in
// mount order; within a plugin, registration order), its vsock services
// append after the host's, and its mounted records flatten into the
// host's (plugin-into-plugin mount composes the same way). plugin() and
// plugin_instance() return Result: mounting the same plugin twice is
// an error. The live plugin() was infallible; the duplicate check is
// runtime, so the unified mount returns Result.
impl KelirBuilder {
    pub fn plugin(mut self, plugin: Self) -> Result<Self, PluginError> {
        let name = match plugin.name {
            Some(name) => name,
            None => {
                return Err(PluginError::reject(
                    "EINVAL",
                    "plugin() mounts a plugin-mode builder: use named()",
                ));
            }
        };
        if self.mounted.iter().any(|mounted| mounted.name == name) {
            return Err(PluginError::reject(
                "EINVAL",
                &format!("plugin already mounted: {name}"),
            ));
        }
        let mut host_only_verbs: Vec<&'static str> = Vec::new();
        if plugin.registration.storage_cfg.is_some() {
            host_only_verbs.push("storage");
        }
        if plugin.registration.terminal_cfg.is_some() {
            host_only_verbs.push("terminal");
        }
        if plugin.registration.wire_cfg.is_some() {
            host_only_verbs.push("wire");
        }
        if plugin.registration.lock_broker_cfg.is_some() {
            host_only_verbs.push("lock_broker");
        }
        if plugin.registration.detached {
            host_only_verbs.push("detached");
        }
        if !matches!(plugin.fast_boot, FastBootMode::Off) {
            host_only_verbs.push("fast_boot");
        }
        if !matches!(plugin.dax_window, DaxWindowMode::Off) {
            host_only_verbs.push("dax_window");
        }
        // trap(filter, handler) is host-only by impl block: the plugin-mode
        // builder exposes trap(|scope| ...) instead, so the compiler (E0592)
        // pins the split — no runtime check, no Registration.traps field.
        if plugin.registration.on_trap_deny.is_some() {
            host_only_verbs.push("on_trap_deny");
        }
        if !plugin.registration.custom_actions.is_empty() {
            host_only_verbs.push("register_custom_action");
        }
        if !plugin.registration.stream_kind_services.is_empty() {
            host_only_verbs.push("register_stream_kind");
        }
        if !plugin.registration.vetoes.is_empty() {
            host_only_verbs.push("lifecycle_mut");
        }
        let vsock_specs: Vec<VsockServiceSpec> = plugin
            .vsock
            .iter()
            .map(|(spec, _)| spec.clone())
            .collect();
        let scope = plugin.scope.clone();
        self.trap.extend(plugin.trap);
        self.fs.extend(plugin.fs);
        self.net.extend(plugin.net);
        self.console.extend(plugin.console);
        self.host.extend(plugin.host);
        self.lifecycle.extend(plugin.lifecycle);
        self.telemetry.extend(plugin.telemetry);
        self.vsock.extend(plugin.vsock);
        self.mounted.push(MountedPlugin {
            name,
            host_only_verbs,
            vsock: vsock_specs,
            scope,
        });
        self.mounted.extend(plugin.mounted);
        Ok(self)
    }

    pub fn plugin_instance<P>(mut self, plugin: P) -> Result<Self, PluginError>
    where
        P: KelirPlugin + PluginDeps + Clone + 'static,
    {
        let name = plugin.name();
        if self.mounted.iter().any(|mounted| mounted.name == name) {
            return Err(PluginError::reject(
                "EINVAL",
                &format!("plugin already mounted: {name}"),
            ));
        }
        let context = plugin.context();
        let scope = plugin.default_scope();
        let mut registration = self.registration.narrow::<P::Context>();
        registration.instances.push((Box::new(plugin), scope.clone()));
        self.plugins = HCons {
            head: context,
            tail: self.plugins,
        };
        self.registration = registration;
        self.mounted.push(MountedPlugin {
            name,
            host_only_verbs: Vec::new(),
            vsock: Vec::new(),
            scope,
        });
        Ok(self)
    }
}

// see: cans/02-plugin-model.md#Unified-builder
// Host-only verbs. They exist on the one struct (like Elysia .listen())
// and are simply not called on a plugin-mode builder; a call on one is
// recorded in its registration and seal() rejects it with the plugin
// named. params() is host-only by type (the arity pin). The live host's
// on_vm_init / on_vm_ready / on_vm_shutdown methods are the ring verbs
// above (append, plain async fn); on_trap_deny stays: it is not one of
// the 24 rings.
impl KelirBuilder {
    pub fn storage(mut self, config: StoragePlaneConfig<Plugins, Params>) -> Self {
        self.registration.storage_cfg = Some(config.retag());
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

    pub fn on_trap_deny<F, Fut, T>(mut self, f: F) -> Self
    where
        F: Fn(&TrapEvent, &kelir_core::Session) -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = Result<T, PluginError>> + Send + 'static,
    {
        self.registration.on_trap_deny = Some(Box::new(
            move |event: &TrapEvent, session: &kelir_core::Session| {
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
    // The one validation. The live seal() was infallible; the unified
    // seal() validates and returns Result. (a) vsock triple and (b) scope
    // triple over the host's own registrations and every mounted
    // plugin's, with the plugin named on failure. (c) deps: enforced at
    // mount by the Satisfies bound on plugin() / plugin_instance(); there
    // is no runtime deps check in Rust, the uniform message is the
    // TS/Python runtime form. (d) host-only verbs recorded on a
    // plugin-mode builder, rejected here with the plugin named.
    pub fn seal(self) -> Result<Self, PluginError> {
        if self.config.is_none() {
            return Err(PluginError::reject(
                "EINVAL",
                "seal requires a host-mode builder",
            ));
        }
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
        for mounted in &self.mounted {
            for spec in &mounted.vsock {
                if spec.port == 0 || spec.port > 65535 {
                    return Err(PluginError::reject(
                        "EINVAL",
                        &format!(
                            "{}: vsock service port outside 1..=65535",
                            mounted.name
                        ),
                    ));
                }
                if spec.service.is_empty() {
                    return Err(PluginError::reject(
                        "EINVAL",
                        &format!(
                            "{}: vsock service declares an empty service name",
                            mounted.name
                        ),
                    ));
                }
            }
            if let Some(paths) = &mounted.scope.paths {
                for path in paths {
                    if path.as_os_str().is_empty() {
                        return Err(PluginError::reject(
                            "EINVAL",
                            &format!("{}: mount scope declares an empty path", mounted.name),
                        ));
                    }
                }
            }
            if let Some(verb) = mounted.host_only_verbs.first() {
                return Err(PluginError::reject(
                    "EINVAL",
                    &format!(
                        "{}: host-only verb '{verb}' used on a plugin-mode builder",
                        mounted.name
                    ),
                ));
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
        self.name.unwrap_or("")
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

// see: cans/02-plugin-model.md#Unified-builder
impl PluginDeps for KelirBuilder {
    type Deps = Deps;
}

// see: cans/02-plugin-model.md#Unified-builder
// Deleted shapes: PluginBuilder, PluginWrapper, plugin_builder_scopes,
// the duplicated Satisfies (one Satisfies lives in hlist_sdk.rs),
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
// <name>`; the missing-prerequisite message is `missing prerequisite:
// <dep> required by <plugin>` (runtime form in TS/Python; Satisfies
// bound in Rust).
