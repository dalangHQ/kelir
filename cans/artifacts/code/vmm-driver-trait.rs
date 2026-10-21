// crates/kelir-core/src/vmm/driver.rs
//
// The universal hypervisor backend abstraction.
//
// Owned by: cans/01-architecture.md#VmmDriver-trait

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct VmConfig {
    pub vcpus: u8,
    pub memory_mb: u32,
    pub kernel_path: PathBuf,
    pub rootfs_erofs_path: PathBuf,
    pub tsc_locked: bool,
    pub entropy_seed: Option<u64>,
    pub enable_gpu_passthrough: bool,
    pub enable_watchdog: bool,
}

#[derive(Debug, Clone)]
pub struct SnapshotDescriptor {
    pub id: String,
    /// Byte count; see cans/01-architecture.md#VmmDriver-trait
    pub dirty_bytes: u64,
    pub created_at_ns: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum WatchdogAction {
    /// Guest kernel lockup detected → hard-reset the vCPUs and reboot the microVM
    Reboot,
    /// Unresponsive VM → flush ledger state, then terminate the instance
    Terminate,
}

#[async_trait]
pub trait VmmDriver: Send + Sync {
    /// Cold-boots the microVM environment in <60ms via the active platform driver
    async fn boot(&mut self, config: VmConfig) -> Result<(), VmmError>;

    /// Branches the running VM into an isolated CoW clone in <15ms
    /// - Linux: userfaultfd + KVM memory cloning
    /// - macOS: HVF memory-mapped page duplication
    async fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError>;

    /// Creates an atomic in-memory checkpoint of guest RAM and vCPU registers
    async fn checkpoint(&mut self, label: &str) -> Result<SnapshotDescriptor, VmmError>;

    /// Reverts guest memory and registers to a prior checkpoint in <8ms
    async fn rollback(&mut self, snapshot: &SnapshotDescriptor) -> Result<(), VmmError>;

    /// Flushes dirty RAM pages to disk and drops host memory consumption to 0 MB
    async fn suspend(&mut self, output_path: &Path) -> Result<(), VmmError>;

    /// Restores a suspended VM image from storage in <50ms
    async fn resume(&mut self, image_path: &Path) -> Result<(), VmmError>;

    /// Offsets the virtual hardware clock (KVM_SET_CLOCK / CNTVOFF_EL2)
    async fn skew_clock(&mut self, offset: Duration) -> Result<(), VmmError>;

    /// Adjusts guest RAM ceiling dynamically via virtio-balloon
    async fn adjust_balloon(&mut self, target_mb: u32) -> Result<(), VmmError>;

    /// Arms the hardware watchdog: when no heartbeat arrives within `timeout`,
    /// the driver executes `action` (Reboot / Terminate)
    async fn set_watchdog(&mut self, timeout: Duration, action: WatchdogAction) -> Result<(), VmmError>;

    /// Yields raw duplex channel to virtio-vsock / HV_SOCKET multiplexer
    fn vsock_connect(&self, port: u32) -> Result<tokio::net::UnixStream, VmmError>;

    /// Terminates hypervisor instance and releases memory allocations
    async fn terminate(&mut self) -> Result<(), VmmError>;
}
