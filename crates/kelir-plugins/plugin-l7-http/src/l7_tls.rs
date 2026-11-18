// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
use crate::l7_parser::L7ParseError;

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
pub trait TlsDecryptor: Send + Sync {
    fn decrypt(&self, record: &[u8]) -> Result<Vec<u8>, L7ParseError>;
}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
#[derive(Debug, Clone, Default)]
pub struct HostKeyringDecryptor {
    pub key_id: Option<String>,
}

// see: cans/plugins/l7-http.md#Invariants-and-policy-(l7-http)
impl TlsDecryptor for HostKeyringDecryptor {
    fn decrypt(&self, _record: &[u8]) -> Result<Vec<u8>, L7ParseError> {
        // TODO (L3) [wire_plane]: fetch the session key from the host keyring and decrypt the TLS record; no keyring read exists yet.; slop: TLS records pass through undecrypted, L7 rules blind
        Err(L7ParseError::TlsUnavailable(
            "host keyring tls decrypt is TODO (L3)".into(),
        ))
    }
}
