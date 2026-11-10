// see: cans/plugins/vault.md#Out-of-band-credential-brokerage
use std::sync::Arc;

use kelir_interface::credential_vault::{CredentialDescriptor, CredentialKind};
use kelir_interface::plugin_model::PluginError;
use kelir_interface::vsock_mesh::VsockPeer;
use kelir_plugin_testkit::LoopbackStream;
use kelir_plugin_testkit::block_on;
use kelir_plugin_testkit::handlers_testkit::handler_ctx;
use plugin_vault::service_vault::{
    CredentialBroker, CredentialBrokerService, TokenRequest, VaultError,
};
use plugin_vault::{MetadataEndpoint, VaultConfig, VaultPlugin};

fn descriptor(name: &str, audience: &str) -> CredentialDescriptor {
    CredentialDescriptor {
        name: name.into(),
        kind: CredentialKind::BearerToken,
        audience: audience.into(),
        placeholder: format!("vault://{name}"),
    }
}

fn bound_broker(descriptors: Vec<CredentialDescriptor>) -> Result<Arc<CredentialBroker>, String> {
    let broker = Arc::new(CredentialBroker::new(descriptors));
    broker
        .bind_session("session-1")
        .map_err(|err| format!("{err:?}"))?;
    Ok(broker)
}

// see: cans/artifacts/test-plan/cases/governance_vault/gv_01_metadata_service_scoped_token.yaml#case_id
#[test]
fn gv_01_metadata_service_scoped_token() -> Result<(), String> {
    let broker = bound_broker(vec![descriptor("openai_key", "api.openai.com")])?;
    let endpoint = MetadataEndpoint::new(VaultConfig::default(), broker);
    let response = block_on(endpoint.serve(b"GET /kelir/credentials/openai_key HTTP/1.1\r\n\r\n"));
    let text = String::from_utf8(response).map_err(|err| format!("{err:?}"))?;
    assert!(
        text.starts_with("HTTP/1.1 200 OK"),
        "unexpected status: {text}"
    );
    assert!(
        text.contains("\"audience\":\"api.openai.com\""),
        "audience missing: {text}"
    );
    assert!(
        text.contains("\"ttl_seconds\":60"),
        "default TTL missing: {text}"
    );
    assert!(text.contains("\"token\":\""), "token missing: {text}");
    assert!(
        !text.contains("\"token\":\"\""),
        "issued token is empty: {text}"
    );
    Ok(())
}

// see: cans/artifacts/test-plan/cases/governance_vault/gv_04_token_ttl_max_ceiling_einval.yaml#case_id
#[test]
fn gv_04_token_ttl_max_ceiling_einval() -> Result<(), String> {
    let broker = bound_broker(vec![descriptor("key", "api.openai.com")])?;
    match block_on(broker.issue(&TokenRequest {
        name: "key".into(),
        ttl_seconds: 1000,
    })) {
        Err(VaultError::Einval(message)) if message.contains("1000") => (),
        Err(err) => {
            return Err(format!("ttl 1000 must fail with Einval, got {err:?}"));
        }
        Ok(token) => {
            return Err(format!(
                "ttl 1000 was issued with ttl {}; the 900s ceiling must reject it",
                token.ttl_seconds
            ));
        }
    }
    let issued = block_on(broker.issue(&TokenRequest {
        name: "key".into(),
        ttl_seconds: 900,
    }))
    .map_err(|err| format!("{err:?}"))?;
    assert_eq!(issued.ttl_seconds, 900);
    assert_eq!(issued.audience, "api.openai.com");
    Ok(())
}

// see: cans/artifacts/test-plan/cases/governance_vault/gv_06_rollback_credential_revocation.yaml#case_id
#[test]
fn gv_06_rollback_credential_revocation() -> Result<(), String> {
    let broker = bound_broker(vec![descriptor("tok", "api.openai.com")])?;
    let issued = block_on(broker.issue(&TokenRequest {
        name: "tok".into(),
        ttl_seconds: 60,
    }))
    .map_err(|err| format!("{err:?}"))?;
    let token = issued.token.clone();
    let valid_before =
        block_on(broker.validate_token("session-1", &token)).map_err(|err| format!("{err:?}"))?;
    assert!(valid_before);
    let revoked = block_on(broker.revoke_all("session-1")).map_err(|err| format!("{err:?}"))?;
    assert_eq!(revoked, 1);
    let valid_after =
        block_on(broker.validate_token("session-1", &token)).map_err(|err| format!("{err:?}"))?;
    assert!(!valid_after);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/governance_vault/gv_07_unregistered_credential_enodata.yaml#case_id
#[test]
fn gv_07_unregistered_credential_enodata() -> Result<(), String> {
    let broker = bound_broker(vec![descriptor("openai_key", "api.openai.com")])?;
    let endpoint = MetadataEndpoint::new(VaultConfig::default(), broker);
    let response =
        block_on(endpoint.serve(b"GET /kelir/credentials/non_existent_key HTTP/1.1\r\n\r\n"));
    let text = String::from_utf8(response).map_err(|err| format!("{err:?}"))?;
    assert!(
        text.starts_with("HTTP/1.1 404 Not Found"),
        "unexpected status: {text}"
    );
    assert!(
        text.contains("\"errno\":\"ENODATA\""),
        "ENODATA missing: {text}"
    );
    Ok(())
}

// see: cans/artifacts/test-plan/cases/governance_vault/gv_09_metadata_non_vsock_peer_econnrefused.yaml#case_id
#[test]
fn gv_09_metadata_non_vsock_peer_econnrefused() -> Result<(), String> {
    use kelir_interface::vsock_mesh::VsockServiceHandler;
    let plugin = VaultPlugin::new().with_session_guest_cid(3);
    let ctx = handler_ctx([0u8; 16], 1);
    let foreign = VsockPeer {
        cid: 7,
        port: 80,
        spiffe_id: None,
    };
    match block_on(plugin.on_connect(&foreign, Box::new(LoopbackStream::new(1)), &ctx)) {
        Err(PluginError::Halt(message)) if message.starts_with("ECONNREFUSED:") => (),
        Err(err) => {
            return Err(format!(
                "non-session peer must fail with ECONNREFUSED, got {err:?}"
            ));
        }
        Ok(()) => {
            return Err(
                "non-session vsock peer was served; the metadata service must refuse it".into(),
            );
        }
    }
    let session_peer = VsockPeer {
        cid: 3,
        port: 80,
        spiffe_id: None,
    };
    block_on(plugin.on_connect(&session_peer, Box::new(LoopbackStream::new(1)), &ctx))
        .map_err(|err| format!("{err:?}"))?;
    Ok(())
}

// see: cans/artifacts/test-plan/cases/governance_vault/gv_03_audience_locking_validation.yaml#case_id
#[test]
#[ignore]
fn gv_03_audience_locking_validation() -> Result<(), String> {
    let broker = bound_broker(vec![descriptor("openai_key", "api.openai.com")])?;
    let endpoint = MetadataEndpoint::new(VaultConfig::default(), broker);
    let response =
        block_on(endpoint.serve(
            b"GET /kelir/credentials/openai_key?audience=api.anthropic.com HTTP/1.1\r\n\r\n",
        ));
    let text = String::from_utf8(response).map_err(|err| format!("{err:?}"))?;
    assert!(
        text.starts_with("HTTP/1.1 403 Forbidden"),
        "audience mismatch must reject with 403, got: {text}"
    );
    assert!(
        text.contains("\"errno\":\"EACCES\""),
        "audience mismatch must carry EACCES, got: {text}"
    );
    assert!(
        !text.contains("\"token\":"),
        "audience mismatch must leak zero token bytes, got: {text}"
    );
    Ok(())
}

// see: cans/artifacts/test-plan/cases/governance_vault/gv_11_mtls_client_cert_bundle_issuance.yaml#case_id
#[test]
#[ignore]
fn gv_11_mtls_client_cert_bundle_issuance() -> Result<(), String> {
    let broker = bound_broker(vec![CredentialDescriptor {
        name: "mtls_corp_cert".into(),
        kind: CredentialKind::MtlsBundle,
        audience: "internal.corp".into(),
        placeholder: "vault://mtls_corp_cert".into(),
    }])?;
    let endpoint = MetadataEndpoint::new(VaultConfig::default(), broker);
    let response =
        block_on(endpoint.serve(b"GET /kelir/credentials/mtls_corp_cert HTTP/1.1\r\n\r\n"));
    let text = String::from_utf8(response).map_err(|err| format!("{err:?}"))?;
    assert!(
        text.starts_with("HTTP/1.1 200 OK"),
        "mTLS bundle issuance must succeed, got: {text}"
    );
    assert!(
        text.contains("-----BEGIN CERTIFICATE-----"),
        "MtlsBundle issuance must return a PEM cert bundle, got: {text}"
    );
    let ttl_marker = "\"ttl_seconds\":";
    let Some(at) = text.find(ttl_marker) else {
        return Err(format!("issued bundle carries no ttl_seconds, got: {text}"));
    };
    let digits: String = text[at + ttl_marker.len()..]
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect();
    let ttl: u32 = digits
        .parse()
        .map_err(|err| format!("bad ttl {digits:?}: {err:?}"))?;
    assert!(
        ttl <= 900,
        "mTLS bundle validity {ttl}s exceeds the 900s ceiling, got: {text}"
    );
    Ok(())
}
