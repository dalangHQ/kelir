// see: cans/02-plugin-model.md#Plugin-tiering
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

pub struct ServiceRegistry {
    services: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
        }
    }

    pub fn register<S: Send + Sync + 'static>(&mut self, service: Arc<S>) {
        self.services.insert(TypeId::of::<S>(), service);
    }

    pub fn lookup<S: Send + Sync + 'static>(&self) -> Option<Arc<S>> {
        self.services
            .get(&TypeId::of::<S>())?
            .clone()
            .downcast::<S>()
            .ok()
    }
}

// TODO (L3): resolve services by trait object (dyn TransactionService) and invoke across plugins; concrete-type lookup only at L2.
