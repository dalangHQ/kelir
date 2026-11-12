// see: cans/02-plugin-model.md#KelirPlugin-trait
use std::path::Path;
#[cfg(feature = "plugin-ledger")]
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use kelir::interface::driver_interface::{Stream, WatchdogVerdict, default_vm_config};
use kelir::interface::plugin_model::{
    ConsoleInterposer, FsInterposer, HandlerCtx, HostInterposer, KelirPlugin, LifecycleParticipant,
    MountScope, NetInterposer, ParkedConnectionReset, PipelineBinder, PluginError, PostExecGate,
    StreamFrameSink, SystemTelemetryParticipant, TokenRevocation, TrapInterposer,
};
use kelir::interface::protocol_wire::{StreamKind, Verdict};
use kelir::interface::vsock_mesh::{VsockPeer, VsockServiceHandler, VsockServiceSpec};
use kelir::plugin_model::FilterAction;
use kelir::{HNil, Host, KelirBuilder, NoParams, ProfilePlugin, TrapFilter, Unsealed};

#[cfg(feature = "plugin-ledger")]
use kelir::LedgerPlugin;
#[cfg(feature = "plugin-chaos")]
use plugin_chaos::{ChaosConfig, ChaosPlugin};

struct _NullVsock;

#[async_trait]
impl VsockServiceHandler for _NullVsock {
    async fn on_connect(
        &self,
        _peer: &VsockPeer,
        _stream: Box<dyn Stream>,
        _ctx: &HandlerCtx,
    ) -> Result<(), PluginError> {
        Ok(())
    }
}

#[derive(Default)]
struct RecordingBinder {
    binds: Vec<String>,
    fs_stages: Vec<Arc<dyn FsInterposer>>,
}

impl PipelineBinder for RecordingBinder {
    fn bind_trap_scoped(&mut self, _stage: Arc<dyn TrapInterposer>, _scope: MountScope) {
        self.binds.push("trap".to_string());
    }

    fn bind_fs_scoped(&mut self, stage: Arc<dyn FsInterposer>, _scope: MountScope) {
        self.binds.push("fs".to_string());
        self.fs_stages.push(stage);
    }

    fn bind_net_scoped(&mut self, _stage: Arc<dyn NetInterposer>, _scope: MountScope) {
        self.binds.push("net".to_string());
    }

    fn bind_console_scoped(&mut self, _stage: Arc<dyn ConsoleInterposer>, _scope: MountScope) {
        self.binds.push("console".to_string());
    }

    fn bind_host_scoped(&mut self, _stage: Arc<dyn HostInterposer>, _scope: MountScope) {
        self.binds.push("host".to_string());
    }

    fn bind_vsock_service_scoped(
        &mut self,
        spec: VsockServiceSpec,
        _service: Arc<dyn VsockServiceHandler>,
        _scope: MountScope,
    ) {
        self.binds
            .push(format!("vsock:{}:{}", spec.port, spec.service));
    }

    fn bind_lifecycle_scoped(&mut self, _stage: Arc<dyn LifecycleParticipant>, _scope: MountScope) {
        self.binds.push("lifecycle".to_string());
    }

    fn bind_telemetry_scoped(
        &mut self,
        _stage: Arc<dyn SystemTelemetryParticipant>,
        _scope: MountScope,
    ) {
        self.binds.push("telemetry".to_string());
    }

    fn bind_post_exec_scoped(&mut self, _gate: Arc<dyn PostExecGate>, _scope: MountScope) {
        self.binds.push("post_exec".to_string());
    }

    fn bind_stream_kind_scoped(
        &mut self,
        _kind: StreamKind,
        _sink: Arc<dyn StreamFrameSink>,
        _scope: MountScope,
    ) {
    }

    fn bind_parked_connection_reset(&mut self, _reset: Arc<dyn ParkedConnectionReset>) {}

    fn bind_token_revoker(&mut self, _revoker: Arc<dyn TokenRevocation>) {}
}

fn error_text<T>(result: Result<T, PluginError>) -> String {
    match result {
        Err(error) => error.to_string(),
        Ok(_) => String::new(),
    }
}

fn test_builder() -> KelirBuilder<Host, Unsealed, (), HNil, HNil, NoParams> {
    KelirBuilder::new(default_vm_config(
        None,
        None,
        Some("kernel/vmlinux".to_string()),
        Some("rootfs.erofs".to_string()),
        None,
        None,
    ))
}

// see: cans/02-plugin-model.md#KelirPlugin-trait
#[test]
fn full_flat_chain_builds_and_binds() -> Result<(), PluginError> {
    let plain = KelirBuilder::named("plain")
        .require::<ProfilePlugin>()
        .context(|| ())
        .scope(MountScope::default())
        .fork_with(|_child| Box::new(ProfilePlugin::new()))
        .on_trap(|_event, _ctx| Ok(Verdict::Pass))
        .on_process_exit(|_event, _ctx| Ok(()))
        .on_fs_lookup(|_path| Ok(Verdict::Pass))
        .on_fs_read(|_path, buffer| Ok(buffer.to_vec()))
        .on_fs_write(|_path, _data, _pid, _ctx| Ok(FilterAction::Forward))
        .on_fs_lock(|_path, _cmd, _kind, _pid| Ok(Verdict::Pass))
        .on_net_outbound(|_frame| Ok(FilterAction::Forward))
        .on_net_inbound(|_frame| Ok(FilterAction::Forward))
        .on_console_input(|_port, bytes| Ok(bytes.to_vec()))
        .on_console_output(|_port, chunk| Ok(chunk.to_vec()))
        .on_exec_stdout(|_pid, _exec_id, chunk| Ok(chunk.to_vec()))
        .on_exec_stderr(|_pid, _exec_id, chunk| Ok(chunk.to_vec()))
        .on_host_lsm_exec(|_path, _pid| Ok(Verdict::Pass))
        .on_host_pressure(|_level| Ok(Verdict::Pass))
        .on_host_keyring_access(|_key| Ok(Verdict::Pass))
        .on_vm_init(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_vm_ready(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_vm_shutdown(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_pre_commit(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_post_commit(|_ctx, _committed| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_host_preempt(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_memory_pressure(|_available, _total| async move { Ok::<(), PluginError>(()) })
        .on_heartbeat(|_uptime| async move { Ok::<(), PluginError>(()) })
        .on_watchdog(|_event| async move {
            Ok::<WatchdogVerdict, PluginError>(WatchdogVerdict::ExecuteDefault)
        })
        .vsock_service(
            VsockServiceSpec {
                port: 5252,
                service: "plain".to_string(),
                schema: None,
            },
            _NullVsock,
        );
    let mut binder = RecordingBinder::default();
    plain.register_interceptors(&mut binder);
    assert_eq!(
        binder.binds,
        vec![
            "trap".to_string(),
            "trap".to_string(),
            "fs".to_string(),
            "fs".to_string(),
            "fs".to_string(),
            "fs".to_string(),
            "net".to_string(),
            "net".to_string(),
            "console".to_string(),
            "console".to_string(),
            "console".to_string(),
            "console".to_string(),
            "host".to_string(),
            "host".to_string(),
            "host".to_string(),
            "vsock:5252:plain".to_string(),
            "lifecycle".to_string(),
            "lifecycle".to_string(),
            "lifecycle".to_string(),
            "lifecycle".to_string(),
            "lifecycle".to_string(),
            "lifecycle".to_string(),
            "telemetry".to_string(),
            "telemetry".to_string(),
            "telemetry".to_string(),
        ]
    );
    let _builder = test_builder()
        .plugin_instance(ProfilePlugin::new())
        .plugin(plain);
    Ok(())
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
#[test]
fn nested_scopes_match_flat_descriptors() {
    let flat = KelirBuilder::named("flat")
        .on_trap(|_event, _ctx| Ok(Verdict::Pass))
        .on_process_exit(|_event, _ctx| Ok(()))
        .on_fs_lookup(|_path| Ok(Verdict::Pass))
        .on_fs_read(|_path, buffer| Ok(buffer.to_vec()))
        .on_fs_write(|_path, _data, _pid, _ctx| Ok(FilterAction::Forward))
        .on_fs_lock(|_path, _cmd, _kind, _pid| Ok(Verdict::Pass))
        .on_net_outbound(|_frame| Ok(FilterAction::Forward))
        .on_net_inbound(|_frame| Ok(FilterAction::Forward))
        .on_console_input(|_port, bytes| Ok(bytes.to_vec()))
        .on_console_output(|_port, chunk| Ok(chunk.to_vec()))
        .on_exec_stdout(|_pid, _exec_id, chunk| Ok(chunk.to_vec()))
        .on_exec_stderr(|_pid, _exec_id, chunk| Ok(chunk.to_vec()))
        .on_host_lsm_exec(|_path, _pid| Ok(Verdict::Pass))
        .on_host_pressure(|_level| Ok(Verdict::Pass))
        .on_host_keyring_access(|_key| Ok(Verdict::Pass))
        .on_host_preempt(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_vm_init(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_vm_ready(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_vm_shutdown(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_pre_commit(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_post_commit(|_ctx, _committed| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_memory_pressure(|_available, _total| async move { Ok::<(), PluginError>(()) })
        .on_heartbeat(|_uptime| async move { Ok::<(), PluginError>(()) })
        .on_watchdog(|_event| async move {
            Ok::<WatchdogVerdict, PluginError>(WatchdogVerdict::ExecuteDefault)
        });
    let nested = KelirBuilder::named("nested")
        .trap_scope(|t| {
            t.on_trap(|_event, _ctx| Ok(Verdict::Pass))
                .on_process_exit(|_event, _ctx| Ok(()))
                .end()
        })
        .fs(|f| {
            f.on_fs_lookup(|_path| Ok(Verdict::Pass))
                .on_fs_read(|_path, buffer| Ok(buffer.to_vec()))
                .on_fs_write(|_path, _data, _pid, _ctx| Ok(FilterAction::Forward))
                .on_fs_lock(|_path, _cmd, _kind, _pid| Ok(Verdict::Pass))
                .end()
        })
        .net(|n| {
            n.on_net_outbound(|_frame| Ok(FilterAction::Forward))
                .on_net_inbound(|_frame| Ok(FilterAction::Forward))
                .end()
        })
        .console(|c| {
            c.on_console_input(|_port, bytes| Ok(bytes.to_vec()))
                .on_console_output(|_port, chunk| Ok(chunk.to_vec()))
                .end()
        })
        .exec(|e| {
            e.on_exec_stdout(|_pid, _exec_id, chunk| Ok(chunk.to_vec()))
                .on_exec_stderr(|_pid, _exec_id, chunk| Ok(chunk.to_vec()))
                .end()
        })
        .host(|h| {
            h.on_host_lsm_exec(|_path, _pid| Ok(Verdict::Pass))
                .on_host_pressure(|_level| Ok(Verdict::Pass))
                .on_host_keyring_access(|_key| Ok(Verdict::Pass))
                .on_host_preempt(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
                .end()
        })
        .lifecycle(|l| {
            l.on_vm_init(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
                .on_vm_ready(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
                .on_vm_shutdown(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
                .on_pre_commit(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
                .on_post_commit(|_ctx, _committed| {
                    Box::pin(async move { Ok::<(), PluginError>(()) })
                })
                .end()
        })
        .telemetry(|t| {
            t.on_memory_pressure(|_available, _total| async move { Ok::<(), PluginError>(()) })
                .on_heartbeat(|_uptime| async move { Ok::<(), PluginError>(()) })
                .on_watchdog(|_event| async move {
                    Ok::<WatchdogVerdict, PluginError>(WatchdogVerdict::ExecuteDefault)
                })
                .end()
        });
    let mut flat_binder = RecordingBinder::default();
    flat.register_interceptors(&mut flat_binder);
    let mut nested_binder = RecordingBinder::default();
    nested.register_interceptors(&mut nested_binder);
    assert_eq!(nested_binder.binds, flat_binder.binds);
    let mixed = KelirBuilder::named("mixed")
        .on_trap(|_event, _ctx| Ok(Verdict::Pass))
        .trap_scope(|t| t.on_process_exit(|_event, _ctx| Ok(())).end())
        .on_trap(|_event, _ctx| Ok(Verdict::Pass));
    let mut mixed_binder = RecordingBinder::default();
    mixed.register_interceptors(&mut mixed_binder);
    assert_eq!(
        mixed_binder.binds,
        vec!["trap".to_string(), "trap".to_string(), "trap".to_string(),]
    );
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
#[test]
fn seal_rejects_bad_vsock_spec() -> Result<(), PluginError> {
    let port_zero = test_builder()
        .plugin(KelirBuilder::named("bad-port").vsock_service(
            VsockServiceSpec {
                port: 0,
                service: "plain".to_string(),
                schema: None,
            },
            _NullVsock,
        ))
        .seal();
    let text = error_text(port_zero);
    assert!(
        text.contains("EConfig")
            && text.contains("bad-port")
            && text.contains("out of range 1..=65535"),
        "unexpected error: {text}"
    );
    let port_high = test_builder()
        .plugin(KelirBuilder::named("bad-port").vsock_service(
            VsockServiceSpec {
                port: 65_536,
                service: "plain".to_string(),
                schema: None,
            },
            _NullVsock,
        ))
        .seal();
    let text = error_text(port_high);
    assert!(
        text.contains("EConfig")
            && text.contains("bad-port")
            && text.contains("out of range 1..=65535"),
        "unexpected error: {text}"
    );
    let empty_service = test_builder()
        .plugin(KelirBuilder::named("bad-service").vsock_service(
            VsockServiceSpec {
                port: 5252,
                service: String::new(),
                schema: None,
            },
            _NullVsock,
        ))
        .seal();
    let text = error_text(empty_service);
    assert!(
        text.contains("EConfig")
            && text.contains("bad-service")
            && text.contains("service name must not be empty"),
        "unexpected error: {text}"
    );
    let host_bad = test_builder()
        .vsock_service(
            VsockServiceSpec {
                port: 0,
                service: "plain".to_string(),
                schema: None,
            },
            _NullVsock,
        )
        .seal();
    let text = error_text(host_bad);
    assert!(
        text.contains("EConfig")
            && text.contains("vsock service port 0 out of range 1..=65535")
            && !text.contains("host"),
        "unexpected error: {text}"
    );
    let ok = test_builder()
        .plugin(KelirBuilder::named("ok").vsock_service(
            VsockServiceSpec {
                port: 65_535,
                service: "plain".to_string(),
                schema: None,
            },
            _NullVsock,
        ))
        .seal();
    assert!(ok.is_ok());
    Ok(())
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
#[test]
fn seal_rejects_ill_formed_scope() -> Result<(), PluginError> {
    let empty_binary = test_builder()
        .plugin(KelirBuilder::named("bad-scope").scope(MountScope::default().with_binaries([""])))
        .seal();
    let text = error_text(empty_binary);
    assert!(
        text.contains("EConfig")
            && text.contains("bad-scope")
            && text.contains("binary name must not be empty"),
        "unexpected error: {text}"
    );
    let empty_path = test_builder()
        .plugin(KelirBuilder::named("bad-scope").scope(MountScope::default().with_paths([""])))
        .seal();
    let text = error_text(empty_path);
    assert!(
        text.contains("EConfig")
            && text.contains("bad-scope")
            && text.contains("path must not be empty"),
        "unexpected error: {text}"
    );
    let ok = test_builder()
        .plugin(KelirBuilder::named("ok").scope(MountScope::default().with_paths(["/workspace"])))
        .seal();
    assert!(ok.is_ok());
    Ok(())
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
#[test]
fn duplicate_handlers_append_in_order() {
    let plugin = KelirBuilder::named("dup")
        .on_fs_write(|_path, _data, _pid, _ctx| Ok(FilterAction::Forward))
        .on_fs_write(|_path, _data, _pid, _ctx| {
            Ok(FilterAction::Drop {
                errno: None,
                reason: "second".to_string(),
            })
        })
        .on_trap(|_event, _ctx| Ok(Verdict::Pass))
        .on_trap(|_event, _ctx| Ok(Verdict::Pass));
    let mut binder = RecordingBinder::default();
    plugin.register_interceptors(&mut binder);
    assert_eq!(
        binder.binds,
        vec![
            "trap".to_string(),
            "trap".to_string(),
            "fs".to_string(),
            "fs".to_string(),
        ]
    );
    assert_eq!(binder.fs_stages.len(), 2);
    let ctx = HandlerCtx::new([0u8; 16], 42, (), Vec::new());
    let mut data = Vec::new();
    let first = binder.fs_stages[0].on_fs_write(Path::new("/workspace/x"), &mut data, 1, &ctx);
    assert!(matches!(first, Ok(FilterAction::Forward)));
    let second = binder.fs_stages[1].on_fs_write(Path::new("/workspace/x"), &mut data, 1, &ctx);
    assert!(matches!(second, Ok(FilterAction::Drop { .. })));
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
#[test]
fn multi_vsock_registers_in_order() {
    let plugin = KelirBuilder::named("multi-vsock")
        .vsock_service(
            VsockServiceSpec {
                port: 1111,
                service: "one".to_string(),
                schema: None,
            },
            _NullVsock,
        )
        .vsock_service(
            VsockServiceSpec {
                port: 2222,
                service: "two".to_string(),
                schema: None,
            },
            _NullVsock,
        )
        .vsock_service(
            VsockServiceSpec {
                port: 3333,
                service: "three".to_string(),
                schema: None,
            },
            _NullVsock,
        );
    let mut binder = RecordingBinder::default();
    plugin.register_interceptors(&mut binder);
    assert_eq!(
        binder.binds,
        vec![
            "vsock:1111:one".to_string(),
            "vsock:2222:two".to_string(),
            "vsock:3333:three".to_string(),
        ]
    );
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
#[cfg(feature = "plugin-ledger")]
#[test]
fn canonical_chaos_shape() -> Result<(), PluginError> {
    let chaos = KelirBuilder::named("chaos")
        .require::<LedgerPlugin>()
        .scope(MountScope::default().with_paths(["/workspace"]))
        .fs(|f| {
            f.on_fs_write(|_path, _data, _pid, _ctx| Ok(FilterAction::Forward))
                .end()
        })
        .net(|n| n.on_net_outbound(|_frame| Ok(FilterAction::Forward)).end());
    let scope = KelirPlugin::default_scope(&chaos);
    assert_eq!(scope.paths, Some(vec![PathBuf::from("/workspace")]));
    let builder = test_builder()
        .plugin_instance(LedgerPlugin::new(false))
        .plugin(chaos);
    let mut binder = RecordingBinder::default();
    builder.bind_registry(&mut binder);
    assert_eq!(binder.binds, vec!["fs".to_string(), "net".to_string()]);
    Ok(())
}

// see: cans/plugins/chaos.md#Plugin-chaos
#[cfg(feature = "plugin-chaos")]
#[test]
fn chaos_plugin_wraps_into_builder() {
    let plugin = ChaosPlugin::new(ChaosConfig::default());
    let context_plugin = plugin.clone();
    let net_plugin = plugin.clone();
    let fs_plugin = plugin.clone();
    let chaos = KelirBuilder::named("chaos")
        .context(move || context_plugin.context())
        .scope(MountScope::default())
        .on_net_outbound(move |frame| net_plugin.on_net_outbound(frame))
        .on_fs_write(move |path, data, pid, ctx| fs_plugin.on_fs_write(path, data, pid, ctx));
    let mut binder = RecordingBinder::default();
    chaos.register_interceptors(&mut binder);
    assert_eq!(binder.binds, vec!["fs".to_string(), "net".to_string()]);
}

// see: cans/02-plugin-model.md#Unified-builder
#[test]
fn host_ring_verbs_bind_before_mounted() -> Result<(), PluginError> {
    let plugin =
        KelirBuilder::named("net-plugin").on_net_outbound(|_frame| Ok(FilterAction::Forward));
    let host = test_builder()
        .on_fs_write(|_path, _data, _pid, _ctx| Ok(FilterAction::Forward))
        .on_vm_shutdown(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .plugin(plugin);
    let mut binder = RecordingBinder::default();
    host.bind_registry(&mut binder);
    assert_eq!(
        binder.binds,
        vec!["fs".to_string(), "lifecycle".to_string(), "net".to_string(),]
    );
    Ok(())
}

// see: cans/02-plugin-model.md#Unified-builder
#[test]
fn mount_rejects_already_mounted() -> Result<(), PluginError> {
    let host = test_builder().plugin(KelirBuilder::named("dup"));
    let text = error_text(host.plugin(KelirBuilder::named("dup")).seal());
    assert!(
        text.contains("EConfig") && text.contains("plugin already mounted: dup"),
        "unexpected error: {text}"
    );
    Ok(())
}

// see: cans/02-plugin-model.md#Unified-builder
#[test]
fn empty_host_seals() {
    assert!(test_builder().seal().is_ok());
}

// see: cans/02-plugin-model.md#Filter-before-FFI-gate
#[test]
fn host_trap_filter_registers_on_engine_ring() -> Result<(), PluginError> {
    let host = test_builder().trap("git", |_event, _ctx| Ok(Verdict::Pass));
    let mut binder = RecordingBinder::default();
    host.bind_registry(&mut binder);
    assert_eq!(binder.binds, vec!["trap".to_string()]);
    let empty_binary = test_builder()
        .trap("", |_event, _ctx| Ok(Verdict::Pass))
        .seal();
    let text = error_text(empty_binary);
    assert!(
        text.contains("EConfig") && text.contains("trap filter binary must not be empty"),
        "unexpected error: {text}"
    );
    let empty_arg = test_builder()
        .trap(TrapFilter::binary("git").with_arg(""), |_event, _ctx| {
            Ok(Verdict::Pass)
        })
        .seal();
    let text = error_text(empty_arg);
    assert!(
        text.contains("EConfig") && text.contains("trap filter arg must not be empty"),
        "unexpected error: {text}"
    );
    Ok(())
}

// see: cans/02-plugin-model.md#Unified-builder
#[test]
fn trait_plugin_still_mounts() {
    let host = test_builder().plugin_instance(ProfilePlugin::new());
    assert!(host.seal().is_ok());
}
