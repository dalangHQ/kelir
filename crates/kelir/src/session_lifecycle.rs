use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

pub use kelir_interface::handles_lifecycle::{
    ClockHandle, FsHandle, StepStack, TerminalHandle, VmHandle,
};
use kelir_interface::hex_codec::encode_hex;
use kelir_interface::plugin_model::{MountScope, PipelineBinder, PluginError};

use kelir_interface::handlers_plugin::Has;

use crate::builder_sdk::{Registration, TrapOutcome};
use crate::hlist_sdk::{HList, HNil, NoParams};
use crate::planes::storage_plane::FsReconcileRequest;
use kelir_core::planes::storage::storage_reconciler::CommitStats;
use kelir_interface::plugin_model::HandlerCtx;
use kelir_interface::protocol_wire::{StreamKind, TrapEvent, Verdict};

// see: cans/03-sdk-architecture.md#Rust-SDK
pub struct Session<Plugins = HNil, Params = NoParams>
where
    Plugins: HList,
{
    core: kelir_core::Session,
    plugins: Plugins,
    registration: Registration<Plugins>,
    _params: PhantomData<fn() -> Params>,
}

impl<Plugins: HList, Params> Session<Plugins, Params> {
    pub(crate) fn from_parts(
        core: kelir_core::Session,
        plugins: Plugins,
        registration: Registration<Plugins>,
    ) -> Self {
        Self {
            core,
            plugins,
            registration,
            _params: PhantomData,
        }
    }

    // see: cans/03-sdk-architecture.md#Service-lookup-across-languages
    pub fn ctx<T, Index>(&self) -> &T
    where
        Plugins: Has<T, Index>,
    {
        self.plugins.get()
    }

    pub fn core(&self) -> &kelir_core::Session {
        &self.core
    }

    // see: cans/03-sdk-architecture.md#Session-surfaces
    pub fn vm(&self) -> VmHandle {
        self.core.vm()
    }

    // see: cans/03-sdk-architecture.md#Idiomatic-Surfaces-Law
    pub async fn spawn(&self, binary: &str, argv: &[&str]) -> Result<SpawnResult, PluginError> {
        let mut builder = self.spawn_builder(binary);
        builder.args.extend(argv.iter().map(|arg| arg.to_string()));
        builder.await
    }

    // see: cans/03-sdk-architecture.md#Idiomatic-Surfaces-Law
    pub fn spawn_builder(&self, binary: &str) -> SpawnBuilder {
        SpawnBuilder::default()
            .args([binary])
            .bind(self.core.session_id(), StreamKind::Exec)
    }

    // see: cans/02-plugin-model.md#Static-dispatch
    pub async fn dispatch_trap<Params2>(
        &self,
        event: &TrapEvent,
        ctx: &mut HandlerCtx<Plugins, Params2>,
    ) -> Result<TrapOutcome, PluginError>
    where
        Plugins: Clone,
        Params2: Clone,
    {
        let lineage = self.core.lineage(ctx.pid);
        self.core.pipeline().hydrate_step(&mut *ctx, lineage)?;
        for entry in &self.registration.traps {
            if !entry.filter.matches(event) {
                continue;
            }
            let outcome = (entry.handler)(ctx.clone().retag::<NoParams>())
                .await
                .map_err(|e| PluginError::Halt(format!("trap handler failed: {e}")))?;
            match outcome {
                TrapOutcome::Verdict(Verdict::Pass) => continue,
                TrapOutcome::PostExec(crate::builder_sdk::PostExecAction::Proceed) => continue,
                outcome => return Ok(outcome),
            }
        }
        let verdict = self
            .core
            .pipeline()
            .dispatch_trap_typed_with_session(event, ctx, &self.core)
            .await?;
        Ok(TrapOutcome::Verdict(verdict))
    }

    // see: cans/08-lifecycle.md#State-machine
    pub async fn shutdown(&mut self) -> Result<(), PluginError> {
        self.core.shutdown().await
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
            // see: cans/08-lifecycle.md#CoW-fork
            traps: Vec::new(),
            storage_cfg: self.registration.storage_cfg.clone(),
            terminal_cfg: self.registration.terminal_cfg.clone(),
            wire_cfg: self.registration.wire_cfg.clone(),
            lock_broker_cfg: self.registration.lock_broker_cfg.clone(),
            on_trap_deny: None,
            custom_actions: Vec::new(),
            stream_kind_services: Vec::new(),
            vetoes: Vec::new(),
            detached: self.registration.detached,
        };
        for (plugin, scope) in &self.registration.instances {
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
        Ok(Self {
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
        let core = kelir_core::Session::attach(session_id)?;
        Ok(Self::from_parts(core, HNil, Registration::default()))
    }
}

impl<Plugins: HList, Params> std::ops::Deref for Session<Plugins, Params> {
    type Target = kelir_core::Session;

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
pub struct WireHandle;

impl WireHandle {
    // TODO (L3): divert the guest TCP stream to the host socket per cans/07-wire-plane.md#Transparent-L4-stream-diversion.
    pub async fn forward_port(&self, _guest: u16, _host: u16) -> Result<(), PluginError> {
        Err(PluginError::Halt(
            "TODO (L3): WireHandle::forward_port".into(),
        ))
    }
}

#[derive(Debug, Clone, Default)]
pub struct StorageHandle;

impl StorageHandle {
    // TODO (L3): replay the upper file and tombstones through the atomic commit reconciler per cans/05-storage-plane.md#Workspace-commit.
    pub async fn reconcile(&self, _req: FsReconcileRequest) -> Result<CommitStats, PluginError> {
        Err(PluginError::Halt(
            "TODO (L3): StorageHandle::reconcile".into(),
        ))
    }
}

#[derive(Debug, Clone, Default)]
pub struct VsockHandle;

impl VsockHandle {
    // TODO (L3): open the multiplexed vsock channel for the stream kind per cans/03-sdk-architecture.md#Session-surfaces.
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
#[derive(Debug, Default)]
pub struct SpawnBuilder {
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub session_id: Option<[u8; 16]>,
    pub stream_kind: StreamKind,
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

    // TODO (L3): serialize the params value into the request buffer so HandlerCtx::params::<T>() decodes it per cans/03-sdk-architecture.md#Params-typed-views.
    pub fn params<T>(self, _params: T) -> Self {
        self
    }
}

impl Future for SpawnBuilder {
    type Output = Result<SpawnResult, PluginError>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.session_id.is_none() {
            return Poll::Ready(Err(PluginError::Halt(
                "SpawnBuilder is unbound: spawn requires a booted session".into(),
            )));
        }
        // TODO (L3): encode the EXEC frame on the bound stream kind and dispatch it over the session transport.
        Poll::Ready(Err(PluginError::Halt("TODO (L3): guest exec".into())))
    }
}
