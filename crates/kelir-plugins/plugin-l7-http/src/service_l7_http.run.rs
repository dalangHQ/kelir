// see: cans/plugins/l7-http.md#Service-exports-(l7-http)
use kelir_interface::plugin_model::{NetInterposer, PluginError};
use kelir_interface::protocol_wire::FilterAction;

use crate::service_l7_http::HttpPolicy;

// see: cans/plugins/l7-http.md#Service-exports-(l7-http)
impl NetInterposer for HttpPolicy {
    fn on_net_outbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        let _ = frame;
        // TODO (L3) [wire_plane]: write the 403 receipt from deny_receipt onto the diverted L4 stream; the wire plane exposes no stream write-back sink yet; slop: denied L7 requests pass without a 403 receipt; the guest sees a hung stream instead of a denial.
        // TODO (L3) [wire_plane]: bind the declared-services L4 direct-egress block so a declared upstream host is reachable only through this proxy, never by raw guest IP/domain dialing; slop: declared services bypass via direct IP dialing, the gateway gate is decorative.
        // TODO (L3) [wire_plane]: run DLP prompt redaction over outbound LLM bodies of declared services before upstream dial; slop: PII leaves the host unredacted inside prompts, DLP config stays dead.
        // TODO (L3) [wire_plane]: sync URLhaus/phishing/Onionoo threat feeds with atomic swap on 304 check and enforce them at L7/L4; slop: threat feed lists stay empty, known-malware and phishing hosts dial out freely.
        Err(PluginError::Halt(
            "TODO (L3): write the 403 receipt from deny_receipt onto the diverted L4 stream".into(),
        ))
    }
}
