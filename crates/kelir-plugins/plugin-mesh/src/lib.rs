// see: cans/plugins/mesh.md#Plugin-mesh
#![forbid(unsafe_code)]

#[path = "federation.run.rs"]
pub mod federation;

#[path = "handshake.flow.rs"]
pub mod handshake;

#[path = "mesh_state.flow.rs"]
pub mod mesh_state;

pub use mesh_state::{MeshCtx, MeshPlugin};

// see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerIdentity {
    Spiffe {
        trust_domain: String,
        workload_prefix: String,
    },
}

// see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceParentPropagation {
    InjectAndValidate,
}

// see: cans/plugins/mesh.md#Error-and-return-semantics-(mesh)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerRejection {
    pub cid: u32,
    pub port: u32,
    pub reason: String,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.mesh.listeners
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerListener {
    pub port: u32,
    pub service: String,
}

// see: cans/artifacts/examples/multi-vm-swarm.rs#MeshConfig
#[derive(Debug, Clone)]
pub struct MeshConfig {
    pub peer_identity: PeerIdentity,
    pub mtls: bool,
    pub traceparent: TraceParentPropagation,
    pub listeners: Vec<PeerListener>,
}
