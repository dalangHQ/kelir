// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use kelir_interface::driver_interface::{WatchdogEvent, WatchdogVerdict};
use kelir_interface::plugin_model::{HandlerCtx, LifecycleCtx, PluginError, VetoEvent};
use kelir_interface::protocol_wire::{FilterAction, Heartbeat, ProcessExit, TrapEvent, Verdict};

use crate::builder_sdk::{KelirBuilder, Unsealed};
use crate::hlist_sdk::HList;

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
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

    pub fn end(self) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params> {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
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

    pub fn end(self) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params> {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
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

    pub fn end(self) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params> {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
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

    pub fn end(self) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params> {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
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

    pub fn end(self) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params> {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
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

    pub fn end(self) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params> {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
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
        self.builder.registration.vetoes.push((event, callback));
        self
    }

    pub fn end(self) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params> {
        self.builder
    }
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
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

    pub fn end(self) -> KelirBuilder<Mode, Unsealed, Ctx, Deps, Plugins, Params> {
        self.builder
    }
}
