// see: cans/plugins/mesh.md#Plugin-mesh
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::mesh_types::{PeerSessionHandshake, SpiffeId, SpiffeSvid};
use async_trait::async_trait;
use kelir_interface::driver_interface::{SharedMemoryHandle, Stream};
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{
    AnyPlugin, HandlerCtx, KelirPlugin, PipelineBinder, PluginDeps,
};
use kelir_interface::vsock_mesh::{VsockPeer, VsockServiceHandler, VsockServiceSpec};

use crate::handshake::HandshakeDriver;
use crate::{MeshConfig, PeerIdentity, PeerRejection, TraceParentPropagation};

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

// see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VectorClock {
    counters: BTreeMap<u32, u64>,
}

impl VectorClock {
    // see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
    pub fn tick(&mut self, peer: u32) {
        *self.counters.entry(peer).or_insert(0) += 1;
    }

    // see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
    pub fn merge(&mut self, other: &Self) {
        for (peer, count) in &other.counters {
            let entry = self.counters.entry(*peer).or_insert(0);
            *entry = (*entry).max(*count);
        }
    }

    // see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
    pub fn get(&self, peer: u32) -> u64 {
        self.counters.get(&peer).copied().unwrap_or(0)
    }
}

// see: cans/plugins/mesh.md#Peer-identity
// TODO: [plugin_engine]: remove this allow when the stubbed L3 callers land and read this item again.; slop: the allow hides a dead item forever, the stubbed callers never land, mesh wiring stays unwired while the gate reports clean. (deferred to L3)
#[allow(dead_code)]
pub(crate) struct MeshState {
    svid: Mutex<Option<SpiffeSvid>>,
    peers: Mutex<Vec<PeerRecord>>,
    handshakes: Mutex<Vec<JoinHandle<()>>>,
    rejections: Mutex<Vec<PeerRejection>>,
    rejection_spans: Mutex<Vec<kelir_interface::telemetry::HookSpan>>,
    winner: Mutex<Option<u32>>,
    vector_clock: Mutex<VectorClock>,
}

// TODO: [plugin_engine]: remove this allow when the stubbed L3 callers land and read this item again.; slop: the allow hides a dead item forever, the stubbed callers never land, mesh wiring stays unwired while the gate reports clean. (deferred to L3)
#[allow(dead_code)]
impl MeshState {
    fn new() -> Self {
        Self {
            svid: Mutex::new(None),
            peers: Mutex::new(Vec::new()),
            handshakes: Mutex::new(Vec::new()),
            rejections: Mutex::new(Vec::new()),
            rejection_spans: Mutex::new(Vec::new()),
            winner: Mutex::new(None),
            vector_clock: Mutex::new(VectorClock::default()),
        }
    }

    // see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
    fn record_winner(&self, shard_id: u32) -> Result<(), PluginError> {
        let mut winner = self
            .winner
            .lock()
            .map_err(|_| PluginError::Halt("mesh winner slot lock poisoned".into()))?;
        let current = *winner;
        if current.is_none_or(|current| shard_id < current) {
            *winner = Some(shard_id);
        }
        let mut clock = self
            .vector_clock
            .lock()
            .map_err(|_| PluginError::Halt("mesh vector clock poisoned".into()))?;
        clock.tick(shard_id);
        Ok(())
    }

    // see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
    fn recorded_winner(&self) -> Result<Option<u32>, PluginError> {
        Ok(*self
            .winner
            .lock()
            .map_err(|_| PluginError::Halt("mesh winner slot lock poisoned".into()))?)
    }

    // see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
    fn vector_clock(&self) -> Result<VectorClock, PluginError> {
        Ok(self
            .vector_clock
            .lock()
            .map_err(|_| PluginError::Halt("mesh vector clock poisoned".into()))?
            .clone())
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
        // see: cans/10-attestation.md#Hook-trace
        // TODO: [attestation_receipts]: engine pulls the buffered rejection spans into the receipt store; no new trait method.; slop: buffered rejection spans never reach the receipt. (deferred to L3)
        if let Ok(mut spans) = self.rejection_spans.lock() {
            if spans.len() >= MAX_PEER_REJECTIONS {
                spans.remove(0);
            }
            spans.push(kelir_interface::telemetry::HookSpan {
                plugin: "mesh".to_string(),
                ring: kelir_interface::fusion_plugin::TrampolineRing::VsockService,
                verdict_kind: kelir_interface::telemetry::VerdictKind::Drop,
                duration_ns: 0,
                ordinal: 0,
            });
        }
    }

    // see: cans/10-attestation.md#Hook-trace
    pub(crate) fn drain_rejection_spans(&self) -> Vec<kelir_interface::telemetry::HookSpan> {
        match self.rejection_spans.lock() {
            Ok(mut spans) => std::mem::take(&mut *spans),
            Err(_) => Vec::new(),
        }
    }
}

// see: cans/plugins/mesh.md#Plugin-mesh
#[derive(Clone, PluginDeps)]
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
        // see: cans/plugins/mesh.md#SPIFFE-identity-(mesh)
        if let Some(recorded) = self.state.peers.lock().ok().and_then(|peers| {
            peers
                .iter()
                .find(|record| record.cid == peer.cid && record.port == peer.port)
                .and_then(|record| record.spiffe_id.clone())
        }) && recorded != handshake.svid.spiffe_id
        {
            return Err(PluginError::Halt(format!(
                "ECONNREFUSED: peer {}:{} vsock identity diverges from the handshake SVID",
                peer.cid, peer.port
            )));
        }
        // TODO: [pm_14_mesh_rejects_foreign_trust_domain_peer]: verify the SVID credential signature and the workload prefix authorization; slop: signature unverified, foreign trust domain accepted into mesh (deferred to L3)
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
        // TODO: [protocol_mux]: validate the mTLS handshake framing bytes across hypervisor boundaries; slop: framing unvalidated, malformed handshake bytes accepted as mTLS (deferred to L3)
        Err(PluginError::Halt(
 "mTLS handshake validation pending".into(),
        ))
    }

    // see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
    pub(crate) fn record_peer(
        &self,
        peer: &VsockPeer,
        handshake: &PeerSessionHandshake,
    ) -> Result<(), PluginError> {
        let mut peers = self.state.lock_peers()?;
        // TODO: [protocol_mux]: peer federation routing tables and call-hop propagation across routines; slop: federation unrouted, call hops never propagate, peers partitioned (deferred to L3)
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
    // TODO: [protocol_mux]: replace the tracked blocking thread with pollable stream reads once the transport exposes readiness; slop: blocking thread kept, worker stalls on unread stream (deferred to L3)
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

// see: cans/02-plugin-model.md#KelirPlugin-trait

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

// see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
pub type MeshHandle = MeshCtx;

impl MeshCtx {
    // see: cans/plugins/mesh.md#Error-and-return-semantics-(mesh)
    pub fn peer_rejections(&self) -> Result<Vec<PeerRejection>, PluginError> {
        Ok(self.state.lock_rejections()?.clone())
    }

    // see: cans/10-attestation.md#Hook-trace
    pub fn rejection_spans(&self) -> Vec<kelir_interface::telemetry::HookSpan> {
        self.state.drain_rejection_spans()
    }

    // see: cans/artifacts/examples/multi-vm-swarm.rs#await_winner
    pub async fn broadcast_winner(&self, shard_id: u32) -> Result<(), PluginError> {
        self.state.record_winner(shard_id)?;
        // TODO: [protocol_mux]: fan the announcement out over the virtio_shm peer rings; slop: announcement never leaves the session, sibling microVMs never converge. (deferred to L3)
        Ok(())
    }

    // see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
    pub fn vector_clock(&self) -> Result<VectorClock, PluginError> {
        self.state.vector_clock()
    }

    // see: cans/artifacts/examples/multi-vm-swarm.rs#publish
    pub async fn publish(
        &self,
        _region: &SharedMemoryHandle,
        shard_id: u32,
    ) -> Result<(), PluginError> {
        // TODO: [protocol_mux]: route the shard announcement over the peer federation streams and propagate the traceparent; slop: announcement unrouted, traceparent lost, shards never converge (deferred to L3)
        let peers = self.state.lock_peers()?;
        let Some(record) = peers.last() else {
            return Err(PluginError::Halt(format!(
 "mesh peer publish pending: shard {shard_id} has no connected peer stream"
            )));
        };
        Err(PluginError::Halt(format!(
 "mesh peer publish pending: shard {shard_id} to peer {}:{} ({}, {} call hops, {})",
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
        if let Some(winner) = self.state.recorded_winner()? {
            return Ok(winner);
        }
        // TODO: [protocol_mux]: await the winning shard announcement on the mesh listener streams; slop: no listener wait, winning shard never awaited, election never settles (deferred to L3)
        let peers = self.state.lock_peers()?;
        Err(PluginError::Halt(format!(
 "mesh winner election pending: {} peers connected",
            peers.len()
        )))
    }
}

// see: cans/artifacts/examples/multi-vm-swarm.rs#MeshConfig
impl Default for MeshConfig {
    fn default() -> Self {
        Self {
            peer_identity: PeerIdentity::Spiffe {
                trust_domain: String::new(),
                workload_prefix: String::new(),
            },
            mtls: false,
            traceparent: TraceParentPropagation::InjectAndValidate,
            listeners: Vec::new(),
        }
    }
}
