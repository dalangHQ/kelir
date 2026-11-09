// see: cans/02-plugin-model.md#Core-context-keys
#![forbid(unsafe_code)]

use std::sync::{Arc, RwLock};
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex as AsyncMutex;

use crate::driver::{
    BalloonConvergence, SharedMemoryHandle, SnapshotDescriptor, SuspendReason, VmmDriver, VmmError,
};
use crate::plugin::PluginError;

// see: cans/08-lifecycle.md#Hypervisor-primitives
#[async_trait]
pub trait VmControl: Send + Sync {
    async fn checkpoint(&self, name: String) -> Result<SnapshotDescriptor, PluginError>;
    async fn rollback(&self, snapshot: &SnapshotDescriptor) -> Result<(), PluginError>;
    async fn suspend(&self, reason: SuspendReason, path: &str) -> Result<(), PluginError>;
    async fn resume(&self, path: &str) -> Result<(), PluginError>;
    async fn export_snapshot(&self) -> Result<Vec<u8>, PluginError>;
    async fn import_snapshot(&self, data: Vec<u8>) -> Result<(), PluginError>;
    // see: cans/01-architecture.md#Host-memory-pressure-watcher
    async fn adjust_balloon(&self, target_mb: u32) -> Result<BalloonConvergence, PluginError>;
    async fn set_vcpus(&self, count: u8) -> Result<(), PluginError>;
    async fn map_shared_region(
        &self,
        name: &str,
        bytes: usize,
    ) -> Result<SharedMemoryHandle, PluginError>;
    async fn skew_clock(&self, offset: Duration) -> Result<(), PluginError>;
}

type VmControlSlot = Arc<RwLock<Option<Arc<dyn VmControl>>>>;

fn unbound_slot() -> VmControlSlot {
    Arc::new(RwLock::new(None))
}

fn read_slot(slot: &VmControlSlot) -> Result<Option<Arc<dyn VmControl>>, PluginError> {
    match slot.read() {
        Ok(guard) => Ok(guard.clone()),
        Err(_) => Err(PluginError::Halt("vm control slot poisoned".into())),
    }
}

fn bind_slot(slot: &VmControlSlot, control: Arc<dyn VmControl>) -> Result<(), PluginError> {
    match slot.write() {
        Ok(mut guard) => {
            *guard = Some(control);
            Ok(())
        }
        Err(_) => Err(PluginError::Halt("vm control slot poisoned".into())),
    }
}

fn slot_bound(slot: &VmControlSlot) -> bool {
    matches!(slot.read(), Ok(guard) if guard.is_some())
}

fn unbound_error(op: &str) -> PluginError {
    PluginError::Halt(format!(
        "VmHandle unbound: no VmControl bound; {op} needs Session::boot to bind the driver control"
    ))
}

// see: cans/08-lifecycle.md#Hypervisor-primitives
pub struct DriverVmControl {
    driver: Arc<AsyncMutex<Box<dyn VmmDriver + Send + Sync>>>,
}

impl DriverVmControl {
    pub fn new(driver: Box<dyn VmmDriver + Send + Sync>) -> Self {
        Self::from_shared(Arc::new(AsyncMutex::new(driver)))
    }

    pub fn from_shared(driver: Arc<AsyncMutex<Box<dyn VmmDriver + Send + Sync>>>) -> Self {
        Self { driver }
    }

    pub fn shared(driver: &Arc<AsyncMutex<Box<dyn VmmDriver + Send + Sync>>>) -> Arc<Self> {
        Arc::new(Self::from_shared(Arc::clone(driver)))
    }
}

fn driver_error(op: &str, error: VmmError) -> PluginError {
    PluginError::Halt(format!("driver {op}: {error}"))
}

#[async_trait]
impl VmControl for DriverVmControl {
    async fn checkpoint(&self, name: String) -> Result<SnapshotDescriptor, PluginError> {
        self.driver
            .lock()
            .await
            .checkpoint(&name)
            .await
            .map_err(|error| driver_error("checkpoint", error))
    }

    async fn rollback(&self, snapshot: &SnapshotDescriptor) -> Result<(), PluginError> {
        self.driver
            .lock()
            .await
            .rollback(snapshot)
            .await
            .map_err(|error| driver_error("rollback", error))
    }

    async fn suspend(&self, reason: SuspendReason, path: &str) -> Result<(), PluginError> {
        self.driver
            .lock()
            .await
            .suspend(reason, std::path::Path::new(path))
            .await
            .map_err(|error| driver_error("suspend", error))
    }

    async fn resume(&self, path: &str) -> Result<(), PluginError> {
        self.driver
            .lock()
            .await
            .resume(std::path::Path::new(path))
            .await
            .map_err(|error| driver_error("resume", error))
    }

    async fn export_snapshot(&self) -> Result<Vec<u8>, PluginError> {
        let mut reader = self
            .driver
            .lock()
            .await
            .export_snapshot()
            .await
            .map_err(|error| driver_error("export_snapshot", error))?;
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .await
            .map_err(|error| PluginError::Halt(format!("driver export_snapshot read: {error}")))?;
        Ok(bytes)
    }

    async fn import_snapshot(&self, data: Vec<u8>) -> Result<(), PluginError> {
        let reader = Box::new(std::io::Cursor::new(data));
        self.driver
            .lock()
            .await
            .import_snapshot(reader)
            .await
            .map_err(|error| driver_error("import_snapshot", error))
    }

    async fn adjust_balloon(&self, target_mb: u32) -> Result<BalloonConvergence, PluginError> {
        self.driver
            .lock()
            .await
            .adjust_balloon(target_mb)
            .await
            .map_err(|error| driver_error("adjust_balloon", error))
    }

    async fn set_vcpus(&self, _count: u8) -> Result<(), PluginError> {
        // TODO (L3): VmmDriver has no set_vcpus surface; the adapter cannot park/unpark host vCPU threads.
        Err(PluginError::Halt(
            "VmmDriver has no set_vcpus surface".into(),
        ))
    }

    async fn map_shared_region(
        &self,
        name: &str,
        bytes: usize,
    ) -> Result<SharedMemoryHandle, PluginError> {
        self.driver
            .lock()
            .await
            .map_shared_region(name, bytes)
            .await
            .map_err(|error| driver_error("map_shared_region", error))
    }

    async fn skew_clock(&self, offset: Duration) -> Result<(), PluginError> {
        self.driver
            .lock()
            .await
            .skew_clock(offset)
            .await
            .map_err(|error| driver_error("skew_clock", error))
    }
}

#[derive(Clone)]
pub struct ClockHandle {
    control: VmControlSlot,
}

impl Default for ClockHandle {
    fn default() -> Self {
        Self {
            control: unbound_slot(),
        }
    }
}

impl std::fmt::Debug for ClockHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClockHandle")
            .field("bound", &slot_bound(&self.control))
            .finish()
    }
}

impl ClockHandle {
    pub fn new(control: Arc<dyn VmControl>) -> Self {
        let handle = Self::default();
        let _ = handle.bind_control(control);
        handle
    }

    pub fn with_control(control: Arc<dyn VmControl>) -> Self {
        Self::new(control)
    }

    pub fn bind_control(&self, control: Arc<dyn VmControl>) -> Result<(), PluginError> {
        bind_slot(&self.control, control)
    }

    pub fn is_bound(&self) -> bool {
        slot_bound(&self.control)
    }

    pub async fn skew(&self, offset: Duration) -> Result<(), PluginError> {
        match read_slot(&self.control)? {
            Some(control) => control.skew_clock(offset).await,
            None => Err(unbound_error("ClockHandle::skew")),
        }
    }
}

#[derive(Clone)]
pub struct VmHandle {
    pub clock: ClockHandle,
    control: VmControlSlot,
}

impl Default for VmHandle {
    fn default() -> Self {
        let control = unbound_slot();
        Self {
            clock: ClockHandle {
                control: Arc::clone(&control),
            },
            control,
        }
    }
}

impl std::fmt::Debug for VmHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VmHandle")
            .field("bound", &slot_bound(&self.control))
            .finish()
    }
}

impl VmHandle {
    pub fn new(control: Arc<dyn VmControl>) -> Self {
        let handle = Self::default();
        let _ = handle.bind_control(control);
        handle
    }

    pub fn with_control(control: Arc<dyn VmControl>) -> Self {
        Self::new(control)
    }

    pub fn bind_control(&self, control: Arc<dyn VmControl>) -> Result<(), PluginError> {
        bind_slot(&self.control, control)
    }

    pub fn is_bound(&self) -> bool {
        slot_bound(&self.control)
    }

    pub fn control(&self) -> Result<Option<Arc<dyn VmControl>>, PluginError> {
        read_slot(&self.control)
    }

    fn bound(&self, op: &str) -> Result<Arc<dyn VmControl>, PluginError> {
        match read_slot(&self.control)? {
            Some(control) => Ok(control),
            None => Err(unbound_error(op)),
        }
    }

    pub async fn checkpoint(
        &self,
        name: impl Into<String>,
    ) -> Result<SnapshotDescriptor, PluginError> {
        self.bound("VmHandle::checkpoint")?
            .checkpoint(name.into())
            .await
    }

    pub async fn rollback(&self, snap: &SnapshotDescriptor) -> Result<(), PluginError> {
        self.bound("VmHandle::rollback")?.rollback(snap).await
    }

    pub async fn suspend(&self, reason: SuspendReason, path: &str) -> Result<(), PluginError> {
        self.bound("VmHandle::suspend")?.suspend(reason, path).await
    }

    pub async fn resume(&self, path: &str) -> Result<(), PluginError> {
        self.bound("VmHandle::resume")?.resume(path).await
    }

    pub async fn export_snapshot(&self) -> Result<Vec<u8>, PluginError> {
        self.bound("VmHandle::export_snapshot")?
            .export_snapshot()
            .await
    }

    pub async fn import_snapshot(&self, data: &[u8]) -> Result<(), PluginError> {
        self.bound("VmHandle::import_snapshot")?
            .import_snapshot(data.to_vec())
            .await
    }

    // see: cans/01-architecture.md#Host-memory-pressure-watcher
    pub async fn adjust_balloon(&self, target_mb: u32) -> Result<BalloonConvergence, PluginError> {
        self.bound("VmHandle::adjust_balloon")?
            .adjust_balloon(target_mb)
            .await
    }

    pub async fn set_vcpus(&self, count: u8) -> Result<(), PluginError> {
        self.bound("VmHandle::set_vcpus")?.set_vcpus(count).await
    }

    pub async fn map_shared_region(
        &self,
        name: &str,
        bytes: usize,
    ) -> Result<SharedMemoryHandle, PluginError> {
        self.bound("VmHandle::map_shared_region")?
            .map_shared_region(name, bytes)
            .await
    }
}

#[derive(Debug, Clone, Default)]
pub struct FsHandle;

impl FsHandle {
    // TODO (L3): prefetch the storage catalog entries for the requested paths into the warm cache.
    pub async fn warm_cache(&self, _paths: &[&str]) -> Result<(), PluginError> {
        Err(PluginError::Halt("TODO (L3): FsHandle::warm_cache".into()))
    }
}

#[derive(Debug, Clone, Default)]
pub struct TerminalHandle;

impl TerminalHandle {
    // TODO (L3): publish the TerminalNotice event to the event bus per cans/04-terminal-plane.md#Terminal-pipeline.
    pub fn notice(&self, _msg: &str) {}
}

// TODO (L3): push SnapshotDescriptor slots on checkpoint create/restore; nothing populates them at L2, so last() always returns None.
#[derive(Debug, Clone, Default)]
pub struct StepStack {
    slots: Vec<SnapshotDescriptor>,
}

impl StepStack {
    pub fn last(&self) -> Option<SnapshotDescriptor> {
        self.slots.last().cloned()
    }
}
