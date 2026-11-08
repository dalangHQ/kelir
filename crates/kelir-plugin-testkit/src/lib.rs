// see: cans/02-plugin-model.md#Plugin-testkit
#![forbid(unsafe_code)]

pub mod assertions;
pub mod fixtures;
pub mod handlers;
pub mod mock_bus;
pub mod vectors;

use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::AsyncRead;

use kelir_interface::driver::{
    BalloonConvergence, DriverCapabilities, DriverError, EntropyBackend, EntropyStreamPosition,
    PmuSampler, SharedMemoryHandle, SnapshotDescriptor, Stream, SuspendReason, VmConfig, VmmDriver,
    WatchdogAction, check_hw_lockfile,
};

// see: cans/01-architecture.md#Transport-engine-abstraction
#[derive(Debug)]
pub struct LoopbackStream {
    channel_id: u32,
    buffer: VecDeque<u8>,
}

impl LoopbackStream {
    pub fn new(channel_id: u32) -> Self {
        Self {
            channel_id,
            buffer: VecDeque::new(),
        }
    }

    pub fn channel_id(&self) -> u32 {
        self.channel_id
    }
}

impl Stream for LoopbackStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let n = buf.len().min(self.buffer.len());
        let bytes: Vec<u8> = self.buffer.drain(..n).collect();
        buf[..n].copy_from_slice(&bytes);
        Ok(n)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.buffer.extend(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
}

pub struct MockVmmDriver;

#[async_trait]
impl VmmDriver for MockVmmDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities {
            backend: "mock",
            cold_boot_budget_ms: 60,
            rollback_budget_ms: 8,
            fork_budget_ms: 10,
            requires_hardware_vmm: false,
            supports_cow_fork: false,
        }
    }
    async fn boot(&mut self, config: VmConfig) -> Result<(), DriverError> {
        // see: cans/15-operations.md#Developer-CLI
        check_hw_lockfile(&config.driver_policy, &self.capabilities())?;
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
            section_handle: std::ptr::null_mut(),
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
    async fn freeze_vcpus(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
    async fn unfreeze_vcpus(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
    async fn drain_fuse(&mut self, _timeout: Duration) -> Result<(), DriverError> {
        Ok(())
    }
    async fn commit_broker_upper(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
    async fn discard_broker_upper(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
    async fn step_clock_window(&mut self, _retired_instructions: u64) -> Result<(), DriverError> {
        Ok(())
    }
    fn register_pmu_sampler(&mut self, _sampler: Arc<dyn PmuSampler>) -> Result<(), DriverError> {
        Ok(())
    }
    async fn set_entropy_backend(&mut self, _backend: EntropyBackend) -> Result<(), DriverError> {
        Ok(())
    }
    async fn entropy_position(&self) -> Result<EntropyStreamPosition, DriverError> {
        Ok(EntropyStreamPosition { offset_bytes: 0 })
    }
    async fn restore_entropy_position(
        &mut self,
        _position: EntropyStreamPosition,
    ) -> Result<(), DriverError> {
        Ok(())
    }
    async fn set_watchdog(
        &mut self,
        _timeout: Duration,
        _action: WatchdogAction,
    ) -> Result<(), DriverError> {
        Ok(())
    }
    fn transport_connect(&self, channel_id: u32) -> Result<Box<dyn Stream>, DriverError> {
        Ok(Box::new(LoopbackStream::new(channel_id)))
    }
    async fn terminate(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
}
