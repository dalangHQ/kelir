// see: cans/plugins/chaos.md#Service-exports-(chaos)
use std::path::Path;

use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{FsInterposer, HandlerCtx, NetInterposer};
use kelir_interface::protocol_wire::FilterAction;

use crate::fault_table::ChaosPlugin;
use crate::{FaultDirection, NetworkFaultRule, StorageFaultRule};

// TODO (L3) [plugin_engine]: remove this allow when the stubbed interposers land and call these rule helpers again.; slop: the allow hides dead rule helpers forever, the stubbed interposers never land, fault rules stay unwired while the gate reports clean.
#[allow(dead_code)]
impl ChaosPlugin {
    fn apply_network_rules(
        &self,
        direction: FaultDirection,
        frame: &mut [u8],
    ) -> Result<FilterAction, PluginError> {
        let _ = (direction, frame);
        // TODO (L3) [plugin_engine]: apply the matching network fault rule to the frame (RST injection, loss, latency).; slop: network fault rules never apply, RST injection and loss never fire, chaos drills pass vacuously.
        Ok(FilterAction::Forward)
    }

    fn apply_storage_rules(&self, path: &Path) -> Result<FilterAction, PluginError> {
        let _ = path;
        // TODO (L3) [plugin_engine]: apply the matching storage fault rule to the write path.; slop: storage fault rules never apply, writes always succeed, storage chaos drills pass vacuously.
        Ok(FilterAction::Forward)
    }

    fn frame_matches(&self, rule: &NetworkFaultRule, frame: &[u8]) -> Result<bool, PluginError> {
        // TODO (L3) [pe_16_chaos_rst_injection_drops_matching_frame]: parse L2/L4 headers and match the rule target domain or host pattern; slop: frame_matches stays Ok(false), RST rule never fires, drill passes vacuously
        let _ = (rule, frame);
        Ok(false)
    }

    fn loss_sampled(&self, rule: &NetworkFaultRule) -> Result<bool, PluginError> {
        // TODO (L3) [plugin_engine]: deterministic synthetic packet loss sampling against drop_rate; slop: loss never sampled, drop_rate ignored, all packets Forward
        let _ = rule;
        Ok(false)
    }

    fn path_matches(&self, pattern: &str, path: &Path) -> Result<bool, PluginError> {
        // TODO (L3) [plugin_engine]: glob match of the rule target_glob over the real FUSE write path argument; slop: glob never matched, wrong paths dropped, real write path ignored
        let _ = (pattern, path);
        Ok(false)
    }

    fn error_sampled(&self, rule: &StorageFaultRule) -> Result<bool, PluginError> {
        // TODO (L3) [plugin_engine]: deterministic error sampling against probability per write() call; slop: error never sampled, probability ignored, writes always succeed
        let _ = rule;
        Ok(false)
    }
}

// see: cans/plugins/chaos.md#Bidirectional-across-wire-plane
impl NetInterposer for ChaosPlugin {
    fn on_net_outbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        let _ = frame;
        // TODO (L3) [plugin_engine]: apply the outbound network fault rules to the frame.; slop: outbound network faults never apply, outbound frames always Forward, egress chaos drills pass vacuously.
        Ok(FilterAction::Forward)
    }

    fn on_net_inbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        let _ = frame;
        // TODO (L3) [plugin_engine]: apply the inbound network fault rules to the frame.; slop: inbound network faults never apply, inbound frames always Forward, ingress chaos drills pass vacuously.
        Ok(FilterAction::Forward)
    }
}

// see: cans/plugins/chaos.md#Storage-chaos
impl FsInterposer for ChaosPlugin {
    fn on_fs_write(
        &self,
        path: &Path,
        data: &mut [u8],
        pid: u32,
        ctx: &HandlerCtx,
    ) -> Result<FilterAction, PluginError> {
        let _ = (path, data, pid, ctx);
        // TODO (L3) [plugin_engine]: apply the storage fault rules to the write.; slop: storage fault rules never apply to the write, writes always succeed, storage chaos drills pass vacuously.
        Ok(FilterAction::Forward)
    }
}
