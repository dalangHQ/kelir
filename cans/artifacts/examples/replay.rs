// see: cans/plugins/replay.md#Plugin-replay
// see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
// see: cans/plugins/replay.md#Deterministic-RNG
// see: cans/plugins/replay.md#Replay-mode-requires-network-isolation-for-determinism

use kelir::{
    KelirBuilder, PidLineage, ReplayConfig, ReplayPlugin, SnapshotDescriptor, VmConfig,
    driver_interface::{DriverPolicy, WxPolicy},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut session = KelirBuilder::new(VmConfig {
        vcpus: 2,
        memory_mb: 1024,
        page_size_kb: 4,
        kernel_path: format!("./fixtures/vmlinux-6.6-{}-kelir", std::env::consts::ARCH).into(),
        rootfs_erofs_path: "./fixtures/rootfs-replay.erofs".into(),
        tsc_locked: true,
        enable_watchdog: true,
        enable_gpu_passthrough: false,
        driver_policy: DriverPolicy {
            require_hardware_vmm: false,
            allow_emulated_fallback: true,
            verify_hw_lockfile: false,
            wx_enforcement: WxPolicy::PermissiveAudit,
            ffi_worker_threads: 2,
            ffi_max_ops_per_sec: 1000,
        },

        deterministic_clock: true,
        network_off: true,
        pinned_clocksource: Some("kvm-clock".into()),
        ns_per_instruction: Some(1),
        preemption_grace_ms: 0,

        entropy_seed: Some(0x5EED),
    })
    .plugin(ReplayPlugin::new(ReplayConfig {
        step_window: 1_000_000,
        kernel_params: vec![
            "clocksource=kvm-clock".into(),
            "tsc=reliable".into(),
            "nohpet".into(),
        ],
        deterministic_rng_seed: 0x5EED,
        network_isolation: true,
    }))
    .boot()
    .await?;

    for run in 0..2 {
        let slot = session.checkpoint.create_step(
            PidLineage::new(1),
            SnapshotDescriptor {
                id: format!("replay-run-{run}"),
                dirty_bytes: 0,
                created_at_ns: 0,
                parent_digest: None,
                guest_clock: None,
            },
        )?;
        let node = session.checkpoint.commit_step(PidLineage::new(1))?;
        eprintln!("[kelir] run {run} -> slot={slot} node={node}");
    }
    // TODO (L3): spawn date +%s.%N in the guest twice and replay both runs deterministically.
    session.stop()?;
    Ok(())
}
