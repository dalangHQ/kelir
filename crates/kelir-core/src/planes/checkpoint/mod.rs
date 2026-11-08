// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-core::checkpoint)
// see: cans/08-lifecycle.md#Delta-chain-compaction
// see: cans/09-snapshots.md#Groups-and-lineage
// see: cans/10-attestation.md#Step-evaluation-loop
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

use kelir_interface::driver::SnapshotDescriptor;
use kelir_interface::plugin::PluginError;
use kelir_interface::telemetry::CausalDelta;

// see: cans/10-attestation.md#Step-evaluation-loop
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PidLineage {
    root_pid: u32,
}

impl PidLineage {
    pub const fn new(root_pid: u32) -> Self {
        Self { root_pid }
    }
    // TODO (L3): resolve guest pid to its lineage root by walking the trap-stream ppid chain.

    pub const fn root_pid(self) -> u32 {
        self.root_pid
    }
}

pub struct CheckpointNode {
    pub node_id: u64,
    pub parent: Option<u64>,
    pub descriptor: SnapshotDescriptor,
    pub delta: Option<CausalDelta>,
}

#[derive(Default)]
pub struct ManifestGraph {
    nodes: HashMap<u64, CheckpointNode>,
    heads: HashMap<PidLineage, u64>,
}

impl ManifestGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            heads: HashMap::new(),
        }
    }

    pub fn insert(&mut self, node: CheckpointNode) {
        self.nodes.insert(node.node_id, node);
    }

    pub fn get(&self, node_id: u64) -> Option<&CheckpointNode> {
        self.nodes.get(&node_id)
    }

    pub fn head(&self, lineage: PidLineage) -> Option<u64> {
        self.heads.get(&lineage).copied()
    }

    pub fn set_head(&mut self, lineage: PidLineage, node_id: u64) {
        self.heads.insert(lineage, node_id);
    }

    pub fn has_lineage(&self, lineage: PidLineage) -> bool {
        self.heads.contains_key(&lineage)
    }

    pub fn lineage(&self, node_id: u64) -> Vec<u64> {
        let mut chain = Vec::new();
        let mut current = Some(node_id);
        while let Some(id) = current {
            chain.push(id);
            current = self.nodes.get(&id).and_then(|node| node.parent);
        }
        chain
    }
}

// see: cans/08-lifecycle.md#Delta-chain-compaction
pub struct StepSlot {
    slot_id: u64,
    base_descriptor: SnapshotDescriptor,
    pending_delta: Option<CausalDelta>,
}

pub struct CheckpointPlane {
    active_steps: Mutex<HashMap<PidLineage, StepSlot>>,
    graph: RwLock<ManifestGraph>,
    next_slot_id: AtomicU64,
    metrics: std::sync::Arc<kelir_otlp::registry::MetricRegistry>,
}

impl Default for CheckpointPlane {
    fn default() -> Self {
        Self::new()
    }
}

impl CheckpointPlane {
    pub fn new() -> Self {
        Self {
            active_steps: Mutex::new(HashMap::new()),
            graph: RwLock::new(ManifestGraph::new()),
            next_slot_id: AtomicU64::new(0),
            metrics: std::sync::Arc::new(kelir_otlp::registry::MetricRegistry::new()),
        }
    }

    pub fn with_metrics(
        mut self,
        metrics: std::sync::Arc<kelir_otlp::registry::MetricRegistry>,
    ) -> Self {
        self.metrics = metrics;
        self
    }

    pub fn metrics(&self) -> &std::sync::Arc<kelir_otlp::registry::MetricRegistry> {
        &self.metrics
    }

    fn lock_steps(&self) -> Result<MutexGuard<'_, HashMap<PidLineage, StepSlot>>, PluginError> {
        self.active_steps
            .lock()
            .map_err(|_| PluginError::Halt("step slot lock poisoned".into()))
    }

    fn lock_graph(&self) -> Result<RwLockWriteGuard<'_, ManifestGraph>, PluginError> {
        self.graph
            .write()
            .map_err(|_| PluginError::Halt("checkpoint graph lock poisoned".into()))
    }

    fn read_graph(&self) -> Result<RwLockReadGuard<'_, ManifestGraph>, PluginError> {
        self.graph
            .read()
            .map_err(|_| PluginError::Halt("checkpoint graph lock poisoned".into()))
    }

    // see: cans/10-attestation.md#Step-evaluation-loop
    pub fn create_step(
        &self,
        lineage: PidLineage,
        base: SnapshotDescriptor,
    ) -> Result<u64, PluginError> {
        let mut steps = self.lock_steps()?;
        if steps.contains_key(&lineage) {
            return Err(PluginError::Halt(
                "step slot already active for PID lineage".into(),
            ));
        }
        let slot_id = self.next_slot_id.fetch_add(1, Ordering::SeqCst);
        steps.insert(
            lineage,
            StepSlot {
                slot_id,
                base_descriptor: base,
                pending_delta: None,
            },
        );
        Ok(slot_id)
    }

    // see: cans/08-lifecycle.md#Delta-chain-compaction
    pub fn commit_step(&self, lineage: PidLineage) -> Result<u64, PluginError> {
        let slot = {
            let mut steps = self.lock_steps()?;
            steps.remove(&lineage)
        };
        let Some(step) = slot else {
            return Err(PluginError::Halt(
                "no active step slot for PID lineage".into(),
            ));
        };
        let mut graph = self.lock_graph()?;
        let parent = graph.head(lineage);
        graph.insert(CheckpointNode {
            node_id: step.slot_id,
            parent,
            descriptor: step.base_descriptor,
            delta: step.pending_delta,
        });
        graph.set_head(lineage, step.slot_id);
        // TODO (L3): flush the staging delta into the Base sparse extents on the checkpoint slot.
        Ok(step.slot_id)
    }

    // see: cans/08-lifecycle.md#Delta-chain-compaction
    pub fn rollback_step(&self, lineage: PidLineage) -> Result<SnapshotDescriptor, PluginError> {
        let mut steps = self.lock_steps()?;
        match steps.remove(&lineage) {
            Some(step) => {
                // TODO (L3): discard the working delta from the checkpoint slot in <8ms.
                Ok(step.base_descriptor)
            }
            None => Err(PluginError::Halt(
                "no active step slot for PID lineage".into(),
            )),
        }
    }

    // see: cans/08-lifecycle.md#Delta-chain-compaction
    pub fn branch_from(&self, node_id: u64, lineage: PidLineage) -> Result<(), PluginError> {
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

    pub fn restore(&self, node_id: u64) -> Result<SnapshotDescriptor, PluginError> {
        let graph = self.read_graph()?;
        match graph.get(node_id) {
            Some(node) => Ok(node.descriptor.clone()),
            None => Err(PluginError::Halt(
                "checkpoint node not in manifest graph".into(),
            )),
        }
    }

    pub fn head(&self, lineage: PidLineage) -> Option<u64> {
        self.read_graph().ok()?.head(lineage)
    }

    pub fn lineage(&self, node_id: u64) -> Result<Vec<u64>, PluginError> {
        let graph = self.read_graph()?;
        if graph.get(node_id).is_none() {
            return Err(PluginError::Halt(
                "checkpoint node not in manifest graph".into(),
            ));
        }
        Ok(graph.lineage(node_id))
    }

    // see: cans/08-lifecycle.md#Delta-chain-compaction
    pub fn compact(&self, lineage: PidLineage) -> Result<u64, PluginError> {
        // TODO (L3): flatten ancestor deltas in a single-pass block copy and write a consolidated manifest pointing at the ancestor base.
        let _ = lineage;
        Err(PluginError::Halt(
            "TODO (L3): CheckpointPlane::compact".into(),
        ))
    }
}
