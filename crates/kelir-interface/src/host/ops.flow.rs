// see: cans/08-lifecycle.md#Host-crash-reaper
use std::sync::Arc;

use async_trait::async_trait;

use crate::driver_interface::BalloonConvergence;
use crate::handles_lifecycle::VmControl;
use crate::plugin_model::PluginError;

use super::ops::{
    BalloonRequest, PressureEvent, PressureLevel, PressureSubscription, ReclaimRequest,
};

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
