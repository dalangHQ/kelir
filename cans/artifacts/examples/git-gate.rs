// see: cans/02-plugin-model.md#Unified-interposition-pipeline
// see: cans/05-storage-plane.md#Encapsulation-invariant
// see: cans/05-storage-plane.md#In-flight-read-masking

use async_trait::async_trait;
use kelir::{
    driver_interface::{DriverPolicy, WxPolicy},
    planes::storage_plane::{
        IdmapConfig, ReadMaskEngine, StoragePlaneConfig, WorkspaceEncapsulation,
    },
    plugin_model::{KelirPlugin, NoDeps, PluginDeps, PluginError, TrapInterposer, Verdict},
    protocol_wire::{ExecveMutation, PatchPayload, TrapEvent},
    KelirBuilder, SnapshotDescriptor, VmConfig,
};
use kelir_interface::plugin_model::HandlerCtx;

const PROTECTED: &[&str] = &["main", "master"];

#[derive(Clone)]
pub struct GitGatePlugin;

impl KelirPlugin for GitGatePlugin {
    type Context = ();
    fn context(&self) -> Self::Context {}
    fn name(&self) -> &'static str {
        "git-gate"
    }
}

impl PluginDeps for GitGatePlugin {
    type Deps = NoDeps;
}

#[async_trait]
impl TrapInterposer for GitGatePlugin {
    async fn on_trap(&self, event: &TrapEvent, _c: &HandlerCtx) -> Result<Verdict, PluginError> {
        let binary = event.binary_path.rsplit('/').next().unwrap_or("");
        if binary != "git" {
            return Ok(Verdict::Pass);
        }
        match event.argv.get(1).map(String::as_str) {
            Some("push") => {
                if event.argv.iter().any(|a| a == "--force") {
                    let argv: Vec<String> = event
                        .argv
                        .iter()
                        .filter(|a| a.as_str() != "--force")
                        .cloned()
                        .collect();
                    return Ok(Verdict::Patch(PatchPayload::ExecveMutation(
                        ExecveMutation {
                            new_binary: None,
                            new_argv: Some(argv),
                            set_env: vec![("KELIR_DISARMED".to_string(), "1".to_string())],
                            unset_env: vec![],
                        },
                    )));
                }
                Ok(Verdict::Pass)
            }
            Some("commit") => {
                let msg = event
                    .argv
                    .iter()
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
    let mut session = KelirBuilder::new(VmConfig {
        vcpus: 2,
        memory_mb: 1024,
        kernel_path: format!("./fixtures/vmlinux-6.6-{}-kelir", std::env::consts::ARCH).into(),
        rootfs_erofs_path: "./fixtures/rootfs-git.erofs".into(),
        tsc_locked: true,
        enable_watchdog: true,
        enable_gpu_passthrough: false,
        driver_policy: DriverPolicy {
            driver: None,
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
    .storage(StoragePlaneConfig {
        idmap: IdmapConfig {
            guest_uid: 0,
            host_uid: 0,
        },
        shared_caches: vec![],
        lock_broker: false,
        synthetic_files: vec![],
        mask_read: vec![],
        encapsulation: WorkspaceEncapsulation::Passthrough,
        scan_engine: ReadMaskEngine::FastRegex,
        carryover_bytes: 1024,
    })
    .plugin_instance(GitGatePlugin)
    .boot()
    .await?;

    session.terminal.resize(120, 40)?;
    let slot = session.checkpoint.create_step(
        session.lineage(1),
        SnapshotDescriptor {
            id: "git-gate-base".into(),
            dirty_bytes: 0,
            created_at_ns: 0,
            parent_digest: None,
            guest_clock: None,
        },
    )?;
    let node = session.checkpoint.commit_step(session.lineage(1))?;
    eprintln!("[kelir] checkpoint slot={slot} node={node}");
    // TODO (L3): spawn git push --force origin main in the guest and gate it via GitGatePlugin.
    session.stop().await?;
    Ok(())
}
