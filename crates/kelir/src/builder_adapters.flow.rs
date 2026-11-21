// see: cans/02-plugin-model.md#Segregated-ring-traits
use std::future::Future;
use std::marker::PhantomData;
use std::path::Path;
use std::pin::Pin;

use async_trait::async_trait;
use kelir_interface::driver_interface::{WatchdogEvent, WatchdogVerdict};
use kelir_interface::fusion_plugin::FilterSnapshot;
use kelir_interface::plugin_model::{
    ConsoleInterposer, FsInterposer, HandlerCtx, HostInterposer, LifecycleCtx,
    LifecycleParticipant, NetInterposer, PluginError, SystemTelemetryParticipant, TrapInterposer,
};
use kelir_interface::protocol_wire::{FilterAction, Heartbeat, ProcessExit, TrapEvent, Verdict};
use kelir_interface::step_verdict::{PostExecAction, PostExecGate};
use kelir_interface::storage::DirEntry;
use kelir_interface::wire::DnsVerdict;

use crate::trap_filter::filter_matches;

pub(crate) struct OnTrap;
pub(crate) struct OnProcessExit;
pub(crate) struct OnSignal;
pub(crate) struct OnFork;
pub(crate) struct OnFsLookup;
pub(crate) struct OnFsRead;
pub(crate) struct OnFsWrite;
pub(crate) struct OnFsLock;
pub(crate) struct OnFsUnlink;
pub(crate) struct OnFsRename;
pub(crate) struct OnFsSetattr;
pub(crate) struct OnFsSymlink;
pub(crate) struct OnFsMkdir;
pub(crate) struct OnFsReaddir;
pub(crate) struct OnFsReadlink;
pub(crate) struct OnNetOutbound;
pub(crate) struct OnNetInbound;
pub(crate) struct OnSocketConnect;
pub(crate) struct OnDnsQuery;
pub(crate) struct OnSocketBind;
pub(crate) struct OnConsoleInput;
pub(crate) struct OnConsoleOutput;
pub(crate) struct OnExecStdout;
pub(crate) struct OnExecStderr;
pub(crate) struct OnExecStdin;
pub(crate) struct OnHostLsmExec;
pub(crate) struct OnHostPressure;
pub(crate) struct OnHostKeyringAccess;
pub(crate) struct OnHostContainmentBreach;
pub(crate) struct OnVmInit;
pub(crate) struct OnVmReady;
pub(crate) struct OnVmShutdown;
pub(crate) struct OnPreCommit;
pub(crate) struct OnPostCommit;
pub(crate) struct OnHostPreempt;
pub(crate) struct OnMemoryPressure;
pub(crate) struct OnHeartbeat;
pub(crate) struct OnWatchdog;

// see: cans/02-plugin-model.md#Segregated-ring-traits
pub struct TrapClosureAdapter<F, Verb> {
    pub(crate) handler: F,
    pub(crate) verb: PhantomData<fn() -> Verb>,
}

// see: cans/02-plugin-model.md#Segregated-ring-traits
pub struct AsyncTrapClosureAdapter<F> {
    pub(crate) filter: FilterSnapshot,
    pub(crate) handler: F,
}

// see: cans/plugins/step-eval.md#Plugin-step-eval
pub struct PostExecGateAdapter<F> {
    pub(crate) filter: FilterSnapshot,
    pub(crate) handler: F,
}

// see: cans/02-plugin-model.md#Segregated-ring-traits
pub struct FsClosureAdapter<F, Verb> {
    pub(crate) handler: F,
    pub(crate) verb: PhantomData<fn() -> Verb>,
}

// see: cans/02-plugin-model.md#Segregated-ring-traits
pub struct NetClosureAdapter<F, Verb> {
    pub(crate) handler: F,
    pub(crate) verb: PhantomData<fn() -> Verb>,
}

// see: cans/02-plugin-model.md#Segregated-ring-traits
pub struct ConsoleClosureAdapter<F, Verb> {
    pub(crate) handler: F,
    pub(crate) verb: PhantomData<fn() -> Verb>,
}

// see: cans/02-plugin-model.md#Segregated-ring-traits
pub struct HostClosureAdapter<F, Verb> {
    pub(crate) handler: F,
    pub(crate) verb: PhantomData<fn() -> Verb>,
}

// see: cans/02-plugin-model.md#Segregated-ring-traits
pub struct LifecycleClosureAdapter<F, Verb> {
    pub(crate) handler: F,
    pub(crate) verb: PhantomData<fn() -> Verb>,
}

// see: cans/02-plugin-model.md#Segregated-ring-traits
pub struct TelemetryClosureAdapter<F, Verb> {
    pub(crate) handler: F,
    pub(crate) verb: PhantomData<fn() -> Verb>,
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
impl<F, Fut> TrapInterposer for AsyncTrapClosureAdapter<F>
where
    F: Fn(HandlerCtx) -> Fut + Send + Sync,
    Fut: Future<Output = Result<Verdict, PluginError>> + Send + 'static,
{
    async fn on_trap(&self, event: &TrapEvent, ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        if !filter_matches(&self.filter, event) {
            return Ok(Verdict::Pass);
        }
        (self.handler)(ctx.clone().with_trap(event.clone())).await
    }
}

#[async_trait]
impl<F, Fut> PostExecGate for PostExecGateAdapter<F>
where
    F: Fn(HandlerCtx) -> Fut + Send + Sync,
    Fut: Future<Output = Result<PostExecAction, PluginError>> + Send + 'static,
{
    async fn check(
        &self,
        event: &TrapEvent,
        ctx: &HandlerCtx,
    ) -> Result<PostExecAction, PluginError> {
        if !filter_matches(&self.filter, event) {
            return Ok(PostExecAction::Proceed);
        }
        (self.handler)(ctx.clone().with_trap(event.clone())).await
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

#[async_trait]
impl<F> TrapInterposer for TrapClosureAdapter<F, OnSignal>
where
    F: Fn(u32, u32, i32) -> Result<Verdict, PluginError> + Send + Sync,
{
    async fn on_signal(&self, sender: u32, target: u32, sig: i32) -> Result<Verdict, PluginError> {
        (self.handler)(sender, target, sig)
    }
}

#[async_trait]
impl<F> TrapInterposer for TrapClosureAdapter<F, OnFork>
where
    F: Fn(u32, u32) -> Result<Verdict, PluginError> + Send + Sync,
{
    async fn on_fork(&self, parent: u32, child: u32) -> Result<Verdict, PluginError> {
        (self.handler)(parent, child)
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
    F: Fn(&Path, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync,
{
    fn on_fs_read(&self, path: &Path, buffer: &[u8]) -> Result<Vec<u8>, PluginError> {
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

impl<F> FsInterposer for FsClosureAdapter<F, OnFsUnlink>
where
    F: Fn(&Path, &HandlerCtx) -> Result<Verdict, PluginError> + Send + Sync,
{
    fn on_fs_unlink(&self, path: &Path, ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        (self.handler)(path, ctx)
    }
}

impl<F> FsInterposer for FsClosureAdapter<F, OnFsRename>
where
    F: Fn(&Path, &Path, &HandlerCtx) -> Result<Verdict, PluginError> + Send + Sync,
{
    fn on_fs_rename(
        &self,
        src: &Path,
        dst: &Path,
        ctx: &HandlerCtx,
    ) -> Result<Verdict, PluginError> {
        (self.handler)(src, dst, ctx)
    }
}

impl<F> FsInterposer for FsClosureAdapter<F, OnFsSetattr>
where
    F: Fn(&Path, u32, u32, u32, u64, &HandlerCtx) -> Result<Verdict, PluginError> + Send + Sync,
{
    fn on_fs_setattr(&self, path: &Path, mode: u32, size: u64) -> Result<Verdict, PluginError> {
        // TODO: [plugin_engine] fill in the L3 wave; thread the live uid, gid and HandlerCtx from the FUSE setattr dispatch; the trait carries path, mode and size only.; slop: setattr handler sees zero uid/gid and a synthetic ctx.
        let ctx = HandlerCtx::new([0; 16], 0, (), Vec::new());
        (self.handler)(path, mode, 0, 0, size, &ctx)
    }
}

impl<F> FsInterposer for FsClosureAdapter<F, OnFsSymlink>
where
    F: Fn(&Path, &Path) -> Result<Verdict, PluginError> + Send + Sync,
{
    fn on_fs_symlink(&self, target: &Path, link: &Path) -> Result<Verdict, PluginError> {
        (self.handler)(target, link)
    }
}

impl<F> FsInterposer for FsClosureAdapter<F, OnFsMkdir>
where
    F: Fn(&Path, u32) -> Result<Verdict, PluginError> + Send + Sync,
{
    fn on_fs_mkdir(&self, path: &Path, mode: u32) -> Result<Verdict, PluginError> {
        (self.handler)(path, mode)
    }
}

impl<F> FsInterposer for FsClosureAdapter<F, OnFsReaddir>
where
    F: Fn(&Path, &mut Vec<DirEntry>) -> Result<FilterAction, PluginError> + Send + Sync,
{
    fn on_fs_readdir(
        &self,
        path: &Path,
        entries: &mut Vec<DirEntry>,
    ) -> Result<FilterAction, PluginError> {
        (self.handler)(path, entries)
    }
}

impl<F> FsInterposer for FsClosureAdapter<F, OnFsReadlink>
where
    F: Fn(&Path) -> Result<Verdict, PluginError> + Send + Sync,
{
    fn on_fs_readlink(&self, path: &Path) -> Result<Verdict, PluginError> {
        (self.handler)(path)
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

impl<F> NetInterposer for NetClosureAdapter<F, OnSocketConnect>
where
    F: Fn(u32, u8, [u8; 4], u16) -> Result<Verdict, PluginError> + Send + Sync,
{
    fn on_socket_connect(
        &self,
        pid: u32,
        proto: u8,
        ip: [u8; 4],
        port: u16,
    ) -> Result<Verdict, PluginError> {
        (self.handler)(pid, proto, ip, port)
    }
}

impl<F> NetInterposer for NetClosureAdapter<F, OnDnsQuery>
where
    F: Fn(u32, &str, u16) -> Result<DnsVerdict, PluginError> + Send + Sync,
{
    fn on_dns_query(&self, pid: u32, domain: &str, qtype: u16) -> Result<DnsVerdict, PluginError> {
        (self.handler)(pid, domain, qtype)
    }
}

impl<F> NetInterposer for NetClosureAdapter<F, OnSocketBind>
where
    F: Fn(u32, u8, [u8; 4], u16) -> Result<Verdict, PluginError> + Send + Sync,
{
    fn on_socket_bind(
        &self,
        pid: u32,
        proto: u8,
        ip: [u8; 4],
        port: u16,
    ) -> Result<Verdict, PluginError> {
        (self.handler)(pid, proto, ip, port)
    }
}

#[async_trait]
impl<F> ConsoleInterposer for ConsoleClosureAdapter<F, OnConsoleInput>
where
    F: Fn(u32, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync,
{
    async fn on_console_input(&self, port: u32, bytes: &[u8]) -> Result<Vec<u8>, PluginError> {
        (self.handler)(port, bytes)
    }
}

#[async_trait]
impl<F> ConsoleInterposer for ConsoleClosureAdapter<F, OnConsoleOutput>
where
    F: Fn(u32, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync,
{
    async fn on_console_output(&self, port: u32, chunk: &[u8]) -> Result<Vec<u8>, PluginError> {
        (self.handler)(port, chunk)
    }
}

#[async_trait]
impl<F> ConsoleInterposer for ConsoleClosureAdapter<F, OnExecStdout>
where
    F: Fn(u32, u64, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync,
{
    async fn on_exec_stdout(
        &self,
        pid: u32,
        exec_id: u64,
        chunk: &[u8],
    ) -> Result<Vec<u8>, PluginError> {
        (self.handler)(pid, exec_id, chunk)
    }
}

#[async_trait]
impl<F> ConsoleInterposer for ConsoleClosureAdapter<F, OnExecStderr>
where
    F: Fn(u32, u64, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync,
{
    async fn on_exec_stderr(
        &self,
        pid: u32,
        exec_id: u64,
        chunk: &[u8],
    ) -> Result<Vec<u8>, PluginError> {
        (self.handler)(pid, exec_id, chunk)
    }
}

#[async_trait]
impl<F> ConsoleInterposer for ConsoleClosureAdapter<F, OnExecStdin>
where
    F: Fn(u32, u64, &[u8]) -> Result<Vec<u8>, PluginError> + Send + Sync,
{
    async fn on_exec_stdin(
        &self,
        pid: u32,
        exec_id: u64,
        chunk: &[u8],
    ) -> Result<Vec<u8>, PluginError> {
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

impl<F> HostInterposer for HostClosureAdapter<F, OnHostContainmentBreach>
where
    F: Fn(u32, &Path, &str) -> Result<(), PluginError> + Send + Sync,
{
    fn on_host_containment_breach(
        &self,
        pid: u32,
        path: &Path,
        kind: &str,
    ) -> Result<(), PluginError> {
        (self.handler)(pid, path, kind)
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
    F: Fn(&Heartbeat) -> Fut + Send + Sync,
    Fut: Future<Output = Result<(), PluginError>> + Send + 'static,
{
    async fn on_heartbeat(&self, event: &Heartbeat) -> Result<(), PluginError> {
        (self.handler)(event).await
    }
}

#[async_trait]
impl<F, Fut> SystemTelemetryParticipant for TelemetryClosureAdapter<F, OnWatchdog>
where
    F: Fn(&WatchdogEvent) -> Fut + Send + Sync,
    Fut: Future<Output = Result<WatchdogVerdict, PluginError>> + Send + 'static,
{
    async fn on_watchdog(&self, event: &WatchdogEvent) -> Result<WatchdogVerdict, PluginError> {
        (self.handler)(event).await
    }
}
