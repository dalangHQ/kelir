// see: cans/plugins/vault.md#Metadata-service-(vault)
use async_trait::async_trait;
use kelir_interface::plugin_model::{
    HandlerCtx, LifecycleCtx, LifecycleParticipant, PluginError, TrapInterposer,
};
use kelir_interface::protocol_wire::{TrapEvent, Verdict};

use crate::metadata_endpoint::VaultPlugin;

// see: cans/plugins/vault.md#Plugin-vault
#[async_trait]
impl TrapInterposer for VaultPlugin {
    async fn on_trap(&self, _event: &TrapEvent, _ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        // TODO (L3) [gv_12_vault_placeholder_profile_resolution]: resolve the placeholder through the metadata service at exec; slop: vault:// placeholder resolves directly to the plaintext secret in the profile AST.
        // TODO (L3) [governance_vault]: bind the CredSubTable to the live metadata/vsock stream so every credential handoff swaps the real secret for the format-preserving fake and every inbound L7 response is scrubbed; slop: the CredSubTable never binds to a live stream, real secrets flow to the guest and echoed credentials pass through responses unscrubbed.
        Err(PluginError::Halt(
            "TODO (L3): resolve the vault:// placeholder through the metadata service at exec"
                .into(),
        ))
    }
}

// see: cans/plugins/vault.md#Revocation
#[async_trait]
impl LifecycleParticipant for VaultPlugin {
    // see: cans/plugins/vault.md#Revocation
    async fn on_vm_init(&self, _ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        // TODO (L3) [governance_vault]: bind the session id on the credential broker; stub binds nothing.; slop: the session id is never bound, credentials leak across sessions, one session reads another session's secrets.
        Err(PluginError::Halt(
            "TODO (L3): bind the session id on the credential broker".into(),
        ))
    }

    // see: cans/plugins/vault.md#Revocation
    async fn on_vm_shutdown(&self, _ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        // TODO (L3) [governance_vault]: revoke all session tokens; stub revokes nothing.; slop: session tokens are never revoked, terminated sessions keep live credentials, revoked access silently persists.
        Err(PluginError::Halt(
            "TODO (L3): revoke all session tokens".into(),
        ))
    }
}
