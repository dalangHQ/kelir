// see: cans/07-wire-plane.md#Wire-architecture
use crate::plugin_model::PluginError;
use crate::protocol_wire::StreamKind;

pub trait L4DivertOps: Send + Sync {
    fn divert_l4(&self, guest_port: u16, host_target: String) -> Result<L4Stream, PluginError>;
}

#[derive(Debug, Clone)]
pub struct L4Stream {
    pub kind: StreamKind,
    pub guest_port: u16,
    pub host_target: String,
}

impl L4Stream {
    pub fn new(guest_port: u16, host_target: String) -> Self {
        Self {
            kind: StreamKind::L4Forward,
            guest_port,
            host_target,
        }
    }
}
