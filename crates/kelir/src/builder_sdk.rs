// see: cans/03-sdk-architecture.md#Rust-SDK
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;

use kelir_interface::driver_interface::{DaxWindowMode, FastBootMode, VmConfig};
use kelir_interface::plugin_model::HandlerCtx;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{
    AnyPlugin, ConsoleInterposer, FsInterposer, HostInterposer, LifecycleEvent,
    LifecycleParticipant, MountScope, NetInterposer, PipelineBinder, PluginDeps,
    SystemTelemetryParticipant, TrapInterposer, VetoEvent, VetoList,
};
use kelir_interface::protocol_wire::{TrapEvent, Verdict};
use kelir_interface::vsock_mesh::{VsockServiceHandler, VsockServiceSpec};

use crate::hlist_sdk::{HCons, HList, HNil, NoParams, Satisfies};
use crate::planes::storage_plane::{LockBrokerConfig, LockScope, StoragePlaneConfig};
use crate::planes::terminal_plane::PtyConfig;
use crate::planes::wire_plane::{TapFilter, WireConfig};
use crate::plugin_model::KelirPlugin;
use crate::session_lifecycle::Session;

// see: cans/plugins/tool-guard.md#Host-extensible-hook-pipeline
#[cfg(feature = "plugin-tool-guard")]
use plugin_tool_guard::{ToolGuardPlugin, ToolSpec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostExecAction {
    Proceed,
    RollbackAndHalt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unsealed;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sealed;

// see: cans/02-plugin-model.md#Unified-builder
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Host;

// see: cans/02-plugin-model.md#Unified-builder
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plugin;

#[derive(Debug, Clone, Default)]
pub struct TrapFilter {
    pub binary: Option<String>,
    pub args: Vec<String>,
}

impl From<&str> for TrapFilter {
    fn from(value: &str) -> Self {
        Self {
            binary: if value == "*" {
                None
            } else {
                Some(value.to_string())
            },
            args: Vec::new(),
        }
    }
}

impl TrapFilter {
    pub fn binary(binary: &str) -> Self {
        Self {
            binary: Some(binary.to_string()),
            args: Vec::new(),
        }
    }

    pub fn with_arg(mut self, arg: &str) -> Self {
        self.args.push(arg.to_string());
        self
    }

    pub(crate) fn matches(&self, event: &TrapEvent) -> bool {
        if self
            .binary
            .as_ref()
            .is_some_and(|binary| binary != &event.binary_path)
        {
            return false;
        }
        self.args.iter().all(|arg| event.argv.contains(arg))
    }
}

pub struct LifecycleMut<'a> {
    vetoes: &'a mut VetoList,
}

impl LifecycleMut<'_> {
    pub fn register_veto(
        &mut self,
        event: VetoEvent,
        callback: Box<dyn Fn() -> bool + Send + Sync>,
    ) {
        self.vetoes.push((event, callback));
    }
}

// see: cans/02-plugin-model.md#Unified-verdict
pub enum TrapOutcome {
    Verdict(Verdict),
    PostExec(PostExecAction),
}

pub trait IntoTrapOutcome: Send + 'static {
    fn into_outcome(self) -> TrapOutcome;
}

impl IntoTrapOutcome for Verdict {
    fn into_outcome(self) -> TrapOutcome {
        TrapOutcome::Verdict(self)
    }
}

impl IntoTrapOutcome for PostExecAction {
    fn into_outcome(self) -> TrapOutcome {
        TrapOutcome::PostExec(self)
    }
}

pub type TrapFuture = Pin<
    Box<dyn Future<Output = Result<TrapOutcome, Box<dyn std::error::Error + Send + Sync>>> + Send>,
>;

// see: cans/03-sdk-architecture.md#Rust-SDK
pub type BoxedTrap<Plugins> =
    Arc<dyn Fn(HandlerCtx<Plugins, NoParams>) -> TrapFuture + Send + Sync>;

pub struct TrapEntry<Plugins: HList> {
    pub(crate) filter: TrapFilter,
    pub(crate) handler: BoxedTrap<Plugins>,
}

impl<Plugins: HList> Clone for TrapEntry<Plugins> {
    fn clone(&self) -> Self {
        Self {
            filter: self.filter.clone(),
            handler: Arc::clone(&self.handler),
        }
    }
}

impl<Plugins: HList> TrapEntry<Plugins> {
    pub(crate) fn narrow<Head>(self) -> TrapEntry<HCons<Head, Plugins>>
    where
        Head: Clone + Send + Sync + 'static,
    {
        let handler = self.handler;
        TrapEntry {
            filter: self.filter,
            handler: Arc::new(move |ctx: HandlerCtx<HCons<Head, Plugins>, NoParams>| {
                (handler)(ctx.map_plugins(|plugins| plugins.tail_value()))
            }),
        }
    }
}

type BoxFutureResult = Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send>>;

// see: cans/03-sdk-architecture.md#Builder-consumption-model-across-languages
type BoxedTrapDenyCallback =
    Box<dyn Fn(&TrapEvent, &kelir_core::Session) -> BoxFutureResult + Send + Sync>;

type CustomActionFuture =
    Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send>>;

type BoxedCustomAction = Box<dyn Fn(ciborium::value::Value) -> CustomActionFuture + Send + Sync>;

#[allow(dead_code)]
pub(crate) struct CustomActionEntry {
    pub(crate) name: String,
    pub(crate) schema: serde_json::Value,
    pub(crate) handler: BoxedCustomAction,
}

type StreamServiceFuture =
    Pin<Box<dyn Future<Output = Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>> + Send>>;

type BoxedStreamService = Box<dyn Fn(Vec<u8>) -> StreamServiceFuture + Send + Sync>;

// TODO (L3): the engine dispatch reads these entries once register_interceptors wiring lands per cans/02-plugin-model.md#Static-dispatch.
#[allow(dead_code)]
pub(crate) struct StreamKindServiceEntry {
    pub(crate) kind: u16,
    pub(crate) schema: serde_json::Value,
    pub(crate) handler: BoxedStreamService,
}

// see: cans/02-plugin-model.md#Unified-builder
pub(crate) type ForkFn = Arc<dyn Fn(&str) -> Box<dyn AnyPlugin> + Send + Sync>;

// see: cans/02-plugin-model.md#Unified-builder
#[derive(Clone, Default)]
pub(crate) struct HookRegistry {
    pub(crate) trap: Vec<Arc<dyn TrapInterposer>>,
    pub(crate) fs: Vec<Arc<dyn FsInterposer>>,
    pub(crate) net: Vec<Arc<dyn NetInterposer>>,
    pub(crate) console: Vec<Arc<dyn ConsoleInterposer>>,
    pub(crate) host: Vec<Arc<dyn HostInterposer>>,
    pub(crate) lifecycle: Vec<Arc<dyn LifecycleParticipant>>,
    pub(crate) telemetry: Vec<Arc<dyn SystemTelemetryParticipant>>,
    pub(crate) vsock: Vec<(VsockServiceSpec, Arc<dyn VsockServiceHandler>)>,
}

impl HookRegistry {
    // see: cans/02-plugin-model.md#Unified-builder
    pub(crate) fn bind(&self, pipeline: &mut dyn PipelineBinder, scope: &MountScope) {
        for stage in &self.trap {
            pipeline.bind_trap_scoped(Arc::clone(stage), scope.clone());
        }
        for stage in &self.fs {
            pipeline.bind_fs_scoped(Arc::clone(stage), scope.clone());
        }
        for stage in &self.net {
            pipeline.bind_net_scoped(Arc::clone(stage), scope.clone());
        }
        for stage in &self.console {
            pipeline.bind_console_scoped(Arc::clone(stage), scope.clone());
        }
        for stage in &self.host {
            pipeline.bind_host_scoped(Arc::clone(stage), scope.clone());
        }
        for (spec, service) in &self.vsock {
            pipeline.bind_vsock_service_scoped(spec.clone(), Arc::clone(service), scope.clone());
        }
        for stage in &self.lifecycle {
            pipeline.bind_lifecycle_scoped(Arc::clone(stage), scope.clone());
        }
        for stage in &self.telemetry {
            pipeline.bind_telemetry_scoped(Arc::clone(stage), scope.clone());
        }
    }

    pub(crate) fn validate_vsock(&self, name: &str) -> Result<(), PluginError> {
        for (spec, _) in &self.vsock {
            validate_vsock_spec(name, spec.port, &spec.service)?;
        }
        Ok(())
    }
}

// see: cans/02-plugin-model.md#Unified-builder
#[derive(Clone)]
pub(crate) struct MountedPlugin {
    pub(crate) name: &'static str,
    pub(crate) scope: MountScope,
    pub(crate) fork: Option<ForkFn>,
    pub(crate) registry: HookRegistry,
}

impl MountedPlugin {
    pub(crate) fn bind(&self, pipeline: &mut dyn PipelineBinder) {
        self.registry.bind(pipeline, &self.scope);
    }
}

fn has_empty<T: AsRef<std::ffi::OsStr>>(values: &Option<Vec<T>>) -> bool {
    values
        .as_ref()
        .is_some_and(|values| values.iter().any(|value| value.as_ref().is_empty()))
}

fn validate_vsock_spec(name: &str, port: u32, service: &str) -> Result<(), PluginError> {
    if port == 0 || port > 65_535 {
        return Err(PluginError::reject(
            "EConfig",
            format!("{name}: vsock service port {port} out of range 1..=65535"),
        ));
    }
    if service.is_empty() {
        return Err(PluginError::reject(
            "EConfig",
            format!("{name}: vsock service name must not be empty"),
        ));
    }
    Ok(())
}

fn validate_scope(name: &str, scope: &MountScope) -> Result<(), PluginError> {
    if has_empty(&scope.binaries) {
        return Err(PluginError::reject(
            "EConfig",
            format!("{name}: scope binary name must not be empty"),
        ));
    }
    if has_empty(&scope.paths) {
        return Err(PluginError::reject(
            "EConfig",
            format!("{name}: scope path must not be empty"),
        ));
    }
    Ok(())
}

pub(crate) struct Registration<Plugins: HList> {
    pub(crate) instances: Vec<(Box<dyn AnyPlugin>, MountScope)>,
    pub(crate) mounted: Vec<MountedPlugin>,
    pub(crate) traps: Vec<TrapEntry<Plugins>>,
    pub(crate) storage_cfg: Option<StoragePlaneConfig<Plugins, NoParams>>,
    pub(crate) terminal_cfg: Option<PtyConfig>,
    pub(crate) wire_cfg: Option<WireConfig>,
    pub(crate) lock_broker_cfg: Option<LockBrokerConfig>,
    pub(crate) on_trap_deny: Option<BoxedTrapDenyCallback>,
    pub(crate) custom_actions: Vec<CustomActionEntry>,
    pub(crate) stream_kind_services: Vec<StreamKindServiceEntry>,
    pub(crate) vetoes: VetoList,
    pub(crate) detached: bool,
}

impl<Plugins: HList> Default for Registration<Plugins> {
    fn default() -> Self {
        Self {
            instances: Vec::new(),
            mounted: Vec::new(),
            traps: Vec::new(),
            storage_cfg: None,
            terminal_cfg: None,
            wire_cfg: None,
            lock_broker_cfg: None,
            on_trap_deny: None,
            custom_actions: Vec::new(),
            stream_kind_services: Vec::new(),
            vetoes: Vec::new(),
            detached: false,
        }
    }
}

impl<Plugins: HList> Registration<Plugins> {
    pub(crate) fn narrow<Head>(self) -> Registration<HCons<Head, Plugins>>
    where
        Head: Clone + Send + Sync + 'static,
    {
        Registration {
            instances: self.instances,
            mounted: self.mounted,
            traps: self.traps.into_iter().map(|entry| entry.narrow()).collect(),
            storage_cfg: self.storage_cfg.map(|cfg| cfg.narrow()),
            terminal_cfg: self.terminal_cfg,
            wire_cfg: self.wire_cfg,
            lock_broker_cfg: self.lock_broker_cfg,
            on_trap_deny: self.on_trap_deny,
            custom_actions: self.custom_actions,
            stream_kind_services: self.stream_kind_services,
            vetoes: self.vetoes,
            detached: self.detached,
        }
    }
}

pub struct KelirBuilder<
    Mode = Host,
    SealState = Unsealed,
    Ctx = (),
    Deps = HNil,
    Plugins = HNil,
    Params = NoParams,
> where
    Plugins: HList,
    Params: 'static,
    Deps: HList,
{
    pub(crate) config: Option<VmConfig>,
    pub(crate) plugins: Plugins,
    pub(crate) registration: Registration<Plugins>,
    pub(crate) fast_boot: FastBootMode,
    pub(crate) dax_window: DaxWindowMode,
    pub(crate) name: &'static str,
    pub(crate) context_factory: Arc<dyn Fn() -> Ctx + Send + Sync>,
    pub(crate) scope: MountScope,
    pub(crate) fork: Option<ForkFn>,
    pub(crate) registry: HookRegistry,
    pub(crate) seal: PhantomData<fn(SealState, Params, Mode)>,
    pub(crate) deps: PhantomData<fn() -> Deps>,
}

impl KelirBuilder<Host, Unsealed, (), HNil, HNil, NoParams> {
    pub fn new(config: VmConfig) -> Self {
        Self {
            config: Some(config),
            plugins: HNil,
            registration: Registration::default(),
            fast_boot: FastBootMode::Off,
            dax_window: DaxWindowMode::Off,
            name: "host",
            context_factory: Arc::new(|| ()),
            scope: MountScope::default(),
            fork: None,
            registry: HookRegistry::default(),
            seal: PhantomData,
            deps: PhantomData,
        }
    }
}

impl KelirBuilder<Plugin, Unsealed, (), HNil, HNil, NoParams> {
    pub fn named(name: &'static str) -> Self {
        Self {
            config: None,
            plugins: HNil,
            registration: Registration::default(),
            fast_boot: FastBootMode::Off,
            dax_window: DaxWindowMode::Off,
            name,
            context_factory: Arc::new(|| ()),
            scope: MountScope::default(),
            fork: None,
            registry: HookRegistry::default(),
            seal: PhantomData,
            deps: PhantomData,
        }
    }
}

// see: cans/03-sdk-architecture.md#Rust-SDK
impl<Plugins: HList, Params: 'static, Ctx, Deps: HList>
    KelirBuilder<Host, Unsealed, Ctx, Deps, Plugins, Params>
{
    // see: cans/03-sdk-architecture.md#Rust-SDK
    pub fn seal(
        self,
    ) -> Result<KelirBuilder<Host, Sealed, Ctx, Deps, Plugins, Params>, PluginError> {
        self.registry.validate_vsock(self.name)?;
        validate_scope(self.name, &self.scope)?;
        for mounted in &self.registration.mounted {
            mounted.registry.validate_vsock(mounted.name)?;
            validate_scope(mounted.name, &mounted.scope)?;
        }
        Ok(KelirBuilder {
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
        })
    }

    // see: cans/02-plugin-model.md#Unified-builder
    pub fn plugin<PCtx, PDeps>(
        mut self,
        plugin: KelirBuilder<Plugin, Unsealed, PCtx, PDeps, HNil, NoParams>,
    ) -> Result<Self, PluginError>
    where
        PDeps: HList,
        Plugins: Satisfies<PDeps>,
    {
        if self
            .registration
            .mounted
            .iter()
            .any(|mounted| mounted.name == plugin.name)
        {
            return Err(PluginError::reject(
                "EConfig",
                format!("plugin already mounted: {}", plugin.name),
            ));
        }
        self.registration.mounted.push(MountedPlugin {
            name: plugin.name,
            scope: plugin.scope,
            fork: plugin.fork,
            registry: plugin.registry,
        });
        Ok(self)
    }

    // see: cans/03-sdk-architecture.md#Rust-SDK
    pub fn plugin_instance<P>(
        self,
        plugin: P,
    ) -> KelirBuilder<Host, Unsealed, Ctx, Deps, HCons<P::Context, Plugins>, Params>
    where
        P: KelirPlugin + Clone + 'static,
        P: PluginDeps,
        Plugins: Satisfies<P::Deps>,
    {
        let context = plugin.context();
        let scope = KelirPlugin::default_scope(&plugin);
        let mut registration = self.registration.narrow::<P::Context>();
        registration.instances.push((Box::new(plugin), scope));
        KelirBuilder {
            config: self.config,
            plugins: HCons {
                head: context,
                tail: self.plugins,
            },
            registration,
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

    pub fn storage(mut self, config: StoragePlaneConfig<Plugins, Params>) -> Self {
        self.registration.storage_cfg = Some(config.retag());
        self
    }

    // see: cans/04-terminal-plane.md#Terminal-pipeline
    pub fn terminal(mut self, config: PtyConfig) -> Self {
        self.registration.terminal_cfg = Some(config);
        self
    }

    // see: cans/07-wire-plane.md#DNS-policy-filter
    pub fn wire(mut self, config: WireConfig) -> Self {
        self.registration.wire_cfg = Some(config);
        self
    }

    // see: cans/05-storage-plane.md#Workspace-commit
    pub fn lock_broker(mut self, config: LockBrokerConfig) -> Self {
        self.registration.lock_broker_cfg = Some(config);
        self
    }

    // see: cans/08-lifecycle.md#Detached-lifecycle
    pub fn detached(mut self, detached: bool) -> Self {
        self.registration.detached = detached;
        self
    }

    // see: cans/01-architecture.md#Boot-budget
    pub fn fast_boot(mut self, mode: FastBootMode) -> Self {
        self.fast_boot = mode;
        self
    }

    // see: cans/05-storage-plane.md#EROFS-rootfs
    pub fn dax_window(mut self, mode: DaxWindowMode) -> Self {
        self.dax_window = mode;
        self
    }

    pub fn is_detached(&self) -> bool {
        self.registration.detached
    }

    pub fn params<T>(self) -> KelirBuilder<Host, Unsealed, Ctx, Deps, Plugins, T> {
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

    pub fn trap<F, Fut, O>(mut self, filter: impl Into<TrapFilter>, f: F) -> Self
    where
        F: Fn(HandlerCtx<Plugins, Params>) -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = Result<O, Box<dyn std::error::Error + Send + Sync>>> + Send + 'static,
        O: IntoTrapOutcome,
    {
        let boxed: BoxedTrap<Plugins> = Arc::new(move |ctx: HandlerCtx<Plugins, NoParams>| {
            let f = f.clone();
            let fut = f(ctx.retag::<Params>());
            Box::pin(async move { fut.await.map(IntoTrapOutcome::into_outcome) })
        });
        self.registration.traps.push(TrapEntry {
            filter: filter.into(),
            handler: boxed,
        });
        self
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

    // TODO (L3): core-kind re-registration aborts EKindReserved and unregistered kinds abort EUnknownKind on OPEN at dispatch/seal per cans/11-protocol.md#Control-channel.
    // see: cans/11-protocol.md#Control-channel
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
}

impl<SealState, Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList>
    KelirBuilder<Mode, SealState, Ctx, Deps, Plugins, Params>
{
    // see: cans/02-plugin-model.md#Unified-builder
    pub(crate) fn bind_registry(&self, pipeline: &mut dyn PipelineBinder) {
        self.registry.bind(pipeline, &self.scope);
        for mounted in &self.registration.mounted {
            mounted.bind(pipeline);
        }
    }
}

impl<SealState, Plugins: HList, Params: 'static, Ctx, Deps: HList>
    KelirBuilder<Host, SealState, Ctx, Deps, Plugins, Params>
{
    async fn finish_boot(mut self) -> Result<Session<Plugins, Params>, PluginError> {
        // see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
        // TODO (L3): plumb FastBootMode into DriverRouter::load_native_driver.
        let _ = (self.fast_boot, self.dax_window);
        let storage = self.storage_config();
        let Some(config) = self.config.take() else {
            return Err(PluginError::reject(
                "EConfig",
                "host builder has no VmConfig",
            ));
        };
        let mut engine = kelir_core::dispatch_plugin::PipelineEngine::new();
        self.bind_registry(&mut engine);
        let mut registration = self.registration;
        let mut plugins: Vec<kelir_core::PluginRegistration> = registration
            .instances
            .iter()
            .map(|(plugin, _)| {
                kelir_core::PluginRegistration::new(plugin.name(), plugin.is_enabled())
            })
            .collect();
        plugins.extend(
            registration
                .mounted
                .iter()
                .map(|mounted| kelir_core::PluginRegistration::new(mounted.name, true)),
        );
        for (instance, scope) in &registration.instances {
            if !instance.is_enabled() {
                continue;
            }
            engine.set_active_scope(scope.clone());
            instance.register_interceptors(&mut engine);
        }
        for entry in std::mem::take(&mut registration.custom_actions) {
            engine.register_custom_action(entry.name, entry.handler);
        }
        if let Some(callback) = registration.on_trap_deny.take() {
            engine.set_on_trap_deny(callback);
        }
        engine.set_active_scope(MountScope::default());
        engine.seal()?;
        let metrics = std::sync::Arc::new(kelir_otlp::registry_otlp::MetricRegistry::new());
        let engine = engine.with_metrics(metrics.clone());
        let synthetic_files = registration
            .storage_cfg
            .as_ref()
            .map(|cfg| cfg.synthetic_files.clone())
            .unwrap_or_default();
        let plane_wiring = kelir_core::session_lifecycle::PlaneWiring {
            terminal: registration.terminal_cfg.as_ref().map(|cfg| {
                kelir_core::planes::terminal::TerminalWiring {
                    cols: cfg.cols,
                    rows: cfg.rows,
                    drop_osc52: cfg.drop_osc52,
                    strip_title: cfg.strip_title,
                    token_shield: cfg.token_shield.as_ref().map(|shield| {
                        kelir_core::planes::terminal::TokenShieldConfig {
                            max_bytes_total: shield.max_bytes_total,
                            max_lines_total: shield.max_lines_total,
                            clamp_strategy: shield.clamp_strategy.clone(),
                            notice: shield.notice.clone(),
                        }
                    }),
                    prompts: cfg
                        .prompts
                        .iter()
                        .map(|rule| kelir_core::planes::terminal::PromptRuleConfig {
                            pattern: rule.pattern.clone(),
                            timeout: rule.timeout,
                            verify_foreground: rule.verify_foreground,
                            action: {
                                let action = Arc::clone(&rule.action);
                                Arc::new(
                                    move |event: &kelir_interface::protocol_wire::TrapEvent| {
                                        action(event, crate::planes::terminal_plane::PromptStream)
                                    },
                                )
                            },
                        })
                        .collect(),
                }
            }),
            wire: registration
                .wire_cfg
                .as_ref()
                .map(|cfg| kelir_core::planes::wire::WireWiring {
                    sinkhole_allowlist: cfg.sinkhole.allowlist.clone(),
                    sinkhole_drop_all: cfg.sinkhole.drop_all,
                    ssh_agent_bridge: cfg.ssh_agent_bridge,
                    vault_placeholders: cfg
                        .vaults
                        .iter()
                        .map(|vault| vault.placeholder.clone())
                        .collect(),
                    l4_proxies: cfg
                        .l4_proxies
                        .iter()
                        .map(|proxy| (proxy.listen_port, proxy.destination.clone()))
                        .collect(),
                    allow_udp_proxied: matches!(cfg.tap_filter, TapFilter::AllowUdpProxied),
                }),
            storage,
        };
        let core = kelir_core::Session::boot_with_engine(
            config,
            &plugins,
            engine,
            metrics,
            synthetic_files,
            plane_wiring,
        )?;
        let mut core = core;
        core.set_detached(registration.detached);
        core.set_vetoes(std::mem::take(&mut registration.vetoes));
        core.run_lifecycle_hook(LifecycleEvent::VmInit).await?;
        core.run_lifecycle_hook(LifecycleEvent::VmReady).await?;
        Ok(Session::from_parts(core, self.plugins, registration))
    }

    // see: cans/05-storage-plane.md#Storage-architecture
    // TODO (L3): snoop on_write closures register as engine FsInterposer hooks at seal per cans/02-plugin-model.md#Unified-interposition-pipeline.
    fn storage_config(&self) -> kelir_core::planes::storage::StorageConfig {
        let Some(cfg) = self.registration.storage_cfg.as_ref() else {
            return kelir_core::planes::storage::StorageConfig::default();
        };
        let lock_broker = self
            .registration
            .lock_broker_cfg
            .as_ref()
            .map(|broker| kelir_core::planes::storage::LockBrokerWiring {
                scope_globs: match &broker.scope {
                    LockScope::Globs(globs) => globs.clone(),
                },
                non_blocking: broker.non_blocking,
            })
            .or_else(|| {
                cfg.lock_broker
                    .then(|| kelir_core::planes::storage::LockBrokerWiring {
                        scope_globs: Vec::new(),
                        non_blocking: false,
                    })
            });
        kelir_core::planes::storage::StorageConfig {
            shared_caches: cfg
                .shared_caches
                .iter()
                .map(std::path::PathBuf::from)
                .collect(),
            lock_broker,
            idmap: Some(kelir_core::planes::storage::IdmapConfig {
                guest_uid: cfg.idmap.guest_uid,
                host_uid: cfg.idmap.host_uid,
            }),
            mask_read: Some(kelir_core::planes::storage::MaskReadConfig {
                engine: cfg.scan_engine,
                carryover_bytes: cfg.carryover_bytes,
                rules: cfg.mask_read.clone(),
            }),
            encapsulation: cfg.encapsulation,
        }
    }
}

impl<Plugins: HList, Params: 'static, Ctx, Deps: HList>
    KelirBuilder<Host, Unsealed, Ctx, Deps, Plugins, Params>
{
    // see: cans/03-sdk-architecture.md#Rust-SDK
    pub async fn boot(self) -> Result<Session<Plugins, Params>, PluginError> {
        self.seal()?.finish_boot().await
    }
}

impl<Plugins: HList, Params: 'static, Ctx, Deps: HList>
    KelirBuilder<Host, Sealed, Ctx, Deps, Plugins, Params>
{
    // see: cans/03-sdk-architecture.md#Rust-SDK
    pub async fn boot(self) -> Result<Session<Plugins, Params>, PluginError> {
        self.finish_boot().await
    }
}

// see: cans/plugins/tool-guard.md#Host-extensible-hook-pipeline
#[cfg(feature = "plugin-tool-guard")]
pub trait ToolGuardExt: Sized {
    fn tool(self, spec: ToolSpec) -> Self;
}

// see: cans/plugins/tool-guard.md#Host-extensible-hook-pipeline
#[cfg(feature = "plugin-tool-guard")]
impl<Plugins: HList, Params: 'static, Ctx, Deps: HList> ToolGuardExt
    for KelirBuilder<Host, Unsealed, Ctx, Deps, Plugins, Params>
{
    fn tool(mut self, spec: ToolSpec) -> Self {
        let plugin = ToolGuardPlugin::new().tool(spec);
        let scope = KelirPlugin::default_scope(&plugin);
        self.registration.instances.push((Box::new(plugin), scope));
        self
    }
}
