// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
#![forbid(unsafe_code)]

use std::any::TypeId;
use std::collections::HashMap;

use crate::capability_registry_flow::CapabilityEntry;

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
pub const CANONICAL_CAPABILITY_KEYS: [&str; 8] = [
    "ledger",
    "db-proxy",
    "tool-guard",
    "transaction",
    "container",
    "mesh",
    "vault",
    "replay",
];

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
#[derive(Clone, Default)]
pub struct CapabilityRegistry {
    pub(crate) entries: HashMap<TypeId, CapabilityEntry>,
}
