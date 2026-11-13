#![allow(dead_code)]

// see: cans/02-plugin-model.md#Unified-builder
// see: cans/03-sdk-architecture.md#Unified-builder-surface
// see: cans/artifacts/examples/fluent-builder-chaos.rs#build_chaos_plugin
// see: cans/artifacts/examples/unified-builder-surface.ts#buildSurfacePlugin
// see: cans/artifacts/examples/unified-builder-surface.py#build_surface_plugin

// The unified builder surface, walked end to end in one file: structural
// verbs, all 24 ring verbs flat, all 8 scope closures, the four chain cases
// (seal, trap filter, trap_scope, lock_broker) and the four mount cases
// (host-only verbs on a host builder, host-only verbs on a plugin builder,
// host.use(builder) merge ordering, nested-scope = flat-chain equivalence).
// Handlers are trivial stubs: Forward for filter verbs, Pass for verdict
// verbs, passthrough bytes for console/exec, Ok(()) for lifecycle and
// telemetry. L1/L2: descriptors only, no execution, no boot().

use async_trait::async_trait;
use kelir_interface::driver_interface::Stream;
use kelir_interface::plugin_model::HandlerCtx;
use kelir_interface::vsock_mesh::{VsockPeer, VsockServiceHandler, VsockServiceSpec};

use kelir::{
    HCons, HNil, Host, KelirBuilder, LedgerCtx, LedgerPlugin, NoParams, Plugin, TrapFilter,
    Unsealed, VmConfig,
    driver_interface::{DriverPolicy, WxPolicy},
    planes::{
        storage_plane::{
            IdmapConfig, LockBrokerConfig, LockScope, ReadMaskEngine, StoragePlaneConfig,
            WorkspaceEncapsulation,
        },
        terminal_plane::PtyConfig,
        wire_plane::{SinkholeConfig, TapFilter, WireConfig},
    },
    plugin_model::{
        AnyPlugin, FilterAction, MountScope, PluginError, Verdict, WatchdogVerdict,
    },
};

// see: cans/02-plugin-model.md#Unified-builder
#[derive(Clone)]
pub struct SurfaceCtx;

// see: cans/02-plugin-model.md#Unified-builder
pub type SurfaceBuilder =
    KelirBuilder<Plugin, Unsealed, SurfaceCtx, HCons<LedgerCtx, HNil>, HNil, NoParams>;

// see: cans/02-plugin-model.md#Unified-builder
type SurfaceHostBuilder = KelirBuilder<Host, Unsealed, (), HNil, HCons<LedgerCtx, HNil>, NoParams>;

// see: cans/11-protocol.md#Control-channel
struct EchoVsock;

// see: cans/11-protocol.md#Control-channel
#[async_trait]
impl VsockServiceHandler for EchoVsock {
    async fn on_connect(
        &self,
        _peer: &VsockPeer,
        _stream: Box<dyn Stream>,
        _ctx: &HandlerCtx,
    ) -> Result<(), PluginError> {
        Ok(())
    }
}

// see: cans/02-plugin-model.md#Unified-builder
// Structural verbs: context, require, scope, fork_with, vsock_service.
// Then the 24 ring verbs flat, then the 8 scope closures.
pub fn build_surface_plugin() -> SurfaceBuilder {
    KelirBuilder::named("surface")
        .context::<SurfaceCtx>(|| SurfaceCtx)
        .require::<LedgerPlugin>()
        .scope(MountScope::default().with_paths(["/workspace"]))
        .fork_with(|_id| Box::new(build_surface_plugin()) as Box<dyn AnyPlugin>)
        .vsock_service(
            VsockServiceSpec {
                port: 0x0013,
                service: "echo".to_string(),
                schema: None,
            },
            EchoVsock,
        )
        .on_trap(|_, _| Ok(Verdict::Pass))
        .on_process_exit(|_, _| Ok(()))
        .on_fs_lookup(|_| Ok(Verdict::Pass))
        .on_fs_read(|_, data| Ok(data.to_vec()))
        .on_fs_write(|_, _, _, _| Ok(FilterAction::Forward))
        .on_fs_lock(|_, _, _, _| Ok(Verdict::Pass))
        .on_net_outbound(|_| Ok(FilterAction::Forward))
        .on_net_inbound(|_| Ok(FilterAction::Forward))
        .on_console_input(|_, bytes| Ok(bytes.to_vec()))
        .on_console_output(|_, bytes| Ok(bytes.to_vec()))
        .on_exec_stdout(|_, _, bytes| Ok(bytes.to_vec()))
        .on_exec_stderr(|_, _, bytes| Ok(bytes.to_vec()))
        .on_host_lsm_exec(|_, _| Ok(Verdict::Pass))
        .on_host_pressure(|_| Ok(Verdict::Pass))
        .on_host_keyring_access(|_| Ok(Verdict::Pass))
        .on_vm_init(|_| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_vm_ready(|_| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_vm_shutdown(|_| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_pre_commit(|_| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_post_commit(|_, _| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_host_preempt(|_| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_memory_pressure(|_, _| async move { Ok::<(), PluginError>(()) })
        .on_heartbeat(|_| async move { Ok::<(), PluginError>(()) })
        .on_watchdog(|_| async move {
            Ok::<WatchdogVerdict, PluginError>(WatchdogVerdict::ExecuteDefault)
        })
        .trap_scope(|t| {
            t.on_trap(|_, _| Ok(Verdict::Pass))
                .on_process_exit(|_, _| Ok(()))
                .end()
        })
        .fs(|f| {
            f.on_fs_lookup(|_| Ok(Verdict::Pass))
                .on_fs_read(|_, data| Ok(data.to_vec()))
                .on_fs_write(|_, _, _, _| Ok(FilterAction::Forward))
                .on_fs_lock(|_, _, _, _| Ok(Verdict::Pass))
                .end()
        })
        .net(|n| {
            n.on_net_outbound(|_| Ok(FilterAction::Forward))
                .on_net_inbound(|_| Ok(FilterAction::Forward))
                .end()
        })
        .console(|c| {
            c.on_console_input(|_, bytes| Ok(bytes.to_vec()))
                .on_console_output(|_, bytes| Ok(bytes.to_vec()))
                .end()
        })
        .exec(|e| {
            e.on_exec_stdout(|_, _, bytes| Ok(bytes.to_vec()))
                .on_exec_stderr(|_, _, bytes| Ok(bytes.to_vec()))
                .end()
        })
        .host(|h| {
            h.on_host_lsm_exec(|_, _| Ok(Verdict::Pass))
                .on_host_pressure(|_| Ok(Verdict::Pass))
                .on_host_keyring_access(|_| Ok(Verdict::Pass))
                .on_host_preempt(|_| Box::pin(async move { Ok::<(), PluginError>(()) }))
                .end()
        })
        .lifecycle(|l| {
            l.on_vm_init(|_| Box::pin(async move { Ok::<(), PluginError>(()) }))
                .on_vm_ready(|_| Box::pin(async move { Ok::<(), PluginError>(()) }))
                .on_vm_shutdown(|_| Box::pin(async move { Ok::<(), PluginError>(()) }))
                .on_pre_commit(|_| Box::pin(async move { Ok::<(), PluginError>(()) }))
                .on_post_commit(|_, _| Box::pin(async move { Ok::<(), PluginError>(()) }))
                .end()
        })
        .telemetry(|t| {
            t.on_memory_pressure(|_, _| async move { Ok::<(), PluginError>(()) })
                .on_heartbeat(|_| async move { Ok::<(), PluginError>(()) })
                .on_watchdog(|_| async move {
                    Ok::<WatchdogVerdict, PluginError>(WatchdogVerdict::ExecuteDefault)
                })
                .end()
        })
}

// see: cans/02-plugin-model.md#Unified-builder
// The merge-ordering second mount: no requires, one trivial verb.
pub fn build_plain_plugin() -> KelirBuilder<Plugin, Unsealed, (), HNil, HNil, NoParams> {
    KelirBuilder::named("plain").on_heartbeat(|_| async move { Ok::<(), PluginError>(()) })
}

// see: cans/02-plugin-model.md#Unified-builder
// Mount case 1: host-only verbs on a host builder (terminal, storage,
// wire, detached, lock_broker). Chain cases: trap filter, trap_scope,
// lock_broker, then seal() in main. Mount case 3: merge ordering — the
// ledger trait instance first (it satisfies the surface plugin's require
// at compile time via Satisfies), then the surface plugin via plugin(),
// then the plain plugin via use(), the same verb under its second name.
pub fn build_host() -> SurfaceHostBuilder {
    KelirBuilder::new(VmConfig {
        vcpus: 2,
        memory_mb: 2048,
        kernel_path: format!("./fixtures/vmlinux-6.6-{}-kelir", std::env::consts::ARCH).into(),
        rootfs_erofs_path: "./fixtures/rootfs-surface.erofs".into(),
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
    .terminal(PtyConfig {
        cols: 120,
        rows: 40,
        drop_osc52: true,
        strip_title: true,
        token_shield: None,
        prompts: Vec::new(),
        stream_engine: "StatelessRegex".into(),
    })
    .storage(StoragePlaneConfig {
        idmap: IdmapConfig {
            guest_uid: 0,
            host_uid: 0,
        },
        shared_caches: Vec::new(),
        lock_broker: true,
        synthetic_files: Vec::new(),
        mask_read: Vec::new(),
        encapsulation: WorkspaceEncapsulation::Passthrough,
        scan_engine: ReadMaskEngine::FastRegex,
        carryover_bytes: 1024,
    })
    .wire(WireConfig {
        sinkhole: SinkholeConfig {
            allowlist: Vec::new(),
            drop_all: true,
        },
        ssh_agent_bridge: false,
        vaults: Vec::new(),
        l4_proxies: Vec::new(),
        tap_filter: TapFilter::AllowUdpProxied,
    })
    .detached(false)
    .plugin_instance(LedgerPlugin::new(true))
    .plugin(build_surface_plugin())
    .r#use(build_plain_plugin())
    .trap(TrapFilter::binary("surface-allow"), |_, _| {
        Ok(Verdict::Pass)
    })
    .trap_scope(|t| {
        t.on_trap(|_, _| Ok(Verdict::Pass))
            .on_process_exit(|_, _| Ok(()))
            .end()
    })
    .lock_broker(LockBrokerConfig {
        scope: LockScope::Globs(vec![
            "**/Cargo.lock".into(),
            "**/package-lock.json".into(),
        ]),
        non_blocking: true,
    })
}

// see: cans/02-plugin-model.md#Unified-builder
// Mount case 2: host-only verbs on a plugin builder. In Rust the rejection
// is a compile error, not a runtime EConfig: the verbs below exist only on
// the Host-mode impl, so there is no method to call on a plugin-mode
// builder. This function is an L2 skeleton that returns its builder
// unchanged; it proves nothing at runtime and must not be read as a pass.
// TODO (L3): no runtime probe exists for this case by construction: uncommenting any line below must fail the build with E0599 (no method named `<verb>`); a live EConfig probe like the TS/Py siblings is impossible in Rust.
pub fn host_only_verbs_rejected_on_plugin(
    plugin: KelirBuilder<Plugin, Unsealed, (), HNil, HNil, NoParams>,
) -> KelirBuilder<Plugin, Unsealed, (), HNil, HNil, NoParams> {
    // plugin.terminal(PtyConfig { cols: 120, rows: 40, drop_osc52: true, strip_title: true, token_shield: None, prompts: Vec::new(), stream_engine: "StatelessRegex".into() });
    // plugin.storage(StoragePlaneConfig { idmap: IdmapConfig { guest_uid: 0, host_uid: 0 }, shared_caches: Vec::new(), lock_broker: true, synthetic_files: Vec::new(), mask_read: Vec::new(), encapsulation: WorkspaceEncapsulation::Passthrough, scan_engine: ReadMaskEngine::FastRegex, carryover_bytes: 1024 });
    // plugin.wire(WireConfig { sinkhole: SinkholeConfig { allowlist: Vec::new(), drop_all: true }, ssh_agent_bridge: false, vaults: Vec::new(), l4_proxies: Vec::new(), tap_filter: TapFilter::AllowUdpProxied });
    // plugin.lock_broker(LockBrokerConfig { scope: LockScope::Globs(vec!["**/Cargo.lock".into()]), non_blocking: true });
    // plugin.detached(true);
    // plugin.seal();
    // plugin.params::<()>();
    // plugin.plugin(build_plain_plugin());
    // plugin.r#use(build_plain_plugin());
    // plugin.plugin_instance(LedgerPlugin::new(true));
    // plugin.trap_async("*", |_| async move { Ok::<Verdict, PluginError>(Verdict::Pass) });
    // plugin.post_exec("*", |_| async move { Ok::<PostExecAction, PluginError>(PostExecAction::Proceed) });
    // plugin.on_trap_deny(|_, _| async move { Ok::<(), PluginError>(()) });
    // plugin.register_custom_action("quarantine", serde_json::json!({}), |_| async move { Ok::<(), Box<dyn std::error::Error + Send + Sync>>(()) });
    // plugin.register_stream_kind(0x0013, serde_json::json!({}), |_| async move { Ok::<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>(Vec::new()) });
    // plugin.lifecycle_mut();
    plugin
}

// see: cans/02-plugin-model.md#Unified-builder
// Mount case 4, scoped form: the same verbs as build_flat_plugin, grouped
// into the 8 scope closures. Scope groups organise the chain only; ring
// membership and descriptors are unchanged by scope use.
pub fn build_scoped_plugin() -> KelirBuilder<Plugin, Unsealed, (), HNil, HNil, NoParams> {
    KelirBuilder::named("surface-scoped")
        .trap_scope(|t| {
            t.on_trap(|_, _| Ok(Verdict::Pass))
                .on_process_exit(|_, _| Ok(()))
                .end()
        })
        .fs(|f| {
            f.on_fs_lookup(|_| Ok(Verdict::Pass))
                .on_fs_read(|_, data| Ok(data.to_vec()))
                .on_fs_write(|_, _, _, _| Ok(FilterAction::Forward))
                .on_fs_lock(|_, _, _, _| Ok(Verdict::Pass))
                .end()
        })
        .net(|n| {
            n.on_net_outbound(|_| Ok(FilterAction::Forward))
                .on_net_inbound(|_| Ok(FilterAction::Forward))
                .end()
        })
        .console(|c| {
            c.on_console_input(|_, bytes| Ok(bytes.to_vec()))
                .on_console_output(|_, bytes| Ok(bytes.to_vec()))
                .end()
        })
        .exec(|e| {
            e.on_exec_stdout(|_, _, bytes| Ok(bytes.to_vec()))
                .on_exec_stderr(|_, _, bytes| Ok(bytes.to_vec()))
                .end()
        })
        .host(|h| {
            h.on_host_lsm_exec(|_, _| Ok(Verdict::Pass))
                .on_host_pressure(|_| Ok(Verdict::Pass))
                .on_host_keyring_access(|_| Ok(Verdict::Pass))
                .on_host_preempt(|_| Box::pin(async move { Ok::<(), PluginError>(()) }))
                .end()
        })
        .lifecycle(|l| {
            l.on_vm_init(|_| Box::pin(async move { Ok::<(), PluginError>(()) }))
                .on_vm_ready(|_| Box::pin(async move { Ok::<(), PluginError>(()) }))
                .on_vm_shutdown(|_| Box::pin(async move { Ok::<(), PluginError>(()) }))
                .on_pre_commit(|_| Box::pin(async move { Ok::<(), PluginError>(()) }))
                .on_post_commit(|_, _| Box::pin(async move { Ok::<(), PluginError>(()) }))
                .end()
        })
        .telemetry(|t| {
            t.on_memory_pressure(|_, _| async move { Ok::<(), PluginError>(()) })
                .on_heartbeat(|_| async move { Ok::<(), PluginError>(()) })
                .on_watchdog(|_| async move {
                    Ok::<WatchdogVerdict, PluginError>(WatchdogVerdict::ExecuteDefault)
                })
                .end()
        })
}

// see: cans/02-plugin-model.md#Unified-builder
// Mount case 4, flat form: the same 24 verbs as build_scoped_plugin, with
// no scope grouping. A nested chain and the equivalent flat chain produce
// identical descriptors: same rings, same ordinals.
pub fn build_flat_plugin() -> KelirBuilder<Plugin, Unsealed, (), HNil, HNil, NoParams> {
    KelirBuilder::named("surface-flat")
        .on_trap(|_, _| Ok(Verdict::Pass))
        .on_process_exit(|_, _| Ok(()))
        .on_fs_lookup(|_| Ok(Verdict::Pass))
        .on_fs_read(|_, data| Ok(data.to_vec()))
        .on_fs_write(|_, _, _, _| Ok(FilterAction::Forward))
        .on_fs_lock(|_, _, _, _| Ok(Verdict::Pass))
        .on_net_outbound(|_| Ok(FilterAction::Forward))
        .on_net_inbound(|_| Ok(FilterAction::Forward))
        .on_console_input(|_, bytes| Ok(bytes.to_vec()))
        .on_console_output(|_, bytes| Ok(bytes.to_vec()))
        .on_exec_stdout(|_, _, bytes| Ok(bytes.to_vec()))
        .on_exec_stderr(|_, _, bytes| Ok(bytes.to_vec()))
        .on_host_lsm_exec(|_, _| Ok(Verdict::Pass))
        .on_host_pressure(|_| Ok(Verdict::Pass))
        .on_host_keyring_access(|_| Ok(Verdict::Pass))
        .on_vm_init(|_| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_vm_ready(|_| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_vm_shutdown(|_| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_pre_commit(|_| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_post_commit(|_, _| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_host_preempt(|_| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_memory_pressure(|_, _| async move { Ok::<(), PluginError>(()) })
        .on_heartbeat(|_| async move { Ok::<(), PluginError>(()) })
        .on_watchdog(|_| async move {
            Ok::<WatchdogVerdict, PluginError>(WatchdogVerdict::ExecuteDefault)
        })
}

// see: cans/02-plugin-model.md#Unified-builder
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _sealed = build_host().seal()?;
    let _plugin = host_only_verbs_rejected_on_plugin(build_plain_plugin());
    let _scoped = build_scoped_plugin();
    let _flat = build_flat_plugin();
    Ok(())
}
