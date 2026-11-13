// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
// see: cans/09-snapshots.md#Groups-and-lineage
use kelir_interface::checkpoint::{ManifestGraph, PidLineage};
use kelir_interface::plugin_model::PluginError;

pub(crate) fn head(graph: &ManifestGraph, lineage: PidLineage) -> Option<u64> {
    graph.head(lineage)
}

pub(crate) fn lineage(graph: &ManifestGraph, node_id: u64) -> Result<Vec<u64>, PluginError> {
    if graph.get(node_id).is_none() {
        return Err(PluginError::Halt(
            "checkpoint node not in manifest graph".into(),
        ));
    }
    Ok(graph.lineage(node_id))
}
