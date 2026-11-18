// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use kelir_interface::checkpoint::{CheckpointNode, CheckpointOps, ManifestGraph, PidLineage};
use kelir_interface::driver_interface::SnapshotDescriptor;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::telemetry::MetricRegistry;

pub(crate) struct RealCheckpointOps {
    pub(crate) graph: RwLock<ManifestGraph>,
    pub(crate) next_node_id: AtomicU64,
    pub(crate) monotonic_watermark_ns: AtomicU64,
    pub(crate) metrics: Arc<MetricRegistry>,
}

impl RealCheckpointOps {
    pub(crate) fn new(metrics: Arc<MetricRegistry>) -> Self {
        Self {
            graph: RwLock::new(ManifestGraph::new()),
            next_node_id: AtomicU64::new(0),
            monotonic_watermark_ns: AtomicU64::new(0),
            metrics,
        }
    }

    pub(crate) fn lock_graph(&self) -> Result<RwLockWriteGuard<'_, ManifestGraph>, PluginError> {
        self.graph
            .write()
            .map_err(|_| PluginError::Halt("checkpoint graph lock poisoned".into()))
    }

    pub(crate) fn read_graph(&self) -> Result<RwLockReadGuard<'_, ManifestGraph>, PluginError> {
        self.graph
            .read()
            .map_err(|_| PluginError::Halt("checkpoint graph lock poisoned".into()))
    }
}

pub(crate) fn build_checkpoint_ops(metrics: Arc<MetricRegistry>) -> Arc<dyn CheckpointOps> {
    Arc::new(RealCheckpointOps::new(metrics))
}

impl CheckpointOps for RealCheckpointOps {
    // see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
    fn snapshot(&self, lineage: PidLineage, base: SnapshotDescriptor) -> Result<u64, PluginError> {
        let mut graph = self.lock_graph()?;
        let node_id = self.next_node_id.fetch_add(1, Ordering::SeqCst);
        let parent = graph.head(lineage);
        graph.insert(CheckpointNode {
            node_id,
            parent,
            descriptor: base,
            delta: None,
        });
        graph.set_head(lineage, node_id);
        Ok(node_id)
    }

    // see: cans/08-lifecycle.md#CoW-fork
    fn fork(&self, node_id: u64, lineage: PidLineage) -> Result<(), PluginError> {
        self.branch_from(node_id, lineage)
    }

    // see: cans/08-lifecycle.md#Delta-chain-compaction
    fn branch_from(&self, node_id: u64, lineage: PidLineage) -> Result<(), PluginError> {
        let mut graph = self.lock_graph()?;
        if graph.get(node_id).is_none() {
            return Err(PluginError::Halt(
                "branch source node not in manifest graph".into(),
            ));
        }
        if graph.has_lineage(lineage) {
            return Err(PluginError::Halt(
                "PID lineage already has a chain head".into(),
            ));
        }
        graph.set_head(lineage, node_id);
        Ok(())
    }

    // see: cans/09-snapshots.md#Restore-modes
    // see: cans/08-lifecycle.md#Clock-invariants
    fn rollback(
        &self,
        node_id: u64,
        guest_monotonic_ns: u64,
    ) -> Result<SnapshotDescriptor, PluginError> {
        let graph = self.read_graph()?;
        crate::compaction::rollback(
            &graph,
            node_id,
            guest_monotonic_ns,
            &self.monotonic_watermark_ns,
        )
    }

    fn head(&self, lineage: PidLineage) -> Option<u64> {
        let graph = self.read_graph().ok()?;
        crate::manifest_graph::head(&graph, lineage)
    }

    fn lineage(&self, node_id: u64) -> Result<Vec<u64>, PluginError> {
        let graph = self.read_graph()?;
        crate::manifest_graph::lineage(&graph, node_id)
    }

    // see: cans/08-lifecycle.md#Clock-invariants
    fn monotonic_watermark_ns(&self) -> u64 {
        self.monotonic_watermark_ns.load(Ordering::Acquire)
    }

    // see: cans/08-lifecycle.md#Delta-chain-compaction
    // see: cans/09-snapshots.md#Groups-and-lineage
    fn compact(&self, lineage: PidLineage) -> Result<u64, PluginError> {
        let mut graph = self.lock_graph()?;
        crate::compaction::compact(&mut graph, &self.next_node_id, lineage)
    }

    fn fresh(&self) -> Arc<dyn CheckpointOps> {
        Arc::new(Self::new(Arc::clone(&self.metrics)))
    }
}
