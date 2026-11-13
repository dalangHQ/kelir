// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

use kelir_interface::checkpoint::{CheckpointOps, ManifestGraph, PidLineage, StepSlot};
use kelir_interface::driver_interface::SnapshotDescriptor;
use kelir_interface::plugin_model::PluginError;
use kelir_otlp::registry_otlp::MetricRegistry;

pub(crate) struct RealCheckpointOps {
    pub(crate) active_steps: Mutex<HashMap<PidLineage, StepSlot>>,
    pub(crate) graph: RwLock<ManifestGraph>,
    pub(crate) next_slot_id: AtomicU64,
    pub(crate) monotonic_watermark_ns: AtomicU64,
    pub(crate) metrics: Arc<MetricRegistry>,
}

impl RealCheckpointOps {
    pub(crate) fn new(metrics: Arc<MetricRegistry>) -> Self {
        Self {
            active_steps: Mutex::new(HashMap::new()),
            graph: RwLock::new(ManifestGraph::new()),
            next_slot_id: AtomicU64::new(0),
            monotonic_watermark_ns: AtomicU64::new(0),
            metrics,
        }
    }

    pub(crate) fn lock_steps(
        &self,
    ) -> Result<MutexGuard<'_, HashMap<PidLineage, StepSlot>>, PluginError> {
        self.active_steps
            .lock()
            .map_err(|_| PluginError::Halt("step slot lock poisoned".into()))
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
    // see: cans/10-attestation.md#Step-evaluation-loop
    fn create_step(
        &self,
        lineage: PidLineage,
        base: SnapshotDescriptor,
    ) -> Result<u64, PluginError> {
        let mut steps = self.lock_steps()?;
        crate::step_slot::create_step(&mut steps, &self.next_slot_id, lineage, base)
    }

    // see: cans/10-attestation.md#Step-evaluation-loop
    fn active_step(&self, lineage: PidLineage) -> Result<Option<SnapshotDescriptor>, PluginError> {
        let steps = self.lock_steps()?;
        Ok(crate::step_slot::active_descriptor(&steps, lineage))
    }

    // see: cans/08-lifecycle.md#Delta-chain-compaction
    fn commit_step(&self, lineage: PidLineage) -> Result<u64, PluginError> {
        let step = {
            let mut steps = self.lock_steps()?;
            crate::step_slot::take_step(&mut steps, lineage)?
        };
        let mut graph = self.lock_graph()?;
        crate::step_slot::commit_step(&mut graph, step, lineage)
    }

    // see: cans/08-lifecycle.md#Delta-chain-compaction
    fn rollback_step(&self, lineage: PidLineage) -> Result<SnapshotDescriptor, PluginError> {
        let mut steps = self.lock_steps()?;
        crate::step_slot::rollback_step(&mut steps, lineage)
    }

    // see: cans/08-lifecycle.md#Delta-chain-compaction
    fn branch_from(&self, node_id: u64, lineage: PidLineage) -> Result<(), PluginError> {
        let mut graph = self.lock_graph()?;
        crate::step_slot::branch_from(&mut graph, node_id, lineage)
    }

    // see: cans/09-snapshots.md#Restore-modes
    // see: cans/08-lifecycle.md#Clock-invariants
    fn restore(
        &self,
        node_id: u64,
        guest_monotonic_ns: u64,
    ) -> Result<SnapshotDescriptor, PluginError> {
        let graph = self.read_graph()?;
        crate::compaction::restore(
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
        crate::compaction::compact(&mut graph, &self.next_slot_id, lineage)
    }

    fn fresh(&self) -> Arc<dyn CheckpointOps> {
        Arc::new(Self::new(Arc::clone(&self.metrics)))
    }
}
