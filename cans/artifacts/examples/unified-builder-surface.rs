#![allow(dead_code)]

// see: cans/02-plugin-model.md#Unified-builder
// see: cans/02-plugin-model.md#Nested-scopes
// see: cans/03-sdk-architecture.md#Unified-builder-surface
// see: cans/artifacts/examples/fluent-builder-chaos.rs#build_chaos_plugin
// see: cans/artifacts/examples/unified-builder-surface.ts#buildSurfacePlugin
// see: cans/artifacts/examples/unified-builder-surface.py#build_surface_plugin

// The unified builder surface, walked end to end in one file: structural
// verbs, all 24 ring verbs reached through their 8 scope closures, the
// five orphan-verb scopes (actions, streams, vsock, params, deny), the
// chain cases (seal, trap filter, trap_scope, .fs config) and the mount
// cases (host-only verbs on a host builder, host-only verbs on a plugin
// builder, host.use(builder) merge ordering, flat-fold = nested-scope
// equivalence via group()). Handlers are trivial stubs: Forward for
// filter verbs, Pass for verdict verbs, passthrough bytes for
// console/exec, Ok(()) for lifecycle and telemetry. L1/L2: descriptors
// only, no execution, no boot().

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
// Post-mount type: only .plugin_instance() grows Plugins (prepending the
// plugin's Context); .plugin()/.r#use() merge the builder's registration
// and check Plugins: Satisfies<PDeps> without changing Plugins. After the
// three mounts below, Plugins = HCons<LedgerCtx, HNil> — the surface
// plugin's Deps HCons<LedgerCtx, HNil> is satisfied positionally by the
// ledger instance mounted first.
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

// see: cans/02-plugin-model.md#Nested-scopes
// Structural verbs: context, require, scope, fork_with. The vsock service
// and the 24 ring verbs are scope-reached: .vsock() carries the service,
// the 8 ring scopes carry the verbs. .end() below is a Rust type-system
// artifact, not a semantic one: the Rust closure must return the builder,
// and .end() is how it does it (see: #Chain-decisions (e)).
pub fn build_surface_plugin() -> SurfaceBuilder {
    KelirBuilder::named("surface")
        .context::<SurfaceCtx>(|| SurfaceCtx)
        .require::<LedgerPlugin>()
        .scope(MountScope::default().with_paths(["/workspace"]))
        .fork_with(|_id| Box::new(build_surface_plugin()) as Box<dyn AnyPlugin>)
        .vsock(|v| {
            v.service(
                VsockServiceSpec {
                    port: 0x0013,
                    service: "echo".to_string(),
                    schema: None,
                },
                EchoVsock,
            )
            .end()
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
    KelirBuilder::named("plain").telemetry(|t| {
        t.on_heartbeat(|_| async move { Ok::<(), PluginError>(()) }).end()
    })
}

// see: cans/02-plugin-model.md#Unified-builder
// Mount case 1: host-only verbs on a host builder (terminal, storage,
// wire, detached). The lock-broker config rides .fs(|f| f.config(…)).
// Chain cases: trap filter, trap_scope, .fs config, then seal() in main.
// Mount case 3: merge ordering — the ledger trait instance first (it
// satisfies the surface plugin's require at compile time via Satisfies),
// then the surface plugin via plugin(), then the plain plugin via use(),
// the same verb under its second name.
// The .plugin()/.r#use() mounts do not grow Plugins (they return Self and
// only check Satisfies against their Deps), so the declared return type
// stays HCons<LedgerCtx, HNil> — the single Context contributed by the
// ledger .plugin_instance() mount.

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
        synthetic_files: Vec::new(),
        mask_read: Vec::new(),
        encapsulation: WorkspaceEncapsulation::Passthrough,
        // lock_broker, carryover_bytes, scan_engine, and snoops ride
        // .fs(|f| f.config(…)) below; the StoragePlaneConfig literals keep
        // their live field set for the host face.
        lock_broker: true,
        scan_engine: ReadMaskEngine::FastRegex,
        carryover_bytes: 1024,
        snoops: Vec::new(),
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
    // see: cans/02-plugin-model.md#TrapFilter-predicates
    // The filter call site carries the full predicate chain (with_arg,
    // with_cwd, with_env, with_uid, argv_matches — AND-ed); the handler
    // body needs no cwd/argv guard.
    .trap(
        TrapFilter::binary("surface-allow")
            .with_arg("run")
            .with_cwd("/workspace/**")
            .with_env("CI", "1")
            .with_uid(0)
            .argv_matches(r"^--fast$"),
        |_, _| { Ok(Verdict::Pass) },
    )
    .trap_scope(|t| {
        t.on_trap(|_, _| Ok(Verdict::Pass))
            .on_process_exit(|_, _| Ok(()))
            .end()
    })
    // see: cans/02-plugin-model.md#Nested-scopes
    // The host-only fs scope sub-nodes: config carries lock_broker,
    // carryover_bytes, scan_engine, snoops; requires / fork_with /
    // timeout / priority / rules pin the shared scope shape. The
    // standalone lock_broker verb folds into this config.
    .fs(|f| {
        f.config(
            Some(LockBrokerConfig {
                scope: LockScope::Globs(vec![
                    "**/Cargo.lock".into(),
                    "**/package-lock.json".into(),
                ]),
                non_blocking: true,
            }),
            Some(1024),
            Some(ReadMaskEngine::FastRegex),
            Vec::new(),
        )
        .requires::<LedgerPlugin>()
        .fork_with(|_id| Box::new(build_plain_plugin()) as Box<dyn AnyPlugin>)
        .timeout(std::time::Duration::from_millis(5))
        .priority(0)
        .rules(Default::default())
        .on_fs_lookup(|_| Ok(Verdict::Pass))
        .end()
    })
    // see: cans/02-plugin-model.md#Nested-scopes
    // The five orphan-verb scopes on the host: actions, streams, vsock,
    // params, deny. The standalone register_custom_action,
    // register_stream_kind, vsock_service, params, on_trap_deny verbs
    // delete; these scopes are the one surface.
    .actions(|a| {
        a.register("quarantine", serde_json::json!({}), |payload| {
            async move {
                let _ = payload;
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            }
        })
        .end()
    })
    .streams(|s| {
        s.register(0x0013, serde_json::json!({}), |request| {
            async move {
                Ok::<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>(request)
            }
        })
        .end()
    })
    .vsock(|v| {
        v.service(
            VsockServiceSpec {
                port: 0x0013,
                service: "echo".to_string(),
                schema: None,
            },
            EchoVsock,
        )
        .end()
    })
    .params(|p| p.model::<NoParams>().end())
    .deny(|d| {
        d.on_trap_deny(|event, verdict, session| {
            let _ = (event, verdict, session);
            async move { Ok::<(), PluginError>(()) }
        })
        .end()
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
    // plugin.fs(|f| f.config(Some(LockBrokerConfig { scope: LockScope::Globs(vec!["**/Cargo.lock".into()]), non_blocking: true }), None, None, Vec::new()));
    // plugin.detached(true);
    // plugin.seal();
    // plugin.params(|p| p.model::<NoParams>().end());
    // plugin.plugin(build_plain_plugin());
    // plugin.r#use(build_plain_plugin());
    // plugin.plugin_instance(LedgerPlugin::new(true));
    // plugin.trap_async("*", |_| async move { Ok::<Verdict, PluginError>(Verdict::Pass) });
    // plugin.post_exec("*", |_| async move { Ok::<PostExecAction, PluginError>(PostExecAction::Proceed) });
    // plugin.deny(|d| d.on_trap_deny(|_, _, _| async move { Ok::<(), PluginError>(()) }).end());
    // plugin.actions(|a| a.register("quarantine", serde_json::json!({}), |payload| async move { Ok::<(), Box<dyn std::error::Error + Send + Sync>>(()) }).end());
    // plugin.streams(|s| s.register(0x0013, serde_json::json!({}), |request| async move { Ok::<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>(request) }).end());
    // plugin.lifecycle_mut();
    plugin
}

// see: cans/02-plugin-model.md#Nested-scopes
// Negative vector: f.on_trap(...) inside .fs(...) is a compile error in
// Rust — FsScope has no on_trap method. Uncommenting the body must fail
// the build with E0599 (no method named `on_trap` found for `FsScope`).
// vectors.json cannot express an expected-error case (its schema carries
// event/expected_verdict only), so the negative case is pinned here as a
// comment-pinned block and in the TS/Py siblings.
// fn negative_fs_on_trap(
//     builder: KelirBuilder<Plugin, Unsealed, (), HNil, HNil, NoParams>,
// ) -> KelirBuilder<Plugin, Unsealed, (), HNil, HNil, NoParams> {
//     builder.fs(|f| f.on_trap(|_, _| Ok(Verdict::Pass)).end())
// }

// see: cans/02-plugin-model.md#Nested-scopes
// Flat-fold pin: a flat chain exists only as a group() fold over the
// scopes — a readability fold that produces the same descriptors (same
// rings, same ordinals). There is no separate flat surface: no
// build_flat_plugin exists, the fold delegates each verb to its scope.
pub fn build_flat_fold_plugin() -> KelirBuilder<Plugin, Unsealed, (), HNil, HNil, NoParams> {
    KelirBuilder::named("surface-flat-fold").group(|g| {
        g.trap_scope(|t| {
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
    })
}

// see: cans/02-plugin-model.md#Unified-builder
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _sealed = build_host().seal()?;
    let _plugin = host_only_verbs_rejected_on_plugin(build_plain_plugin());
    let _flat_fold = build_flat_fold_plugin();
    Ok(())
}
