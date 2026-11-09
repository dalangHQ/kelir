// see: cans/artifacts/test-plan/cases/plugin_engine/pe_04_interceptor_pipeline_monotonic_verdict.yaml#case_id
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use kelir_core::Session;
use kelir_core::dispatch_plugin::PipelineEngine;
use kelir_interface::driver_interface::default_vm_config;
use kelir_interface::fusion_plugin::{
    FusedTrampolineDescriptor, HookEntry, HookStage, RingChain, TrampolineRing, TrampolineTable,
};
use kelir_interface::plugin_model::{
    FsInterposer, HandlerCtx, KelirPlugin, PipelineBinder, TrapInterposer,
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
