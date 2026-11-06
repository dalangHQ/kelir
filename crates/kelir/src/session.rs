use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use kelir_interface::driver::SnapshotDescriptor;
use kelir_interface::errors::PluginError;
use kelir_interface::telemetry::CausalDelta;

use crate::planes::storage::{FsReconcileRequest, ReconcileStats};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecResult {
    pub exit_code: i32,
}

#[derive(Debug, Clone, Default)]
pub struct ClockHandle;

impl ClockHandle {
    pub async fn skew(&self, _offset: std::time::Duration) -> Result<(), PluginError> {
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct VmHandle {
    pub clock: ClockHandle,
}

impl VmHandle {
    pub async fn checkpoint(
        &self,
        name: impl Into<String>,
    ) -> Result<SnapshotDescriptor, PluginError> {
        Ok(SnapshotDescriptor {
            id: name.into(),
            dirty_bytes: 0,
            created_at_ns: 0,
            parent_digest: None,
            guest_clock: None,
        })
    }

    pub async fn rollback(&self, _snap: &SnapshotDescriptor) -> Result<(), PluginError> {
        Ok(())
    }

    pub async fn fork(&self) -> Result<Session, PluginError> {
        Ok(Session::default())
    }

    pub async fn suspend(&self, _path: &str) -> Result<(), PluginError> {
        Ok(())
    }

    pub async fn map_shared_region(
        &self,
        _name: &str,
        bytes: usize,
    ) -> Result<Vec<u8>, PluginError> {
        Ok(vec![0; bytes])
    }
}

#[derive(Debug, Clone, Default)]
pub struct WireHandle;

impl WireHandle {
    pub async fn forward_port(&self, _guest: u16, _host: u16) -> Result<(), PluginError> {
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct StorageHandle;

impl StorageHandle {
    pub async fn reconcile(&self, _req: FsReconcileRequest) -> Result<ReconcileStats, PluginError> {
        Ok(ReconcileStats {
            files_changed: 0,
            tombstones_resolved: 0,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct VsockHandle;

impl VsockHandle {
    pub async fn channel(&self, _kind: u16) -> Result<VsockChannel, PluginError> {
        let (stream, _peer) = tokio::io::duplex(65536);
        Ok(VsockChannel { inner: stream })
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

#[derive(Debug, Clone, Default)]
pub struct FsHandle;

impl FsHandle {
    pub async fn warm_cache(&self, _paths: &[&str]) -> Result<(), PluginError> {
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct TerminalHandle;

impl TerminalHandle {
    pub fn notice(&self, _msg: &str) {}
}

#[derive(Debug, Clone, Default)]
pub struct StepStack {
    slots: Vec<SnapshotDescriptor>,
}

impl StepStack {
    pub fn last(&self) -> Option<SnapshotDescriptor> {
        self.slots.last().cloned()
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
        Poll::Ready(Ok(ExecResult { exit_code: 0 }))
    }
}

#[derive(Debug, Clone, Default)]
pub struct Session {
    pub vm: VmHandle,
    pub wire: WireHandle,
    pub storage: StorageHandle,
    pub vsock: VsockHandle,
}

impl Session {
    pub fn spawn(&self, _binary: &str) -> SpawnBuilder {
        SpawnBuilder::default()
    }

    pub async fn exec_in_guest(
        &self,
        _binary: &str,
        _args: &[&str],
    ) -> Result<ExecResult, PluginError> {
        Ok(ExecResult { exit_code: 0 })
    }

    pub async fn terminate(&self) -> Result<(), PluginError> {
        Ok(())
    }

    pub async fn suspend(&self, _path: &str) -> Result<(), PluginError> {
        Ok(())
    }

    pub async fn resume(&self, _path: &str) -> Result<(), PluginError> {
        Ok(())
    }

    pub fn driver_name(&self) -> &'static str {
        "stub"
    }

    pub fn boot_ms(&self) -> u64 {
        0
    }

    pub fn ctx<T: Default>(&self) -> T {
        T::default()
    }

    pub async fn console(&self) -> Result<(), PluginError> {
        Ok(())
    }

    pub async fn get_causal_delta_from_parent(&self) -> Result<CausalDelta, PluginError> {
        Err(PluginError::Halt(
            "stub: no causal delta from parent".into(),
        ))
    }
}
