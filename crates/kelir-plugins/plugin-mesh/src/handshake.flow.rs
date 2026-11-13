// see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
use kelir_interface::codec_protocol::MAX_FRAME_PAYLOAD;
use kelir_interface::driver_interface::Stream;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::vsock_mesh::{PeerSessionHandshake, VsockPeer};

use crate::{MeshPlugin, PeerRejection};

// see: cans/02-plugin-model.md#Latency-ceiling
pub(crate) struct HandshakeDriver {
    pub(crate) plugin: MeshPlugin,
    pub(crate) peer: VsockPeer,
    pub(crate) stream: Box<dyn Stream>,
}

impl HandshakeDriver {
    // see: cans/plugins/mesh.md#Error-and-return-semantics-(mesh)
    pub(crate) fn run(&mut self) {
        if let Err(error) = self.handshake() {
            // TODO (L3) [protocol_mux]: forward the peer fingerprint to plugin-ledger on mTLS rejection per cans/plugins/mesh.md; slop: rejection never forwarded to ledger, audit misses the foreign peer
            self.plugin.state.record_rejection(PeerRejection {
                cid: self.peer.cid,
                port: self.peer.port,
                reason: error.to_string(),
            });
        }
    }

    // see: cans/plugins/mesh.md#Invariants-and-policy-(mesh)
    fn handshake(&mut self) -> Result<(), PluginError> {
        let frame = self.read_handshake_frame()?;
        let handshake = decode_peer_handshake(&frame)?;
        self.plugin.assert_peer_identity(&self.peer, &handshake)?;
        self.plugin.validate_mtls_framing(&handshake, &frame)?;
        self.plugin.record_peer(&self.peer, &handshake)
    }

    // see: cans/11-protocol.md#Wire-framing
    // TODO (L3) [protocol_mux]: the peer handshake encoder must emit the u32 LE length prefix this reader expects; slop: encoder omits u32 LE prefix, reader misframes handshake, peers never join
    fn read_handshake_frame(&mut self) -> Result<Vec<u8>, PluginError> {
        let mut prefix = [0u8; 4];
        self.read_exact(&mut prefix)?;
        let len = u32::from_le_bytes(prefix) as usize;
        if len == 0 || len > MAX_FRAME_PAYLOAD {
            return Err(PluginError::Halt(format!(
                "mesh handshake frame {len} bytes is outside the 1..={MAX_FRAME_PAYLOAD} frame range"
            )));
        }
        let mut frame = vec![0u8; len];
        self.read_exact(&mut frame)?;
        Ok(frame)
    }

    // see: cans/11-protocol.md#Wire-framing
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), PluginError> {
        let mut filled = 0;
        while filled < buf.len() {
            match self.stream.read(&mut buf[filled..]) {
                Ok(0) => {
                    return Err(PluginError::Halt(
                        "mesh handshake stream closed before the frame completed".into(),
                    ));
                }
                Ok(n) => filled += n,
                Err(e) => {
                    return Err(PluginError::Halt(format!(
                        "mesh handshake read failed: {e}"
                    )));
                }
            }
        }
        Ok(())
    }
}

// see: cans/plugins/mesh.md#Error-and-return-semantics-(mesh)
fn decode_peer_handshake(buf: &[u8]) -> Result<PeerSessionHandshake, PluginError> {
    // TODO (L3) [pm_14_mesh_rejects_foreign_trust_domain_peer]: decode the Channel 0 PeerSessionHandshake per cans/11-protocol.md#Wire-framing; slop: handshake decode stays Err Halt, identity never checked, foreign peer unchallenged
    Err(PluginError::Halt(format!(
        "peer handshake decode pending (L3): {} bytes",
        buf.len()
    )))
}
