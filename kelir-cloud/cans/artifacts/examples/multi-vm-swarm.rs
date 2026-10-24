//! examples/multi-vm-swarm.rs
//!
//! Cooperative agent swarm: N workers share a warm base microVM over a
//! host memfd, acquire repository locks via lock-broker, and exchange
//! peer streams over the mesh. The openshell-coordinator holds a hard
//! Fork veto until the shared fixture cache is hydrated.
//!
//! Mounts:     mesh, lock-broker, supply-chain, openshell-coordinator.
//! Primitives: map_shared_region, register_veto(Fork).

use std::time::Duration;

use serde::Deserialize;
use tokio::task::JoinSet;

use kelir::{
    DriverChoice, KelirBuilder, Session, VmConfig,
    plugin::VetoEvent,
    plugins::{
        lock_broker::{LockBrokerConfig, LockBrokerPlugin, LockMode, LockScope},
        mesh::{MeshConfig, MeshPlugin, PeerIdentity, TraceParentPropagation},
        openshell_coordinator::{
            CheckpointPolicy, CoordinatorConfig, CoordinatorPlugin, GpuBinding,
        },
        supply_chain::{
            LifecyclePolicy, PackageManager, SupplyChainConfig, SupplyChainPlugin,
        },
    },
};

#[derive(Clone, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
enum WorkerAction {
    RunShard { shard_id: u32, total_shards: u32, repo: String, base_sha: String },
}

const SWARM_REGION: &str = "swarm-coordination";
const SWARM_BYTES: usize = 32;

fn swarm_builder(coordinator: CoordinatorPlugin) -> KelirBuilder {
    KelirBuilder::new(VmConfig {
        vcpus: 4, memory_mb: 8192,
        kernel: "./fixtures/vmlinux-6.6-aarch64-kelir".into(),
        rootfs_erofs: "./fixtures/rootfs-swarm.erofs".into(),
        tsc_locked: true, enable_watchdog: true,
        enable_gpu_passthrough: true,
        driver: DriverChoice::Auto,
        entropy_seed: Some(0x5EED),
    })
    // Lock-broker: FUSE_SETLK on declared globs delegates to host flock.
    .plugin(LockBrokerPlugin::new(LockBrokerConfig {
        scope: LockScope::Globs(vec![
            "**/Cargo.lock".into(),
            "**/package-lock.json".into(),
        ]),
        default_mode: LockMode::Exclusive,
        non_blocking: true,
    }))
    // Mesh: peer vsock with mTLS + W3C traceparent propagation.
    .plugin(MeshPlugin::new(MeshConfig {
        peer_identity: PeerIdentity::Spiffe {
            trust_domain: "kelir.local".into(),
            workload_prefix: "swarm/worker".into(),
        },
        mtls: true,
        traceparent: TraceParentPropagation::InjectAndValidate,
        listeners: vec![
            kelir::plugins::mesh::PeerListener {
                port: 7001, service: "shard-exchange".into(),
            },
            kelir::plugins::mesh::PeerListener {
                port: 7002, service: "winner-announce".into(),
            },
        ],
    }))
    // Supply-chain: deny lifecycle scripts inside dependency directories.
    .plugin(SupplyChainPlugin::new(SupplyChainConfig {
        managers: vec![
            PackageManager::Npm,
            PackageManager::Pip,
            PackageManager::Cargo,
        ],
        policy: LifecyclePolicy::DenyInDependencyDirs,
        trap_on_ancestor_chain: true,
    }))
    .plugin(coordinator)
}

async fn boot_swarm() -> Result<Session, Box<dyn std::error::Error + Send + Sync>> {
    let coordinator = CoordinatorPlugin::new(CoordinatorConfig {
        tenant_id: "swarm-tenant-a".into(),
        checkpoint_policy: CheckpointPolicy {
            every_n_requests: 64, max_latency_ms: 250,
        },
        gpu: Some(GpuBinding::Cdi {
            device_ids: vec!["nvidia.com/gpu=0".into()],
            driver: "driver-rustvmm".into(),
        }),
    });

    // Register the Fork veto before boot. Without it, a worker that calls
    // fork() during cache hydration clones a half-warm page cache.
    let coord = coordinator.handle();
    let mut lifecycle = swarm_builder(coordinator).lifecycle_mut();
    lifecycle.register_veto(VetoEvent::Fork, Box::new(move || coord.cache_ready()));
    let session = lifecycle.seal().boot().await?;

    // Base VM hydrates caches. The veto blocks every child fork until
    // this returns; there is no flag the caller can flip early.
    session.exec_in_guest("bash", &[
        "-lc",
        "npm ci --prefer-offline && cargo fetch --locked \
         && pip install -q -r requirements.txt",
    ]).await?;
    session.coordinator_mark_cache_ready().await?;

    Ok(session)
}

async fn run_shard(
    parent: &Session,
    shard_id: u32,
    total: u32,
    region: &kelir::SharedMemoryHandle,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let child = parent.vm.fork().await?;

    // Zero-copy coordination: same host memfd, per-child guest phys addr.
    // Both the parent and every child observe the same bytes with no
    // vsock hop and no FUSE round-trip.
    let mapped = child.vm.map_shared_region(region, SWARM_REGION).await?;

    // Lock-broker intercepts the Cargo.lock write inside the child and
    // delegates to host flock; a conflicting writer sees EWOULDBLOCK,
    // not a stalled vCPU.
    let run = child.spawn("cargo")
        .args(["test", "--shard", &format!("{shard_id}/{total}"), "--locked"])
        .cwd("/workspace/swarm-repo")
        .params::<WorkerAction>(WorkerAction::RunShard {
            shard_id,
            total_shards: total,
            repo: "swarm-repo".into(),
            base_sha: "0000000000000000000000000000000000000000".into(),
        })
        .await?;

    if run.exit_code == 0 {
        // Mesh: peer-to-peer winner announcement on port 7002. The
        // traceparent flows from the winning shard's trap across the
        // swarm; the host ledger reconstructs one distributed trace.
        child.mesh.publish(&mapped, shard_id).await?;
    }

    child.terminate().await?;
    Ok(run.exit_code == 0)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let session = boot_swarm().await?;
    let region = session.vm.map_shared_region(
        &kelir::SharedMemoryHandle::named(SWARM_REGION, SWARM_BYTES),
        SWARM_REGION,
    ).await?;

    let mut joins = JoinSet::new();
    for shard_id in 0..8u32 {
        let parent = session.clone();
        let region = region.clone();
        joins.spawn(async move { run_shard(&parent, shard_id, 8, &region).await });
    }

    let mut passed = 0u32;
    while let Some(join) = joins.join_next().await {
        if join?? { passed += 1; }
    }

    let winner = session.mesh.await_winner(Duration::from_secs(2)).await?;
    let chain_denials = session.ledger.count_by_kind("supply-chain-deny").await?;
    eprintln!(
        "[kelir] {passed}/8 shards passed; winner={winner:?}; \
         supply-chain denials={chain_denials}"
    );

    session.terminate().await?;
    Ok(())
}
