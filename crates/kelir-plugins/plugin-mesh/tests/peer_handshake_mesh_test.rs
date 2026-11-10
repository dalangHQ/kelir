// see: cans/artifacts/test-plan/cases/protocol_mux/pm_14_mesh_rejects_foreign_trust_domain_peer.yaml#case_id
use std::io;

use kelir_interface::driver_interface::Stream;
use kelir_interface::plugin_model::{HandlerCtx, KelirPlugin};
use kelir_interface::vsock_mesh::{
    PeerSessionHandshake, SpiffeId, SpiffeSvid, VsockPeer, VsockServiceHandler,
};
use plugin_mesh::{MeshConfig, MeshPlugin, PeerIdentity, TraceParentPropagation};

struct ForeignHandshakeStream {
    data: Vec<u8>,
    pos: usize,
}

impl ForeignHandshakeStream {
    fn new(frame: Vec<u8>) -> Self {
        Self {
            data: frame,
            pos: 0,
        }
    }
}

impl Stream for ForeignHandshakeStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        if self.pos >= self.data.len() {
            return Ok(0);
        }
        let count = buf.len().min(self.data.len() - self.pos);
        buf[..count].copy_from_slice(&self.data[self.pos..self.pos + count]);
        self.pos += count;
        Ok(count)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
}

fn foreign_domain_frame() -> Result<Vec<u8>, String> {
    let handshake = PeerSessionHandshake {
        svid: SpiffeSvid {
            spiffe_id: SpiffeId {
                trust_domain: "foreign.test".to_string(),
                workload_prefix: "sa/worker".to_string(),
            },
            credential: Vec::new(),
            expires_at_ns: 0,
        },
        call_hops: 1,
        traceparent: None,
    };
    let mut body = Vec::new();
    ciborium::into_writer(&handshake, &mut body).map_err(|e| e.to_string())?;
    let mut frame = Vec::with_capacity(4 + body.len());
    frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
    frame.extend_from_slice(&body);
    Ok(frame)
}

// see: cans/artifacts/test-plan/cases/protocol_mux/pm_14_mesh_rejects_foreign_trust_domain_peer.yaml#case_id
#[tokio::test]
#[ignore]
async fn pm_14_mesh_rejects_foreign_trust_domain_peer() -> Result<(), String> {
    let plugin = MeshPlugin::new(MeshConfig {
        peer_identity: PeerIdentity::Spiffe {
            trust_domain: "example.test".to_string(),
            workload_prefix: "sa/self".to_string(),
        },
        mtls: false,
        traceparent: TraceParentPropagation::InjectAndValidate,
        listeners: Vec::new(),
    });
    let peer = VsockPeer {
        cid: 7,
        port: 5252,
        spiffe_id: None,
    };
    let stream: Box<dyn Stream> = Box::new(ForeignHandshakeStream::new(foreign_domain_frame()?));
    let ctx = HandlerCtx::new([0u8; 16], 42, (), Vec::new());
    plugin
        .on_connect(&peer, stream, &ctx)
        .await
        .map_err(|e| e.to_string())?;
    let mut rejection = None;
    for _ in 0..4_000_000 {
        let rejections = plugin
            .context()
            .peer_rejections()
            .map_err(|e| e.to_string())?;
        if let Some(first) = rejections.first() {
            rejection = Some(first.clone());
            break;
        }
        std::thread::yield_now();
    }
    let Some(rejection) = rejection else {
        return Err("foreign-trust-domain peer handshake was never recorded".to_string());
    };
    assert!(
        rejection.reason.contains("foreign.test"),
        "rejection reason does not name the foreign trust domain: {rejection:?}"
    );
    assert!(
        rejection.reason.contains("ECONNREFUSED"),
        "rejection reason is not an ECONNREFUSED rejection: {rejection:?}"
    );
    Ok(())
}
