// see: cans/02-plugin-model.md#Plugin-testkit
#![forbid(unsafe_code)]

pub mod assertions;
pub mod fixtures;
pub mod handlers;
pub mod mock_bus;

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::AsyncRead;

use kelir_interface::driver::{
    BalloonConvergence, DriverError, DuplexChannel, SharedMemoryHandle, SnapshotDescriptor,
    SuspendReason, VmConfig, VmmDriver, WatchdogAction,
};

pub struct MockVmmDriver;

#[async_trait]
impl VmmDriver for MockVmmDriver {
    async fn boot(&mut self, _config: VmConfig) -> Result<(), DriverError> {
        Ok(())
    }
    async fn fork(&self) -> Result<Box<dyn VmmDriver>, DriverError> {
        Ok(Box::new(MockVmmDriver))
    }
    async fn checkpoint(&mut self, _label: &str) -> Result<SnapshotDescriptor, DriverError> {
        Ok(SnapshotDescriptor {
            id: String::new(),
            dirty_bytes: 0,
            created_at_ns: 0,
            parent_digest: None,
            guest_clock: None,
        })
    }
    async fn map_shared_region(
        &mut self,
        tag: &str,
        size_bytes: usize,
    ) -> Result<SharedMemoryHandle, DriverError> {
        Ok(SharedMemoryHandle {
            tag: tag.to_string(),
            size_bytes,
            guest_phys_addr: 0,
            host_ptr: std::ptr::null_mut(),
            #[cfg(unix)]
            memfd_raw: -1,
            #[cfg(windows)]
            handle_raw: std::ptr::null_mut(),
        })
    }
    async fn export_snapshot(
        &self,
    ) -> Result<Box<dyn AsyncRead + Send + Sync + Unpin>, DriverError> {
        Ok(Box::new(tokio::io::empty()))
    }
    async fn import_snapshot(
        &mut self,
        _reader: Box<dyn AsyncRead + Send + Sync + Unpin>,
    ) -> Result<(), DriverError> {
        Ok(())
    }
    async fn rollback(&mut self, _snapshot: &SnapshotDescriptor) -> Result<(), DriverError> {
        Ok(())
    }
    async fn suspend(
        &mut self,
        _reason: SuspendReason,
        _output_path: &Path,
    ) -> Result<(), DriverError> {
        Ok(())
    }
    async fn resume(&mut self, _image_path: &Path) -> Result<(), DriverError> {
        Ok(())
    }
    async fn skew_clock(&mut self, _offset: Duration) -> Result<(), DriverError> {
        Ok(())
    }
    async fn adjust_balloon(&mut self, target_mb: u32) -> Result<BalloonConvergence, DriverError> {
        Ok(BalloonConvergence {
            target_mb,
            converged: true,
        })
    }
    async fn set_watchdog(
        &mut self,
        _timeout: Duration,
        _action: WatchdogAction,
    ) -> Result<(), DriverError> {
        Ok(())
    }
    fn transport_connect(&self, _channel_id: u32) -> Result<Box<dyn DuplexChannel>, DriverError> {
        Err(DriverError::Unsupported(
            "mock transport_connect (L3)".into(),
        ))
    }
    async fn terminate(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
}
