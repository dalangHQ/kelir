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
    DriverChoice, HandlerCtx, KelirBuilder, Session, VmConfig,
    plugin::{AnyPlugin, KelirPlugin, PluginError, TrapInterposer},
    protocol::{TrapAction, TrapEvent},
    planes::storage::{IdmapConfig, ReadMaskEngine, StorageConfig, WorkspaceEncapsulation},
};

const PROTECTED: &[&str] = &["main", "master"];

pub struct GitGatePlugin;

impl KelirPlugin for GitGatePlugin {
    fn name(&self) -> &'static str { "git-gate" }
    fn fork_state(&self, _: &str) -> Box<dyn AnyPlugin> { Box::new(Self) }
}

#[async_trait]
impl TrapInterposer for GitGatePlugin {
    async fn on_trap(
        &self, event: &TrapEvent, _c: &HandlerCtx,
    ) -> Result<Option<TrapAction>, PluginError> {
        let binary = event.binary_path.rsplit('/').next().unwrap_or("");
        if binary != "git" { return Ok(None); }
        match event.argv.get(1).map(String::as_str) {
            Some("push") => {
                if event.argv.iter().any(|a| a == "--force") {
                    return Ok(Some(TrapAction::Deny {
                        exit_code: 1,
                        diagnostic_payload: b"EPOLICY: --force rejected".to_vec(),
                    }));
                }
                Ok(None)
            }
            Some("commit") => {
                let msg = event.argv.iter()
                    .skip_while(|a| *a != "-m")
                    .nth(1)
                    .map(String::as_str)
                    .unwrap_or("");
                if PROTECTED.iter().any(|p| msg.contains(p)) {
                    return Ok(Some(TrapAction::Abort {
                        exit_code: 1,
                        reason: "EPOLICY: commit references protected branch".into(),
                    }));
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let session: Session = KelirBuilder::new(VmConfig {
        vcpus: 2, memory_mb: 1024,
        kernel: format!("./fixtures/vmlinux-6.6-{}-kelir", std::env::consts::ARCH).into(),
        rootfs_erofs: "./fixtures/rootfs-git.erofs".into(),
        tsc_locked: true, enable_watchdog: true,
        enable_gpu_passthrough: false,
        driver: DriverChoice::Auto,
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