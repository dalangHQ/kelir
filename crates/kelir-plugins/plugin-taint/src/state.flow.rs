// see: cans/plugins/taint.md#Plugin-taint
use std::sync::Arc;

use kelir_interface::plugin_model::{
    FsInterposer, KelirPlugin, NetInterposer, PipelineBinder, PluginDeps, PluginError,
    TrapInterposer,
};

use crate::service_taint::{ByteRange, TaintTracker, TaintTrackingService};

// see: cans/plugins/taint.md#Crate-dependencies-(taint)
#[derive(Clone, PluginDeps)]
pub struct TaintPlugin {
    tracker: Arc<TaintTracker>,
}

impl TaintPlugin {
    pub fn new() -> Self {
        Self {
            tracker: Arc::new(TaintTracker::new()),
        }
    }

    pub fn tracker(&self) -> Arc<TaintTracker> {
        Arc::clone(&self.tracker)
    }
}

impl Default for TaintPlugin {
    fn default() -> Self {
        Self::new()
    }
}

// see: cans/plugins/taint.md#Service-exports-(taint)
#[derive(Clone)]
pub struct TaintTrackingCtx {
    tracker: Arc<TaintTracker>,
}

impl TaintTrackingCtx {
    pub fn tracker(&self) -> Arc<TaintTracker> {
        Arc::clone(&self.tracker)
    }

    // see: cans/plugins/taint.md#Invariants-and-policy-(taint)
    pub fn mark(&self, source: &str, range: ByteRange) -> Result<(), PluginError> {
        self.tracker.mark(source, range)
    }

    // see: cans/plugins/taint.md#Invariants-and-policy-(taint)
    pub fn propagate(&self, source: &str, into_len: usize) -> Result<Vec<ByteRange>, PluginError> {
        self.tracker.propagate(source, into_len)
    }

    // see: cans/plugins/taint.md#Invariants-and-policy-(taint)
    pub fn overlaps(&self, source: &str, range: ByteRange) -> Result<bool, PluginError> {
        self.tracker.overlaps(source, range)
    }
}

impl KelirPlugin for TaintPlugin {
    type Context = TaintTrackingCtx;
    fn context(&self) -> Self::Context {
        TaintTrackingCtx {
            tracker: Arc::clone(&self.tracker),
        }
    }
    fn name(&self) -> &'static str {
        "taint"
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        // see: cans/plugins/taint.md#Invariants-and-policy-(taint)
        let trap_stage: Arc<dyn TrapInterposer> = Arc::new(self.clone());
        pipeline.bind_trap_scoped(trap_stage, self.default_scope());
        let fs_stage: Arc<dyn FsInterposer> = Arc::new(self.clone());
        pipeline.bind_fs_scoped(fs_stage, self.default_scope());
        let net_stage: Arc<dyn NetInterposer> = Arc::new(self.clone());
        pipeline.bind_net_scoped(net_stage, self.default_scope());
    }
}
