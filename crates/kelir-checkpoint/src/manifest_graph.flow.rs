// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
// see: cans/09-snapshots.md#Groups-and-lineage
use kelir_interface::checkpoint::{ManifestGraph, PidLineage};
use kelir_interface::plugin_model::PluginError;

// TODO: [lifecycle_checkpoint]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it. (deferred to L3)
#[allow(dead_code)]
pub(crate) fn head(graph: &ManifestGraph, lineage: PidLineage) -> Option<u64> {
    graph.head(lineage)
}

// TODO: [lifecycle_checkpoint]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it. (deferred to L3)
#[allow(dead_code)]
pub(crate) fn lineage(graph: &ManifestGraph, node_id: u64) -> Result<Vec<u64>, PluginError> {
    if graph.get(node_id).is_none() {
        return Err(PluginError::Halt(
            "checkpoint node not in manifest graph".into(),
        ));
    }
    Ok(graph.lineage(node_id))
}
