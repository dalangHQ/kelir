// see: cans/05-storage-plane.md#Storage-architecture
use kelir_interface::plugin_model::{HandlerCtx, PluginError};
use kelir_interface::storage::{FuseOps, FuseRequest, FuseResponse};
use std::sync::Arc;

pub struct FuseDispatcher {
    ops: Arc<dyn FuseOps>,
}

impl FuseDispatcher {
    pub fn new(ops: Arc<dyn FuseOps>) -> Self {
        Self { ops }
    }

    pub fn dispatch(
        &self,
        request: &mut FuseRequest,
        ctx: &HandlerCtx,
    ) -> Result<FuseResponse, PluginError> {
        self.ops.dispatch(request, ctx)
    }
}
