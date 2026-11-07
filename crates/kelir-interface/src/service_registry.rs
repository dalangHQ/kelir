// see: cans/02-plugin-model.md#Plugin-tiering
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

struct ServiceRef<S: ?Sized + Send + Sync + 'static> {
    inner: Arc<S>,
}

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

    pub fn register<S: ?Sized + Send + Sync + 'static>(&mut self, service: Arc<S>) {
        self.services.insert(
            TypeId::of::<ServiceRef<S>>(),
            Arc::new(ServiceRef { inner: service }),
        );
    }

    pub fn lookup<S: ?Sized + Send + Sync + 'static>(&self) -> Option<Arc<S>> {
        match self.services.get(&TypeId::of::<ServiceRef<S>>()).cloned() {
            Some(erased) => erased
                .downcast::<ServiceRef<S>>()
                .ok()
                .map(|wrapped| Arc::clone(&wrapped.inner)),
            None => None,
        }
    }
}

// TODO (L3): core boot wires each plugin's register_interceptors exports into the session-scoped registry.
