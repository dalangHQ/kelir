// ILLUSTRATIVE, NON-STANDALONE — do not compile this file directly.
// It pins the fluent plugin-creation builder surface; the live shape is
// crates/kelir/src/plugin_builder.rs (Agent 3) and the normative contract is
// cans/02-plugin-model.md#Fluent-plugin-creation-builder (Agent 1).
//
// Owned by: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// see: cans/03-sdk-architecture.md#Fluent-plugin-surface

use std::sync::Arc;

use kelir_interface::handlers::HandlerCtx;
use kelir_interface::plugin_model::{
    ConsoleInterposer, FsInterposer, HostInterposer, KelirPlugin, LifecycleParticipant,
    MountScope, NetInterposer, PipelineBinder, PluginError, SystemTelemetryParticipant,
    TrapInterposer,
};
use kelir_interface::protocol_wire::{FilterAction, TrapEvent, Verdict};
use kelir_interface::vsock_mesh::{VsockServiceHandler, VsockServiceSpec};
use kelir::hlist_sdk::{HCons, HList, HNil};

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
pub trait Satisfies<Other: HList> {}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
pub trait PluginDeps {
    type Deps: HList;
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
pub struct PluginBuilder<Ctx, Deps: HList> {
    name: &'static str,
    scope: MountScope,
    context: Option<Ctx>,
    fork_with: Option<Arc<dyn Fn(&Ctx) -> Ctx + Send + Sync>>,
    trap: Option<Arc<dyn TrapInterposer>>,
    fs: Option<Arc<dyn FsInterposer>>,
    net: Option<Arc<dyn NetInterposer>>,
    console: Option<Arc<dyn ConsoleInterposer>>,
    host: Option<Arc<dyn HostInterposer>>,
    lifecycle: Option<Arc<dyn LifecycleParticipant>>,
    telemetry: Option<Arc<dyn SystemTelemetryParticipant>>,
    vsock: Vec<(VsockServiceSpec, Arc<dyn VsockServiceHandler>)>,
    deps: std::marker::PhantomData<Deps>,
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
pub struct PluginWrapper<Ctx, Deps: HList> {
    name: &'static str,
    scope: MountScope,
    context: Ctx,
    fork_with: Option<Arc<dyn Fn(&Ctx) -> Ctx + Send + Sync>>,
    trap: Option<Arc<dyn TrapInterposer>>,
    fs: Option<Arc<dyn FsInterposer>>,
    net: Option<Arc<dyn NetInterposer>>,
    console: Option<Arc<dyn ConsoleInterposer>>,
    host: Option<Arc<dyn HostInterposer>>,
    lifecycle: Option<Arc<dyn LifecycleParticipant>>,
    telemetry: Option<Arc<dyn SystemTelemetryParticipant>>,
    vsock: Vec<(VsockServiceSpec, Arc<dyn VsockServiceHandler>)>,
    deps: std::marker::PhantomData<Deps>,
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
impl<Ctx: Clone + Send + Sync + 'static> PluginBuilder<Ctx, HNil> {
    pub fn new(name: &'static str) -> Self {
        PluginBuilder {
            name,
            scope: MountScope::default(),
            context: None,
            fork_with: None,
            trap: None,
            fs: None,
            net: None,
            console: None,
            host: None,
            lifecycle: None,
            telemetry: None,
            vsock: Vec::new(),
            deps: std::marker::PhantomData,
        }
    }
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
impl<Ctx, Deps: HList> PluginBuilder<Ctx, Deps> {
    pub fn context<F>(mut self, make: F) -> Self
    where
        F: FnOnce() -> Ctx,
    {
        self.context = Some(make());
        self
    }

    pub fn require<P>(self) -> PluginBuilder<Ctx, HCons<P, Deps>>
    where
        P: PluginDeps,
    {
        PluginBuilder {
            name: self.name,
            scope: self.scope,
            context: self.context,
            fork_with: self.fork_with,
            trap: self.trap,
            fs: self.fs,
            net: self.net,
            console: self.console,
            host: self.host,
            lifecycle: self.lifecycle,
            telemetry: self.telemetry,
            vsock: self.vsock,
            deps: std::marker::PhantomData,
        }
    }

    pub fn scope(mut self, scope: MountScope) -> Self {
        self.scope = scope;
        self
    }

    pub fn fork_with<F>(mut self, fork: F) -> Self
    where
        F: Fn(&Ctx) -> Ctx + Send + Sync + 'static,
    {
        self.fork_with = Some(Arc::new(fork));
        self
    }

    pub fn on_trap<F>(mut self, handler: F) -> Self
    where
        F: Fn(&TrapEvent, &HandlerCtx) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        let _ = handler;
        self.trap = None;
        self
    }

    pub fn on_fs_write<F>(mut self, handler: F) -> Self
    where
        F: Fn(&std::path::Path, &mut [u8], u32, &HandlerCtx) -> Result<FilterAction, PluginError>
            + Send
            + Sync
            + 'static,
    {
        let _ = handler;
        self.fs = None;
        self
    }

    pub fn on_net_outbound<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut [u8]) -> Result<FilterAction, PluginError> + Send + Sync + 'static,
    {
        let _ = handler;
        self.net = None;
        self
    }

    pub fn vsock_service(mut self, spec: VsockServiceSpec, handler: Arc<dyn VsockServiceHandler>) -> Self {
        self.vsock.push((spec, handler));
        self
    }

    pub fn build(self) -> PluginWrapper<Ctx, Deps>
    where
        Ctx: Clone + Default,
    {
        // TBD: whether a missing .context() falls back to Ctx::default() or
        // is rejected at compile time via typestate is the live builder's
        // call (Agent 3); this sketch shows the defaulting shape only.
        PluginWrapper {
            name: self.name,
            scope: self.scope,
            context: self.context.unwrap_or_default(),
            fork_with: self.fork_with,
            trap: self.trap,
            fs: self.fs,
            net: self.net,
            console: self.console,
            host: self.host,
            lifecycle: self.lifecycle,
            telemetry: self.telemetry,
            vsock: self.vsock,
            deps: std::marker::PhantomData,
        }
    }
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
impl<Ctx: Clone + Send + Sync + 'static, Deps: HList> KelirPlugin for PluginWrapper<Ctx, Deps> {
    type Context = Ctx;

    fn context(&self) -> Self::Context {
        self.context.clone()
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn default_scope(&self) -> MountScope {
        self.scope.clone()
    }

    fn fork_state(&self, _child_session_id: &str) -> Box<dyn kelir_interface::plugin_model::AnyPlugin> {
        match &self.fork_with {
            Some(fork) => {
                let _ = fork(&self.context);
                self.clone_box()
            }
            None => self.clone_box(),
        }
    }

    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        if let Some(trap) = &self.trap {
            pipeline.bind_trap_scoped(Arc::clone(trap), self.scope.clone());
        }
        if let Some(fs) = &self.fs {
            pipeline.bind_fs_scoped(Arc::clone(fs), self.scope.clone());
        }
        if let Some(net) = &self.net {
            pipeline.bind_net_scoped(Arc::clone(net), self.scope.clone());
        }
        if let Some(console) = &self.console {
            pipeline.bind_console_scoped(Arc::clone(console), self.scope.clone());
        }
        if let Some(host) = &self.host {
            pipeline.bind_host_scoped(Arc::clone(host), self.scope.clone());
        }
        for (spec, service) in &self.vsock {
            pipeline.bind_vsock_service_scoped(spec.clone(), Arc::clone(service), self.scope.clone());
        }
    }
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// The remaining verbs (.on_process_exit, .on_fs_lookup, .on_fs_read,
// .on_fs_lock, .on_net_inbound, .on_console_input, .on_console_output,
// .on_exec_stdout, .on_exec_stderr, .on_host_lsm_exec, .on_host_pressure,
// .on_host_keyring_access, .on_vm_init, .on_vm_ready, .on_vm_shutdown,
// .on_pre_commit, .on_post_commit, .on_host_preempt, .on_memory_pressure,
// .on_heartbeat, .on_watchdog) follow the .on_trap / .on_fs_write shape
// above: each takes one closure, wraps it in a per-ring ClosureAdapter,
// stores one Arc<dyn Trait> on the builder, and is bound by
// register_interceptors above. Post-.build() calls are type errors: build
// consumes self and returns PluginWrapper, which exposes no verb methods.
