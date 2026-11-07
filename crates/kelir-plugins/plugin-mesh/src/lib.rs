// see: cans/plugins/mesh.md
#![forbid(unsafe_code)]

// TODO (L3): tests/test_mesh.rs per cans/artifacts/config/project-structure.txt.
use async_trait::async_trait;
use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{
    KelirPlugin, LifecycleCtx, LifecycleParticipant, MountScope, VirtioDevice,
};

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

impl Default for MeshConfig {
    fn default() -> Self {
        Self {
            peer_identity: PeerIdentity::Spiffe {
                trust_domain: "kelir.local".to_string(),
                workload_prefix: "swarm/worker".to_string(),
            },
            mtls: true,
            traceparent: TraceParentPropagation::InjectAndValidate,
            listeners: vec![
                PeerListener {
                    port: 7001,
                    service: "shard-exchange".to_string(),
                },
                PeerListener {
                    port: 7002,
                    service: "winner-announce".to_string(),
                },
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub struct MeshPlugin {
    pub config: MeshConfig,
}

impl MeshPlugin {
    pub fn new(config: MeshConfig) -> Self {
        Self { config }
    }

    pub fn listener_for(&self, port: u16) -> Option<&PeerListener> {
        self.config.listeners.iter().find(|l| l.port == port)
    }

    pub fn handshake_peer(&self, port: u16, spiffe_id: &str) -> Result<(), PluginError> {
        let listener = match self.listener_for(port) {
            Some(listener) => listener,
            None => {
                return Err(PluginError::Halt(format!(
                    "mesh: no vsock listener on port {port} (ECONNREFUSED)"
                )));
            }
        };
        let _ = listener;
        let trust_domain = match &self.config.peer_identity {
            PeerIdentity::Spiffe { trust_domain, .. } => trust_domain,
        };
        let expected_prefix = format!("spiffe://{trust_domain}/");
        if !spiffe_id.starts_with(&expected_prefix) {
            return Err(PluginError::Halt(format!(
                "mesh: SPIFFE identity outside trust domain {trust_domain} (ECONNREFUSED)"
            )));
        }
        // TODO (L3): validate the mTLS framing and SVID signature at the session boundary; SPIFFE prefix match only at L2.
        Ok(())
    }
}

impl Default for MeshPlugin {
    fn default() -> Self {
        Self::new(MeshConfig::default())
    }
}

impl KelirPlugin for MeshPlugin {
    type Context = ();
    fn context(&self) {}
    fn name(&self) -> &'static str {
        "mesh"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default().with_devices([VirtioDevice::Vsock])
    }
}

#[async_trait]
impl LifecycleParticipant for MeshPlugin {
    async fn on_vm_ready(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        // TODO (L3): issue the SPIFFE SVID bounded by the session lifetime at on_vm_ready.
        Ok(())
    }
}
