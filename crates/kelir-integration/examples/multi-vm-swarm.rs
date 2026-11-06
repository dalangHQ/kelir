#![allow(dead_code)]

// see: cans/08-lifecycle.md#CoW-fork

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use serde::Deserialize;
use tokio::task::JoinSet;

use kelir::{
    KelirBuilder, Session, VmConfig,
    driver::{DriverPolicy, WxPolicy},
    planes::storage::{LockBrokerConfig, LockMode, LockScope},
    plugin::VetoEvent,
    plugins::{
        ledger::{LedgerConfig, LedgerCtx, LedgerPlugin, ReceiptConfig},
        mesh::{MeshConfig, MeshCtx, MeshPlugin, PeerIdentity, TraceParentPropagation},
        supply_chain::{LifecyclePolicy, PackageManager, SupplyChainConfig, SupplyChainPlugin},
    },
};

#[derive(Clone, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
enum WorkerAction {
    RunShard {
        shard_id: u32,
        total_shards: u32,
        repo: String,
        base_sha: String,
    },
}

const SWARM_REGION: &str = "swarm-coordination";
const SWARM_BYTES: usize = 32;

fn swarm_builder() -> KelirBuilder {
    KelirBuilder::new(VmConfig {
        vcpus: 4,
        memory_mb: 8192,
        kernel_path: "./fixtures/vmlinux-6.6-aarch64-kelir".into(),
        rootfs_erofs_path: "./fixtures/rootfs-swarm.erofs".into(),
        tsc_locked: true,
        enable_watchdog: true,
        enable_gpu_passthrough: true,
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
        entropy_seed: Some(0x5EED),
    })
    .lock_broker(LockBrokerConfig {
        scope: LockScope::Globs(vec!["**/Cargo.lock".into(), "**/package-lock.json".into()]),
        default_mode: LockMode::Exclusive,
        non_blocking: true,
    })
    .plugin(MeshPlugin::new(MeshConfig {
        peer_identity: PeerIdentity::Spiffe {
            trust_domain: "kelir.local".into(),
            workload_prefix: "swarm/worker".into(),
        },
        mtls: true,
        traceparent: TraceParentPropagation::InjectAndValidate,
        listeners: vec![
            kelir::plugins::mesh::PeerListener {
                port: 7001,
                service: "shard-exchange".into(),
            },
            kelir::plugins::mesh::PeerListener {
                port: 7002,
                service: "winner-announce".into(),
            },
        ],
    }))
    .plugin(SupplyChainPlugin::new(SupplyChainConfig {
        managers: vec![
            PackageManager::Npm,
            PackageManager::Pip,
            PackageManager::Cargo,
        ],
        policy: LifecyclePolicy::DenyInDependencyDirs,
    }))
    .plugin(LedgerPlugin::new(LedgerConfig {
        store: ".kelir/swarm-audit.merkle.db".into(),
        merkle_chain: true,
        otel: None,
        receipts: ReceiptConfig {
            include_syscall_count: false,
        },
    }))
}

async fn boot_swarm() -> Result<Session, Box<dyn std::error::Error + Send + Sync>> {
    let cache_ready = Arc::new(AtomicBool::new(false));
    let veto_flag = cache_ready.clone();
    let mut builder = swarm_builder();
    builder.lifecycle_mut().register_veto(
        VetoEvent::Fork,
        Box::new(move || veto_flag.load(Ordering::SeqCst)),
    );
    let session = builder.seal().boot().await?;

    session
        .exec_in_guest(
            "bash",
            &[
                "-lc",
                "npm ci --prefer-offline && cargo fetch --locked \
         && pip install -q -r requirements.txt",
            ],
        )
        .await?;
    cache_ready.store(true, Ordering::SeqCst);

    Ok(session)
}

async fn run_shard(
    parent: &Session,
    shard_id: u32,
    total: u32,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let child = parent.vm.fork().await?;

    let mapped = child
        .vm
        .map_shared_region(SWARM_REGION, SWARM_BYTES)
        .await?;

    let run = child
        .spawn("cargo")
        .args([
            "test",
            "--shard",
            &format!("{shard_id}/{total}"),
            "--locked",
        ])
        .cwd("/workspace/swarm-repo")
        .params::<WorkerAction>(WorkerAction::RunShard {
            shard_id,
            total_shards: total,
            repo: "swarm-repo".into(),
            base_sha: "0000000000000000000000000000000000000000".into(),
        })
        .await?;

    if run.exit_code == 0 {
        child.ctx::<MeshCtx>().publish(&mapped, shard_id).await?;
    }

    child.terminate().await?;
    Ok(run.exit_code == 0)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let session = boot_swarm().await?;

    let mut joins = JoinSet::new();
    for shard_id in 0..8u32 {
        let parent = session.clone();
        joins.spawn(async move { run_shard(&parent, shard_id, 8).await });
    }

    let mut passed = 0u32;
    while let Some(join) = joins.join_next().await {
        if join?? {
            passed += 1;
        }
    }

    let winner = session
        .ctx::<MeshCtx>()
        .await_winner(Duration::from_secs(2))
        .await?;
    let chain_denials = session
        .ctx::<LedgerCtx>()
        .count_by_kind("supply-chain-deny")
        .await?;
    eprintln!(
        "[kelir] {passed}/8 shards passed; winner={winner:?}; \
         supply-chain denials={chain_denials}"
    );

    session.terminate().await?;
    Ok(())
}
