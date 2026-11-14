#![allow(dead_code)]

// see: cans/08-lifecycle.md#CoW-fork

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

use serde::Deserialize;
use tokio::task::JoinSet;

use kelir::{
    HCons, HNil, Host, KelirBuilder, LedgerConfig, LedgerCtx, LedgerPlugin, MeshConfig, MeshCtx,
    MeshPlugin, NoParams, PackageGuardConfig, PeerIdentity, PeerListener, ReceiptConfig, Session,
    SnapshotDescriptor, SupplyChainConfig, SupplyChainCtx, SupplyChainPlugin,
    TraceParentPropagation, Unsealed, VmConfig, WxJitConfig,
    interface::driver_interface::{DriverPolicy, WxPolicy},
    planes::storage_plane::{LockBrokerConfig, LockScope},
    plugin_model::VetoEvent,
};

type SwarmPlugins = HCons<LedgerCtx, HCons<SupplyChainCtx, HCons<MeshCtx, HNil>>>;

// see: cans/03-sdk-architecture.md#.params(model:-type[P])-pins-Params;-handler-c.params-is-typed
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

fn swarm_builder() -> KelirBuilder<Host, Unsealed, (), HNil, SwarmPlugins, NoParams> {
    KelirBuilder::new(VmConfig {
        vcpus: 4,
        memory_mb: 8192,
        kernel_path: "./fixtures/vmlinux-6.6-aarch64-kelir".into(),
        rootfs_erofs_path: "./fixtures/rootfs-swarm.erofs".into(),
        tsc_locked: true,
        enable_watchdog: true,
        enable_gpu_passthrough: true,
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
        entropy_seed: Some(0x5EED),
    })
    .lock_broker(LockBrokerConfig {
        scope: LockScope::Globs(vec!["**/Cargo.lock".into(), "**/package-lock.json".into()]),
        non_blocking: true,
    })
    // see: cans/plugins/mesh.md#Plugin-mesh
    .plugin_instance(MeshPlugin::new(MeshConfig {
        peer_identity: PeerIdentity::Spiffe {
            trust_domain: "kelir.local".into(),
            workload_prefix: "swarm/worker".into(),
        },
        mtls: true,
        traceparent: TraceParentPropagation::InjectAndValidate,
        listeners: vec![
            PeerListener {
                port: 7001,
                service: "shard-exchange".into(),
            },
            PeerListener {
                port: 7002,
                service: "winner-announce".into(),
            },
        ],
    }))
    // see: cans/plugins/supply-chain.md#Plugin-supply-chain
    .plugin_instance(SupplyChainPlugin::new(SupplyChainConfig {
        wx_jit: WxJitConfig::default(),
        package_guard: PackageGuardConfig {
            managers: vec!["npm".into(), "pip".into(), "cargo".into()],
            ..PackageGuardConfig::default()
        },
    }))
    // see: cans/plugins/ledger.md#Plugin-ledger
    .plugin_instance(LedgerPlugin::new(true).with_config(LedgerConfig {
        store: ".kelir/swarm-audit.merkle.db".into(),
        merkle_chain: true,
        suppress_local_db: false,
        otel: None,
        receipts: ReceiptConfig {
            include_syscall_count: false,
            ..ReceiptConfig::default()
        },
    }))
}

async fn boot_swarm(
) -> Result<Session<SwarmPlugins, NoParams>, Box<dyn std::error::Error + Send + Sync>> {
    let cache_ready = Arc::new(AtomicBool::new(false));
    let veto_flag = cache_ready.clone();
    let mut builder = swarm_builder();
    builder.lifecycle_mut().register_veto(
        VetoEvent::Fork,
        Box::new(move || veto_flag.load(Ordering::SeqCst)),
    );
    let session = builder.seal()?.boot().await?;

    session.terminal.resize(120, 40)?;
    session.checkpoint.create_step(
        session.lineage(1),
        SnapshotDescriptor {
            id: "swarm-cache-ready".into(),
            dirty_bytes: 0,
            created_at_ns: 0,
            parent_digest: None,
            guest_clock: None,
        },
    )?;
    session.checkpoint.commit_step(session.lineage(1))?;
    cache_ready.store(true, Ordering::SeqCst);

    Ok(session)
}

async fn run_shard(
    mut child: Session<SwarmPlugins, NoParams>,
    shard_id: u32,
    total: u32,
) -> Result<(u32, i32), Box<dyn std::error::Error + Send + Sync>> {
    child.checkpoint.create_step(
        child.lineage(1),
        SnapshotDescriptor {
            id: format!("swarm-shard-{shard_id}/{total}"),
            dirty_bytes: 0,
            created_at_ns: 0,
            parent_digest: None,
            guest_clock: None,
        },
    )?;
    child.checkpoint.commit_step(child.lineage(1))?;
    // TODO (L3): map the shard-exchange shared region, spawn cargo test --shard, and publish the result via MeshCtx.
    child.stop().await?;
    Ok((shard_id, 0))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut session = boot_swarm().await?;

    let mut joins = JoinSet::new();
    for shard_id in 0..8u32 {
        let child = session.fork().await?;
        joins.spawn(async move { run_shard(child, shard_id, 8).await });
    }

    let mut passed = 0u32;
    let mut winner: Option<u32> = None;
    while let Some(join) = joins.join_next().await {
        let (shard_id, code) = join??;
        if code == 0 {
            passed += 1;
            if winner.is_none() {
                winner = Some(shard_id);
            }
        }
    }

    match session
        .ctx::<MeshCtx, _>()
        .await_winner(Duration::from_secs(2))
        .await
    {
        Ok(mesh_winner) => eprintln!("[kelir] mesh winner shard={mesh_winner}"),
        Err(err) => eprintln!("[kelir] mesh await_winner pending (L3): {err}"),
    }
    eprintln!("[kelir] {passed}/8 shards passed; winner shard={winner:?}");
    let denials = session
        .ctx::<LedgerCtx, _>()
        .count_denials(std::process::id())
        .await?;
    eprintln!("[kelir] supply-chain denials={denials}");

    session.stop().await?;
    Ok(())
}
