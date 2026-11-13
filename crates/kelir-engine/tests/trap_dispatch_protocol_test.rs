// see: cans/artifacts/test-plan/cases/plugin_engine/pe_04_interceptor_pipeline_monotonic_verdict.yaml#case_id
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use kelir_checkpoint::RealCheckpointOps;
use kelir_engine::PluginRegistration;
use kelir_engine::Session;
use kelir_engine::dispatch_plugin::PipelineEngine;
use kelir_engine::planes::checkpoint::CheckpointPlane;
use kelir_engine::session_lifecycle::PlaneWiring;
use kelir_interface::checkpoint::PidLineage;
use kelir_interface::driver_interface::{SnapshotDescriptor, default_vm_config};
use kelir_interface::errno_types::Errno;
use kelir_interface::fusion_plugin::{
    FusedTrampolineCallback, FusedTrampolineDescriptor, HookEntry, HookStage, RingChain,
    TrampolineRing, TrampolineTable,
};
use kelir_interface::plugin_model::{
    AnyPlugin, ConsoleInterposer, FsInterposer, HandlerCtx, HostInterposer, KelirPlugin,
    MountScope, NetInterposer, PipelineBinder, PluginError, TrapInterposer,
};
use kelir_interface::protocol_wire::{FilterAction, TrapEvent, Verdict};
use std::sync::Mutex;

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

fn plane_wiring() -> kelir_engine::session_lifecycle::PlaneWiring {
    let metrics = || Arc::new(kelir_otlp::registry_otlp::MetricRegistry::new());
    let dns_ops: Arc<dyn kelir_interface::wire::dns::DnsOps> =
        Arc::new(kelir_wire::RealDnsOps::new());
    kelir_engine::session_lifecycle::PlaneWiring {
        terminal_ops: Some(Arc::new(kelir_terminal::RealTerminalOps::new(metrics()))),
        checkpoint_ops: Some(Arc::new(
            kelir_checkpoint::RealCheckpointOps::new(metrics()),
        )),
        host_ops: Some(Arc::new(kelir_host::RealHostOps::new(metrics()))),
        net_ops: Some(Arc::new(kelir_wire::RealNetOps::new(Arc::clone(&dns_ops)))),
        dns_ops: Some(dns_ops),
        udp_nat_ops: Some(Arc::new(kelir_wire::RealUdpNatOps::new())),
        l4_ops: Some(Arc::new(kelir_wire::RealL4DivertOps::new())),
        storage_io: Some(Arc::new(kelir_storage_io::RealStorageIo)),
        ..Default::default()
    }
}

fn checkpoint_plane() -> Arc<CheckpointPlane> {
    Arc::new(CheckpointPlane::new(Arc::new(RealCheckpointOps::new(
        Arc::new(kelir_otlp::registry_otlp::MetricRegistry::new()),
    ))))
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
            errno: Some(Errno::Eperm),
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
    async fn on_console_output(&self, _port: u32, chunk: &[u8]) -> Result<Vec<u8>, PluginError> {
        self.called.store(true, Ordering::SeqCst);
        Ok(chunk.to_vec())
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
        } => assert_eq!(errno, Errno::Eperm),
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
            assert_eq!(errno, Errno::Efault);
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
        } => assert_eq!(errno, Errno::Efault),
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
    let mut session =
        Session::boot_with_plugins(boot_config(), &[], None, Vec::new(), plane_wiring())
            .map_err(|e| e.to_string())?;
    assert!(!session.pipeline().is_sealed());
    session.pipeline_mut().seal().map_err(|e| e.to_string())?;
    assert!(session.pipeline().is_sealed());
    match session.pipeline_mut().seal() {
        Err(kelir_interface::plugin_model::PluginError::Halt(message)) => {
            assert_eq!(message, "pipeline already sealed");
        }
        Err(e) => return Err(format!("late seal gave wrong error: {e:?}")),
        Ok(()) => return Err("late seal after seal succeeded".to_string()),
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

// see: cans/11-protocol.md#Trap-fast-path
#[tokio::test]
async fn interpreter_dash_c_payload_forfeits_trusted_prefix_skip() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    let called = Arc::new(AtomicBool::new(false));
    engine.bind_trap(Arc::new(CountingTrap {
        called: Arc::clone(&called),
    }));
    let mut event = trap_event();
    event.binary_path = "/bin/sh".to_string();
    event.argv = vec![
        "sh".to_string(),
        "-c".to_string(),
        "git push --force".to_string(),
    ];
    let ctx = trap_ctx();
    engine
        .dispatch_trap(&event, &ctx)
        .await
        .map_err(|e| e.to_string())?;
    assert!(
        called.load(Ordering::SeqCst),
        "sh -c payload kept the trusted-prefix skip and never crossed Gate 3"
    );
    Ok(())
}

// see: cans/11-protocol.md#Trap-fast-path
#[tokio::test]
async fn interpreter_trusted_script_fails_closed_pending_canonical_identity() -> Result<(), String>
{
    let mut engine = PipelineEngine::new();
    let called = Arc::new(AtomicBool::new(false));
    engine.bind_trap(Arc::new(CountingTrap {
        called: Arc::clone(&called),
    }));
    let mut event = trap_event();
    event.binary_path = "/bin/sh".to_string();
    event.argv = vec!["sh".to_string(), "/usr/bin/ls".to_string()];
    let ctx = trap_ctx();
    engine
        .dispatch_trap(&event, &ctx)
        .await
        .map_err(|e| e.to_string())?;
    assert!(
        called.load(Ordering::SeqCst),
        "trusted-prefix skip stays inert until the canonical (st_dev, st_ino, binary_blake3) identity lands"
    );
    Ok(())
}

// see: cans/11-protocol.md#Trap-fast-path
#[tokio::test]
async fn interpreter_relative_script_forfeits_trusted_prefix_skip() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    let called = Arc::new(AtomicBool::new(false));
    engine.bind_trap(Arc::new(CountingTrap {
        called: Arc::clone(&called),
    }));
    let mut event = trap_event();
    event.binary_path = "/usr/bin/python3".to_string();
    event.argv = vec!["python3".to_string(), "script.py".to_string()];
    let ctx = trap_ctx();
    engine
        .dispatch_trap(&event, &ctx)
        .await
        .map_err(|e| e.to_string())?;
    assert!(
        called.load(Ordering::SeqCst),
        "relative script argv kept the trusted-prefix skip without EROFS resolution"
    );
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
    let plane = checkpoint_plane();
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

#[derive(Clone)]
struct ScopeMutator {
    name: &'static str,
    scope_paths: &'static [&'static str],
}

impl KelirPlugin for ScopeMutator {
    type Context = ();
    fn context(&self) -> Self::Context {}
    fn name(&self) -> &'static str {
        self.name
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default().with_paths(self.scope_paths.iter().copied())
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        pipeline.bind_fs_scoped(Arc::new(MutatingFs), KelirPlugin::default_scope(self));
    }
}

// see: cans/artifacts/test-plan/cases/plugin_engine/pe_03_overlapping_mutator_scope_eambiguous.yaml#case_id
#[test]
#[ignore]
fn pe_03_overlapping_mutator_scope_eambiguous() -> Result<(), String> {
    let mut engine = PipelineEngine::new();
    let probe_a = ScopeMutator {
        name: "mutator-a",
        scope_paths: &["/workspace/src"],
    };
    KelirPlugin::register_interceptors(&probe_a, &mut engine);
    let probe_b = ScopeMutator {
        name: "mutator-b",
        scope_paths: &["/workspace/src/main.rs"],
    };
    KelirPlugin::register_interceptors(&probe_b, &mut engine);
    let mut session = Session::boot_with_engine(
        boot_config(),
        &[],
        engine,
        std::sync::Arc::new(kelir_otlp::registry_otlp::MetricRegistry::new()),
        Vec::new(),
        PlaneWiring::default(),
    )
    .map_err(|e| e.to_string())?;
    match session.pipeline_mut().seal() {
        Err(PluginError::AmbiguousMutation { resource, conflict }) => {
            assert!(
                resource.contains("main.rs") || conflict.contains("main.rs"),
                "EAmbiguousMutation must name the overlapping target, got resource={resource:?} conflict={conflict:?}"
            );
        }
        Err(e) => {
            return Err(format!("overlapping mutator seal gave wrong error: {e:?}"));
        }
        Ok(()) => {
            return Err(
                "boot accepted overlapping mutator scopes on /workspace/src/main.rs; expected EAmbiguousMutation"
                    .into(),
            );
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/plugin_engine/pe_02_cyclic_dependency_graph_abort.yaml#case_id
#[test]
#[ignore]
fn pe_02_cyclic_dependency_graph_abort() -> Result<(), String> {
    let registrations = [
        PluginRegistration::new("plugin_a -> plugin_b", true),
        PluginRegistration::new("plugin_b -> plugin_a", true),
    ];
    match Session::boot_with_plugins(
        boot_config(),
        &registrations,
        None,
        Vec::new(),
        PlaneWiring::default(),
    ) {
        Err(PluginError::Halt(message)) => {
            let lowered = message.to_lowercase();
            assert!(
                lowered.contains("cycle")
                    && message.contains("plugin_a")
                    && message.contains("plugin_b"),
                "cyclic registration abort must name the cycle, got: {message}"
            );
        }
        Err(e) => {
            return Err(format!("cyclic registration abort gave wrong error: {e:?}"));
        }
        Ok(_) => {
            return Err(
                "Session::boot_with_plugins accepted the cyclic registration graph plugin_a -> plugin_b -> plugin_a"
                    .into(),
            );
        }
    }
    Ok(())
}

struct ChunkRecordingFs {
    chunks: Arc<Mutex<Vec<usize>>>,
}

impl FsInterposer for ChunkRecordingFs {
    fn on_fs_write(
        &self,
        _path: &std::path::Path,
        data: &mut [u8],
        _pid: u32,
        _ctx: &HandlerCtx,
    ) -> Result<FilterAction, PluginError> {
        self.chunks
            .lock()
            .map_err(|_| PluginError::Halt("chunk log poisoned".into()))?
            .push(data.len());
        Ok(FilterAction::Forward)
    }
}

// see: cans/artifacts/test-plan/cases/plugin_engine/pe_11_ffi_buffer_streaming_chunk_ceiling.yaml#case_id
#[test]
#[ignore]
fn pe_11_ffi_buffer_streaming_chunk_ceiling() -> Result<(), String> {
    let chunks = Arc::new(Mutex::new(Vec::new()));
    let mut engine = PipelineEngine::new();
    engine.bind_fs(Arc::new(ChunkRecordingFs {
        chunks: Arc::clone(&chunks),
    }));
    let mut buf = vec![0u8; 1_048_576];
    let ctx = trap_ctx();
    engine
        .dispatch_fs_write(
            std::path::Path::new("/workspace/blob.bin"),
            &mut buf,
            42,
            &ctx,
        )
        .map_err(|e| e.to_string())?;
    let observed = match chunks.lock() {
        Ok(guard) => guard,
        Err(_) => return Err("chunk log poisoned".into()),
    };
    assert_eq!(
        observed.len(),
        16,
        "1 MiB FUSE write must stream as exactly 16 chunks, got {observed:?}"
    );
    let max = observed.iter().copied().max().unwrap_or(0);
    assert!(
        max <= 65536,
        "per-chunk FFI ceiling is 65536 bytes, observed max {max} in {observed:?}"
    );
    Ok(())
}

#[derive(Clone)]
struct ForkProbe {
    tag: &'static str,
    log: Arc<Mutex<Vec<String>>>,
}

struct ForkProbeTrap {
    tag: &'static str,
    log: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl TrapInterposer for ForkProbeTrap {
    async fn on_trap(&self, _event: &TrapEvent, _ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        self.log
            .lock()
            .map_err(|_| PluginError::Halt("probe log poisoned".into()))?
            .push(format!("trap:{}", self.tag));
        Ok(Verdict::Pass)
    }
}

impl KelirPlugin for ForkProbe {
    type Context = ();
    fn context(&self) -> Self::Context {}
    fn name(&self) -> &'static str {
        "fork-probe"
    }
    fn fork_state(&self, _child_session_id: &str) -> Box<dyn AnyPlugin> {
        Box::new(ForkProbe {
            tag: "child",
            log: Arc::clone(&self.log),
        })
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        pipeline.bind_trap(Arc::new(ForkProbeTrap {
            tag: self.tag,
            log: Arc::clone(&self.log),
        }));
    }
}

// see: cans/artifacts/test-plan/cases/plugin_engine/pe_13_fork_state_specialization_isolation.yaml#case_id
#[tokio::test]
#[ignore]
async fn pe_13_fork_state_specialization_isolation() -> Result<(), String> {
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut engine = PipelineEngine::new();
    let parent_probe = ForkProbe {
        tag: "parent",
        log: Arc::clone(&log),
    };
    KelirPlugin::register_interceptors(&parent_probe, &mut engine);
    let parent = Session::boot_with_engine(
        boot_config(),
        &[PluginRegistration::new("fork-probe", true)],
        engine,
        std::sync::Arc::new(kelir_otlp::registry_otlp::MetricRegistry::new()),
        Vec::new(),
        PlaneWiring::default(),
    )
    .map_err(|e| e.to_string())?;
    let event = trap_event();
    let ctx = trap_ctx();
    parent
        .pipeline()
        .dispatch_trap(&event, &ctx)
        .await
        .map_err(|e| e.to_string())?;
    match log.lock() {
        Ok(entries) => assert!(
            entries.iter().any(|entry| entry == "trap:parent"),
            "parent dispatch did not reach the probe: {entries:?}"
        ),
        Err(_) => return Err("probe log poisoned".into()),
    }
    let child = parent.fork().await.map_err(|e| e.to_string())?;
    assert_ne!(
        child.incarnation_id(),
        parent.incarnation_id(),
        "fork must mint a distinct incarnation"
    );
    child
        .pipeline()
        .dispatch_trap(&event, &ctx)
        .await
        .map_err(|e| e.to_string())?;
    match log.lock() {
        Ok(entries) => assert!(
            entries.iter().any(|entry| entry == "trap:child"),
            "child pipeline carried no fork_state-specialized plugin probe: {entries:?}"
        ),
        Err(_) => return Err("probe log poisoned".into()),
    }
    Ok(())
}
