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
