// see: cans/01-architecture.md#Host-plane-subsystem
// see: cans/01-architecture.md#Host-memory-pressure-watcher
// see: cans/08-lifecycle.md#Host-crash-reaper
use std::sync::Arc;

use async_trait::async_trait;

use crate::driver_interface::BalloonConvergence;
use crate::handles_lifecycle::VmControl;
use crate::plugin_model::PluginError;

// see: cans/08-lifecycle.md#Host-crash-reaper
#[async_trait]
pub trait HostOps: Send + Sync {
    fn refresh_rss(&self) -> Option<u64>;
    fn lsm_status(&self) -> bool;
    fn pressure_level(&self) -> u8;
    fn pressure_level_typed(&self) -> PressureLevel;
    fn subscribe_pressure(&self) -> Result<PressureSubscription, PluginError>;
    fn poll_pressure_events(&self) -> Result<Vec<PressureEvent>, PluginError>;
    async fn inflate_balloon(
        &self,
        request: BalloonRequest,
    ) -> Result<BalloonConvergence, PluginError>;
    fn reclaim(&self, request: ReclaimRequest) -> Result<u64, PluginError>;
    fn arm_host_reaper(&self, parent_pid: u32) -> Result<(), PluginError>;
    fn set_vm_control(&self, control: Arc<dyn VmControl>);
    fn fresh(&self) -> Arc<dyn HostOps>;
}

pub enum HostReaperKind {
    LinuxPdeathsig,
    DarwinKqueue,
    WindowsJobObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureSource {
    LinuxCgroupV2MemoryPressure,
    MacosDispatchMemoryPressure,
    WindowsJobObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureLevel {
    None,
    Low,
    Medium,
    High,
    Critical,
}

impl PressureLevel {
    pub fn from_u8(level: u8) -> Self {
        match level {
            0 => Self::None,
            1 => Self::Low,
            2 => Self::Medium,
            3 => Self::High,
            _ => Self::Critical,
        }
    }

    pub fn as_u8(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 3,
            Self::Critical => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PressureEvent {
    pub level: PressureLevel,
    pub at_ns: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PressureSubscription {
    pub source: PressureSource,
    pub armed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BalloonRequest {
    pub target_mb: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReclaimRequest {
    pub bytes: u64,
}
