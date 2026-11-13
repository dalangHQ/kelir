// see: cans/08-lifecycle.md#Delta-chain-compaction
// see: cans/09-snapshots.md#Groups-and-lineage
// see: cans/10-attestation.md#Step-evaluation-loop
use std::collections::HashMap;

use crate::driver_interface::SnapshotDescriptor;
use crate::telemetry::CausalDelta;

// see: cans/10-attestation.md#Step-evaluation-loop
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PidLineage {
    root_pid: u32,
    incarnation_id: [u8; 16],
}

impl PidLineage {
    // see: cans/08-lifecycle.md#CoW-fork
    pub const fn new_in_session(root_pid: u32, incarnation_id: [u8; 16]) -> Self {
        Self {
            root_pid,
            incarnation_id,
        }
    }
    // TODO (L3) [lifecycle_checkpoint]: resolve guest pid to its lineage root by walking the trap-stream ppid chain; slop: step attribution without lineage root, wrong step blamed

    pub const fn root_pid(self) -> u32 {
        self.root_pid
    }

    pub const fn incarnation_id(self) -> [u8; 16] {
        self.incarnation_id
    }
}

#[derive(Clone)]
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

impl StepSlot {
    pub fn new(slot_id: u64, base_descriptor: SnapshotDescriptor) -> Self {
        Self {
            slot_id,
            base_descriptor,
            pending_delta: None,
        }
    }

    pub fn slot_id(&self) -> u64 {
        self.slot_id
    }

    pub fn base_descriptor(&self) -> &SnapshotDescriptor {
        &self.base_descriptor
    }

    pub fn pending_delta(&self) -> Option<&CausalDelta> {
        self.pending_delta.as_ref()
    }
}
