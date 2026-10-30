//! examples/replay.rs
//!
//! Focused example: cycle-exact instruction clock stepping.
//! One plugin (replay), one primitive (clock), one workload.
//!
//! Non-default driver knobs exercised in context (formerly knob-matrix):
//!   wx_enforcement PermissiveAudit (replay binary is not JIT-tight)
//!   allow_emulated_fallback true    (replay works under bwrap)
//!   driver bwrap                    (emulated tier; replay is host-CPU)
//!
//! Cargo.toml:
//!   [dependencies]
//!   kelir = { version = "3.0", features = ["replay"] }
//!   tokio = { version = "1", features = ["rt-multi-thread", "macros"] }

use kelir::{
    DriverChoice, KelirBuilder, Session, VmConfig,
    driver::{DriverPolicy, WxPolicy},
    plugins::replay::{ReplayConfig, ReplayPlugin},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let session: Session = KelirBuilder::new(VmConfig {
        vcpus: 2, memory_mb: 1024,
        kernel: format!("./fixtures/vmlinux-6.6-{}-kelir", std::env::consts::ARCH).into(),
        rootfs_erofs: "./fixtures/rootfs-replay.erofs".into(),
        tsc_locked: true, enable_watchdog: true,
        enable_gpu_passthrough: false,
        driver_policy: DriverPolicy {
            require_hardware_vmm: false,
            allow_emulated_fallback: true,
            verify_hw_lockfile: false,
            wx_enforcement: WxPolicy::PermissiveAudit,
            ffi_worker_threads: 2,
            ffi_max_ops_per_sec: 1000,
        },
        deterministic_clock: false,
        pinned_clocksource: Some("kvm-clock".into()),
        ns_per_instruction: Some(1),
        preemption_grace_ms: 0,
        // Replay mode: virtio-rng is substituted with a seeded ChaCha20
        // stream, so getrandom() payloads match across runs.
        entropy_seed: Some(0x5EED),
    })
    .plugin(ReplayPlugin::new(ReplayConfig {
        // Advance guest clock strictly on retired-instruction windows.
        step_window: 1_000_000,
        kernel_params: vec![
            "clocksource=kvm-clock".into(),
            "tsc=reliable".into(),
            "nohpet".into(),
        ],
    }))
    .boot()
    .await?;

    // Two runs of the same workload produce identical wall_clock_ns.
    for run in 0..2 {
        let r = session.spawn("date").args(["+%s.%N"]).await?;
        eprintln!("[kelir] run {run} -> exit={}", r.exit_code);
    }

    session.terminate().await?;
    Ok(())
}