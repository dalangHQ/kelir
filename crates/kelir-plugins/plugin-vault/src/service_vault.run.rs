// see: cans/plugins/vault.md#In-ring-carrier-secret-injection
use std::path::Path;

use crate::service_vault::VaultError;

// see: cans/plugins/vault.md#In-ring-carrier-secret-injection
pub fn read_mtls_files(
    cert_path: &Path,
    key_path: &Path,
) -> Result<(Vec<u8>, Vec<u8>), VaultError> {
    // TODO (L3) [governance_vault]: read the mTLS cert and key files and return their bytes.; slop: mTLS material is never read, the vault serves no client certificate, authenticated vault access silently fails.
    let _ = (cert_path, key_path);
    Err(VaultError::Enodata(
        "TODO (L3): mTLS material read pending (L3)".into(),
    ))
}
