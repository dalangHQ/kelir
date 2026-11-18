// see: cans/plugins/taint.md#Plugin-taint
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use kelir_interface::plugin_model::{
    FsInterposer, HandlerCtx, KelirPlugin, NetInterposer, PipelineBinder, PluginDeps, PluginError,
    TrapInterposer,
};
use kelir_interface::protocol_wire::{FilterAction, TrapEvent, Verdict};

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

// see: cans/02-plugin-model.md#KelirPlugin-trait
#[async_trait]
impl TrapInterposer for TaintPlugin {
    async fn on_trap(&self, event: &TrapEvent, _ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        let _ = event;
        // TODO (L3) [governance_tools]: propagate FUSE-read taint labels into execve argv/envp spans; slop: tainted reads execute unflagged; argv/envp carry no taint labels.
        Ok(Verdict::Pass)
    }
}

impl FsInterposer for TaintPlugin {
    fn on_fs_read(&self, path: &Path, buffer: &[u8]) -> Result<Vec<u8>, PluginError> {
        let _ = path;
        // TODO (L3) [governance_tools]: label the read span when the source carries untrusted ingress; slop: reads from untrusted ingress run unlabeled; downstream spans miss the taint.
        Ok(buffer.to_vec())
    }

    fn on_fs_write(
        &self,
        path: &Path,
        data: &mut [u8],
        pid: u32,
        ctx: &HandlerCtx,
    ) -> Result<FilterAction, PluginError> {
        let _ = (path, data, pid, ctx);
        // TODO (L3) [governance_tools]: propagate taint labels into written bytes before the verdict; slop: tainted writes reach the verdict unlabeled; the gate cannot see the taint.
        Ok(FilterAction::Forward)
    }
}

impl NetInterposer for TaintPlugin {
    fn on_net_outbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        let _ = frame;
        // TODO (L3) [governance_tools]: hold egress spans that overlap a taint label for escalation; slop: tainted egress leaves without escalation; exfiltration spans pass unheld.
        Ok(FilterAction::Forward)
    }
}
