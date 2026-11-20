// see: cans/plugins/mesh.md#Plugin-mesh
use async_trait::async_trait;
use kelir_interface::plugin_model::{LifecycleCtx, LifecycleParticipant, PluginError};
use kelir_interface::vsock_mesh::SpiffeSvid;

use crate::mesh_state::{MeshPlugin, MeshState};

// TODO (L3) [plugin_engine]: remove this allow when the stubbed L3 callers land and read this item again.; slop: the allow hides a dead item forever, the stubbed callers never land, mesh wiring stays unwired while the gate reports clean.
#[allow(dead_code)]
impl MeshState {
    fn set_svid(&self, svid: SpiffeSvid) -> Result<(), PluginError> {
        let _ = svid;
        // TODO (L3) [plugin_engine]: store the session SVID on the mesh state.; slop: the session SVID is never stored, mesh peers cannot authenticate the session, attested identity is silently absent.
        Ok(())
    }
}

// TODO (L3) [plugin_engine]: remove this allow when the stubbed L3 callers land and read this item again.; slop: the allow hides a dead item forever, the stubbed callers never land, mesh wiring stays unwired while the gate reports clean.
#[allow(dead_code)]
impl MeshPlugin {
    fn issue_session_svid(&self) -> Result<SpiffeSvid, PluginError> {
        let _ = self;
        // L3-GAP: [protocol_mux]: obtain the host-issued SVID credential bytes and the session-bounded expiry; slop: SVID self-minted locally, host issuance bypassed, expiry self-asserted
        // TODO (L3) [plugin_engine]: obtain the host-issued SVID credential bytes and the session-bounded expiry.; slop: no SVID credential is ever obtained, the session runs without attested identity, peers trust an unauthenticated session.
        Err(PluginError::Halt(
            "TODO (L3): mesh session SVID issuance pending".into(),
        ))
    }
}

// see: cans/plugins/mesh.md#Peer-identity
#[async_trait]
impl LifecycleParticipant for MeshPlugin {
    async fn on_vm_ready(&self, _ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        // TODO (L3) [plugin_engine]: mint the session SVID and store it on the mesh state.; slop: the session SVID is never minted, mesh state carries no credential, the session joins the mesh unauthenticated.
        Ok(())
    }
}
