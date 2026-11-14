use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use kelir_interface::errno_types::Errno;
use kelir_interface::handlers_plugin::Capability;
pub use kelir_interface::handles_lifecycle::{
    ClockHandle, FsHandle, StepStack, TerminalHandle, VmHandle,
};
use kelir_interface::hex_codec::encode_hex;
use kelir_interface::plugin_model::{LifecycleEvent, MountScope, PipelineBinder, PluginError};
use kelir_interface::spawn_transport::{SpawnRequest, SpawnTransport};
use serde::Serialize;

use crate::builder_sdk::Registration;
use crate::hlist_sdk::{HList, HNil, NoParams};
use kelir_interface::plugin_model::HandlerCtx;
use kelir_interface::protocol_wire::{ProcessLimits, StreamKind, TrapEvent, Verdict};

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
pub trait Plane: std::any::Any {
    fn plane_kind(&self) -> &'static str;
}

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
impl Plane for kelir_engine::planes::terminal::TerminalPlane {
    fn plane_kind(&self) -> &'static str {
        "terminal"
    }
}

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
impl Plane for kelir_engine::planes::storage::StoragePlane {
    fn plane_kind(&self) -> &'static str {
        "storage"
    }
}

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
impl Plane for kelir_engine::planes::wire::WirePlane {
    fn plane_kind(&self) -> &'static str {
        "wire"
    }
}

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
impl Plane for kelir_engine::planes::checkpoint::CheckpointPlane {
    fn plane_kind(&self) -> &'static str {
        "checkpoint"
    }
}

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
impl Plane for kelir_engine::planes::host::HostPlane {
    fn plane_kind(&self) -> &'static str {
        "host"
    }
}

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
impl Plane for VmHandle {
    fn plane_kind(&self) -> &'static str {
        "vm"
    }
}

// see: cans/03-sdk-architecture.md#Session-surfaces
pub struct Session<Plugins = HNil, Params = NoParams>
where
    Plugins: HList,
{
    core: kelir_engine::Session,
    plugins: Plugins,
    registration: Registration,
    // see: cans/03-sdk-architecture.md#Session-surfaces
    pub vm: VmHandle,
    pub vsock: VsockHandle,
    _params: PhantomData<fn() -> Params>,
}

impl<Plugins: HList, Params> Session<Plugins, Params> {
    pub(crate) fn from_parts(
        core: kelir_engine::Session,
        plugins: Plugins,
        registration: Registration,
    ) -> Self {
        Self {
            vm: core.vm(),
            vsock: VsockHandle,
            core,
            plugins,
            registration,
            _params: PhantomData,
        }
    }

    // see: cans/03-sdk-architecture.md#Service-lookup-across-languages
    pub fn cap<P: 'static>(&self) -> Option<&P>
    where
        Plugins: Capability<P>,
    {
        self.plugins.capability()
    }

    // see: cans/03-sdk-architecture.md#Service-lookup-across-languages
    pub fn plane<P: Plane>(&self) -> Option<&P> {
        match std::any::type_name::<P>() {
            "kelir_engine::planes::terminal::TerminalPlane" => {
                self.core.terminal.as_ref() as &dyn std::any::Any
            }
            "kelir_engine::planes::storage::StoragePlane" => {
                &self.core.storage as &dyn std::any::Any
            }
            "kelir_engine::planes::wire::WirePlane" => &self.core.wire as &dyn std::any::Any,
            "kelir_engine::planes::checkpoint::CheckpointPlane" => {
                self.core.checkpoint.as_ref() as &dyn std::any::Any
            }
            "kelir_engine::planes::host::HostPlane" => &self.core.host as &dyn std::any::Any,
            "kelir_interface::handles_lifecycle::VmHandle" => &self.vm as &dyn std::any::Any,
            _ => return None,
        }
        .downcast_ref::<P>()
    }

    // see: cans/03-sdk-architecture.md#Service-lookup-across-languages
    pub fn plane_named(&self, name: &str) -> Option<&dyn std::any::Any> {
        match name {
            "vm" => Some(&self.vm),
            "terminal" => Some(self.core.terminal.as_ref()),
            "storage" => Some(&self.core.storage),
            "wire" => Some(&self.core.wire),
            "checkpoint" => Some(self.core.checkpoint.as_ref()),
            "host" => Some(&self.core.host),
            "vsock" => Some(&self.vsock),
            _ => None,
        }
    }

    pub fn core(&self) -> &kelir_engine::Session {
        &self.core
    }

    // see: cans/10-attestation.md#Execution-receipts
    pub fn store_receipt(&self, receipt: kelir_interface::telemetry::ExecutionReceipt) {
        self.core.store_receipt(receipt);
    }

    // see: cans/10-attestation.md#Execution-receipts
    pub fn receipt(&self, pid: u32) -> Option<kelir_interface::telemetry::ExecutionReceipt> {
        self.core.receipt(pid)
    }

    // see: cans/03-sdk-architecture.md#Session-surfaces
    pub fn vm(&self) -> VmHandle {
        self.vm.clone()
    }

    // see: cans/03-sdk-architecture.md#Idiomatic-Surfaces-Law
    pub async fn spawn(&self, binary: &str, argv: &[&str]) -> Result<SpawnResult, PluginError> {
        self.exec(binary, argv, None).await
    }

    // see: cans/03-sdk-architecture.md#Session-surfaces
    pub fn exec(&self, binary: &str, argv: &[&str], limits: Option<ProcessLimits>) -> SpawnBuilder {
        let mut builder = self.spawn_builder(binary);
        builder.args.extend(argv.iter().map(|arg| arg.to_string()));
        builder.limits = limits;
        builder
    }

    // see: cans/08-lifecycle.md#Clock-skew
    pub async fn skew_clock(&self, delta: Duration) -> Result<(), PluginError> {
        self.vm().clock.skew(delta).await
    }

    // see: cans/09-snapshots.md#Groups-and-lineage
    pub fn lineage(&self, pid: u32) -> crate::PidLineage {
        self.core.lineage(pid)
    }

    // see: cans/08-lifecycle.md#Rollback
    pub async fn rollback_here(&mut self, pid: u32) -> Result<(), PluginError> {
        let lineage = self.core.lineage(pid);
        self.core.rollback(lineage).await
    }

    // see: cans/03-sdk-architecture.md#Session-surfaces
    // TODO (L2): surface the step handle as a Session property per cans/03-sdk-architecture.md#Session-surfaces.
    pub fn vsock(&self) -> VsockHandle {
        VsockHandle
    }

    // see: cans/03-sdk-architecture.md#Idiomatic-Surfaces-Law
    pub fn spawn_builder(&self, binary: &str) -> SpawnBuilder {
        SpawnBuilder::default()
            .args([binary])
            .bind(self.core.session_id(), StreamKind::Exec)
            .with_transport(self.core.spawn_transport())
    }

    // see: cans/02-plugin-model.md#Static-dispatch
    pub async fn dispatch_trap<Params2>(
        &self,
        event: &TrapEvent,
        ctx: &mut HandlerCtx<Plugins, Params2>,
    ) -> Result<Verdict, PluginError>
    where
        Plugins: HList,
        Params2: 'static,
    {
        let lineage = self.core.lineage(ctx.pid);
        self.core.pipeline().hydrate_step(&mut *ctx, lineage)?;
        self.core
            .pipeline()
            .dispatch_trap_typed_with_session(event, ctx, &self.core)
            .await
    }

    // see: cans/08-lifecycle.md#State-machine
    pub async fn shutdown(&mut self) -> Result<(), PluginError> {
        self.core.shutdown().await
    }

    // see: cans/08-lifecycle.md#Hypervisor-primitives
    pub fn terminate(&mut self) -> Result<(), PluginError> {
        self.core.terminate()
    }

    // see: cans/03-sdk-architecture.md#Rust-SDK
    // see: cans/08-lifecycle.md#CoW-fork
    pub async fn fork(&self) -> Result<Session<Plugins, Params>, PluginError> {
        let mut child = self.core.fork().await?;
        let child_session_id = encode_hex(&child.session_id());
        // see: cans/08-lifecycle.md#CoW-fork
        let mut registration = Registration {
            instances: Vec::new(),
            mounted: Vec::new(),
            storage_cfg: self.registration.storage_cfg.clone(),
            terminal_cfg: self.registration.terminal_cfg.clone(),
            wire_cfg: self.registration.wire_cfg.clone(),
            lock_broker_cfg: self.registration.lock_broker_cfg.clone(),
            on_trap_deny: None,
            custom_actions: Vec::new(),
            stream_kind_services: Vec::new(),
            // see: cans/08-lifecycle.md#CoW-fork
            vetoes: Vec::new(),
            detached: self.registration.detached,
        };
        for (plugin, scope) in &self.registration.instances {
            // TODO (L2): fork_state is synchronous on the runtime thread; clone state without blocking_lock/block_on per cans/02-plugin-model.md#KelirPlugin-trait.
            let forked = plugin.fork_state(&child_session_id);
            if forked.is_enabled() {
                child.pipeline_mut().set_active_scope(scope.clone());
                forked.register_interceptors(child.pipeline_mut());
            }
            registration.instances.push((forked, scope.clone()));
        }
        for mounted in &self.registration.mounted {
            match &mounted.fork {
                Some(fork) => {
                    let forked = fork(&child_session_id);
                    if forked.is_enabled() {
                        child.pipeline_mut().set_active_scope(mounted.scope.clone());
                        forked.register_interceptors(child.pipeline_mut());
                    }
                    registration.instances.push((forked, mounted.scope.clone()));
                }
                None => {
                    child.pipeline_mut().set_active_scope(mounted.scope.clone());
                    mounted.bind(child.pipeline_mut());
                    registration.mounted.push(mounted.clone());
                }
            }
        }
        child.pipeline_mut().set_active_scope(MountScope::default());
        child.pipeline_mut().seal()?;
        // see: cans/11-protocol.md#Kind-registration
        child.bind_stream_kind_sinks()?;
        // see: cans/08-lifecycle.md#CoW-fork
        // see: cans/08-lifecycle.md#Watchdog
        child.run_lifecycle_hook(LifecycleEvent::VmInit).await?;
        Ok(Self {
            vm: child.vm(),
            vsock: VsockHandle,
            core: child,
            plugins: self.plugins.clone(),
            registration,
            _params: PhantomData,
        })
    }
}

impl Session<HNil, NoParams> {
    // see: cans/03-sdk-architecture.md#Session-surfaces
    pub async fn attach(session_id: &[u8; 16]) -> Result<Self, PluginError> {
        let core = kelir_engine::Session::attach(session_id)?;
        Ok(Self::from_parts(core, HNil, Registration::default()))
    }
}

// see: cans/plugins/ledger.md#Canonical-accessor-(SSOT-A1-pin)
#[cfg(feature = "plugin-ledger")]
impl<Plugins: HList, Params> Session<Plugins, Params> {
    // see: cans/plugins/ledger.md#Causal-delta-access-(ledger)
    pub async fn get_causal_delta_from_parent(
        &self,
    ) -> Result<kelir_interface::telemetry::CausalDelta, PluginError>
    where
        Plugins: Capability<plugin_ledger::LedgerCtx>,
    {
        let ledger = self
            .cap::<plugin_ledger::LedgerCtx>()
            .ok_or_else(|| PluginError::Halt("ECONFIG: ledger capability not mounted".into()))?;
        ledger.get_causal_delta_from_parent().await
    }

    // see: cans/plugins/ledger.md#Ledger-handler-API
    pub async fn count_denials(&self, ppid: u32) -> Result<u64, PluginError>
    where
        Plugins: Capability<plugin_ledger::LedgerCtx>,
    {
        let ledger = self
            .cap::<plugin_ledger::LedgerCtx>()
            .ok_or_else(|| PluginError::Halt("ECONFIG: ledger capability not mounted".into()))?;
        ledger.count_denials(ppid).await
    }
}

impl<Plugins: HList, Params> std::ops::Deref for Session<Plugins, Params> {
    type Target = kelir_engine::Session;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl<Plugins: HList, Params> std::ops::DerefMut for Session<Plugins, Params> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.core
    }
}

// see: cans/03-sdk-architecture.md#Idiomatic-Surfaces-Law
#[derive(Debug, Clone, Default)]
pub struct SpawnResult {
    pub exit_code: i32,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct VsockHandle;

impl VsockHandle {
    // TODO (L3) [wire_plane]: open the multiplexed vsock channel for the stream kind per cans/03-sdk-architecture.md#Session-surfaces.; slop: channel Ok, multiplexed vsock bytes never flow
    pub async fn channel(&self, _kind: u16) -> Result<VsockChannel, PluginError> {
        Err(PluginError::Halt("TODO (L3): VsockHandle::channel".into()))
    }
}

pub struct VsockChannel {
    inner: tokio::io::DuplexStream,
}

impl tokio::io::AsyncRead for VsockChannel {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_read(cx, buf)
    }
}

impl tokio::io::AsyncWrite for VsockChannel {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

// see: cans/11-protocol.md#Binary-exec-channel
#[derive(Default)]
pub struct SpawnBuilder {
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub session_id: Option<[u8; 16]>,
    pub stream_kind: StreamKind,
    pub limits: Option<ProcessLimits>,
    pub transport: Option<Arc<dyn SpawnTransport>>,
    pub params_bytes: Vec<u8>,
    params_error: Option<String>,
}

impl SpawnBuilder {
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.args = args.into_iter().map(|a| a.as_ref().to_string()).collect();
        self
    }

    pub fn cwd(mut self, cwd: &str) -> Self {
        self.cwd = Some(cwd.to_string());
        self
    }

    // see: cans/11-protocol.md#Binary-exec-channel
    pub fn bind(mut self, session_id: [u8; 16], stream_kind: StreamKind) -> Self {
        self.session_id = Some(session_id);
        self.stream_kind = stream_kind;
        self
    }

    pub fn is_bound(&self) -> bool {
        self.session_id.is_some()
    }

    // see: cans/11-protocol.md#Binary-exec-channel
    pub fn with_transport(mut self, transport: Arc<dyn SpawnTransport>) -> Self {
        self.transport = Some(transport);
        self
    }

    // see: cans/03-sdk-architecture.md#Params-typed-views
    pub fn params<T: Serialize>(mut self, params: T) -> Self {
        let mut buffer = Vec::new();
        match ciborium::into_writer(&params, &mut buffer) {
            Ok(()) => self.params_bytes = buffer,
            Err(error) => self.params_error = Some(format!("params encode failed: {error}")),
        }
        self
    }
}

// see: cans/11-protocol.md#Per-command-rlimits
fn encode_exec_control_limits(limits: Option<&ProcessLimits>) -> Vec<u8> {
    // TODO (L3) [protocol_mux]: serialize the rlimit descriptor onto EXEC control sub-lane 0x03.; slop: rlimit dropped, EXEC sub-lane 0x03 never carries it
    let _ = limits;
    Vec::new()
}

impl Future for SpawnBuilder {
    type Output = Result<SpawnResult, PluginError>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.session_id.is_none() {
            return Poll::Ready(Err(PluginError::Halt(
                "SpawnBuilder is unbound: spawn requires a booted session".into(),
            )));
        }
        if let Some(error) = &self.params_error {
            return Poll::Ready(Err(PluginError::Halt(error.clone())));
        }
        if self.limits.is_some() {
            return Poll::Ready(Err(PluginError::reject(
                Errno::Econfig,
                "per-command rlimits need the L3 EXEC control sub-lane 0x03 encoder",
            )));
        }
        let Some(transport) = &self.transport else {
            return Poll::Ready(Err(PluginError::Halt(
                "SpawnBuilder has no transport: spawn requires a booted session".into(),
            )));
        };
        let Some(session_id) = self.session_id else {
            return Poll::Ready(Err(PluginError::Halt(
                "SpawnBuilder is unbound: spawn requires a booted session".into(),
            )));
        };
        let request = SpawnRequest {
            session_id,
            stream_kind: self.stream_kind,
            args: self.args.clone(),
            cwd: self.cwd.clone(),
            limits_bytes: encode_exec_control_limits(self.limits.as_ref()),
            params_bytes: self.params_bytes.clone(),
        };
        match transport.dispatch_exec(&request) {
            Ok(()) => {
                // TODO (L3) [protocol_mux]: the SpawnResult (exit code, stdout, stderr) lands with live guest exec; dispatch accepted the request but no result channel exists yet.; slop: spawn Ok without a guest result
                Poll::Ready(Err(PluginError::Halt(
                    "TODO (L3): guest exec result".into(),
                )))
            }
            Err(error) => Poll::Ready(Err(error)),
        }
    }
}
