// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
#![forbid(unsafe_code)]

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

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
    entries: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<T: Send + Sync + 'static>(&mut self, value: Arc<T>) {
        self.entries.insert(TypeId::of::<T>(), value);
    }

    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.entries
            .get(&TypeId::of::<T>())
            .and_then(|entry| entry.downcast_ref::<T>())
    }

    pub fn merge(&mut self, other: &Self) {
        for (key, entry) in &other.entries {
            self.entries
                .entry(*key)
                .or_insert_with(|| Arc::clone(entry));
        }
    }
}
