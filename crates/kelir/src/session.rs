use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

pub use kelir_interface::handles::{ClockHandle, FsHandle, StepStack, TerminalHandle, VmHandle};
use kelir_interface::plugin::PluginError;

use kelir_interface::handlers::Has;

use crate::builder::Registration;
use crate::hlist::{HList, HNil, NoParams};
use crate::planes::storage::FsReconcileRequest;
use kelir_core::planes::storage::reconciler::CommitStats;

// see: cans/03-sdk-architecture.md#Rust-SDK
pub struct Session<Plugins = HNil, Params = NoParams>
where
    Plugins: HList,
{
    core: kelir_core::Session,
    plugins: Plugins,
    // TODO (L3): engine dispatch consumes the mounted registration per cans/02-plugin-model.md#Static-dispatch.
    #[allow(dead_code)]
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

    // see: cans/03-sdk-architecture.md#Rust-SDK
    pub fn fork(&self) -> Result<Session<Plugins, Params>, PluginError> {
        // TODO (L3): fork the engine session, clone the capability list, and rebuild the child registration via fork_state.
        let child = self.core.fork()?;
        Ok(Self {
            core: child,
            plugins: self.plugins.clone(),
            registration: Registration::default(),
            _params: PhantomData,
        })
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecResult {
    pub exit_code: i32,
}

#[derive(Debug, Clone, Default)]
pub struct WireHandle;

impl WireHandle {
    pub async fn forward_port(&self, _guest: u16, _host: u16) -> Result<(), PluginError> {
        Err(PluginError::Halt(
            "TODO (L3): WireHandle::forward_port".into(),
        ))
    }
}

#[derive(Debug, Clone, Default)]
pub struct StorageHandle;

impl StorageHandle {
    pub async fn reconcile(&self, _req: FsReconcileRequest) -> Result<CommitStats, PluginError> {
        Err(PluginError::Halt(
            "TODO (L3): StorageHandle::reconcile".into(),
        ))
    }
}

#[derive(Debug, Clone, Default)]
pub struct VsockHandle;

impl VsockHandle {
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

#[derive(Debug, Default)]
pub struct SpawnBuilder {
    pub args: Vec<String>,
    pub cwd: Option<String>,
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

    pub fn params<T>(self, _params: T) -> Self {
        self
    }
}

impl Future for SpawnBuilder {
    type Output = Result<ExecResult, PluginError>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(Err(PluginError::Halt("TODO (L3): guest exec".into())))
    }
}
