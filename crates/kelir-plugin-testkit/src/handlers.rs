// see: cans/02-plugin-model.md#Plugin-testkit
use kelir_interface::driver::WatchdogAction;
use kelir_interface::plugin::{HandlerCtx, LifecycleCtx};
use kelir_interface::protocol::TrapEvent;

pub fn handler_ctx(session_id: [u8; 16], pid: u32) -> HandlerCtx<(), ()> {
    HandlerCtx::restricted(session_id, pid)
}

pub fn handler_ctx_for_trap(session_id: [u8; 16], event: &TrapEvent) -> HandlerCtx<(), ()> {
    HandlerCtx::restricted(session_id, event.pid).with_trap(event.clone())
}

pub fn lifecycle_ctx() -> LifecycleCtx {
    LifecycleCtx::new()
}

pub fn lifecycle_ctx_with_watchdog(action: WatchdogAction) -> LifecycleCtx {
    let mut ctx = LifecycleCtx::new();
    ctx.watchdog = Some(action);
    ctx
}
