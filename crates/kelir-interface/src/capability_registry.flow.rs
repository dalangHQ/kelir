// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
use std::any::{Any, TypeId};
use std::sync::Arc;

use crate::capability_registry::CapabilityRegistry;

pub(crate) type CapabilityEntry = Arc<dyn Any + Send + Sync>;

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
