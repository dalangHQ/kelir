use std::path::Path;

use async_trait::async_trait;

pub use kelir_interface::driver::{WatchdogEvent, WatchdogVerdict};
pub use kelir_interface::errors::PluginError;
pub use kelir_interface::plugin::VetoEvent;
pub use kelir_interface::protocol::{FilterAction, ProcessExit, TrapEvent, Verdict};

use crate::HandlerCtx;

pub trait PluginClone: Send + Sync {
    fn clone_box(&self) -> Box<dyn AnyPlugin>;
}

impl<T> PluginClone for T
where
    T: 'static + KelirPlugin + Clone,
{
    fn clone_box(&self) -> Box<dyn AnyPlugin> {
        Box::new(self.clone())
    }
}

pub trait AnyPlugin: Send + Sync {
    fn name(&self) -> &'static str;
    fn fork_state(&self, _child_session_id: &str) -> Box<dyn AnyPlugin>;
}

impl<T> AnyPlugin for T
where
    T: 'static + KelirPlugin + Clone,
{
    fn name(&self) -> &'static str {
        KelirPlugin::name(self)
    }

    fn fork_state(&self, child_session_id: &str) -> Box<dyn AnyPlugin> {
        KelirPlugin::fork_state(self, child_session_id)
    }
}

pub trait KelirPlugin: Send + Sync + PluginClone {
    type Context;
    fn context(&self) -> Self::Context
    where
        Self: Sized;
    fn name(&self) -> &'static str;
    fn fork_state(&self, _child_session_id: &str) -> Box<dyn AnyPlugin> {
        self.clone_box()
    }
}

#[derive(Default)]
pub struct LifecycleCtx {
    vetoes: Vec<(VetoEvent, Box<dyn Fn() -> bool + Send + Sync>)>,
}

impl LifecycleCtx {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_veto(
        &mut self,
        event: VetoEvent,
        callback: Box<dyn Fn() -> bool + Send + Sync>,
    ) {
        self.vetoes.push((event, callback));
    }
}

#[async_trait]
pub trait LifecycleParticipant: Send + Sync {
    async fn on_vm_init(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        Ok(())
    }
    async fn on_vm_ready(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        Ok(())
    }
    async fn on_vm_shutdown(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        Ok(())
    }
    async fn on_pre_commit(&self) -> Result<(), PluginError> {
        Ok(())
    }
    async fn on_post_commit(&self, _success: bool) -> Result<(), PluginError> {
        Ok(())
    }
    async fn on_host_preempt(&self) -> Result<(), PluginError> {
        Ok(())
    }
}

#[async_trait]
pub trait TrapInterposer: Send + Sync {
    async fn on_trap(&self, _event: &TrapEvent, _ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        Ok(Verdict::Pass)
    }
    async fn on_process_exit(
        &self,
        _event: &ProcessExit,
        _ctx: &HandlerCtx,
    ) -> Result<(), PluginError> {
        Ok(())
    }
}

#[async_trait]
pub trait ConsoleInterposer: Send + Sync {
    async fn on_console_input(&self, _port: u32, _bytes: &mut Vec<u8>) -> Result<(), PluginError> {
        Ok(())
    }
    async fn on_console_output(&self, _port: u32, _chunk: &mut Vec<u8>) -> Result<(), PluginError> {
        Ok(())
    }
    async fn on_exec_stdout(
        &self,
        _pid: u32,
        _exec_id: u64,
        _chunk: &mut Vec<u8>,
    ) -> Result<(), PluginError> {
        Ok(())
    }
    async fn on_exec_stderr(
        &self,
        _pid: u32,
        _exec_id: u64,
        _chunk: &mut Vec<u8>,
    ) -> Result<(), PluginError> {
        Ok(())
    }
}

#[async_trait]
pub trait SystemTelemetryParticipant: Send + Sync {
    async fn on_memory_pressure(
        &self,
        _available_mb: u32,
        _total_mb: u32,
    ) -> Result<(), PluginError> {
        Ok(())
    }
    async fn on_heartbeat(&self, _uptime_ms: u64) -> Result<(), PluginError> {
        Ok(())
    }
    async fn on_watchdog(&self, _event: &WatchdogEvent) -> Result<WatchdogVerdict, PluginError> {
        Ok(WatchdogVerdict::ExecuteDefault)
    }
}

pub trait FsInterposer: Send + Sync {
    fn on_fs_lookup(&self, _path: &Path) -> Result<Verdict, PluginError> {
        Ok(Verdict::Pass)
    }
    fn on_fs_read(&self, _path: &Path, _buffer: &mut [u8]) -> Result<(), PluginError> {
        Ok(())
    }
    fn on_fs_write(
        &self,
        _path: &Path,
        _data: &mut [u8],
        _pid: u32,
        _ctx: &HandlerCtx,
    ) -> Result<FilterAction, PluginError> {
        Ok(FilterAction::Forward)
    }
    fn on_fs_lock(
        &self,
        _path: &Path,
        _flock_cmd: i32,
        _flock_type: i16,
        _pid: u32,
    ) -> Result<Verdict, PluginError> {
        Ok(Verdict::Pass)
    }
}

pub trait NetInterposer: Send + Sync {
    fn on_net_outbound(&self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        Ok(FilterAction::Forward)
    }
    fn on_net_inbound(&self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        Ok(FilterAction::Forward)
    }
}

pub trait HostInterposer: Send + Sync {
    fn on_host_lsm_exec(&self, _path: &Path, _pid: u32) -> Result<Verdict, PluginError> {
        Ok(Verdict::Pass)
    }
    fn on_host_pressure(&self, _level: u32) -> Result<Verdict, PluginError> {
        Ok(Verdict::Pass)
    }
    fn on_host_keyring_access(&self, _key: &str) -> Result<Verdict, PluginError> {
        Ok(Verdict::Pass)
    }
}
