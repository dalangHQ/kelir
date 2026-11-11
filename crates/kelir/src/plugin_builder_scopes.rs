// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
use std::future::Future;
use std::path::Path;

use kelir_interface::driver_interface::{WatchdogEvent, WatchdogVerdict};
use kelir_interface::plugin_model::{HandlerCtx, LifecycleCtx, PluginError};
use kelir_interface::protocol_wire::{FilterAction, ProcessExit, TrapEvent, Verdict};

use crate::hlist_sdk::HList;
use crate::plugin_builder::PluginBuilder;

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
pub struct TrapScope<Ctx, Deps: HList> {
    pub(crate) builder: PluginBuilder<Ctx, Deps>,
}

impl<Ctx, Deps: HList> TrapScope<Ctx, Deps> {
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
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
pub struct FsScope<Ctx, Deps: HList> {
    pub(crate) builder: PluginBuilder<Ctx, Deps>,
}

impl<Ctx, Deps: HList> FsScope<Ctx, Deps> {
    pub fn on_fs_lookup<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path) -> Result<Verdict, PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_fs_lookup(f);
        self
    }

    pub fn on_fs_read<F>(mut self, f: F) -> Self
    where
        F: Fn(&Path, &mut [u8]) -> Result<(), PluginError> + Send + Sync + 'static,
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
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
pub struct NetScope<Ctx, Deps: HList> {
    pub(crate) builder: PluginBuilder<Ctx, Deps>,
}

impl<Ctx, Deps: HList> NetScope<Ctx, Deps> {
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
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
pub struct ConsoleScope<Ctx, Deps: HList> {
    pub(crate) builder: PluginBuilder<Ctx, Deps>,
}

impl<Ctx, Deps: HList> ConsoleScope<Ctx, Deps> {
    pub fn on_console_input<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, &mut Vec<u8>) -> Result<(), PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_console_input(f);
        self
    }

    pub fn on_console_output<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, &mut Vec<u8>) -> Result<(), PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_console_output(f);
        self
    }
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
pub struct ExecScope<Ctx, Deps: HList> {
    pub(crate) builder: PluginBuilder<Ctx, Deps>,
}

impl<Ctx, Deps: HList> ExecScope<Ctx, Deps> {
    pub fn on_exec_stdout<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, u64, &mut Vec<u8>) -> Result<(), PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_exec_stdout(f);
        self
    }

    pub fn on_exec_stderr<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, u64, &mut Vec<u8>) -> Result<(), PluginError> + Send + Sync + 'static,
    {
        self.builder = self.builder.on_exec_stderr(f);
        self
    }
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
pub struct HostScope<Ctx, Deps: HList> {
    pub(crate) builder: PluginBuilder<Ctx, Deps>,
}

impl<Ctx, Deps: HList> HostScope<Ctx, Deps> {
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

    pub fn on_host_preempt<F, Fut>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(&'c mut LifecycleCtx<'i>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.builder = self.builder.on_host_preempt(f);
        self
    }
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
pub struct LifecycleScope<Ctx, Deps: HList> {
    pub(crate) builder: PluginBuilder<Ctx, Deps>,
}

impl<Ctx, Deps: HList> LifecycleScope<Ctx, Deps> {
    pub fn on_vm_init<F, Fut>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(&'c mut LifecycleCtx<'i>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.builder = self.builder.on_vm_init(f);
        self
    }

    pub fn on_vm_ready<F, Fut>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(&'c mut LifecycleCtx<'i>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.builder = self.builder.on_vm_ready(f);
        self
    }

    pub fn on_vm_shutdown<F, Fut>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(&'c mut LifecycleCtx<'i>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.builder = self.builder.on_vm_shutdown(f);
        self
    }

    pub fn on_pre_commit<F, Fut>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(&'c mut LifecycleCtx<'i>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.builder = self.builder.on_pre_commit(f);
        self
    }

    pub fn on_post_commit<F, Fut>(mut self, f: F) -> Self
    where
        F: for<'c, 'i> Fn(&'c mut LifecycleCtx<'i>, bool) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
    {
        self.builder = self.builder.on_post_commit(f);
        self
    }
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
pub struct TelemetryScope<Ctx, Deps: HList> {
    pub(crate) builder: PluginBuilder<Ctx, Deps>,
}

impl<Ctx, Deps: HList> TelemetryScope<Ctx, Deps> {
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
        F: Fn(u64) -> Fut + Send + Sync + 'static,
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
}
