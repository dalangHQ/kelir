// see: cans/01-architecture.md#Host-state-root
// see: cans/06-storage-catalog.md#Entity-model
use std::time::{SystemTime, UNIX_EPOCH};

use crate::session_support::mint_uuid_v7;
use kelir_catalog::schema_catalog::DEFAULT_TENANT_ID;
use kelir_interface::hex_codec::encode_hex;

// see: cans/01-architecture.md#Host-state-root
pub fn session_dir(
    session_id: &[u8; 16],
) -> Result<std::path::PathBuf, kelir_interface::plugin_model::PluginError> {
    let home = std::env::var("HOME").map_err(|_| {
        kelir_interface::plugin_model::PluginError::Halt(
            "HOME is not set; cannot locate ~/.kelir session root".into(),
        )
    })?;
    Ok(std::path::PathBuf::from(home)
        .join(".kelir")
        .join("sessions")
        .join(encode_hex(session_id)))
}

// see: cans/01-architecture.md#Host-state-root
pub fn reap_ephemeral_session_dir(
    session_id: &[u8; 16],
) -> Result<(), kelir_interface::plugin_model::PluginError> {
    let dir = session_dir(session_id)?;
    match std::fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(kelir_interface::plugin_model::PluginError::Halt(format!(
            "reap session dir: {error}"
        ))),
    }
}

// see: cans/01-architecture.md#Host-state-root
pub(crate) fn ensure_workspace_root(
    session_id: &[u8; 16],
) -> Result<std::path::PathBuf, kelir_interface::plugin_model::PluginError> {
    let workspace_root = session_dir(session_id)?.join("workspace");
    std::fs::create_dir_all(&workspace_root)?;
    Ok(workspace_root)
}

// see: cans/06-storage-catalog.md#Entity-model
pub(crate) fn register_catalog_row(
    session_id: [u8; 16],
    incarnation_id: [u8; 16],
    tenant_id: &str,
) -> Result<(), kelir_interface::plugin_model::PluginError> {
    let row = kelir_catalog::schema_catalog::SessionRow {
        tenant_id: tenant_id.to_string(),
        session_id,
        incarnation_id: kelir_catalog::schema_catalog::IncarnationId::from_bytes(incarnation_id),
        lease_kind: kelir_catalog::schema_catalog::LeaseKind::Ephemeral,
        expires_at_unix: None,
        runner_pid: i64::from(std::process::id()),
        boot_id: encode_hex(&mint_uuid_v7()),
        start_time_ns: i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default(),
        )
        .unwrap_or(i64::MAX),
    };
    let catalog =
        kelir_catalog::CatalogDb::open(kelir_catalog::catalog_path().map_err(|error| {
            kelir_interface::plugin_model::PluginError::Halt(format!("{error}"))
        })?)
        .map_err(|error| {
            kelir_interface::plugin_model::PluginError::Halt(format!("catalog open: {error}"))
        })?;
    // see: cans/06-storage-catalog.md#Entity-model
    let already_registered = catalog
        .lookup_session(tenant_id, &session_id)
        .map_err(|error| {
            kelir_interface::plugin_model::PluginError::Halt(format!(
                "catalog session lookup: {error}"
            ))
        })?
        .is_some();
    if already_registered {
        tracing::warn!(
            "session {} already registered in the catalog; booting idempotently without a second insert",
            encode_hex(&session_id)
        );
        return Ok(());
    }
    catalog.register_session(&row).map_err(|error| {
        kelir_interface::plugin_model::PluginError::Halt(format!(
            "catalog session registration: {error}"
        ))
    })
}

// see: cans/06-storage-catalog.md#Entity-model
pub(crate) fn tenant_id(config: &kelir_interface::driver_interface::VmConfig) -> &str {
    config.tenant_id.as_deref().unwrap_or(DEFAULT_TENANT_ID)
}
