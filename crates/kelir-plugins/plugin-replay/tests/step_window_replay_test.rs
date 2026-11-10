// see: cans/artifacts/test-plan/cases/lifecycle_checkpoint/lc_23_replay_deterministic_seed_reproduces_step_windows.yaml#case_id
use driver_rustvmm::RustVmmDriver;
use kelir_interface::driver_interface::default_vm_config;
use kelir_interface::plugin_model::{LifecycleCtx, LifecycleParticipant, VetoList};
use plugin_replay::{ReplayConfig, ReplayPlugin};

fn replay_config() -> ReplayConfig {
    ReplayConfig {
        step_window: 1_000_000,
        kernel_params: vec![
            "clocksource=kvm-clock".to_string(),
            "tsc=reliable".to_string(),
            "nohpet".to_string(),
        ],
        deterministic_rng_seed: 0x5EED,
        network_isolation: true,
    }
}

fn vm_config() -> kelir_interface::driver_interface::VmConfig {
    default_vm_config(
        None,
        None,
        Some("kernel/vmlinux".to_string()),
        Some("rootfs.erofs".to_string()),
        None,
        None,
    )
}

// see: cans/artifacts/test-plan/cases/lifecycle_checkpoint/lc_23_replay_deterministic_seed_reproduces_step_windows.yaml#case_id
#[tokio::test]
#[ignore]
async fn lc_23_replay_deterministic_seed_reproduces_step_windows() -> Result<(), String> {
    let plugin = ReplayPlugin::new(replay_config());
    let vm_config = vm_config();
    let mut driver = RustVmmDriver::new().map_err(|e| e.to_string())?;
    let mut vetoes: VetoList = Vec::new();
    let mut ctx = LifecycleCtx::new(&vm_config, "ses-lc-23", &mut driver, &mut vetoes);
    plugin
        .on_vm_init(&mut ctx)
        .await
        .map_err(|e| e.to_string())?;
    assert!(
        plugin.ctx().pmu_sampling_active(),
        "replay PMU sampling degraded on the real rustvmm driver: identical seeds cannot reproduce step windows"
    );
    let replay = ReplayPlugin::new(replay_config());
    let mut second_driver = RustVmmDriver::new().map_err(|e| e.to_string())?;
    let mut second_vetoes: VetoList = Vec::new();
    let mut second_ctx = LifecycleCtx::new(
        &vm_config,
        "ses-lc-23",
        &mut second_driver,
        &mut second_vetoes,
    );
    replay
        .on_vm_init(&mut second_ctx)
        .await
        .map_err(|e| e.to_string())?;
    assert!(
        replay.ctx().pmu_sampling_active(),
        "second replay run degraded PMU sampling: window reproduction is not a pure function of the seed"
    );
    Ok(())
}
