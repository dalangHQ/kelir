// see: cans/03-sdk-architecture.md#Params-typed-views
use super::*;

// see: cans/03-sdk-architecture.md#Params-typed-views
pub fn hydrate_params_ctx(ctx: &HandlerCtx, session: Option<&Session>) -> Option<HandlerCtx> {
    let exec_ctx = ctx.exec_ctx.as_ref()?;
    let session = session?;
    let buffer = session.exec_params(exec_ctx.exec_id, exec_ctx.stream_id)?;
    Some(ctx.clone().with_params_buffer(buffer))
}
