// see: cans/plugins/db-proxy.md#Plugin-db-proxy
// see: cans/plugins/ledger.md#Plugin-ledger
// see: cans/plugins/transaction.md#Plugin-transaction

use std::env;

use serde::Deserialize;

use kelir::{
    DbProxyConfig, DbProxyPlugin, FinalizeRequest, HCons, HNil, HandlerCtx, Host, KelirBuilder,
    LedgerConfig, LedgerCtx, LedgerPlugin, NoParams, PostExecAction, ReceiptConfig,
    SqlProxyCtx, TransactionConfig, TransactionCtx, TransactionPlugin, TrapFilter, Unsealed,
    Verdict, VmConfig,
    driver_interface::{DriverPolicy, WxPolicy},
    interface::telemetry::CausalDelta,
    plugin_model::{KelirPlugin, PluginError},
};

// see: cans/03-sdk-architecture.md#.params(model:-type[P])-pins-Params;-handler-c.params-is-typed
#[derive(Clone, Deserialize)]
struct MigrateParams {
    action: String,
    schema_name: String,
    target_version: u32,
    dry_run: bool,
}

const LOCAL_SNAPSHOT: &str = "./fixtures/migrate-snapshot.erofs";

fn build_runtime(
    pg_host: &str,
    _pg_secret: &str,
) -> KelirBuilder<
    Host,
    Unsealed,
    (),
    HNil,
    HCons<LedgerCtx, HCons<TransactionCtx, HCons<SqlProxyCtx, HNil>>>,
    NoParams,
> {
    // see: cans/plugins/db-proxy.md#L7-pgwire-rewrite-and-2PC-commit
    let mut db_config = DbProxyConfig::default();
    db_config.postgres.listen_port = 5432;
    db_config.postgres.upstream = format!("{pg_host}:5432");
    db_config.postgres.rewrite_rules = vec![("public.".into(), "migrate_schema.".into())];
    db_config.sqlite.guest_socket = "/run/kelir/db.sock".into();
    db_config.sqlite.host_path = "/var/lib/app/migrations.sqlite".into();
    // TODO: the ephemeral credentials (vault://pg/app_migrate, 300s) are not set: EphemeralCredentials is not re-exported by the kelir facade and has no Default, so the port cannot name it.
    let db_plugin = DbProxyPlugin::new().with_config(db_config);

    // see: cans/plugins/ledger.md#Merkle-WAL
    let ledger_plugin = LedgerPlugin::new(true).with_config(LedgerConfig {
        store: "sqlite://.kelir/migrate-audit.merkle.db".into(),
        merkle_chain: true,
        suppress_local_db: false,
        otel: None,
        receipts: ReceiptConfig {
            include_dirty_bytes: false,
            include_binary_blake3: false,
            include_syscall_count: false,
            include_wall_clock_ns: false,
            include_phase_timings: false,
        },
    });
    let ledger_ctx = ledger_plugin.context();

    // see: cans/plugins/transaction.md#Two-phase-commit-demonstration
    let tx_plugin = TransactionPlugin::new(TransactionConfig {
        timeout_ms: 30000,
        auto_rollback_on_taint: true,
        flush_fuse_before_commit: true,
    });

    KelirBuilder::new(VmConfig {
        vcpus: 2,
        memory_mb: 2048,
        kernel_path: format!("./fixtures/vmlinux-6.6-{}-kelir", std::env::consts::ARCH).into(),
        rootfs_erofs_path: "./fixtures/rootfs-migrate.erofs".into(),
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
    .plugin_instance(db_plugin)
    // Transaction mounts directly after DbProxy: Satisfies is positional, so SqlProxyCtx must head Plugins at that mount; see cans/03-sdk-architecture.md#Rust-SDK.
    .plugin_instance(tx_plugin)
    .plugin_instance(ledger_plugin)
    // see: cans/plugins/db-proxy.md#Plugin-db-proxy
    .trap_async(TrapFilter::binary("migrate"), |_c: HandlerCtx<_, _>| async move {
        // TODO (L3): drop unless the db-proxy rewrite target is "migrate_schema."; SqlProxyCtx exposes no rewrite_target accessor, so the check cannot read the live target yet.
        Ok(Verdict::Pass)
    })
    .post_exec(TrapFilter::binary("migrate"), |c: HandlerCtx<_, _>| async move {
        let receipt = c.receipt().await?;
        if receipt
            .extensions
            .iter()
            .any(|e| e.namespace == "net:l7:sql")
        {
            Ok(PostExecAction::Proceed)
        } else {
            Ok(PostExecAction::RollbackAndHalt)
        }
    })
    // see: cans/plugins/transaction.md#Plugin-transaction
    .on_pre_commit({
        let ledger = ledger_ctx.clone();
        move |_ctx| {
            let ledger = ledger.clone();
            Box::pin(async move { ledger.flush().await })
        }
    })
    .on_post_commit(|_ctx, committed| {
        Box::pin(async move {
            if committed {
                eprintln!(
                    "[kelir] commit: per-entry leaf commit, staged snapshot is next cold-start fixture"
                );
            } else {
                eprintln!(
                    "[kelir] commit aborted: staging unlinked, broker upper and tombstones dropped, last good snapshot kept"
                );
            }
            Ok::<(), PluginError>(())
        })
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let pg_host = env::var("KELIR_PG_HOST")?;
    let pg_secret = env::var("KELIR_PG_SECRET")?;

    let mut session = build_runtime(&pg_host, &pg_secret).seal()?.boot().await?;

    // see: cans/09-snapshots.md#Snapshots
    let snapshot = tokio::fs::read(LOCAL_SNAPSHOT).await?;
    session.vm().import_snapshot(&snapshot).await?;

    // TODO (L3): open the transaction scope "migrate-42" over /workspace/migrations + migrate_schema, savepoint before the migrate, and roll back to it on failure; TransactionCtx exposes no scope/savepoint API yet.
    let params = MigrateParams {
        action: "migrate".into(),
        schema_name: "migrate".into(),
        target_version: 42,
        dry_run: false,
    };
    eprintln!(
        "[kelir] migrate action={} schema={} target={} dry_run={}",
        params.action, params.schema_name, params.target_version, params.dry_run
    );
    let run = session
        .spawn(
            "python",
            &["-m", "app.migrate", "--schema", params.schema_name.as_str()],
        )
        .await?;
    if run.exit_code != 0 {
        return Err(format!("migration failed: {}", run.exit_code).into());
    }

    let out = session.vm().export_snapshot().await?;
    tokio::fs::write(LOCAL_SNAPSHOT, out).await?;

    // see: cans/10-attestation.md#Execution-receipts
    // TODO (L3): pass the live causal delta; the session facade exposes no get_causal_delta_from_parent yet, so the request carries a zero delta.
    let ledger = session.ctx::<LedgerCtx, _>();
    let final_receipt = ledger
        .finalize(FinalizeRequest {
            include_delta: CausalDelta::new(0, vec![], vec![], 0, [0; 32], [0; 32]),
            include_merkle_root: true,
        })
        .await?;
    eprintln!(
        "[kelir] merkle={} jws={}…",
        final_receipt.merkle_root,
        final_receipt
            .signed_jws
            .get(..64)
            .unwrap_or(&final_receipt.signed_jws)
    );

    session.stop().await?;
    Ok(())
}
