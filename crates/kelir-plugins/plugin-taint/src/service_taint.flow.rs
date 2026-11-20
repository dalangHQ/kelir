// see: cans/plugins/taint.md#Invariants-and-policy-(taint)
use std::collections::HashMap;
use std::sync::Mutex;

// see: cans/02-plugin-model.md#Facade-blindness
pub use kelir_interface::byte_range::{ByteRange, TaintSet};

use kelir_interface::plugin_model::PluginError;

// see: cans/plugins/taint.md#Service-exports-(taint)
pub trait TaintTrackingService: Send + Sync {
    fn mark(&self, source: &str, range: ByteRange) -> Result<(), PluginError>;
    fn propagate(&self, source: &str, into_len: usize) -> Result<Vec<ByteRange>, PluginError>;
    fn overlaps(&self, source: &str, range: ByteRange) -> Result<bool, PluginError>;
}

// see: cans/plugins/taint.md#Service-exports-(taint)
#[derive(Debug, Default)]
pub struct TaintTracker {
    labels: Mutex<HashMap<String, TaintSet>>,
}

impl TaintTracker {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock_labels(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<String, TaintSet>>, PluginError> {
        self.labels
            .lock()
            .map_err(|_| PluginError::Halt("taint label table poisoned".into()))
    }
}

impl TaintTrackingService for TaintTracker {
    // see: cans/plugins/taint.md#Invariants-and-policy-(taint)
    fn mark(&self, source: &str, range: ByteRange) -> Result<(), PluginError> {
        self.lock_labels()?
            .entry(source.to_string())
            .or_default()
            .mark(range);
        Ok(())
    }

    // see: cans/plugins/taint.md#Invariants-and-policy-(taint)
    fn propagate(&self, source: &str, into_len: usize) -> Result<Vec<ByteRange>, PluginError> {
        Ok(self
            .lock_labels()?
            .get(source)
            .map(|set| set.propagate(into_len))
            .unwrap_or_default())
    }

    // see: cans/plugins/taint.md#Invariants-and-policy-(taint)
    fn overlaps(&self, source: &str, range: ByteRange) -> Result<bool, PluginError> {
        Ok(self
            .lock_labels()?
            .get(source)
            .is_some_and(|set| set.overlaps(range)))
    }
}
