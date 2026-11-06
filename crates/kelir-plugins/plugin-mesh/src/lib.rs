// see: cans/plugins/mesh.md
#![forbid(unsafe_code)]

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use async_trait::async_trait;
use kelir_interface::codec::MAX_CONTROL_PAYLOAD;
use kelir_interface::driver::{SharedMemoryHandle, Stream};
use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{
    AnyPlugin, HandlerCtx, KelirPlugin, LifecycleCtx, LifecycleParticipant, MountScope,
    PipelineBinder,
};
use kelir_interface::vsock::{
    PeerSessionHandshake, SpiffeId, SpiffeSvid, VsockPeer, VsockServiceHandler, VsockServiceSpec,
};

// see: cans/plugins/mesh.md#Error-and-return-semantics-(mesh)
const ECONNREFUSED: &str = "ECONNREFUSED";

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

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.mesh.listeners
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerListener {
    pub port: u32,
    pub service: String,
}

// see: cans/artifacts/examples/multi-vm-swarm.rs
#[derive(Debug, Clone)]
pub struct MeshConfig {
    pub peer_identity: PeerIdentity,
    pub mtls: bool,
    pub traceparent: TraceParentPropagation,
    pub listeners: Vec<PeerListener>,
}

// see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
#[derive(Clone)]
struct PeerRecord {
    cid: u32,
    port: u32,
    spiffe_id: Option<SpiffeId>,
    call_hops: u32,
    traceparent: Option<String>,
}

// see: cans/plugins/mesh.md#Peer-identity
struct MeshState {
    svid: Mutex<Option<SpiffeSvid>>,
    peers: Mutex<Vec<PeerRecord>>,
}

impl MeshState {
    fn new() -> Self {
        Self {
            svid: Mutex::new(None),
            peers: Mutex::new(Vec::new()),
        }
    }

    fn lock_svid(&self) -> Result<MutexGuard<'_, Option<SpiffeSvid>>, PluginError> {
        self.svid
            .lock()
            .map_err(|_| PluginError::Halt("mesh svid slot lock poisoned".into()))
    }

    fn lock_peers(&self) -> Result<MutexGuard<'_, Vec<PeerRecord>>, PluginError> {
        self.peers
            .lock()
            .map_err(|_| PluginError::Halt("mesh peer table lock poisoned".into()))
    }

    // see: cans/plugins/mesh.md#Peer-identity
    fn set_svid(&self, svid: SpiffeSvid) -> Result<(), PluginError> {
        *self.lock_svid()? = Some(svid);
        Ok(())
    }
}

// see: cans/plugins/mesh.md
#[derive(Clone)]
pub struct MeshPlugin {
    config: MeshConfig,
    state: Arc<MeshState>,
}

impl MeshPlugin {
    pub fn new(config: MeshConfig) -> Self {
        Self {
            config,
            state: Arc::new(MeshState::new()),
        }
    }

    // see: cans/plugins/mesh.md#Peer-identity
    fn issue_session_svid(&self) -> Result<SpiffeSvid, PluginError> {
        match &self.config.peer_identity {
            PeerIdentity::Spiffe {
                trust_domain,
                workload_prefix,
            } => {
                // TODO (L3): obtain the host-issued SVID credential bytes and the session-bounded expiry.
                Ok(SpiffeSvid {
                    spiffe_id: SpiffeId {
                        trust_domain: trust_domain.clone(),
                        workload_prefix: workload_prefix.clone(),
                    },
                    credential: Vec::new(),
                    expires_at_ns: 0,
                })
            }
        }
    }

    // see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
    fn assert_peer_identity(
        &self,
        peer: &VsockPeer,
        handshake: &PeerSessionHandshake,
    ) -> Result<(), PluginError> {
        let PeerIdentity::Spiffe { trust_domain, .. } = &self.config.peer_identity;
        if &handshake.svid.spiffe_id.trust_domain != trust_domain {
            return Err(PluginError::reject(
                ECONNREFUSED,
                format!(
                    "peer {}:{} presented trust domain {}",
                    peer.cid, peer.port, handshake.svid.spiffe_id.trust_domain
                ),
            ));
        }
        if let Some(peer_id) = &peer.spiffe_id
            && peer_id != &handshake.svid.spiffe_id
        {
            return Err(PluginError::reject(
                ECONNREFUSED,
                format!(
                    "peer {}:{} vsock identity diverges from the handshake SVID",
                    peer.cid, peer.port
                ),
            ));
        }
        // TODO (L3): verify the SVID credential signature and the workload prefix authorization.
        Ok(())
    }

    // see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
    fn validate_mtls_framing(
        &self,
        _handshake: &PeerSessionHandshake,
        _bytes: &[u8],
    ) -> Result<(), PluginError> {
        if !self.config.mtls {
            return Ok(());
        }
        // TODO (L3): validate the mTLS handshake framing bytes across hypervisor boundaries.
        Err(PluginError::Halt(
            "mTLS handshake validation pending (L3)".into(),
        ))
    }

    // see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
    fn record_peer(
        &self,
        peer: &VsockPeer,
        handshake: &PeerSessionHandshake,
    ) -> Result<(), PluginError> {
        let mut peers = self.state.lock_peers()?;
        // TODO (L3): peer federation routing tables and call-hop propagation across routines.
        peers.push(PeerRecord {
            cid: peer.cid,
            port: peer.port,
            spiffe_id: Some(handshake.svid.spiffe_id.clone()),
            call_hops: handshake.call_hops,
            traceparent: handshake.traceparent.clone(),
        });
        Ok(())
    }
}

// see: cans/plugins/mesh.md
impl KelirPlugin for MeshPlugin {
    type Context = MeshCtx;
    fn context(&self) -> Self::Context {
        MeshCtx {
            state: Arc::clone(&self.state),
        }
    }
    fn name(&self) -> &'static str {
        "mesh"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }

    // see: cans/plugins/mesh.md#Peer-identity
    fn fork_state(&self, _child_session_id: &str) -> Box<dyn AnyPlugin> {
        // see: cans/08-lifecycle.md#CoW-fork
        Box::new(MeshPlugin {
            config: self.config.clone(),
            state: Arc::new(MeshState::new()),
        })
    }

    // see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        let handler: Arc<dyn VsockServiceHandler> = Arc::new(self.clone());
        for listener in &self.config.listeners {
            pipeline.bind_vsock_service(
                VsockServiceSpec {
                    port: listener.port,
                    service: listener.service.clone(),
                },
                Arc::clone(&handler),
            );
        }
    }
}

// see: cans/plugins/mesh.md#Peer-identity
#[async_trait]
impl LifecycleParticipant for MeshPlugin {
    async fn on_vm_ready(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        let svid = self.issue_session_svid()?;
        self.state.set_svid(svid)
    }
}

// see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
#[async_trait]
impl VsockServiceHandler for MeshPlugin {
    async fn on_connect(
        &self,
        peer: &VsockPeer,
        mut stream: Box<dyn Stream>,
        _ctx: &HandlerCtx,
    ) -> Result<(), PluginError> {
        // see: cans/11-protocol.md#Control-channel
        let mut scratch = [0u8; MAX_CONTROL_PAYLOAD];
        let n = stream
            .read(&mut scratch)
            .map_err(|e| PluginError::Halt(format!("mesh handshake read failed: {e}")))?;
        let handshake = decode_peer_handshake(&scratch[..n])?;
        self.assert_peer_identity(peer, &handshake)?;
        self.validate_mtls_framing(&handshake, &scratch[..n])?;
        self.record_peer(peer, &handshake)
    }
}

// see: cans/plugins/mesh.md#Error-and-return-semantics-(mesh)
fn decode_peer_handshake(buf: &[u8]) -> Result<PeerSessionHandshake, PluginError> {
    // TODO (L3): decode the Channel 0 PeerSessionHandshake per cans/11-protocol.md#Wire-framing.
    Err(PluginError::Halt(format!(
        "peer handshake decode pending (L3): {} bytes",
        buf.len()
    )))
}

// see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
#[derive(Clone)]
pub struct MeshCtx {
    state: Arc<MeshState>,
}

impl MeshCtx {
    // see: cans/artifacts/examples/multi-vm-swarm.rs
    pub async fn publish(
        &self,
        _region: &SharedMemoryHandle,
        shard_id: u32,
    ) -> Result<(), PluginError> {
        // TODO (L3): route the shard announcement over the peer federation streams and propagate the traceparent.
        let peers = self.state.lock_peers()?;
        let Some(record) = peers.last() else {
            return Err(PluginError::Halt(format!(
                "mesh peer publish pending (L3): shard {shard_id} has no connected peer stream"
            )));
        };
        Err(PluginError::Halt(format!(
            "mesh peer publish pending (L3): shard {shard_id} to peer {}:{} ({}, {} call hops, {})",
            record.cid,
            record.port,
            match &record.spiffe_id {
                Some(id) => format!("{}/{}", id.trust_domain, id.workload_prefix),
                None => "unauthenticated".into(),
            },
            record.call_hops,
            record.traceparent.as_deref().unwrap_or("untraced"),
        )))
    }

    // see: cans/artifacts/examples/multi-vm-swarm.rs
    pub async fn await_winner(&self, _timeout: Duration) -> Result<u32, PluginError> {
        // TODO (L3): await the winning shard announcement on the mesh listener streams.
        let peers = self.state.lock_peers()?;
        Err(PluginError::Halt(format!(
            "mesh winner election pending (L3): {} peers connected",
            peers.len()
        )))
    }
}
