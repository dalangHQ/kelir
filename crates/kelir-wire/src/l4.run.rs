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
    fn divert_l4(&self, _guest_port: u16, _host_target: String) -> Result<L4Stream, PluginError> {
        // L3-GAP: [wp_12_l4_diversion_passthrough_tls]: divert the guest TCP stream over stream kind 0x0005 to the host target; slop: TLS terminated in supervisor, plaintext in memory
        // TODO (L3) [wire_plane]: divert the guest TCP stream over stream kind 0x0005 to the host target.; slop: diversion never happens, guest TCP bypasses the L4 target and connects direct.
        Err(PluginError::Halt(
            "TODO (L3): L4DivertOps::divert_l4 needs the L4 stream diversion over 0x0005".into(),
        ))
    }
}
