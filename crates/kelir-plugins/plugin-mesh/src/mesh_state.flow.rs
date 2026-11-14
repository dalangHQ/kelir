// see: cans/plugins/mesh.md#Plugin-mesh
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use async_trait::async_trait;
use kelir_interface::driver_interface::{SharedMemoryHandle, Stream};
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{
    AnyPlugin, HandlerCtx, KelirPlugin, LifecycleCtx, LifecycleParticipant, NoDeps, PipelineBinder,
    PluginDeps,
};
use kelir_interface::vsock_mesh::{
    PeerSessionHandshake, SpiffeId, SpiffeSvid, VsockPeer, VsockServiceHandler, VsockServiceSpec,
};

use crate::handshake::HandshakeDriver;
use crate::{MeshConfig, PeerIdentity, PeerRejection};

// see: cans/02-plugin-model.md#Latency-ceiling
const MAX_CONCURRENT_HANDSHAKES: usize = 64;

// see: cans/plugins/mesh.md#Error-and-return-semantics-(mesh)
const MAX_PEER_REJECTIONS: usize = 256;

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
pub(crate) struct MeshState {
    svid: Mutex<Option<SpiffeSvid>>,
    peers: Mutex<Vec<PeerRecord>>,
    handshakes: Mutex<Vec<JoinHandle<()>>>,
    rejections: Mutex<Vec<PeerRejection>>,
}

impl MeshState {
    fn new() -> Self {
        Self {
            svid: Mutex::new(None),
            peers: Mutex::new(Vec::new()),
            handshakes: Mutex::new(Vec::new()),
            rejections: Mutex::new(Vec::new()),
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

    fn lock_rejections(&self) -> Result<MutexGuard<'_, Vec<PeerRejection>>, PluginError> {
        self.rejections
            .lock()
            .map_err(|_| PluginError::Halt("mesh rejection table lock poisoned".into()))
    }

    fn lock_handshakes(&self) -> Result<MutexGuard<'_, Vec<JoinHandle<()>>>, PluginError> {
        self.handshakes
            .lock()
            .map_err(|_| PluginError::Halt("mesh handshake table lock poisoned".into()))
    }

    // see: cans/02-plugin-model.md#Latency-ceiling
    fn track_handshake(&self, handle: JoinHandle<()>) -> Result<(), PluginError> {
        let mut handshakes = self.lock_handshakes()?;
        handshakes.retain(|handle| !handle.is_finished());
        if handshakes.len() >= MAX_CONCURRENT_HANDSHAKES {
            return Err(PluginError::Halt(
                "ECONNREFUSED: mesh peer handshake capacity exceeded".into(),
            ));
        }
        handshakes.push(handle);
        Ok(())
    }

    // see: cans/plugins/mesh.md#Error-and-return-semantics-(mesh)
    pub(crate) fn record_rejection(&self, rejection: PeerRejection) {
        if let Ok(mut rejections) = self.rejections.lock() {
            if rejections.len() >= MAX_PEER_REJECTIONS {
                rejections.remove(0);
            }
            rejections.push(rejection);
        }
    }

    // see: cans/plugins/mesh.md#Peer-identity
    fn set_svid(&self, svid: SpiffeSvid) -> Result<(), PluginError> {
        *self.lock_svid()? = Some(svid);
        Ok(())
    }
}

// see: cans/plugins/mesh.md#Plugin-mesh
#[derive(Clone)]
pub struct MeshPlugin {
    config: MeshConfig,
    pub(crate) state: Arc<MeshState>,
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
                // TODO (L3) [protocol_mux]: obtain the host-issued SVID credential bytes and the session-bounded expiry; slop: SVID self-minted locally, host issuance bypassed, expiry self-asserted
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
    pub(crate) fn assert_peer_identity(
        &self,
        peer: &VsockPeer,
        handshake: &PeerSessionHandshake,
    ) -> Result<(), PluginError> {
        let PeerIdentity::Spiffe { trust_domain, .. } = &self.config.peer_identity;
        if &handshake.svid.spiffe_id.trust_domain != trust_domain {
            return Err(PluginError::Halt(format!(
                "ECONNREFUSED: peer {}:{} presented trust domain {}",
                peer.cid, peer.port, handshake.svid.spiffe_id.trust_domain
            )));
        }
        if peer
            .spiffe_id
            .as_ref()
            .is_some_and(|peer_id| peer_id != &handshake.svid.spiffe_id)
        {
            return Err(PluginError::Halt(format!(
                "ECONNREFUSED: peer {}:{} vsock identity diverges from the handshake SVID",
                peer.cid, peer.port
            )));
        }
        // TODO (L3) [pm_14_mesh_rejects_foreign_trust_domain_peer]: verify the SVID credential signature and the workload prefix authorization; slop: signature unverified, foreign trust domain accepted into mesh
        Ok(())
    }

    // see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
    pub(crate) fn validate_mtls_framing(
        &self,
        _handshake: &PeerSessionHandshake,
        _bytes: &[u8],
    ) -> Result<(), PluginError> {
        if !self.config.mtls {
            return Ok(());
        }
        // TODO (L3) [protocol_mux]: validate the mTLS handshake framing bytes across hypervisor boundaries; slop: framing unvalidated, malformed handshake bytes accepted as mTLS
        Err(PluginError::Halt(
            "mTLS handshake validation pending (L3)".into(),
        ))
    }

    // see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
    pub(crate) fn record_peer(
        &self,
        peer: &VsockPeer,
        handshake: &PeerSessionHandshake,
    ) -> Result<(), PluginError> {
        let mut peers = self.state.lock_peers()?;
        // TODO (L3) [protocol_mux]: peer federation routing tables and call-hop propagation across routines; slop: federation unrouted, call hops never propagate, peers partitioned
        peers.push(PeerRecord {
            cid: peer.cid,
            port: peer.port,
            spiffe_id: Some(handshake.svid.spiffe_id.clone()),
            call_hops: handshake.call_hops,
            traceparent: handshake.traceparent.clone(),
        });
        Ok(())
    }

    // see: cans/02-plugin-model.md#Latency-ceiling
    // TODO (L3) [protocol_mux]: replace the tracked blocking thread with pollable stream reads once the transport exposes readiness; slop: blocking thread kept, worker stalls on unread stream
    fn spawn_peer_handshake(
        &self,
        peer: &VsockPeer,
        stream: Box<dyn Stream>,
    ) -> Result<(), PluginError> {
        let mut driver = HandshakeDriver {
            plugin: self.clone(),
            peer: peer.clone(),
            stream,
        };
        let handle = std::thread::Builder::new()
            .name("mesh-peer-handshake".into())
            .spawn(move || driver.run())
            .map_err(PluginError::Io)?;
        self.state.track_handshake(handle)
    }
}

// see: cans/plugins/mesh.md#Plugin-mesh
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
            pipeline.bind_vsock_service_scoped(
                VsockServiceSpec {
                    port: listener.port,
                    service: listener.service.clone(),
                    schema: None,
                },
                Arc::clone(&handler),
                KelirPlugin::default_scope(self),
            );
        }
    }
}

// see: cans/plugins/mesh.md#Peer-identity
#[async_trait]
impl LifecycleParticipant for MeshPlugin {
    async fn on_vm_ready(&self, _ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        let svid = self.issue_session_svid()?;
        self.state.set_svid(svid)
    }
}

// see: cans/02-plugin-model.md#KelirPlugin-trait
impl PluginDeps for MeshPlugin {
    type Deps = NoDeps;
}

// see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
#[async_trait]
impl VsockServiceHandler for MeshPlugin {
    async fn on_connect(
        &self,
        peer: &VsockPeer,
        stream: Box<dyn Stream>,
        _ctx: &HandlerCtx,
    ) -> Result<(), PluginError> {
        // see: cans/02-plugin-model.md#Latency-ceiling
        self.spawn_peer_handshake(peer, stream)
    }
}

// see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
#[derive(Clone)]
pub struct MeshCtx {
    state: Arc<MeshState>,
}

impl MeshCtx {
    // see: cans/plugins/mesh.md#Error-and-return-semantics-(mesh)
    pub fn peer_rejections(&self) -> Result<Vec<PeerRejection>, PluginError> {
        Ok(self.state.lock_rejections()?.clone())
    }

    // see: cans/artifacts/examples/multi-vm-swarm.rs#publish
    pub async fn publish(
        &self,
        _region: &SharedMemoryHandle,
        shard_id: u32,
    ) -> Result<(), PluginError> {
        // TODO (L3) [protocol_mux]: route the shard announcement over the peer federation streams and propagate the traceparent; slop: announcement unrouted, traceparent lost, shards never converge
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

    // see: cans/artifacts/examples/multi-vm-swarm.rs#await_winner
    pub async fn await_winner(&self, _timeout: Duration) -> Result<u32, PluginError> {
        // TODO (L3) [protocol_mux]: await the winning shard announcement on the mesh listener streams; slop: no listener wait, winning shard never awaited, election never settles
        let peers = self.state.lock_peers()?;
        Err(PluginError::Halt(format!(
            "mesh winner election pending (L3): {} peers connected",
            peers.len()
        )))
    }
}
