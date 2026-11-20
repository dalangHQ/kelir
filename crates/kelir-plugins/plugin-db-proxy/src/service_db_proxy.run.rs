// see: cans/plugins/db-proxy.md#Plugin-db-proxy
use kelir_interface::plugin_model::{NetInterposer, PluginError};
use kelir_interface::protocol_wire::FilterAction;

use crate::DbProxyPlugin;
use crate::reassembler_wire::NetDirection;

// see: cans/plugins/db-proxy.md#L7-pgwire-rewrite-and-2PC-commit
impl NetInterposer for DbProxyPlugin {
    fn on_net_outbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        self.intercept_ethernet_frame(NetDirection::GuestToUpstream, frame)
    }

    fn on_net_inbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        self.intercept_ethernet_frame(NetDirection::UpstreamToGuest, frame)
    }
}
