// see: cans/02-plugin-model.md#Core-context-keys
#![forbid(unsafe_code)]

use crate::driver::SnapshotDescriptor;
use crate::plugin::PluginError;

#[derive(Debug, Clone, Default)]
pub struct ClockHandle;

impl ClockHandle {
    pub async fn skew(&self, _offset: std::time::Duration) -> Result<(), PluginError> {
        Err(PluginError::Halt("TODO (L3): ClockHandle::skew".into()))
    }
}

#[derive(Debug, Clone, Default)]
pub struct VmHandle {
    pub clock: ClockHandle,
}

impl VmHandle {
    pub async fn checkpoint(
        &self,
        _name: impl Into<String>,
    ) -> Result<SnapshotDescriptor, PluginError> {
        Err(PluginError::Halt("TODO (L3): VmHandle::checkpoint".into()))
    }

    pub async fn rollback(&self, _snap: &SnapshotDescriptor) -> Result<(), PluginError> {
        Err(PluginError::Halt("TODO (L3): VmHandle::rollback".into()))
    }

    pub async fn suspend(&self, _path: &str) -> Result<(), PluginError> {
        Err(PluginError::Halt("TODO (L3): VmHandle::suspend".into()))
    }

    pub async fn resume(&self, _path: &str) -> Result<(), PluginError> {
        Err(PluginError::Halt("TODO (L3): VmHandle::resume".into()))
    }

    pub async fn export_snapshot(&self) -> Result<Vec<u8>, PluginError> {
        Err(PluginError::Halt(
            "TODO (L3): VmHandle::export_snapshot".into(),
        ))
    }

    pub async fn import_snapshot(&self, _data: &[u8]) -> Result<(), PluginError> {
        Err(PluginError::Halt(
            "TODO (L3): VmHandle::import_snapshot".into(),
        ))
    }

    pub async fn adjust_balloon(&self, _target_mb: u32) -> Result<(), PluginError> {
        Err(PluginError::Halt(
            "TODO (L3): VmHandle::adjust_balloon".into(),
        ))
    }

    pub async fn set_vcpus(&self, _count: u8) -> Result<(), PluginError> {
        Err(PluginError::Halt("TODO (L3): VmHandle::set_vcpus".into()))
    }

    pub async fn map_shared_region(
        &self,
        _name: &str,
        _bytes: usize,
    ) -> Result<Vec<u8>, PluginError> {
        Err(PluginError::Halt(
            "TODO (L3): VmHandle::map_shared_region".into(),
        ))
    }
}

#[derive(Debug, Clone, Default)]
pub struct FsHandle;

impl FsHandle {
    pub async fn warm_cache(&self, _paths: &[&str]) -> Result<(), PluginError> {
        Err(PluginError::Halt("TODO (L3): FsHandle::warm_cache".into()))
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
