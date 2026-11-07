// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-core::checkpoint)
// see: cans/08-lifecycle.md#Delta-chain-compaction
// see: cans/09-snapshots.md#Groups-and-lineage
// see: cans/10-attestation.md#Step-evaluation-loop
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};

pub struct CheckpointNode {
    pub node_id: u64,
    pub parent: Option<u64>,
    pub descriptor: kelir_interface::driver::SnapshotDescriptor,
    pub delta: Option<kelir_interface::telemetry::CausalDelta>,
}

#[derive(Default)]
pub struct ManifestGraph {
    nodes: HashMap<u64, CheckpointNode>,
    head: Option<u64>,
}

impl ManifestGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            head: None,
        }
    }

    pub fn insert(&mut self, node: CheckpointNode) {
        self.nodes.insert(node.node_id, node);
    }

    pub fn get(&self, node_id: u64) -> Option<&CheckpointNode> {
        self.nodes.get(&node_id)
    }

    pub fn head(&self) -> Option<u64> {
        self.head
    }

    pub fn set_head(&mut self, node_id: u64) {
        self.head = Some(node_id);
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

pub struct StepSlot {
    slot_id: u64,
    base_descriptor: kelir_interface::driver::SnapshotDescriptor,
    pending_delta: Option<kelir_interface::telemetry::CausalDelta>,
}

pub struct CheckpointPlane {
    active_step: Mutex<Option<StepSlot>>,
    graph: RwLock<ManifestGraph>,
    next_slot_id: AtomicU64,
}

impl Default for CheckpointPlane {
    fn default() -> Self {
        Self::new()
    }
}

impl CheckpointPlane {
    pub fn new() -> Self {
        Self {
            active_step: Mutex::new(None),
            graph: RwLock::new(ManifestGraph::new()),
            next_slot_id: AtomicU64::new(0),
        }
    }

    pub fn create_step(
        &self,
        base: kelir_interface::driver::SnapshotDescriptor,
    ) -> Result<u64, kelir_interface::plugin::PluginError> {
        let mut step = self.active_step.lock().map_err(|_| {
            kelir_interface::plugin::PluginError::Halt("step slot lock poisoned".into())
        })?;
        if step.is_some() {
            return Err(kelir_interface::plugin::PluginError::Halt(
                "step slot already active for lineage".into(),
            ));
        }
        let slot_id = self.next_slot_id.fetch_add(1, Ordering::SeqCst);
        *step = Some(StepSlot {
            slot_id,
            base_descriptor: base,
            pending_delta: None,
        });
        Ok(slot_id)
    }

    pub fn commit_step(&self) -> Result<u64, kelir_interface::plugin::PluginError> {
        let slot = {
            let mut step = self.active_step.lock().map_err(|_| {
                kelir_interface::plugin::PluginError::Halt("step slot lock poisoned".into())
            })?;
            step.take()
        };
        match slot {
            Some(step) => {
                let mut graph = self.graph.write().map_err(|_| {
                    kelir_interface::plugin::PluginError::Halt(
                        "checkpoint graph lock poisoned".into(),
                    )
                })?;
                let parent = graph.head();
                graph.insert(CheckpointNode {
                    node_id: step.slot_id,
                    parent,
                    descriptor: step.base_descriptor,
                    delta: step.pending_delta,
                });
                graph.set_head(step.slot_id);
                Ok(step.slot_id)
            }
            None => Err(kelir_interface::plugin::PluginError::Halt(
                "no active step slot".into(),
            )),
        }
    }

    pub fn rollback_step(
        &self,
    ) -> Result<kelir_interface::driver::SnapshotDescriptor, kelir_interface::plugin::PluginError>
    {
        let mut step = self.active_step.lock().map_err(|_| {
            kelir_interface::plugin::PluginError::Halt("step slot lock poisoned".into())
        })?;
        match step.take() {
            Some(step) => Ok(step.base_descriptor),
            None => Err(kelir_interface::plugin::PluginError::Halt(
                "no active step slot".into(),
            )),
        }
    }

    pub fn branch_from(&self, node_id: u64) -> Result<(), kelir_interface::plugin::PluginError> {
        let mut graph = self.graph.write().map_err(|_| {
            kelir_interface::plugin::PluginError::Halt("checkpoint graph lock poisoned".into())
        })?;
        if graph.get(node_id).is_none() {
            return Err(kelir_interface::plugin::PluginError::Halt(
                "branch source node not in manifest graph".into(),
            ));
        }
        graph.set_head(node_id);
        Ok(())
    }

    pub fn restore(
        &self,
        node_id: u64,
    ) -> Result<kelir_interface::driver::SnapshotDescriptor, kelir_interface::plugin::PluginError>
    {
        let graph = self.graph.read().map_err(|_| {
            kelir_interface::plugin::PluginError::Halt("checkpoint graph lock poisoned".into())
        })?;
        match graph.get(node_id) {
            Some(node) => Ok(node.descriptor.clone()),
            None => Err(kelir_interface::plugin::PluginError::Halt(
                "checkpoint node not in manifest graph".into(),
            )),
        }
    }

    pub fn head(&self) -> Option<u64> {
        self.graph.read().ok()?.head()
    }

    pub fn lineage(&self, node_id: u64) -> Result<Vec<u64>, kelir_interface::plugin::PluginError> {
        let graph = self.graph.read().map_err(|_| {
            kelir_interface::plugin::PluginError::Halt("checkpoint graph lock poisoned".into())
        })?;
        if graph.get(node_id).is_none() {
            return Err(kelir_interface::plugin::PluginError::Halt(
                "checkpoint node not in manifest graph".into(),
            ));
        }
        Ok(graph.lineage(node_id))
    }

    pub fn compact(&self) -> Result<u64, kelir_interface::plugin::PluginError> {
        // TODO (L3): flatten ancestor deltas in a single-pass block copy and write a consolidated manifest pointing at the ancestor base.
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO (L3): CheckpointPlane::compact".into(),
        ))
    }
}
