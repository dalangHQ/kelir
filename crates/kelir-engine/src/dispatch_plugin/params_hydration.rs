// see: cans/03-sdk-architecture.md#Params-typed-views
use super::*;

// see: cans/03-sdk-architecture.md#Params-typed-views
// TODO (L3): hydrate the exec-param buffer in params_hydration.flow.rs; contract keeps the signature.
pub fn hydrate_params_ctx(ctx: &HandlerCtx, session: Option<&Session>) -> Option<HandlerCtx> {
    let _ = (ctx, session);
    None
}
