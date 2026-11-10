// see: cans/artifacts/test-plan/cases/plugin_engine/pe_04_interceptor_pipeline_monotonic_verdict.yaml#case_id
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use kelir_core::Session;
use kelir_core::dispatch_plugin::PipelineEngine;
use kelir_core::planes::checkpoint::{CheckpointPlane, PidLineage};
use kelir_interface::driver_interface::{SnapshotDescriptor, default_vm_config};
use kelir_interface::fusion_plugin::{
    FusedTrampolineDescriptor, HookEntry, HookStage, RingChain, TrampolineRing, TrampolineTable,
};
use kelir_interface::plugin_model::{
    ConsoleInterposer, FsInterposer, HandlerCtx, HostInterposer, KelirPlugin, NetInterposer,
    PipelineBinder, PluginError, TrapInterposer,
};
use kelir_interface::protocol_wire::{FilterAction, TrapEvent, Verdict};

fn trap_ctx() -> HandlerCtx<(), ()> {
    HandlerCtx::new([0u8; 16], 42, (), Vec::new())
}

fn trap_event() -> TrapEvent {
    TrapEvent {
        binary_path: "/workspace/tool".to_string(),
        ..Default::default()
    }
}

fn boot_config() -> kelir_interface::driver_interface::VmConfig {
    default_vm_config(
        None,
        None,
        Some("kernel/vmlinux".to_string()),
        Some("rootfs.erofs".to_string()),
        None,
        None,
    )
}

struct DropTrap;

#[async_trait]
impl TrapInterposer for DropTrap {
    async fn on_trap(
        &self,
        _event: &TrapEvent,
        _ctx: &HandlerCtx,
    ) -> Result<Verdict, kelir_interface::plugin_model::PluginError> {
        Ok(Verdict::Drop {
            errno: Some("EPERM".into()),
            reason: "denied".to_string(),
        })
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

struct PanicTrap;

#[async_trait]
impl TrapInterposer for PanicTrap {
    async fn on_trap(
        &self,
        event: &TrapEvent,
        _ctx: &HandlerCtx,
    ) -> Result<Verdict, kelir_interface::plugin_model::PluginError> {
        assert_eq!(event.pid, u32::MAX);
        Ok(Verdict::Pass)
    }
}

struct MutatingFs;
impl FsInterposer for MutatingFs {
    fn on_fs_write(
        &self,
        _path: &std::path::Path,
        data: &mut [u8],
        _pid: u32,
        _ctx: &HandlerCtx,
    ) -> Result<FilterAction, kelir_interface::plugin_model::PluginError> {
        data.copy_from_slice(b"SELECT 2");
        Ok(FilterAction::Forward)
    }
}

struct CountingNet {
    called: Arc<AtomicBool>,
}

impl NetInterposer for CountingNet {
    fn on_net_inbound(&self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        self.called.store(true, Ordering::SeqCst);
        Ok(FilterAction::Forward)
    }
}

struct CountingConsole {
    called: Arc<AtomicBool>,
}

#[async_trait]
impl ConsoleInterposer for CountingConsole {
    async fn on_console_output(&self, _port: u32, _chunk: &mut Vec<u8>) -> Result<(), PluginError> {
        self.called.store(true, Ordering::SeqCst);
        Ok(())
    }
}

struct CountingHostLsm {
    called: Arc<AtomicBool>,
}

impl HostInterposer for CountingHostLsm {
    fn on_host_lsm_exec(&self, _path: &std::path::Path, _pid: u32) -> Result<Verdict, PluginError> {
        self.called.store(true, Ordering::SeqCst);
        Ok(Verdict::Pass)
    }
}

#[derive(Clone)]
struct LatePlugin;

impl KelirPlugin for LatePlugin {
    type Context = ();
    fn context(&self) -> Self::Context {}
    fn name(&self) -> &'static str {
        "late-plugin"
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
    }
}

// see: cans/artifacts/test-plan/cases/plugin_engine/pe_04_interceptor_pipeline_monotonic_verdict.yaml#case_id
#[tokio::test]
async fn pe_04_interceptor_pipeline_monotonic_verdict() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    engine.bind_trap(Arc::new(DropTrap));
    let second_called = Arc::new(AtomicBool::new(false));
    engine.bind_trap(Arc::new(CountingTrap {
        called: Arc::clone(&second_called),
    }));
    let event = trap_event();
    let ctx = trap_ctx();
    let verdict = engine
        .dispatch_trap(&event, &ctx)
        .await
        .map_err(|e| e.to_string())?;
    match verdict {
        Verdict::Drop {
            errno: Some(errno), ..
        } => assert_eq!(errno, "EPERM"),
        other => return Err(format!("first Drop did not halt pipeline: {other:?}")),
    }
    assert!(!second_called.load(Ordering::SeqCst));
    Ok(())
}

// see: cans/artifacts/test-plan/cases/plugin_engine/pe_05_filter_action_in_place_mutation.yaml#case_id
#[test]
fn pe_05_filter_action_in_place_mutation() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    engine.bind_fs(Arc::new(MutatingFs));
    let mut buf = *b"SELECT 1";
    let before = buf.as_ptr();
    let ctx = trap_ctx();
    let action = engine
        .dispatch_fs_write(
            std::path::Path::new("/workspace/data.sql"),
            &mut buf,
            42,
            &ctx,
        )
        .map_err(|e| e.to_string())?;
    match action {
        FilterAction::Forward => {}
        other => return Err(format!("mutating write did not forward: {other:?}")),
    }
    assert_eq!(buf, *b"SELECT 2");
    assert_eq!(buf.as_ptr(), before);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/plugin_engine/pe_07_aot_hook_fusion_single_trampoline.yaml#case_id
#[test]
fn pe_07_aot_hook_fusion_single_trampoline() -> Result<(), String> {
    let descriptor = FusedTrampolineDescriptor {
        rings: vec![RingChain {
            ring: TrampolineRing::Trap,
            hooks: vec![
                hook_entry(0),
                hook_entry(1),
                hook_entry(2),
                hook_entry(3),
                hook_entry(4),
            ],
        }],
    };
    let table = TrampolineTable::from_descriptor(&descriptor).map_err(|e| e.to_string())?;
    assert_eq!(table.entry_count(), 1);
    match table.entry_for(TrampolineRing::Trap) {
        Some(entry) => assert_eq!(entry.hook_count, 5),
        None => return Err("fused table has no trap entry".to_string()),
    }
    let duplicate = FusedTrampolineDescriptor {
        rings: vec![
            RingChain {
                ring: TrampolineRing::Trap,
                hooks: vec![hook_entry(0)],
            },
            RingChain {
                ring: TrampolineRing::Trap,
                hooks: vec![hook_entry(1)],
            },
        ],
    };
    match TrampolineTable::from_descriptor(&duplicate) {
        Err(message) => assert!(message.contains("more than once")),
        Ok(_) => return Err("duplicate ring accepted".to_string()),
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/plugin_engine/pe_08_segregated_ring_traits_zero_vtable.yaml#case_id
#[test]
fn pe_08_segregated_ring_traits_zero_vtable() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    engine.bind_fs(Arc::new(MutatingFs));
    let mut buf = *b"SELECT 1";
    let ctx = trap_ctx();
    let action = engine
        .dispatch_fs_write(
            std::path::Path::new("/workspace/data.sql"),
            &mut buf,
            42,
            &ctx,
        )
        .map_err(|e| e.to_string())?;
    match action {
        FilterAction::Forward => {}
        other => return Err(format!("fs ring did not apply interceptor: {other:?}")),
    }
    assert_eq!(buf, *b"SELECT 2");
    let mut frame = vec![0x01u8, 0x02, 0x03];
    let net_action = engine
        .dispatch_net_outbound(&mut frame)
        .map_err(|e| e.to_string())?;
    match net_action {
        FilterAction::Forward => {}
        other => return Err(format!("net ring did not forward untouched: {other:?}")),
    }
    assert_eq!(frame, vec![0x01u8, 0x02, 0x03]);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/plugin_engine/pe_09_panic_quarantine_catch_unwind_efault.yaml#case_id
#[tokio::test]
async fn pe_09_panic_quarantine_catch_unwind_efault() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    engine.bind_trap(Arc::new(PanicTrap));
    let mut event = trap_event();
    event.pid = 42;
    let ctx = trap_ctx();
    let verdict = engine
        .dispatch_trap(&event, &ctx)
        .await
        .map_err(|e| e.to_string())?;
    match verdict {
        Verdict::Drop {
            errno: Some(errno),
            reason,
        } => {
            assert_eq!(errno, "EFAULT");
            assert_eq!(reason, "Plugin panicked");
        }
        other => {
            return Err(format!(
                "panicking hook did not quarantine to EFAULT Drop: {other:?}"
            ));
        }
    }
    let second = engine
        .dispatch_trap(&event, &ctx)
        .await
        .map_err(|e| e.to_string())?;
    match second {
        Verdict::Drop {
            errno: Some(errno), ..
        } => assert_eq!(errno, "EFAULT"),
        other => {
            return Err(format!(
                "engine did not survive quarantined panic: {other:?}"
            ));
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/plugin_engine/pe_10_immutable_dispatch_table_post_seal.yaml#case_id
#[test]
fn pe_10_immutable_dispatch_table_post_seal() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    assert!(!engine.is_sealed());
    engine.seal().map_err(|e| e.to_string())?;
    assert!(engine.is_sealed());
    match engine.seal() {
        Err(kelir_interface::plugin_model::PluginError::Halt(message)) => {
            assert_eq!(message, "pipeline already sealed");
        }
        Err(e) => return Err(format!("second seal gave wrong error: {e}")),
        Ok(()) => return Err("second seal succeeded".to_string()),
    }
    let mut session = Session::boot(boot_config()).map_err(|e| e.to_string())?;
    assert!(!session.pipeline().is_sealed());
    session.seal_pipeline().map_err(|e| e.to_string())?;
    assert!(session.pipeline().is_sealed());
    let late = LatePlugin;
    match session.bind_plugin(&late) {
        Err(kelir_interface::plugin_model::PluginError::Halt(message)) => {
            assert_eq!(message, "pipeline already sealed");
        }
        Err(e) => return Err(format!("late bind gave wrong error: {e}")),
        Ok(()) => return Err("late bind after seal succeeded".to_string()),
    }
    Ok(())
}

fn saturate_foreign_queue(engine: &PipelineEngine) {
    while engine.foreign_queue().try_submit().is_ok() {}
}

#[test]
fn ffi_queue_saturation_rejects_net_inbound_ring() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    let called = Arc::new(AtomicBool::new(false));
    engine.bind_net(Arc::new(CountingNet {
        called: Arc::clone(&called),
    }));
    saturate_foreign_queue(&engine);
    let mut frame = [0x11u8, 0x22, 0x33];
    match engine.dispatch_net_inbound(&mut frame) {
        Err(PluginError::FfiQueueSaturated { depth }) => assert_eq!(depth, 4096),
        other => {
            return Err(format!(
                "saturated net-inbound ring did not reject: {other:?}"
            ));
        }
    }
    assert!(!called.load(Ordering::SeqCst));
    Ok(())
}

#[test]
fn ffi_queue_saturation_rejects_host_lsm_exec_ring() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    let called = Arc::new(AtomicBool::new(false));
    engine.bind_host(Arc::new(CountingHostLsm {
        called: Arc::clone(&called),
    }));
    saturate_foreign_queue(&engine);
    match engine.dispatch_host_lsm_exec(std::path::Path::new("/usr/bin/ssh"), 7) {
        Err(PluginError::FfiQueueSaturated { depth }) => assert_eq!(depth, 4096),
        other => return Err(format!("saturated host-lsm ring did not reject: {other:?}")),
    }
    assert!(!called.load(Ordering::SeqCst));
    Ok(())
}

#[tokio::test]
async fn ffi_rate_limit_rejects_console_output_ring() -> Result<(), String> {
    let mut engine = PipelineEngine::new().with_ffi_rate_limit(1);
    let called = Arc::new(AtomicBool::new(false));
    engine.bind_console(Arc::new(CountingConsole {
        called: Arc::clone(&called),
    }));
    let mut chunk = vec![b'k'];
    engine
        .dispatch_console_output(0, &mut chunk)
        .await
        .map_err(|e| e.to_string())?;
    assert!(called.load(Ordering::SeqCst));
    match engine.dispatch_console_output(0, &mut chunk).await {
        Err(PluginError::Halt(message)) => {
            assert!(message.contains("FFI rate limit exceeded"))
        }
        other => {
            return Err(format!(
                "second console chunk did not hit the rate gate: {other:?}"
            ));
        }
    }
    assert_eq!(engine.foreign_queue().depth(), 0);
    Ok(())
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

fn pre_step_descriptor(id: &str) -> SnapshotDescriptor {
    SnapshotDescriptor {
        id: id.to_string(),
        dirty_bytes: 0,
        created_at_ns: 0,
        parent_digest: None,
        guest_clock: None,
    }
}

#[tokio::test]
async fn hydrate_step_populates_handler_ctx_from_checkpoint_plane() -> Result<(), String> {
    let plane = Arc::new(CheckpointPlane::new());
    let lineage = PidLineage::new_in_session(1, [7u8; 16]);
    plane
        .create_step(lineage, pre_step_descriptor("pre-exec"))
        .map_err(|e| e.to_string())?;
    let engine = PipelineEngine::new().with_checkpoint(Arc::clone(&plane));
    let mut ctx: HandlerCtx<(), ()> = HandlerCtx::new([0u8; 16], 1, (), Vec::new());
    engine
        .hydrate_step(&mut ctx, lineage)
        .map_err(|e| e.to_string())?;
    match ctx.step.last() {
        Some(step) => assert_eq!(step.id, "pre-exec"),
        None => return Err("hydrate_step left the pre-exec step stack empty".to_string()),
    }
    plane.commit_step(lineage).map_err(|e| e.to_string())?;
    match plane.active_step(lineage) {
        Ok(None) => {}
        Ok(Some(step)) => return Err(format!("committed step stayed active: {}", step.id)),
        Err(e) => return Err(format!("active_step after commit failed: {e}")),
    }
    let mut post: HandlerCtx<(), ()> = HandlerCtx::new([0u8; 16], 1, (), Vec::new());
    engine
        .hydrate_step(&mut post, lineage)
        .map_err(|e| e.to_string())?;
    match post.step.last() {
        None => {}
        Some(step) => {
            return Err(format!(
                "hydrate pushed a step with no active slot: {}",
                step.id
            ));
        }
    }
    let event = trap_event();
    let verdict = engine
        .dispatch_trap(&event, &post)
        .await
        .map_err(|e| e.to_string())?;
    match verdict {
        Verdict::Pass => {}
        other => return Err(format!("hydrated dispatch did not pass: {other:?}")),
    }
    Ok(())
}

#[tokio::test]
async fn hydrate_step_without_checkpoint_plane_is_inert() -> Result<(), String> {
    let engine = PipelineEngine::new();
    let mut ctx: HandlerCtx<(), ()> = HandlerCtx::new([0u8; 16], 42, (), Vec::new());
    engine
        .hydrate_step(&mut ctx, PidLineage::new_in_session(1, [7u8; 16]))
        .map_err(|e| e.to_string())?;
    match ctx.step.last() {
        None => {}
        Some(step) => return Err(format!("inert hydrate pushed a step: {}", step.id)),
    }
    Ok(())
}
