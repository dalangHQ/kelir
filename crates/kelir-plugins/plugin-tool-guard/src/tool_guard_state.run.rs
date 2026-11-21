// see: cans/plugins/tool-guard.md#Plugin-tool-guard
use async_trait::async_trait;
use kelir_interface::plugin_model::{LifecycleCtx, LifecycleParticipant, PluginError};

use crate::tool_guard_state::ToolGuardPlugin;

// see: cans/plugins/tool-guard.md#Immutable-schema-locking
// TODO (L3) [governance_tools]: remove this allow when the stubbed on_vm_ready lands and routes this query again.; slop: the allow hides a dead query forever, on_vm_ready never routes it, the tool query stays unwired while the gate reports clean.
#[allow(dead_code)]
const TOOLS_LIST_QUERY: &[u8] = b"{\"jsonrpc\":\"2.0\",\"id\":0,\"method\":\"tools/list\"}";

// see: cans/plugins/tool-guard.md#Immutable-schema-locking
#[async_trait]
impl LifecycleParticipant for ToolGuardPlugin {
    async fn on_vm_ready(&self, _ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        // TODO (L3) [governance_tools]: boot-time tools/list fetch+lock halts here until route_request and decode_tools_list land; an empty-catalog boot would fake the spec-mandated lock; slop: empty-catalog boot fakes the lock, unpinned tools run
        // TODO (L3) [governance_tools]: fetch tools/list at boot and lock the tool catalog; stub skips the lock.; slop: the tool catalog is never locked, tools change under a running session, unapproved tools execute unchecked.
        Err(PluginError::Halt(
            "TODO (L3): fetch tools/list at boot and lock the tool catalog".into(),
        ))
    }
}
