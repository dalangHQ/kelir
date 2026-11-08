// see: cans/03-sdk-architecture.md#Rust-SDK
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::time::Duration;

use kelir_interface::driver::{VmConfig, WatchdogAction};
use kelir_interface::plugin::HandlerCtx;
use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{AnyPlugin, CompositePlugin, MountScope, VetoEvent};
use kelir_interface::protocol::{TrapEvent, Verdict};

// see: cans/08-lifecycle.md#Lifecycle-vetoes
pub(crate) type VetoList = Vec<(VetoEvent, Box<dyn Fn() -> bool + Send + Sync>)>;

use crate::hlist::{HCons, HList, HNil, NoParams};
use crate::planes::storage::{LockBrokerConfig, StorageConfig};
use crate::planes::terminal::PtyConfig;
use crate::planes::wire::WireConfig;
use crate::plugin::KelirPlugin;
use crate::session::Session;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostExecAction {
    Proceed,
    RollbackAndHalt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unsealed;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sealed;

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
}

pub fn handler<F, Fut, A, O>(f: F) -> F
where
    F: Fn(A) -> Fut,
    Fut: Future<Output = Result<O, Box<dyn std::error::Error + Send + Sync>>>,
{
    f
}

#[derive(Debug, Clone, Default)]
pub struct DriverHandle;

impl DriverHandle {
    pub async fn set_watchdog(
        &self,
        _timeout: Duration,
        _action: WatchdogAction,
    ) -> Result<(), PluginError> {
        Err(PluginError::Halt(
            "TODO (L3): DriverHandle::set_watchdog".into(),
        ))
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

pub type BoxedTrap<Plugins> =
    Box<dyn Fn(HandlerCtx<Plugins, NoParams>) -> TrapFuture + Send + Sync>;

pub struct TrapEntry<Plugins: HList> {
    pub(crate) filter: TrapFilter,
    pub(crate) handler: BoxedTrap<Plugins>,
}

impl<Plugins: HList> TrapEntry<Plugins> {
    pub(crate) fn narrow<Head>(self) -> TrapEntry<HCons<Head, Plugins>>
    where
        Head: Clone + Send + Sync + 'static,
    {
        let handler = self.handler;
        TrapEntry {
            filter: self.filter,
            handler: Box::new(move |ctx: HandlerCtx<HCons<Head, Plugins>, NoParams>| {
                (handler)(ctx.map_plugins(|plugins| plugins.tail_value()))
            }),
        }
    }
}

type BoxFutureResult = Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send>>;

type BoxedDriverCallback = Box<dyn Fn(DriverHandle) -> BoxFutureResult + Send + Sync>;

// see: cans/03-sdk-architecture.md#Builder-consumption-model-across-languages
type BoxedCoreCallback = Box<dyn Fn(&kelir_core::Session) -> BoxFutureResult + Send + Sync>;

type BoxedTrapDenyCallback =
    Box<dyn Fn(&TrapEvent, &kelir_core::Session) -> BoxFutureResult + Send + Sync>;

type CustomActionFuture =
    Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send>>;

type BoxedCustomAction = Box<dyn Fn(ciborium::value::Value) -> CustomActionFuture + Send + Sync>;

// TODO (L3): the engine dispatch reads these entries once register_interceptors wiring lands per cans/02-plugin-model.md#Static-dispatch.
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
pub(crate) struct VsockServiceEntry {
    pub(crate) port: u32,
    pub(crate) service: String,
    pub(crate) schema: serde_json::Value,
    pub(crate) handler: BoxedStreamService,
}

// TODO (L3): the engine dispatch reads these entries once register_interceptors wiring lands per cans/02-plugin-model.md#Static-dispatch.
#[allow(dead_code)]
pub(crate) struct StreamKindServiceEntry {
    pub(crate) kind: u16,
    pub(crate) schema: serde_json::Value,
    pub(crate) handler: BoxedStreamService,
}

pub(crate) struct Registration<Plugins: HList> {
    pub(crate) instances: Vec<Box<dyn AnyPlugin>>,
    pub(crate) traps: Vec<TrapEntry<Plugins>>,
    pub(crate) terminal_cfg: Option<PtyConfig>,
    pub(crate) storage_cfg: Option<StorageConfig<Plugins, NoParams>>,
    pub(crate) wire_cfg: Option<WireConfig>,
    pub(crate) lock_broker_cfg: Option<LockBrokerConfig>,
    pub(crate) on_vm_init: Option<BoxedDriverCallback>,
    pub(crate) on_vm_ready: Option<BoxedCoreCallback>,
    pub(crate) on_vm_shutdown: Option<BoxedCoreCallback>,
    pub(crate) on_trap_deny: Option<BoxedTrapDenyCallback>,
    pub(crate) custom_actions: Vec<CustomActionEntry>,
    pub(crate) vsock_services: Vec<VsockServiceEntry>,
    pub(crate) stream_kind_services: Vec<StreamKindServiceEntry>,
    pub(crate) vetoes: VetoList,
}

impl<Plugins: HList> Default for Registration<Plugins> {
    fn default() -> Self {
        Self {
            instances: Vec::new(),
            traps: Vec::new(),
            terminal_cfg: None,
            storage_cfg: None,
            wire_cfg: None,
            lock_broker_cfg: None,
            on_vm_init: None,
            on_vm_ready: None,
            on_vm_shutdown: None,
            on_trap_deny: None,
            custom_actions: Vec::new(),
            vsock_services: Vec::new(),
            stream_kind_services: Vec::new(),
            vetoes: Vec::new(),
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
            traps: self.traps.into_iter().map(|entry| entry.narrow()).collect(),
            terminal_cfg: self.terminal_cfg,
            storage_cfg: self.storage_cfg.map(|cfg| cfg.narrow()),
            wire_cfg: self.wire_cfg,
            lock_broker_cfg: self.lock_broker_cfg,
            on_vm_init: self.on_vm_init,
            on_vm_ready: self.on_vm_ready,
            on_vm_shutdown: self.on_vm_shutdown,
            on_trap_deny: self.on_trap_deny,
            custom_actions: self.custom_actions,
            vsock_services: self.vsock_services,
            stream_kind_services: self.stream_kind_services,
            vetoes: self.vetoes,
        }
    }
}

pub struct KelirBuilder<SealState = Unsealed, Plugins = HNil, Params = NoParams>
where
    Plugins: HList,
    Params: 'static,
{
    config: VmConfig,
    plugins: Plugins,
    registration: Registration<Plugins>,
    seal: PhantomData<fn(SealState, Params)>,
}

impl KelirBuilder<Unsealed, HNil, NoParams> {
    pub fn new(config: VmConfig) -> Self {
        Self {
            config,
            plugins: HNil,
            registration: Registration::default(),
            seal: PhantomData,
        }
    }
}

impl<Plugins: HList, Params: 'static> KelirBuilder<Unsealed, Plugins, Params> {
    pub fn seal(self) -> KelirBuilder<Sealed, Plugins, Params> {
        KelirBuilder {
            config: self.config,
            plugins: self.plugins,
            registration: self.registration,
            seal: PhantomData,
        }
    }
}

impl<SealState, Plugins: HList, Params: 'static> KelirBuilder<SealState, Plugins, Params> {
    pub fn plugin<P>(self, plugin: P) -> KelirBuilder<SealState, HCons<P::Context, Plugins>, Params>
    where
        P: KelirPlugin + Clone + 'static,
    {
        let context = plugin.context();
        let mut registration = self.registration.narrow::<P::Context>();
        registration.instances.push(Box::new(plugin));
        KelirBuilder {
            config: self.config,
            plugins: HCons {
                head: context,
                tail: self.plugins,
            },
            registration,
            seal: PhantomData,
        }
    }

    pub fn terminal(mut self, config: PtyConfig) -> Self {
        self.registration.terminal_cfg = Some(config);
        self
    }

    pub fn storage(mut self, config: StorageConfig<Plugins, Params>) -> Self {
        self.registration.storage_cfg = Some(config.retag());
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

    pub fn params<T>(self) -> KelirBuilder<SealState, Plugins, T> {
        KelirBuilder {
            config: self.config,
            plugins: self.plugins,
            registration: self.registration,
            seal: PhantomData,
        }
    }

    pub fn trap<F, Fut, O>(mut self, filter: impl Into<TrapFilter>, f: F) -> Self
    where
        F: Fn(HandlerCtx<Plugins, Params>) -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = Result<O, Box<dyn std::error::Error + Send + Sync>>> + Send + 'static,
        O: IntoTrapOutcome,
    {
        let boxed: BoxedTrap<Plugins> = Box::new(move |ctx: HandlerCtx<Plugins, NoParams>| {
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

    pub fn on_vm_init<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(DriverHandle) -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.registration.on_vm_init = Some(Box::new(move |driver: DriverHandle| {
            let fut = f.clone()(driver);
            Box::pin(fut)
        }));
        self
    }

    pub fn on_vm_ready<F, Fut, T>(mut self, f: F) -> Self
    where
        F: Fn(&kelir_core::Session) -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = Result<T, PluginError>> + Send + 'static,
    {
        self.registration.on_vm_ready = Some(Box::new(move |session: &kelir_core::Session| {
            let fut = f.clone()(session);
            Box::pin(async move { fut.await.map(drop) })
        }));
        self
    }

    pub fn on_vm_shutdown<F, Fut, T>(mut self, f: F) -> Self
    where
        F: Fn(&kelir_core::Session) -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = Result<T, PluginError>> + Send + 'static,
    {
        self.registration.on_vm_shutdown = Some(Box::new(move |session: &kelir_core::Session| {
            let fut = f.clone()(session);
            Box::pin(async move { fut.await.map(drop) })
        }));
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

    // see: cans/11-protocol.md#Control-channel
    pub fn register_vsock_service<F, Fut>(
        mut self,
        port: u32,
        service: &str,
        schema: serde_json::Value,
        f: F,
    ) -> Self
    where
        F: Fn(Vec<u8>) -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>>
            + Send
            + 'static,
    {
        self.registration.vsock_services.push(VsockServiceEntry {
            port,
            service: service.to_string(),
            schema,
            handler: Box::new(move |request: Vec<u8>| {
                let fut = f.clone()(request);
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

    pub fn with_composite<C>(mut self, bundle: C, scope: MountScope) -> Self
    where
        C: CompositePlugin + 'static,
    {
        // TODO (L3): flatten composite children into typed capability mounts once interface AnyPlugin carries KelirPlugin per cans/artifacts/code/plugin-trait.rs.
        for (plugin, _child_scope) in CompositePlugin::unpack(Box::new(bundle), &scope) {
            self.registration.instances.push(plugin);
        }
        self
    }

    async fn finish_boot(self) -> Result<Session<Plugins, Params>, PluginError> {
        // TODO (L3): register plugin instances, dispatch tables, and vetoes into the engine per cans/02-plugin-model.md#Static-dispatch before handing the session back.
        let plugins: Vec<kelir_core::PluginRegistration> = self
            .registration
            .instances
            .iter()
            .map(|plugin| kelir_core::PluginRegistration::new(plugin.name(), true))
            .collect();
        let core = kelir_core::Session::boot_with_plugins(self.config, &plugins)?;
        Ok(Session::from_parts(core, self.plugins, self.registration))
    }
}

impl<Plugins: HList, Params: 'static> KelirBuilder<Unsealed, Plugins, Params> {
    // see: cans/03-sdk-architecture.md#Rust-SDK
    pub async fn boot(self) -> Result<Session<Plugins, Params>, PluginError> {
        self.finish_boot().await
    }
}

impl<Plugins: HList, Params: 'static> KelirBuilder<Sealed, Plugins, Params> {
    pub async fn boot(self) -> Result<Session<Plugins, Params>, PluginError> {
        self.finish_boot().await
    }
}
