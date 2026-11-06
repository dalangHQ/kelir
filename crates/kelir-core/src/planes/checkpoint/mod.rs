// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-core::checkpoint)
// see: cans/08-lifecycle.md#Delta-chain-compaction
// see: cans/09-snapshots.md#Groups-and-lineage
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
    heads: HashMap<String, u64>,
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

    pub fn head(&self, branch: &str) -> Option<u64> {
        self.heads.get(branch).copied()
    }

    pub fn set_head(&mut self, branch: &str, node_id: u64) {
        self.heads.insert(branch.to_string(), node_id);
    }

    pub fn has_branch(&self, branch: &str) -> bool {
        self.heads.contains_key(branch)
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
    active_steps: Mutex<HashMap<String, StepSlot>>,
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
            active_steps: Mutex::new(HashMap::new()),
            graph: RwLock::new(ManifestGraph::new()),
            next_slot_id: AtomicU64::new(0),
        }
    }

    pub fn create_step(
        &self,
        base: kelir_interface::driver::SnapshotDescriptor,
    ) -> Result<u64, kelir_interface::errors::PluginError> {
        self.create_step_on_branch("main", base)
    }

    pub fn create_step_on_branch(
        &self,
        branch: &str,
        base: kelir_interface::driver::SnapshotDescriptor,
    ) -> Result<u64, kelir_interface::errors::PluginError> {
        let mut steps = self.active_steps.lock().map_err(|_| {
            kelir_interface::errors::PluginError::Halt("step slot lock poisoned".into())
        })?;
        if steps.contains_key(branch) {
            return Err(kelir_interface::errors::PluginError::Halt(
                "step already active on branch".into(),
            ));
        }
        let slot_id = self.next_slot_id.fetch_add(1, Ordering::SeqCst);
        steps.insert(
            branch.to_string(),
            StepSlot {
                slot_id,
                base_descriptor: base,
                pending_delta: None,
            },
        );
        Ok(slot_id)
    }

    pub fn commit_step(&self) -> Result<u64, kelir_interface::errors::PluginError> {
        self.commit_step_on_branch("main")
    }

    pub fn commit_step_on_branch(
        &self,
        branch: &str,
    ) -> Result<u64, kelir_interface::errors::PluginError> {
        let slot = {
            let mut steps = self.active_steps.lock().map_err(|_| {
                kelir_interface::errors::PluginError::Halt("step slot lock poisoned".into())
            })?;
            steps.remove(branch)
        };
        match slot {
            Some(step) => {
                let mut graph = self.graph.write().map_err(|_| {
                    kelir_interface::errors::PluginError::Halt(
                        "checkpoint graph lock poisoned".into(),
                    )
                })?;
                let parent = graph.head(branch);
                graph.insert(CheckpointNode {
                    node_id: step.slot_id,
                    parent,
                    descriptor: step.base_descriptor,
                    delta: step.pending_delta,
                });
                graph.set_head(branch, step.slot_id);
                Ok(step.slot_id)
            }
            None => Err(kelir_interface::errors::PluginError::Halt(
                "no active step slot on branch".into(),
            )),
        }
    }

    pub fn rollback_step(
        &self,
    ) -> Result<kelir_interface::driver::SnapshotDescriptor, kelir_interface::errors::PluginError>
    {
        self.rollback_step_on_branch("main")
    }

    pub fn rollback_step_on_branch(
        &self,
        branch: &str,
    ) -> Result<kelir_interface::driver::SnapshotDescriptor, kelir_interface::errors::PluginError>
    {
        let mut steps = self.active_steps.lock().map_err(|_| {
            kelir_interface::errors::PluginError::Halt("step slot lock poisoned".into())
        })?;
        match steps.remove(branch) {
            Some(step) => Ok(step.base_descriptor),
            None => Err(kelir_interface::errors::PluginError::Halt(
                "no active step slot on branch".into(),
            )),
        }
    }

    pub fn branch_from(
        &self,
        node_id: u64,
        branch: &str,
    ) -> Result<(), kelir_interface::errors::PluginError> {
        let mut graph = self.graph.write().map_err(|_| {
            kelir_interface::errors::PluginError::Halt("checkpoint graph lock poisoned".into())
        })?;
        if graph.get(node_id).is_none() {
            return Err(kelir_interface::errors::PluginError::Halt(
                "branch source node not in manifest graph".into(),
            ));
        }
        if graph.has_branch(branch) {
            return Err(kelir_interface::errors::PluginError::Halt(
                "branch already exists".into(),
            ));
        }
        graph.set_head(branch, node_id);
        Ok(())
    }

    pub fn restore(
        &self,
        node_id: u64,
    ) -> Result<kelir_interface::driver::SnapshotDescriptor, kelir_interface::errors::PluginError>
    {
        let graph = self.graph.read().map_err(|_| {
            kelir_interface::errors::PluginError::Halt("checkpoint graph lock poisoned".into())
        })?;
        match graph.get(node_id) {
            Some(node) => Ok(node.descriptor.clone()),
            None => Err(kelir_interface::errors::PluginError::Halt(
                "checkpoint node not in manifest graph".into(),
            )),
        }
    }

    pub fn head(&self, branch: &str) -> Option<u64> {
        self.graph.read().ok()?.head(branch)
    }

    pub fn lineage(&self, node_id: u64) -> Result<Vec<u64>, kelir_interface::errors::PluginError> {
        let graph = self.graph.read().map_err(|_| {
            kelir_interface::errors::PluginError::Halt("checkpoint graph lock poisoned".into())
        })?;
        if graph.get(node_id).is_none() {
            return Err(kelir_interface::errors::PluginError::Halt(
                "checkpoint node not in manifest graph".into(),
            ));
        }
        Ok(graph.lineage(node_id))
    }

    pub fn compact(&self, branch: &str) -> Result<u64, kelir_interface::errors::PluginError> {
        // TODO (L3): flatten ancestor deltas in a single-pass block copy and write a consolidated manifest pointing at the ancestor base.
        let _ = branch;
        Err(kelir_interface::errors::PluginError::Halt(
            "TODO (L3): CheckpointPlane::compact".into(),
        ))
    }
}
