// see: cans/07-wire-plane.md#Wire-architecture
use std::sync::Arc;

use kelir_interface::plugin_model::PluginError;
use kelir_interface::wire::l4::{L4DivertOps, L4Stream};

pub(crate) struct RealL4DivertOps;

impl RealL4DivertOps {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for RealL4DivertOps {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn build_l4_ops() -> Arc<dyn L4DivertOps> {
    Arc::new(RealL4DivertOps::new())
}

impl L4DivertOps for RealL4DivertOps {
    fn divert_l4(&self, guest_port: u16, host_target: String) -> Result<L4Stream, PluginError> {
        // TODO (L3) [wp_12_l4_diversion_passthrough_tls]: divert the guest TCP stream over stream kind 0x0005 to the host target; slop: TLS terminated in supervisor, plaintext in memory
        let _ = L4Stream::new(guest_port, host_target);
        Err(PluginError::Halt(
            "L4 stream diversion over 0x0005 pending (L3)".into(),
        ))
    }
}
