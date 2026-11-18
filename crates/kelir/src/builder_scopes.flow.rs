// see: cans/02-plugin-model.md#Nested-scopes
// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
use std::future::Future;
use std::marker::PhantomData;
use std::path::Path;
use std::pin::Pin;
use std::time::Duration;

use kelir_interface::driver_interface::{WatchdogEvent, WatchdogVerdict};
use kelir_interface::plugin_model::{
    AnyPlugin, HandlerCtx, KelirPlugin, LifecycleCtx, PluginError, Veto, VetoEvent,
};
use kelir_interface::protocol_wire::{FilterAction, Heartbeat, ProcessExit, TrapEvent, Verdict};
use kelir_interface::vsock_mesh::{VsockServiceHandler, VsockServiceSpec};

use crate::builder_sdk::{Host, KelirBuilder, Plugin, Unsealed};
use crate::hlist_sdk::HList;
use crate::planes::storage_plane::LockBrokerConfig;

// see: cans/02-plugin-model.md#Nested-scopes
pub struct TrapScope<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList> {
    pub(crate) builder: KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params>,
}

impl<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList>
    TrapScope<Plugins, Params, Mode, Ctx, Deps>
{
    pub fn on_trap<F>(mut self, f: F) -> Self
    where
        F: Fn(&TrapEvent, &HandlerCtx) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_trap(f);
        self
    }

    pub fn on_process_exit<F>(mut self, f: F) -> Self
    where
        F: Fn(&ProcessExit, &HandlerCtx) -> Result<(), PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_process_exit(f);
        self
    }

    pub fn requires<P>(mut self) -> Self
    where
        P: KelirPlugin,
    {
        self.builder = self.builder.scope_require::<P>();
        self
    }

    pub fn fork_with<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> Box<dyn AnyPlugin> + Send + Sync + 'static,
    {
        self.builder = self.builder.fork_with(f);
        self
    }

    pub fn timeout(mut self, d: Duration) -> Self {
        self.builder = self.builder.scope_timeout("trap", d);
        self
    }

    pub fn priority(mut self, i: i32) -> Self {
        self.builder = self.builder.scope_priority("trap", i);
        self
    }

    pub fn end(self) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params> {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Nested-scopes
pub struct FsScope<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList> {
    pub(crate) builder: KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params>,
}

impl<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList>
    FsScope<Plugins, Params, Mode, Ctx, Deps>
{
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

    pub fn on_fs_unlink<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, &HandlerCtx) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_fs_unlink(f);
        self
    }

    pub fn on_fs_rename<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, &Path, &HandlerCtx) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_fs_rename(f);
        self
    }

    // see: cans/05-storage-plane.md#Host-inode-lock-broker
    pub fn config(mut self, cfg: FsScopeConfig) -> Self {
        self.builder = self.builder.fs_config(cfg);
        self
    }

    pub fn requires<P>(mut self) -> Self
    where
        P: KelirPlugin,
    {
        self.builder = self.builder.scope_require::<P>();
        self
    }

    pub fn fork_with<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> Box<dyn AnyPlugin> + Send + Sync + 'static,
    {
        self.builder = self.builder.fork_with(f);
        self
    }

    pub fn timeout(mut self, d: Duration) -> Self {
        self.builder = self.builder.scope_timeout("fs", d);
        self
    }

    pub fn priority(mut self, i: i32) -> Self {
        self.builder = self.builder.scope_priority("fs", i);
        self
    }

    // see: cans/02-plugin-model.md#Nested-scopes
    pub fn rules<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut RuleScope),
    {
        let mut scope = RuleScope::default();
        f(&mut scope);
        self.builder = self.builder.scope_rules("fs", scope.rules);
        self
    }

    pub fn end(self) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params> {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Nested-scopes
#[derive(Debug, Clone, Default)]
pub struct FsScopeConfig {
    pub lock_broker: Option<LockBrokerConfig>,
}

// see: cans/02-plugin-model.md#Nested-scopes
#[derive(Debug, Clone, Default)]
pub struct ReadRules {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct WriteRules {
    pub mask: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ScopeRules {
    pub read: ReadRules,
    pub write: WriteRules,
}

// see: cans/02-plugin-model.md#Nested-scopes
#[derive(Default)]
pub struct RuleScope {
    pub(crate) rules: ScopeRules,
}

impl RuleScope {
    pub fn read(&mut self) -> &mut ReadRules {
        &mut self.rules.read
    }

    pub fn write(&mut self) -> &mut WriteRules {
        &mut self.rules.write
    }
}

impl ReadRules {
    pub fn allow(&mut self, glob: &str) -> &mut Self {
        self.allow.push(glob.to_string());
        self
    }

    pub fn deny(&mut self, glob: &str) -> &mut Self {
        self.deny.push(glob.to_string());
        self
    }
}

impl WriteRules {
    pub fn mask(&mut self, regex: &str) -> &mut Self {
        self.mask.push(regex.to_string());
        self
    }
}

// see: cans/02-plugin-model.md#Nested-scopes
pub struct NetScope<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList> {
    pub(crate) builder: KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params>,
}

impl<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList>
    NetScope<Plugins, Params, Mode, Ctx, Deps>
{
    pub fn on_net_outbound<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut [u8]) -> Result<FilterAction, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_net_outbound(f);
        self
    }

    pub fn on_net_inbound<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut [u8]) -> Result<FilterAction, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_net_inbound(f);
        self
    }

    pub fn requires<P>(mut self) -> Self
    where
        P: KelirPlugin,
    {
        self.builder = self.builder.scope_require::<P>();
        self
    }

    pub fn timeout(mut self, d: Duration) -> Self {
        self.builder = self.builder.scope_timeout("net", d);
        self
    }

    pub fn priority(mut self, i: i32) -> Self {
        self.builder = self.builder.scope_priority("net", i);
        self
    }

    pub fn end(self) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params> {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Nested-scopes
pub struct ConsoleScope<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList> {
    pub(crate) builder: KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params>,
}

impl<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList>
    ConsoleScope<Plugins, Params, Mode, Ctx, Deps>
{
    pub fn on_console_input<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_console_input(f);
        self
    }

    pub fn on_console_output<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_console_output(f);
        self
    }

    pub fn requires<P>(mut self) -> Self
    where
        P: KelirPlugin,
    {
        self.builder = self.builder.scope_require::<P>();
        self
    }

    pub fn timeout(mut self, d: Duration) -> Self {
        self.builder = self.builder.scope_timeout("console", d);
        self
    }

    pub fn priority(mut self, i: i32) -> Self {
        self.builder = self.builder.scope_priority("console", i);
        self
    }

    pub fn end(self) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params> {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Nested-scopes
pub struct ExecScope<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList> {
    pub(crate) builder: KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params>,
}

impl<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList>
    ExecScope<Plugins, Params, Mode, Ctx, Deps>
{
    pub fn on_exec_stdout<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, u64, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_exec_stdout(f);
        self
    }

    pub fn on_exec_stderr<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, u64, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_exec_stderr(f);
        self
    }

    pub fn requires<P>(mut self) -> Self
    where
        P: KelirPlugin,
    {
        self.builder = self.builder.scope_require::<P>();
        self
    }

    pub fn timeout(mut self, d: Duration) -> Self {
        self.builder = self.builder.scope_timeout("exec", d);
        self
    }

    pub fn priority(mut self, i: i32) -> Self {
        self.builder = self.builder.scope_priority("exec", i);
        self
    }

    pub fn end(self) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params> {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Nested-scopes
pub struct HostScope<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList> {
    pub(crate) builder: KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params>,
}

impl<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList>
    HostScope<Plugins, Params, Mode, Ctx, Deps>
{
    pub fn on_host_lsm_exec<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, u32) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_host_lsm_exec(f);
        self
    }

    pub fn on_host_pressure<F>(mut self, f: F) -> Self
    where
        F: Fn(u32) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_host_pressure(f);
        self
    }

    pub fn on_host_keyring_access<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_host_keyring_access(f);
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
        self.builder = self.builder.on_host_preempt(f);
        self
    }

    pub fn requires<P>(mut self) -> Self
    where
        P: KelirPlugin,
    {
        self.builder = self.builder.scope_require::<P>();
        self
    }

    pub fn timeout(mut self, d: Duration) -> Self {
        self.builder = self.builder.scope_timeout("host", d);
        self
    }

    pub fn priority(mut self, i: i32) -> Self {
        self.builder = self.builder.scope_priority("host", i);
        self
    }

    pub fn end(self) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params> {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Nested-scopes
pub struct LifecycleScope<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList> {
    pub(crate) builder: KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params>,
}

impl<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList>
    LifecycleScope<Plugins, Params, Mode, Ctx, Deps>
{
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
        self.builder = self.builder.on_vm_init(f);
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
        self.builder = self.builder.on_vm_ready(f);
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
        self.builder = self.builder.on_vm_shutdown(f);
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
        self.builder = self.builder.on_pre_commit(f);
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
        self.builder = self.builder.on_post_commit(f);
        self
    }

    pub fn register_veto(
        mut self,
        event: VetoEvent,
        callback: Box<dyn Fn() -> bool + Send + Sync>,
    ) -> Self {
        // see: cans/08-lifecycle.md#Lifecycle-vetoes
        let desugared = Box::new(move || Veto {
            allowed: callback(),
            by: String::new(),
            reason: String::new(),
            recovery: None,
        });
        self.builder.registration.vetoes.push((event, desugared));
        self
    }

    pub fn requires<P>(mut self) -> Self
    where
        P: KelirPlugin,
    {
        self.builder = self.builder.scope_require::<P>();
        self
    }

    pub fn fork_with<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> Box<dyn AnyPlugin> + Send + Sync + 'static,
    {
        self.builder = self.builder.fork_with(f);
        self
    }

    pub fn timeout(mut self, d: Duration) -> Self {
        self.builder = self.builder.scope_timeout("lifecycle", d);
        self
    }

    pub fn priority(mut self, i: i32) -> Self {
        self.builder = self.builder.scope_priority("lifecycle", i);
        self
    }

    pub fn end(self) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params> {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Nested-scopes
pub struct TelemetryScope<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList> {
    pub(crate) builder: KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params>,
}

impl<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList>
    TelemetryScope<Plugins, Params, Mode, Ctx, Deps>
{
    pub fn on_memory_pressure<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(u32, u32) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.builder = self.builder.on_memory_pressure(f);
        self
    }

    pub fn on_heartbeat<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(&Heartbeat) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.builder = self.builder.on_heartbeat(f);
        self
    }

    pub fn on_watchdog<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(&WatchdogEvent) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<WatchdogVerdict, PluginError>> + Send + 'static,
    {
        self.builder = self.builder.on_watchdog(f);
        self
    }

    pub fn requires<P>(mut self) -> Self
    where
        P: KelirPlugin,
    {
        self.builder = self.builder.scope_require::<P>();
        self
    }

    pub fn timeout(mut self, d: Duration) -> Self {
        self.builder = self.builder.scope_timeout("telemetry", d);
        self
    }

    pub fn priority(mut self, i: i32) -> Self {
        self.builder = self.builder.scope_priority("telemetry", i);
        self
    }

    pub fn end(self) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params> {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Nested-scopes
pub struct ActionsScope<Plugins: HList, Params: 'static, Ctx, Deps: HList> {
    pub(crate) builder: KelirBuilder<Host, Unsealed, Ctx, Deps, Plugins, Params>,
}

impl<Plugins: HList, Params: 'static, Ctx, Deps: HList> ActionsScope<Plugins, Params, Ctx, Deps> {
    pub fn register<T, F, Fut>(mut self, name: &str, schema: serde_json::Value, f: F) -> Self
    where
        T: serde::de::DeserializeOwned + Send + 'static,
        F: Fn(T) -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'static,
    {
        self.builder = self
            .builder
            .register_custom_action::<T, F, Fut>(name, schema, f);
        self
    }

    pub fn requires<P>(mut self) -> Self
    where
        P: KelirPlugin,
    {
        self.builder = self.builder.scope_require::<P>();
        self
    }

    pub fn timeout(mut self, d: Duration) -> Self {
        self.builder = self.builder.scope_timeout("actions", d);
        self
    }

    pub fn priority(mut self, i: i32) -> Self {
        self.builder = self.builder.scope_priority("actions", i);
        self
    }

    pub fn end(self) -> KelirBuilder<Host, Unsealed, Ctx, Deps, Plugins, Params> {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Nested-scopes
pub struct StreamsScope<Plugins: HList, Params: 'static, Ctx, Deps: HList> {
    pub(crate) builder: KelirBuilder<Host, Unsealed, Ctx, Deps, Plugins, Params>,
}

impl<Plugins: HList, Params: 'static, Ctx, Deps: HList> StreamsScope<Plugins, Params, Ctx, Deps> {
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

    pub fn requires<P>(mut self) -> Self
    where
        P: KelirPlugin,
    {
        self.builder = self.builder.scope_require::<P>();
        self
    }

    pub fn timeout(mut self, d: Duration) -> Self {
        self.builder = self.builder.scope_timeout("streams", d);
        self
    }

    pub fn priority(mut self, i: i32) -> Self {
        self.builder = self.builder.scope_priority("streams", i);
        self
    }

    pub fn end(self) -> KelirBuilder<Host, Unsealed, Ctx, Deps, Plugins, Params> {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Nested-scopes
pub struct VsockScope<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList> {
    pub(crate) builder: KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params>,
}

impl<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList>
    VsockScope<Plugins, Params, Mode, Ctx, Deps>
{
    pub fn service<H>(mut self, spec: VsockServiceSpec, handler: H) -> Self
    where
        H: VsockServiceHandler + 'static,
    {
        self.builder = self.builder.vsock_service(spec, handler);
        self
    }

    pub fn requires<P>(mut self) -> Self
    where
        P: KelirPlugin,
    {
        self.builder = self.builder.scope_require::<P>();
        self
    }

    pub fn timeout(mut self, d: Duration) -> Self {
        self.builder = self.builder.scope_timeout("vsock", d);
        self
    }

    pub fn priority(mut self, i: i32) -> Self {
        self.builder = self.builder.scope_priority("vsock", i);
        self
    }

    pub fn end(self) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params> {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Nested-scopes
pub struct ParamsScope<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList> {
    pub(crate) builder: KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params>,
}

impl<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList>
    ParamsScope<Plugins, Params, Mode, Ctx, Deps>
{
    // see: cans/02-plugin-model.md#Nested-scopes
    pub fn model<M>(self) -> Self
    where
        M: Default + Send + Sync + 'static,
    {
        self
    }

    pub fn requires<P>(mut self) -> Self
    where
        P: KelirPlugin,
    {
        self.builder = self.builder.scope_require::<P>();
        self
    }

    pub fn timeout(mut self, d: Duration) -> Self {
        self.builder = self.builder.scope_timeout("params", d);
        self
    }

    pub fn priority(mut self, i: i32) -> Self {
        self.builder = self.builder.scope_priority("params", i);
        self
    }

    pub fn end(self) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params> {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Nested-scopes
pub struct DenyScope<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList> {
    pub(crate) builder: KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params>,
}

impl<Plugins: HList, Params: 'static, Mode, Ctx, Deps: HList>
    DenyScope<Plugins, Params, Mode, Ctx, Deps>
{
    pub fn on_trap_deny<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(&TrapEvent, &Verdict, &kelir_engine::Session) -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.builder = self.builder.on_trap_deny_ex(f);
        self
    }

    pub fn requires<P>(mut self) -> Self
    where
        P: KelirPlugin,
    {
        self.builder = self.builder.scope_require::<P>();
        self
    }

    pub fn timeout(mut self, d: Duration) -> Self {
        self.builder = self.builder.scope_timeout("deny", d);
        self
    }

    pub fn priority(mut self, i: i32) -> Self {
        self.builder = self.builder.scope_priority("deny", i);
        self
    }

    pub fn end(self) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params> {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Nested-scopes
impl<Plugins: HList, Params: 'static, Ctx, Deps: HList>
    KelirBuilder<Host, Unsealed, Ctx, Deps, Plugins, Params>
{
    pub fn actions<F>(self, configure: F) -> Self
    where
        F: FnOnce(ActionsScope<Plugins, Params, Ctx, Deps>) -> Self,
    {
        configure(ActionsScope { builder: self })
    }

    pub fn streams<F>(self, configure: F) -> Self
    where
        F: FnOnce(StreamsScope<Plugins, Params, Ctx, Deps>) -> Self,
    {
        configure(StreamsScope { builder: self })
    }

    pub fn vsock<F>(self, configure: F) -> Self
    where
        F: FnOnce(VsockScope<Plugins, Params, Host, Ctx, Deps>) -> Self,
    {
        configure(VsockScope { builder: self })
    }

    // see: cans/02-plugin-model.md#Nested-scopes
    pub fn params<T, F>(self, configure: F) -> KelirBuilder<Host, Unsealed, Ctx, Deps, Plugins, T>
    where
        T: Default + Send + Sync + 'static,
        F: FnOnce(
            ParamsScope<Plugins, T, Host, Ctx, Deps>,
        ) -> KelirBuilder<Host, Unsealed, Ctx, Deps, Plugins, T>,
    {
        let retyped = KelirBuilder {
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
        };
        configure(ParamsScope { builder: retyped })
    }

    pub fn deny<F>(self, configure: F) -> Self
    where
        F: FnOnce(DenyScope<Plugins, Params, Host, Ctx, Deps>) -> Self,
    {
        configure(DenyScope { builder: self })
    }
}

// see: cans/02-plugin-model.md#Nested-scopes
// see: cans/02-plugin-model.md#One-validation,-at-seal-and-boot
// Plugin-mode builders expose vsock() only: seal validates a mounted
// plugin's vsock spec, so the spec must be reachable on the plugin.
impl<Plugins: HList, Params: 'static, Ctx, Deps: HList>
    KelirBuilder<Plugin, Unsealed, Ctx, Deps, Plugins, Params>
{
    pub fn vsock<F>(self, configure: F) -> Self
    where
        F: FnOnce(VsockScope<Plugins, Params, Plugin, Ctx, Deps>) -> Self,
    {
        configure(VsockScope { builder: self })
    }
}
