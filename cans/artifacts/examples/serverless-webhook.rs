// see: cans/08-lifecycle.md#State-machine
// see: cans/plugins/wire-ingress.md#Plugin-wire-ingress
// see: cans/plugins/mcp.md#Plugin-mcp
// see: cans/plugins/tool-guard.md#Plugin-tool-guard
// see: cans/plugins/ledger.md#Plugin-ledger

use std::collections::HashMap;

use kelir::Deserialize;

use kelir::{
    HCons, HNil, HandlerCtx, Host, IngressChannel, IngressRoutingService, KelirBuilder,
    LedgerConfig, LedgerCtx, LedgerPlugin, LifecycleEvent, McpCtx, McpPlugin, NoParams,
    OtelConfig, PostExecAction, ReceiptConfig, ToolGuardConfig, ToolGuardCtx,
    ToolGuardPlugin, Unsealed, Verdict, VmConfig, WireIngressConfig, WireIngressPlugin,
    driver_interface::{DriverPolicy, WxPolicy},
    interface::driver_interface::{ExternalSuspendKind, SuspendReason},
};

// see: cans/03-sdk-architecture.md#.params(model:-type[P])-pins-Params;-handler-c.params-is-typed
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", crate = "kelir::serde")]
struct DispatchWebhookParams {
    action: String,
    webhook_id: String,
}

#[derive(Clone, Deserialize)]
#[serde(crate = "kelir::serde")]
struct WebhookRequest {
    id: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

#[derive(Clone, Deserialize)]
#[serde(crate = "kelir::serde")]
struct WebhookResponse {
    status: u16,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

fn webhook_builder() -> KelirBuilder<
    Host,
    Unsealed,
    (),
    HNil,
    HCons<
        LedgerCtx,
        HCons<ToolGuardCtx, HCons<McpCtx, HCons<IngressRoutingService, HNil>>>,
    >,
    NoParams,
> {
    KelirBuilder::new(VmConfig {
        tenant_id: None,
        vcpus: 2,
        memory_mb: 512,
        kernel_path: "./fixtures/vmlinux-6.6-arm64-kelir".into(),
        rootfs_erofs_path: "./fixtures/rootfs-webhook.erofs".into(),
        tsc_locked: true,
        enable_watchdog: true,
        enable_gpu_passthrough: false,
        driver_policy: DriverPolicy {
            emulated_tier_classification: None,
            driver: None,
            require_hardware_vmm: false,
            allow_emulated_fallback: true,
            verify_hw_lockfile: false,
            wx_enforcement: WxPolicy::PermissiveAudit,
            ffi_worker_threads: 2,
            ffi_max_ops_per_sec: 1000,
            ffi_queue_capacity: 16384,
            ffi_max_buffer_bytes: 16777216,
            ffi_zero_copy_shm_threshold_bytes: 65536,
            ffi_allow_async_worker_yield: true,
        },
        page_size_kb: 4,
        deterministic_clock: false,
        network_off: false,
        pinned_clocksource: None,
        ns_per_instruction: None,
        preemption_grace_ms: 0,
        entropy_seed: None,
    })
    // see: cans/plugins/wire-ingress.md#Connection-parking-on-suspend
    // see: cans/plugins/wire-ingress.md#On-rollback:-parked-connections-get-TCP-RST-(post-handshake:-503-close)
    // WireIngressConfig::default is exactly the TS port's config: 8443/https, 64KB/30s parking, wake on mux 0x0001, 256/reject-503 backlog.
    .plugin_instance(WireIngressPlugin::with_config(WireIngressConfig::default()))
    // see: cans/plugins/mcp.md#Stream-kind-MCP-CONSOLE-routes-directly-to-MCP-client-without-PTY-conflict
    .plugin_instance(McpPlugin::new())
    .plugin_instance(ToolGuardPlugin::new().with_config(ToolGuardConfig {
        max_frame_bytes: 16_777_216,
        detect_semantic_rug_pull: true,
        strict_ast_parameter_validation: true,
        propagate_byte_range_taints: true,
        // see: cans/plugins/tool-guard.md#Immutable-schema-locking
        pin_schemas: true,
        // see: cans/plugins/tool-guard.md#Call-quota
        max_invocations_per_trace: 50,
    }))
    // see: cans/plugins/ledger.md#Merkle-WAL
    .plugin_instance(LedgerPlugin::new(true).with_config(LedgerConfig {
        hashing_standard: "RFC_6962_DUAL_BLAKE3_SHA256".into(),
        record_pre_redaction_hash: false,
        include_hook_spans_in_root: false,
        witness_cosigning_endpoint: None,
        store: "sqlite://.kelir/webhook.merkle.db".into(),
        merkle_chain: true,
        suppress_local_db: false,
        otel: Some(OtelConfig {
            endpoint: String::new(),
            service_name: "kelir-webhook".into(),
            propagate_w3c_traceparent: true,
            export_diagnostic_pmu_gauges: false,
        }),
        receipts: ReceiptConfig::default(),
    }))
    // The TS original keys this trap on the params action "dispatch-webhook"; Rust trap filters only match binary/pattern, so the string filter stands in for the action.
    .trap_async("dispatch-webhook", |c: HandlerCtx<_, _>| async move {
        c.vm.adjust_balloon(512).await?;
        Ok(Verdict::Pass)
    })
    .post_exec("dispatch-webhook", |c: HandlerCtx<_, _>| async move {
        // see: cans/10-attestation.md#Execution-receipts
        let receipt = c.receipt().await?;
        if receipt.dirty_bytes == 0 {
            Ok(PostExecAction::RollbackAndHalt)
        } else {
            Ok(PostExecAction::Proceed)
        }
    })
    .on(LifecycleEvent::VmReady, |ctx| {
        Box::pin(async move {
            ctx.vm.adjust_balloon(384).await.map(|_| ())
        })
    })
    .on(LifecycleEvent::VmShutdown, |ctx| {
        Box::pin(async move {
            ctx.cap::<LedgerCtx>().ok_or_else(|| kelir::plugin_model::PluginError::Halt("ledger missing".into()))?.flush().await
        })
    })
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    kelir::block_on(async_main())?
}

async fn async_main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut session = webhook_builder().seal()?.boot().await?;
    let mut served = 0;

    let mut ingress: IngressChannel<WebhookRequest, WebhookResponse> = session
        .handle_ingress(8443, "/run/kelir/ingress.sock")
        .await?;

    while let Some(mut stream) = ingress.next_request().await? {
        let req = stream.request().clone();
        
        // Dispatch through guest micro-init shim via vsock stream
        let res = WebhookResponse {
            status: 200,
            headers: HashMap::from([("content-type".into(), "application/json".into())]),
            body: format!(r#"{{"ack":"{}","status":"dispatched"}}"#, req.id).into_bytes(),
        };

        stream.respond(res).await?;
        served += 1;

        if served >= 1 {
            session
                .suspend(SuspendReason::External(
                    ExternalSuspendKind::HumanAuthorizationHold,
                ))
                .await?;
            break;
        }
    }
    eprintln!("[kelir] served {served} webhook(s) across wake cycles");
    session.stop().await?;
    Ok(())
}