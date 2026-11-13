// see: cans/plugins/mesh.md#Peer-identity
#![forbid(unsafe_code)]

use std::sync::{Mutex, MutexGuard};
use std::thread::JoinHandle;

use kelir_interface::plugin_model::PluginError;
use kelir_interface::vsock_mesh::{SpiffeId, SpiffeSvid};

use crate::PeerRejection;

// see: cans/02-plugin-model.md#Latency-ceiling
const MAX_CONCURRENT_HANDSHAKES: usize = 64;

// see: cans/plugins/mesh.md#Error-and-return-semantics-(mesh)
const MAX_PEER_REJECTIONS: usize = 256;

// see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
#[derive(Clone)]
pub(crate) struct PeerRecord {
    pub(crate) cid: u32,
    pub(crate) port: u32,
    pub(crate) spiffe_id: Option<SpiffeId>,
    pub(crate) call_hops: u32,
    pub(crate) traceparent: Option<String>,
}

// see: cans/plugins/mesh.md#Peer-identity
pub(crate) struct MeshState {
    pub(crate) svid: Mutex<Option<SpiffeSvid>>,
    pub(crate) peers: Mutex<Vec<PeerRecord>>,
    pub(crate) handshakes: Mutex<Vec<JoinHandle<()>>>,
    pub(crate) rejections: Mutex<Vec<PeerRejection>>,
}

impl MeshState {
    pub(crate) fn new() -> Self {
        Self {
            svid: Mutex::new(None),
            peers: Mutex::new(Vec::new()),
            handshakes: Mutex::new(Vec::new()),
            rejections: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn lock_svid(&self) -> Result<MutexGuard<'_, Option<SpiffeSvid>>, PluginError> {
        self.svid
            .lock()
            .map_err(|_| PluginError::Halt("mesh svid slot lock poisoned".into()))
    }

    pub(crate) fn lock_peers(&self) -> Result<MutexGuard<'_, Vec<PeerRecord>>, PluginError> {
        self.peers
            .lock()
            .map_err(|_| PluginError::Halt("mesh peer table lock poisoned".into()))
    }

    pub(crate) fn lock_rejections(
        &self,
    ) -> Result<MutexGuard<'_, Vec<PeerRejection>>, PluginError> {
        self.rejections
            .lock()
            .map_err(|_| PluginError::Halt("mesh rejection table lock poisoned".into()))
    }

    pub(crate) fn lock_handshakes(
        &self,
    ) -> Result<MutexGuard<'_, Vec<JoinHandle<()>>>, PluginError> {
        self.handshakes
            .lock()
            .map_err(|_| PluginError::Halt("mesh handshake table lock poisoned".into()))
    }

    // see: cans/02-plugin-model.md#Latency-ceiling
    pub(crate) fn track_handshake(&self, handle: JoinHandle<()>) -> Result<(), PluginError> {
        let mut handshakes = self.lock_handshakes()?;
        handshakes.retain(|handle| !handle.is_finished());
        if handshakes.len() >= MAX_CONCURRENT_HANDSHAKES {
            return Err(PluginError::reject(
                crate::ECONNREFUSED,
                "mesh peer handshake capacity exceeded",
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
    pub(crate) fn set_svid(&self, svid: SpiffeSvid) -> Result<(), PluginError> {
        *self.lock_svid()? = Some(svid);
        Ok(())
    }
}
