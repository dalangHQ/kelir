// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#UDP-socket-reaper
use crate::plugin_model::PluginError;

pub trait UdpNatOps: Send + Sync {
    fn udp_nat_alloc(&self, guest_source_port: u16, now_unix: u64) -> Result<(), PluginError>;
    fn udp_nat_slab_used(&self) -> Result<usize, PluginError>;
    fn reap_stale_udp_sockets(&self, now_unix: u64) -> Result<usize, PluginError>;
    fn quiesce(&self) -> Result<(), PluginError>;
}

pub struct UdpNatEntry {
    guest_source_port: u16,
    last_active_unix: u64,
}

impl UdpNatEntry {
    pub fn new(guest_source_port: u16, now_unix: u64) -> Self {
        Self {
            guest_source_port,
            last_active_unix: now_unix,
        }
    }

    pub fn guest_source_port(&self) -> u16 {
        self.guest_source_port
    }

    pub fn last_active_unix(&self) -> u64 {
        self.last_active_unix
    }

    pub fn touch(&mut self, now_unix: u64) {
        self.last_active_unix = now_unix;
    }
}

// see: cans/07-wire-plane.md#UDP-socket-reaper
pub const UDP_NAT_SLAB_CAPACITY: usize = 1_024;

// see: cans/07-wire-plane.md#UDP-socket-reaper
pub const UDP_NAT_IDLE_TIMEOUT_SECS: u64 = 30;
