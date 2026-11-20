// see: cans/plugins/l7-http.md#Service-exports-(l7-http)
use kelir_interface::plugin_model::{NetInterposer, PluginError};
use kelir_interface::protocol_wire::FilterAction;

use crate::service_l7_http::HttpPolicy;

// see: cans/plugins/l7-http.md#Service-exports-(l7-http)
impl NetInterposer for HttpPolicy {
    fn on_net_outbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        let _ = frame;
        // TODO (L3) [wire_plane]: write the 403 receipt from deny_receipt onto the diverted L4 stream; the wire plane exposes no stream write-back sink yet; slop: denied L7 requests pass without a 403 receipt; the guest sees a hung stream instead of a denial.
        Err(PluginError::Halt(
            "TODO (L3): write the 403 receipt from deny_receipt onto the diverted L4 stream".into(),
        ))
    }
}
