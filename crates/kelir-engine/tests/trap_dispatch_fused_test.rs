// see: cans/artifacts/test-plan/cases/plugin_engine/pe_07_aot_hook_fusion_single_trampoline.yaml#case_id
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;

use kelir_engine::dispatch_plugin::PipelineEngine;
use kelir_interface::errno_types::Errno;
use kelir_interface::fusion_plugin::{
    FusedTrampolineCallback, FusedTrampolineDescriptor, HookEntry, HookStage, RingChain,
    TrampolineRing,
};
use kelir_interface::plugin_model::{HandlerCtx, PipelineBinder, PluginError, TrapInterposer};
use kelir_interface::protocol_wire::{TrapEvent, Verdict};

fn trap_ctx() -> HandlerCtx<(), ()> {
    HandlerCtx::new([0u8; 16], 42, (), Vec::new())
}

fn trap_event() -> TrapEvent {
    TrapEvent {
        binary_path: "/workspace/tool".to_string(),
        ..Default::default()
    }
}

fn hook_entry(ordinal: u32) -> HookEntry {
    HookEntry {
        ordinal,
        stage: HookStage::Pre,
        plugin_name: Some(format!("hook-{ordinal}")),
        filter: None,
        lifecycle_tag: None,
        custom_action_name: None,
        vsock_kind: None,
        run: None,
    }
}

struct CountingTrap {
    called: Arc<AtomicBool>,
}

#[async_trait]
impl TrapInterposer for CountingTrap {
    async fn on_trap(
        &self,
        _event: &TrapEvent,
        _ctx: &HandlerCtx,
    ) -> Result<Verdict, kelir_interface::plugin_model::PluginError> {
        self.called.store(true, Ordering::SeqCst);
        Ok(Verdict::Pass)
    }
}

fn fused_trap_descriptor() -> FusedTrampolineDescriptor {
    FusedTrampolineDescriptor {
        rings: vec![RingChain {
            ring: TrampolineRing::Trap,
            hooks: vec![hook_entry(0), hook_entry(1), hook_entry(2)],
        }],
    }
}

fn bind_three_counting_traps(engine: &mut PipelineEngine) -> Vec<Arc<AtomicBool>> {
    let flags = vec![
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicBool::new(false)),
    ];
    for flag in &flags {
        engine.bind_trap(Arc::new(CountingTrap {
            called: Arc::clone(flag),
        }));
    }
    flags
}

struct FusedCallback {
    verdict: Verdict,
    called: Arc<AtomicBool>,
}

impl FusedTrampolineCallback for FusedCallback {
    fn run(&self, _ring: TrampolineRing, _ordinal: u32) -> Result<Verdict, PluginError> {
        self.called.store(true, Ordering::SeqCst);
        Ok(self.verdict.clone())
    }
}

fn fused_callback(verdict: Verdict) -> (Arc<FusedCallback>, Arc<AtomicBool>) {
    let called = Arc::new(AtomicBool::new(false));
    let callback = Arc::new(FusedCallback {
        verdict,
        called: Arc::clone(&called),
    });
    (callback, called)
}

fn fused_drop() -> Verdict {
    Verdict::Drop {
        errno: Some(Errno::Eperm),
        reason: "denied".to_string(),
    }
}

// see: cans/artifacts/test-plan/cases/plugin_engine/pe_07_aot_hook_fusion_single_trampoline.yaml#case_id
#[tokio::test]
async fn fused_callback_drop_short_circuits_trap_dispatch() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    let (callback, called) = fused_callback(fused_drop());
    engine.register_fused_callback(TrampolineRing::Trap, 0, callback);
    let native_called = Arc::new(AtomicBool::new(false));
    engine.bind_trap(Arc::new(CountingTrap {
        called: Arc::clone(&native_called),
    }));
    let verdict = engine
        .dispatch_trap(&trap_event(), &trap_ctx())
        .await
        .map_err(|e| e.to_string())?;
    match verdict {
        Verdict::Drop {
            errno: Some(Errno::Eperm),
            ..
        } => {}
        other => return Err(format!("fused drop did not win: {other:?}")),
    }
    if !called.load(Ordering::SeqCst) {
        return Err("fused callback never ran".to_string());
    }
    if native_called.load(Ordering::SeqCst) {
        return Err("native interceptor ran after fused drop".to_string());
    }
    assert_eq!(engine.foreign_queue().depth(), 0);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/plugin_engine/pe_07_aot_hook_fusion_single_trampoline.yaml#case_id
#[tokio::test]
async fn fused_callbacks_run_in_ordinal_order_first_non_pass_wins() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    let (pass, pass_called) = fused_callback(Verdict::Pass);
    let (drop, _) = fused_callback(fused_drop());
    engine.register_fused_callback(TrampolineRing::Trap, 1, pass);
    engine.register_fused_callback(TrampolineRing::Trap, 0, drop);
    let verdict = engine
        .dispatch_trap(&trap_event(), &trap_ctx())
        .await
        .map_err(|e| e.to_string())?;
    if !matches!(verdict, Verdict::Drop { .. }) {
        return Err(format!("fused drop did not win: {verdict:?}"));
    }
    if pass_called.load(Ordering::SeqCst) {
        return Err("ordinal 1 callback ran before ordinal 0 drop".to_string());
    }
    assert_eq!(engine.foreign_queue().depth(), 0);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/plugin_engine/pe_07_aot_hook_fusion_single_trampoline.yaml#case_id
#[tokio::test]
async fn fused_callback_pass_falls_through_to_native_trap() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    let (callback, called) = fused_callback(Verdict::Pass);
    engine.register_fused_callback(TrampolineRing::Trap, 0, callback);
    let native_called = Arc::new(AtomicBool::new(false));
    engine.bind_trap(Arc::new(CountingTrap {
        called: Arc::clone(&native_called),
    }));
    let verdict = engine
        .dispatch_trap(&trap_event(), &trap_ctx())
        .await
        .map_err(|e| e.to_string())?;
    if !matches!(verdict, Verdict::Pass) {
        return Err(format!("fused pass did not pass: {verdict:?}"));
    }
    if !called.load(Ordering::SeqCst) || !native_called.load(Ordering::SeqCst) {
        return Err("fused pass or native trap never ran".to_string());
    }
    assert_eq!(engine.foreign_queue().depth(), 0);
    Ok(())
}

#[tokio::test]
async fn fused_trampoline_collapses_trap_ring_to_single_crossing() -> Result<(), String> {
    let mut engine = PipelineEngine::new()
        .with_trampoline(fused_trap_descriptor())
        .with_ffi_rate_limit(2);
    let flags = bind_three_counting_traps(&mut engine);
    let event = trap_event();
    let ctx = trap_ctx();
    let verdict = engine
        .dispatch_trap(&event, &ctx)
        .await
        .map_err(|e| e.to_string())?;
    match verdict {
        Verdict::Pass => {}
        other => return Err(format!("fused trap ring did not pass: {other:?}")),
    }
    for flag in &flags {
        assert!(flag.load(Ordering::SeqCst));
    }
    assert_eq!(engine.foreign_queue().depth(), 0);
    Ok(())
}

#[tokio::test]
async fn unfused_trap_ring_crosses_ffi_per_hook() -> Result<(), String> {
    let mut engine = PipelineEngine::new().with_ffi_rate_limit(2);
    let flags = bind_three_counting_traps(&mut engine);
    let event = trap_event();
    let ctx = trap_ctx();
    match engine.dispatch_trap(&event, &ctx).await {
        Err(PluginError::Halt(message)) => {
            assert!(message.contains("FFI rate limit exceeded"))
        }
        other => {
            return Err(format!(
                "per-hook crossings did not hit the rate gate: {other:?}"
            ));
        }
    }
    assert!(flags[0].load(Ordering::SeqCst));
    assert!(flags[1].load(Ordering::SeqCst));
    assert!(!flags[2].load(Ordering::SeqCst));
    assert_eq!(engine.foreign_queue().depth(), 0);
    Ok(())
}
