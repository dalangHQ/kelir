// Mesh plugin stub.
use std::time::Duration;

use kelir_interface::errors::PluginError;

use crate::plugin::KelirPlugin;

#[derive(Debug, Clone)]
pub enum PeerIdentity {
    Spiffe {
        trust_domain: String,
        workload_prefix: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceParentPropagation {
    InjectAndValidate,
}

#[derive(Debug, Clone)]
pub struct PeerListener {
    pub port: u16,
    pub service: String,
}

#[derive(Debug, Clone)]
pub struct MeshConfig {
    pub peer_identity: PeerIdentity,
    pub mtls: bool,
    pub traceparent: TraceParentPropagation,
    pub listeners: Vec<PeerListener>,
}

#[derive(Debug, Clone, Default)]
pub struct MeshPlugin;

impl MeshPlugin {
    pub fn new(_config: MeshConfig) -> Self {
        Self
    }
}

impl KelirPlugin for MeshPlugin {
    type Context = ();
    fn context(&self) -> Self::Context {}
    fn name(&self) -> &'static str {
        "mesh"
    }
}

/// Typed mesh handle (`session.ctx::<MeshCtx>()`).
#[derive(Debug, Clone, Default)]
pub struct MeshCtx;

impl MeshCtx {
    pub async fn publish(&self, _data: &[u8], _shard: u32) -> Result<(), PluginError> {
        Ok(())
    }

    pub async fn await_winner(&self, _timeout: Duration) -> Result<u32, PluginError> {
        Ok(0)
    }
}
