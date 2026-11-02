//! examples/git-gate.rs
//!
//! Focused example: terminal + trap interposition.
//! One plugin, one hook (on_trap), one binary family (git).
//!
//! Non-default storage knobs exercised in context (formerly knob-matrix):
//!   encapsulation   Passthrough     (git-gate does not lock workspace)
//!   scan_engine     FastRegex       (diff inspection is regex-shaped)
//!   carryover_bytes 1024            (diff chunks exceed 512)
//!
//! Cargo.toml:
//!   [dependencies]
//!   kelir = "3.0"
//!   tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
//!   async-trait = "0.1"

use std::path::Path;
use async_trait::async_trait;
use kelir::{
    driver::{DriverPolicy, WxPolicy}, HandlerCtx, KelirBuilder, Session, VmConfig,
    plugin::{AnyPlugin, KelirPlugin, PluginError, TrapInterposer, Verdict},
    protocol::TrapEvent,
    planes::storage::{IdmapConfig, ReadMaskEngine, StorageConfig, WorkspaceEncapsulation},
};

const PROTECTED: &[&str] = &["main", "master"];

pub struct GitGatePlugin;

impl KelirPlugin for GitGatePlugin {
    type Context = ();
    fn context(&self) -> Self::Context { () }
    fn name(&self) -> &'static str { "git-gate" }
    fn fork_state(&self, _: &str) -> Box<dyn AnyPlugin> { Box::new(Self) }
}

#[async_trait]
impl TrapInterposer for GitGatePlugin {
    async fn on_trap(
        &self, event: &TrapEvent, _c: &HandlerCtx,
    ) -> Result<Verdict, PluginError> {
        let binary = event.binary_path.rsplit('/').next().unwrap_or("");
        if binary != "git" { return Ok(Verdict::Pass); }
        match event.argv.get(1).map(String::as_str) {
            Some("push") => {
                if event.argv.iter().any(|a| a == "--force") {
                    return Ok(Verdict::Drop {
                        errno: Some("EPOLICY"),
                        reason: "EPOLICY: --force rejected".into(),
                    });
                }
                Ok(Verdict::Pass)
            }
            Some("commit") => {
                let msg = event.argv.iter()
                    .skip_while(|a| *a != "-m")
                    .nth(1)
                    .map(String::as_str)
                    .unwrap_or("");
                if PROTECTED.iter().any(|p| msg.contains(p)) {
                    return Ok(Verdict::Abort {
                        reason: "EPOLICY: commit references protected branch".into(),
                    });
                }
                Ok(Verdict::Pass)
            }
            _ => Ok(Verdict::Pass),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let session: Session = KelirBuilder::new(VmConfig {
        vcpus: 2, memory_mb: 1024,
        kernel_path: format!("./fixtures/vmlinux-6.6-{}-kelir", std::env::consts::ARCH).into(),
        rootfs_erofs_path: "./fixtures/rootfs-git.erofs".into(),
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
        page_size_kb: 4,
        deterministic_clock: false,
        network_off: false,
        pinned_clocksource: None,
        ns_per_instruction: None,
        preemption_grace_ms: 0,
        entropy_seed: None,
    })
    .storage(StorageConfig {
        idmap: IdmapConfig { guest_uid: 0, host_uid: 0 },
        shared_caches: vec![],
        lock_broker: false,
        synthetic_files: vec![],
        mask_read: vec![],
        snoops: vec![],
        encapsulation: WorkspaceEncapsulation::Passthrough,
        scan_engine: ReadMaskEngine::FastRegex,
        carryover_bytes: 1024,
    })
    .plugin(GitGatePlugin)
    .boot()
    .await?;

    let run = session.spawn("git")
        .args(["push", "--force", "origin", "main"])
        .cwd("/workspace/repo")
        .await?;
    eprintln!("[kelir] git push --force -> exit={}", run.exit_code);

    session.terminate().await?;
    Ok(())
}